use crate::app::App;
use crate::collectors::{self, Snapshot};
use crate::theme::Theme;
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, snapshot: &Snapshot) {
    let alerts = active_alerts(app, snapshot);
    let health_score = system_health_score(snapshot, alerts.len());

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(9),
            Constraint::Min(10),
        ])
        .split(area);

    // -- Top: metric cards with enhanced styling ----
    let metrics = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(17),
            Constraint::Percentage(17),
            Constraint::Percentage(16),
            Constraint::Percentage(16),
            Constraint::Percentage(17),
            Constraint::Percentage(17),
        ])
        .split(sections[0]);

    let mem_pct = collectors::percentage(snapshot.used_memory, snapshot.total_memory);

    // Animate metric cards with pulsing effect when critical (TODO: wire up to gauge)
    let _pulse_cpu = if app.animation_frame % 30 < 15 && snapshot.cpu_usage >= 85.0 {
        widgets::pulse_opacity(app.animation_frame)
    } else {
        1.0
    };

    frame.render_widget(
        widgets::gauge_card(
            &app.theme,
            "CPU",
            snapshot.cpu_usage as f64,
            &format!("{:.1}% | {} cores", snapshot.cpu_usage, snapshot.cpu_cores),
            widgets::status_color(&app.theme, snapshot.cpu_usage as f64),
            app.animation_frame,
        ),
        metrics[0],
    );

    frame.render_widget(
        widgets::gauge_card(
            &app.theme,
            "Memory",
            mem_pct,
            &format!(
                "{:.1}% | {} cached",
                mem_pct,
                collectors::format_bytes(snapshot.cached_memory)
            ),
            widgets::status_color(&app.theme, mem_pct),
            app.animation_frame,
        ),
        metrics[1],
    );

    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Network",
            &widgets::format_rate(app.total_rx_rate() + app.total_tx_rate()),
            &format!("{} ifaces", app.interfaces.len()),
            app.theme.brand,
            app.animation_frame,
        ),
        metrics[2],
    );

    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Disk",
            &worst_disk_usage(snapshot),
            "top mount pressure",
            disk_summary_color(&app.theme, snapshot),
            app.animation_frame,
        ),
        metrics[3],
    );

    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Alerts",
            &alerts.len().to_string(),
            if alerts.is_empty() {
                "all clear"
            } else {
                "attention needed"
            },
            if alerts.is_empty() {
                app.theme.status_good
            } else {
                app.theme.status_error
            },
            app.animation_frame,
        ),
        metrics[4],
    );
    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Health",
            &format!("{health_score}/100"),
            if health_score >= 80 {
                "healthy"
            } else if health_score >= 60 {
                "degraded"
            } else {
                "critical"
            },
            if health_score >= 80 {
                app.theme.status_good
            } else if health_score >= 60 {
                app.theme.status_warn
            } else {
                app.theme.status_error
            },
            app.animation_frame,
        ),
        metrics[5],
    );

    // -- Middle: sparklines + network chart with better spacing --------
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(sections[1]);

    frame.render_widget(
        widgets::spark_panel(
            &app.theme,
            "CPU Trend",
            &app.histories.cpu_total,
            app.theme.status_info,
            app.animation_frame,
        ),
        middle[0],
    );

    frame.render_widget(
        widgets::spark_panel(
            &app.theme,
            "Memory Trend",
            &app.histories.memory_used,
            app.theme.status_warn,
            app.animation_frame,
        ),
        middle[1],
    );

    frame.render_widget(network_chart(app), middle[2]);

    // -- Bottom: overview previews with enhanced layout --------
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(sections[2]);

    frame.render_widget(overview(app, snapshot, &alerts, health_score), bottom[0]);
    frame.render_widget(top_offenders(app, snapshot), bottom[1]);
    frame.render_widget(network_preview(app), bottom[2]);
}

// ---------------------------------------------------------------------------
// Dashboard sub-widgets
// ---------------------------------------------------------------------------

