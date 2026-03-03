use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use iced::{
    alignment,
    time,
    widget::{button, column, container, row, text},
    Color, Element, Length, Subscription, Task,
};
use std::time::Duration;

use crate::command::{ComponentEvent, SlashCommand};
use crate::component::{Component, NavDirection};
use crate::config::Config;
use crate::launcher::{launch_app, AppEntry};
use crate::ui::{app_grid, search_bar, SearchIcon, ShakeState};

pub struct AppLauncher {
    pub query: String,
    pub filtered: Vec<usize>,
    pub page: usize,
    pub selected: Option<usize>,
    pub shake: ShakeState,
}

#[derive(Debug, Clone)]
pub enum Msg {
    QueryChanged(String),
    AppActivated(usize),
    GoToPage(usize),
    ShakeTick,
}

impl AppLauncher {
    pub fn new(apps: &[AppEntry]) -> Self {
        Self {
            query: String::new(),
            filtered: (0..apps.len()).collect(),
            page: 0,
            selected: None,
            shake: ShakeState::default(),
        }
    }

    /// Reset to empty query showing all apps.
    pub fn reset(&mut self, apps: &[AppEntry]) {
        self.query = String::new();
        self.page = 0;
        self.selected = None;
        self.shake = ShakeState::default();
        self.apply_filter(apps, "");
    }

