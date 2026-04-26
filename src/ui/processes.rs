use crate::app::{App, ProcessViewRow, Tab};
use crate::collectors::{self, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{
        BarChart, Cell, List, ListItem, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState, Wrap,
    },
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, snapshot: &Snapshot) {
    // Rebuild process label cache early to avoid Box::leak
    // Collect names first, then drop the view rows reference
    let names: Vec<String> = {
        let rows = app.process_view_rows(snapshot);
        rows.iter()
            .take(5)
            .map(|row| collectors::truncate(&row.process.name, 10))
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
            Constraint::Length(6), // Stats
            Constraint::Min(6),    // Memory Chart
            Constraint::Min(10),   // Details
            Constraint::Min(8),    // Guidance
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

    // Safety: ratatui BarChart Horizontal panics if area.width < max_label_width + 1
    // We truncated labels to 10, so we need at least 12 width.
    if !process_memory_data.is_empty() && sidebar[1].height > 2 && sidebar[1].width > 12 {
        let process_memory_widget = BarChart::default()
            .block(widgets::block(&app.theme, "Top Memory (MB)"))
            .data(&process_memory_data)
            .bar_width(1)
            .bar_gap(0)
            .bar_style(Style::default().fg(app.theme.status_info))
            .value_style(
                Style::default()
                    .fg(app.theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            )
            .direction(Direction::Horizontal)
            .label_style(Style::default().fg(app.theme.text_secondary));
        frame.render_widget(process_memory_widget, sidebar[1]);
    }

    frame.render_widget(process_details_panel(app, snapshot), sidebar[2]);
    frame.render_widget(process_guidance(app, snapshot), sidebar[3]);
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
    let mut table_rows = Vec::new();
    let mut current_group = String::new();

    for (idx, row) in rows
        .iter()
        .enumerate()
        .skip(offset.min(rows.len()))
        .take(height)
    {
        let process = row.process;
        let process_group = app.process_group_label(process).to_string();

        if app.active_tab == Tab::Processes && current_group != process_group {
            if !current_group.is_empty() {
                table_rows.push(
                    Row::new(vec![
                        Cell::from(""),
                        Cell::from(format!("━ {}", process_group)),
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(""),
                    ])
                    .style(
                        Style::default()
                            .fg(app.theme.text_muted)
                            .add_modifier(Modifier::DIM),
                    ),
                );
            }
            current_group = process_group;
        }

        let cpu_color = widgets::status_color(&app.theme, process.cpu as f64);
        let mem_pct = (process.memory as f64 / (1024.0 * 1024.0 * 1024.0)).min(100.0);
        let mem_color = widgets::status_color(&app.theme, mem_pct);
        let row_style = if process.suspicious.is_some() {
            Style::default().bg(app.theme.highlight_bg)
        } else if idx % 2 == 0 {
            Style::default()
        } else {
            Style::default().bg(app.theme.alt_row_bg)
        };
        let base_name = if row.depth == 0 {
            collectors::truncate(&process.name, 26)
        } else {
            let prefix = format!("{}└ ", "  ".repeat(row.depth.min(6)));
            collectors::truncate(&(prefix + &process.name), 26)
        };
        let name = if process.suspicious.is_some() {
            format!("⚠ {base_name}")
        } else {
            base_name
        };
        let name_style = if process.suspicious.is_some() {
            Style::default()
                .fg(app.theme.status_error)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        table_rows.push(
            Row::new(vec![
                Cell::from(process.pid.clone()),
                Cell::from(name).style(name_style),
                Cell::from(format!("{:.1}", process.cpu)).style(Style::default().fg(cpu_color)),
                Cell::from(collectors::format_bytes(process.memory))
                    .style(Style::default().fg(mem_color)),
                Cell::from(collectors::truncate(app.process_group_label(process), 16)),
                Cell::from(process.status.clone()),
            ])
            .style(row_style),
        );
    }

    let selected_style = Style::default()
        .bg(app.theme.highlight_bg)
        .add_modifier(Modifier::BOLD);

    Table::new(
        table_rows,
        [
            Constraint::Length(8),
            Constraint::Percentage(36),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(18),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new(vec!["PID", "Name", "CPU%", "Memory", "Group", "Status"]).style(
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
    let suspicious_count = snapshot
        .processes
        .iter()
        .filter(|p| p.suspicious.is_some())
        .count();
    let suspicious_style = if suspicious_count > 0 {
        Style::default()
            .fg(app.theme.status_error)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.status_good)
    };
    let mut lines = vec![
        widgets::kv_line(&app.theme, "Loaded", &snapshot.processes.len().to_string()),
        widgets::kv_line(&app.theme, "Visible", &filtered_count.to_string()),
        widgets::kv_line(&app.theme, "Count", &snapshot.process_count.to_string()),
        widgets::kv_line(
            &app.theme,
            "Sort",
            widgets::process_sort_label(app.process_sort),
        ),
        widgets::kv_line(&app.theme, "View", app.process_view_label()),
    ];
    lines.push(Line::from(vec![
        Span::styled("Suspicious: ", Style::default().fg(app.theme.text_muted)),
        Span::styled(suspicious_count.to_string(), suspicious_style),
    ]));
    Paragraph::new(lines).block(widgets::block(&app.theme, "Summary"))
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
        ListItem::new("• `p` cycles view: flat → tree → user → service → container"),
        ListItem::new("• `x` term, `z` kill, `n` renice, `a` pin core"),
        ListItem::new("• `j/k` to scroll, `gg/G` top/bottom"),
    ];

    List::new(items)
        .block(widgets::block(&app.theme, "Status"))
        .highlight_style(Style::default().fg(app.theme.brand))
}

fn process_details_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    let selected = app
        .selected_process(snapshot)
        .map(|process| process.pid.clone())
        .unwrap_or_else(|| "none".into());
    lines.push(widgets::kv_line(&app.theme, "PID", &selected));

    // Show suspicious flag prominently if set.
    if let Some(reason) = app
        .selected_process(snapshot)
        .and_then(|p| p.suspicious.as_ref())
    {
        lines.push(Line::from(vec![
            Span::styled(
                "⚠ Suspicious: ",
                Style::default()
                    .fg(app.theme.status_error)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(reason.clone(), Style::default().fg(app.theme.status_error)),
        ]));
    }

    if let Some(cmdline) = &app.process_cmdline {
        lines.push(widgets::kv_line(
            &app.theme,
            "Cmdline",
            &collectors::truncate(cmdline, 46),
        ));
    }

    if let Some(history) = app.histories.process_cpu.get(&selected) {
        let sparkline_data: Vec<u64> = history.iter().copied().collect();
        let max_usage = sparkline_data.iter().copied().max().unwrap_or(0).max(1);
        let spark_str = if !sparkline_data.is_empty() {
            let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
            sparkline_data
                .iter()
                .map(|&val| {
                    let chars_count = chars.len();
                    if chars_count == 0 {
                        return ' ';
                    }
                    let denom = (max_usage as f32 + 1.0).max(1.0);
                    let idx = ((val as f32 / denom) * (chars_count - 1) as f32).round() as usize;
                    chars[idx.min(chars_count - 1)]
                })
                .collect::<String>()
        } else {
            "—".into()
        };
        lines.push(Line::from(vec![
            Span::styled("CPU hist: ", Style::default().fg(app.theme.text_muted)),
            Span::raw(spark_str),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Open files",
        Style::default()
            .fg(app.theme.brand)
            .add_modifier(Modifier::BOLD),
    )));
    if let Some(error) = &app.process_detail_error {
        lines.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(app.theme.status_error),
        )));
    } else {
        match &app.process_open_files {
            Ok(files) if files.is_empty() => {
                lines.push(Line::from("No open file entries"));
            }
            Ok(files) => {
                for row in files {
                    lines.push(Line::from(format!("• {}", collectors::truncate(row, 40))));
                }
            }
            Err(error) => {
                lines.push(Line::from(Span::styled(
                    error.clone(),
                    Style::default().fg(app.theme.status_error),
                )));
            }
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Open ports",
        Style::default()
            .fg(app.theme.brand)
            .add_modifier(Modifier::BOLD),
    )));
    if app.process_open_ports.is_empty() {
        lines.push(Line::from("No open ports for PID"));
    } else {
        for row in &app.process_open_ports {
            lines.push(Line::from(format!("• {}", collectors::truncate(row, 46))));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Environment",
        Style::default()
            .fg(app.theme.brand)
            .add_modifier(Modifier::BOLD),
    )));
    if app.process_environ.is_empty() {
        lines.push(Line::from("No environment variables"));
    } else {
        for var in app.process_environ.iter().take(2) {
            lines.push(Line::from(format!("• {}", collectors::truncate(var, 50))));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Maps",
        Style::default()
            .fg(app.theme.brand)
            .add_modifier(Modifier::BOLD),
    )));
    if app.process_maps.is_empty() {
        lines.push(Line::from("No memory mappings"));
    } else {
        for map in app.process_maps.iter().take(2) {
            lines.push(Line::from(format!("• {}", collectors::truncate(map, 50))));
        }
    }

    Paragraph::new(lines)
        .block(widgets::block(&app.theme, "PID Details"))
        .wrap(Wrap { trim: false })
}
