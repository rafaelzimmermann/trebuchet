use iced::{
    event,
    event::Status,
    mouse,
    widget::{container, mouse_area},
    Background, Border, Element, Event, Length, Subscription, Task,
};
use iced_layershell::to_layer_message;

use crate::components::app_launcher::{self, AppLauncher};
use crate::components::cmd::{self, Cmd};
use crate::components::command::{ComponentEvent, SlashCommand};
use crate::components::component::Component;
use crate::components::settings::{self, Settings};
use crate::components::window_mover::{self, WindowMover};
use crate::config::Config;
use crate::launcher::{scan_applications, AppEntry};
// ── Active component ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveComponent {
    Launcher,
    Cmd,
    Settings,
    WindowMover,
}

// ── App state ─────────────────────────────────────────────────────────────────

pub struct Trebuchet {
    pub apps: Vec<AppEntry>,
    pub config: Config,
    pub active: ActiveComponent,
    pub launcher: AppLauncher,
    pub cmd: Cmd,
    pub settings: Settings,
    pub window_mover: WindowMover,
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    Close,
    /// Absorbs clicks anywhere inside the window so they don't propagate as Ignored.
    Absorb,
    AppsLoaded(Vec<AppEntry>),
    /// Delivered when the lazy icon-resolution task finishes. The grid has
    /// been visible with fallback icons since `AppsLoaded`; this swaps in the
    /// real `IconHandle`s without rearranging the apps.
    IconsLoaded(Vec<Option<crate::icons::IconHandle>>),
    /// Delivered when the async `Config::load` started in `boot` completes.
    /// The initial frame is rendered with `Config::default()` so the window
    /// can appear immediately; this swaps in the user's real config.
    ConfigLoaded(Config),
    IcedEvent(Event, Status),
    Launcher(app_launcher::Msg),
    Cmd(cmd::Msg),
    Settings(settings::Msg),
    WindowMover(window_mover::Msg),
}

// ── Boot ──────────────────────────────────────────────────────────────────────

pub fn boot() -> (Trebuchet, Task<Message>) {
    // Start with the default config so the window can appear immediately.
    // The real `Config::load()` runs in parallel with `scan_applications`
    // and replaces this placeholder via `Message::ConfigLoaded` as soon as it
    // completes. This matters at cold boot where reading trebuchet.conf +
    // current-theme + themes/<name>.conf from cold disk can take 30–100 ms.
    let state = Trebuchet {
        apps: Vec::new(),
        config: Config::default(),
        active: ActiveComponent::Launcher,
        launcher: AppLauncher::new(&[]),
        cmd: Cmd::new(),
        settings: Settings::new(),
        window_mover: WindowMover::new(),
    };
    let task = Task::batch([
        Task::perform(
            async {
                tokio::task::spawn_blocking(scan_applications)
                    .await
                    .unwrap_or_default()
            },
            Message::AppsLoaded,
        ),
        Task::perform(
            async {
                tokio::task::spawn_blocking(Config::load)
                    .await
                    .unwrap_or_default()
            },
            Message::ConfigLoaded,
        ),
    ]);
    (state, task)
}

