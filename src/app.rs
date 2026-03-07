use iced::{
    event,
    event::Status,
    mouse,
    widget::{container, mouse_area},
    Background, Border, Element, Event, Length, Subscription, Task,
};
use iced_layershell::to_layer_message;

use crate::components::ai_agent::{self, AIAgent};
use crate::components::app_launcher::{self, AppLauncher};
use crate::components::cmd::{self, Cmd};
use crate::components::command::{ComponentEvent, SlashCommand};
use crate::components::component::Component;
use crate::components::settings::{self, Settings};
use crate::config::Config;
use crate::launcher::{scan_applications, AppEntry};
// ── Active component ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveComponent {
    Launcher,
    Ai,
    Cmd,
    Settings,
}

// ── App state ─────────────────────────────────────────────────────────────────

pub struct Trebuchet {
    pub apps: Vec<AppEntry>,
    pub config: Config,
    pub active: ActiveComponent,
    pub launcher: AppLauncher,
    pub ai_agent: AIAgent,
    pub cmd: Cmd,
    pub settings: Settings,
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    Close,
    /// Absorbs clicks anywhere inside the window so they don't propagate as Ignored.
    Absorb,
    IcedEvent(Event, Status),
    Launcher(app_launcher::Msg),
    Ai(ai_agent::Msg),
    Cmd(cmd::Msg),
    Settings(settings::Msg),
}

// ── Boot ──────────────────────────────────────────────────────────────────────

pub fn boot() -> (Trebuchet, Task<Message>) {
    let apps = scan_applications();
    let launcher = AppLauncher::new(&apps);
    let state = Trebuchet {
        apps,
        config: Config::load(),
        active: ActiveComponent::Launcher,
        launcher,
        ai_agent: AIAgent::new(),
        cmd: Cmd::new(),
        settings: Settings::new(),
    };
    (state, Task::none())
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

fn apply_event(state: &mut Trebuchet, event: ComponentEvent) {
    match event {
        ComponentEvent::Handled => {}
        ComponentEvent::Exit => std::process::exit(0),

        ComponentEvent::ThemeChanged(name, theme) => {
            state.config.theme = *theme;
            persist_theme(&name);
        }

        ComponentEvent::CommandInvoked(SlashCommand::Ai, args) => {
            state.active = ActiveComponent::Ai;
            state.ai_agent.reset(args);
        }
        ComponentEvent::CommandInvoked(SlashCommand::App, _) => {
            state.active = ActiveComponent::Launcher;
            let apps = state.apps.clone();
            state.launcher.reset(&apps);
        }
        ComponentEvent::CommandInvoked(SlashCommand::Config, _) => {
            state.active = ActiveComponent::Settings;
            state.settings.reset();
        }
        ComponentEvent::CommandInvoked(SlashCommand::Cmd, _) => {
            state.active = ActiveComponent::Cmd;
            state.cmd.reset();
        }
        // No component currently produces CommandInvoked(Unknown) — unknown
        // slash commands are handled locally (shake) in each component.
        ComponentEvent::CommandInvoked(SlashCommand::Unknown(_), _) => {}
    }
}

// ── Update ────────────────────────────────────────────────────────────────────

pub fn update(state: &mut Trebuchet, msg: Message) -> Task<Message> {
    match msg {
        Message::Close => std::process::exit(0),
        Message::Absorb => {}

        Message::Launcher(m) => {
            let (task, evt) = state.launcher.update(m, &state.apps, &state.config);
            apply_event(state, evt);
            return task.map(Message::Launcher);
        }
        Message::Ai(m) => {
            let (task, evt) = state.ai_agent.update(m, &state.apps, &state.config);
            apply_event(state, evt);
            return task.map(Message::Ai);
        }
        Message::Cmd(m) => {
            let (task, evt) = state.cmd.update(m, &state.apps, &state.config);
            apply_event(state, evt);
            return task.map(Message::Cmd);
        }
        Message::Settings(m) => {
            let (task, evt) = state.settings.update(m, &state.apps, &state.config);
            apply_event(state, evt);
            return task.map(Message::Settings);
        }

        Message::IcedEvent(event, status) => {
            let (task, evt) = match state.active {
                ActiveComponent::Launcher => {
                    let (t, e) = state.launcher.handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::Launcher), e)
                }
                ActiveComponent::Ai => {
                    let (t, e) = state.ai_agent.handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::Ai), e)
                }
                ActiveComponent::Cmd => {
                    let (t, e) = state.cmd.handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::Cmd), e)
                }
                ActiveComponent::Settings => {
                    let (t, e) = state.settings.handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::Settings), e)
                }
            };
            apply_event(state, evt);
            return task;
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
        ActiveComponent::Ai => {
            state.ai_agent.view(&state.apps, &state.config).map(Message::Ai)
        }
        ActiveComponent::Cmd => {
            state.cmd.view(&state.apps, &state.config).map(Message::Cmd)
        }
        ActiveComponent::Settings => {
            state.settings.view(&state.apps, &state.config).map(Message::Settings)
        }
    };

    let bg = state.config.theme.background;
    mouse_area(
        container(content)
            .padding([0, 0])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(bg)),
                border: Border { radius: 16.0.into(), ..Default::default() },
                ..Default::default()
            }),
    )
    .on_press(Message::Absorb)
    .into()
}

// ── Event handler ─────────────────────────────────────────────────────────────

fn on_event(event: Event, status: Status, _id: iced::window::Id) -> Option<Message> {
    match &event {
        // Close when the cursor leaves the window (click outside on Wayland).
        Event::Mouse(mouse::Event::CursorLeft) => Some(Message::Close),
        Event::Keyboard(_) => Some(Message::IcedEvent(event, status)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn click_inside_does_not_close() {
        // All clicks inside the window are absorbed by the top-level mouse_area
        // in view(); on_event no longer maps ButtonPressed to Close.
        let result = on_event(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Status::Ignored,
            iced::window::Id::unique(),
        );
        assert!(result.is_none());
    }
}

// ── Subscription ──────────────────────────────────────────────────────────────

pub fn subscription(state: &Trebuchet) -> Subscription<Message> {
    let events = event::listen_with(on_event);
    let component = match state.active {
        ActiveComponent::Launcher => state.launcher.subscription().map(Message::Launcher),
        ActiveComponent::Ai => state.ai_agent.subscription().map(Message::Ai),
        ActiveComponent::Cmd => state.cmd.subscription().map(Message::Cmd),
        ActiveComponent::Settings => state.settings.subscription().map(Message::Settings),
    };
    Subscription::batch([events, component])
}
