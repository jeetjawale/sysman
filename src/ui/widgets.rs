use crate::collectors::{self, DiskRow, Snapshot};
use crate::theme::Theme;
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Gauge, Padding, Paragraph, Sparkline},
};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Animation helpers
// ---------------------------------------------------------------------------

/// Get pulsing opacity (0.3 to 1.0) based on animation frame
pub fn pulse_opacity(frame: u32) -> f32 {
    let cycle = (frame % 60) as f32 / 60.0;
    0.3 + 0.7 * (std::f32::consts::PI * 2.0 * cycle).sin().abs()
}

/// Get smooth pulse for breathing effect
pub fn breathe(frame: u32) -> f32 {
    let cycle = (frame % 120) as f32 / 120.0;
    0.5 + 0.5 * (std::f32::consts::PI * 2.0 * cycle).sin()
}

/// Spinner character for loading states
pub fn spinner_char(frame: u32) -> char {
    const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    FRAMES[(frame as usize / 3) % FRAMES.len()]
}

/// Interpolate between two colors
pub fn blend_colors(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => Color::Rgb(
            (r1 as f32 * (1.0 - t) + r2 as f32 * t) as u8,
            (g1 as f32 * (1.0 - t) + g2 as f32 * t) as u8,
            (b1 as f32 * (1.0 - t) + b2 as f32 * t) as u8,
        ),
        _ => {
            if t < 0.5 {
                a
            } else {
                b
            }
        }
    }
}

/// Get animated glow color
pub fn glow_color(base: Color, intensity: f32, frame: u32) -> Color {
    let pulse = breathe(frame);
    let adjusted = intensity * (0.6 + 0.4 * pulse);
    match base {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as f32 * adjusted).min(255.0)) as u8,
            ((g as f32 * adjusted).min(255.0)) as u8,
            ((b as f32 * adjusted).min(255.0)) as u8,
        ),
        _ => base,
    }
}

/// Get status color with smooth gradient
pub fn smooth_status_color(theme: &Theme, value: f64) -> Color {
    if value >= 90.0 {
        theme.status_error
    } else if value >= 75.0 {
        let t = ((value - 75.0) / 15.0) as f32;
        blend_colors(theme.status_warn, theme.status_error, t)
    } else if value >= 60.0 {
        let t = ((value - 60.0) / 15.0) as f32;
        blend_colors(theme.status_good, theme.status_warn, t)
    } else {
        theme.status_good
    }
}

// ---------------------------------------------------------------------------
// Block builders with animations
// ---------------------------------------------------------------------------

/// Standard bordered block with rounded corners and subtle styling
pub fn block<'a>(theme: &Theme, title: &'a str) -> Block<'a> {
    Block::default()
        .title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(theme.brand)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .padding(Padding::new(1, 1, 0, 0))
}

/// Block with glowing active state
pub fn active_block<'a>(theme: &Theme, title: &'a str, frame: u32) -> Block<'a> {
    let glow = glow_color(theme.border_active, 1.2, frame);
    Block::default()
        .title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(theme.brand)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(glow).add_modifier(Modifier::BOLD))
        .padding(Padding::new(1, 1, 0, 0))
}

/// Block with subtle surface background
pub fn surface_block<'a>(theme: &Theme, title: &'a str) -> Block<'a> {
    Block::default()
        .title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(theme.text_secondary)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.surface))
        .padding(Padding::new(1, 1, 0, 0))
}

// ---------------------------------------------------------------------------
// Metric cards with enhanced visuals
// ---------------------------------------------------------------------------

/// Simple value card with title, large value, and subtitle
pub fn metric_card<'a>(
    theme: &Theme,
    title: &'a str,
    value: &'a str,
    subtitle: &'a str,
    color: Color,
    frame: u32,
) -> Paragraph<'a> {
    let glow = glow_color(color, 1.1, frame);
    Paragraph::new(vec![
        Line::from(vec![Span::styled(
            value,
            Style::default()
                .fg(glow)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )]),
        Line::from(vec![
            Span::styled("↳ ", Style::default().fg(theme.text_muted)),
            Span::styled(subtitle, Style::default().fg(theme.text_secondary)),
        ]),
    ])
    .block(block(theme, title))
    .alignment(Alignment::Center)
}

