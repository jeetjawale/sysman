use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Paragraph, Wrap},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let text = vec![
        Line::from(Span::styled(
            "Current implementation baseline",
            Style::default()
                .fg(app.theme.brand)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Dashboard", Style::default().fg(app.theme.status_info)),
            Span::raw(": short summary across system, processes, network, disks, services"),
        ]),
        Line::from(vec![
            Span::styled("System", Style::default().fg(app.theme.status_info)),
            Span::raw(": full vitals + per-core history grid"),
        ]),
        Line::from(vec![
            Span::styled("Processes", Style::default().fg(app.theme.status_info)),
            Span::raw(": sort cycle + name filter"),
        ]),
        Line::from(vec![
            Span::styled("Network", Style::default().fg(app.theme.status_info)),
            Span::raw(": interfaces, throughput, active connections"),
        ]),
        Line::from(vec![
            Span::styled("Disks", Style::default().fg(app.theme.status_info)),
            Span::raw(": full partition table"),
        ]),
        Line::from(vec![
            Span::styled("Services", Style::default().fg(app.theme.status_info)),
            Span::raw(": systemd state when the bus is accessible"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Keys",
            Style::default()
                .fg(app.theme.active_tab)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("1-7", Style::default().fg(app.theme.status_warn)),
            Span::raw(" tabs  "),
            Span::styled("h/l", Style::default().fg(app.theme.status_warn)),
            Span::raw(" switch  "),
            Span::styled("j/k", Style::default().fg(app.theme.status_warn)),
            Span::raw(" scroll  "),
            Span::styled("gg/G", Style::default().fg(app.theme.status_warn)),
            Span::raw(" top/bottom"),
        ]),
        Line::from(vec![
            Span::styled("/", Style::default().fg(app.theme.status_warn)),
            Span::raw(" filter  "),
            Span::styled("s", Style::default().fg(app.theme.status_warn)),
            Span::raw(" sort  "),
            Span::styled("r", Style::default().fg(app.theme.status_warn)),
            Span::raw(" refresh  "),
            Span::styled("q", Style::default().fg(app.theme.status_warn)),
            Span::raw(" quit"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Roadmap",
            Style::default()
                .fg(app.theme.status_warn)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("process actions → logs → security → hardware → containers → plugins"),
    ];

    frame.render_widget(
        Paragraph::new(text)
            .block(widgets::block(&app.theme, "Help"))
            .wrap(Wrap { trim: false }),
        area,
    );
}
