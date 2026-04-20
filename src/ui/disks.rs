use crate::app::App;
use crate::collectors::{self, DiskRow, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{
        BarChart, Cell, List, ListItem, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState, Wrap,
    },
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, _snapshot: &Snapshot) {
    // Rebuild disk label cache early to avoid Box::leak
    let labels: Vec<String> = {
        let snapshot = app.snapshot.as_ref().unwrap();
        snapshot
            .disks
            .iter()
            .take(5)
            .map(|d| d.mount.rsplit('/').next().unwrap_or(&d.mount).to_string())
            .collect()
    };
    app.disk_chart_labels.clear();
    app.disk_chart_labels.extend(labels);

    let snapshot = app.snapshot.as_ref().unwrap();

    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(area);

    // -- Disk table ---------------------------------------------------------
    let mut disk_state = TableState::default();
    disk_state.select(Some(app.disk_scroll));
    frame.render_stateful_widget(
        disk_table(
            app,
            &snapshot.disks,
            app.disk_scroll,
            widgets::visible_rows(sections[0], 4),
        ),
        sections[0],
        &mut disk_state,
    );
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(Style::default().fg(app.theme.brand))
        .track_style(Style::default().fg(app.theme.border));
    let mut scrollbar_state = ScrollbarState::new(snapshot.disks.len())
        .position(app.disk_scroll)
        .viewport_content_length(widgets::visible_rows(sections[0], 4));
    frame.render_stateful_widget(scrollbar, sections[0], &mut scrollbar_state);

    // -- Sidebar ------------------------------------------------------------
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(7),
        ])
        .split(sections[1]);

    frame.render_widget(disk_stats(app, snapshot), sidebar[0]);
    frame.render_widget(disk_hotspots(app, snapshot), sidebar[1]);
    frame.render_widget(disk_io_panel(app), sidebar[2]);
    frame.render_widget(disk_smart_panel(app), sidebar[3]);
    frame.render_widget(disk_inode_panel(app, snapshot), sidebar[4]);

    // Build disk usage barchart using cached labels
    let disk_data: Vec<(&str, u64)> = app
        .disk_chart_labels
        .iter()
        .zip(snapshot.disks.iter().take(5))
        .map(|(label, d): (&String, &DiskRow)| (label.as_str(), d.usage as u64))
        .collect();
    let disk_widget = BarChart::default()
        .block(widgets::block(&app.theme, "Usage by Mount"))
        .data(&disk_data)
        .max(100)
        .bar_width(7)
        .bar_gap(1)
        .bar_style(Style::default().fg(app.theme.status_warn))
        .value_style(
            Style::default()
                .fg(app.theme.text_primary)
                .add_modifier(Modifier::BOLD),
        )
        .direction(Direction::Horizontal)
        .label_style(Style::default().fg(app.theme.text_secondary));
    frame.render_widget(disk_widget, sidebar[5]);
    frame.render_widget(disk_large_files(app), sidebar[6]);
    frame.render_widget(disk_guidance(app), sidebar[7]);
}

// ---------------------------------------------------------------------------
// Disk sub-widgets
// ---------------------------------------------------------------------------