fn network_chart(app: &App) -> Chart<'_> {
    const HISTORY_LEN: usize = 60;

    let max_val = app
        .histories
        .network_chart_rx
        .iter()
        .chain(app.histories.network_chart_tx.iter())
        .map(|(_, v)| *v)
        .fold(1.0, f64::max);

    Chart::new(vec![
        Dataset::default()
            .data(&app.histories.network_chart_rx)
            .name("RX KB/s")
            .graph_type(GraphType::Line)
            .style(Style::default().fg(app.theme.chart_colors[0])),
        Dataset::default()
            .data(&app.histories.network_chart_tx)
            .name("TX KB/s")
            .graph_type(GraphType::Line)
            .style(Style::default().fg(app.theme.chart_colors[2])),
    ])
    .block(widgets::block(&app.theme, "Network Trend"))
    .x_axis(
        Axis::default()
            .bounds([0.0, HISTORY_LEN as f64])
            .labels(vec![Span::raw("60s ago"), Span::raw("now")])
            .style(Style::default().fg(app.theme.text_secondary)),
    )
    .y_axis(
        Axis::default()
            .title(Span::styled(
                "KB/s",
                Style::default().fg(app.theme.text_secondary),
            ))
            .bounds([0.0, max_val.max(100.0)])
            .style(Style::default().fg(app.theme.text_secondary)),
    )
}

fn overview(
    app: &App,
    snapshot: &Snapshot,
    alerts: &[String],
    health_score: i32,
) -> Paragraph<'static> {
    let mut lines = vec![
        widgets::kv_line(&app.theme, "Host", &snapshot.host),
        widgets::kv_line(&app.theme, "OS", &snapshot.os),
        widgets::kv_line(&app.theme, "Kernel", &snapshot.kernel),
        widgets::kv_line(
            &app.theme,
            "Uptime",
            &collectors::format_duration(snapshot.uptime),
        ),
        widgets::kv_line(&app.theme, "Load", &snapshot.load_average),
        widgets::kv_line(
            &app.theme,
            "Free Mem",
            &collectors::format_bytes(snapshot.available_memory),
        ),
        widgets::kv_line(&app.theme, "Health", &format!("{health_score}/100")),
    ];

    if let Some(first_alert) = alerts.first() {
        lines.push(Line::from(vec![
            Span::styled("Alert: ", Style::default().fg(app.theme.status_error)),
            Span::styled(
                collectors::truncate(first_alert, 28),
                Style::default().fg(app.theme.status_error),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Alert: ", Style::default().fg(app.theme.status_good)),
            Span::styled("All clear", Style::default().fg(app.theme.status_good)),
        ]));
    }

    Paragraph::new(lines).block(widgets::block(&app.theme, "Overview"))
}

fn top_offenders(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    let top_cpu = snapshot
        .processes
        .iter()
        .max_by(|a, b| a.cpu.total_cmp(&b.cpu));
    let top_mem = snapshot
        .processes
        .iter()
        .max_by_key(|process| process.memory);
    let top_disk = app
        .disk_io_rows
        .iter()
        .max_by_key(|row| row.read_bps + row.write_bps);
    let top_net = app
        .network_process_rows
        .iter()
        .max_by_key(|row| row.rx_bps + row.tx_bps);

    lines.push(widgets::kv_line(
        &app.theme,
        "CPU",
        &top_cpu
            .map(|process| {
                format!(
                    "{} {:.1}%",
                    collectors::truncate(&process.name, 16),
                    process.cpu
                )
            })
            .unwrap_or_else(|| "n/a".into()),
    ));
    lines.push(widgets::kv_line(
        &app.theme,
        "Memory",
        &top_mem
            .map(|process| {
                format!(
                    "{} {}",
                    collectors::truncate(&process.name, 14),
                    collectors::format_bytes(process.memory)
                )
            })
            .unwrap_or_else(|| "n/a".into()),
    ));
    lines.push(widgets::kv_line(
        &app.theme,
        "Disk I/O",
        &top_disk
            .map(|row| {
                format!(
                    "{} r:{} w:{}",
                    row.device,
                    widgets::format_rate(row.read_bps),
                    widgets::format_rate(row.write_bps)
                )
            })
            .unwrap_or_else(|| "n/a".into()),
    ));
    lines.push(widgets::kv_line(
        &app.theme,
        "Network",
        &top_net
            .map(|row| {
                format!(
                    "{} {}",
                    collectors::truncate(&row.process, 14),
                    widgets::format_rate(row.rx_bps + row.tx_bps)
                )
            })
            .unwrap_or_else(|| "n/a".into()),
    ));

    Paragraph::new(lines).block(widgets::block(&app.theme, "Top Offenders"))
}

