use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use iced::{
    alignment,
    event,
    keyboard::{self, key::Named, Key},
    mouse,
    time,
    widget::{button, column, container, row, text},
    Background, Border, Color, Element, Event, Length, Subscription, Task,
};
use iced::event::Status;
use iced_layershell::to_layer_message;
use std::time::Duration;

use crate::ai_client::{self, AiRequest};
use crate::config::Config;
use crate::launcher::{launch_app, scan_applications, AppEntry};
use crate::ui::{ai_panel, app_grid, search_bar};

// ── New mode/state types ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Default,
    Ai,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiStatus {
    Idle,
    Loading { tick: u8 },
    Done(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShakeState {
    pub active: bool,
    pub tick: u8,
}

// ── App state ───────────────────────────────────────────────────────────────

pub struct Trebuchet {
    pub apps: Vec<AppEntry>,
    pub filtered: Vec<usize>,
    pub query: String,
    pub config: Config,
    pub page: usize,
    pub selected: Option<usize>,
    pub mode: AppMode,
    pub ai_status: AiStatus,
    pub ai_prompt: String,
    pub shake_state: ShakeState,
    pub copy_feedback: bool,
}

// ── Messages ─────────────────────────────────────────────────────────────────

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    SearchAppend(String),
    SearchBackspace,
    AppActivated(usize),
    KeyPressed(Key),
    GoToPage(usize),
    PageNext,
    PagePrev,
    Close,
    SelectNext,
    SelectPrev,
    SelectDown,
    SelectUp,
    ActivateSelected,
    AiSubmit,
    AiResponse(Result<String, String>),
    AiRetry,
    AiCopyResponse,
    AiLoadingTick,
    ShakeTick,
    AiCopied,
}

// ── Boot ─────────────────────────────────────────────────────────────────────

pub fn boot() -> (Trebuchet, Task<Message>) {
    let apps = scan_applications();
    let filtered = (0..apps.len()).collect();
    let state = Trebuchet {
        apps,
        filtered,
        query: String::new(),
        config: Config::load(),
        page: 0,
        selected: None,
        mode: AppMode::Default,
        ai_status: AiStatus::Idle,
        ai_prompt: String::new(),
        shake_state: ShakeState::default(),
        copy_feedback: false,
    };
    (state, Task::none())
}

pub fn namespace() -> String {
    "trebuchet".into()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn apply_filter(state: &mut Trebuchet) {
    if state.query.is_empty() {
        state.filtered = (0..state.apps.len()).collect();
    } else {
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(usize, i64)> = state
            .apps
            .iter()
            .enumerate()
            .filter_map(|(i, app)| matcher.fuzzy_match(&app.name, &state.query).map(|s| (i, s)))
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        state.filtered = scored.into_iter().map(|(i, _)| i).collect();
    }
    if !state.query.is_empty() && !state.filtered.is_empty() {
        state.selected = Some(0);
    } else {
        state.selected = None;
    }
}

fn move_selection(state: &mut Trebuchet, delta: isize) {
    let page_size = state.config.columns * state.config.rows;
    if state.filtered.is_empty() {
        return;
    }
    let current = state.selected.unwrap_or(state.page * page_size);
    let next = (current as isize + delta)
        .clamp(0, state.filtered.len() as isize - 1) as usize;
    state.selected = Some(next);
    state.page = next / page_size;
}

/// Sync mode based on current query; clears AI state when leaving AI mode.
fn sync_mode(state: &mut Trebuchet) {
    let new_mode = if state.query.starts_with("/ai") { AppMode::Ai } else { AppMode::Default };
    if new_mode != state.mode {
        state.mode = new_mode;
        if state.mode == AppMode::Default {
            state.ai_status = AiStatus::Idle;
            state.ai_prompt.clear();
        }
    }
}

fn make_ai_request(state: &Trebuchet, prompt: String) -> AiRequest {
    AiRequest {
        prompt,
        provider: state.config.ai_provider.clone(),
        api_key: state.config.ai_api_key.clone(),
        model: state.config.ai_model.clone(),
        base_url: state.config.ai_base_url.clone(),
    }
}

// ── Update ───────────────────────────────────────────────────────────────────

pub fn update(state: &mut Trebuchet, msg: Message) -> Task<Message> {
    let page_size = state.config.columns * state.config.rows;

    match msg {
        Message::SearchChanged(query) => {
            state.query = query;
            state.page = 0;
            apply_filter(state);
            sync_mode(state);
        }
        Message::SearchAppend(c) => {
            state.query.push_str(&c);
            state.page = 0;
            apply_filter(state);
            sync_mode(state);
        }
        Message::SearchBackspace => {
            state.query.pop();
            state.page = 0;
            apply_filter(state);
            sync_mode(state);
        }
        Message::AppActivated(idx) => {
            if let Some(app) = state.apps.get(idx) {
                launch_app(&app.exec.clone(), app.terminal);
                std::process::exit(0);
            }
        }
        Message::KeyPressed(key) => match key {
            Key::Named(Named::Escape) => {
                if state.mode == AppMode::Ai {
                    state.query.clear();
                    state.mode = AppMode::Default;
                    state.ai_status = AiStatus::Idle;
                    state.ai_prompt.clear();
                    state.page = 0;
                    apply_filter(state);
                } else {
                    std::process::exit(0);
                }
            }
            Key::Named(Named::PageDown) => {
                let total = pages(state.filtered.len(), page_size);
                if state.page + 1 < total {
                    state.page += 1;
                }
            }
            Key::Named(Named::PageUp) => {
                if state.page > 0 {
                    state.page -= 1;
                }
            }
            _ => {}
        },
        Message::GoToPage(p) => {
            let total = pages(state.filtered.len(), page_size);
            state.page = p.min(total.saturating_sub(1));
        }
        Message::PageNext => {
            let total = pages(state.filtered.len(), page_size);
            if state.page + 1 < total {
                state.page += 1;
            }
        }
        Message::PagePrev => {
            if state.page > 0 {
                state.page -= 1;
            }
        }
        Message::Close => std::process::exit(0),
        Message::SelectNext => move_selection(state, 1),
        Message::SelectPrev => move_selection(state, -1),
        Message::SelectDown => move_selection(state, state.config.columns as isize),
        Message::SelectUp => move_selection(state, -(state.config.columns as isize)),
        Message::ActivateSelected => {
            if let Some(sel) = state.selected {
                if let Some(&app_idx) = state.filtered.get(sel) {
                    if let Some(app) = state.apps.get(app_idx) {
                        launch_app(&app.exec.clone(), app.terminal);
                        std::process::exit(0);
                    }
                }
            }
        }
        Message::AiSubmit => {
            if state.mode == AppMode::Default {
                if let Some(sel) = state.selected {
                    if let Some(&app_idx) = state.filtered.get(sel) {
                        if let Some(app) = state.apps.get(app_idx) {
                            launch_app(&app.exec.clone(), app.terminal);
                            std::process::exit(0);
                        }
                    }
                }
            } else {
                let prompt = state.query.trim_start_matches("/ai").trim().to_string();
                if prompt.is_empty() {
                    state.shake_state = ShakeState { active: true, tick: 0 };
                    return Task::none();
                }
                state.ai_prompt = prompt.clone();
                state.ai_status = AiStatus::Loading { tick: 0 };
                let req = make_ai_request(state, prompt);
                return Task::perform(ai_client::query(req), Message::AiResponse);
            }
        }
        Message::AiResponse(result) => {
            state.ai_status = match result {
                Ok(text) => AiStatus::Done(text),
                Err(err) => AiStatus::Error(err),
            };
        }
        Message::AiRetry => {
            let prompt = state.ai_prompt.clone();
            if prompt.is_empty() {
                return Task::none();
            }
            state.ai_status = AiStatus::Loading { tick: 0 };
            let req = make_ai_request(state, prompt);
            return Task::perform(ai_client::query(req), Message::AiResponse);
        }
        Message::AiCopyResponse => {
            if let AiStatus::Done(text) = &state.ai_status {
                let _ = std::process::Command::new("wl-copy").arg(text).spawn();
                state.copy_feedback = true;
                return Task::perform(
                    async { tokio::time::sleep(Duration::from_secs(2)).await },
                    |_| Message::AiCopied,
                );
            }
        }
        Message::AiCopied => {
            state.copy_feedback = false;
        }
        Message::AiLoadingTick => {
            if let AiStatus::Loading { tick } = &mut state.ai_status {
                *tick = (*tick + 1) % 3;
            }
        }
        Message::ShakeTick => {
            state.shake_state.tick += 1;
            if state.shake_state.tick >= 6 {
                state.shake_state = ShakeState::default();
            }
        }
        _ => {}
    }
    Task::none()
}

// ── View ─────────────────────────────────────────────────────────────────────

pub fn view(state: &Trebuchet) -> Element<'_, Message> {
    let page_size = state.config.columns * state.config.rows;
    let total_pages = pages(state.filtered.len(), page_size);
    let start = state.page * page_size;
    let end = (start + page_size).min(state.filtered.len());
    let page_slice = &state.filtered[start..end];

    let dots: Vec<Element<'_, Message>> = (0..total_pages)
        .map(|i| {
            let color = if i == state.page {
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

    let highlighted = state.selected.and_then(|s| {
        if s >= start && s < end { Some(s - start) } else { None }
    });

    let body: Element<'_, Message> = match state.mode {
        AppMode::Default => column![
            app_grid(&state.apps, page_slice, &state.config, highlighted),
            pagination,
        ]
        .into(),
        AppMode::Ai => ai_panel(&state.ai_status, &state.ai_prompt, state.copy_feedback),
    };

    let content = column![
        search_bar(&state.query, &state.shake_state),
        body,
    ]
    .spacing(16)
    .padding(iced::Padding { top: 24.0, bottom: 24.0, left: 80.0, right: 80.0 })
    .width(Length::Fill)
    .height(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color {
                r: 0.08,
                g: 0.08,
                b: 0.12,
                a: 0.92,
            })),
            border: Border {
                radius: 16.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

// ── Pages helper ──────────────────────────────────────────────────────────────

pub(crate) fn pages(total: usize, page_size: usize) -> usize {
    if page_size == 0 { 1 } else { total.div_ceil(page_size) }
}

// ── Event handler ─────────────────────────────────────────────────────────────

fn on_event(event: Event, status: Status, _id: iced::window::Id) -> Option<Message> {
    match event {
        Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, text, .. }) => match &key {
            Key::Named(Named::Escape)
            | Key::Named(Named::PageDown)
            | Key::Named(Named::PageUp) => Some(Message::KeyPressed(key)),
            Key::Named(Named::Enter) => Some(Message::AiSubmit),
            Key::Named(Named::ArrowRight) if status == Status::Ignored => {
                Some(Message::SelectNext)
            }
            Key::Named(Named::ArrowLeft) if status == Status::Ignored => {
                Some(Message::SelectPrev)
            }
            Key::Named(Named::ArrowDown) if status == Status::Ignored => {
                Some(Message::SelectDown)
            }
            Key::Named(Named::ArrowUp) if status == Status::Ignored => {
                Some(Message::SelectUp)
            }
            // Route backspace and printable characters to the search query when
            // no widget (e.g. a focused text_input) already consumed the event.
            Key::Named(Named::Backspace) if status == Status::Ignored => {
                Some(Message::SearchBackspace)
            }
            Key::Named(Named::Space) if status == Status::Ignored => {
                Some(Message::SearchAppend(" ".to_string()))
            }
            Key::Character(_)
                if status == Status::Ignored
                    && !modifiers.control()
                    && !modifiers.alt()
                    && !modifiers.logo() =>
            {
                // Use `text` (not `key`) so Shift, dead keys and compose are respected.
                text.as_ref().map(|t| Message::SearchAppend(t.to_string()))
            }
            _ => None,
        },
        // Cursor left our surface → user moved to another monitor.
        Event::Mouse(mouse::Event::CursorLeft) => Some(Message::Close),
        // Click that landed on background (not consumed by any widget).
        Event::Mouse(mouse::Event::ButtonPressed(_)) if status == Status::Ignored => {
            Some(Message::Close)
        }
        _ => None,
    }
}

// ── Subscription ──────────────────────────────────────────────────────────────

pub fn subscription(state: &Trebuchet) -> Subscription<Message> {
    let events = event::listen_with(on_event);

    let loading = if matches!(state.ai_status, AiStatus::Loading { .. }) {
        time::every(Duration::from_millis(400)).map(|_| Message::AiLoadingTick)
    } else {
        Subscription::none()
    };

    let shake = if state.shake_state.active {
        time::every(Duration::from_millis(67)).map(|_| Message::ShakeTick)
    } else {
        Subscription::none()
    };

    Subscription::batch([events, loading, shake])
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::launcher::AppEntry;

    fn make_state(names: &[&str]) -> Trebuchet {
        let apps = names
            .iter()
            .map(|n| AppEntry { name: n.to_string(), exec: n.to_string(), terminal: false, icon: None })
            .collect::<Vec<_>>();
        let filtered = (0..apps.len()).collect();
        Trebuchet {
            apps,
            filtered,
            query: String::new(),
            config: Config { columns: 3, rows: 2, icon_size: 64, ..Config::default() },
            page: 0,
            selected: None,
            mode: AppMode::Default,
            ai_status: AiStatus::Idle,
            ai_prompt: String::new(),
            shake_state: ShakeState::default(),
            copy_feedback: false,
        }
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
        let mut state = make_state(&["Firefox", "Code", "Terminal"]);
        let _ = update(&mut state, Message::SearchChanged(String::new()));
        assert_eq!(state.filtered, vec![0, 1, 2]);
    }

    #[test]
    fn query_filters_by_name() {
        let mut state = make_state(&["Firefox", "Files", "Terminal"]);
        let _ = update(&mut state, Message::SearchChanged("fire".into()));
        assert!(state.filtered.contains(&0), "Firefox should match 'fire'");
        assert!(!state.filtered.contains(&2), "Terminal should not match 'fire'");
    }

    #[test]
    fn no_match_yields_empty() {
        let mut state = make_state(&["Firefox", "Code", "Terminal"]);
        let _ = update(&mut state, Message::SearchChanged("zzzzzz".into()));
        assert!(state.filtered.is_empty());
    }

    #[test]
    fn search_resets_page_to_zero() {
        let mut state = make_state(&["A", "B", "C", "D", "E", "F", "G"]);
        state.page = 1;
        let _ = update(&mut state, Message::SearchChanged("A".into()));
        assert_eq!(state.page, 0);
    }

    #[test]
    fn clearing_search_restores_all() {
        let mut state = make_state(&["Firefox", "Code", "Terminal"]);
        let _ = update(&mut state, Message::SearchChanged("fire".into()));
        assert_eq!(state.filtered.len(), 1);
        let _ = update(&mut state, Message::SearchChanged(String::new()));
        assert_eq!(state.filtered.len(), 3);
    }

    // ── pagination ────────────────────────────────────────────────────────────
    // Config has columns=3, rows=2 → page_size=6.

    #[test]
    fn page_next_advances() {
        // 7 items, page_size=6 → 2 pages
        let mut state = make_state(&["A", "B", "C", "D", "E", "F", "G"]);
        let _ = update(&mut state, Message::PageNext);
        assert_eq!(state.page, 1);
    }

    #[test]
    fn page_next_clamps_at_last() {
        let mut state = make_state(&["A", "B", "C", "D", "E", "F", "G"]);
        state.page = 1;
        let _ = update(&mut state, Message::PageNext); // already on last page
        assert_eq!(state.page, 1);
    }

    #[test]
    fn page_prev_decrements() {
        let mut state = make_state(&["A", "B", "C", "D", "E", "F", "G"]);
        state.page = 1;
        let _ = update(&mut state, Message::PagePrev);
        assert_eq!(state.page, 0);
    }

    #[test]
    fn page_prev_clamps_at_zero() {
        let mut state = make_state(&["A", "B", "C"]);
        let _ = update(&mut state, Message::PagePrev);
        assert_eq!(state.page, 0);
    }

    #[test]
    fn go_to_page_clamps_to_last() {
        // 6 items, page_size=6 → 1 page (index 0 only)
        let mut state = make_state(&["A", "B", "C", "D", "E", "F"]);
        let _ = update(&mut state, Message::GoToPage(99));
        assert_eq!(state.page, 0);
    }
}
