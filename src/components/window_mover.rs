use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use iced::{
    alignment,
    event::Status,
    keyboard::{self, key::Named, Key},
    time,
    widget::{button, column, container, image, mouse_area, row, svg, text, Space},
    Alignment, Background, Border, Element, Event, Length, Subscription, Task,
};
use serde::Deserialize;
use std::time::Duration;

use super::command::{ComponentEvent, SlashCommand};
use super::component::Component;
use crate::config::Config;
use crate::icons::{icon_for_window, IconHandle, FALLBACK_ICON};
use crate::launcher::AppEntry;
use crate::ui::{search_bar, SearchIcon, ShakeState, PANEL_PADDING};

// ── Hyprctl JSON shapes ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct HyprWorkspace {
    id: i64,
    name: String,
}

#[derive(Deserialize)]
struct HyprClient {
    address: String,
    class: String,
    title: String,
    #[serde(rename = "initialTitle")]
    initial_title: String,
    at: [i64; 2],
    workspace: HyprWorkspace,
}

#[derive(Deserialize)]
struct HyprActiveWorkspace {
    id: i64,
}

// ── Window data types ──────────────────────────────────────────────────────────

/// Raw data returned from the async hyprctl fetch — safe to `Send`.
#[derive(Debug, Clone)]
pub struct WindowData {
    pub address: String,
    pub class: String,
    pub title: String,
    pub initial_title: String,
    pub workspace_name: String,
}

/// Fully resolved entry with an optional icon handle.
#[derive(Clone)]
pub struct WindowEntry {
    pub address: String,
    pub class: String,
    pub title: String,
    pub workspace_name: String,
    pub icon: Option<IconHandle>,
}

// ── Component state ────────────────────────────────────────────────────────────

