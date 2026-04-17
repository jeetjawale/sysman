use crate::app::App;
use crate::collectors::Snapshot;
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Paragraph, Wrap},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, _snapshot: &Snapshot) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Min(6),
        ])
        .split(area);

    // -- Tab overview -------------------------------------------------------
    let tab_lines = vec![
        Line::from(Span::styled(
            "Tab Overview",
            Style::default()
                .fg(app.theme.brand)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        help_entry(&app.theme, "1 Dashboard", "System-wide overview with CPU, memory, network, disk metrics and live trends"),
        help_entry(&app.theme, "2 System", "Detailed host info, per-core CPU sparklines, memory and swap gauges"),
        help_entry(&app.theme, "3 Processes", "Sortable and filterable process table with memory chart"),
        help_entry(&app.theme, "4 Network", "Interface throughput, addresses, and active connections"),
        help_entry(&app.theme, "5 Disks", "Mounted partitions, usage hotspots, and capacity bar chart"),
        help_entry(&app.theme, "6 Services", "systemd service listing with state indicators"),
        help_entry(&app.theme, "7 Help", "This page"),
    ];
    frame.render_widget(
        Paragraph::new(tab_lines)
            .block(widgets::block(&app.theme, "Tabs"))
            .wrap(Wrap { trim: false }),
        sections[0],
    );

    // -- Keybindings --------------------------------------------------------
    let key_lines = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(app.theme.brand)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        key_line(&app.theme, "1-7", "Jump to tab"),
        key_line(&app.theme, "h / l", "Previous / next tab"),
        key_line(&app.theme, "j / k", "Scroll down / up"),
        key_line(&app.theme, "gg / G", "Jump to top / bottom"),
        key_line(&app.theme, "/", "Filter processes by name"),
        key_line(&app.theme, "s", "Cycle sort: CPU → Memory → Name"),
        key_line(&app.theme, "r", "Force refresh"),
        key_line(&app.theme, "q", "Quit"),
    ];
    frame.render_widget(
        Paragraph::new(key_lines)
            .block(widgets::block(&app.theme, "Keys"))
            .wrap(Wrap { trim: false }),
        sections[1],
    );

    // -- About --------------------------------------------------------------
    let about_lines = vec![
        Line::from(vec![
            Span::styled("Sysman", Style::default().fg(app.theme.brand).add_modifier(Modifier::BOLD)),
            Span::styled(" — ", Style::default().fg(app.theme.text_muted)),
            Span::raw("Terminal system monitor"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Refresh: ", Style::default().fg(app.theme.text_muted)),
            Span::styled("1s", Style::default().fg(app.theme.status_good)),
            Span::styled("  •  History: ", Style::default().fg(app.theme.text_muted)),
            Span::styled("60 samples", Style::default().fg(app.theme.status_good)),
            Span::styled("  •  Processes: ", Style::default().fg(app.theme.text_muted)),
            Span::styled("top 200", Style::default().fg(app.theme.status_good)),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(about_lines)
            .block(widgets::block(&app.theme, "About"))
            .wrap(Wrap { trim: false }),
        sections[2],
    );
}

fn help_entry<'a>(theme: &crate::theme::Theme, key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{key:<14}"),
            Style::default().fg(theme.status_info),
        ),
        Span::raw(desc),
    ])
}

fn key_line<'a>(theme: &crate::theme::Theme, key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{key:<12}"),
            Style::default()
                .fg(theme.status_warn)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(desc),
    ])
}
