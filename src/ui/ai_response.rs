use iced::{
    alignment,
    widget::{button, column, container, markdown, row, scrollable, svg, text, Space},
    Alignment, Background, Border, Color, Element, Font, Length, Padding,
};

use crate::app::Message;
use crate::modes::AiStatus;

const VERBS: [&str; 4] = ["Catapulting", "Launching", "Hurling", "Slinging"];

const RETRY_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
  fill="none" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/>
  <path d="M3 3v5h5"/>
</svg>"#;

const COPY_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
  fill="none" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <rect x="9" y="9" width="13" height="13" rx="2"/>
  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
</svg>"#;

fn icon_btn<'a>(icon_bytes: &'static [u8], msg: Message, enabled: bool) -> iced::widget::Button<'a, Message> {
    let alpha = if enabled { 1.0 } else { 0.35 };
    let icon = svg(svg::Handle::from_memory(icon_bytes.to_vec()))
        .width(16)
        .height(16)
        .style(move |_theme, _status| svg::Style {
            color: Some(Color { r: 1.0, g: 1.0, b: 1.0, a: alpha }),
        });
    let mut btn = button(icon)
        .padding(8)
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(Color {
                r: 1.0, g: 1.0, b: 1.0,
                a: if enabled { 0.10 } else { 0.04 },
            })),
            border: Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        });
    if enabled {
        btn = btn.on_press(msg);
    }
    btn
}

fn md_settings() -> markdown::Settings {
    markdown::Settings::with_text_size(15, markdown::Style {
        font: Font::default(),
        inline_code_highlight: markdown::Highlight {
            background: Background::Color(Color { r: 0.18, g: 0.18, b: 0.26, a: 1.0 }),
            border: Border { radius: 3.0.into(), ..Default::default() },
        },
        inline_code_padding: Padding { top: 1.0, right: 4.0, bottom: 1.0, left: 4.0 },
        inline_code_color: Color { r: 0.95, g: 0.78, b: 0.50, a: 1.0 },
        inline_code_font: Font::MONOSPACE,
        code_block_font: Font::MONOSPACE,
        link_color: Color { r: 0.45, g: 0.75, b: 1.0, a: 1.0 },
    })
}

pub fn ai_panel<'a>(
    status: &'a AiStatus,
    prompt: &str,
    copy_feedback: bool,
    items: &'a [markdown::Item],
) -> Element<'a, Message> {
    let can_copy  = matches!(status, AiStatus::Done(_));
    let can_retry = matches!(status, AiStatus::Done(_) | AiStatus::Error(_));

    // ── Body — lives inside the dark rounded box ──────────────────────────────

    let body: Element<'a, Message> = match status {
        AiStatus::Idle => container(
            text("Type /ai followed by your question")
                .size(15)
                .color(Color { r: 0.6, g: 0.6, b: 0.6, a: 1.0 }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
        .into(),

        AiStatus::Loading { tick } => {
            let verb = VERBS[prompt.len() % 4];
            let dots = match tick {
                0 => ".  ",
                1 => ".. ",
                _ => "...",
            };
            container(text(format!("{verb}{dots}")).size(18).color(Color::WHITE))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
                .into()
        }

        AiStatus::Done(_) => scrollable(
            container(
                markdown::view(items, md_settings()).map(Message::LinkClicked),
            )
            .width(Length::Fill)
            .padding(4),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),

        AiStatus::Error(err) => container(
            text(format!("Error: {err}"))
                .size(15)
                .color(Color { r: 1.0, g: 0.4, b: 0.4, a: 1.0 }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
    };

    let text_area = container(body)
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color { r: 0.05, g: 0.05, b: 0.09, a: 1.0 })),
            border: Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([16, 20]);

    // ── Action bar — outside the dark box ─────────────────────────────────────

    let feedback: Element<'a, Message> = if copy_feedback {
        text("Copied to clipboard")
            .size(13)
            .color(Color { r: 0.5, g: 0.9, b: 0.6, a: 1.0 })
            .into()
    } else {
        text("").size(13).into()
    };

    let action_bar = row![
        feedback,
        Space::new().width(Length::Fill),
        icon_btn(COPY_ICON, Message::AiCopyResponse, can_copy),
        icon_btn(RETRY_ICON, Message::AiRetry, can_retry),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    column![text_area, action_bar]
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
