use iced::{
    event::Status,
    keyboard::{self, key::Named, Key},
    time,
    widget::{button, column, container, row, scrollable, svg, text, Space},
    Alignment, Background, Border, Color, Element, Event, Font, Length, Subscription, Task,
};
use std::time::Duration;

use super::command::{ComponentEvent, SlashCommand};
use super::component::Component;
use crate::config::Config;
use crate::launcher::AppEntry;
use crate::ui::{search_bar, SearchIcon, ShakeState};

const COPY_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
  fill="none" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <rect x="9" y="9" width="13" height="13" rx="2"/>
  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
</svg>"#;

pub struct CommandResult {
    /// Current text in the search bar — editable, cleared after each command.
    query: String,
    /// Prefix of the last executed command, shown as the terminal prompt.
    last_prefix: String,
    /// Captured stdout from the last command.
    output: String,
    copy_feedback: bool,
    shake: ShakeState,
}

#[derive(Debug, Clone)]
pub enum Msg {
    QueryChanged(String),
    Copy,
    Copied,
    ShakeTick,
}

impl CommandResult {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            last_prefix: String::new(),
            output: String::new(),
            copy_feedback: false,
            shake: ShakeState::default(),
        }
    }

    /// Called when a command with `display_result = true` runs.
    /// Clears the input so the user can type a new command immediately.
    pub fn show(&mut self, prefix: String, output: String) {
        self.last_prefix = prefix;
        self.output = output;
        self.query = String::new();
        self.copy_feedback = false;
        self.shake = ShakeState::default();
    }
}

impl Component for CommandResult {
    type Msg = Msg;

    fn handle_event(
        &mut self,
        event: &Event,
        status: Status,
        _apps: &[AppEntry],
        config: &Config,
    ) -> (Task<Msg>, ComponentEvent) {
        let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, text, .. }) = event
        else {
            return (Task::none(), ComponentEvent::Handled);
        };

        match key {
            Key::Named(Named::Escape) => {
                (Task::none(), ComponentEvent::CommandInvoked(SlashCommand::App, String::new()))
            }

            Key::Named(Named::Enter) => {
                let trimmed = self.query.trim().to_string();

                // `/app` returns to launcher.
                if let Some((SlashCommand::App, args)) =
                    SlashCommand::detect(&format!("{} ", trimmed))
                {
                    return (
                        Task::none(),
                        ComponentEvent::CommandInvoked(SlashCommand::App, args),
                    );
                }

                // Run a matching custom command.
                if let Some(cmd) = config.commands.iter().find(|c| c.prefix == trimmed) {
                    let shell_cmd = cmd.command.clone();
                    if cmd.display_result {
                        let output = match std::process::Command::new("sh")
                            .args(["-c", &shell_cmd])
                            .output()
                        {
                            Ok(o) => {
                                let out =
                                    String::from_utf8_lossy(&o.stdout).trim().to_string();
                                if out.is_empty() { "(no output)".to_string() } else { out }
                            }
                            Err(e) => format!("Error: {e}"),
                        };
                        self.last_prefix = trimmed;
                        self.output = output;
                        self.query = String::new();
                        self.copy_feedback = false;
                    } else {
                        let _ = std::process::Command::new("sh")
                            .args(["-c", &shell_cmd])
                            .spawn();
                        return (Task::none(), ComponentEvent::Exit);
                    }
                    return (Task::none(), ComponentEvent::Handled);
                }

                if !trimmed.is_empty() {
                    self.shake = ShakeState::trigger();
                }
                (Task::none(), ComponentEvent::Handled)
            }

            Key::Named(Named::Backspace) if status == Status::Ignored => {
                self.query.pop();
                (Task::none(), ComponentEvent::Handled)
            }

            Key::Named(Named::Space) if status == Status::Ignored => {
                self.query.push(' ');
                (Task::none(), ComponentEvent::Handled)
            }

            Key::Character(_)
                if status == Status::Ignored
                    && !modifiers.control()
                    && !modifiers.alt()
                    && !modifiers.logo() =>
            {
                if let Some(t) = text.as_ref() {
                    self.query.push_str(t);
                }
                (Task::none(), ComponentEvent::Handled)
            }

            _ => (Task::none(), ComponentEvent::Handled),
        }
    }

    fn update(&mut self, msg: Msg, _apps: &[AppEntry], _config: &Config) -> (Task<Msg>, ComponentEvent) {
        match msg {
            Msg::QueryChanged(s) => {
                self.query = s;
            }
            Msg::Copy => {
                let _ = std::process::Command::new("wl-copy").arg(&self.output).spawn();
                self.copy_feedback = true;
                return (
                    Task::perform(
                        async { tokio::time::sleep(Duration::from_secs(2)).await },
                        |_| Msg::Copied,
                    ),
                    ComponentEvent::Handled,
                );
            }
            Msg::Copied => {
                self.copy_feedback = false;
            }
            Msg::ShakeTick => {
                self.shake.advance();
            }
        }
        (Task::none(), ComponentEvent::Handled)
    }

    fn view<'a>(&'a self, _apps: &'a [AppEntry], config: &'a Config) -> Element<'a, Msg> {
        let (btn_bg, feedback_color) = (config.theme.button_background, config.theme.copy_feedback);

        // ── Copy button ───────────────────────────────────────────────────────
        let copy_icon = svg(svg::Handle::from_memory(COPY_ICON.to_vec()))
            .width(16)
            .height(16)
            .style(|_theme, _status| svg::Style { color: Some(Color::WHITE) });
        let copy_btn = button(copy_icon)
            .on_press(Msg::Copy)
            .padding(8)
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(btn_bg)),
                border: Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            });

        let feedback: Element<'a, Msg> = if self.copy_feedback {
            text("Copied to clipboard").size(13).color(feedback_color).into()
        } else {
            text("").size(13).into()
        };

        let action_bar = row![
            feedback,
            Space::new().width(Length::Fill),
            copy_btn,
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        // ── Terminal body: prompt + output ────────────────────────────────────
        let prompt_line: Element<'a, Msg> = if !self.last_prefix.is_empty() {
            text(format!("$ {}", self.last_prefix))
                .font(Font::MONOSPACE)
                .size(14)
                .color(config.theme.terminal_prompt)
                .into()
        } else {
            Space::new().width(0).height(0).into()
        };

        let output_text = text(&self.output)
            .font(Font::MONOSPACE)
            .size(14)
            .color(config.theme.terminal_output);

        let terminal_body = column![prompt_line, output_text].spacing(6);

        let term_bg = config.theme.terminal_background;
        let output_area = container(
            scrollable(container(terminal_body).width(Length::Fill).padding([0, 4]))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(move |_theme| container::Style {
            background: Some(Background::Color(term_bg)),
            border: Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([16, 20]);

        let panel = column![output_area, action_bar]
            .spacing(8)
            .width(Length::Fill)
            .height(Length::Fill);

        column![
            search_bar(&self.query, &self.shake, SearchIcon::Terminal, &config.theme, Msg::QueryChanged),
            panel,
        ]
        .spacing(16)
        .padding(iced::Padding { top: 24.0, bottom: 24.0, left: 80.0, right: 80.0 })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn subscription(&self) -> Subscription<Msg> {
        if self.shake.active {
            time::every(Duration::from_millis(67)).map(|_| Msg::ShakeTick)
        } else {
            Subscription::none()
        }
    }
}
