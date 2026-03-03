use iced::{
    event,
    keyboard::{self, key::Named, Key},
    mouse,
    time,
    widget::{column, container},
    Background, Border, Color, Element, Event, Length, Subscription, Task,
};
use iced::event::Status;
use iced_layershell::to_layer_message;
use std::time::Duration;

use crate::config::Config;
use crate::launcher::{launch_app, scan_applications, AppEntry};
use crate::modes::{ai::AiState, search::SearchState, Mode};
use crate::ui::search_bar;

// ── Shake animation state ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShakeState {
    pub active: bool,
    pub tick: u8,
}

// ── App state ─────────────────────────────────────────────────────────────────

pub struct Trebuchet {
    pub apps: Vec<AppEntry>,
    pub query: String,
    pub config: Config,
    pub shake_state: ShakeState,
    pub mode: Mode,
}

// ── Messages ──────────────────────────────────────────────────────────────────

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
    LinkClicked(String),
}

// ── Boot ──────────────────────────────────────────────────────────────────────

pub fn boot() -> (Trebuchet, Task<Message>) {
    let apps = scan_applications();
    let mode = Mode::Search(SearchState::new(&apps));
    let state = Trebuchet {
        apps,
        query: String::new(),
        config: Config::load(),
        shake_state: ShakeState::default(),
        mode,
    };
    (state, Task::none())
}

pub fn namespace() -> String {
    "trebuchet".into()
}

// ── Mode sync ─────────────────────────────────────────────────────────────────

fn sync_mode(state: &mut Trebuchet) {
    let should_be_ai = state.query.starts_with("/ai");
    match (&state.mode, should_be_ai) {
        (Mode::Search(_), true) => {
            state.mode = Mode::Ai(AiState::new());
        }
        (Mode::Ai(_), false) => {
            let mut search = SearchState::new(&state.apps);
            search.apply_filter(&state.apps, &state.query);
            state.mode = Mode::Search(search);
        }
        _ => {}
    }
}

// ── Update ────────────────────────────────────────────────────────────────────

pub fn update(state: &mut Trebuchet, msg: Message) -> Task<Message> {
    match msg {
        Message::SearchChanged(query) => {
            state.query = query;
            sync_mode(state);
            if let Mode::Search(ref mut search) = state.mode {
                search.apply_filter(&state.apps, &state.query);
            }
        }
        Message::SearchAppend(c) => {
            state.query.push_str(&c);
            sync_mode(state);
            if let Mode::Search(ref mut search) = state.mode {
                search.apply_filter(&state.apps, &state.query);
            }
        }
        Message::SearchBackspace => {
            state.query.pop();
            sync_mode(state);
            if let Mode::Search(ref mut search) = state.mode {
                search.apply_filter(&state.apps, &state.query);
            }
        }
        Message::Close => std::process::exit(0),
        Message::KeyPressed(key) => match &key {
            Key::Named(Named::Escape) => match &state.mode {
                Mode::Ai(_) => {
                    state.query.clear();
                    state.mode = Mode::Search(SearchState::new(&state.apps));
                }
                Mode::Search(_) => std::process::exit(0),
            },
            Key::Named(Named::PageDown) => {
                if let Mode::Search(ref mut search) = state.mode {
                    let _ = search.update(Message::PageNext, &state.apps, &state.config);
                }
            }
            Key::Named(Named::PageUp) => {
                if let Mode::Search(ref mut search) = state.mode {
                    let _ = search.update(Message::PagePrev, &state.apps, &state.config);
                }
            }
            _ => {}
        },
        Message::AiSubmit => {
            let is_ai = matches!(state.mode, Mode::Ai(_));
            if is_ai {
                let prompt = state.query.trim_start_matches("/ai").trim().to_string();
                if prompt.is_empty() {
                    state.shake_state = ShakeState { active: true, tick: 0 };
                    return Task::none();
                }
                if let Mode::Ai(ref mut ai) = state.mode {
                    return ai.start_query(prompt, &state.config);
                }
            } else if let Mode::Search(ref search) = state.mode {
                let to_launch = search.selected
                    .and_then(|sel| search.filtered.get(sel).copied());
                if let Some(app_idx) = to_launch {
                    if let Some(app) = state.apps.get(app_idx) {
                        launch_app(&app.exec.clone(), app.terminal);
                        std::process::exit(0);
                    }
                }
            }
        }
        Message::ShakeTick => {
            state.shake_state.tick += 1;
            if state.shake_state.tick >= 6 {
                state.shake_state = ShakeState::default();
            }
        }
        msg => return state.mode.update(msg, &state.apps, &state.config),
    }
    Task::none()
}

// ── View ──────────────────────────────────────────────────────────────────────

pub fn view(state: &Trebuchet) -> Element<'_, Message> {
    let body = state.mode.view(&state.apps, &state.config);

    let content = column![search_bar(&state.query, &state.shake_state), body]
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
                text.as_ref().map(|t| Message::SearchAppend(t.to_string()))
            }
            _ => None,
        },
        Event::Mouse(mouse::Event::CursorLeft) => Some(Message::Close),
        Event::Mouse(mouse::Event::ButtonPressed(_)) if status == Status::Ignored => {
            Some(Message::Close)
        }
        _ => None,
    }
}

// ── Subscription ──────────────────────────────────────────────────────────────

pub fn subscription(state: &Trebuchet) -> Subscription<Message> {
    let events = event::listen_with(on_event);

    let loading = if state.mode.is_ai_loading() {
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