/// Gauge bar card with animated color transitions
pub fn gauge_card<'a>(
    theme: &Theme,
    title: &'a str,
    value: f64,
    label: &'a str,
    _color: Color,
    frame: u32,
) -> Gauge<'a> {
    let ratio = (value / 100.0).clamp(0.0, 1.0);
    let smooth_color = smooth_status_color(theme, value);
    let glow = glow_color(smooth_color, 1.0, frame);

    let bar_style = if value >= 90.0 {
        Style::default()
            .fg(theme.status_error)
            .bg(theme.selection_bg)
            .add_modifier(Modifier::BOLD)
    } else if value >= 75.0 {
        Style::default()
            .fg(theme.status_warn)
            .bg(theme.selection_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(glow)
            .bg(theme.selection_bg)
            .add_modifier(Modifier::BOLD)
    };

    Gauge::default()
        .block(block(theme, title))
        .gauge_style(bar_style)
        .ratio(ratio)
        .label(label)
}

// ---------------------------------------------------------------------------
// Chart and sparkline widgets
// ---------------------------------------------------------------------------

/// Sparkline panel for historical data with gradient effect
pub fn spark_panel<'a>(
    theme: &Theme,
    title: &'a str,
    history: &VecDeque<u64>,
    color: Color,
    frame: u32,
) -> Sparkline<'a> {
    let data: Vec<u64> = history.iter().copied().collect();
    let glow = glow_color(color, 0.9, frame);

    Sparkline::default()
        .block(surface_block(theme, title))
        .style(Style::default().fg(glow).bg(theme.alt_row_bg))
        .data(&data)
        .max(100)
        .direction(ratatui::widgets::RenderDirection::LeftToRight)
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

/// Key-value line for info panels
pub fn kv_line(theme: &Theme, key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{}: ", key), Style::default().fg(theme.text_muted)),
        Span::styled(value.to_string(), Style::default().fg(theme.text_primary)),
    ])
}

/// Color based on threshold value (green < 75 < yellow < 90 < red)
pub fn status_color(theme: &Theme, value: f64) -> Color {
    if value >= 90.0 {
        theme.status_error
    } else if value >= 75.0 {
        theme.status_warn
    } else {
        theme.status_good
    }
}

/// Style colored by usage threshold
pub fn usage_style(theme: &Theme, usage: f64) -> Style {
    Style::default().fg(status_color(theme, usage))
}

/// Format bytes/s as human-readable rate
pub fn format_rate(bytes_per_second: u64) -> String {
    format!("{}/s", collectors::format_bytes(bytes_per_second))
}

/// Pad status string to fixed width
pub fn pad_status(value: &str) -> String {
    format!("{value:<8}")
}

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------

/// Calculate visible rows in a table area (subtracting header/borders)
pub fn visible_rows(area: Rect, reserved: u16) -> usize {
    area.height.saturating_sub(reserved).max(1) as usize
}

/// Center a popup rectangle within a parent area
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// ---------------------------------------------------------------------------
// Data helpers shared across tabs
// ---------------------------------------------------------------------------

/// Get the N disks with highest usage
pub fn hottest_disks(snapshot: &Snapshot, limit: usize) -> Vec<&DiskRow> {
    let mut disks: Vec<_> = snapshot.disks.iter().collect();
    disks.sort_by(|a, b| b.usage.total_cmp(&a.usage));
    disks.into_iter().take(limit).collect()
}

/// Label for current process sort mode
pub fn process_sort_label(sort: crate::cli::ProcessSort) -> &'static str {
    match sort {
        crate::cli::ProcessSort::Cpu => "cpu",
        crate::cli::ProcessSort::Memory => "memory",
        crate::cli::ProcessSort::Pid => "pid",
        crate::cli::ProcessSort::Name => "name",
    }
}