    pub fn apply_filter(&mut self, apps: &[AppEntry], query: &str) {
        if query.is_empty() {
            self.filtered = (0..apps.len()).collect();
        } else {
            let matcher = SkimMatcherV2::default();
            let mut scored: Vec<(usize, i64)> = apps
                .iter()
                .enumerate()
                .filter_map(|(i, app)| matcher.fuzzy_match(&app.name, query).map(|s| (i, s)))
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
}

impl Component for AppLauncher {
    type Msg = Msg;

    fn handle_char(
        &mut self,
        c: String,
        apps: &[AppEntry],
        _config: &Config,
    ) -> (Task<Msg>, ComponentEvent) {
        self.query.push_str(&c);

        if let Some((cmd, args)) = SlashCommand::detect(&self.query) {
            match cmd {
                SlashCommand::Ai => {
                    return (Task::none(), ComponentEvent::CommandInvoked(SlashCommand::Ai, args));
                }
                SlashCommand::App => {
                    return (Task::none(), ComponentEvent::CommandInvoked(SlashCommand::App, args));
                }
                SlashCommand::Unknown(_) => {
                    self.shake = ShakeState::trigger();
                    self.query.clear();
                    self.apply_filter(apps, "");
                    return (Task::none(), ComponentEvent::Handled);
                }
            }
        }

        let q = self.query.clone();
        self.apply_filter(apps, &q);
        (Task::none(), ComponentEvent::Handled)
    }

    fn handle_backspace(
        &mut self,
        apps: &[AppEntry],
        _config: &Config,
    ) -> (Task<Msg>, ComponentEvent) {
        self.query.pop();
        let q = self.query.clone();
        self.apply_filter(apps, &q);
        (Task::none(), ComponentEvent::Handled)
    }

    fn handle_submit(
        &mut self,
        apps: &[AppEntry],
        _config: &Config,
    ) -> (Task<Msg>, ComponentEvent) {
        if let Some(sel) = self.selected {
            if let Some(&app_idx) = self.filtered.get(sel) {
                if let Some(app) = apps.get(app_idx) {
                    launch_app(&app.exec.clone(), app.terminal);
                    std::process::exit(0);
                }
            }
        }
        self.shake = ShakeState::trigger();
        (Task::none(), ComponentEvent::Handled)
    }

    fn handle_escape(&mut self) -> ComponentEvent {
        ComponentEvent::Exit
    }

    fn handle_nav(&mut self, dir: NavDirection, config: &Config) -> ComponentEvent {
        match dir {
            NavDirection::Right => self.move_selection(1, config),
            NavDirection::Left => self.move_selection(-1, config),
            NavDirection::Down => self.move_selection(config.columns as isize, config),
            NavDirection::Up => self.move_selection(-(config.columns as isize), config),
        }
        ComponentEvent::Handled
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

    fn handle_go_to_page(&mut self, p: usize, config: &Config) -> ComponentEvent {
        let page_size = config.columns * config.rows;
        let total = pages(self.filtered.len(), page_size);
        self.page = p.min(total.saturating_sub(1));
        ComponentEvent::Handled
    }

    fn update(&mut self, msg: Msg, apps: &[AppEntry], config: &Config) -> Task<Msg> {
        match msg {
            Msg::QueryChanged(s) => {
                self.apply_filter(apps, &s);
                self.query = s;
            }
            Msg::AppActivated(idx) => {
                if let Some(app) = apps.get(idx) {
                    launch_app(&app.exec.clone(), app.terminal);
                    std::process::exit(0);
                }
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
        Task::none()
    }

    fn view<'a>(&'a self, apps: &'a [AppEntry], config: &'a Config) -> Element<'a, Msg> {
        let page_size = config.columns * config.rows;
        let total_pages = pages(self.filtered.len(), page_size);
        let start = self.page * page_size;
        let end = (start + page_size).min(self.filtered.len());
        let page_slice = &self.filtered[start..end];

        let dots: Vec<Element<'_, Msg>> = (0..total_pages)
            .map(|i| {
                let color = if i == self.page {
                    Color::WHITE
                } else {
                    Color { r: 1.0, g: 1.0, b: 1.0, a: 0.35 }
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

        let highlighted = self.selected.and_then(|s| {
            if s >= start && s < end { Some(s - start) } else { None }
        });

        let content = column![
            search_bar(&self.query, &self.shake, SearchIcon::Search, Msg::QueryChanged),
            app_grid(apps, page_slice, config, highlighted, Msg::AppActivated),
            pagination,
        ]
        .spacing(16)
        .padding(iced::Padding { top: 24.0, bottom: 24.0, left: 80.0, right: 80.0 })
        .width(Length::Fill)
        .height(Length::Fill);

        content.into()
    }

    fn subscription(&self) -> Subscription<Msg> {
        if self.shake.active {
            time::every(Duration::from_millis(67)).map(|_| Msg::ShakeTick)
        } else {
            Subscription::none()
        }
    }
}

fn pages(total: usize, page_size: usize) -> usize {
    if page_size == 0 { 1 } else { total.div_ceil(page_size) }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::launcher::AppEntry;

    fn make_apps(names: &[&str]) -> Vec<AppEntry> {
        names
            .iter()
            .map(|n| AppEntry {
                name: n.to_string(),
                exec: n.to_string(),
                terminal: false,
                icon: None,
            })
            .collect()
    }

    fn make_launcher(names: &[&str]) -> (Vec<AppEntry>, AppLauncher) {
        let apps = make_apps(names);
        let launcher = AppLauncher::new(&apps);
        (apps, launcher)
    }

    fn cfg() -> Config {
        Config { columns: 3, rows: 2, icon_size: 64, ..Config::default() }
    }

    // ── pages() ──────────────────────────────────────────────────────────────

    #[test]
    fn pages_zero_items() {
        assert_eq!(pages(0, 35), 0);
    }

    #[test]
    fn pages_exact_fit() {
        assert_eq!(pages(35, 35), 1);
    }

    #[test]
    fn pages_one_over() {
        assert_eq!(pages(36, 35), 2);
    }

    #[test]
    fn pages_zero_page_size_returns_one() {
        assert_eq!(pages(100, 0), 1);
    }

    #[test]
    fn pages_single_item() {
        assert_eq!(pages(1, 35), 1);
    }

    // ── search filter ─────────────────────────────────────────────────────────

    #[test]
    fn empty_query_shows_all() {
        let (_apps, launcher) = make_launcher(&["Firefox", "Code", "Terminal"]);
        // New launcher shows all by default
        assert_eq!(launcher.filtered, vec![0, 1, 2]);
    }

    #[test]
    fn query_filters_by_name() {
        let (apps, mut launcher) = make_launcher(&["Firefox", "Files", "Terminal"]);
        launcher.apply_filter(&apps, "fire");
        assert!(launcher.filtered.contains(&0), "Firefox should match 'fire'");
        assert!(!launcher.filtered.contains(&2), "Terminal should not match 'fire'");
    }

    #[test]
    fn no_match_yields_empty() {
        let (apps, mut launcher) = make_launcher(&["Firefox", "Code", "Terminal"]);
        launcher.apply_filter(&apps, "zzzzzz");
        assert!(launcher.filtered.is_empty());
    }

    #[test]
    fn search_resets_page_to_zero() {
        let (apps, mut launcher) = make_launcher(&["A", "B", "C", "D", "E", "F", "G"]);
        launcher.page = 1;
        launcher.apply_filter(&apps, "A");
        assert_eq!(launcher.page, 0);
    }

    #[test]
    fn clearing_search_restores_all() {
        let (apps, mut launcher) = make_launcher(&["Firefox", "Code", "Terminal"]);
        launcher.apply_filter(&apps, "fire");
        assert_eq!(launcher.filtered.len(), 1);
        launcher.apply_filter(&apps, "");
        assert_eq!(launcher.filtered.len(), 3);
    }

    // ── pagination ────────────────────────────────────────────────────────────
    // cfg() has columns=3, rows=2 → page_size=6.

    #[test]
    fn page_next_advances() {
        // 7 items, page_size=6 → 2 pages
        let (_apps, mut launcher) = make_launcher(&["A", "B", "C", "D", "E", "F", "G"]);
        launcher.handle_page(1, &cfg());
        assert_eq!(launcher.page, 1);
    }

    #[test]
    fn page_next_clamps_at_last() {
        let (_apps, mut launcher) = make_launcher(&["A", "B", "C", "D", "E", "F", "G"]);
        launcher.page = 1;
        launcher.handle_page(1, &cfg());
        assert_eq!(launcher.page, 1);
    }

    #[test]
    fn page_prev_decrements() {
        let (_apps, mut launcher) = make_launcher(&["A", "B", "C", "D", "E", "F", "G"]);
        launcher.page = 1;
        launcher.handle_page(-1, &cfg());
        assert_eq!(launcher.page, 0);
    }

    #[test]
    fn page_prev_clamps_at_zero() {
        let (_apps, mut launcher) = make_launcher(&["A", "B", "C"]);
        launcher.handle_page(-1, &cfg());
        assert_eq!(launcher.page, 0);
    }

    #[test]
    fn go_to_page_clamps_to_last() {
        // 6 items, page_size=6 → 1 page (index 0 only)
        let (_apps, mut launcher) = make_launcher(&["A", "B", "C", "D", "E", "F"]);
        launcher.handle_go_to_page(99, &cfg());
        assert_eq!(launcher.page, 0);
    }
}
