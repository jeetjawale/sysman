use crate::app::App;
use crate::collectors::{self, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{
        Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState,
        Wrap,
    },
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

    let suspicious_count = app
        .connections
        .iter()
        .filter(|conn| conn.suspicious.is_some())
        .count();

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
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
    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Suspicious",
            &suspicious_count.to_string(),
            "flagged remote flows",
            if suspicious_count > 0 {
                app.theme.status_error
            } else {
                app.theme.status_good
            },
            app.animation_frame,
        ),
        top[3],
    );

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

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
        .split(sections[2]);

    let filtered_connections = app.filtered_connections();
    let mut conn_state = TableState::default();
    conn_state.select(Some(app.connection_scroll));
    frame.render_stateful_widget(
        connection_table(
            app,
            &filtered_connections,
            app.connection_scroll,
            widgets::visible_rows(bottom[0], 4),
        ),
        bottom[0],
        &mut conn_state,
    );
    let conn_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(Style::default().fg(app.theme.brand))
        .track_style(Style::default().fg(app.theme.border));
    let mut conn_scrollbar_state = ScrollbarState::new(filtered_connections.len())
        .position(app.connection_scroll)
        .viewport_content_length(widgets::visible_rows(bottom[0], 4));
    frame.render_stateful_widget(conn_scrollbar, bottom[0], &mut conn_scrollbar_state);

    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(10),
        ])
        .split(bottom[1]);
    frame.render_widget(process_bandwidth_table(app), sidebar[0]);
    frame.render_widget(open_ports_panel(app), sidebar[1]);
    frame.render_widget(network_tools_panel(app), sidebar[2]);
}

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
                Cell::from(collectors::truncate(&iface.state, 10)),
                Cell::from(collectors::truncate(&iface.mac, 18)),
                Cell::from(iface.mtu.clone()),
                Cell::from(collectors::truncate(&iface.addresses, 24)),
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
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(18),
            Constraint::Length(6),
            Constraint::Percentage(32),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new(vec![
            "Iface",
            "State",
            "MAC",
            "MTU",
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

fn connection_table(
    app: &App,
    connections: &[&collectors::ConnectionRow],
    offset: usize,
    height: usize,
) -> Table<'static> {
    let rows: Vec<Row> = connections
        .iter()
        .enumerate()
        .skip(offset.min(connections.len()))
        .take(height)
        .map(|(idx, &conn)| {
            let state_color = match conn.state.as_str() {
                "ESTAB" | "ESTABLISHED" => app.theme.status_good,
                "LISTEN" => app.theme.status_info,
                "TIME_WAIT" => app.theme.status_warn,
                "CLOSE_WAIT" | "CLOSING" | "SYN-SENT" | "SYN-RECV" => app.theme.status_error,
                _ => app.theme.text_secondary,
            };
            let row_style = if idx % 2 == 0 {
                Style::default()
            } else {
                Style::default().bg(app.theme.alt_row_bg)
            };
            let flag = conn.suspicious.clone().unwrap_or_default();
            Row::new(vec![
                Cell::from(conn.proto.clone()),
                Cell::from(conn.state.clone()).style(Style::default().fg(state_color)),
                Cell::from(collectors::truncate(&conn.local, 20)),
                Cell::from(collectors::truncate(&conn.remote, 22)),
                Cell::from(
                    conn.pid
                        .map(|pid| pid.to_string())
                        .unwrap_or_else(|| "-".into()),
                ),
                Cell::from(collectors::truncate(&flag, 30)).style(Style::default().fg(
                    if conn.suspicious.is_some() {
                        app.theme.status_error
                    } else {
                        app.theme.text_muted
                    },
                )),
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
            Constraint::Length(11),
            Constraint::Percentage(24),
            Constraint::Percentage(26),
            Constraint::Length(8),
            Constraint::Percentage(24),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from("Proto"),
            Cell::from(format!("State[{}]", app.connection_state_filter_label())),
            Cell::from("Local"),
            Cell::from("Remote"),
            Cell::from("PID"),
            Cell::from("Flag"),
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
        "Active Connections (c filter, x kill, b block IP)",
        app.animation_frame,
    ))
}

fn process_bandwidth_table(app: &App) -> Table<'static> {
    let rows: Vec<Row> = app
        .network_process_rows
        .iter()
        .map(|row| {
            Row::new(vec![
                Cell::from(row.pid.to_string()),
                Cell::from(collectors::truncate(&row.process, 14)),
                Cell::from(widgets::format_rate(row.rx_bps))
                    .style(Style::default().fg(app.theme.status_good)),
                Cell::from(widgets::format_rate(row.tx_bps))
                    .style(Style::default().fg(app.theme.status_info)),
                Cell::from(row.connections.to_string()),
            ])
        })
        .collect();

    Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(16),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(6),
        ],
    )
    .header(
        Row::new(vec!["PID", "Process", "RX/s", "TX/s", "Conn"]).style(
            Style::default()
                .fg(app.theme.active_tab)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(widgets::block(&app.theme, "Per-Process Bandwidth"))
}

fn network_tools_panel(app: &App) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();

    if let Some(conn) = app.selected_connection() {
        lines.push(Line::from(format!(
            "Selected: {}",
            collectors::truncate(&conn.remote, 42)
        )));
        lines.push(Line::from(format!("Process: {}", conn.process_name)));
        lines.push(Line::from(format!("Remote IP: {}", conn.remote_ip)));
        if let Some(reason) = &conn.suspicious {
            lines.push(Line::from(Span::styled(
                format!("Flag: {reason}"),
                Style::default().fg(app.theme.status_error),
            )));
        }
    } else {
        lines.push(Line::from("No connection selected"));
    }
    lines.push(Line::from(""));

    if app.network_tool_output.is_empty() {
        lines.push(Line::from("Press `t` and enter host/IP"));
        lines.push(Line::from("Runs DNS, ping, traceroute, HTTP probe"));
    } else {
        for line in &app.network_tool_output {
            lines.push(Line::from(collectors::truncate(line, 64)));
        }
    }

    Paragraph::new(lines)
        .block(widgets::block(&app.theme, "DNS / Ping / Trace / HTTP"))
        .wrap(Wrap { trim: false })
}

fn open_ports_panel(app: &App) -> Table<'static> {
    let rows: Vec<Row> = app
        .connections
        .iter()
        .filter(|conn| conn.state == "LISTEN")
        .take(6)
        .map(|conn| {
            Row::new(vec![
                Cell::from(conn.proto.clone()),
                Cell::from(collectors::truncate(&conn.local, 24)),
                Cell::from(collectors::truncate(&conn.process_name, 14)),
            ])
        })
        .collect();

    Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(26),
            Constraint::Length(16),
        ],
    )
    .header(
        Row::new(vec!["Proto", "Local Listen", "Process"]).style(
            Style::default()
                .fg(app.theme.active_tab)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(widgets::block(&app.theme, "Open Ports"))
}
