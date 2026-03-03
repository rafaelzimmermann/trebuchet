pub mod ai;
pub mod search;

pub use ai::AiStatus;

use iced::{Element, Subscription, Task};

use crate::app::Message;
use crate::config::Config;
use crate::launcher::AppEntry;

pub enum Mode {
    Search(search::SearchState),
    Ai(ai::AiState),
}

impl Mode {
    pub fn subscription(&self) -> Subscription<Message> {
        match self {
            Mode::Search(_) => Subscription::none(),
            Mode::Ai(ai) => ai.subscription(),
        }
    }

    pub fn update(&mut self, msg: Message, apps: &[AppEntry], config: &Config) -> Task<Message> {
        match self {
            Mode::Search(search) => search.update(msg, apps, config),
            Mode::Ai(ai) => ai.update(msg, config),
        }
    }

    pub fn view<'a>(&'a self, apps: &'a [AppEntry], config: &'a Config) -> Element<'a, Message> {
        match self {
            Mode::Search(search) => search.view(apps, config),
            Mode::Ai(ai) => ai.view(),
        }
    }
}
