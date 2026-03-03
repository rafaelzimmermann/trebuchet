use iced::{
    widget::{button, column, image, row, svg, text, Space},
    Alignment, Background, Border, Color, Element, Length,
};

/// Fallback icon shown when no icon can be resolved for an app.
/// A faint rounded square containing a 2×2 grid of tiles — evokes
/// "application" without being tied to any specific look.
const FALLBACK_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <rect x="6" y="6" width="52" height="52" rx="12"
        fill="white" fill-opacity="0.06"
        stroke="white" stroke-opacity="0.22" stroke-width="1.5"/>
  <rect x="16" y="19" width="11" height="11" rx="2.5" fill="white" fill-opacity="0.32"/>
  <rect x="31" y="19" width="11" height="11" rx="2.5" fill="white" fill-opacity="0.32"/>
  <rect x="16" y="34" width="11" height="11" rx="2.5" fill="white" fill-opacity="0.32"/>
  <rect x="31" y="34" width="11" height="11" rx="2.5" fill="white" fill-opacity="0.32"/>
</svg>"#;

use crate::config::Config;
use crate::launcher::{AppEntry, IconHandle};

pub fn app_grid<'a, Msg: Clone + 'a>(
    apps: &'a [AppEntry],
    indices: &[usize],
    config: &Config,
    highlighted: Option<usize>,
    on_activate: impl Fn(usize) -> Msg + 'a,
) -> Element<'a, Msg> {
    let icon_size = config.icon_size as f32;

    let mut rows: Vec<Element<'a, Msg>> = indices
        .chunks(config.columns)
        .enumerate()
        .map(|(row_idx, chunk)| {
            let mut cells: Vec<Element<'a, Msg>> = chunk
                .iter()
                .enumerate()
                .map(|(col_idx, &idx)| {
                    let page_position = row_idx * config.columns + col_idx;
                    let is_selected = highlighted == Some(page_position);

                    let app = &apps[idx];

                    let icon: Element<'a, Msg> = match &app.icon {
                        Some(IconHandle::Vector(handle)) => svg(handle.clone())
                            .width(icon_size)
                            .height(icon_size)
                            .into(),
                        Some(IconHandle::Raster(handle)) => image(handle.clone())
                            .width(icon_size)
                            .height(icon_size)
                            .into(),
                        None => svg(svg::Handle::from_memory(FALLBACK_ICON.to_vec()))
                            .width(icon_size)
                            .height(icon_size)
                            .into(),
                    };

                    let label = text(app.name.as_str())
                        .size(13)
                        .color(Color::WHITE);

                    let cell = column![icon, label]
                        .align_x(Alignment::Center)
                        .spacing(6);

                    button(cell)
                        .on_press(on_activate(idx))
                        .padding(12)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(move |_theme, _status| button::Style {
                            text_color: Color::WHITE,
                            background: if is_selected {
                                Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.15 }))
                            } else {
                                None
                            },
                            border: if is_selected {
                                Border { radius: 8.0.into(), ..Default::default() }
                            } else {
                                Border::default()
                            },
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
        let cells: Vec<Element<'a, Msg>> = (0..config.columns)
            .map(|_| Space::new().width(Length::Fill).height(Length::Fill).into())
            .collect();
        rows.push(row(cells).width(Length::Fill).height(Length::Fill).into());
    }

    column(rows)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
