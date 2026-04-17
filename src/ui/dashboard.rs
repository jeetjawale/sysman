use crate::app::App;
use crate::collectors::{self, Snapshot};
use crate::theme::Theme;
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let snapshot = app.snapshot.as_ref().unwrap();

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(8),
            Constraint::Min(12),
        ])
        .split(area);

    // -- Top: metric cards ------------------------------------------------
    let metrics = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ])
        .split(sections[0]);

    let mem_pct = collectors::percentage(snapshot.used_memory, snapshot.total_memory);
    frame.render_widget(
        widgets::gauge_card(
            &app.theme,
            "CPU",
            snapshot.cpu_usage as f64,
            &format!("{:.1}% | {} cores", snapshot.cpu_usage, snapshot.cpu_cores),
            widgets::status_color(&app.theme, snapshot.cpu_usage as f64),
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
        ),
        metrics[3],
    );
    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Services",
            &service_summary_label(snapshot),
            "systemd overview",
            service_summary_color(&app.theme, snapshot),
        ),
        metrics[4],
    );

    // -- Middle: sparklines + network chart --------------------------------
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
        ),
        middle[0],
    );
    frame.render_widget(
        widgets::spark_panel(
            &app.theme,
            "Memory Trend",
            &app.histories.memory_used,
            app.theme.status_warn,
        ),
        middle[1],
    );
    frame.render_widget(network_chart(app), middle[2]);

    // -- Bottom: overview previews -----------------------------------------
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(sections[2]);

    frame.render_widget(overview(app, snapshot), bottom[0]);
    frame.render_widget(process_preview(app, snapshot), bottom[1]);
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

fn overview(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    Paragraph::new(vec![
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
    ])
    .block(widgets::block(&app.theme, "Overview"))
}

fn process_preview(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines = vec![widgets::kv_line(
        &app.theme,
        "Sort",
        widgets::process_sort_label(app.process_sort),
    )];
    for process in snapshot.processes.iter().take(5) {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>5.1}% ", process.cpu),
                Style::default().fg(app.theme.status_info),
            ),
            Span::raw(collectors::truncate(&process.name, 18)),
        ]));
    }
    Paragraph::new(lines).block(widgets::block(&app.theme, "Process Preview"))
}

fn network_preview(app: &App) -> Paragraph<'static> {
    let mut lines = Vec::new();
    for iface in app.interfaces.iter().take(4) {
        lines.push(Line::from(vec![
            Span::styled(
                collectors::truncate(&iface.name, 10),
                Style::default().fg(app.theme.brand),
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

fn service_summary_label(snapshot: &Snapshot) -> String {
    snapshot
        .service_summary
        .map(|summary| format!("{} up / {} fail", summary.running, summary.failed))
        .unwrap_or_else(|| "Unavailable".into())
}

fn service_summary_color(theme: &Theme, snapshot: &Snapshot) -> Color {
    match snapshot.service_summary {
        Some(summary) if summary.failed > 0 => theme.status_error,
        Some(_) => theme.status_good,
        None => theme.status_warn,
    }
}
