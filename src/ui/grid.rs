use iced::{
    alignment,
    widget::{button, column, container, image, row, svg, text, Space},
    Alignment, Color, Element, Length,
};

use crate::app::Message;
use crate::config::Config;
use crate::launcher::{AppEntry, IconHandle};

pub fn app_grid<'a>(
    apps: &'a [AppEntry],
    indices: &[usize],
    config: &Config,
) -> Element<'a, Message> {
    let icon_size = config.icon_size as f32;

    let mut rows: Vec<Element<'a, Message>> = indices
        .chunks(config.columns)
        .map(|chunk| {
            let mut cells: Vec<Element<'a, Message>> = chunk
                .iter()
                .map(|&idx| {
                    let app = &apps[idx];

                    let icon: Element<'a, Message> = match &app.icon {
                        Some(IconHandle::Vector(handle)) => svg(handle.clone())
                            .width(icon_size)
                            .height(icon_size)
                            .into(),
                        Some(IconHandle::Raster(handle)) => image(handle.clone())
                            .width(icon_size)
                            .height(icon_size)
                            .into(),
                        None => container(text("?").size(32).color(Color::WHITE))
                            .width(icon_size)
                            .height(icon_size)
                            .align_x(alignment::Horizontal::Center)
                            .align_y(alignment::Vertical::Center)
                            .into(),
                    };

                    let label = text(app.name.as_str())
                        .size(13)
                        .color(Color::WHITE);

                    let cell = column![icon, label]
                        .align_x(Alignment::Center)
                        .spacing(6);

                    button(cell)
                        .on_press(Message::AppActivated(idx))
                        .padding(12)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(|_theme, _status| button::Style {
                            text_color: Color::WHITE,
                            background: None,
                            ..Default::default()
                        })
                        .into()
                })
                .collect();

            // Pad short rows so every column lines up with the rows above.
            while cells.len() < config.columns {
                cells.push(Space::new().width(Length::Fill).height(Length::Fill).into());
            }

            row(cells)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        })
        .collect();

    // Pad missing rows so the grid height stays constant while searching.
    while rows.len() < config.rows {
        let cells: Vec<Element<'a, Message>> = (0..config.columns)
            .map(|_| Space::new().width(Length::Fill).height(Length::Fill).into())
            .collect();
        rows.push(row(cells).width(Length::Fill).height(Length::Fill).into());
    }

    column(rows)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