fn disk_table<'a>(
    app: &'a App,
    disks: &'a [collectors::DiskRow],
    offset: usize,
    height: usize,
) -> Table<'a> {
    let rows: Vec<Row> = disks
        .iter()
        .enumerate()
        .skip(offset.min(disks.len()))
        .take(height)
        .map(|(idx, disk)| {
            let usage_color = widgets::status_color(&app.theme, disk.usage);
            let row_style = if idx % 2 == 0 {
                Style::default()
            } else {
                Style::default().bg(app.theme.alt_row_bg)
            };
            Row::new(vec![
                Cell::from(disk_alert_badge(disk).0)
                    .style(Style::default().fg(disk_alert_badge(disk).1)),
                Cell::from(collectors::truncate(&disk.mount, 26)),
                Cell::from(disk.filesystem.clone()),
                Cell::from(collectors::format_bytes(disk.used)),
                Cell::from(collectors::format_bytes(disk.total)),
                Cell::from(format!("{:.1}%", disk.usage)).style(
                    Style::default()
                        .fg(usage_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(
                    disk.inode_usage
                        .map(|usage| format!("{usage:.1}%"))
                        .unwrap_or_else(|| "-".into()),
                )
                .style(Style::default().fg(inode_usage_color(app, disk))),
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
            Constraint::Length(6),
            Constraint::Percentage(34),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(9),
        ],
    )
    .header(
        Row::new(vec![
            "Alert", "Mount", "FS", "Used", "Total", "Use%", "Inode%",
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
        "Disk Usage",
        app.animation_frame,
    ))
}

fn disk_stats(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let hot = snapshot
        .disks
        .iter()
        .filter(|disk| disk.usage >= 80.0)
        .count();
    let critical = snapshot
        .disks
        .iter()
        .filter(|disk| disk_is_critical(disk))
        .count();
    Paragraph::new(vec![
        widgets::kv_line(&app.theme, "Mounted", &snapshot.disks.len().to_string()),
        widgets::kv_line(&app.theme, "80%+", &hot.to_string()),
        widgets::kv_line(&app.theme, "Critical", &critical.to_string()),
        widgets::kv_line(&app.theme, "Worst", &worst_disk_mount(snapshot)),
    ])
    .block(widgets::block(&app.theme, "Summary"))
}

fn disk_hotspots(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines = Vec::new();
    for disk in widgets::hottest_disks(snapshot, 5) {
        lines.push(Line::from(vec![
            Span::styled(
                format!("[{}] ", disk_alert_badge(disk).0),
                Style::default().fg(disk_alert_badge(disk).1),
            ),
            Span::styled(
                format!("{:>5.1}% ", disk.usage),
                widgets::usage_style(&app.theme, disk.usage),
            ),
            Span::raw(collectors::truncate(&disk.mount, 20)),
        ]));
    }
    Paragraph::new(lines).block(widgets::block(&app.theme, "Hotspots"))
}

fn disk_guidance(app: &App) -> List<'_> {
    let mut items = vec![
        ListItem::new(Span::styled(
            "Directory Explorer",
            Style::default()
                .fg(app.theme.brand)
                .add_modifier(Modifier::BOLD),
        )),
        ListItem::new(""),
    ];
    if app.disk_scan_in_progress {
        let elapsed = app
            .disk_scan_started_at
            .map(|started| started.elapsed().as_secs())
            .unwrap_or(0);
        items.push(ListItem::new(format!(
            "Scan running: {} ({}s)",
            app.disk_scan_progress.as_deref().unwrap_or("working"),
            elapsed
        )));
        items.push(ListItem::new("Please wait for async worker"));
        return List::new(items)
            .block(widgets::block(&app.theme, "Explorer (ncdu-style)"))
            .highlight_style(Style::default().fg(app.theme.brand));
    }
    if let Some(target) = &app.dir_scan_target {
        items.push(ListItem::new(format!(
            "Mount: {target} (depth {})",
            app.dir_scan_depth
        )));
        for (path, size) in app.dir_scan_rows.iter().take(6) {
            let name = path.strip_prefix(target).unwrap_or(path);
            items.push(ListItem::new(format!(
                "• {} {}",
                collectors::truncate(name, 22),
                collectors::format_bytes(*size)
            )));
        }
        if app.dir_scan_rows.is_empty() {
            items.push(ListItem::new("No directory data"));
        }
    } else {
        items.push(ListItem::new("Press `f` to scan selected mount"));
        items.push(ListItem::new("Press `m` to cycle explorer depth"));
        items.push(ListItem::new("Depth cycles: 1 → 2 → 3"));
    }

    List::new(items)
        .block(widgets::block(&app.theme, "Explorer (ncdu-style)"))
        .highlight_style(Style::default().fg(app.theme.brand))
}

fn disk_io_panel(app: &App) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    for row in app.disk_io_rows.iter().take(3) {
        lines.push(Line::from(format!(
            "{} r:{} w:{}",
            row.device,
            widgets::format_rate(row.read_bps),
            widgets::format_rate(row.write_bps)
        )));
    }
    if lines.is_empty() {
        lines.push(Line::from("No disk I/O data"));
    }
    Paragraph::new(lines).block(widgets::block(&app.theme, "Disk I/O"))
}

fn disk_inode_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    for disk in snapshot
        .disks
        .iter()
        .filter(|disk| disk.inode_usage.is_some())
        .take(3)
    {
        if let (Some(usage), Some(used), Some(total)) =
            (disk.inode_usage, disk.inode_used, disk.inode_total)
        {
            lines.push(Line::from(vec![
                Span::styled(
                    collectors::truncate(&disk.mount, 12),
                    Style::default().fg(app.theme.brand),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{usage:>5.1}%"),
                    Style::default().fg(inode_usage_color(app, disk)),
                ),
            ]));
            lines.push(Line::from(format!("  {used}/{total} inodes")));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from("No inode data available"));
    }
    Paragraph::new(lines).block(widgets::block(&app.theme, "Inode Usage"))
}

fn disk_large_files(app: &App) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    for (path, size) in app.large_file_rows.iter().take(5) {
        lines.push(Line::from(format!(
            "{} {}",
            collectors::format_bytes(*size),
            collectors::truncate(path, 36)
        )));
    }
    if lines.is_empty() {
        lines.push(Line::from("Press `f` to scan large files"));
    }
    Paragraph::new(lines)
        .block(widgets::block(&app.theme, "Large Files"))
        .wrap(Wrap { trim: false })
}

