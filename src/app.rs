use iced::{
    event,
    event::Status,
    mouse,
    widget::container,
    Background, Border, Color, Element, Event, Length, Subscription, Task,
};
use iced_layershell::to_layer_message;

use crate::components::ai_agent::{self, AIAgent};
use crate::components::app_launcher::{self, AppLauncher};
use crate::components::command::{ComponentEvent, SlashCommand};
use crate::components::command_result::{self, CommandResult};
use crate::components::component::Component;
use crate::config::Config;
use crate::launcher::{scan_applications, AppEntry};
use crate::ui::ShakeState;

// ── Active component ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveComponent {
    Launcher,
    Ai,
    CommandResult,
}

// ── App state ─────────────────────────────────────────────────────────────────

pub struct Trebuchet {
    pub apps: Vec<AppEntry>,
    pub config: Config,
    pub active: ActiveComponent,
    pub launcher: AppLauncher,
    pub ai_agent: AIAgent,
    pub command_result: CommandResult,
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    Close,
    IcedEvent(Event, Status),
    Launcher(app_launcher::Msg),
    Ai(ai_agent::Msg),
    CommandResult(command_result::Msg),
}

// ── Boot ──────────────────────────────────────────────────────────────────────

pub fn boot() -> (Trebuchet, Task<Message>) {
    let apps = scan_applications();
    let launcher = AppLauncher::new(&apps);
    let ai_agent = AIAgent::new();
    let command_result = CommandResult::new();
    let state = Trebuchet {
        apps,
        config: Config::load(),
        active: ActiveComponent::Launcher,
        launcher,
        ai_agent,
        command_result,
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
        ComponentEvent::CommandInvoked(SlashCommand::Unknown(name), _) => {
            let prefix = format!("/{}", name);
            if let Some(cmd) = state.config.commands.iter().find(|c| c.prefix == prefix) {
                let shell_cmd = cmd.command.clone();
                if cmd.display_result {
                    let output = match std::process::Command::new("sh")
                        .args(["-c", &shell_cmd])
                        .output()
                    {
                        Ok(o) => {
                            let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
                            if out.is_empty() { "(no output)".to_string() } else { out }
                        }
                        Err(e) => format!("Error: {e}"),
                    };
                    state.active = ActiveComponent::CommandResult;
                    state.command_result.show(prefix, output);
                } else {
                    let _ = std::process::Command::new("sh")
                        .args(["-c", &shell_cmd])
                        .spawn();
                    std::process::exit(0);
                }
            } else {
                state.launcher.shake = ShakeState::trigger();
                state.launcher.query.clear();
                let apps = state.apps.clone();
                state.launcher.apply_filter(&apps, "");
            }
        }
    }
}

// ── Update ────────────────────────────────────────────────────────────────────

pub fn update(state: &mut Trebuchet, msg: Message) -> Task<Message> {
    match msg {
        Message::Close => std::process::exit(0),

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
        Message::CommandResult(m) => {
            let (task, evt) = state.command_result.update(m, &state.apps, &state.config);
            apply_event(state, evt);
            return task.map(Message::CommandResult);
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
                ActiveComponent::CommandResult => {
                    let (t, e) = state.command_result.handle_event(&event, status, &state.apps, &state.config);
                    (t.map(Message::CommandResult), e)
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
        ActiveComponent::CommandResult => {
            state.command_result.view(&state.apps, &state.config).map(Message::CommandResult)
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
        ActiveComponent::CommandResult => state.command_result.subscription().map(Message::CommandResult),
    };
    Subscription::batch([events, component])
}
