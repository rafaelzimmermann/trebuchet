use iced::{event::Status, Element, Event, Subscription, Task};

use crate::command::ComponentEvent;
use crate::config::Config;
use crate::launcher::AppEntry;

pub trait Component {
    type Msg: Clone + std::fmt::Debug + Send + 'static;

    fn handle_event(
        &mut self,
        event: &Event,
        status: Status,
        apps: &[AppEntry],
        config: &Config,
    ) -> (Task<Self::Msg>, ComponentEvent);

    fn update(&mut self, msg: Self::Msg, apps: &[AppEntry], config: &Config) -> Task<Self::Msg>;

    fn view<'a>(&'a self, apps: &'a [AppEntry], config: &'a Config) -> Element<'a, Self::Msg>;

    fn subscription(&self) -> Subscription<Self::Msg>;
}
