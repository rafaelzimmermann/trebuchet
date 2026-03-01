use iced_layershell::{
    reexport::{Anchor, KeyboardInteractivity, Layer},
    settings::{LayerShellSettings, StartMode},
    Settings,
};

mod app;
mod config;
mod launcher;
mod ui;

fn main() -> iced_layershell::Result {
    let layer_settings = LayerShellSettings {
        anchor: Anchor::empty(),
        layer: Layer::Overlay,
        exclusive_zone: 0,
        size: Some((960, 640)),
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
        .settings(settings)
        .run()
}
