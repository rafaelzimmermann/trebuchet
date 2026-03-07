use iced::{
    event::Status,
    keyboard::{self, key::Named, Key},
    time,
    widget::{column, container, row, scrollable, text, Space},
    Alignment, Background, Border, Element, Event, Font, Length, Subscription, Task,
};
use std::time::Duration;

use super::command::{ComponentEvent, SlashCommand};
use super::component::Component;
use crate::config::Config;
use crate::launcher::AppEntry;
use crate::ui::panel::{icon_btn, PanelState, COPY_ICON};
use crate::ui::{search_bar, SearchIcon, ShakeState, PANEL_PADDING};

pub struct Cmd {
    query: String,
    panel: PanelState,
    copy_feedback: bool,
    shake: ShakeState,
}

#[derive(Debug, Clone)]
pub enum Msg {
    QueryChanged(String),
    Copy,
    Copied,
    ShakeTick,
    /// Delivered when an async `display_result` command finishes.
    CommandOutput(Result<String, String>),
}

impl Cmd {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            panel: PanelState::Idle,
            copy_feedback: false,
            shake: ShakeState::default(),
        }
    }

    pub fn reset(&mut self) {
        self.query = String::new();
        self.panel = PanelState::Idle;
        self.copy_feedback = false;
        self.shake = ShakeState::default();
    }

    /// Find and run the command matching `query`.
    ///
    /// * `display_result = true` — transitions to `Running`, launches an async
    ///   subprocess via `tokio::process`; result arrives as `Msg::CommandOutput`.
    /// * `display_result = false` — spawns-and-forgets, returns `Exit` immediately.
    fn execute(&mut self, query: &str, config: &Config) -> (Task<Msg>, ComponentEvent) {
        let q = query.trim();
        if let Some(cmd) = config.commands.iter().find(|c| c.prefix == q) {
            let shell_cmd = cmd.command.clone();
            if cmd.display_result {
                self.panel = PanelState::Running { prompt: q.to_string() };
                self.query.clear();
                self.copy_feedback = false;
                let task = Task::perform(
                    async move {
                        match tokio::process::Command::new("sh")
                            .args(["-c", &shell_cmd])
                            .output()
                            .await
                        {
                            Ok(o) => {
                                let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
                                if out.is_empty() { Ok("(no output)".to_string()) } else { Ok(out) }
                            }
                            Err(e) => Err(format!("Error: {e}")),
                        }
                    },
                    Msg::CommandOutput,
                );
                (task, ComponentEvent::Handled)
            } else {
                let _ = std::process::Command::new("sh").args(["-c", &shell_cmd]).spawn();
                (Task::none(), ComponentEvent::Exit)
            }
        } else {
            self.shake = ShakeState::trigger();
            (Task::none(), ComponentEvent::Handled)
        }
    }
}

