use crate::app::App;
use crate::collectors::{self, ServiceRow, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{
        Cell, Clear, List, ListItem, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState, Wrap,
    },
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, snapshot: &Snapshot) {
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(area);

    // -- Service table ------------------------------------------------------
    let mut service_state = TableState::default();
    service_state.select(Some(app.service_scroll));
    frame.render_stateful_widget(
        service_table(
            app,
            &snapshot.services,
            app.service_scroll,
            widgets::visible_rows(sections[0], 4),
        ),
        sections[0],
        &mut service_state,
    );
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(Style::default().fg(app.theme.brand))
        .track_style(Style::default().fg(app.theme.border));
    let mut scrollbar_state = ScrollbarState::new(snapshot.services.len())
        .position(app.service_scroll)
        .viewport_content_length(widgets::visible_rows(sections[0], 4));
    frame.render_stateful_widget(scrollbar, sections[0], &mut scrollbar_state);

    // -- Sidebar ------------------------------------------------------------
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Min(9),
        ])
        .split(sections[1]);

    frame.render_widget(service_stats(app, snapshot), sidebar[0]);
    frame.render_widget(service_focus(app, snapshot), sidebar[1]);
    frame.render_widget(service_failure_details(app), sidebar[2]);
    frame.render_widget(service_guidance(app, snapshot), sidebar[3]);
    frame.render_widget(service_logs(app, snapshot), sidebar[4]);

    // -- Error popup (if service data unavailable) --------------------------
    if let Some(error) = &app.service_error {
        let popup = widgets::centered_rect(64, 20, area);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(format!(
                "{error}\n\nSystem, process, disk, and network tabs still have live data.\nThe service tab needs access to the systemd bus."
            ))
            .block(widgets::block(&app.theme, "Service Notice"))
            .wrap(Wrap { trim: false }),
            popup,
        );
    }
}

// ---------------------------------------------------------------------------
// Service sub-widgets
// ---------------------------------------------------------------------------

