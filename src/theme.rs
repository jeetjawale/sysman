use ratatui::prelude::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub brand: Color,
    pub active_tab: Color,
    pub inactive_tab: Color,
    pub border: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub status_good: Color,
    pub status_warn: Color,
    pub status_error: Color,
    pub status_info: Color,
    pub selection_bg: Color,
}

pub fn default_theme() -> Theme {
    Theme {
        brand: Color::Cyan,
        active_tab: Color::Yellow,
        inactive_tab: Color::DarkGray,
        border: Color::DarkGray,
        text_primary: Color::White,
        text_secondary: Color::Gray,
        text_muted: Color::DarkGray,
        status_good: Color::Green,
        status_warn: Color::Yellow,
        status_error: Color::Red,
        status_info: Color::Cyan,
        selection_bg: Color::Rgb(40, 40, 60),
    }
}
