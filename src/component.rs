use iced::{Element, Subscription, Task};

use crate::command::ComponentEvent;
use crate::config::Config;
use crate::launcher::AppEntry;

#[derive(Debug, Clone, PartialEq)]
pub enum NavDirection {
    Left,
    Right,
    Up,
    Down,
}

pub trait Component {
    type Msg: Clone + std::fmt::Debug + Send + 'static;

    fn handle_char(
        &mut self,
        c: String,
        apps: &[AppEntry],
        config: &Config,
    ) -> (Task<Self::Msg>, ComponentEvent);

    fn handle_backspace(
        &mut self,
        apps: &[AppEntry],
        config: &Config,
    ) -> (Task<Self::Msg>, ComponentEvent);

    fn handle_submit(
        &mut self,
        apps: &[AppEntry],
        config: &Config,
    ) -> (Task<Self::Msg>, ComponentEvent);

    fn handle_escape(&mut self) -> ComponentEvent;

    fn handle_nav(&mut self, dir: NavDirection, config: &Config) -> ComponentEvent;

    /// +1 = next page, -1 = prev page
    fn handle_page(&mut self, delta: i32, config: &Config) -> ComponentEvent;

    fn handle_go_to_page(&mut self, p: usize, config: &Config) -> ComponentEvent;

    fn update(&mut self, msg: Self::Msg, apps: &[AppEntry], config: &Config) -> Task<Self::Msg>;

    fn view<'a>(&'a self, apps: &'a [AppEntry], config: &'a Config) -> Element<'a, Self::Msg>;

    fn subscription(&self) -> Subscription<Self::Msg>;
}
