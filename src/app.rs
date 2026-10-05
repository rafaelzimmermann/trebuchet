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
use crate::launcher::{cached_applications, scan_and_cache_applications, AppEntry};
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
    icon_generation: u64,
    icons_resolved: Vec<bool>,
    icons_loading: bool,
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    Close,
    /// Absorbs clicks anywhere inside the window so they don't propagate as Ignored.
    Absorb,
    InitialAppsLoaded(Vec<AppEntry>, bool),
    AppsLoaded(Vec<AppEntry>),
    /// Results are tied to a catalogue generation and explicit app indices.
    IconsLoaded(u64, Vec<(usize, Option<crate::icons::IconHandle>)>),
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
    // The real `Config::load()` runs in parallel with the cached app-list load
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
        icon_generation: 0,
        icons_resolved: Vec::new(),
        icons_loading: false,
    };
    let task = Task::batch([
        Task::perform(
            async {
                tokio::task::spawn_blocking(|| match cached_applications() {
                    Some(apps) => (apps, true),
                    None => (scan_and_cache_applications(), false),
                })
                .await
                .unwrap_or_default()
            },
            |(apps, cached)| Message::InitialAppsLoaded(apps, cached),
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
    if matches!(&event, ComponentEvent::CommandInvoked(command, _) if !matches!(command, SlashCommand::Unknown(_)))
    {
        match state.active {
            ActiveComponent::Cmd => state.cmd.leave(),
            ActiveComponent::WindowMover => state.window_mover.leave(),
            _ => {}
        }
    }
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

/// One bounded batch at a time: visible results first, then background pages.
/// Re-evaluate on each completion so typing/navigation can change priority.
fn next_icon_indices(state: &Trebuchet) -> Vec<usize> {
    let page_size = (state.config.columns * state.config.rows).max(1);
    let visible: Vec<_> = if state.active == ActiveComponent::Launcher {
        state
            .launcher
            .filtered
            .iter()
            .skip(state.launcher.page * page_size)
            .take(page_size)
            .copied()
            .filter(|&index| state.icons_resolved.get(index) == Some(&false))
            .collect()
    } else {
        Vec::new()
    };
    if !visible.is_empty() {
        return visible;
    }
    state
        .icons_resolved
        .iter()
        .enumerate()
        .filter_map(|(index, &resolved)| (!resolved).then_some(index))
        .take(8)
        .collect()
}

fn schedule_icons(state: &mut Trebuchet) -> Task<Message> {
    if state.icons_loading {
        return Task::none();
    }
    let indices = next_icon_indices(state);
    if indices.is_empty() {
        return Task::none();
    }
    state.icons_loading = true;
    let generation = state.icon_generation;
    let apps: Vec<_> = indices.iter().map(|&i| state.apps[i].clone()).collect();
    Task::perform(
        async move {
            let count = apps.len();
            let icons =
                tokio::task::spawn_blocking(move || crate::launcher::resolve_all_icons(&apps))
                    .await
                    .unwrap_or_else(|_| vec![None; count]);
            indices.into_iter().zip(icons).collect()
        },
        move |icons| Message::IconsLoaded(generation, icons),
    )
}

fn replace_apps(state: &mut Trebuchet, mut apps: Vec<AppEntry>) {
    // The common case: nothing changed. Keep in-flight work and navigation intact.
    if state.apps.len() == apps.len()
        && state
            .apps
            .iter()
            .zip(&apps)
            .all(|(old, new)| old.same_application(new))
    {
        return;
    }
    let selected = state
        .launcher
        .selected
        .and_then(|i| state.launcher.filtered.get(i))
        .and_then(|&i| state.apps.get(i))
        .cloned();
    let page = state.launcher.page;
    let resolved = apps
        .iter_mut()
        .map(|app| {
            if let Some(i) = state.apps.iter().position(|old| old.same_application(app)) {
                app.icon = state.apps[i].icon.clone();
                state.icons_resolved.get(i).copied().unwrap_or(false)
            } else {
                false
            }
        })
        .collect();
    state.apps = apps;
    state.icons_resolved = resolved;
    state.icon_generation += 1;
    state.icons_loading = false;
    let query = state.launcher.query.clone();
    state.launcher.apply_filter(&state.apps, &query);
    let page_size = (state.config.columns * state.config.rows).max(1);
    state.launcher.page = page.min(state.launcher.filtered.len().saturating_sub(1) / page_size);
    if let Some(selected) = selected {
        state.launcher.selected = state
            .launcher
            .filtered
            .iter()
            .position(|&i| state.apps[i].same_application(&selected));
        if let Some(i) = state.launcher.selected {
            state.launcher.page = i / page_size;
        }
    }
}

pub fn update(state: &mut Trebuchet, msg: Message) -> Task<Message> {
    let task = update_inner(state, msg);
    Task::batch([task, schedule_icons(state)])
}

fn update_inner(state: &mut Trebuchet, msg: Message) -> Task<Message> {
    match msg {
        Message::Close => std::process::exit(0),
        Message::Absorb => {}
        Message::InitialAppsLoaded(apps, cached) => {
            replace_apps(state, apps);
            if cached {
                return Task::perform(
                    async {
                        tokio::task::spawn_blocking(scan_and_cache_applications)
                            .await
                            .ok()
                    },
                    |apps| match apps {
                        Some(apps) => Message::AppsLoaded(apps),
                        None => Message::Absorb,
                    },
                );
            }
        }
        Message::AppsLoaded(apps) => replace_apps(state, apps),
        Message::IconsLoaded(generation, icons) => {
            if generation != state.icon_generation {
                return Task::none();
            }
            state.icons_loading = false;
            for (idx, icon) in icons {
                if let Some(app) = state.apps.get_mut(idx) {
                    app.icon = icon;
                    if let Some(resolved) = state.icons_resolved.get_mut(idx) {
                        *resolved = true;
                    }
                }
            }
        }

        Message::ConfigLoaded(config) => {
            state.config = config;
            let page_size = (state.config.columns * state.config.rows).max(1);
            state.launcher.page = state
                .launcher
                .selected
                .map(|i| i / page_size)
                .unwrap_or(state.launcher.page)
                .min(state.launcher.filtered.len().saturating_sub(1) / page_size);
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
                    let (t, e) =
                        state
                            .launcher
                            .handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::Launcher), e)
                }
                ActiveComponent::Cmd => {
                    let (t, e) = state
                        .cmd
                        .handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::Cmd), e)
                }
                ActiveComponent::Settings => {
                    let (t, e) =
                        state
                            .settings
                            .handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::Settings), e)
                }
                ActiveComponent::WindowMover => {
                    let (t, e) =
                        state
                            .window_mover
                            .handle_event(&event, status, &state.apps, &state.config);
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
        ActiveComponent::Launcher => state
            .launcher
            .view(&state.apps, &state.config)
            .map(Message::Launcher),
        ActiveComponent::Cmd => state.cmd.view(&state.apps, &state.config).map(Message::Cmd),
        ActiveComponent::Settings => state
            .settings
            .view(&state.apps, &state.config)
            .map(Message::Settings),
        ActiveComponent::WindowMover => state
            .window_mover
            .view(&state.apps, &state.config)
            .map(Message::WindowMover),
    };

    let bg = state.config.theme.background;
    container(mouse_area(content).on_press(Message::Absorb))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 16.0.into(),
                ..Default::default()
            },
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
            icon_generation: 0,
            icons_resolved: Vec::new(),
            icons_loading: false,
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

        let _ = update(&mut state, Message::ConfigLoaded(Config::default()));
        assert_eq!(state.cmd.query, "partial", "Cmd state must be untouched");
    }

    // ── IconsLoaded ───────────────────────────────────────────────────────────

    fn app_entry(name: &str) -> crate::launcher::AppEntry {
        crate::launcher::AppEntry {
            name: name.to_string(),
            exec: vec![name.to_string()],
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
        let _ = update(
            &mut state,
            Message::IconsLoaded(0, icons.into_iter().enumerate().collect()),
        );

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
        let _ = update(
            &mut state,
            Message::IconsLoaded(0, icons.into_iter().enumerate().collect()),
        );
        assert_eq!(state.apps.len(), 1);
    }

    #[test]
    fn icons_loaded_with_empty_vec_is_noop() {
        let mut state = test_state();
        state.apps = vec![app_entry("a"), app_entry("b")];
        let _ = update(&mut state, Message::IconsLoaded(0, vec![]));
        assert!(state.apps.iter().all(|a| a.icon.is_none()));
    }
    fn populated_state(count: usize) -> Trebuchet {
        let mut state = test_state();
        state.config.columns = 2;
        state.config.rows = 2;
        replace_apps(
            &mut state,
            (0..count)
                .map(|i| app_entry(&format!("App {i:02}")))
                .collect(),
        );
        state
    }

    #[test]
    fn first_page_precedes_background_icons() {
        let mut state = populated_state(20);
        assert_eq!(next_icon_indices(&state), vec![0, 1, 2, 3]);
        state.icons_resolved[..4].fill(true);
        assert_eq!(next_icon_indices(&state), (4..12).collect::<Vec<_>>());
        state.icons_resolved.fill(true);
        assert!(next_icon_indices(&state).is_empty());
    }

    #[test]
    fn navigation_and_search_reprioritize_icons() {
        let mut state = populated_state(20);
        let _ = state
            .launcher
            .update(app_launcher::Msg::GoToPage(3), &state.apps, &state.config);
        assert_eq!(next_icon_indices(&state), vec![12, 13, 14, 15]);
        let _ = state.launcher.update(
            app_launcher::Msg::QueryChanged("App 19".into()),
            &state.apps,
            &state.config,
        );
        assert_eq!(next_icon_indices(&state), vec![19]);
    }

    #[test]
    fn stale_icon_results_cannot_overwrite_refreshed_catalogue() {
        let mut state = populated_state(4);
        let generation = state.icon_generation;
        replace_apps(&mut state, vec![app_entry("New app")]);
        state.icons_loading = true;
        let svg = iced::widget::svg::Handle::from_memory(vec![]);
        let _ = update_inner(
            &mut state,
            Message::IconsLoaded(
                generation,
                vec![(0, Some(crate::icons::IconHandle::Vector(svg)))],
            ),
        );
        assert!(state.apps[0].icon.is_none());
        assert!(
            state.icons_loading,
            "old completion must not clear new batch"
        );
    }

    #[test]
    fn refresh_preserves_search_selection_and_resolved_icons() {
        let mut state = populated_state(20);
        state.launcher.query = "App".into();
        state.launcher.apply_filter(&state.apps, "App");
        state.launcher.selected = Some(13);
        state.launcher.page = 3;
        let selected = state.apps[13].clone();
        state.icons_resolved[13] = true;
        state.apps[13].icon = Some(crate::icons::IconHandle::Vector(
            iced::widget::svg::Handle::from_memory(vec![]),
        ));
        let mut apps = state.apps.clone();
        apps.insert(0, app_entry("Another App"));
        replace_apps(&mut state, apps);
        assert_eq!(state.launcher.query, "App");
        let index = state.launcher.filtered[state.launcher.selected.unwrap()];
        assert!(state.apps[index].same_application(&selected));
        assert!(state.apps[index].icon.is_some());
        assert!(state.icons_resolved[index]);
        assert_eq!(state.launcher.page, state.launcher.selected.unwrap() / 4);
    }

    #[test]
    fn unchanged_refresh_keeps_inflight_batch() {
        let mut state = populated_state(4);
        state.icons_loading = true;
        let generation = state.icon_generation;
        let apps = state.apps.clone();
        replace_apps(&mut state, apps);
        assert_eq!(state.icon_generation, generation);
        assert!(state.icons_loading);
    }

    #[test]
    fn missing_icons_are_completed_without_retrying_forever() {
        let mut state = populated_state(4);
        let generation = state.icon_generation;
        let _ = update_inner(
            &mut state,
            Message::IconsLoaded(generation, vec![(3, None), (1, None), (0, None), (2, None)]),
        );
        assert!(next_icon_indices(&state).is_empty());
    }

    #[test]
    fn refresh_removal_clamps_page_and_clears_removed_selection() {
        let mut state = populated_state(20);
        state.launcher.page = 4;
        state.launcher.selected = Some(19);
        replace_apps(&mut state, vec![app_entry("Remaining")]);
        assert_eq!(state.launcher.page, 0);
        assert_eq!(state.launcher.selected, None);
    }

    #[test]
    fn custom_page_size_changes_visible_priority() {
        let mut state = populated_state(20);
        let mut config = state.config.clone();
        config.columns = 3;
        config.rows = 2;
        let _ = update_inner(&mut state, Message::ConfigLoaded(config));
        assert_eq!(next_icon_indices(&state), (0..6).collect::<Vec<_>>());
    }
}