fn disk_smart_panel(app: &App) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    for row in app.smart_health_rows.iter().take(3) {
        let status_color = if row.overall.to_ascii_lowercase().contains("pass")
            || row.overall.to_ascii_lowercase().contains("ok")
            || row.overall.to_ascii_lowercase().contains("available")
        {
            app.theme.status_good
        } else if row.overall == "unknown" {
            app.theme.status_warn
        } else {
            app.theme.status_error
        };
        lines.push(Line::from(vec![
            Span::styled(
                collectors::truncate(&row.device, 10),
                Style::default().fg(app.theme.brand),
            ),
            Span::raw(" "),
            Span::styled(
                collectors::truncate(&row.overall, 14),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        if let Some(temp) = row.temperature_c {
            lines.push(Line::from(format!("  temp {temp}C")));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from("smartctl data unavailable"));
    }
    Paragraph::new(lines).block(widgets::block(&app.theme, "S.M.A.R.T. Health"))
}

fn worst_disk_mount(snapshot: &Snapshot) -> String {
    widgets::hottest_disks(snapshot, 1)
        .first()
        .map(|disk| disk.mount.clone())
        .unwrap_or_else(|| "n/a".into())
}

fn disk_is_critical(disk: &DiskRow) -> bool {
    disk.usage >= 90.0 || disk.inode_usage.is_some_and(|usage| usage >= 90.0)
}

fn disk_alert_badge(disk: &DiskRow) -> (&'static str, Color) {
    if disk_is_critical(disk) {
        ("CRIT", Color::Red)
    } else if disk.usage >= 80.0 || disk.inode_usage.is_some_and(|usage| usage >= 80.0) {
        ("WARN", Color::Yellow)
    } else {
        ("OK", Color::Green)
    }
}

fn inode_usage_color(app: &App, disk: &DiskRow) -> Color {
    let usage = disk.inode_usage.unwrap_or(0.0);
    widgets::status_color(&app.theme, usage)
}
