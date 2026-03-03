use iced_layershell::{
    reexport::{Anchor, KeyboardInteractivity, Layer},
    settings::{LayerShellSettings, StartMode},
    Settings,
};

mod ai_client;
mod app;
mod config;
mod launcher;
mod ui;

fn main() -> iced_layershell::Result {
    let layer_settings = LayerShellSettings {
        // No anchoring — the compositor centres the surface on the active screen.
        anchor: Anchor::empty(),
        layer: Layer::Overlay,
        exclusive_zone: 0,
        // Width × height tuned to fit 7 cols × 5 rows of 96px icons plus chrome.
        size: Some((1000, 860)),
        margin: (0, 0, 0, 0),
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        start_mode: StartMode::Active,
        events_transparent: false,
    };

    let settings = Settings {
        layer_settings,
        id: Some("trebuchet".into()),
        ..Default::default()
    };

    iced_layershell::application(app::boot, app::namespace, app::update, app::view)
        .subscription(app::subscription)
        // Fully transparent window — the rounded container in view() provides
        // the visible background so the compositor can clip the corners cleanly.
        .style(|_state: &app::Trebuchet, _theme: &iced::Theme| iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: iced::Color::WHITE,
        })
        .settings(settings)
        .run()
}
