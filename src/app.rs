use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use iced::{
    event,
    keyboard::{self, key::Named, Key, Modifiers},
    widget::{column, container},
    Element, Event, Length, Subscription, Task,
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
}

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    AppActivated(usize),
    KeyPressed(Key, Modifiers),
}

pub fn boot() -> (Trebuchet, Task<Message>) {
    let apps = scan_applications();
    let filtered = (0..apps.len()).collect();
    let state = Trebuchet {
        apps,
        filtered,
        query: String::new(),
        config: Config::default(),
    };
    (state, Task::none())
}

pub fn namespace() -> String {
    "trebuchet".into()
}

pub fn update(state: &mut Trebuchet, msg: Message) -> Task<Message> {
    match msg {
        Message::SearchChanged(query) => {
            state.query = query.clone();
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
        Message::KeyPressed(key, _) => {
            if key == Key::Named(Named::Escape) {
                std::process::exit(0);
            }
        }
        _ => {}
    }
    Task::none()
}

pub fn view(state: &Trebuchet) -> Element<'_, Message> {
    let content = column![
        search_bar(&state.query),
        app_grid(&state.apps, &state.filtered, &state.config),
    ]
    .spacing(20)
    .padding(40);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn on_event(event: Event, _status: Status, _id: iced::window::Id) -> Option<Message> {
    match event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::Escape),
            modifiers,
            ..
        }) => Some(Message::KeyPressed(Key::Named(Named::Escape), modifiers)),
        _ => None,
    }
}

pub fn subscription(_state: &Trebuchet) -> Subscription<Message> {
    event::listen_with(on_event)
}
