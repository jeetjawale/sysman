use crate::app::{App, LogLevelFilter, LogSourceFilter};
use crate::collectors::{self, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Paragraph, Wrap},
};
use regex::Regex;

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, _snapshot: &Snapshot) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(12)])
        .split(area);

    let regex = if app.logs_query.trim().is_empty() {
        None
    } else {
        Regex::new(app.logs_query.trim()).ok()
    };

    let journal_rows = filtered_logs(&app.logs_journal, app.logs_level_filter, regex.as_ref());
    let syslog_rows = filtered_logs(&app.logs_syslog, app.logs_level_filter, regex.as_ref());
    let dmesg_rows = filtered_logs(&app.logs_dmesg, app.logs_level_filter, regex.as_ref());

    frame.render_widget(
        logs_summary(app, &journal_rows, &syslog_rows, &dmesg_rows),
        sections[0],
    );

    match app.logs_source_filter {
        LogSourceFilter::All => {
            let panes = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(32),
                    Constraint::Percentage(34),
                    Constraint::Percentage(34),
                ])
                .split(sections[1]);
            frame.render_widget(
                log_panel(
                    app,
                    "Journalctl",
                    &journal_rows,
                    app.logs_scroll,
                    panes[0].height.saturating_sub(3) as usize,
                    regex.as_ref(),
                ),
                panes[0],
            );
            frame.render_widget(
                log_panel(
                    app,
                    "Syslog",
                    &syslog_rows,
                    app.logs_scroll,
                    panes[1].height.saturating_sub(3) as usize,
                    regex.as_ref(),
                ),
                panes[1],
            );
            frame.render_widget(
                log_panel(
                    app,
                    "Dmesg",
                    &dmesg_rows,
                    app.logs_scroll,
                    panes[2].height.saturating_sub(3) as usize,
                    regex.as_ref(),
                ),
                panes[2],
            );
        }
        LogSourceFilter::Journal => frame.render_widget(
            log_panel(
                app,
                "Journalctl",
                &journal_rows,
                app.logs_scroll,
                sections[1].height.saturating_sub(3) as usize,
                regex.as_ref(),
            ),
            sections[1],
        ),
        LogSourceFilter::Syslog => frame.render_widget(
            log_panel(
                app,
                "Syslog",
                &syslog_rows,
                app.logs_scroll,
                sections[1].height.saturating_sub(3) as usize,
                regex.as_ref(),
            ),
            sections[1],
        ),
        LogSourceFilter::Dmesg => frame.render_widget(
            log_panel(
                app,
                "Dmesg",
                &dmesg_rows,
                app.logs_scroll,
                sections[1].height.saturating_sub(3) as usize,
                regex.as_ref(),
            ),
            sections[1],
        ),
    }
}

fn logs_summary(
    app: &App,
    journal: &[String],
    syslog: &[String],
    dmesg: &[String],
) -> Paragraph<'static> {
    let total = journal.len() + syslog.len() + dmesg.len();
    let all: Vec<String> = journal
        .iter()
        .chain(syslog.iter())
        .chain(dmesg.iter())
        .cloned()
        .collect();
    let spike = detect_error_spike(&all).unwrap_or_else(|| "stable".into());
    let spike_is_stable = spike == "stable";

    Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Level: ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                app.logs_level_label().to_uppercase(),
                Style::default()
                    .fg(app.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Source: ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                app.logs_source_label().to_uppercase(),
                Style::default()
                    .fg(app.theme.status_info)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Regex: ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                if app.logs_query.is_empty() {
                    "none".into()
                } else {
                    app.logs_query.clone()
                },
                Style::default().fg(app.theme.status_info),
            ),
            Span::raw("  "),
            Span::styled("Rows: ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                total.to_string(),
                Style::default().fg(app.theme.text_primary),
            ),
        ]),
        Line::from(vec![
            Span::styled("Spike: ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                spike,
                Style::default().fg(if spike_is_stable {
                    app.theme.status_good
                } else {
                    app.theme.status_error
                }),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "(`/` regex, `v` level, `o` source, `a` autoscroll:{}, `n/N` matches)",
                    if app.logs_autoscroll { "on" } else { "off" }
                ),
                Style::default().fg(app.theme.text_muted),
            ),
        ]),
    ])
    .block(widgets::block(&app.theme, "Logs Controls"))
}

