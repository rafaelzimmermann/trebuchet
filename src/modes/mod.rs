pub mod ai;
pub mod search;

pub use ai::AiStatus;

use iced::{Element, Task};

use crate::app::Message;
use crate::config::Config;
use crate::launcher::AppEntry;

pub enum Mode {
    Search(search::SearchState),
    Ai(ai::AiState),
}

impl Mode {
    pub fn is_ai_loading(&self) -> bool {
        matches!(self, Mode::Ai(ai) if ai.is_loading())
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
