use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use iced::{
    alignment,
    event,
    keyboard::{self, key::Named, Key},
    mouse,
    widget::{button, column, container, row, text},
    Background, Border, Color, Element, Event, Length, Subscription, Task,
};
use iced::event::Status;
use iced_layershell::to_layer_message;

use crate::config::Config;
use crate::launcher::{launch_app, scan_applications, AppEntry};
use crate::ui::{app_grid, search_bar};

pub struct Trebuchet {
    pub apps: Vec<AppEntry>,
    pub filtered: Vec<usize>,
    pub query: String,
    pub config: Config,
    pub page: usize,
}

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    AppActivated(usize),
    KeyPressed(Key),
    GoToPage(usize),
    PageNext,
    PagePrev,
    Close,
}

pub fn boot() -> (Trebuchet, Task<Message>) {
    let apps = scan_applications();
    let filtered = (0..apps.len()).collect();
    let state = Trebuchet {
        apps,
        filtered,
        query: String::new(),
        config: Config::default(),
        page: 0,
    };
    (state, Task::none())
}

pub fn namespace() -> String {
    "trebuchet".into()
}

pub fn update(state: &mut Trebuchet, msg: Message) -> Task<Message> {
    let page_size = state.config.columns * state.config.rows;

    match msg {
        Message::SearchChanged(query) => {
            state.query = query.clone();
            state.page = 0;
            if query.is_empty() {
                state.filtered = (0..state.apps.len()).collect();
            } else {
                let matcher = SkimMatcherV2::default();
                let mut scored: Vec<(usize, i64)> = state
                    .apps
                    .iter()
                    .enumerate()
                    .filter_map(|(i, app)| {
                        matcher.fuzzy_match(&app.name, &query).map(|s| (i, s))
                    })
                    .collect();
                scored.sort_by(|a, b| b.1.cmp(&a.1));
                state.filtered = scored.into_iter().map(|(i, _)| i).collect();
            }
        }
        Message::AppActivated(idx) => {
            if let Some(app) = state.apps.get(idx) {
                launch_app(&app.exec.clone());
                std::process::exit(0);
            }
        }
        Message::KeyPressed(key) => match key {
            Key::Named(Named::Escape) => std::process::exit(0),
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
        _ => {}
    }
    Task::none()
}

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

    let content = column![
        search_bar(&state.query),
        app_grid(&state.apps, page_slice, &state.config),
        pagination,
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

fn pages(total: usize, page_size: usize) -> usize {
    if page_size == 0 { 1 } else { total.div_ceil(page_size) }
}

fn on_event(event: Event, status: Status, _id: iced::window::Id) -> Option<Message> {
    match event {
        Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => match &key {
            Key::Named(Named::Escape)
            | Key::Named(Named::PageDown)
            | Key::Named(Named::PageUp) => Some(Message::KeyPressed(key)),
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

pub fn subscription(_state: &Trebuchet) -> Subscription<Message> {
    event::listen_with(on_event)
}