fn log_panel<'a>(
    app: &App,
    title: &'a str,
    lines: &[String],
    scroll: usize,
    height: usize,
    regex: Option<&Regex>,
) -> Paragraph<'a> {
    let mut rows: Vec<Line> = Vec::new();
    if lines.is_empty() {
        rows.push(Line::from("No log data available"));
    } else {
        let offset = if lines.len() <= height {
            0
        } else {
            scroll.min(lines.len() - height)
        };
        for line in lines.iter().skip(offset).take(height.max(1)) {
            rows.push(highlight_line(app, &collectors::truncate(line, 180), regex));
        }
    }
    Paragraph::new(rows)
        .block(widgets::block(&app.theme, title))
        .wrap(Wrap { trim: false })
}

fn filtered_logs(lines: &[String], level: LogLevelFilter, regex: Option<&Regex>) -> Vec<String> {
    lines
        .iter()
        .filter(|line| matches_level(line, level))
        .filter(|line| match regex {
            Some(re) => re.is_match(line),
            None => true,
        })
        .cloned()
        .collect()
}

fn matches_level(line: &str, level: LogLevelFilter) -> bool {
    let lower = line.to_ascii_lowercase();
    let is_error = lower.contains("error")
        || lower.contains(" err ")
        || lower.contains("failed")
        || lower.contains("panic")
        || lower.contains("fatal")
        || lower.contains("crit");
    let is_warn = lower.contains("warn");
    let is_info = lower.contains("info");

    match level {
        LogLevelFilter::All => true,
        LogLevelFilter::Error => is_error,
        LogLevelFilter::Warn => is_warn,
        LogLevelFilter::Info => is_info,
    }
}

fn detect_error_spike(lines: &[String]) -> Option<String> {
    if lines.len() < 24 {
        return None;
    }
    let window = (lines.len() / 6).clamp(12, 40);
    if lines.len() < window * 2 {
        return None;
    }

    let errors: Vec<bool> = lines
        .iter()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("error")
                || lower.contains(" err ")
                || lower.contains("failed")
                || lower.contains("panic")
                || lower.contains("fatal")
                || lower.contains("crit")
        })
        .collect();
    let recent_errors = errors[errors.len() - window..]
        .iter()
        .filter(|is_error| **is_error)
        .count();
    let prev_errors = errors[errors.len() - (window * 2)..errors.len() - window]
        .iter()
        .filter(|is_error| **is_error)
        .count();
    let recent_rate = recent_errors as f64 / window as f64;
    let prev_rate = prev_errors as f64 / window as f64;

    if recent_errors >= 6 && recent_rate >= (prev_rate * 1.8).max(0.15) {
        Some(format!(
            "spike {}→{} errs/window (w={window})",
            prev_errors, recent_errors
        ))
    } else if recent_errors >= window / 2 {
        Some(format!("high error pressure ({recent_errors}/{window})"))
    } else {
        None
    }
}

fn highlight_line(app: &App, line: &str, regex: Option<&Regex>) -> Line<'static> {
    let Some(regex) = regex else {
        return Line::from(line.to_string());
    };

    let mut spans: Vec<Span> = Vec::new();
    let mut cursor = 0usize;
    for matched in regex.find_iter(line) {
        if matched.start() > cursor {
            spans.push(Span::raw(line[cursor..matched.start()].to_string()));
        }
        spans.push(Span::styled(
            line[matched.start()..matched.end()].to_string(),
            Style::default()
                .bg(app.theme.highlight_bg)
                .fg(app.theme.status_warn)
                .add_modifier(Modifier::BOLD),
        ));
        cursor = matched.end();
    }
    if cursor < line.len() {
        spans.push(Span::raw(line[cursor..].to_string()));
    }

    if spans.is_empty() {
        Line::from(line.to_string())
    } else {
        Line::from(spans)
    }
}