pub fn namespace() -> String {
    "trebuchet".into()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn persist_theme(name: &str) {
    let Some(dir) = std::env::var("HOME")
        .ok()
        .map(|h| std::path::PathBuf::from(h).join(".config/trebuchet"))
    else {
        return;
    };
    let _ = std::fs::write(dir.join("current-theme"), name);
}

// ── Event application ─────────────────────────────────────────────────────────

fn apply_event(state: &mut Trebuchet, event: ComponentEvent) -> Task<Message> {
    match event {
        ComponentEvent::Handled => {}
        ComponentEvent::Exit => std::process::exit(0),

        ComponentEvent::ThemeChanged(name, theme) => {
            state.config.theme = *theme;
            persist_theme(&name);
        }

        ComponentEvent::CommandInvoked(SlashCommand::App, _) => {
            state.active = ActiveComponent::Launcher;
            state.launcher.reset(&state.apps);
        }
        ComponentEvent::CommandInvoked(SlashCommand::Config, _) => {
            state.active = ActiveComponent::Settings;
            state.settings.reset();
        }
        ComponentEvent::CommandInvoked(SlashCommand::Cmd, _) => {
            state.active = ActiveComponent::Cmd;
            state.cmd.reset(&state.config);
        }
        ComponentEvent::CommandInvoked(SlashCommand::Mv, args) => {
            state.active = ActiveComponent::WindowMover;
            let task = state.window_mover.reset(args);
            return task.map(Message::WindowMover);
        }
        ComponentEvent::CommandInvoked(SlashCommand::Unknown(_), _) => {}
    }
    Task::none()
}

// ── Update ────────────────────────────────────────────────────────────────────

pub fn update(state: &mut Trebuchet, msg: Message) -> Task<Message> {
    match msg {
        Message::Close => std::process::exit(0),
        Message::Absorb => {}

        Message::AppsLoaded(apps) => {
            state.launcher.reset(&apps);
            state.apps = apps;
            // Dispatch lazy icon resolution. The launcher is already fully
            // usable (names + execs are known); icons stream in once the task
            // completes via Message::IconsLoaded.
            let apps_snapshot = state.apps.clone();
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        crate::launcher::resolve_all_icons(&apps_snapshot)
                    })
                    .await
                    .unwrap_or_default()
                },
                Message::IconsLoaded,
            );
        }

        Message::IconsLoaded(icons) => {
            // Apply each resolved icon to its app in-place. Length may differ
            // if a re-scan raced (unlikely in practice); only update what we can.
            for (idx, icon) in icons.into_iter().enumerate() {
                if let Some(app) = state.apps.get_mut(idx) {
                    app.icon = icon;
                }
            }
        }

        Message::ConfigLoaded(config) => {
            state.config = config;
            // Cmd builds its filter from config.commands. If the user
            // navigated to /cmd before this message arrived, the filter was
            // built from the empty default — rebuild it now.
            if matches!(state.active, ActiveComponent::Cmd) {
                state.cmd.reset(&state.config);
            }
        }

        Message::Launcher(m) => {
            let (task, evt) = state.launcher.update(m, &state.apps, &state.config);
            let evt_task = apply_event(state, evt);
            return Task::batch([task.map(Message::Launcher), evt_task]);
        }
        Message::Cmd(m) => {
            let (task, evt) = state.cmd.update(m, &state.apps, &state.config);
            let evt_task = apply_event(state, evt);
            return Task::batch([task.map(Message::Cmd), evt_task]);
        }
        Message::Settings(m) => {
            let (task, evt) = state.settings.update(m, &state.apps, &state.config);
            let evt_task = apply_event(state, evt);
            return Task::batch([task.map(Message::Settings), evt_task]);
        }
        Message::WindowMover(m) => {
            let (task, evt) = state.window_mover.update(m, &state.apps, &state.config);
            let evt_task = apply_event(state, evt);
            return Task::batch([task.map(Message::WindowMover), evt_task]);
        }

        Message::IcedEvent(event, status) => {
            let (task, evt) = match state.active {
                ActiveComponent::Launcher => {
                    let (t, e) = state.launcher.handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::Launcher), e)
                }
                ActiveComponent::Cmd => {
                    let (t, e) = state.cmd.handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::Cmd), e)
                }
                ActiveComponent::Settings => {
                    let (t, e) = state.settings.handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::Settings), e)
                }
                ActiveComponent::WindowMover => {
                    let (t, e) = state.window_mover.handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::WindowMover), e)
                }
            };
            let evt_task = apply_event(state, evt);
            return Task::batch([task, evt_task]);
        }

        // Extra variants injected by #[to_layer_message] (layershell protocol messages).
        _ => {}
    }
    Task::none()
}

// ── View ──────────────────────────────────────────────────────────────────────

