use iced::{
    alignment,
    widget::{button, column, container, markdown, pick_list, row, scrollable, svg, text, Space},
    Alignment, Background, Border, Color, Element, Font, Length, Padding,
};

use crate::components::ai_agent::AiStatus;
use crate::theme::Theme;

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

fn icon_btn<'a, Msg: Clone + 'a>(
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
    // Always register on_press so the button consumes the click even when
    // visually disabled. Without it, the click leaks as Status::Ignored and
    // app.rs interprets that as a background click, closing the launcher.
    button(icon)
        .padding(8)
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(bg)),
            border: Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        })
        .on_press(msg)
}

fn md_settings(theme: &Theme) -> markdown::Settings {
    let (code_bg, code_text, link) =
        (theme.ai_code_background, theme.ai_code_text, theme.ai_link);
    markdown::Settings::with_text_size(15, markdown::Style {
        font: Font::default(),
        inline_code_highlight: markdown::Highlight {
            background: Background::Color(code_bg),
            border: Border { radius: 3.0.into(), ..Default::default() },
        },
        inline_code_padding: Padding { top: 1.0, right: 4.0, bottom: 1.0, left: 4.0 },
        inline_code_color: code_text,
        inline_code_font: Font::MONOSPACE,
        code_block_font: Font::MONOSPACE,
        link_color: link,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn ai_panel<'a, Msg: Clone + 'a>(
    status: &'a AiStatus,
    prompt: &str,
    copy_feedback: bool,
    items: &'a [markdown::Item],
    models: Vec<String>,
    selected_model: Option<String>,
    theme: &Theme,
    on_copy: Msg,
    on_retry: Msg,
    on_link: impl Fn(String) -> Msg + 'a,
    on_model_select: impl Fn(String) -> Msg + 'a,
) -> Element<'a, Msg> {
    let can_copy = matches!(status, AiStatus::Done(_));
    let can_retry = matches!(status, AiStatus::Done(_) | AiStatus::Error(_));

    // ── Body — lives inside the dark rounded box ──────────────────────────────

    let (idle_color, error_color, text_color) =
        (theme.ai_idle, theme.ai_error, theme.search_text);
    let body: Element<'a, Msg> = match status {
        AiStatus::Idle => container(
            text("Type /ai followed by your question")
                .size(15)
                .color(idle_color),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
        .into(),

        AiStatus::Loading { tick } => {
            let verb = VERBS[prompt.len() % 4];
            let dots = match tick { 0 => ".  ", 1 => ".. ", _ => "..." };
            container(text(format!("{verb}{dots}")).size(18).color(text_color))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
                .into()
        }

        AiStatus::Done(_) => scrollable(
            container(markdown::view(items, md_settings(theme)).map(on_link))
                .width(Length::Fill)
                .padding(4),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),

        AiStatus::Error(err) => container(
            text(format!("Error: {err}")).size(15).color(error_color),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
    };

    let panel_bg = theme.ai_panel;
    let text_area = container(body)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(panel_bg)),
            border: Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([16, 20]);

    // ── Action bar — outside the dark box ─────────────────────────────────────

    let (btn_bg, idle_color2, feedback_color) =
        (theme.button_background, theme.ai_idle, theme.copy_feedback);
    let model_picker: Element<'a, Msg> = if models.is_empty() {
        Space::new().width(0).height(0).into()
    } else {
        pick_list(models, selected_model, on_model_select)
            .text_size(13)
            .padding([4, 8])
            .style(move |_theme, _status| pick_list::Style {
                text_color: Color::WHITE,
                placeholder_color: idle_color2,
                handle_color: idle_color2,
                background: Background::Color(btn_bg),
                border: Border { radius: 6.0.into(), ..Default::default() },
            })
            .into()
    };

    let feedback: Element<'a, Msg> = if copy_feedback {
        text("Copied to clipboard").size(13).color(feedback_color).into()
    } else {
        text("").size(13).into()
    };

    let action_bar = row![
        model_picker,
        feedback,
        Space::new().width(Length::Fill),
        icon_btn(COPY_ICON, on_copy, can_copy, btn_bg),
        icon_btn(RETRY_ICON, on_retry, can_retry, btn_bg),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    column![text_area, action_bar]
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
