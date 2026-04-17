use crate::app::App;
use crate::collectors::{self, ProcessRow, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{
        BarChart, Cell, List, ListItem, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState,
    },
};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let snapshot = app.snapshot.as_ref().unwrap();

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(10)])
        .split(area);

    let filtered = app.filtered_processes(snapshot);

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
        process_table(app, &filtered, app.process_scroll, widgets::visible_rows(bottom[0], 4)),
        bottom[0],
        &mut table_state,
    );

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(Style::default().fg(app.theme.brand))
        .track_style(Style::default().fg(app.theme.border));
    let mut scrollbar_state = ScrollbarState::new(filtered.len())
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

    frame.render_widget(process_stats(app, snapshot, filtered.len()), sidebar[0]);
    frame.render_widget(process_memory_barchart(app, &filtered), sidebar[1]);
    frame.render_widget(process_guidance(app, snapshot), sidebar[2]);
}

// ---------------------------------------------------------------------------
// Process sub-widgets
// ---------------------------------------------------------------------------

fn process_table<'a>(
    app: &App,
    processes: &[&'a ProcessRow],
    offset: usize,
    height: usize,
) -> Table<'a> {
    let rows: Vec<Row> = processes
        .iter()
        .enumerate()
        .skip(offset.min(processes.len()))
        .take(height)
        .map(|(idx, process)| {
            let cpu_color = widgets::status_color(&app.theme, process.cpu as f64);
            let mem_pct = (process.memory as f64 / (1024.0 * 1024.0 * 1024.0)).min(100.0);
            let mem_color = widgets::status_color(&app.theme, mem_pct);
            let row_style = if idx % 2 == 0 {
                Style::default()
            } else {
                Style::default().bg(app.theme.alt_row_bg)
            };
            Row::new(vec![
                Cell::from(process.pid.clone()),
                Cell::from(collectors::truncate(&process.name, 28)),
                Cell::from(format!("{:.1}", process.cpu))
                    .style(Style::default().fg(cpu_color)),
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
    .block(widgets::active_block(&app.theme, "Processes"))
}

fn process_stats(app: &App, snapshot: &Snapshot, filtered_count: usize) -> Paragraph<'static> {
    Paragraph::new(vec![
        widgets::kv_line(&app.theme, "Loaded", &snapshot.processes.len().to_string()),
        widgets::kv_line(&app.theme, "Filtered", &filtered_count.to_string()),
        widgets::kv_line(&app.theme, "Count", &snapshot.process_count.to_string()),
        widgets::kv_line(
            &app.theme,
            "Sort",
            widgets::process_sort_label(app.process_sort),
        ),
    ])
    .block(widgets::block(&app.theme, "Summary"))
}

fn process_memory_barchart<'a>(app: &App, processes: &[&'a ProcessRow]) -> BarChart<'a> {
    let data: Vec<(&str, u64)> = processes
        .iter()
        .take(5)
        .map(|p| {
            let name = if p.name.len() > 10 {
                &p.name[..10]
            } else {
                &p.name
            };
            let name_ref: &'static str = Box::leak(name.to_string().into_boxed_str());
            (name_ref, p.memory / (1024 * 1024)) // MB
        })
        .collect();

    BarChart::default()
        .block(widgets::block(&app.theme, "Top Memory (MB)"))
        .data(&data)
        .bar_width(8)
        .bar_gap(1)
        .bar_style(Style::default().fg(app.theme.status_info))
        .value_style(
            Style::default()
                .fg(app.theme.text_primary)
                .add_modifier(Modifier::BOLD),
        )
        .direction(Direction::Horizontal)
        .label_style(Style::default().fg(app.theme.text_secondary))
}

fn process_guidance<'a>(app: &'a App, snapshot: &'a Snapshot) -> List<'a> {
    let note = if snapshot.cpu_usage >= 90.0 {
        "CPU alert threshold exceeded"
    } else if collectors::percentage(snapshot.used_memory, snapshot.total_memory) >= 90.0 {
        "Memory alert threshold exceeded"
    } else {
        "No active alert threshold"
    };
    let note_color = widgets::status_color(
        &app.theme,
        (snapshot.cpu_usage as f64)
            .max(collectors::percentage(snapshot.used_memory, snapshot.total_memory)),
    );

    let items = vec![
        ListItem::new(Span::styled(note, Style::default().fg(note_color))),
        ListItem::new(""),
        ListItem::new("`/` filters by process name"),
        ListItem::new("`s` cycles CPU → memory → name"),
        ListItem::new("Process actions are next phase"),
    ];

    List::new(items)
        .block(widgets::block(&app.theme, "Notes"))
        .highlight_style(Style::default().fg(app.theme.brand))
}
