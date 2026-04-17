use crate::collectors::{self, DiskRow, Snapshot};
use crate::theme::Theme;
use ratatui::{
    prelude::*,
    widgets::{
        Block, BorderType, Borders, Gauge, LineGauge, Padding, Paragraph, Sparkline,
    },
};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Block builders
// ---------------------------------------------------------------------------

/// Standard bordered block with rounded corners.
pub fn block<'a>(theme: &Theme, title: &'a str) -> Block<'a> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(theme.brand)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .padding(Padding::new(1, 1, 0, 0))
}

/// Highlighted block with double borders for active selection.
pub fn active_block<'a>(theme: &Theme, title: &'a str) -> Block<'a> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(theme.brand)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme.border_active))
        .padding(Padding::new(1, 1, 0, 0))
}

// ---------------------------------------------------------------------------
// Metric cards
// ---------------------------------------------------------------------------

/// Simple value card with title, large value, and subtitle.
pub fn metric_card<'a>(
    theme: &Theme,
    title: &'a str,
    value: &'a str,
    subtitle: &'a str,
    color: Color,
) -> Paragraph<'a> {
    Paragraph::new(vec![
        Line::from(vec![Span::styled(
            value,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("↳ ", Style::default().fg(theme.text_muted)),
            Span::styled(subtitle, Style::default().fg(theme.text_secondary)),
        ]),
    ])
    .block(block(theme, title))
    .alignment(Alignment::Center)
}

/// Gauge bar card with color thresholds.
pub fn gauge_card<'a>(
    theme: &Theme,
    title: &'a str,
    value: f64,
    label: &'a str,
    color: Color,
) -> Gauge<'a> {
    let ratio = (value / 100.0).clamp(0.0, 1.0);
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
            .fg(color)
            .bg(theme.selection_bg)
            .add_modifier(Modifier::BOLD)
    };
    Gauge::default()
        .block(block(theme, title))
        .gauge_style(bar_style)
        .ratio(ratio)
        .label(label)
}

/// Sparkline panel for historical data.
pub fn spark_panel<'a>(
    theme: &Theme,
    title: &'a str,
    history: &VecDeque<u64>,
    color: Color,
) -> Sparkline<'a> {
    let data: Vec<u64> = history.iter().copied().collect();
    Sparkline::default()
        .block(block(theme, title))
        .style(Style::default().fg(color).bg(theme.alt_row_bg))
        .data(&data)
        .max(100)
        .direction(ratatui::widgets::RenderDirection::LeftToRight)
}

/// LineGauge panel with color thresholds.
pub fn line_gauge_panel<'a>(
    theme: &Theme,
    title: &'a str,
    ratio: f64,
    color: Color,
    label: &'a str,
) -> LineGauge<'a> {
    let filled_style = if ratio >= 0.9 {
        Style::default()
            .fg(theme.status_error)
            .add_modifier(Modifier::BOLD)
    } else if ratio >= 0.75 {
        Style::default().fg(theme.status_warn)
    } else {
        Style::default().fg(color)
    };
    LineGauge::default()
        .block(block(theme, title))
        .filled_style(filled_style)
        .unfilled_style(Style::default().fg(theme.border))
        .label(label)
        .ratio(ratio.clamp(0.0, 1.0))
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

/// Key-value line for info panels.
pub fn kv_line(theme: &Theme, key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key}: "), Style::default().fg(theme.text_muted)),
        Span::raw(value.to_string()),
    ])
}

/// Color based on threshold value (green < 75 < yellow < 90 < red).
pub fn status_color(theme: &Theme, value: f64) -> Color {
    if value >= 90.0 {
        theme.status_error
    } else if value >= 75.0 {
        theme.status_warn
    } else {
        theme.status_good
    }
}

/// Style colored by usage threshold.
pub fn usage_style(theme: &Theme, usage: f64) -> Style {
    Style::default().fg(status_color(theme, usage))
}

/// Format bytes/s as human-readable rate.
pub fn format_rate(bytes_per_second: u64) -> String {
    format!("{}/s", collectors::format_bytes(bytes_per_second))
}

/// Pad status string to fixed width.
pub fn pad_status(value: &str) -> String {
    format!("{value:<8}")
}

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------

/// Calculate visible rows in a table area (subtracting header/borders).
pub fn visible_rows(area: Rect, reserved: u16) -> usize {
    area.height.saturating_sub(reserved).max(1) as usize
}

/// Center a popup rectangle within a parent area.
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

/// Get the N disks with highest usage.
pub fn hottest_disks<'a>(snapshot: &'a Snapshot, limit: usize) -> Vec<&'a DiskRow> {
    let mut disks: Vec<_> = snapshot.disks.iter().collect();
    disks.sort_by(|a, b| b.usage.total_cmp(&a.usage));
    disks.into_iter().take(limit).collect()
}

/// Label for current process sort mode.
pub fn process_sort_label(sort: crate::cli::ProcessSort) -> &'static str {
    match sort {
        crate::cli::ProcessSort::Cpu => "cpu",
        crate::cli::ProcessSort::Memory => "memory",
        crate::cli::ProcessSort::Name => "name",
    }
}
