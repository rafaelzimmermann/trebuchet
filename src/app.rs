use iced::{
    event,
    event::Status,
    mouse,
    widget::container,
    Background, Border, Color, Element, Event, Length, Subscription, Task,
};
use iced_layershell::to_layer_message;

use crate::ai_agent::{self, AIAgent};
use crate::app_launcher::{self, AppLauncher};
use crate::command::{ComponentEvent, SlashCommand};
use crate::component::Component;
use crate::config::Config;
use crate::launcher::{scan_applications, AppEntry};

// ── Active component ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveComponent {
    Launcher,
    Ai,
}

// ── App state ─────────────────────────────────────────────────────────────────

pub struct Trebuchet {
    pub apps: Vec<AppEntry>,
    pub config: Config,
    pub active: ActiveComponent,
    pub launcher: AppLauncher,
    pub ai_agent: AIAgent,
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    Close,
    IcedEvent(Event, Status),
    Launcher(app_launcher::Msg),
    Ai(ai_agent::Msg),
}

// ── Boot ──────────────────────────────────────────────────────────────────────

pub fn boot() -> (Trebuchet, Task<Message>) {
    let apps = scan_applications();
    let launcher = AppLauncher::new(&apps);
    let ai_agent = AIAgent::new();
    let state = Trebuchet {
        apps,
        config: Config::load(),
        active: ActiveComponent::Launcher,
        launcher,
        ai_agent,
    };
    (state, Task::none())
}

pub fn namespace() -> String {
    "trebuchet".into()
}

// ── Event application ─────────────────────────────────────────────────────────

fn apply_event(state: &mut Trebuchet, event: ComponentEvent) {
    match event {
        ComponentEvent::Handled => {}
        ComponentEvent::Exit => std::process::exit(0),
        ComponentEvent::CommandInvoked(SlashCommand::Ai, args) => {
            state.active = ActiveComponent::Ai;
            state.ai_agent.reset(args);
        }
        ComponentEvent::CommandInvoked(SlashCommand::App, _) => {
            state.active = ActiveComponent::Launcher;
            let apps = state.apps.clone();
            state.launcher.reset(&apps);
        }
        ComponentEvent::CommandInvoked(SlashCommand::Unknown(_), _) => {}
    }
}

// ── Update ────────────────────────────────────────────────────────────────────

pub fn update(state: &mut Trebuchet, msg: Message) -> Task<Message> {
    match msg {
        Message::Close => std::process::exit(0),

        Message::Launcher(m) => {
            return state.launcher.update(m, &state.apps, &state.config).map(Message::Launcher);
        }
        Message::Ai(m) => {
            return state.ai_agent.update(m, &state.apps, &state.config).map(Message::Ai);
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
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color {
                r: 0.08,
                g: 0.08,
                b: 0.12,
                a: 0.92,
            })),
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
        ActiveComponent::Ai => state.ai_agent.subscription().map(Message::Ai),
    };
    Subscription::batch([events, component])
}
