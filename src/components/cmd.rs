use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use iced::{
    alignment,
    event::Status,
    keyboard::{self, key::Named, Key},
    time,
    widget::{button, column, container, row, scrollable, text, Space},
    Alignment, Background, Border, Element, Event, Font, Length, Subscription, Task,
};
use std::time::Duration;

use super::command::{ComponentEvent, SlashCommand};
use super::component::Component;
use crate::config::{Config, CustomCommand};
use crate::launcher::AppEntry;
use crate::ui::panel::{icon_btn, PanelState, COPY_ICON};
use crate::ui::{search_bar, SearchIcon, ShakeState, PANEL_PADDING};

pub struct Cmd {
    query: String,
    filtered: Vec<usize>,
    page: usize,
    selected: Option<usize>,
    panel: PanelState,
    copy_feedback: bool,
    shake: ShakeState,
}

#[derive(Debug, Clone)]
pub enum Msg {
    QueryChanged(String),
    CommandActivated(usize),
    GoToPage(usize),
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
            filtered: Vec::new(),
            page: 0,
            selected: None,
            panel: PanelState::Idle,
            copy_feedback: false,
            shake: ShakeState::default(),
        }
    }

    pub fn reset(&mut self, config: &Config) {
        self.query = String::new();
        self.panel = PanelState::Idle;
        self.copy_feedback = false;
        self.shake = ShakeState::default();
        self.page = 0;
        self.selected = None;
        self.apply_filter(&config.commands, "");
    }

    /// Filter `commands` by `query` (fuzzy match on prefix), resetting page/selection.
    pub fn apply_filter(&mut self, commands: &[CustomCommand], query: &str) {
        if query.is_empty() {
            self.filtered = (0..commands.len()).collect();
        } else {
            let matcher = SkimMatcherV2::default();
            let mut scored: Vec<(usize, i64)> = commands
                .iter()
                .enumerate()
                .filter_map(|(i, c)| {
                    matcher.fuzzy_match(&c.prefix, query).map(|s| (i, s))
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }
        self.selected = if !query.is_empty() && !self.filtered.is_empty() {
            Some(0)
        } else {
            None
        };
        self.page = 0;
    }

    fn move_selection(&mut self, delta: isize, config: &Config) {
        let page_size = config.columns * config.rows;
        if self.filtered.is_empty() {
            return;
        }
        let current = self.selected.unwrap_or(self.page * page_size);
        let next = (current as isize + delta)
            .clamp(0, self.filtered.len() as isize - 1) as usize;
        self.selected = Some(next);
        self.page = next / page_size;
    }

    fn handle_page(&mut self, delta: i32, config: &Config) -> ComponentEvent {
        let page_size = config.columns * config.rows;
        let total = pages(self.filtered.len(), page_size);
        if delta > 0 {
            if self.page + 1 < total {
                self.page += 1;
            }
        } else if self.page > 0 {
            self.page -= 1;
        }
        ComponentEvent::Handled
    }

    /// Run the command at `commands[idx]`. Returns Exit for silent commands,
    /// otherwise transitions to `Running` and returns Handled.
    fn execute_idx(&mut self, idx: usize, config: &Config) -> (Task<Msg>, ComponentEvent) {
        let Some(cmd) = config.commands.get(idx) else {
            self.shake = ShakeState::trigger();
            return (Task::none(), ComponentEvent::Handled);
        };
        let shell_cmd = cmd.command.clone();
        let prompt = cmd.prefix.clone();
        if cmd.display_result {
            self.panel = PanelState::Running { prompt };
            self.query.clear();
            self.selected = None;
            self.copy_feedback = false;
            // Reset the filter so the grid shows every command when the user
            // returns to Idle after the result is displayed.
            self.apply_filter(&config.commands, "");
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
    }

    fn handle_char(&mut self, c: String, config: &Config) -> (Task<Msg>, ComponentEvent) {
        // Typing into a Running panel is meaningless; into a Result panel the
        // user is starting a new search, so drop back to Idle.
        if !matches!(self.panel, PanelState::Idle) {
            self.panel = PanelState::Idle;
        }
        self.query.push_str(&c);

        if let Some((cmd, args)) = SlashCommand::detect(&self.query) {
            if matches!(
                cmd,
                SlashCommand::App
                    | SlashCommand::Config
                    | SlashCommand::Cmd
                    | SlashCommand::Mv
            ) {
                self.query.clear();
                self.apply_filter(&config.commands, "");
                return (Task::none(), ComponentEvent::CommandInvoked(cmd, args));
            }
        }

        let q = self.query.clone();
        self.apply_filter(&config.commands, &q);
        (Task::none(), ComponentEvent::Handled)
    }

    fn handle_backspace(&mut self, config: &Config) -> (Task<Msg>, ComponentEvent) {
        if !matches!(self.panel, PanelState::Idle) {
            self.panel = PanelState::Idle;
        }
        self.query.pop();
        let q = self.query.clone();
        self.apply_filter(&config.commands, &q);
        (Task::none(), ComponentEvent::Handled)
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

        let is_idle = matches!(self.panel, PanelState::Idle);

        match key {
            Key::Named(Named::Escape) => {
                (Task::none(), ComponentEvent::CommandInvoked(SlashCommand::App, String::new()))
            }

            Key::Named(Named::Enter) => {
                // While a `display_result` command is running, ignore Enter.
                if matches!(self.panel, PanelState::Running { .. }) {
                    return (Task::none(), ComponentEvent::Handled);
                }
                // Allow slash-command navigation typed in the search bar.
                let q = self.query.trim().to_string();
                if let Some(evt) = SlashCommand::as_nav_event(&format!("{} ", &q)) {
                    self.query.clear();
                    self.apply_filter(&config.commands, "");
                    return (Task::none(), evt);
                }
                // Run the highlighted command from the grid.
                if let Some(sel) = self.selected {
                    if let Some(&cmd_idx) = self.filtered.get(sel) {
                        return self.execute_idx(cmd_idx, config);
                    }
                }
                self.shake = ShakeState::trigger();
                (Task::none(), ComponentEvent::Handled)
            }

            Key::Named(Named::PageDown) if is_idle => (Task::none(), self.handle_page(1, config)),
            Key::Named(Named::PageUp) if is_idle => (Task::none(), self.handle_page(-1, config)),

            Key::Named(Named::ArrowRight) if status == Status::Ignored && is_idle => {
                self.move_selection(1, config);
                (Task::none(), ComponentEvent::Handled)
            }
            Key::Named(Named::ArrowLeft) if status == Status::Ignored && is_idle => {
                self.move_selection(-1, config);
                (Task::none(), ComponentEvent::Handled)
            }
            Key::Named(Named::ArrowDown) if status == Status::Ignored && is_idle => {
                self.move_selection(config.columns as isize, config);
                (Task::none(), ComponentEvent::Handled)
            }
            Key::Named(Named::ArrowUp) if status == Status::Ignored && is_idle => {
                self.move_selection(-(config.columns as isize), config);
                (Task::none(), ComponentEvent::Handled)
            }

            Key::Named(Named::Backspace) if status == Status::Ignored => {
                self.handle_backspace(config)
            }

            Key::Named(Named::Space) if status == Status::Ignored => {
                self.handle_char(" ".to_string(), config)
            }

            Key::Character(_)
                if status == Status::Ignored
                    && !modifiers.control()
                    && !modifiers.alt()
                    && !modifiers.logo() =>
            {
                if let Some(t) = text.as_ref() {
                    self.handle_char(t.to_string(), config)
                } else {
                    (Task::none(), ComponentEvent::Handled)
                }
            }

            _ => (Task::none(), ComponentEvent::Handled),
        }
    }

    fn update(
        &mut self,
        msg: Msg,
        _apps: &[AppEntry],
        config: &Config,
    ) -> (Task<Msg>, ComponentEvent) {
        match msg {
            Msg::QueryChanged(s) => {
                if let Some((cmd, args)) = SlashCommand::detect(&s) {
                    if matches!(
                        cmd,
                        SlashCommand::App
                            | SlashCommand::Config
                            | SlashCommand::Cmd
                            | SlashCommand::Mv
                    ) {
                        self.query = String::new();
                        self.panel = PanelState::Idle;
                        self.apply_filter(&config.commands, "");
                        return (Task::none(), ComponentEvent::CommandInvoked(cmd, args));
                    }
                }
                if !matches!(self.panel, PanelState::Idle) {
                    self.panel = PanelState::Idle;
                }
                self.apply_filter(&config.commands, &s);
                self.query = s;
            }
            Msg::CommandActivated(idx) => {
                return self.execute_idx(idx, config);
            }
            Msg::GoToPage(p) => {
                let page_size = config.columns * config.rows;
                let total = pages(self.filtered.len(), page_size);
                self.page = p.min(total.saturating_sub(1));
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
        let (idle_color, text_color, prompt_color) = (
            config.theme.ai_idle,
            config.theme.terminal_output,
            config.theme.terminal_prompt,
        );

        let page_size = config.columns * config.rows;
        let total_pages = pages(self.filtered.len(), page_size);
        let start = self.page * page_size;
        let end = (start + page_size).min(self.filtered.len());
        let page_slice = &self.filtered[start..end];
        let highlighted = self.selected.and_then(|s| {
            if s >= start && s < end { Some(s - start) } else { None }
        });

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
                    command_grid(&config.commands, page_slice, config, highlighted)
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

        // Wrap Running/Result text bodies in the terminal-style scrollable
        // panel; show the grid raw (no inner background) for Idle so it
        // matches the app launcher's look.
        let panel_bg = config.theme.terminal_background;
        let panel: Element<'a, Msg> = if matches!(self.panel, PanelState::Idle) {
            container(body).width(Length::Fill).height(Length::Fill).into()
        } else {
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
            .padding([16, 20])
            .into()
        };

        let has_result = matches!(self.panel, PanelState::Result { .. });
        let (btn_bg, feedback_color) =
            (config.theme.button_background, config.theme.copy_feedback);

        let feedback: Element<'a, Msg> = if self.copy_feedback {
            text("Copied to clipboard").size(13).color(feedback_color).into()
        } else {
            text("").size(13).into()
        };

        // Pagination dots only make sense while the grid is visible.
        let pagination: Element<'a, Msg> = if matches!(self.panel, PanelState::Idle) {
            let dots: Vec<Element<'_, Msg>> = (0..total_pages)
                .map(|i| {
                    let color = if i == self.page {
                        config.theme.dot_active
                    } else {
                        config.theme.dot_inactive
                    };
                    button(text("●").size(10).color(color))
                        .on_press(Msg::GoToPage(i))
                        .padding([4, 5])
                        .style(|_theme, _status| button::Style {
                            background: None,
                            ..Default::default()
                        })
                        .into()
                })
                .collect();
            container(row(dots).spacing(2))
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .into()
        } else {
            text("").size(10).into()
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
                search_bar(
                    &self.query,
                    &self.shake,
                    SearchIcon::Terminal,
                    "Search commands\u{2026}",
                    &config.theme,
                    Msg::QueryChanged,
                ),
                panel,
                pagination,
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

// ── Grid widget for command cells ─────────────────────────────────────────────

fn command_grid<'a>(
    commands: &'a [CustomCommand],
    indices: &[usize],
    config: &Config,
    highlighted: Option<usize>,
) -> Element<'a, Msg> {
    let mut rows: Vec<Element<'a, Msg>> = indices
        .chunks(config.columns)
        .enumerate()
        .map(|(row_idx, chunk)| {
            let mut cells: Vec<Element<'a, Msg>> = chunk
                .iter()
                .enumerate()
                .map(|(col_idx, &cmd_idx)| {
                    let page_position = row_idx * config.columns + col_idx;
                    let is_selected = highlighted == Some(page_position);
                    let cmd = &commands[cmd_idx];

                    let prefix = text(format!("/{}", cmd.prefix))
                        .font(Font::MONOSPACE)
                        .size(15)
                        .color(config.theme.terminal_prompt);

                    let command_text = text(cmd.command.as_str())
                        .font(Font::MONOSPACE)
                        .size(11)
                        .color(config.theme.ai_idle);

                    let cell = column![prefix, command_text]
                        .align_x(Alignment::Center)
                        .spacing(4);

                    let (label_color, selected_bg) =
                        (config.theme.app_label, config.theme.app_selected);
                    button(cell)
                        .on_press(Msg::CommandActivated(cmd_idx))
                        .padding(12)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(move |_theme, _status| button::Style {
                            text_color: label_color,
                            background: if is_selected {
                                Some(Background::Color(selected_bg))
                            } else {
                                None
                            },
                            border: if is_selected {
                                Border { radius: 8.0.into(), ..Default::default() }
                            } else {
                                Border::default()
                            },
                            ..Default::default()
                        })
                        .into()
                })
                .collect();

            // Pad short rows so every column lines up with the rows above.
            while cells.len() < config.columns {
                cells.push(Space::new().width(Length::Fill).height(Length::Fill).into());
            }

            row(cells).width(Length::Fill).height(Length::Fill).into()
        })
        .collect();

    // Pad missing rows so the grid height stays constant while searching.
    while rows.len() < config.rows {
        let cells: Vec<Element<'a, Msg>> = (0..config.columns)
            .map(|_| Space::new().width(Length::Fill).height(Length::Fill).into())
            .collect();
        rows.push(row(cells).width(Length::Fill).height(Length::Fill).into());
    }

    column(rows).width(Length::Fill).height(Length::Fill).into()
}

fn pages(total: usize, page_size: usize) -> usize {
    if page_size == 0 {
        1
    } else {
        total.div_ceil(page_size)
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
        CustomCommand {
            prefix: prefix.to_string(),
            command: command.to_string(),
            display_result: false,
        }
    }

    fn display_cmd(prefix: &str, command: &str) -> CustomCommand {
        CustomCommand {
            prefix: prefix.to_string(),
            command: command.to_string(),
            display_result: true,
        }
    }

    // ── Cmd::new / reset ──────────────────────────────────────────────────────

    #[test]
    fn new_starts_empty_and_idle() {
        let c = Cmd::new();
        assert!(c.query.is_empty());
        assert!(c.filtered.is_empty());
        assert_eq!(c.page, 0);
        assert_eq!(c.selected, None);
        assert!(matches!(c.panel, PanelState::Idle));
        assert!(!c.copy_feedback);
    }

    #[test]
    fn reset_clears_query_and_panel_and_populates_filter() {
        let mut c = Cmd::new();
        c.query = "uptime".to_string();
        c.panel = PanelState::Result {
            prompt: "uptime".to_string(),
            output: "up 3 hours".to_string(),
            copy_text: "$ uptime\nup 3 hours".to_string(),
        };
        c.copy_feedback = true;
        c.page = 2;
        c.selected = Some(3);
        let cfg = config_with(vec![silent_cmd("a", "echo a"), silent_cmd("b", "echo b")]);
        c.reset(&cfg);
        assert!(c.query.is_empty());
        assert!(matches!(c.panel, PanelState::Idle));
        assert!(!c.copy_feedback);
        assert_eq!(c.page, 0);
        assert_eq!(c.selected, None);
        assert_eq!(c.filtered, vec![0, 1], "reset should show all commands");
    }

    // ── apply_filter ──────────────────────────────────────────────────────────

    #[test]
    fn apply_filter_empty_shows_all() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![silent_cmd("hi", "x"), silent_cmd("bye", "x")]);
        c.apply_filter(&cfg.commands, "");
        assert_eq!(c.filtered, vec![0, 1]);
        assert_eq!(c.selected, None);
    }

    #[test]
    fn apply_filter_matches_prefix() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![
            silent_cmd("uptime", "x"),
            silent_cmd("ip", "x"),
            silent_cmd("shutdown", "x"),
        ]);
        c.apply_filter(&cfg.commands, "up");
        assert!(c.filtered.contains(&0), "uptime matches 'up'");
        assert!(!c.filtered.contains(&1), "ip does not match 'up'");
        assert!(!c.filtered.contains(&2));
        assert_eq!(c.selected, Some(0));
    }

    #[test]
    fn apply_filter_no_match_yields_empty() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![silent_cmd("hi", "x")]);
        c.apply_filter(&cfg.commands, "zz");
        assert!(c.filtered.is_empty());
        assert_eq!(c.selected, None);
    }

    #[test]
    fn apply_filter_resets_page() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![silent_cmd("hi", "x")]);
        c.page = 5;
        c.apply_filter(&cfg.commands, "hi");
        assert_eq!(c.page, 0);
    }

    // ── move_selection / handle_page ──────────────────────────────────────────

    #[test]
    fn move_selection_advances_within_filtered() {
        let mut c = Cmd::new();
        let cfg = Config { columns: 3, rows: 2, ..Config::default() };
        c.filtered = vec![10, 20, 30, 40];
        c.selected = Some(0);
        c.move_selection(1, &cfg);
        assert_eq!(c.selected, Some(1));
    }

    #[test]
    fn move_selection_clamps_at_end() {
        let mut c = Cmd::new();
        let cfg = Config { columns: 3, rows: 2, ..Config::default() };
        c.filtered = vec![10, 20];
        c.selected = Some(1);
        c.move_selection(1, &cfg);
        assert_eq!(c.selected, Some(1));
    }

    #[test]
    fn move_selection_noop_when_empty() {
        let mut c = Cmd::new();
        let cfg = Config::default();
        c.filtered = vec![];
        c.move_selection(1, &cfg);
        assert_eq!(c.selected, None);
    }

    #[test]
    fn handle_page_advances_and_clamps() {
        let mut c = Cmd::new();
        let cfg = Config { columns: 2, rows: 1, ..Config::default() };
        c.filtered = vec![0, 1, 2, 3, 4]; // 3 pages
        assert_eq!(c.handle_page(1, &cfg), ComponentEvent::Handled);
        assert_eq!(c.page, 1);
        assert_eq!(c.handle_page(1, &cfg), ComponentEvent::Handled);
        assert_eq!(c.page, 2);
        // Clamp at last.
        assert_eq!(c.handle_page(1, &cfg), ComponentEvent::Handled);
        assert_eq!(c.page, 2);
        // Prev.
        assert_eq!(c.handle_page(-1, &cfg), ComponentEvent::Handled);
        assert_eq!(c.page, 1);
        // Clamp at zero.
        c.page = 0;
        assert_eq!(c.handle_page(-1, &cfg), ComponentEvent::Handled);
        assert_eq!(c.page, 0);
    }

    // ── execute_idx ───────────────────────────────────────────────────────────

    #[test]
    fn execute_idx_unknown_shakes_and_handled() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![]);
        let (_, evt) = c.execute_idx(0, &cfg);
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(c.shake.active, "out-of-range index should shake");
    }

    #[test]
    fn execute_idx_display_result_transitions_to_running() {
        let mut c = Cmd::new();
        c.query = "hi".to_string();
        c.selected = Some(0);
        let cfg = config_with(vec![display_cmd("hi", "echo hello")]);
        let (_, evt) = c.execute_idx(0, &cfg);
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(matches!(&c.panel, PanelState::Running { prompt } if prompt == "hi"));
        assert!(c.query.is_empty(), "query should be cleared on execute");
        assert_eq!(c.selected, None, "selection cleared on execute");
        assert_eq!(c.filtered, vec![0], "filter reset to all commands");
    }

    #[test]
    fn execute_idx_silent_returns_exit() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![silent_cmd("noop", "true")]);
        let (_, evt) = c.execute_idx(0, &cfg);
        assert_eq!(evt, ComponentEvent::Exit);
    }

    // ── Msg::CommandOutput (async completion) ─────────────────────────────────

    #[test]
    fn command_output_ok_sets_result_panel() {
        let mut c = Cmd::new();
        c.panel = PanelState::Running { prompt: "hi".to_string() };
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) =
            c.update(Msg::CommandOutput(Ok("hello".to_string())), &apps, &Config::default());
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(matches!(&c.panel, PanelState::Result { output, .. } if output == "hello"));
    }

    #[test]
    fn command_output_empty_shows_no_output_placeholder() {
        let mut c = Cmd::new();
        c.panel = PanelState::Running { prompt: "noop".to_string() };
        let apps: Vec<AppEntry> = vec![];
        let _ = c.update(
            Msg::CommandOutput(Ok("(no output)".to_string())),
            &apps,
            &Config::default(),
        );
        assert!(matches!(&c.panel, PanelState::Result { output, .. } if output == "(no output)"));
    }

    #[test]
    fn command_output_err_shows_error_string() {
        let mut c = Cmd::new();
        c.panel = PanelState::Running { prompt: "oops".to_string() };
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) = c.update(
            Msg::CommandOutput(Err("Error: no such file".to_string())),
            &apps,
            &Config::default(),
        );
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

    // ── CommandActivated msg ──────────────────────────────────────────────────

    #[test]
    fn command_activated_runs_the_command() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![display_cmd("hi", "echo hi")]);
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) = c.update(Msg::CommandActivated(0), &apps, &cfg);
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(matches!(&c.panel, PanelState::Running { prompt } if prompt == "hi"));
    }

    #[test]
    fn command_activated_invalid_index_shakes() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![]);
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) = c.update(Msg::CommandActivated(7), &apps, &cfg);
        assert_eq!(evt, ComponentEvent::Handled);
        assert!(c.shake.active);
    }

    // ── GoToPage ──────────────────────────────────────────────────────────────

    #[test]
    fn go_to_page_clamps_to_last() {
        let mut c = Cmd::new();
        let cfg = Config { columns: 2, rows: 1, ..Config::default() };
        c.filtered = vec![0, 1, 2, 3]; // 2 pages
        let apps: Vec<AppEntry> = vec![];
        let _ = c.update(Msg::GoToPage(99), &apps, &cfg);
        assert_eq!(c.page, 1);
    }

    // ── QueryChanged — filter + slash nav ─────────────────────────────────────

    #[test]
    fn query_changed_filters_commands() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![silent_cmd("uptime", "x"), silent_cmd("ip", "x")]);
        let apps: Vec<AppEntry> = vec![];
        let _ = c.update(Msg::QueryChanged("up".to_string()), &apps, &cfg);
        assert_eq!(c.query, "up");
        assert_eq!(c.filtered, vec![0]);
        assert_eq!(c.selected, Some(0));
    }

    #[test]
    fn query_changed_slash_command_invokes_navigation() {
        let mut c = Cmd::new();
        let cfg = config_with(vec![silent_cmd("hi", "x")]);
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) = c.update(Msg::QueryChanged("/app ".to_string()), &apps, &cfg);
        assert!(matches!(evt, ComponentEvent::CommandInvoked(SlashCommand::App, _)));
        assert!(c.query.is_empty());
        assert!(matches!(c.panel, PanelState::Idle));
    }

    #[test]
    fn query_changed_resets_result_to_idle() {
        let mut c = Cmd::new();
        c.panel = PanelState::Result {
            prompt: "old".to_string(),
            output: "out".to_string(),
            copy_text: "$ old\nout".to_string(),
        };
        let cfg = config_with(vec![silent_cmd("hi", "x")]);
        let apps: Vec<AppEntry> = vec![];
        let _ = c.update(Msg::QueryChanged("h".to_string()), &apps, &cfg);
        assert!(matches!(c.panel, PanelState::Idle));
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

    // ── pages helper ──────────────────────────────────────────────────────────

    #[test]
    fn pages_zero_items() {
        assert_eq!(pages(0, 6), 0);
    }

    #[test]
    fn pages_exact_fit() {
        assert_eq!(pages(6, 6), 1);
    }

    #[test]
    fn pages_one_over() {
        assert_eq!(pages(7, 6), 2);
    }

    #[test]
    fn pages_zero_page_size_returns_one() {
        assert_eq!(pages(10, 0), 1);
    }
}
