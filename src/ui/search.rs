use iced::{
    alignment,
    widget::{container, row, text, text_input, Id},
    Alignment, Background, Border, Color, Element, Length,
};

use crate::app::Message;

pub const SEARCH_ID: &str = "trebuchet_search";

pub fn search_bar(query: &str) -> Element<'_, Message> {
    let input = text_input("Search apps...", query)
        .id(Id::new(SEARCH_ID))
        .on_input(Message::SearchChanged)
        .padding(0)
        .size(20)
        .width(Length::Fill)
        .style(|_theme, _status| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: Color::WHITE,
            placeholder: Color { r: 0.6, g: 0.6, b: 0.7, a: 1.0 },
            value: Color::WHITE,
            selection: Color { r: 0.4, g: 0.5, b: 0.9, a: 0.45 },
        });

    let inner = row![text("🔍").size(18).color(Color::WHITE), input]
        .spacing(12)
        .align_y(Alignment::Center);

    let pill = container(inner)
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.12,
            })),
            border: Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.22 },
            },
            ..Default::default()
        })
        .padding([12, 20])
        .width(620);

    container(pill)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .into()
}
