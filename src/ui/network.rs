use crate::app::App;
use crate::collectors::{self, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, _snapshot: &Snapshot) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Min(12),
        ])
        .split(area);

    // -- Top: aggregate stats -----------------------------------------------
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(sections[0]);

    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "RX",
            &widgets::format_rate(app.total_rx_rate()),
            "aggregate inbound",
            app.theme.status_good,
            app.animation_frame,
        ),
        top[0],
    );
    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "TX",
            &widgets::format_rate(app.total_tx_rate()),
            "aggregate outbound",
            app.theme.status_info,
            app.animation_frame,
        ),
        top[1],
    );
    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Connections",
            &app.connections.len().to_string(),
            "ss-style snapshot",
            app.theme.brand,
            app.animation_frame,
        ),
        top[2],
    );

    // -- Interface table ----------------------------------------------------
    let mut iface_state = TableState::default();
    iface_state.select(Some(app.network_scroll));
    frame.render_stateful_widget(
        interface_table(
            app,
            app.network_scroll,
            widgets::visible_rows(sections[1], 4),
        ),
        sections[1],
        &mut iface_state,
    );
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(Style::default().fg(app.theme.brand))
        .track_style(Style::default().fg(app.theme.border));
    let mut scrollbar_state = ScrollbarState::new(app.interfaces.len())
        .position(app.network_scroll)
        .viewport_content_length(widgets::visible_rows(sections[1], 4));
    frame.render_stateful_widget(scrollbar, sections[1], &mut scrollbar_state);

    // -- Connection table ---------------------------------------------------
    let mut conn_state = TableState::default();
    conn_state.select(Some(app.connection_scroll));
    frame.render_stateful_widget(
        connection_table(
            app,
            app.connection_scroll,
            widgets::visible_rows(sections[2], 4),
        ),
        sections[2],
        &mut conn_state,
    );
    let conn_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(Style::default().fg(app.theme.brand))
        .track_style(Style::default().fg(app.theme.border));
    let mut conn_scrollbar_state = ScrollbarState::new(app.connections.len())
        .position(app.connection_scroll)
        .viewport_content_length(widgets::visible_rows(sections[2], 4));
    frame.render_stateful_widget(conn_scrollbar, sections[2], &mut conn_scrollbar_state);
}

// ---------------------------------------------------------------------------
// Network sub-widgets
// ---------------------------------------------------------------------------

fn interface_table(app: &App, offset: usize, height: usize) -> Table<'static> {
    let rows: Vec<Row> = app
        .interfaces
        .iter()
        .enumerate()
        .skip(offset.min(app.interfaces.len()))
        .take(height)
        .map(|(idx, iface)| {
            let row_style = if idx % 2 == 0 {
                Style::default()
            } else {
                Style::default().bg(app.theme.alt_row_bg)
            };
            Row::new(vec![
                Cell::from(iface.name.clone()).style(Style::default().fg(app.theme.brand)),
                Cell::from(collectors::truncate(&iface.addresses, 28)),
                Cell::from(widgets::format_rate(iface.rx_rate))
                    .style(Style::default().fg(app.theme.status_good)),
                Cell::from(widgets::format_rate(iface.tx_rate))
                    .style(Style::default().fg(app.theme.status_info)),
                Cell::from(collectors::format_bytes(iface.total_rx)),
                Cell::from(collectors::format_bytes(iface.total_tx)),
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
            Constraint::Length(12),
            Constraint::Percentage(38),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new(vec![
            "Iface",
            "Addresses",
            "RX/s",
            "TX/s",
            "RX total",
            "TX total",
        ])
        .style(
            Style::default()
                .fg(app.theme.active_tab)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .row_highlight_style(selected_style)
    .block(widgets::block(&app.theme, "Interfaces"))
}

fn connection_table(app: &App, offset: usize, height: usize) -> Table<'static> {
    let rows: Vec<Row> = app
        .connections
        .iter()
        .enumerate()
        .skip(offset.min(app.connections.len()))
        .take(height)
        .map(|(idx, conn)| {
            let state_color = match conn.state.as_str() {
                "ESTABLISHED" => app.theme.status_good,
                "LISTEN" => app.theme.status_info,
                "TIME_WAIT" => app.theme.status_warn,
                "CLOSE_WAIT" | "CLOSING" => app.theme.status_error,
                _ => app.theme.text_secondary,
            };
            let row_style = if idx % 2 == 0 {
                Style::default()
            } else {
                Style::default().bg(app.theme.alt_row_bg)
            };
            Row::new(vec![
                Cell::from(conn.proto.clone()),
                Cell::from(conn.state.clone()).style(Style::default().fg(state_color)),
                Cell::from(collectors::truncate(&conn.local, 24)),
                Cell::from(collectors::truncate(&conn.remote, 24)),
                Cell::from(collectors::truncate(&conn.process, 24))
                    .style(Style::default().fg(app.theme.text_muted)),
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
            Constraint::Length(13),
            Constraint::Percentage(27),
            Constraint::Percentage(27),
            Constraint::Percentage(26),
        ],
    )
    .header(
        Row::new(vec!["Proto", "State", "Local", "Remote", "Process"]).style(
            Style::default()
                .fg(app.theme.active_tab)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .row_highlight_style(selected_style)
    .block(widgets::block(&app.theme, "Active Connections"))
}
