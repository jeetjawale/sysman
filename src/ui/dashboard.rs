use crate::app::App;
use crate::collectors::{self, Snapshot};
use crate::theme::Theme;
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph, Wrap},
};
use std::collections::HashMap;

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, snapshot: &Snapshot) {
    let alerts = active_alerts(app, snapshot);
    let health_score = system_health_score(app, snapshot, alerts.len());

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
    frame.render_widget(top_offenders(app, snapshot, &app.theme), bottom[1]);
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
    alerts: &[(String, String)],
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

    if !alerts.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Smart Insight:",
            Style::default()
                .fg(app.theme.brand)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            app.explain_system_status(),
            Style::default()
                .fg(app.theme.text_secondary)
                .add_modifier(Modifier::ITALIC),
        )));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Alert: ", Style::default().fg(app.theme.status_good)),
            Span::styled("All clear", Style::default().fg(app.theme.status_good)),
        ]));
    }

    Paragraph::new(lines)
        .block(widgets::block(&app.theme, "Overview"))
        .wrap(Wrap { trim: true })
}

fn top_offenders(app: &App, snapshot: &Snapshot, theme: &Theme) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();

    let (cpu_offender, mem_offender) = collectors::procs::find_top_offenders(&snapshot.processes);

    if let Some(p) = cpu_offender {
        let reason = if p.cpu > 50.0 { " [SPIKE]" } else { "" };
        lines.push(Line::from(vec![
            Span::styled("CPU: ", Style::default().fg(theme.text_secondary)),
            Span::raw(format!(
                "{} {:.1}%",
                collectors::truncate(&p.name, 16),
                p.cpu
            )),
            Span::styled(
                reason,
                Style::default()
                    .fg(theme.status_error)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    if let Some(p) = mem_offender {
        let reason = if p.memory > 1024 * 1024 * 1024 {
            " [HEAVY]"
        } else {
            ""
        };
        lines.push(Line::from(vec![
            Span::styled("Mem: ", Style::default().fg(theme.text_secondary)),
            Span::raw(format!(
                "{} {}",
                collectors::truncate(&p.name, 16),
                collectors::format_bytes(p.memory)
            )),
            Span::styled(
                reason,
                Style::default()
                    .fg(theme.status_warn)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    let top_disk = app
        .disk_io_rows
        .iter()
        .max_by_key(|row| row.read_bps + row.write_bps);
    if let Some(row) = top_disk {
        lines.push(Line::from(vec![
            Span::styled("Disk: ", Style::default().fg(theme.text_secondary)),
            Span::raw(format!(
                "{} r:{} w:{}",
                row.device,
                widgets::format_rate(row.read_bps),
                widgets::format_rate(row.write_bps)
            )),
        ]));
    }

    let top_net = app
        .network_process_rows
        .iter()
        .max_by_key(|row| row.rx_bps + row.tx_bps);
    if let Some(row) = top_net {
        lines.push(Line::from(vec![
            Span::styled("Net: ", Style::default().fg(theme.text_secondary)),
            Span::raw(format!(
                "{} {}",
                collectors::truncate(&row.process, 16),
                widgets::format_rate(row.rx_bps + row.tx_bps)
            )),
        ]));
    }

    Paragraph::new(lines).block(widgets::block(theme, "Top Offenders"))
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

fn active_alerts(app: &App, snapshot: &Snapshot) -> Vec<(String, String)> {
    let mut alerts = Vec::new();
    let mem_pct = collectors::percentage(snapshot.used_memory, snapshot.total_memory);

    if snapshot.cpu_usage >= app.config.thresholds.cpu_high {
        alerts.push((
            "Critical CPU Usage".into(),
            format!(
                "{:.1}% load on {} cores",
                snapshot.cpu_usage, snapshot.cpu_cores
            ),
        ));
    }
    if mem_pct >= app.config.thresholds.mem_high as f64 {
        alerts.push((
            "Low System Memory".into(),
            format!("{:.1}% RAM utilized", mem_pct),
        ));
    }
    if let Some(disk) = snapshot
        .disks
        .iter()
        .max_by(|a, b| a.usage.total_cmp(&b.usage))
        && disk.usage >= app.config.thresholds.disk_high as f64
    {
        alerts.push((
            "Disk Space Low".into(),
            format!("{} is {:.1}% full", disk.mount, disk.usage),
        ));
    }
    if let Some(summary) = snapshot.service_summary
        && summary.failed > 0
    {
        alerts.push((
            "Failed Services".into(),
            format!("{} system units failed", summary.failed),
        ));
    }
    if let Some(explanation) = log_error_spike_explanation(app) {
        alerts.push(("Log Error Spike".into(), format!("Trend: {}", explanation)));
    }
    let suspicious_count = snapshot
        .processes
        .iter()
        .filter(|p| p.suspicious.is_some())
        .count();
    if suspicious_count > 0 {
        alerts.push((
            "Security Warning".into(),
            format!("{suspicious_count} process(es) with suspicious traits"),
        ));
    }

    alerts
}

fn log_error_spike_explanation(app: &App) -> Option<String> {
    use crate::app::is_error_line;
    let lines: Vec<&String> = app
        .logs_journal
        .iter()
        .chain(app.logs_syslog.iter())
        .chain(app.logs_dmesg.iter())
        .collect();
    if lines.len() < 20 {
        return None;
    }

    let split = lines.len() / 2;
    let second_half: Vec<_> = lines[split..]
        .iter()
        .filter(|line| is_error_line(line))
        .collect();

    if second_half.len() < 6 {
        return None;
    }

    // Heuristic: find the most common keyword in the spike
    let mut keywords = HashMap::new();
    for line in &second_half {
        for word in line.split_whitespace() {
            if word.len() > 4 && word.chars().all(|c| c.is_alphabetic()) {
                *keywords.entry(word.to_lowercase()).or_insert(0) += 1;
            }
        }
    }
    let top_word = keywords
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(word, _)| word);

    if let Some(word) = top_word {
        Some(format!("surge in errors containing '{}'", word))
    } else {
        Some("unusually high error frequency".into())
    }
}

fn system_health_score(app: &App, snapshot: &Snapshot, alert_count: usize) -> i32 {
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

    // Use config thresholds to weight penalties
    let cpu_weight = 25.0 / (app.config.thresholds.cpu_high as f64).max(1.0);
    let mem_weight = 25.0 / (app.config.thresholds.mem_high as f64).max(1.0);
    let disk_weight = 20.0 / (app.config.thresholds.disk_high as f64).max(1.0);

    score -= (snapshot.cpu_usage as f64 * cpu_weight).min(25.0);
    score -= (mem_pct * mem_weight).min(25.0);
    score -= (swap_pct * 0.15).min(15.0);
    score -= (worst_disk * disk_weight).min(20.0);
    score -= (alert_count as f64 * 4.0).min(20.0);
    score -= (suspicious_count as f64 * 5.0).min(10.0);

    score.round().clamp(0.0, 100.0) as i32
}
