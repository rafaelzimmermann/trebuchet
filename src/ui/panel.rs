use iced::{
    widget::{button, svg},
    Background, Border, Color,
};

/// SVG copy-to-clipboard icon shared by Cmd and Settings.
pub const COPY_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
  fill="none" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <rect x="9" y="9" width="13" height="13" rx="2"/>
  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
</svg>"#;

/// Small icon button used in the action bar of terminal-style panels.
pub fn icon_btn<'a, Msg: Clone + 'a>(
    icon_bytes: &'static [u8],
    msg: Msg,
    enabled: bool,
    btn_bg: Color,
) -> iced::widget::Button<'a, Msg> {
    let icon_color = Color { a: if enabled { 1.0 } else { 0.35 }, ..Color::WHITE };
    let bg = Color { a: if enabled { btn_bg.a } else { btn_bg.a * 0.4 }, ..btn_bg };
    let icon = svg(svg::Handle::from_memory(icon_bytes.to_vec()))
        .width(16)
        .height(16)
        .style(move |_theme, _status| svg::Style { color: Some(icon_color) });
    let mut btn = button(icon)
        .padding(8)
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(bg)),
            border: Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        });
    if enabled {
        btn = btn.on_press(msg);
    }
    btn
}

/// Display state for terminal-style panels (Cmd, Settings).
pub enum PanelState {
    /// Nothing run yet — component shows its available commands or themes.
    Idle,
    /// A `display_result` command is running asynchronously (Cmd only).
    Running { prompt: String },
    /// Last operation completed — shows prompt line + output text.
    Result {
        prompt: String,
        output: String,
        /// Pre-built clipboard text ("$ prompt\noutput").
        copy_text: String,
    },
}
