use crate::app::App;
use crate::collectors::{self, Snapshot};
use crate::ui::widgets;
use ratatui::{prelude::*, widgets::Paragraph};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, snapshot: &Snapshot) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Min(10),
        ])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(sections[0]);

    let mem_pct = collectors::percentage(snapshot.used_memory, snapshot.total_memory);
    let swap_pct = collectors::percentage(snapshot.used_swap, snapshot.total_swap);
    frame.render_widget(
        widgets::gauge_card(
            &app.theme,
            "RAM",
            mem_pct,
            &format!(
                "{} / {}",
                collectors::format_bytes(snapshot.used_memory),
                collectors::format_bytes(snapshot.total_memory)
            ),
            widgets::status_color(&app.theme, mem_pct),
            app.animation_frame,
        ),
        top[0],
    );
    frame.render_widget(
        widgets::gauge_card(
            &app.theme,
            "Swap",
            swap_pct,
            &format!(
                "{} / {}",
                collectors::format_bytes(snapshot.used_swap),
                collectors::format_bytes(snapshot.total_swap)
            ),
            widgets::status_color(&app.theme, swap_pct),
            app.animation_frame,
        ),
        top[1],
    );
    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Cached",
            &collectors::format_bytes(snapshot.cached_memory),
            &format!(
                "avail {}",
                collectors::format_bytes(snapshot.available_memory)
            ),
            app.theme.status_info,
            app.animation_frame,
        ),
        top[2],
    );

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(sections[1]);
    frame.render_widget(
        widgets::spark_panel(
            &app.theme,
            "RAM Trend",
            &app.histories.memory_used,
            app.theme.status_warn,
            app.animation_frame,
        ),
        middle[0],
    );
    frame.render_widget(
        widgets::spark_panel(
            &app.theme,
            "Swap Trend",
            &app.histories.swap_used,
            app.theme.status_error,
            app.animation_frame,
        ),
        middle[1],
    );

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(sections[2]);
    frame.render_widget(top_memory_panel(app, snapshot), bottom[0]);
    frame.render_widget(memory_diagnostics_panel(app, snapshot), bottom[1]);
}

fn top_memory_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut procs = snapshot.processes.clone();
    procs.sort_by(|a, b| b.memory.cmp(&a.memory).then_with(|| a.name.cmp(&b.name)));

    let mut lines = vec![
        widgets::kv_line(&app.theme, "Pressure", &memory_pressure_label(snapshot)),
        Line::from(""),
    ];
    for process in procs.iter().take(10) {
        lines.push(Line::from(format!(
            "{:>8}  {:>8.1}%  {}",
            collectors::format_bytes(process.memory),
            process.cpu,
            collectors::truncate(&process.name, 28),
        )));
    }
    if procs.is_empty() {
        lines.push(Line::from("No process data"));
    }

    Paragraph::new(lines).block(widgets::block(&app.theme, "Top Memory Consumers"))
}

fn memory_diagnostics_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines = vec![];
    if let Some(pressure) = &snapshot.memory_runtime.pressure {
        let pressure_state = memory_psi_state(pressure.some_avg10.max(pressure.full_avg10));
        lines.push(Line::from(vec![
            Span::styled("PSI: ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                format!(
                    "{} (some10 {:.2} full10 {:.2})",
                    pressure_state, pressure.some_avg10, pressure.full_avg10
                ),
                Style::default().fg(memory_psi_color(
                    app,
                    pressure.some_avg10.max(pressure.full_avg10),
                )),
            ),
        ]));
        lines.push(Line::from(format!(
            "avg60 some {:.2} / full {:.2}",
            pressure.some_avg60, pressure.full_avg60
        )));
    } else {
        lines.push(Line::from("PSI: unavailable"));
    }
    lines.push(Line::from(""));

    if let Some(page_faults) = &snapshot.memory_runtime.page_faults {
        lines.push(widgets::kv_line(
            &app.theme,
            "Page Faults",
            &page_faults.minor.to_string(),
        ));
        lines.push(widgets::kv_line(
            &app.theme,
            "Major Faults",
            &page_faults.major.to_string(),
        ));
        lines.push(widgets::kv_line(
            &app.theme,
            "Fault Rate",
            &app.memory_page_fault_rate
                .map(|value| format!("{value}/s"))
                .unwrap_or_else(|| "n/a".into()),
        ));
        lines.push(widgets::kv_line(
            &app.theme,
            "Major Rate",
            &app.memory_major_fault_rate
                .map(|value| format!("{value}/s"))
                .unwrap_or_else(|| "n/a".into()),
        ));
    } else {
        lines.push(Line::from("Page faults: unavailable"));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Leak growth suspects",
        Style::default()
            .fg(app.theme.text_secondary)
            .add_modifier(Modifier::BOLD),
    )));
    if app.memory_leak_suspects.is_empty() {
        lines.push(Line::from("No sustained memory growth detected"));
    } else {
        for suspect in app.memory_leak_suspects.iter().take(5) {
            lines.push(Line::from(format!(
                "{} {} +{}/s ({} samples)",
                suspect.pid,
                collectors::truncate(&suspect.name, 14),
                collectors::format_bytes(suspect.growth_rate),
                suspect.streak
            )));
        }
    }

    Paragraph::new(lines).block(widgets::block(&app.theme, "Memory Diagnostics"))
}

fn memory_pressure_label(snapshot: &Snapshot) -> String {
    let mem_pct = collectors::percentage(snapshot.used_memory, snapshot.total_memory);
    if mem_pct >= 90.0 {
        "critical".into()
    } else if mem_pct >= 75.0 {
        "high".into()
    } else if mem_pct >= 60.0 {
        "medium".into()
    } else {
        "low".into()
    }
}

fn memory_psi_color(app: &App, value: f64) -> Color {
    if value >= 1.0 {
        app.theme.status_error
    } else if value >= 0.3 {
        app.theme.status_warn
    } else {
        app.theme.status_good
    }
}

fn memory_psi_state(value: f64) -> &'static str {
    if value >= 1.0 {
        "critical"
    } else if value >= 0.3 {
        "elevated"
    } else {
        "normal"
    }
}
