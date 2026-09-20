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
    WindowMoved(Result<(), String>),
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
            Err(error) => {
                eprintln!("Could not load windows: {error}");
                Msg::LoadFailed
            }
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
            Msg::WindowMoved,
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

            Msg::WindowMoved(Ok(())) => {
                return (Task::none(), ComponentEvent::Exit);
            }

            Msg::WindowMoved(Err(error)) => {
                eprintln!("Could not move window: {error}");
                self.shake = ShakeState::trigger();
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
                    "Search windows...",
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
    let active_out = hyprctl(&["activeworkspace", "-j"]).await?;
    let active: HyprActiveWorkspace =
        serde_json::from_str(&active_out).map_err(|e| e.to_string())?;
    let active_id = active.id;
    let clients_out = hyprctl(&["clients", "-j"]).await?;
    let clients: Vec<HyprClient> = serde_json::from_str(&clients_out).map_err(|e| e.to_string())?;

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

/// Bound IPC latency and check process status; dispatch replies need a separate check.
async fn hyprctl(args: &[&str]) -> Result<String, String> {
    let output = tokio::process::Command::new("hyprctl")
        .args(args)
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(Duration::from_secs(5), output)
        .await
        .map_err(|_| "hyprctl timed out".to_string())?
        .map_err(|e| format!("Could not run hyprctl: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() {
        return Err(format!(
            "hyprctl failed ({}): {} {}",
            output.status,
            stdout,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(stdout)
}

fn dispatch_result(reply: &str) -> Result<(), String> {
    if reply.trim() == "ok" {
        Ok(())
    } else {
        Err(format!("Hyprland rejected the move: {reply}"))
    }
}

fn lua_move(active_workspace_id: i64, address: &str) -> Result<String, String> {
    // Addresses come from IPC, but must still be safe inside a Lua expression.
    let hex = address.strip_prefix("0x").unwrap_or_default();
    if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid Hyprland window address".into());
    }
    Ok(format!(
        "hl.dsp.window.move({{ workspace = {active_workspace_id}, follow = false, window = \"address:{address}\" }})"
    ))
}

async fn move_window(active_workspace_id: i64, address: String) -> Result<(), String> {
    move_window_using(active_workspace_id, address, |args| async move {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        hyprctl(&args).await
    })
    .await
}

async fn move_window_using<F, Fut>(
    active_workspace_id: i64,
    address: String,
    mut run: F,
) -> Result<(), String>
where
    F: FnMut(Vec<String>) -> Fut,
    Fut: std::future::Future<Output = Result<String, String>>,
{
    let lua = lua_move(active_workspace_id, &address)?;
    let target = format!("{},address:{}", active_workspace_id, address);
    let reply = run(vec![
        "dispatch".into(),
        "movetoworkspacesilent".into(),
        target,
    ])
    .await;
    // hyprctl exits nonzero for Lua parser errors, so inspect both success and
    // error replies. Retry only this explicit rejection, never transport errors.
    let diagnostic = match &reply {
        Ok(reply) | Err(reply) => reply,
    };
    if diagnostic.contains("dispatch in lua is a shorthand for hl.dispatch(...)") {
        let reply = run(vec!["dispatch".into(), lua]).await?;
        dispatch_result(&reply)
    } else {
        dispatch_result(&reply?)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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

    #[tokio::test]
    async fn retries_lua_parser_rejection_even_when_hyprctl_exits_nonzero() {
        let diagnostic = "error: [string \"return hl.dispatch(movetoworkspacesilent 3,ad...\"]:1: ')' expected near '3'\n\n → Note: dispatch in lua is a shorthand for hl.dispatch(...), your syntax might need to be updated.";
        // Real hyprctl 0.56.2 exits 7; also tolerate versions returning exit 0.
        for rejection in [
            Err(format!("hyprctl failed (exit status: 7): {diagnostic}")),
            Ok(diagnostic.to_string()),
        ] {
            let mut calls = Vec::new();
            let result = move_window_using(3, "0xabc123".into(), |args| {
                calls.push(args);
                std::future::ready(if calls.len() == 1 {
                    rejection.clone()
                } else {
                    Ok("ok".into())
                })
            })
            .await;
            assert!(result.is_ok());
            assert_eq!(
                calls,
                vec![
                    vec![
                        "dispatch".to_string(),
                        "movetoworkspacesilent".into(),
                        "3,address:0xabc123".into()
                    ],
                    vec!["dispatch".into(), lua_move(3, "0xabc123").unwrap()],
                ]
            );
        }
    }

    #[tokio::test]
    async fn legacy_success_and_ambiguous_failures_are_never_retried() {
        for reply in [
            Ok("ok".to_string()),
            Ok("Window not found".into()),
            Err("timeout".into()),
        ] {
            let mut calls = 0;
            let result = move_window_using(3, "0xabc123".into(), |_| {
                calls += 1;
                std::future::ready(reply.clone())
            })
            .await;
            assert_eq!(calls, 1);
            assert_eq!(result.is_ok(), reply == Ok("ok".into()));
        }
    }

    #[test]
    fn dispatch_requires_explicit_success() {
        assert!(dispatch_result("ok\n").is_ok());
        for reply in ["", "Invalid dispatcher", "Window not found", "ok\nerror"] {
            assert!(dispatch_result(reply).is_err());
        }
    }

    #[test]
    fn lua_move_targets_the_requested_window_without_following() {
        assert_eq!(
            lua_move(3, "0xabc123").unwrap(),
            "hl.dsp.window.move({ workspace = 3, follow = false, window = \"address:0xabc123\" })"
        );
    }

    #[test]
    fn lua_move_rejects_invalid_or_injected_addresses() {
        for address in ["", "0x", "abc", "0xghi", "0x1\" }); os.exit() --"] {
            assert!(lua_move(3, address).is_err());
        }
    }

    #[test]
    fn failed_move_keeps_launcher_open() {
        let mut mover = WindowMover::new();
        mover.query = "firefox".into();
        let (_, event) = mover.update(
            Msg::WindowMoved(Err("Window not found".into())),
            &[],
            &Config::default(),
        );
        assert!(matches!(event, ComponentEvent::Handled));
        assert!(mover.shake.active);
        assert_eq!(mover.query, "firefox");
    }

    #[test]
    fn successful_move_closes_launcher() {
        let mut mover = WindowMover::new();
        let (_, event) = mover.update(Msg::WindowMoved(Ok(())), &[], &Config::default());
        assert!(matches!(event, ComponentEvent::Exit));
    }

    #[tokio::test]
    #[ignore = "requires a running Hyprland Lua session; targets a nonexistent window"]
    async fn live_lua_dispatch_retry() {
        // Null cannot identify a live client. Exercise the real CLI exit status
        // and fallback without changing any windows in the user's session.
        move_window(1, "0x0".into()).await.expect("Lua retry must succeed");
    }

    #[tokio::test]
    #[ignore = "requires a running Hyprland session; reads windows without moving them"]
    async fn live_hyprland_window_query() {
        fetch_windows()
            .await
            .expect("Hyprland IPC must be readable");
    }
}
