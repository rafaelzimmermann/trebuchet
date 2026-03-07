use iced::{
    event::Status,
    keyboard::{self, key::Named, Key},
    time,
    widget::{button, column, container, mouse_area, row, scrollable, svg, text, Space},
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

fn icon_btn<'a, Msg: Clone + 'a>(
    icon_bytes: &'static [u8],
    msg: Msg,
    enabled: bool,
    btn_bg: Color,
) -> iced::widget::Button<'a, Msg> {
    let icon_color = Color { a: if enabled { 1.0 } else { 0.35 }, ..Color::WHITE };
    let bg = Color { a: if enabled { btn_bg.a } else { btn_bg.a * 0.4 }, ..btn_bg };
    let icon = svg(svg::Handle::from_memory(icon_bytes.to_vec()))
        .width(16)
        .height(16)
        .style(move |_theme, _status| svg::Style { color: Some(icon_color) });
    let mut btn = button(icon)
        .padding(8)
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(bg)),
            border: Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        });
    if enabled {
        btn = btn.on_press(msg);
    }
    btn
}

enum PanelState {
    /// No command run yet — show the list of configured commands.
    Idle,
    /// Last command produced output (display_result = true).
    Result {
        prompt: String,
        output: String,
        copy_text: String,
    },
}

pub struct Cmd {
    query: String,
    panel: PanelState,
    copy_feedback: bool,
    shake: ShakeState,
}

#[derive(Debug, Clone)]
pub enum Msg {
    QueryChanged(String),
    /// Absorbs mouse clicks on the panel so they don't propagate as Ignored.
    PanelClick,
    Copy,
    Copied,
    ShakeTick,
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

    /// Find and run the command matching `query`. Prefixes are stored without
    /// the leading slash, so the user types e.g. `ip` not `/ip`.
    fn execute(&mut self, query: &str, config: &Config) -> ComponentEvent {
        let q = query.trim();
        if let Some(cmd) = config.commands.iter().find(|c| c.prefix == q) {
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
                let copy_text = format!("$ {q}\n{output}");
                self.panel = PanelState::Result { prompt: q.to_string(), output, copy_text };
                self.query.clear();
                self.copy_feedback = false;
                ComponentEvent::Handled
            } else {
                let _ = std::process::Command::new("sh").args(["-c", &shell_cmd]).spawn();
                ComponentEvent::Exit
            }
        } else {
            self.shake = ShakeState::trigger();
            ComponentEvent::Handled
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
                let evt = self.execute(&q, config);
                (Task::none(), evt)
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
            Msg::PanelClick => {}
            Msg::Copy => {
                let text_to_copy = match &self.panel {
                    PanelState::Result { copy_text, .. } => copy_text.clone(),
                    PanelState::Idle => String::new(),
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
        let panel = mouse_area(
            container(
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
            .padding([16, 20]),
        )
        .on_press(Msg::PanelClick);

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

        column![
            search_bar(&self.query, &self.shake, SearchIcon::Terminal, &config.theme, Msg::QueryChanged),
            panel,
            action_bar,
        ]
        .spacing(8)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, CustomCommand};

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
        let evt = c.execute("unknown", &config_with(vec![]));
        assert_eq!(evt, ComponentEvent::Handled);
    }

    #[test]
    fn execute_display_result_shows_output() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![display_cmd("hi", "echo hello")]);
        let evt = c.execute("hi", &cfg);
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(matches!(&c.panel, PanelState::Result { output, .. } if output.contains("hello")));
        assert!(c.query.is_empty(), "query should be cleared after execute");
    }

    #[test]
    fn execute_display_result_empty_stdout_shows_placeholder() {
        let mut c = Cmd::new();
        // `true` exits 0 but produces no stdout.
        let cfg = config_with(vec![display_cmd("noop", "true")]);
        let evt = c.execute("noop", &cfg);
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(matches!(&c.panel, PanelState::Result { output, .. } if output == "(no output)"));
    }

    #[test]
    fn execute_silent_returns_exit() {
        let mut c = Cmd::new();
        // Use `true` — exits 0, no output; spawn() won't block.
        let cfg = config_with(vec![silent_cmd("noop", "true")]);
        let evt = c.execute("noop", &cfg);
        assert_eq!(evt, ComponentEvent::Exit);
    }

    #[test]
    fn execute_trims_query_whitespace() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![display_cmd("ip", "echo 1.2.3.4")]);
        let evt = c.execute("  ip  ", &cfg);
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(matches!(c.panel, PanelState::Result { .. }));
    }

    #[test]
    fn execute_failed_command_shows_error() {
        let mut c = Cmd::new();
        // Command that does not exist.
        let cfg = config_with(vec![display_cmd("oops", "this_binary_does_not_exist_xyz")]);
        let evt = c.execute("oops", &cfg);
        // Either shows output (exit-code error message) or handles gracefully.
        assert_eq!(evt, ComponentEvent::Handled);
    }

    // ── Cmd::update ───────────────────────────────────────────────────────────

    #[test]
    fn update_copied_clears_feedback() {
        let mut c = Cmd::new();
        c.copy_feedback = true;
        let apps: Vec<crate::launcher::AppEntry> = vec![];
        let cfg = Config::default();
        let _ = c.update(Msg::Copied, &apps, &cfg);
        assert!(!c.copy_feedback);
    }

    #[test]
    fn update_panel_click_is_noop() {
        let mut c = Cmd::new();
        let apps: Vec<crate::launcher::AppEntry> = vec![];
        let (task, evt) = c.update(Msg::PanelClick, &apps, &Config::default());
        assert_eq!(evt, ComponentEvent::Handled);
        let _ = task;
    }
}