fn service_table<'a>(
    app: &'a App,
    services: &'a [ServiceRow],
    offset: usize,
    height: usize,
) -> Table<'a> {
    let rows: Vec<Row> = services
        .iter()
        .enumerate()
        .skip(offset.min(services.len()))
        .take(height)
        .map(|(idx, service)| {
            let active_color = if service.active == "active" {
                app.theme.status_good
            } else if service.active == "failed" {
                app.theme.status_error
            } else if service.active == "inactive" {
                app.theme.text_muted
            } else {
                app.theme.status_warn
            };
            let row_style = if idx % 2 == 0 {
                Style::default()
            } else {
                Style::default().bg(app.theme.alt_row_bg)
            };
            Row::new(vec![
                Cell::from(collectors::truncate(&service.name, 36)),
                Cell::from(service.active.clone()).style(
                    Style::default()
                        .fg(active_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(service.sub.clone()),
            ])
            .style(row_style)
        })
        .collect();

    let selected_style = Style::default()
        .bg(app.theme.highlight_bg)
        .add_modifier(Modifier::BOLD);

    Table::new(
        rows,
        [
            Constraint::Percentage(60),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new(vec![
            format!("Name ({})", app.service_state_filter_label()),
            "Active".into(),
            "Sub".into(),
        ])
        .style(
            Style::default()
                .fg(app.theme.active_tab)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .row_highlight_style(selected_style)
    .block(widgets::active_block(
        &app.theme,
        "Services",
        app.animation_frame,
    ))
}

fn service_stats(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let counts = snapshot.service_state_counts.unwrap_or_default();
    let mut lines = vec![
        widgets::kv_line(&app.theme, "Rows", &snapshot.services.len().to_string()),
        widgets::kv_line(&app.theme, "Filter", app.service_state_filter_label()),
        widgets::kv_line(&app.theme, "Running", &counts.running.to_string()),
        widgets::kv_line(&app.theme, "Failed", &counts.failed.to_string()),
    ];
    lines.push(Line::from(vec![
        Span::styled(
            format!("[run {}] ", counts.running),
            Style::default().fg(app.theme.status_good),
        ),
        Span::styled(
            format!("[fail {}] ", counts.failed),
            Style::default().fg(app.theme.status_error),
        ),
        Span::styled(
            format!("[inactive {}]", counts.inactive),
            Style::default().fg(app.theme.text_muted),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!("[activating {}] ", counts.activating),
            Style::default().fg(app.theme.status_info),
        ),
        Span::styled(
            format!("[deactivating {}]", counts.deactivating),
            Style::default().fg(app.theme.status_warn),
        ),
    ]));
    Paragraph::new(lines).block(widgets::block(&app.theme, "Summary"))
}

fn service_focus(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines = Vec::new();
    for service in snapshot.services.iter().take(5) {
        lines.push(Line::from(vec![
            Span::styled(
                widgets::pad_status(&service.sub),
                service_row_style(app, service),
            ),
            Span::raw(format!(" {}", collectors::truncate(&service.name, 18))),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from("No service rows available"));
    }
    Paragraph::new(lines).block(widgets::block(&app.theme, "Visible Services"))
}

fn service_guidance<'a>(app: &'a App, snapshot: &'a Snapshot) -> List<'a> {
    let headline = if snapshot.service_summary.is_some() {
        "systemd listing is live"
    } else {
        "systemd access blocked"
    };
    let headline_color = if snapshot.service_summary.is_some() {
        app.theme.status_good
    } else {
        app.theme.status_error
    };
    let items = vec![
        ListItem::new(Span::styled(headline, Style::default().fg(headline_color))),
        ListItem::new(""),
        ListItem::new("• `s` cycle filter running/failed/all"),
        ListItem::new("• `u/i/o` start/stop/restart"),
        ListItem::new("• `e/d` enable/disable"),
        ListItem::new("• `w/W` mask/unmask"),
    ];

    List::new(items)
        .block(widgets::block(&app.theme, "Info"))
        .highlight_style(Style::default().fg(app.theme.brand))
}

fn service_logs(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    let selected = app
        .selected_service_name(snapshot)
        .unwrap_or_else(|| "none".into());
    lines.push(widgets::kv_line(&app.theme, "Service", &selected));
    lines.push(Line::from(""));

    if let Some(error) = &app.service_logs_error {
        lines.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(app.theme.status_error),
        )));
    } else if app.service_logs.is_empty() {
        lines.push(Line::from("No journal lines"));
    } else {
        for line in &app.service_logs {
            lines.push(Line::from(collectors::truncate(line, 70)));
        }
    }

    Paragraph::new(lines)
        .block(widgets::block(&app.theme, "Journalctl (last 8)"))
        .wrap(Wrap { trim: false })
}

fn service_failure_details(app: &App) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();

    if let Some(error) = &app.service_failure_error {
        lines.push(Line::from(Span::styled(
            collectors::truncate(error, 68),
            Style::default().fg(app.theme.status_error),
        )));
    } else if let Some(details) = &app.service_failure_details {
        lines.push(widgets::kv_line(
            &app.theme,
            "Result",
            if details.result.is_empty() {
                "unknown"
            } else {
                &details.result
            },
        ));
        lines.push(widgets::kv_line(
            &app.theme,
            "Exit",
            &format!(
                "{} {}",
                if details.exec_main_code.is_empty() {
                    "-"
                } else {
                    &details.exec_main_code
                },
                details
                    .exec_main_status
                    .map(|status| status.to_string())
                    .unwrap_or_else(|| "-".into())
            ),
        ));
        lines.push(widgets::kv_line(
            &app.theme,
            "State",
            &format!(
                "{} / {}",
                if details.active_state.is_empty() {
                    "-"
                } else {
                    &details.active_state
                },
                if details.sub_state.is_empty() {
                    "-"
                } else {
                    &details.sub_state
                }
            ),
        ));
        lines.push(widgets::kv_line(
            &app.theme,
            "UnitFile",
            if details.unit_file_state.is_empty() {
                "-"
            } else {
                &details.unit_file_state
            },
        ));
        lines.push(widgets::kv_line(
            &app.theme,
            "MainPID",
            &details
                .main_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "-".into()),
        ));
        lines.push(widgets::kv_line(
            &app.theme,
            "Tasks",
            &details
                .tasks_current
                .map(|tasks| tasks.to_string())
                .unwrap_or_else(|| "-".into()),
        ));
        lines.push(widgets::kv_line(
            &app.theme,
            "Memory",
            &details
                .memory_current
                .map(collectors::format_bytes)
                .unwrap_or_else(|| "-".into()),
        ));
        lines.push(widgets::kv_line(
            &app.theme,
            "Restarts",
            &details
                .n_restarts
                .map(|count| count.to_string())
                .unwrap_or_else(|| "-".into()),
        ));
        if !details.status_text.is_empty() {
            lines.push(widgets::kv_line(
                &app.theme,
                "Status",
                &collectors::truncate(&details.status_text, 40),
            ));
        }
        if !details.last_error.is_empty() {
            lines.push(Line::from(Span::styled(
                collectors::truncate(&details.last_error, 68),
                Style::default().fg(app.theme.status_error),
            )));
        }
    } else {
        lines.push(Line::from("No failure details available"));
    }

    Paragraph::new(lines).block(widgets::block(&app.theme, "Failure / Last Exit"))
}

fn service_row_style(app: &App, service: &ServiceRow) -> Style {
    if service.active == "failed" {
        Style::default().fg(app.theme.status_error)
    } else if service.active == "active" {
        Style::default().fg(app.theme.status_good)
    } else {
        Style::default().fg(app.theme.text_secondary)
    }
}
