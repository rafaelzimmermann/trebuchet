mod grid;
pub mod panel;
mod search;
pub mod ai_response;

pub use grid::app_grid;
pub use search::{search_bar, SearchIcon, ShakeState};
pub use ai_response::ai_panel;

pub const PANEL_PADDING: iced::Padding =
    iced::Padding { top: 24.0, bottom: 24.0, left: 80.0, right: 80.0 };
