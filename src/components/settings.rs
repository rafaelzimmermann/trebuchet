use iced::{
    event::Status,
    keyboard::{self, key::Named, Key},
    time,
    widget::{button, column, container, mouse_area, row, scrollable, svg, text, Space},
    Alignment, Background, Border, Color, Element, Event, Font, Length, Subscription, Task,
};
use std::path::PathBuf;
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
    /// No command run yet — show the list of available sub-commands.
    Idle,
    /// Last command produced a result (success or error).
    Result {
        prompt: String,
        output: String,
        /// Pre-built copy text (prompt + output joined).
        copy_text: String,
    },
}

pub struct Settings {
    query: String,
    panel: PanelState,
    copy_feedback: bool,
    shake: ShakeState,
}

#[derive(Debug, Clone)]
pub enum Msg {
    QueryChanged(String),
    /// Absorbs mouse clicks on the panel so they don't propagate as Ignored
    /// (which would close the window).
    PanelClick,
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
        }
    }

    pub fn reset(&mut self) {
        self.query = String::new();
        self.shake = ShakeState::default();
        self.copy_feedback = false;
        self.panel = PanelState::Idle;
    }

    /// Parse the current query and execute the matching sub-command.
    /// Returns a ComponentEvent to propagate upward if the theme was applied.
    fn execute(&mut self, query: &str) -> ComponentEvent {
        let q = query.trim();

        // ── theme <name> ───────────────────────────────────────────────────────
        if q == "theme" || q.starts_with("theme ") {
            let name = q.strip_prefix("theme").unwrap_or("").trim();
            if name.is_empty() {
                // Show theme usage + list
                let themes = list_themes();
                let list = if themes.is_empty() {
                    "  (no themes found in ~/.config/trebuchet/themes/)".into()
                } else {
                    themes.iter().map(|n| format!("  {n}")).collect::<Vec<_>>().join("\n")
                };
                let output = format!("Usage: theme <name>\n\nAvailable themes:\n{list}");
                let copy_text = format!("$ {q}\n{output}");
                self.panel = PanelState::Result { prompt: q.to_string(), output, copy_text };
                self.query.clear();
                return ComponentEvent::Handled;
            }

            let path = themes_dir().map(|d| d.join(format!("{}.conf", name)));
            match path.and_then(|p| crate::theme::Theme::from_file(&p)) {
                Some(theme) => {
                    let output = format!("Theme '{name}' applied.");
                    let copy_text = format!("$ {q}\n{output}");
                    self.panel = PanelState::Result {
                        prompt: q.to_string(),
                        output,
                        copy_text,
                    };
                    self.query.clear();
                    self.copy_feedback = false;
                    return ComponentEvent::ThemeChanged(name.to_string(), theme);
                }
                None => {
                    let themes = list_themes();
                    let list = if themes.is_empty() {
                        "  (no themes found in ~/.config/trebuchet/themes/)".into()
                    } else {
                        themes.iter().map(|n| format!("  {n}")).collect::<Vec<_>>().join("\n")
                    };
                    let output = format!(
                        "Theme '{name}' not found.\n\nAvailable themes:\n{list}"
                    );
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
        // ── Panel body ────────────────────────────────────────────────────────
        let (idle_color, text_color, prompt_color) =
            (config.theme.ai_idle, config.theme.terminal_output, config.theme.terminal_prompt);

        let body: Element<'a, Msg> = match &self.panel {
            PanelState::Idle => {
                let themes = list_themes();
                let theme_list: String = if themes.is_empty() {
                    "  (no themes found in ~/.config/trebuchet/themes/)".into()
                } else {
                    themes.iter().map(|n| format!("  {n}")).collect::<Vec<_>>().join("\n")
                };
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

        // ── Action bar (ai_agent style) ───────────────────────────────────────
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
