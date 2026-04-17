use crate::app::{App, ProcessViewRow};
use crate::collectors::{self, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{
        BarChart, Cell, List, ListItem, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState,
    },
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, snapshot: &Snapshot) {
    // Rebuild process label cache early to avoid Box::leak
    // Collect names first, then drop the view rows reference
    let names: Vec<String> = {
        let rows = app.process_view_rows(snapshot);
        rows.iter()
            .take(5)
            .map(|row| {
                let p = row.process;
                if p.name.len() > 10 {
                    p.name[..10].to_string()
                } else {
                    p.name.clone()
                }
            })
            .collect()
    };
    app.process_chart_labels.clear();
    app.process_chart_labels.extend(names);

    let view_rows = app.process_view_rows(snapshot);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(10)])
        .split(area);

    // -- Header: sort + filter info ----------------------------------------
    let header = Paragraph::new(vec![
        widgets::kv_line(
            &app.theme,
            "Sort",
            widgets::process_sort_label(app.process_sort),
        ),
        widgets::kv_line(
            &app.theme,
            "Filter",
            if app.process_filter.is_empty() {
                "none"
            } else {
                &app.process_filter
            },
        ),
        widgets::kv_line(&app.theme, "View", app.process_view_label()),
    ])
    .block(widgets::block(&app.theme, "Process Controls"));
    frame.render_widget(header, sections[0]);

    // -- Body: table + sidebar ---------------------------------------------
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .split(sections[1]);

    let mut table_state = TableState::default();
    table_state.select(Some(app.process_scroll));
    frame.render_stateful_widget(
        process_table(
            app,
            &view_rows,
            app.process_scroll,
            widgets::visible_rows(bottom[0], 4),
        ),
        bottom[0],
        &mut table_state,
    );

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(Style::default().fg(app.theme.brand))
        .track_style(Style::default().fg(app.theme.border));
    let mut scrollbar_state = ScrollbarState::new(view_rows.len())
        .position(app.process_scroll)
        .viewport_content_length(widgets::visible_rows(bottom[0], 4));
    frame.render_stateful_widget(scrollbar, bottom[0], &mut scrollbar_state);

    // -- Sidebar -----------------------------------------------------------
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Min(8),
        ])
        .split(bottom[1]);

    frame.render_widget(process_stats(app, snapshot, view_rows.len()), sidebar[0]);

    // Build process memory barchart using cached labels
    let process_memory_data: Vec<(&str, u64)> = app
        .process_chart_labels
        .iter()
        .zip(view_rows.iter().take(5))
        .map(|(label, row)| (label.as_str(), row.process.memory / (1024 * 1024)))
        .collect();
    let process_memory_widget = BarChart::default()
        .block(widgets::block(&app.theme, "Top Memory (MB)"))
        .data(&process_memory_data)
        .bar_width(8)
        .bar_gap(1)
        .bar_style(Style::default().fg(app.theme.status_info))
        .value_style(
            Style::default()
                .fg(app.theme.text_primary)
                .add_modifier(Modifier::BOLD),
        )
        .direction(Direction::Horizontal)
        .label_style(Style::default().fg(app.theme.text_secondary));
    frame.render_widget(process_memory_widget, sidebar[1]);

    frame.render_widget(process_guidance(app, snapshot), sidebar[2]);
}

// ---------------------------------------------------------------------------
// Process sub-widgets
// ---------------------------------------------------------------------------

fn process_table<'a>(
    app: &App,
    rows: &[ProcessViewRow<'a>],
    offset: usize,
    height: usize,
) -> Table<'a> {
    let rows: Vec<Row> = rows
        .iter()
        .enumerate()
        .skip(offset.min(rows.len()))
        .take(height)
        .map(|(idx, row)| {
            let process = row.process;
            let cpu_color = widgets::status_color(&app.theme, process.cpu as f64);
            let mem_pct = (process.memory as f64 / (1024.0 * 1024.0 * 1024.0)).min(100.0);
            let mem_color = widgets::status_color(&app.theme, mem_pct);
            let row_style = if idx % 2 == 0 {
                Style::default()
            } else {
                Style::default().bg(app.theme.alt_row_bg)
            };
            let name = if row.depth == 0 {
                collectors::truncate(&process.name, 28)
            } else {
                let prefix = format!("{}└ ", "  ".repeat(row.depth.min(6)));
                collectors::truncate(&(prefix + &process.name), 28)
            };
            Row::new(vec![
                Cell::from(process.pid.clone()),
                Cell::from(name),
                Cell::from(format!("{:.1}", process.cpu)).style(Style::default().fg(cpu_color)),
                Cell::from(collectors::format_bytes(process.memory))
                    .style(Style::default().fg(mem_color)),
                Cell::from(process.status.clone()),
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
            Constraint::Length(8),
            Constraint::Percentage(45),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new(vec!["PID", "Name", "CPU%", "Memory", "Status"]).style(
            Style::default()
                .fg(app.theme.active_tab)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .row_highlight_style(selected_style)
    .block(widgets::active_block(
        &app.theme,
        "Processes",
        app.animation_frame,
    ))
}

fn process_stats(app: &App, snapshot: &Snapshot, filtered_count: usize) -> Paragraph<'static> {
    Paragraph::new(vec![
        widgets::kv_line(&app.theme, "Loaded", &snapshot.processes.len().to_string()),
        widgets::kv_line(&app.theme, "Visible", &filtered_count.to_string()),
        widgets::kv_line(&app.theme, "Count", &snapshot.process_count.to_string()),
        widgets::kv_line(
            &app.theme,
            "Sort",
            widgets::process_sort_label(app.process_sort),
        ),
        widgets::kv_line(&app.theme, "View", app.process_view_label()),
    ])
    .block(widgets::block(&app.theme, "Summary"))
}

fn process_guidance<'a>(app: &'a App, snapshot: &'a Snapshot) -> List<'a> {
    let note = if snapshot.cpu_usage >= 90.0 {
        "⚠ CPU alert threshold exceeded"
    } else if collectors::percentage(snapshot.used_memory, snapshot.total_memory) >= 90.0 {
        "⚠ Memory alert threshold exceeded"
    } else {
        "✓ All metrics within normal range"
    };
    let note_color = widgets::status_color(
        &app.theme,
        (snapshot.cpu_usage as f64).max(collectors::percentage(
            snapshot.used_memory,
            snapshot.total_memory,
        )),
    );

    let items = vec![
        ListItem::new(Span::styled(note, Style::default().fg(note_color))),
        ListItem::new(""),
        ListItem::new("• `/` to filter by process name"),
        ListItem::new("• `s` cycles CPU → memory → pid → name"),
        ListItem::new("• `p` cycles view: flat → tree → user"),
        ListItem::new("• `x` term, `z` kill, `n` renice selected"),
        ListItem::new("• `j/k` to scroll, `gg/G` top/bottom"),
    ];

    List::new(items)
        .block(widgets::block(&app.theme, "Status"))
        .highlight_style(Style::default().fg(app.theme.brand))
}
