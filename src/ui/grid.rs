use iced::{
    alignment,
    widget::{button, column, container, image, row, svg, text},
    Alignment, Element, Length,
};

use crate::app::Message;
use crate::config::Config;
use crate::launcher::AppEntry;

pub fn app_grid<'a>(
    apps: &'a [AppEntry],
    indices: &[usize],
    config: &Config,
) -> Element<'a, Message> {
    let icon_size = config.icon_size as f32;

    let rows: Vec<Element<'a, Message>> = indices
        .chunks(config.columns)
        .map(|chunk| {
            let cells: Vec<Element<'a, Message>> = chunk
                .iter()
                .map(|&idx| {
                    let app = &apps[idx];

                    let icon: Element<'a, Message> = match &app.icon {
                        Some(path) => {
                            let ext = path
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("");
                            if ext == "svg" {
                                svg(svg::Handle::from_path(path))
                                    .width(icon_size)
                                    .height(icon_size)
                                    .into()
                            } else {
                                image(image::Handle::from_path(path))
                                    .width(icon_size)
                                    .height(icon_size)
                                    .into()
                            }
                        }
                        None => container(text("?").size(32))
                            .width(icon_size)
                            .height(icon_size)
                            .align_x(alignment::Horizontal::Center)
                            .align_y(alignment::Vertical::Center)
                            .into(),
                    };

                    let cell = column![icon, text(app.name.as_str()).size(12)]
                        .align_x(Alignment::Center)
                        .spacing(4)
                        .width(120);

                    button(cell)
                        .on_press(Message::AppActivated(idx))
                        .padding(8)
                        .style(button::text)
                        .into()
                })
                .collect();

            row(cells).spacing(8).into()
        })
        .collect();

    column(rows)
        .spacing(16)
        .padding(20)
        .width(Length::Fill)
        .into()
}