fn network_preview(app: &App) -> Paragraph<'static> {
    let mut lines = Vec::new();
    for iface in app.interfaces.iter().take(4) {
        let status_color = if iface.rx_rate + iface.tx_rate > 1_000_000 {
            app.theme.status_warn
        } else {
            app.theme.status_good
        };

        lines.push(Line::from(vec![
            Span::styled(
                collectors::truncate(&iface.name, 10),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                " {} / {}",
                widgets::format_rate(iface.rx_rate),
                widgets::format_rate(iface.tx_rate)
            )),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from("No network data"));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "Connections: {}",
        app.connections.len()
    )));
    Paragraph::new(lines).block(widgets::block(&app.theme, "Network Preview"))
}

// ---------------------------------------------------------------------------
// Dashboard-specific helpers
// ---------------------------------------------------------------------------

fn worst_disk_usage(snapshot: &Snapshot) -> String {
    widgets::hottest_disks(snapshot, 1)
        .first()
        .map(|disk| format!("{:.1}%", disk.usage))
        .unwrap_or_else(|| "n/a".into())
}

fn disk_summary_color(theme: &Theme, snapshot: &Snapshot) -> Color {
    widgets::hottest_disks(snapshot, 1)
        .first()
        .map(|disk| widgets::status_color(theme, disk.usage))
        .unwrap_or(theme.text_secondary)
}

fn active_alerts(app: &App, snapshot: &Snapshot) -> Vec<String> {
    let mut alerts = Vec::new();
    let mem_pct = collectors::percentage(snapshot.used_memory, snapshot.total_memory);

    if snapshot.cpu_usage >= 85.0 {
        alerts.push(format!("High CPU {:.1}%", snapshot.cpu_usage));
    }
    if mem_pct >= 85.0 {
        alerts.push(format!("High memory {:.1}%", mem_pct));
    }
    if let Some(disk) = snapshot
        .disks
        .iter()
        .max_by(|a, b| a.usage.total_cmp(&b.usage))
        && disk.usage >= 90.0
    {
        alerts.push(format!("Disk {} at {:.1}%", disk.mount, disk.usage));
    }
    if let Some(summary) = snapshot.service_summary
        && summary.failed > 0
    {
        alerts.push(format!("{} failed services", summary.failed));
    }
    if log_error_spike(app) {
        alerts.push("Log error spike detected".into());
    }
    let suspicious_count = snapshot
        .processes
        .iter()
        .filter(|p| p.suspicious.is_some())
        .count();
    if suspicious_count > 0 {
        alerts.push(format!("{suspicious_count} suspicious process(es)"));
    }

    alerts
}


fn system_health_score(snapshot: &Snapshot, alert_count: usize) -> i32 {
    let mut score = 100.0;
    let mem_pct = collectors::percentage(snapshot.used_memory, snapshot.total_memory);
    let swap_pct = collectors::percentage(snapshot.used_swap, snapshot.total_swap);
    let worst_disk = snapshot
        .disks
        .iter()
        .map(|disk| disk.usage)
        .fold(0.0, f64::max);
    let suspicious_count = snapshot
        .processes
        .iter()
        .filter(|p| p.suspicious.is_some())
        .count();

    score -= (snapshot.cpu_usage as f64 * 0.25).min(25.0);
    score -= (mem_pct * 0.25).min(25.0);
    score -= (swap_pct * 0.15).min(15.0);
    score -= (worst_disk * 0.2).min(20.0);
    score -= (alert_count as f64 * 4.0).min(20.0);
    score -= (suspicious_count as f64 * 5.0).min(10.0);

    score.round().clamp(0.0, 100.0) as i32
}


fn log_error_spike(app: &App) -> bool {
    use crate::app::is_error_line;
    let lines: Vec<&String> = app
        .logs_journal
        .iter()
        .chain(app.logs_syslog.iter())
        .chain(app.logs_dmesg.iter())
        .collect();
    if lines.len() < 20 {
        return false;
    }

    let split = lines.len() / 2;
    let first_half_errors = lines[..split]
        .iter()
        .filter(|line| is_error_line(line))
        .count();
    let second_half_errors = lines[split..]
        .iter()
        .filter(|line| is_error_line(line))
        .count();

    second_half_errors >= 6 && second_half_errors >= first_half_errors.saturating_mul(2).max(3)
}

