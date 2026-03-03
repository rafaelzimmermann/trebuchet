use iced::{
    widget::{container, row, svg, text, text_input, Id, Space},
    Alignment, Background, Border, Color, Element, Length,
};

use crate::app::{Message, ShakeState};

pub const SEARCH_ID: &str = "trebuchet_search";

const ROBOT_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
  fill="none" stroke="white" stroke-width="1.5" stroke-linecap="round">
  <rect x="5" y="7" width="14" height="10" rx="2"/>
  <line x1="12" y1="7" x2="12" y2="4"/>
  <circle cx="12" cy="3.5" r="1"/>
  <circle cx="9" cy="11" r="1.2" fill="white"/>
  <circle cx="15" cy="11" r="1.2" fill="white"/>
  <line x1="9" y1="14" x2="15" y2="14"/>
</svg>"#;

const SHAKE_OFFSETS: [f32; 6] = [-8.0, 8.0, -5.0, 5.0, -2.0, 0.0];

pub fn search_bar<'a>(query: &str, shake: &ShakeState) -> Element<'a, Message> {
    let icon: Element<'a, Message> = if query.starts_with("/ai") {
        svg(svg::Handle::from_memory(ROBOT_SVG.to_vec()))
            .width(20)
            .height(20)
            .into()
    } else {
        text("🔍").size(18).color(Color::WHITE).into()
    };

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

    let inner = row![icon, input]
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

    // Nominal left offset to centre the 620px pill within 840px available width.
    // (840 = 1000 window – 80 left pad – 80 right pad)
    let nominal: f32 = 110.0;
    let offset: f32 = if shake.active {
        SHAKE_OFFSETS[shake.tick as usize]
    } else {
        0.0
    };

    row![Space::new().width(Length::Fixed(nominal + offset)), pill]
        .width(Length::Fill)
        .into()
}
