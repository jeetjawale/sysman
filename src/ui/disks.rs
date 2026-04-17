use crate::app::App;
use crate::collectors::{self, DiskRow, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{
        BarChart, Cell, List, ListItem, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState,
    },
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, _snapshot: &Snapshot) {
    // Rebuild disk label cache early to avoid Box::leak
    let labels: Vec<String> = {
        let snapshot = app.snapshot.as_ref().unwrap();
        snapshot.disks.iter().take(5).map(|d| {
            d.mount.rsplit('/').next().unwrap_or(&d.mount).to_string()
        }).collect()
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
        disk_table(app, &snapshot.disks, app.disk_scroll, widgets::visible_rows(sections[0], 4)),
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
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(6),
        ])
        .split(sections[1]);

    frame.render_widget(disk_stats(app, snapshot), sidebar[0]);
    frame.render_widget(disk_hotspots(app, snapshot), sidebar[1]);
    
    // Build disk usage barchart using cached labels
    let disk_data: Vec<(&str, u64)> = app.disk_chart_labels
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
    frame.render_widget(disk_widget, sidebar[2]);
    
    frame.render_widget(disk_guidance(app), sidebar[3]);
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
                Cell::from(collectors::truncate(&disk.mount, 26)),
                Cell::from(disk.filesystem.clone()),
                Cell::from(collectors::format_bytes(disk.used)),
                Cell::from(collectors::format_bytes(disk.total)),
                Cell::from(format!("{:.1}%", disk.usage))
                    .style(Style::default().fg(usage_color).add_modifier(Modifier::BOLD)),
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
            Constraint::Percentage(34),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["Mount", "FS", "Used", "Total", "Use%"]).style(
            Style::default()
                .fg(app.theme.active_tab)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .row_highlight_style(selected_style)
    .block(widgets::active_block(&app.theme, "Disk Usage", app.animation_frame))
}

fn disk_stats(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let hot = snapshot
        .disks
        .iter()
        .filter(|disk| disk.usage >= 80.0)
        .count();
    Paragraph::new(vec![
        widgets::kv_line(&app.theme, "Mounted", &snapshot.disks.len().to_string()),
        widgets::kv_line(&app.theme, "80%+", &hot.to_string()),
        widgets::kv_line(&app.theme, "Worst", &worst_disk_mount(snapshot)),
    ])
    .block(widgets::block(&app.theme, "Summary"))
}

fn disk_hotspots(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines = Vec::new();
    for disk in widgets::hottest_disks(snapshot, 5) {
        lines.push(Line::from(vec![
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
    let items = vec![
        ListItem::new(Span::styled(
            "Disk Health Tips",
            Style::default()
                .fg(app.theme.brand)
                .add_modifier(Modifier::BOLD),
        )),
        ListItem::new(""),
        ListItem::new("• Keep usage below 80%"),
        ListItem::new("• Monitor /home and / closely"),
        ListItem::new("• Use `df -h` for quick check"),
    ];

    List::new(items)
        .block(widgets::block(&app.theme, "Tips"))
        .highlight_style(Style::default().fg(app.theme.brand))
}

fn worst_disk_mount(snapshot: &Snapshot) -> String {
    widgets::hottest_disks(snapshot, 1)
        .first()
        .map(|disk| disk.mount.clone())
        .unwrap_or_else(|| "n/a".into())
}
