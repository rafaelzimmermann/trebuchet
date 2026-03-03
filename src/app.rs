use iced::{
    event,
    keyboard::{self, key::Named, Key},
    mouse,
    widget::container,
    Background, Border, Color, Element, Event, Length, Subscription, Task,
};
use iced::event::Status;
use iced_layershell::to_layer_message;

use crate::ai_agent::{self, AIAgent};
use crate::app_launcher::{self, AppLauncher};
use crate::command::{ComponentEvent, SlashCommand};
use crate::component::{Component, NavDirection};
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
    // Raw input events from on_event
    InputChar(String),
    InputBackspace,
    Submit,
    Escape,
    NavLeft,
    NavRight,
    NavUp,
    NavDown,
    PageNext,
    PagePrev,
    GoToPage(usize),
    Close,
    // Component message wrappers
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

// ── Dispatch helpers ──────────────────────────────────────────────────────────

fn dispatch_input(
    state: &mut Trebuchet,
    f_launcher: impl FnOnce(&mut AppLauncher, &[AppEntry], &Config) -> (Task<app_launcher::Msg>, ComponentEvent),
    f_ai: impl FnOnce(&mut AIAgent, &[AppEntry], &Config) -> (Task<ai_agent::Msg>, ComponentEvent),
) -> Task<Message> {
    let (task, event) = match state.active {
        ActiveComponent::Launcher => {
            let (t, e) = f_launcher(&mut state.launcher, &state.apps, &state.config);
            (t.map(Message::Launcher), e)
        }
        ActiveComponent::Ai => {
            let (t, e) = f_ai(&mut state.ai_agent, &state.apps, &state.config);
            (t.map(Message::Ai), e)
        }
    };
    apply_event(state, event);
    task
}

fn dispatch_nav(state: &mut Trebuchet, dir: NavDirection) {
    let event = match state.active {
        ActiveComponent::Launcher => state.launcher.handle_nav(dir, &state.config),
        ActiveComponent::Ai => state.ai_agent.handle_nav(dir, &state.config),
    };
    apply_event(state, event);
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

        Message::InputChar(c) => {
            return dispatch_input(
                state,
                |l, apps, cfg| l.handle_char(c.clone(), apps, cfg),
                |a, apps, cfg| a.handle_char(c.clone(), apps, cfg),
            );
        }
        Message::InputBackspace => {
            return dispatch_input(
                state,
                |l, apps, cfg| l.handle_backspace(apps, cfg),
                |a, apps, cfg| a.handle_backspace(apps, cfg),
            );
        }
        Message::Submit => {
            return dispatch_input(
                state,
                |l, apps, cfg| l.handle_submit(apps, cfg),
                |a, apps, cfg| a.handle_submit(apps, cfg),
            );
        }

        Message::Escape => {
            let event = match state.active {
                ActiveComponent::Launcher => state.launcher.handle_escape(),
                ActiveComponent::Ai => state.ai_agent.handle_escape(),
            };
            apply_event(state, event);
        }

        Message::NavLeft => dispatch_nav(state, NavDirection::Left),
        Message::NavRight => dispatch_nav(state, NavDirection::Right),
        Message::NavUp => dispatch_nav(state, NavDirection::Up),
        Message::NavDown => dispatch_nav(state, NavDirection::Down),

        Message::PageNext => {
            let event = match state.active {
                ActiveComponent::Launcher => state.launcher.handle_page(1, &state.config),
                ActiveComponent::Ai => state.ai_agent.handle_page(1, &state.config),
            };
            apply_event(state, event);
        }
        Message::PagePrev => {
            let event = match state.active {
                ActiveComponent::Launcher => state.launcher.handle_page(-1, &state.config),
                ActiveComponent::Ai => state.ai_agent.handle_page(-1, &state.config),
            };
            apply_event(state, event);
        }
        Message::GoToPage(p) => {
            let event = match state.active {
                ActiveComponent::Launcher => state.launcher.handle_go_to_page(p, &state.config),
                ActiveComponent::Ai => state.ai_agent.handle_go_to_page(p, &state.config),
            };
            apply_event(state, event);
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
    match event {
        Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, text, .. }) => match &key {
            Key::Named(Named::Enter) => Some(Message::Submit),
            Key::Named(Named::Escape) => Some(Message::Escape),
            Key::Named(Named::PageDown) => Some(Message::PageNext),
            Key::Named(Named::PageUp) => Some(Message::PagePrev),
            Key::Named(Named::ArrowRight) if status == Status::Ignored => {
                Some(Message::NavRight)
            }
            Key::Named(Named::ArrowLeft) if status == Status::Ignored => {
                Some(Message::NavLeft)
            }
            Key::Named(Named::ArrowDown) if status == Status::Ignored => {
                Some(Message::NavDown)
            }
            Key::Named(Named::ArrowUp) if status == Status::Ignored => {
                Some(Message::NavUp)
            }
            Key::Named(Named::Backspace) if status == Status::Ignored => {
                Some(Message::InputBackspace)
            }
            Key::Named(Named::Space) if status == Status::Ignored => {
                Some(Message::InputChar(" ".to_string()))
            }
            Key::Character(_)
                if status == Status::Ignored
                    && !modifiers.control()
                    && !modifiers.alt()
                    && !modifiers.logo() =>
            {
                text.as_ref().map(|t| Message::InputChar(t.to_string()))
            }
            _ => None,
        },
        Event::Mouse(mouse::Event::CursorLeft) => Some(Message::Close),
        Event::Mouse(mouse::Event::ButtonPressed(_)) if status == Status::Ignored => {
            Some(Message::Close)
        }
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
