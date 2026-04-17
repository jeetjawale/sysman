use crate::app::App;
use crate::collectors::{self, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Paragraph, Wrap},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, _snapshot: &Snapshot) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(area);

    frame.render_widget(
        log_panel(
            app,
            "Journalctl",
            &app.logs_journal,
            app.logs_scroll,
            sections[0].height.saturating_sub(3) as usize,
        ),
        sections[0],
    );
    frame.render_widget(
        log_panel(
            app,
            "Syslog",
            &app.logs_syslog,
            app.logs_scroll,
            sections[1].height.saturating_sub(3) as usize,
        ),
        sections[1],
    );
    frame.render_widget(
        log_panel(
            app,
            "Dmesg",
            &app.logs_dmesg,
            app.logs_scroll,
            sections[2].height.saturating_sub(3) as usize,
        ),
        sections[2],
    );
}

fn log_panel<'a>(
    app: &'a App,
    title: &'a str,
    lines: &[String],
    scroll: usize,
    height: usize,
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
            rows.push(Line::from(collectors::truncate(line, 150)));
        }
    }
    Paragraph::new(rows)
        .block(widgets::block(&app.theme, title))
        .wrap(Wrap { trim: false })
}
