use iced::{
    widget::{container, row, svg, text_input, Id, Space},
    Alignment, Background, Border, Color, Element, Length,
};

use crate::theme::Theme;

const SEARCH_ID_LAUNCHER:  &str = "trebuchet_search_launcher";
const SEARCH_ID_AI:        &str = "trebuchet_search_ai";
const SEARCH_ID_TERMINAL:  &str = "trebuchet_search_terminal";

const SEARCH_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
  fill="none" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="11" cy="11" r="7"/>
  <line x1="16.5" y1="16.5" x2="21" y2="21"/>
</svg>"#;

const ROBOT_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
  fill="none" stroke="white" stroke-width="1.5" stroke-linecap="round">
  <rect x="5" y="7" width="14" height="10" rx="2"/>
  <line x1="12" y1="7" x2="12" y2="4"/>
  <circle cx="12" cy="3.5" r="1"/>
  <circle cx="9" cy="11" r="1.2" fill="white"/>
  <circle cx="15" cy="11" r="1.2" fill="white"/>
  <line x1="9" y1="14" x2="15" y2="14"/>
</svg>"#;

const TERMINAL_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
  fill="none" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <rect x="2" y="3" width="20" height="16" rx="2"/>
  <polyline points="6,8 10,12 6,16"/>
  <line x1="13" y1="16" x2="19" y2="16"/>
</svg>"#;

const SHAKE_OFFSETS: [f32; 6] = [-8.0, 8.0, -5.0, 5.0, -2.0, 0.0];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShakeState {
    pub active: bool,
    pub tick: u8,
}

impl ShakeState {
    pub fn trigger() -> Self {
        ShakeState { active: true, tick: 0 }
    }

    pub fn advance(&mut self) {
        self.tick += 1;
        if self.tick >= 6 {
            *self = ShakeState::default();
        }
    }
}

#[derive(Clone, Copy)]
pub enum SearchIcon {
    Search,
    Robot,
    Terminal,
}

pub fn search_bar<'a, Msg: Clone + 'a>(
    query: &str,
    shake: &ShakeState,
    icon: SearchIcon,
    theme: &Theme,
    on_input: impl Fn(String) -> Msg + 'a,
) -> Element<'a, Msg> {
    let (icon_bytes, search_id) = match icon {
        SearchIcon::Search   => (SEARCH_SVG,   SEARCH_ID_LAUNCHER),
        SearchIcon::Robot    => (ROBOT_SVG,    SEARCH_ID_AI),
        SearchIcon::Terminal => (TERMINAL_SVG, SEARCH_ID_TERMINAL),
    };
    let icon_widget: Element<'a, Msg> = svg(svg::Handle::from_memory(icon_bytes.to_vec()))
        .width(20)
        .height(20)
        .into();

    let placeholder = match icon {
        SearchIcon::Search   => "Search apps...",
        SearchIcon::Robot    => "Ask anything...",
        SearchIcon::Terminal => "",
    };
    let (text_color, placeholder_color, selection_color) =
        (theme.search_text, theme.search_placeholder, theme.search_selection);
    let input = text_input(placeholder, query)
        .id(Id::new(search_id))
        .on_input(on_input)
        .padding(0)
        .size(20)
        .width(Length::Fill)
        .style(move |_theme, _status| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: text_color,
            placeholder: placeholder_color,
            value: text_color,
            selection: selection_color,
        });

    let inner = row![icon_widget, input]
        .spacing(12)
        .align_y(Alignment::Center);

    let (bg, border_color) = (theme.search_background, theme.search_border);
    let pill = container(inner)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border { radius: 12.0.into(), width: 1.0, color: border_color },
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