impl Component for Cmd {
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
                let q = self.query.trim().to_string();
                if q.is_empty() {
                    return (Task::none(), ComponentEvent::Handled);
                }
                if let Some(evt) = SlashCommand::as_nav_event(&format!("{} ", &q)) {
                    self.query.clear();
                    return (Task::none(), evt);
                }
                self.execute(&q, config)
            }

            Key::Named(Named::Backspace) if status == Status::Ignored => {
                self.query.pop();
                (Task::none(), ComponentEvent::Handled)
            }

            Key::Named(Named::Space) if status == Status::Ignored => {
                self.query.push(' ');
                if let Some(evt) = SlashCommand::as_nav_event(&self.query) {
                    self.query.clear();
                    return (Task::none(), evt);
                }
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

    fn update(
        &mut self,
        msg: Msg,
        _apps: &[AppEntry],
        _config: &Config,
    ) -> (Task<Msg>, ComponentEvent) {
        match msg {
            Msg::QueryChanged(s) => {
                self.query = s;
            }
            Msg::Copy => {
                let text_to_copy = match &self.panel {
                    PanelState::Result { copy_text, .. } => copy_text.clone(),
                    _ => String::new(),
                };
                if !text_to_copy.is_empty() {
                    let _ = std::process::Command::new("wl-copy").arg(&text_to_copy).spawn();
                    self.copy_feedback = true;
                    return (
                        Task::perform(
                            async { tokio::time::sleep(Duration::from_secs(2)).await },
                            |_| Msg::Copied,
                        ),
                        ComponentEvent::Handled,
                    );
                }
            }
            Msg::Copied => {
                self.copy_feedback = false;
            }
            Msg::ShakeTick => {
                self.shake.advance();
            }
            Msg::CommandOutput(result) => {
                let prompt = if let PanelState::Running { prompt } = &self.panel {
                    prompt.clone()
                } else {
                    String::new()
                };
                let output = result.unwrap_or_else(|e| e);
                let copy_text = format!("$ {prompt}\n{output}");
                self.panel = PanelState::Result { prompt, output, copy_text };
                return (Task::none(), ComponentEvent::Handled);
            }
        }
        (Task::none(), ComponentEvent::Handled)
    }

    fn view<'a>(&'a self, _apps: &'a [AppEntry], config: &'a Config) -> Element<'a, Msg> {
        let (idle_color, text_color, prompt_color) =
            (config.theme.ai_idle, config.theme.terminal_output, config.theme.terminal_prompt);

        let body: Element<'a, Msg> = match &self.panel {
            PanelState::Idle => {
                if config.commands.is_empty() {
                    column![
                        text("No commands configured.").size(13).color(prompt_color),
                        text("Add [[command]] blocks to ~/.config/trebuchet/trebuchet.conf")
                            .font(Font::MONOSPACE)
                            .size(13)
                            .color(idle_color),
                    ]
                    .spacing(6)
                    .into()
                } else {
                    let mut items: Vec<Element<'a, Msg>> = vec![
                        text("Available commands:").size(13).color(prompt_color).into(),
                    ];
                    for cmd in &config.commands {
                        items.push(
                            text(format!("  {}", cmd.prefix))
                                .font(Font::MONOSPACE)
                                .size(14)
                                .color(idle_color)
                                .into(),
                        );
                    }
                    column(items).spacing(6).into()
                }
            }
            PanelState::Running { prompt } => {
                column![
                    text(format!("$ {prompt}")).font(Font::MONOSPACE).size(14).color(prompt_color),
                    text("Running\u{2026}").font(Font::MONOSPACE).size(14).color(idle_color),
                ]
                .spacing(6)
                .into()
            }
            PanelState::Result { prompt, output, .. } => {
                let prompt_line = text(format!("$ {prompt}"))
                    .font(Font::MONOSPACE)
                    .size(14)
                    .color(prompt_color);
                let output_text = text(output.as_str())
                    .font(Font::MONOSPACE)
                    .size(14)
                    .color(text_color);
                column![prompt_line, output_text].spacing(6).into()
            }
        };

        let panel_bg = config.theme.terminal_background;
        let panel = container(
            scrollable(container(body).width(Length::Fill).padding([0, 4]))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(move |_theme| container::Style {
            background: Some(Background::Color(panel_bg)),
            border: Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([0, 0]);

        let has_result = matches!(self.panel, PanelState::Result { .. });
        let (btn_bg, feedback_color) =
            (config.theme.button_background, config.theme.copy_feedback);

        let feedback: Element<'a, Msg> = if self.copy_feedback {
            text("Copied to clipboard").size(13).color(feedback_color).into()
        } else {
            text("").size(13).into()
        };

        let action_bar = row![
            feedback,
            Space::new().width(Length::Fill),
            icon_btn(COPY_ICON, Msg::Copy, has_result, btn_bg),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        container(
            column![
                search_bar(&self.query, &self.shake, SearchIcon::Terminal, &config.theme, Msg::QueryChanged),
                panel,
                action_bar,
            ]
            .spacing(8)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .padding(PANEL_PADDING)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, CustomCommand};
    use crate::ui::panel::PanelState;

    fn config_with(cmds: Vec<CustomCommand>) -> Config {
        Config { commands: cmds, ..Config::default() }
    }

    fn silent_cmd(prefix: &str, command: &str) -> CustomCommand {
        CustomCommand { prefix: prefix.to_string(), command: command.to_string(), display_result: false }
    }

    fn display_cmd(prefix: &str, command: &str) -> CustomCommand {
        CustomCommand { prefix: prefix.to_string(), command: command.to_string(), display_result: true }
    }

    // ── Cmd::new / reset ──────────────────────────────────────────────────────

    #[test]
    fn new_starts_empty_and_idle() {
        let c = Cmd::new();
        assert!(c.query.is_empty());
        assert!(matches!(c.panel, PanelState::Idle));
        assert!(!c.copy_feedback);
    }

    #[test]
    fn reset_clears_query_and_panel() {
        let mut c = Cmd::new();
        c.query = "uptime".to_string();
        c.panel = PanelState::Result {
            prompt: "uptime".to_string(),
            output: "up 3 hours".to_string(),
            copy_text: "$ uptime\nup 3 hours".to_string(),
        };
        c.copy_feedback = true;
        c.reset();
        assert!(c.query.is_empty());
        assert!(matches!(c.panel, PanelState::Idle));
        assert!(!c.copy_feedback);
    }

    // ── Cmd::execute ──────────────────────────────────────────────────────────

    #[test]
    fn execute_no_match_returns_handled() {
        let mut c = Cmd::new();
        let (_, evt) = c.execute("unknown", &config_with(vec![]));
        assert_eq!(evt, ComponentEvent::Handled);
    }

    #[test]
    fn execute_display_result_transitions_to_running() {
        let mut c = Cmd::new();
        c.query = "hi".to_string();
        let cfg = config_with(vec![display_cmd("hi", "echo hello")]);
        let (_, evt) = c.execute("hi", &cfg);
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(matches!(&c.panel, PanelState::Running { prompt } if prompt == "hi"));
        assert!(c.query.is_empty(), "query should be cleared on execute");
    }

    #[test]
    fn execute_silent_returns_exit() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![silent_cmd("noop", "true")]);
        let (_, evt) = c.execute("noop", &cfg);
        assert_eq!(evt, ComponentEvent::Exit);
    }

    #[test]
    fn execute_trims_query_whitespace() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![display_cmd("ip", "echo 1.2.3.4")]);
        let (_, evt) = c.execute("  ip  ", &cfg);
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(matches!(c.panel, PanelState::Running { .. }));
    }

    // ── Msg::CommandOutput (async completion) ─────────────────────────────────

    #[test]
    fn command_output_ok_sets_result_panel() {
        let mut c = Cmd::new();
        c.panel = PanelState::Running { prompt: "hi".to_string() };
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) = c.update(Msg::CommandOutput(Ok("hello".to_string())), &apps, &Config::default());
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(matches!(&c.panel, PanelState::Result { output, .. } if output == "hello"));
    }

    #[test]
    fn command_output_empty_shows_no_output_placeholder() {
        let mut c = Cmd::new();
        c.panel = PanelState::Running { prompt: "noop".to_string() };
        let apps: Vec<AppEntry> = vec![];
        let (_, _) = c.update(Msg::CommandOutput(Ok("(no output)".to_string())), &apps, &Config::default());
        assert!(matches!(&c.panel, PanelState::Result { output, .. } if output == "(no output)"));
    }

    #[test]
    fn command_output_err_shows_error_string() {
        let mut c = Cmd::new();
        c.panel = PanelState::Running { prompt: "oops".to_string() };
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) = c.update(Msg::CommandOutput(Err("Error: no such file".to_string())), &apps, &Config::default());
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(matches!(&c.panel, PanelState::Result { output, .. } if output.contains("Error")));
    }

    #[test]
    fn command_output_preserves_prompt_from_running_state() {
        let mut c = Cmd::new();
        c.panel = PanelState::Running { prompt: "mycommand".to_string() };
        let apps: Vec<AppEntry> = vec![];
        let _ = c.update(Msg::CommandOutput(Ok("done".to_string())), &apps, &Config::default());
        assert!(matches!(&c.panel, PanelState::Result { prompt, .. } if prompt == "mycommand"));
    }

    // ── Copy button: must not exit when there is no output ────────────────────
    // Regression for: clicking Copy while idle/running caused launcher exit
    // because a button without on_press leaks Status::Ignored, which app.rs
    // maps to Message::Close / process::exit(0).

    #[test]
    fn copy_when_idle_returns_handled() {
        let mut c = Cmd::new(); // panel starts Idle
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) = c.update(Msg::Copy, &apps, &Config::default());
        assert_eq!(evt, ComponentEvent::Handled);
    }

    #[test]
    fn copy_when_running_returns_handled() {
        let mut c = Cmd::new();
        c.panel = PanelState::Running { prompt: "uptime".to_string() };
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) = c.update(Msg::Copy, &apps, &Config::default());
        assert_eq!(evt, ComponentEvent::Handled);
    }

    // ── Cmd::update misc ──────────────────────────────────────────────────────

    #[test]
    fn update_copied_clears_feedback() {
        let mut c = Cmd::new();
        c.copy_feedback = true;
        let apps: Vec<AppEntry> = vec![];
        let _ = c.update(Msg::Copied, &apps, &Config::default());
        assert!(!c.copy_feedback);
    }

}