pub struct WindowMover {
    pub query: String,
    pub windows: Vec<WindowEntry>,
    pub filtered: Vec<usize>,
    pub page: usize,
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub shake: ShakeState,
    pub loading: bool,
    /// Active workspace ID populated on load; used as the move-target.
    active_workspace_id: i64,
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Msg {
    QueryChanged(String),
    WindowsLoaded(Vec<WindowData>, i64),
    LoadFailed,
    WindowActivated(usize),
    WindowHovered(Option<usize>),
    WindowMoved,
    GoToPage(usize),
    ShakeTick,
}

// ── Impl ──────────────────────────────────────────────────────────────────────

impl WindowMover {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            windows: Vec::new(),
            filtered: Vec::new(),
            page: 0,
            selected: None,
            hovered: None,
            shake: ShakeState::default(),
            loading: false,
            active_workspace_id: 0,
        }
    }

    /// Enter window-mover mode: clear state and kick off an async window fetch.
    pub fn reset(&mut self, args: String) -> Task<Msg> {
        self.query = args;
        self.windows = Vec::new();
        self.filtered = Vec::new();
        self.page = 0;
        self.selected = None;
        self.hovered = None;
        self.shake = ShakeState::default();
        self.loading = true;
        self.active_workspace_id = 0;
        Task::perform(fetch_windows(), |result| match result {
            Ok((data, active_id)) => Msg::WindowsLoaded(data, active_id),
            Err(_) => Msg::LoadFailed,
        })
    }

    fn apply_filter(&mut self, query: &str) {
        if query.is_empty() {
            self.filtered = (0..self.windows.len()).collect();
        } else {
            let matcher = SkimMatcherV2::default();
            let mut scored: Vec<(usize, i64)> = self
                .windows
                .iter()
                .enumerate()
                .filter_map(|(i, w)| {
                    // Match title and class only — workspace name is display-only.
                    let ts = matcher.fuzzy_match(&w.title, query);
                    let cs = matcher.fuzzy_match(&w.class, query);
                    let best = match (ts, cs) {
                        (Some(a), Some(b)) => Some(a.max(b)),
                        (a, b) => a.or(b),
                    };
                    best.map(|s| (i, s))
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

    fn handle_char(&mut self, c: String) -> (Task<Msg>, ComponentEvent) {
        self.query.push_str(&c);
        if let Some(evt) = SlashCommand::as_nav_event(&self.query) {
            self.query.clear();
            return (Task::none(), evt);
        }
        let q = self.query.clone();
        self.apply_filter(&q);
        (Task::none(), ComponentEvent::Handled)
    }

    fn handle_backspace(&mut self) -> (Task<Msg>, ComponentEvent) {
        self.query.pop();
        let q = self.query.clone();
        self.apply_filter(&q);
        (Task::none(), ComponentEvent::Handled)
    }

    fn handle_submit(&mut self, _config: &Config) -> (Task<Msg>, ComponentEvent) {
        if let Some(sel) = self.selected {
            if let Some(&win_idx) = self.filtered.get(sel) {
                if let Some(win) = self.windows.get(win_idx) {
                    return self.dispatch_move(win.address.clone());
                }
            }
        }
        self.shake = ShakeState::trigger();
        (Task::none(), ComponentEvent::Handled)
    }

    fn dispatch_move(&self, address: String) -> (Task<Msg>, ComponentEvent) {
        let active_ws = self.active_workspace_id;
        let task = Task::perform(
            async move { move_window(active_ws, address).await },
            |_| Msg::WindowMoved,
        );
        (task, ComponentEvent::Handled)
    }

    /// Window index (into `self.windows`) for the status bar: hover takes precedence.
    fn status_win_idx(&self) -> Option<usize> {
        self.hovered
            .or_else(|| self.selected.and_then(|s| self.filtered.get(s)).copied())
    }
}

// ── Component impl ────────────────────────────────────────────────────────────

impl Component for WindowMover {
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
            Key::Named(Named::Enter) => {
                let trimmed = self.query.trim().to_string();
                if let Some(evt) = SlashCommand::as_nav_event(&format!("{} ", trimmed)) {
                    self.query.clear();
                    return (Task::none(), evt);
                }
                self.handle_submit(config)
            }
            Key::Named(Named::Escape) => {
                (Task::none(), ComponentEvent::CommandInvoked(SlashCommand::App, String::new()))
            }
            Key::Named(Named::PageDown) => (Task::none(), self.handle_page(1, config)),
            Key::Named(Named::PageUp) => (Task::none(), self.handle_page(-1, config)),
            Key::Named(Named::ArrowRight) if status == Status::Ignored => {
                self.move_selection(1, config);
                (Task::none(), ComponentEvent::Handled)
            }
            Key::Named(Named::ArrowLeft) if status == Status::Ignored => {
                self.move_selection(-1, config);
                (Task::none(), ComponentEvent::Handled)
            }
            Key::Named(Named::ArrowDown) if status == Status::Ignored => {
                self.move_selection(config.columns as isize, config);
                (Task::none(), ComponentEvent::Handled)
            }
            Key::Named(Named::ArrowUp) if status == Status::Ignored => {
                self.move_selection(-(config.columns as isize), config);
                (Task::none(), ComponentEvent::Handled)
            }
            Key::Named(Named::Backspace) if status == Status::Ignored => self.handle_backspace(),
            Key::Named(Named::Space) if status == Status::Ignored => {
                self.handle_char(" ".to_string())
            }
            Key::Character(_)
                if status == Status::Ignored
                    && !modifiers.control()
                    && !modifiers.alt()
                    && !modifiers.logo() =>
            {
                if let Some(t) = text.as_ref() {
                    self.handle_char(t.to_string())
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
                if let Some(evt) = SlashCommand::as_nav_event(&s) {
                    self.query = String::new();
                    self.apply_filter("");
                    return (Task::none(), evt);
                }
                self.apply_filter(&s);
                self.query = s;
            }

            Msg::WindowsLoaded(data, active_id) => {
                self.loading = false;
                self.active_workspace_id = active_id;
                self.windows = data
                    .into_iter()
                    .map(|d| {
                        let icon = icon_for_window(&d.class, &d.initial_title);
                        WindowEntry {
                            address: d.address,
                            class: d.class,
                            title: d.title,
                            workspace_name: d.workspace_name,
                            icon,
                        }
                    })
                    .collect();
                let q = self.query.clone();
                self.apply_filter(&q);
            }

            Msg::LoadFailed => {
                self.loading = false;
            }

            Msg::WindowActivated(win_idx) => {
                if let Some(win) = self.windows.get(win_idx) {
                    return self.dispatch_move(win.address.clone());
                }
            }

            Msg::WindowHovered(idx) => {
                self.hovered = idx;
            }

            Msg::WindowMoved => {
                return (Task::none(), ComponentEvent::Exit);
            }

            Msg::GoToPage(p) => {
                let page_size = config.columns * config.rows;
                let total = pages(self.filtered.len(), page_size);
                self.page = p.min(total.saturating_sub(1));
            }

            Msg::ShakeTick => {
                self.shake.advance();
            }
        }
        (Task::none(), ComponentEvent::Handled)
    }

    fn view<'a>(&'a self, _apps: &'a [AppEntry], config: &'a Config) -> Element<'a, Msg> {
        let page_size = config.columns * config.rows;
        let total_pages = pages(self.filtered.len(), page_size);
        let start = self.page * page_size;
        let end = (start + page_size).min(self.filtered.len());
        let page_slice = &self.filtered[start..end];

        let highlighted = self.selected.and_then(|s| {
            if s >= start && s < end { Some(s - start) } else { None }
        });

        let faded = config.theme.search_placeholder;

        let grid: Element<'_, Msg> = if self.loading {
            container(text("Loading windows…").size(14).color(faded))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
                .into()
        } else if self.windows.is_empty() {
            container(text("No windows on other workspaces").size(14).color(faded))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
                .into()
        } else {
            window_grid(&self.windows, page_slice, config, highlighted)
        };

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

        let pagination = container(row(dots).spacing(2))
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center);

        // Status bar: full "workspace:title" for the hovered or keyboard-selected entry.
        let status_text = self
            .status_win_idx()
            .and_then(|idx| self.windows.get(idx))
            .map(|w| format!("{}:{}", w.workspace_name, w.title))
            .unwrap_or_default();

        let status_bar = container(text(status_text).size(12).color(faded).width(Length::Fill))
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center);

        container(
            column![
                search_bar(
                    &self.query,
                    &self.shake,
                    SearchIcon::Window,
                    &config.theme,
                    Msg::QueryChanged,
                ),
                grid,
                pagination,
                status_bar,
            ]
            .spacing(16)
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

// ── Private grid widget ────────────────────────────────────────────────────────

fn window_grid<'a>(
    windows: &'a [WindowEntry],
    indices: &[usize],
    config: &Config,
    highlighted: Option<usize>,
) -> Element<'a, Msg> {
    let icon_size = config.icon_size as f32;

    let mut rows: Vec<Element<'a, Msg>> = indices
        .chunks(config.columns)
        .enumerate()
        .map(|(row_idx, chunk)| {
            let mut cells: Vec<Element<'a, Msg>> = chunk
                .iter()
                .enumerate()
                .map(|(col_idx, &win_idx)| {
                    let page_position = row_idx * config.columns + col_idx;
                    let is_selected = highlighted == Some(page_position);

                    let win = &windows[win_idx];

                    let icon: Element<'a, Msg> = match &win.icon {
                        Some(IconHandle::Vector(h)) => {
                            svg(h.clone()).width(icon_size).height(icon_size).into()
                        }
                        Some(IconHandle::Raster(h)) => {
                            image(h.clone()).width(icon_size).height(icon_size).into()
                        }
                        None => svg(svg::Handle::from_memory(FALLBACK_ICON.to_vec()))
                            .width(icon_size)
                            .height(icon_size)
                            .into(),
                    };

                    // Truncated label clipped by cell width.
                    let label_str = format!("{}:{}", win.workspace_name, win.title);
                    let label = text(label_str)
                        .size(11)
                        .color(config.theme.app_label)
                        .width(Length::Fill);

                    let cell = column![icon, label].align_x(Alignment::Center).spacing(6);

                    let (label_color, selected_bg) =
                        (config.theme.app_label, config.theme.app_selected);
                    let btn = button(cell)
                        .on_press(Msg::WindowActivated(win_idx))
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
                        });

                    mouse_area(btn)
                        .on_enter(Msg::WindowHovered(Some(win_idx)))
                        .on_exit(Msg::WindowHovered(None))
                        .into()
                })
                .collect();

            while cells.len() < config.columns {
                cells.push(Space::new().width(Length::Fill).height(Length::Fill).into());
            }

            row(cells).width(Length::Fill).height(Length::Fill).into()
        })
        .collect();

    while rows.len() < config.rows {
        let cells: Vec<Element<'a, Msg>> = (0..config.columns)
            .map(|_| Space::new().width(Length::Fill).height(Length::Fill).into())
            .collect();
        rows.push(row(cells).width(Length::Fill).height(Length::Fill).into());
    }

    column(rows).width(Length::Fill).height(Length::Fill).into()
}

