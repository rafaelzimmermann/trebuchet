use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use iced::{
    alignment,
    widget::{button, column, container, row, text},
    Color, Element, Length, Task,
};

use crate::app::Message;
use crate::config::Config;
use crate::launcher::{launch_app, AppEntry};
use crate::ui::app_grid;

pub struct SearchState {
    pub filtered: Vec<usize>,
    pub page: usize,
    pub selected: Option<usize>,
}

impl SearchState {
    pub fn new(apps: &[AppEntry]) -> Self {
        Self { filtered: (0..apps.len()).collect(), page: 0, selected: None }
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

    pub fn update(&mut self, msg: Message, apps: &[AppEntry], config: &Config) -> Task<Message> {
        let page_size = config.columns * config.rows;
        let total = pages(self.filtered.len(), page_size);

        match msg {
            Message::GoToPage(p) => {
                self.page = p.min(total.saturating_sub(1));
            }
            Message::PageNext => {
                if self.page + 1 < total {
                    self.page += 1;
                }
            }
            Message::PagePrev => {
                if self.page > 0 {
                    self.page -= 1;
                }
            }
            Message::SelectNext => self.move_selection(1, config),
            Message::SelectPrev => self.move_selection(-1, config),
            Message::SelectDown => self.move_selection(config.columns as isize, config),
            Message::SelectUp => self.move_selection(-(config.columns as isize), config),
            Message::AppActivated(idx) => {
                if let Some(app) = apps.get(idx) {
                    launch_app(&app.exec.clone(), app.terminal);
                    std::process::exit(0);
                }
            }
            Message::ActivateSelected => {
                if let Some(sel) = self.selected {
                    if let Some(&app_idx) = self.filtered.get(sel) {
                        if let Some(app) = apps.get(app_idx) {
                            launch_app(&app.exec.clone(), app.terminal);
                            std::process::exit(0);
                        }
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }

    pub fn view<'a>(&'a self, apps: &'a [AppEntry], config: &'a Config) -> Element<'a, Message> {
        let page_size = config.columns * config.rows;
        let total_pages = pages(self.filtered.len(), page_size);
        let start = self.page * page_size;
        let end = (start + page_size).min(self.filtered.len());
        let page_slice = &self.filtered[start..end];

        let dots: Vec<Element<'_, Message>> = (0..total_pages)
            .map(|i| {
                let color = if i == self.page {
                    Color::WHITE
                } else {
                    Color { r: 1.0, g: 1.0, b: 1.0, a: 0.35 }
                };
                button(text("●").size(10).color(color))
                    .on_press(Message::GoToPage(i))
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

        column![app_grid(apps, page_slice, config, highlighted), pagination].into()
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
            .map(|n| AppEntry { name: n.to_string(), exec: n.to_string(), terminal: false, icon: None })
            .collect()
    }

    fn make_search(names: &[&str]) -> (Vec<AppEntry>, SearchState) {
        let apps = make_apps(names);
        let state = SearchState::new(&apps);
        (apps, state)
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
        let (apps, mut search) = make_search(&["Firefox", "Code", "Terminal"]);
        search.apply_filter(&apps, "");
        assert_eq!(search.filtered, vec![0, 1, 2]);
    }

    #[test]
    fn query_filters_by_name() {
        let (apps, mut search) = make_search(&["Firefox", "Files", "Terminal"]);
        search.apply_filter(&apps, "fire");
        assert!(search.filtered.contains(&0), "Firefox should match 'fire'");
        assert!(!search.filtered.contains(&2), "Terminal should not match 'fire'");
    }

    #[test]
    fn no_match_yields_empty() {
        let (apps, mut search) = make_search(&["Firefox", "Code", "Terminal"]);
        search.apply_filter(&apps, "zzzzzz");
        assert!(search.filtered.is_empty());
    }

    #[test]
    fn search_resets_page_to_zero() {
        let (apps, mut search) = make_search(&["A", "B", "C", "D", "E", "F", "G"]);
        search.page = 1;
        search.apply_filter(&apps, "A");
        assert_eq!(search.page, 0);
    }

    #[test]
    fn clearing_search_restores_all() {
        let (apps, mut search) = make_search(&["Firefox", "Code", "Terminal"]);
        search.apply_filter(&apps, "fire");
        assert_eq!(search.filtered.len(), 1);
        search.apply_filter(&apps, "");
        assert_eq!(search.filtered.len(), 3);
    }

    // ── pagination ────────────────────────────────────────────────────────────
    // cfg() has columns=3, rows=2 → page_size=6.

    #[test]
    fn page_next_advances() {
        // 7 items, page_size=6 → 2 pages
        let (apps, mut search) = make_search(&["A", "B", "C", "D", "E", "F", "G"]);
        let _ = search.update(Message::PageNext, &apps, &cfg());
        assert_eq!(search.page, 1);
    }

    #[test]
    fn page_next_clamps_at_last() {
        let (apps, mut search) = make_search(&["A", "B", "C", "D", "E", "F", "G"]);
        search.page = 1;
        let _ = search.update(Message::PageNext, &apps, &cfg());
        assert_eq!(search.page, 1);
    }

    #[test]
    fn page_prev_decrements() {
        let (apps, mut search) = make_search(&["A", "B", "C", "D", "E", "F", "G"]);
        search.page = 1;
        let _ = search.update(Message::PagePrev, &apps, &cfg());
        assert_eq!(search.page, 0);
    }

    #[test]
    fn page_prev_clamps_at_zero() {
        let (apps, mut search) = make_search(&["A", "B", "C"]);
        let _ = search.update(Message::PagePrev, &apps, &cfg());
        assert_eq!(search.page, 0);
    }

    #[test]
    fn go_to_page_clamps_to_last() {
        // 6 items, page_size=6 → 1 page (index 0 only)
        let (apps, mut search) = make_search(&["A", "B", "C", "D", "E", "F"]);
        let _ = search.update(Message::GoToPage(99), &apps, &cfg());
        assert_eq!(search.page, 0);
    }
}
