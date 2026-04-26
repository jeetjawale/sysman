use ratatui::prelude::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub surface: Color,
    pub brand: Color,
    pub border: Color,
    pub border_active: Color,
    pub divider: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub text_dim: Color,
    pub status_good: Color,
    pub status_warn: Color,
    pub status_error: Color,
    pub status_info: Color,
    pub selection_bg: Color,
    pub highlight_bg: Color,
    pub alt_row_bg: Color,
    pub active_tab: Color,
    pub inactive_tab: Color,
    pub chart_colors: Vec<Color>,
}

/// Modern cyber-dark theme with deep purples and electric accents
pub fn default_theme() -> Theme {
    Theme {
        surface: Color::Rgb(22, 22, 30),
        brand: Color::Rgb(139, 92, 246),
        border: Color::Rgb(55, 55, 70),
        border_active: Color::Rgb(139, 92, 246),
        divider: Color::Rgb(40, 40, 55),
        text_primary: Color::Rgb(250, 250, 252),
        text_secondary: Color::Rgb(161, 161, 180),
        text_muted: Color::Rgb(113, 113, 130),
        text_dim: Color::Rgb(75, 75, 90),
        status_good: Color::Rgb(52, 211, 153),
        status_warn: Color::Rgb(251, 191, 36),
        status_error: Color::Rgb(248, 113, 113),
        status_info: Color::Rgb(6, 182, 212),
        selection_bg: Color::Rgb(35, 35, 50),
        highlight_bg: Color::Rgb(50, 45, 75),
        alt_row_bg: Color::Rgb(18, 18, 26),
        active_tab: Color::Rgb(139, 92, 246),
        inactive_tab: Color::Rgb(113, 113, 130),
        chart_colors: vec![
            Color::Rgb(52, 211, 153),
            Color::Rgb(6, 182, 212),
            Color::Rgb(139, 92, 246),
            Color::Rgb(244, 114, 182),
            Color::Rgb(251, 191, 36),
            Color::Rgb(248, 113, 113),
        ],
    }
}

pub fn parse_color(hex: &str) -> Result<Color, String> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err("Hex color must be 6 characters (e.g. #FFFFFF)".into());
    }
    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
    Ok(Color::Rgb(r, g, b))
}