// ── Async hyprctl helpers ─────────────────────────────────────────────────────

async fn fetch_windows() -> Result<(Vec<WindowData>, i64), String> {
    let active_out = tokio::process::Command::new("hyprctl")
        .args(["activeworkspace", "-j"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let active: HyprActiveWorkspace =
        serde_json::from_slice(&active_out.stdout).map_err(|e| e.to_string())?;
    let active_id = active.id;

    let clients_out = tokio::process::Command::new("hyprctl")
        .args(["clients", "-j"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let clients: Vec<HyprClient> =
        serde_json::from_slice(&clients_out.stdout).map_err(|e| e.to_string())?;

    let mut clients: Vec<HyprClient> = clients
        .into_iter()
        .filter(|c| c.workspace.id != active_id && !c.class.is_empty())
        .collect();
    clients.sort_by_key(|c| (c.workspace.id, c.at[0]));

    let windows = clients
        .into_iter()
        .map(|c| WindowData {
            address: c.address,
            class: c.class,
            title: c.title,
            initial_title: c.initial_title,
            workspace_name: c.workspace.name,
        })
        .collect();

    Ok((windows, active_id))
}

async fn move_window(active_workspace_id: i64, address: String) {
    let target = format!("{},address:{}", active_workspace_id, address);
    let _ = tokio::process::Command::new("hyprctl")
        .args(["dispatch", "movetoworkspacesilent", &target])
        .output()
        .await;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn pages(total: usize, page_size: usize) -> usize {
    if page_size == 0 { 1 } else { total.div_ceil(page_size) }
}