pub fn view(state: &Trebuchet) -> Element<'_, Message> {
    let content = match state.active {
        ActiveComponent::Launcher => {
            state.launcher.view(&state.apps, &state.config).map(Message::Launcher)
        }
        ActiveComponent::Cmd => {
            state.cmd.view(&state.apps, &state.config).map(Message::Cmd)
        }
        ActiveComponent::Settings => {
            state.settings.view(&state.apps, &state.config).map(Message::Settings)
        }
        ActiveComponent::WindowMover => {
            state.window_mover.view(&state.apps, &state.config).map(Message::WindowMover)
        }
    };

    let bg = state.config.theme.background;
    container(mouse_area(content).on_press(Message::Absorb))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border { radius: 16.0.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

// ── Event handler ─────────────────────────────────────────────────────────────

fn on_event(event: Event, status: Status, _id: iced::window::Id) -> Option<Message> {
    match &event {
        Event::Mouse(mouse::Event::CursorLeft) => Some(Message::Close),
        // Margin clicks (outside the content area but inside the window) produce
        // Status::Ignored because the mouse_area in view() only wraps the content.
        Event::Mouse(mouse::Event::ButtonPressed(_)) if status == Status::Ignored => {
            Some(Message::Close)
        }
        Event::Keyboard(_) => Some(Message::IcedEvent(event, status)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use iced::mouse;

    #[test]
    fn cursor_left_closes_launcher() {
        let result = on_event(
            Event::Mouse(mouse::Event::CursorLeft),
            Status::Ignored,
            iced::window::Id::unique(),
        );
        assert!(matches!(result, Some(Message::Close)));
    }

    #[test]
    fn margin_click_closes_launcher() {
        // Status::Ignored means the click landed in the padding margin, not the content.
        let result = on_event(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Status::Ignored,
            iced::window::Id::unique(),
        );
        assert!(matches!(result, Some(Message::Close)));
    }

    #[test]
    fn captured_click_does_not_close() {
        // Status::Captured means a widget (or the content mouse_area) handled the click.
        let result = on_event(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Status::Captured,
            iced::window::Id::unique(),
        );
        assert!(result.is_none());
    }

    // ── ConfigLoaded ──────────────────────────────────────────────────────────

    fn test_state() -> Trebuchet {
        Trebuchet {
            apps: Vec::new(),
            config: Config::default(),
            active: ActiveComponent::Launcher,
            launcher: AppLauncher::new(&[]),
            cmd: Cmd::new(),
            settings: Settings::new(),
            window_mover: WindowMover::new(),
        }
    }

    #[test]
    fn config_loaded_updates_state_config() {
        let mut state = test_state();
        let custom = Config {
            columns: 11,
            rows: 7,
            icon_size: 48,
            commands: Vec::new(),
            theme: Theme::default(),
        };
        let _ = update(&mut state, Message::ConfigLoaded(custom));
        assert_eq!(state.config.columns, 11);
        assert_eq!(state.config.rows, 7);
        assert_eq!(state.config.icon_size, 48);
    }

    #[test]
    fn config_loaded_resets_cmd_when_cmd_is_active() {
        // Regression: if the user opens /cmd before ConfigLoaded arrives, the
        // command list would stay empty forever. ConfigLoaded must rebuild it.
        use crate::config::CustomCommand;
        let mut state = test_state();
        state.active = ActiveComponent::Cmd;
        state.cmd = Cmd::new(); // empty filter (no commands in default config)

        let custom = Config {
            columns: 7,
            rows: 5,
            icon_size: 96,
            commands: vec![CustomCommand {
                prefix: "hi".to_string(),
                command: "echo hi".to_string(),
                display_result: false,
            }],
            theme: Theme::default(),
        };
        let _ = update(&mut state, Message::ConfigLoaded(custom));

        assert_eq!(state.config.commands.len(), 1);
        assert_eq!(state.cmd.filtered.len(), 1, "Cmd filter should be rebuilt");
    }

    #[test]
    fn config_loaded_does_not_touch_cmd_when_other_component_active() {
        // No spurious reset when the user is in the launcher.
        let mut state = test_state();
        state.active = ActiveComponent::Launcher;

        let mut cmd = Cmd::new();
        cmd.query = "partial".to_string(); // pretend user is typing in another panel
        state.cmd = cmd;

        let _ = update(
            &mut state,
            Message::ConfigLoaded(Config::default()),
        );
        assert_eq!(state.cmd.query, "partial", "Cmd state must be untouched");
    }

    // ── IconsLoaded ───────────────────────────────────────────────────────────

    fn app_entry(name: &str) -> crate::launcher::AppEntry {
        crate::launcher::AppEntry {
            name: name.to_string(),
            exec: format!("{name} %U"),
            terminal: false,
            icon_name: Some(name.to_string()),
            icon: None,
        }
    }

    #[test]
    fn icons_loaded_assigns_icons_in_order() {
        use crate::icons::IconHandle;
        let mut state = test_state();
        state.apps = vec![app_entry("a"), app_entry("b"), app_entry("c")];

        // Pretend only the middle app resolved.
        let svg = iced::widget::svg::Handle::from_memory(vec![]);
        let icons = vec![None, Some(IconHandle::Vector(svg)), None];
        let _ = update(&mut state, Message::IconsLoaded(icons));

        assert!(state.apps[0].icon.is_none());
        assert!(state.apps[1].icon.is_some(), "middle app should have icon");
        assert!(state.apps[2].icon.is_none());
    }

    #[test]
    fn icons_loaded_ignores_extra_entries_safely() {
        // If the IconsLoaded vec is longer than state.apps (shouldn’t happen
        // but cheap to defend against), the handler must not panic.
        let mut state = test_state();
        state.apps = vec![app_entry("a")];
        let icons = vec![None, None, None];
        let _ = update(&mut state, Message::IconsLoaded(icons));
        assert_eq!(state.apps.len(), 1);
    }

    #[test]
    fn icons_loaded_with_empty_vec_is_noop() {
        let mut state = test_state();
        state.apps = vec![app_entry("a"), app_entry("b")];
        let _ = update(&mut state, Message::IconsLoaded(vec![]));
        assert!(state.apps.iter().all(|a| a.icon.is_none()));
    }
}

// ── Subscription ──────────────────────────────────────────────────────────────

pub fn subscription(state: &Trebuchet) -> Subscription<Message> {
    let events = event::listen_with(on_event);
    let component = match state.active {
        ActiveComponent::Launcher => state.launcher.subscription().map(Message::Launcher),
        ActiveComponent::Cmd => state.cmd.subscription().map(Message::Cmd),
        ActiveComponent::Settings => state.settings.subscription().map(Message::Settings),
        ActiveComponent::WindowMover => state.window_mover.subscription().map(Message::WindowMover),
    };
    Subscription::batch([events, component])
}
