use iced::{
    alignment,
    widget::{container, text_input},
    Element, Length,
};

use crate::app::Message;

pub fn search_bar(query: &str) -> Element<'_, Message> {
    container(
        text_input("Search apps...", query)
            .on_input(Message::SearchChanged)
            .padding(12)
            .size(20)
            .width(400),
    )
    .width(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .into()
}
