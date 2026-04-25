use crate::app::App;
use crate::collectors::{self, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, snapshot: &Snapshot) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(10)])
        .split(area);

    // Summary header
    let count_str = snapshot.containers.len().to_string();
    let header = widgets::metric_card(
        &app.theme,
        "Containers",
        &count_str,
        "Docker / Podman active",
        app.theme.brand,
        app.animation_frame,
    );
    frame.render_widget(header, sections[0]);

    let mut state = TableState::default();
    state.select(Some(app.container_scroll));

    frame.render_stateful_widget(
        container_table(app, snapshot, app.container_scroll, widgets::visible_rows(sections[1], 4)),
        sections[1],
        &mut state,
    );

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(Style::default().fg(app.theme.brand))
        .track_style(Style::default().fg(app.theme.border));
    let mut scrollbar_state = ScrollbarState::new(snapshot.containers.len())
        .position(app.container_scroll)
        .viewport_content_length(widgets::visible_rows(sections[1], 4));
    frame.render_stateful_widget(scrollbar, sections[1], &mut scrollbar_state);
}

fn container_table<'a>(app: &App, snapshot: &'a Snapshot, offset: usize, height: usize) -> Table<'a> {
    let rows: Vec<Row> = snapshot.containers
        .iter()
        .enumerate()
        .skip(offset.min(snapshot.containers.len()))
        .take(height)
        .map(|(idx, c)| {
            let row_style = if idx % 2 == 0 {
                Style::default()
            } else {
                Style::default().bg(app.theme.alt_row_bg)
            };

            Row::new(vec![
                Cell::from(c.id.chars().take(12).collect::<String>()).style(Style::default().fg(app.theme.status_info)),
                Cell::from(collectors::truncate(&c.name, 20)).style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from(collectors::truncate(&c.image, 24)),
                Cell::from(c.status.clone()),
                Cell::from(c.cpu.clone()).style(Style::default().fg(app.theme.status_warn)),
                Cell::from(c.memory.clone()).style(Style::default().fg(app.theme.status_good)),
                Cell::from(c.net_io.clone()),
                Cell::from(c.block_io.clone()),
                Cell::from(c.pids.to_string()),
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
            Constraint::Length(14),
            Constraint::Length(22),
            Constraint::Length(26),
            Constraint::Length(16),
            Constraint::Length(8),
            Constraint::Length(16),
            Constraint::Length(18),
            Constraint::Length(18),
            Constraint::Length(6),
        ],
    )
    .header(
        Row::new(vec!["ID", "Name", "Image", "Status", "CPU%", "Mem", "Net I/O", "Block I/O", "PIDs"]).style(
            Style::default()
                .fg(app.theme.active_tab)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .row_highlight_style(selected_style)
    .block(widgets::active_block(
        &app.theme,
        "Containers (docker/podman stats)",
        app.animation_frame,
    ))
}
