use iced::{
    event::Status,
    keyboard::{self, key::Named, Key},
    time,
    widget::{column, container, row, scrollable, text, Space},
    Alignment, Background, Border, Element, Event, Font, Length, Subscription, Task,
};
use std::path::PathBuf;
use std::time::Duration;

use super::command::{ComponentEvent, SlashCommand};
use super::component::Component;
use crate::config::Config;
use crate::launcher::AppEntry;
use crate::ui::panel::{icon_btn, PanelState, COPY_ICON};
use crate::ui::{search_bar, SearchIcon, ShakeState};

pub struct Settings {
    query: String,
    panel: PanelState,
    copy_feedback: bool,
    shake: ShakeState,
    /// Cached list of installed theme names, refreshed on reset().
    themes: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    QueryChanged(String),
    Copy,
    Copied,
    ShakeTick,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn themes_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config/trebuchet/themes"))
}

fn list_themes() -> Vec<String> {
    themes_dir()
        .and_then(|d| std::fs::read_dir(d).ok())
        .map(|entries| {
            let mut names: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let p = e.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("conf") {
                        p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            names.sort();
            names
        })
        .unwrap_or_default()
}

// ── Settings impl ─────────────────────────────────────────────────────────────

impl Settings {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            panel: PanelState::Idle,
            copy_feedback: false,
            shake: ShakeState::default(),
            themes: list_themes(),
        }
    }

    pub fn reset(&mut self) {
        self.query = String::new();
        self.shake = ShakeState::default();
        self.copy_feedback = false;
        self.panel = PanelState::Idle;
        self.themes = list_themes();
    }

    /// Parse the current query and execute the matching sub-command.
    /// Returns a ComponentEvent to propagate upward if the theme was applied.
    fn execute(&mut self, query: &str) -> ComponentEvent {
        let q = query.trim();

        // ── theme <name> ───────────────────────────────────────────────────────
        if q == "theme" || q.starts_with("theme ") {
            let name = q.strip_prefix("theme").unwrap_or("").trim();
            if name.is_empty() {
                let list = self.format_theme_list();
                let output = format!("Usage: theme <name>\n\nAvailable themes:\n{list}");
                let copy_text = format!("$ {q}\n{output}");
                self.panel = PanelState::Result { prompt: q.to_string(), output, copy_text };
                self.query.clear();
                return ComponentEvent::Handled;
            }

            let path = themes_dir().map(|d| d.join(format!("{name}.conf")));
            match path.and_then(|p| crate::theme::Theme::from_file(&p)) {
                Some(theme) => {
                    let output = format!("Theme '{name}' applied.");
                    let copy_text = format!("$ {q}\n{output}");
                    self.panel = PanelState::Result { prompt: q.to_string(), output, copy_text };
                    self.query.clear();
                    self.copy_feedback = false;
                    return ComponentEvent::ThemeChanged(name.to_string(), Box::new(theme));
                }
                None => {
                    let list = self.format_theme_list();
                    let output = format!("Theme '{name}' not found.\n\nAvailable themes:\n{list}");
                    let copy_text = format!("$ {q}\n{output}");
                    self.panel = PanelState::Result { prompt: q.to_string(), output, copy_text };
                    self.shake = ShakeState::trigger();
                    return ComponentEvent::Handled;
                }
            }
        }

        // ── Unknown sub-command ────────────────────────────────────────────────
        let output = format!("Unknown command: {q}\n\nAvailable commands:\n  theme <name>    switch the colour theme");
        let copy_text = format!("$ {q}\n{output}");
        self.panel = PanelState::Result { prompt: q.to_string(), output, copy_text };
        self.shake = ShakeState::trigger();
        ComponentEvent::Handled
    }

    fn format_theme_list(&self) -> String {
        if self.themes.is_empty() {
            "  (no themes found in ~/.config/trebuchet/themes/)".into()
        } else {
            self.themes.iter().map(|n| format!("  {n}")).collect::<Vec<_>>().join("\n")
        }
    }
}

// ── Component impl ────────────────────────────────────────────────────────────

impl Component for Settings {
    type Msg = Msg;

    fn handle_event(
        &mut self,
        event: &Event,
        status: Status,
        _apps: &[AppEntry],
        _config: &Config,
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
                let evt = self.execute(&q);
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
        }
        (Task::none(), ComponentEvent::Handled)
    }

    fn view<'a>(&'a self, _apps: &'a [AppEntry], config: &'a Config) -> Element<'a, Msg> {
        let (idle_color, text_color, prompt_color) =
            (config.theme.ai_idle, config.theme.terminal_output, config.theme.terminal_prompt);

        let body: Element<'a, Msg> = match &self.panel {
            PanelState::Idle => {
                let theme_list = self.format_theme_list();
                let mut items: Vec<Element<'a, Msg>> = vec![
                    text("Available commands:").size(13).color(prompt_color).into(),
                    text("  theme <name>    switch the colour theme")
                        .font(Font::MONOSPACE)
                        .size(14)
                        .color(idle_color)
                        .into(),
                    text("").size(6).into(),
                    text("Available themes:").size(13).color(prompt_color).into(),
                ];
                for line in theme_list.lines() {
                    items.push(
                        text(line.to_string())
                            .font(Font::MONOSPACE)
                            .size(14)
                            .color(idle_color)
                            .into(),
                    );
                }
                column(items).spacing(6).into()
            }
            // Settings never enters Running — this arm is here because PanelState
            // is shared with Cmd which does have async commands.
            PanelState::Running { .. } => column![].into(),
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
        .padding([16, 20]);

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
    use crate::components::command::ComponentEvent;
    use crate::ui::panel::PanelState;

    // Regression: Copy when idle must not exit the launcher.
    // A button rendered without on_press leaks Status::Ignored which app.rs
    // maps to Message::Close. The handler must return Handled in all non-Result states.

    #[test]
    fn copy_when_idle_returns_handled() {
        let mut s = Settings::new(); // panel starts Idle
        let apps: Vec<crate::launcher::AppEntry> = vec![];
        let (_, evt) = s.update(Msg::Copy, &apps, &crate::config::Config::default());
        assert_eq!(evt, ComponentEvent::Handled);
    }

    #[test]
    fn reset_leaves_panel_idle() {
        let mut s = Settings::new();
        s.panel = PanelState::Result {
            prompt: "theme dark".to_string(),
            output: "Theme 'dark' applied.".to_string(),
            copy_text: "$ theme dark\nTheme 'dark' applied.".to_string(),
        };
        s.reset();
        assert!(matches!(s.panel, PanelState::Idle));
        assert!(s.query.is_empty());
    }
}
