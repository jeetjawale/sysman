use crate::cli::{ProcessSort, ServiceState};
use crate::commands::{self, ConnectionRow, DiskRow, ProcessRow, ServiceRow, Snapshot};
use crate::theme::{Theme, default_theme};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{
        Axis, BarChart, Block, Borders, Cell, Chart, Clear, Dataset,
        GraphType, Gauge, LineGauge, List, ListItem, ListState,
        Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Sparkline, Table, TableState, Tabs, Wrap,
    },
};
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::time::{Duration, Instant};
use sysinfo::Networks;

const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const TICK_RATE: Duration = Duration::from_millis(200);
const HISTORY_CAPACITY: usize = 60;

pub fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();
    app.refresh();

    loop {
        terminal.draw(|frame| app.draw(frame))?;

        if event::poll(TICK_RATE)? {
            if let Event::Key(key) = event::read()? {
                if app.handle_key(key) {
                    break;
                }
            }
        }

        if app.last_refresh.elapsed() >= REFRESH_INTERVAL {
            app.refresh();
        }
    }

    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dashboard,
    System,
    Processes,
    Network,
    Disks,
    Services,
    Help,
}

#[derive(Default)]
struct HistoryStore {
    cpu_total: VecDeque<u64>,
    memory_used: VecDeque<u64>,
    swap_used: VecDeque<u64>,
    network_rx: VecDeque<u64>,
    network_tx: VecDeque<u64>,
    per_core: Vec<VecDeque<u64>>,
    // Chart data (owned for Chart widget)
    network_chart_rx: Vec<(f64, f64)>,
    network_chart_tx: Vec<(f64, f64)>,
}

#[derive(Clone)]
struct NetworkInterfaceView {
    name: String,
    addresses: String,
    rx_rate: u64,
    tx_rate: u64,
    total_rx: u64,
    total_tx: u64,
}

struct App {
    active_tab: Tab,
    snapshot: Option<Snapshot>,
    service_error: Option<String>,
    status_line: String,
    last_refresh: Instant,
    process_scroll: usize,
    network_scroll: usize,
    connection_scroll: usize,
    disk_scroll: usize,
    service_scroll: usize,
    process_sort: ProcessSort,
    process_filter: String,
    filter_input: bool,
    pending_g: bool,
    theme: Theme,
    histories: HistoryStore,
    networks: Networks,
    interfaces: Vec<NetworkInterfaceView>,
    connections: Vec<ConnectionRow>,
    // Stateful widget states
    process_table_state: TableState,
    interface_table_state: TableState,
    connection_table_state: TableState,
    disk_table_state: TableState,
    service_table_state: TableState,
    process_list_state: ListState,
    disk_list_state: ListState,
    service_list_state: ListState,
}

impl App {
    fn new() -> Self {
        Self {
            active_tab: Tab::Dashboard,
            snapshot: None,
            service_error: None,
            status_line: "Loading system snapshot...".into(),
            last_refresh: Instant::now() - REFRESH_INTERVAL,
            process_scroll: 0,
            network_scroll: 0,
            connection_scroll: 0,
            disk_scroll: 0,
            service_scroll: 0,
            process_sort: ProcessSort::Cpu,
            process_filter: String::new(),
            filter_input: false,
            pending_g: false,
            theme: default_theme(),
            histories: HistoryStore::default(),
            networks: Networks::new_with_refreshed_list(),
            interfaces: Vec::new(),
            connections: Vec::new(),
            // Stateful widget states
            process_table_state: TableState::default(),
            interface_table_state: TableState::default(),
            connection_table_state: TableState::default(),
            disk_table_state: TableState::default(),
            service_table_state: TableState::default(),
            process_list_state: ListState::default(),
            disk_list_state: ListState::default(),
            service_list_state: ListState::default(),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.filter_input {
            match key.code {
                KeyCode::Esc => self.filter_input = false,
                KeyCode::Enter => self.filter_input = false,
                KeyCode::Backspace => {
                    self.process_filter.pop();
                }
                KeyCode::Char(ch) => self.process_filter.push(ch),
                _ => {}
            }
            return false;
        }

        let quit = match key.code {
            KeyCode::Char('q') => true,
            KeyCode::Left | KeyCode::Char('h') => {
                self.previous_tab();
                false
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.next_tab();
                false
            }
            KeyCode::Char('1') => {
                self.active_tab = Tab::Dashboard;
                false
            }
            KeyCode::Char('2') => {
                self.active_tab = Tab::System;
                false
            }
            KeyCode::Char('3') => {
                self.active_tab = Tab::Processes;
                false
            }
            KeyCode::Char('4') => {
                self.active_tab = Tab::Network;
                false
            }
            KeyCode::Char('5') => {
                self.active_tab = Tab::Disks;
                false
            }
            KeyCode::Char('6') => {
                self.active_tab = Tab::Services;
                false
            }
            KeyCode::Char('7') => {
                self.active_tab = Tab::Help;
                false
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down();
                false
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up();
                false
            }
            KeyCode::Char('/') => {
                if self.active_tab == Tab::Processes {
                    self.filter_input = true;
                }
                false
            }
            KeyCode::Esc => {
                self.process_filter.clear();
                false
            }
            KeyCode::Char('s') => {
                if self.active_tab == Tab::Processes {
                    self.cycle_process_sort();
                    self.refresh();
                }
                false
            }
            KeyCode::Char('r') => {
                self.refresh();
                false
            }
            KeyCode::Char('g') => {
                if self.pending_g {
                    self.scroll_top();
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
                false
            }
            KeyCode::Char('G') => {
                self.scroll_bottom();
                self.pending_g = false;
                false
            }
            _ => {
                self.pending_g = false;
                false
            }
        };

        if !matches!(key.code, KeyCode::Char('g')) {
            self.pending_g = false;
        }

        quit
    }

    fn refresh(&mut self) {
        let elapsed = self.last_refresh.elapsed().as_secs_f64().max(1.0);

        match commands::collect_snapshot(ServiceState::Running, 200) {
            Ok(mut snapshot) => {
                snapshot.processes = commands::collect_processes(200, self.process_sort);
                self.service_error = if snapshot.services.is_empty() && cfg!(target_os = "linux") {
                    Some("Service data unavailable in the current environment".into())
                } else {
                    None
                };

                self.networks.refresh(true);
                let interface_addresses = commands::collect_interface_addresses();
                self.interfaces = self.collect_interface_views(&interface_addresses, elapsed);
                self.connections = commands::collect_connections(200);
                self.push_histories(&snapshot);

                self.status_line = format!(
                    "{} | CPU {:.1}% | Mem {:.1}% | RX {} / TX {}",
                    snapshot.host,
                    snapshot.cpu_usage,
                    percentage(snapshot.used_memory, snapshot.total_memory),
                    format_rate(self.total_rx_rate()),
                    format_rate(self.total_tx_rate()),
                );
                self.snapshot = Some(snapshot);
            }
            Err(error) => self.status_line = format!("Refresh failed: {error}"),
        }

        self.last_refresh = Instant::now();
    }

    fn collect_interface_views(
        &self,
        addresses: &BTreeMap<String, Vec<String>>,
        elapsed: f64,
    ) -> Vec<NetworkInterfaceView> {
        let mut interfaces: Vec<_> = self
            .networks
            .iter()
            .map(|(name, data)| NetworkInterfaceView {
                name: name.clone(),
                addresses: addresses
                    .get(name)
                    .map(|list| list.join(", "))
                    .unwrap_or_else(|| "-".into()),
                rx_rate: (data.received() as f64 / elapsed) as u64,
                tx_rate: (data.transmitted() as f64 / elapsed) as u64,
                total_rx: data.total_received(),
                total_tx: data.total_transmitted(),
            })
            .collect();

        interfaces.sort_by(|a, b| {
            (b.rx_rate + b.tx_rate)
                .cmp(&(a.rx_rate + a.tx_rate))
                .then_with(|| a.name.cmp(&b.name))
        });
        interfaces
    }

    fn push_histories(&mut self, snapshot: &Snapshot) {
        let total_rx_kb = self.total_rx_rate() / 1024;
        let total_tx_kb = self.total_tx_rate() / 1024;
        push_history_value(
            &mut self.histories.cpu_total,
            snapshot.cpu_usage.round() as u64,
        );
        push_history_value(
            &mut self.histories.memory_used,
            percentage(snapshot.used_memory, snapshot.total_memory).round() as u64,
        );
        push_history_value(
            &mut self.histories.swap_used,
            percentage(snapshot.used_swap, snapshot.total_swap).round() as u64,
        );
        push_history_value(&mut self.histories.network_rx, total_rx_kb);
        push_history_value(&mut self.histories.network_tx, total_tx_kb);

        // Update chart data (owned vectors for Chart widget)
        self.histories.network_chart_rx = self
            .histories
            .network_rx
            .iter()
            .enumerate()
            .map(|(i, v)| (i as f64, *v as f64))
            .collect();
        self.histories.network_chart_tx = self
            .histories
            .network_tx
            .iter()
            .enumerate()
            .map(|(i, v)| (i as f64, *v as f64))
            .collect();

        if self.histories.per_core.len() < snapshot.cpu_per_core.len() {
            self.histories
                .per_core
                .resize_with(snapshot.cpu_per_core.len(), VecDeque::new);
        }
        for (index, usage) in snapshot.cpu_per_core.iter().enumerate() {
            push_history_value(&mut self.histories.per_core[index], usage.round() as u64);
        }
    }

    fn cycle_process_sort(&mut self) {
        self.process_sort = match self.process_sort {
            ProcessSort::Cpu => ProcessSort::Memory,
            ProcessSort::Memory => ProcessSort::Name,
            ProcessSort::Name => ProcessSort::Cpu,
        };
    }

    fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            Tab::Dashboard => Tab::System,
            Tab::System => Tab::Processes,
            Tab::Processes => Tab::Network,
            Tab::Network => Tab::Disks,
            Tab::Disks => Tab::Services,
            Tab::Services => Tab::Help,
            Tab::Help => Tab::Dashboard,
        };
    }

    fn previous_tab(&mut self) {
        self.active_tab = match self.active_tab {
            Tab::Dashboard => Tab::Help,
            Tab::System => Tab::Dashboard,
            Tab::Processes => Tab::System,
            Tab::Network => Tab::Processes,
            Tab::Disks => Tab::Network,
            Tab::Services => Tab::Disks,
            Tab::Help => Tab::Services,
        };
    }

    fn scroll_down(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = self.process_scroll.saturating_add(1),
            Tab::Network => self.connection_scroll = self.connection_scroll.saturating_add(1),
            Tab::Disks => self.disk_scroll = self.disk_scroll.saturating_add(1),
            Tab::Services => self.service_scroll = self.service_scroll.saturating_add(1),
            _ => {}
        }
    }

    fn scroll_up(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = self.process_scroll.saturating_sub(1),
            Tab::Network => self.connection_scroll = self.connection_scroll.saturating_sub(1),
            Tab::Disks => self.disk_scroll = self.disk_scroll.saturating_sub(1),
            Tab::Services => self.service_scroll = self.service_scroll.saturating_sub(1),
            _ => {}
        }
    }

    fn scroll_top(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = 0,
            Tab::Network => self.connection_scroll = 0,
            Tab::Disks => self.disk_scroll = 0,
            Tab::Services => self.service_scroll = 0,
            _ => {}
        }
    }

    fn scroll_bottom(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = usize::MAX / 4,
            Tab::Network => self.connection_scroll = usize::MAX / 4,
            Tab::Disks => self.disk_scroll = usize::MAX / 4,
            Tab::Services => self.service_scroll = usize::MAX / 4,
            _ => {}
        }
    }

    fn total_rx_rate(&self) -> u64 {
        self.interfaces.iter().map(|iface| iface.rx_rate).sum()
    }

    fn total_tx_rate(&self) -> u64 {
        self.interfaces.iter().map(|iface| iface.tx_rate).sum()
    }

    fn filtered_processes<'a>(&'a self, snapshot: &'a Snapshot) -> Vec<&'a ProcessRow> {
        if self.process_filter.trim().is_empty() {
            return snapshot.processes.iter().collect();
        }

        let needle = self.process_filter.to_lowercase();
        snapshot
            .processes
            .iter()
            .filter(|process| process.name.to_lowercase().contains(&needle))
            .collect()
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(2),
            ])
            .split(area);

        frame.render_widget(self.tabs(), layout[0]);

        if let Some(snapshot) = &self.snapshot {
            match self.active_tab {
                Tab::Dashboard => self.draw_dashboard(frame, layout[1], snapshot),
                Tab::System => self.draw_system(frame, layout[1], snapshot),
                Tab::Processes => self.draw_processes(frame, layout[1], snapshot),
                Tab::Network => self.draw_network(frame, layout[1], snapshot),
                Tab::Disks => self.draw_disks(frame, layout[1], snapshot),
                Tab::Services => self.draw_services(frame, layout[1], snapshot),
                Tab::Help => self.draw_help(frame, layout[1]),
            }
        } else {
            frame.render_widget(
                Paragraph::new("No data loaded yet")
                    .block(self.block("Sysman"))
                    .style(Style::default().fg(self.theme.text_secondary)),
                layout[1],
            );
        }

        let footer = if self.filter_input {
            format!(
                "/ filter: {}{}",
                self.process_filter,
                if self.active_tab == Tab::Processes {
                    "  |  Enter apply  Esc cancel"
                } else {
                    ""
                }
            )
        } else {
            format!(
                "{}   |   1-7 tabs  h/l switch  j/k scroll  / filter  s sort  gg/G  r refresh  q quit",
                self.status_line
            )
        };
        frame.render_widget(
            Paragraph::new(footer).style(Style::default().fg(self.theme.text_secondary)),
            layout[2],
        );
    }

    fn tabs(&self) -> Tabs<'static> {
        let titles = vec![
            "1 Dashboard",
            "2 System",
            "3 Processes",
            "4 Network",
            "5 Disks",
            "6 Services",
            "7 Help",
        ];
        let selected = match self.active_tab {
            Tab::Dashboard => 0,
            Tab::System => 1,
            Tab::Processes => 2,
            Tab::Network => 3,
            Tab::Disks => 4,
            Tab::Services => 5,
            Tab::Help => 6,
        };

        Tabs::new(titles)
            .select(selected)
            .block(
                Block::default()
                    .title(Span::styled(
                        " Sysman ",
                        Style::default()
                            .fg(self.theme.brand)
                            .add_modifier(Modifier::BOLD),
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.theme.border)),
            )
            .highlight_style(
                Style::default()
                    .fg(self.theme.active_tab)
                    .bg(self.theme.selection_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().fg(self.theme.inactive_tab))
    }

    fn draw_dashboard(&self, frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),
                Constraint::Length(8),
                Constraint::Min(12),
            ])
            .split(area);

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

        frame.render_widget(
            self.metric_card(
                "CPU",
                &format!("{:.1}%", snapshot.cpu_usage),
                &format!("{} cores", snapshot.cpu_cores),
                self.theme.status_info,
            ),
            metrics[0],
        );
        frame.render_widget(
            self.metric_card(
                "Memory",
                &format!(
                    "{:.1}%",
                    percentage(snapshot.used_memory, snapshot.total_memory)
                ),
                &format!("{} cached", commands::format_bytes(snapshot.cached_memory)),
                self.theme.status_warn,
            ),
            metrics[1],
        );
        frame.render_widget(
            self.metric_card(
                "Network",
                &format_rate(self.total_rx_rate() + self.total_tx_rate()),
                &format!("{} ifaces", self.interfaces.len()),
                self.theme.brand,
            ),
            metrics[2],
        );
        frame.render_widget(
            self.metric_card(
                "Disk",
                &worst_disk_usage(snapshot),
                "top mount pressure",
                disk_summary_color(&self.theme, snapshot),
            ),
            metrics[3],
        );
        frame.render_widget(
            self.metric_card(
                "Services",
                &service_summary_label(snapshot),
                "systemd overview",
                service_summary_color(&self.theme, snapshot),
            ),
            metrics[4],
        );

        let middle = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .split(sections[1]);
        frame.render_widget(
            self.spark_panel(
                "CPU Trend",
                &self.histories.cpu_total,
                self.theme.status_info,
            ),
            middle[0],
        );
        frame.render_widget(
            self.spark_panel(
                "Memory Trend",
                &self.histories.memory_used,
                self.theme.status_warn,
            ),
            middle[1],
        );
        frame.render_widget(self.network_chart(), middle[2]);

        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(sections[2]);

        frame.render_widget(self.dashboard_overview(snapshot), bottom[0]);
        frame.render_widget(self.dashboard_process_preview(snapshot), bottom[1]);
        frame.render_widget(self.dashboard_network_preview(), bottom[2]);
    }

    fn draw_system(&self, frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Length(9),
                Constraint::Min(12),
            ])
            .split(area);

        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(sections[0]);
        frame.render_widget(self.system_identity(snapshot), top[0]);
        frame.render_widget(self.system_runtime(snapshot), top[1]);

        let middle = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(sections[1]);
        frame.render_widget(
            self.line_gauge_panel(
                "CPU",
                snapshot.cpu_usage as f64 / 100.0,
                self.theme.status_info,
                &format!("{:.1}% total", snapshot.cpu_usage),
            ),
            middle[0],
        );
        frame.render_widget(
            self.line_gauge_panel(
                "Memory",
                percentage(snapshot.used_memory, snapshot.total_memory) / 100.0,
                self.theme.status_warn,
                &format!(
                    "{} / {}",
                    commands::format_bytes(snapshot.used_memory),
                    commands::format_bytes(snapshot.total_memory)
                ),
            ),
            middle[1],
        );
        frame.render_widget(
            self.line_gauge_panel(
                "Swap",
                percentage(snapshot.used_swap, snapshot.total_swap) / 100.0,
                self.theme.status_error,
                &format!(
                    "{} / {}",
                    commands::format_bytes(snapshot.used_swap),
                    commands::format_bytes(snapshot.total_swap)
                ),
            ),
            middle[2],
        );

        self.render_per_core_grid(frame, sections[2], snapshot);
    }

    fn draw_processes(&self, frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(10)])
            .split(area);

        let filtered = self.filtered_processes(snapshot);
        let header = Paragraph::new(vec![
            kv_line(&self.theme, "Sort", process_sort_label(self.process_sort)),
            kv_line(
                &self.theme,
                "Filter",
                if self.process_filter.is_empty() {
                    "none"
                } else {
                    &self.process_filter
                },
            ),
        ])
        .block(self.block("Process Controls"));
        frame.render_widget(header, sections[0]);

        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
            .split(sections[1]);

        let mut table_state = TableState::default();
        table_state.select(Some(self.process_scroll));
        frame.render_stateful_widget(
            self.process_table(&filtered, self.process_scroll, visible_rows(bottom[0], 4)),
            bottom[0],
            &mut table_state,
        );

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(self.theme.brand))
            .track_style(Style::default().fg(self.theme.border));
        let mut scrollbar_state = ScrollbarState::new(filtered.len())
            .position(self.process_scroll)
            .viewport_content_length(visible_rows(bottom[0], 4));
        frame.render_stateful_widget(scrollbar, bottom[0], &mut scrollbar_state);

        let sidebar = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Length(7),
                Constraint::Min(8),
            ])
            .split(bottom[1]);
        frame.render_widget(self.process_stats(snapshot, filtered.len()), sidebar[0]);
        frame.render_widget(self.process_memory_barchart(&filtered), sidebar[1]);
        frame.render_widget(self.process_guidance(snapshot), sidebar[2]);
    }

    fn draw_network(&self, frame: &mut Frame, area: Rect, _snapshot: &Snapshot) {
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),
                Constraint::Length(10),
                Constraint::Min(12),
            ])
            .split(area);

        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(sections[0]);
        frame.render_widget(
            self.metric_card(
                "RX",
                &format_rate(self.total_rx_rate()),
                "aggregate inbound",
                self.theme.status_good,
            ),
            top[0],
        );
        frame.render_widget(
            self.metric_card(
                "TX",
                &format_rate(self.total_tx_rate()),
                "aggregate outbound",
                self.theme.status_info,
            ),
            top[1],
        );
        frame.render_widget(
            self.metric_card(
                "Connections",
                &self.connections.len().to_string(),
                "ss-style snapshot",
                self.theme.brand,
            ),
            top[2],
        );

        let mut iface_state = TableState::default();
        iface_state.select(Some(self.network_scroll));
        frame.render_stateful_widget(
            self.interface_table(self.network_scroll, visible_rows(sections[1], 4)),
            sections[1],
            &mut iface_state,
        );
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(self.theme.brand))
            .track_style(Style::default().fg(self.theme.border));
        let mut scrollbar_state = ScrollbarState::new(self.interfaces.len())
            .position(self.network_scroll)
            .viewport_content_length(visible_rows(sections[1], 4));
        frame.render_stateful_widget(scrollbar, sections[1], &mut scrollbar_state);

        let mut conn_state = TableState::default();
        conn_state.select(Some(self.connection_scroll));
        frame.render_stateful_widget(
            self.connection_table(self.connection_scroll, visible_rows(sections[2], 4)),
            sections[2],
            &mut conn_state,
        );
        let conn_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(self.theme.brand))
            .track_style(Style::default().fg(self.theme.border));
        let mut conn_scrollbar_state = ScrollbarState::new(self.connections.len())
            .position(self.connection_scroll)
            .viewport_content_length(visible_rows(sections[2], 4));
        frame.render_stateful_widget(conn_scrollbar, sections[2], &mut conn_scrollbar_state);
    }

    fn draw_disks(&self, frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
        let sections = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
            .split(area);

        let mut disk_state = TableState::default();
        disk_state.select(Some(self.disk_scroll));
        frame.render_stateful_widget(
            self.disk_table(
                &snapshot.disks,
                self.disk_scroll,
                visible_rows(sections[0], 4),
            ),
            sections[0],
            &mut disk_state,
        );
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(self.theme.brand))
            .track_style(Style::default().fg(self.theme.border));
        let mut scrollbar_state = ScrollbarState::new(snapshot.disks.len())
            .position(self.disk_scroll)
            .viewport_content_length(visible_rows(sections[0], 4));
        frame.render_stateful_widget(scrollbar, sections[0], &mut scrollbar_state);

        let sidebar = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Min(6),
            ])
            .split(sections[1]);
        frame.render_widget(self.disk_stats(snapshot), sidebar[0]);
        frame.render_widget(self.disk_hotspots(snapshot), sidebar[1]);
        frame.render_widget(self.disk_barchart(snapshot), sidebar[2]);
        frame.render_widget(self.disk_guidance(), sidebar[3]);
    }

    fn draw_services(&self, frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
        let sections = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
            .split(area);

        let mut service_state = TableState::default();
        service_state.select(Some(self.service_scroll));
        frame.render_stateful_widget(
            self.service_table(
                &snapshot.services,
                self.service_scroll,
                visible_rows(sections[0], 4),
            ),
            sections[0],
            &mut service_state,
        );
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(self.theme.brand))
            .track_style(Style::default().fg(self.theme.border));
        let mut scrollbar_state = ScrollbarState::new(snapshot.services.len())
            .position(self.service_scroll)
            .viewport_content_length(visible_rows(sections[0], 4));
        frame.render_stateful_widget(scrollbar, sections[0], &mut scrollbar_state);

        let sidebar = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Length(7),
                Constraint::Min(8),
            ])
            .split(sections[1]);
        frame.render_widget(self.service_stats(snapshot), sidebar[0]);
        frame.render_widget(self.service_focus(snapshot), sidebar[1]);
        frame.render_widget(self.service_guidance(snapshot), sidebar[2]);

        if let Some(error) = &self.service_error {
            let popup = centered_rect(64, 20, area);
            frame.render_widget(Clear, popup);
            frame.render_widget(
                Paragraph::new(format!(
                    "{error}\n\nSystem, process, disk, and network tabs still have live data.\nThe service tab needs access to the systemd bus."
                ))
                .block(self.block("Service Notice"))
                .wrap(Wrap { trim: false }),
                popup,
            );
        }
    }

    fn draw_help(&self, frame: &mut Frame, area: Rect) {
        let text = vec![
            Line::from(Span::styled(
                "Current implementation baseline",
                Style::default()
                    .fg(self.theme.brand)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(
                "Dashboard: short summary across system, processes, network, disks, services",
            ),
            Line::from("System: full vitals + per-core history grid"),
            Line::from("Processes: sort cycle + name filter"),
            Line::from("Network: interfaces, throughput, active connections"),
            Line::from("Disks: full partition table"),
            Line::from("Services: systemd state when the bus is accessible"),
            Line::from(""),
            Line::from("Keys"),
            Line::from(
                "1-7 tabs  h/l switch  j/k scroll  / filter  s sort  gg/G  r refresh  q quit",
            ),
            Line::from(""),
            Line::from(
                "Roadmap next: process actions, logs, security, hardware, containers, plugins",
            ),
        ];

        frame.render_widget(
            Paragraph::new(text)
                .block(self.block("Help"))
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn metric_card<'a>(
        &self,
        title: &'a str,
        value: &'a str,
        subtitle: &'a str,
        color: Color,
    ) -> Paragraph<'a> {
        Paragraph::new(vec![
            Line::from(Span::styled(
                value,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                subtitle,
                Style::default().fg(self.theme.text_secondary),
            )),
        ])
        .block(self.block(title))
    }

    fn spark_panel<'a>(
        &self,
        title: &'a str,
        history: &VecDeque<u64>,
        color: Color,
    ) -> Sparkline<'a> {
        let data: Vec<u64> = history.iter().copied().collect();
        Sparkline::default()
            .block(self.block(title))
            .style(Style::default().fg(color))
            .data(&data)
            .max(100)
            .direction(ratatui::widgets::RenderDirection::LeftToRight)
            .absent_value_style(Style::default().fg(self.theme.text_muted))
    }

    fn network_chart(&self) -> Chart<'_> {
        const HISTORY_CAPACITY: usize = 60;

        Chart::new(vec![
            Dataset::default()
                .data(&self.histories.network_chart_rx)
                .name("RX KB/s")
                .graph_type(GraphType::Line)
                .style(Style::default().fg(self.theme.status_good)),
            Dataset::default()
                .data(&self.histories.network_chart_tx)
                .name("TX KB/s")
                .graph_type(GraphType::Line)
                .style(Style::default().fg(self.theme.status_info)),
        ])
        .block(self.block("Network Trend"))
        .x_axis(
            Axis::default()
                .bounds([0.0, HISTORY_CAPACITY as f64])
                .labels(vec![Span::raw("60s ago"), Span::raw("now")]),
        )
        .y_axis(Axis::default().title("KB/s"))
    }

    fn dashboard_overview(&self, snapshot: &Snapshot) -> Paragraph<'static> {
        Paragraph::new(vec![
            kv_line(&self.theme, "Host", &snapshot.host),
            kv_line(&self.theme, "OS", &snapshot.os),
            kv_line(&self.theme, "Kernel", &snapshot.kernel),
            kv_line(
                &self.theme,
                "Uptime",
                &commands::format_duration(snapshot.uptime),
            ),
            kv_line(&self.theme, "Load", &snapshot.load_average),
            kv_line(
                &self.theme,
                "Free Mem",
                &commands::format_bytes(snapshot.available_memory),
            ),
        ])
        .block(self.block("Overview"))
    }

    fn dashboard_process_preview(&self, snapshot: &Snapshot) -> Paragraph<'static> {
        let mut lines = vec![kv_line(
            &self.theme,
            "Sort",
            process_sort_label(self.process_sort),
        )];
        for process in snapshot.processes.iter().take(5) {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>5.1}% ", process.cpu),
                    Style::default().fg(self.theme.status_info),
                ),
                Span::raw(truncate(&process.name, 18)),
            ]));
        }
        Paragraph::new(lines).block(self.block("Process Preview"))
    }

    fn dashboard_network_preview(&self) -> Paragraph<'static> {
        let mut lines = Vec::new();
        for iface in self.interfaces.iter().take(4) {
            lines.push(Line::from(vec![
                Span::styled(
                    truncate(&iface.name, 10),
                    Style::default().fg(self.theme.brand),
                ),
                Span::raw(format!(
                    " {} / {}",
                    format_rate(iface.rx_rate),
                    format_rate(iface.tx_rate)
                )),
            ]));
        }
        if lines.is_empty() {
            lines.push(Line::from("No network data"));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "Connections: {}",
            self.connections.len()
        )));
        Paragraph::new(lines).block(self.block("Network Preview"))
    }

    fn system_identity(&self, snapshot: &Snapshot) -> Paragraph<'static> {
        Paragraph::new(vec![
            kv_line(&self.theme, "Host", &snapshot.host),
            kv_line(&self.theme, "Distribution", &snapshot.distro),
            kv_line(&self.theme, "Kernel", &snapshot.kernel),
        ])
        .block(self.block("Identity"))
    }

    fn system_runtime(&self, snapshot: &Snapshot) -> Paragraph<'static> {
        Paragraph::new(vec![
            kv_line(
                &self.theme,
                "Uptime",
                &commands::format_duration(snapshot.uptime),
            ),
            kv_line(&self.theme, "Boot Time", &snapshot.boot_time.to_string()),
            kv_line(&self.theme, "Load Avg", &snapshot.load_average),
        ])
        .block(self.block("Runtime"))
    }

    fn line_gauge_panel<'a>(
        &self,
        title: &'a str,
        ratio: f64,
        color: Color,
        label: &'a str,
    ) -> LineGauge<'a> {
        LineGauge::default()
            .block(self.block(title))
            .filled_style(Style::default().fg(color))
            .label(label)
            .ratio(ratio.clamp(0.0, 1.0))
    }

    fn render_per_core_grid(&self, frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
        let core_count = snapshot.cpu_per_core.len().max(1);
        let columns = 4usize.min(core_count);
        let rows = core_count.div_ceil(columns);
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Ratio(1, rows as u32); rows])
            .split(area);

        for (row_index, row_area) in vertical.iter().enumerate() {
            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Ratio(1, columns as u32); columns])
                .split(*row_area);

            for (col_index, cell) in horizontal.iter().enumerate() {
                let core_index = row_index * columns + col_index;
                if core_index >= core_count {
                    continue;
                }
                let history = self
                    .histories
                    .per_core
                    .get(core_index)
                    .cloned()
                    .unwrap_or_default();
                let data: Vec<u64> = history.iter().copied().collect();
                frame.render_widget(
                    Sparkline::default()
                        .block(self.block(&format!(
                            "Core {}  {:.1}%",
                            core_index, snapshot.cpu_per_core[core_index]
                        )))
                        .style(Style::default().fg(status_color(
                            &self.theme,
                            snapshot.cpu_per_core[core_index] as f64,
                        )))
                        .data(&data)
                        .max(100),
                    *cell,
                );
            }
        }
    }

    fn process_table<'a>(
        &self,
        processes: &[&'a ProcessRow],
        offset: usize,
        height: usize,
    ) -> Table<'a> {
        let rows: Vec<Row> = processes
            .iter()
            .skip(offset.min(processes.len()))
            .take(height)
            .map(|process| {
                Row::new(vec![
                    Cell::from(process.pid.clone()),
                    Cell::from(truncate(&process.name, 28)),
                    Cell::from(format!("{:.1}", process.cpu)),
                    Cell::from(commands::format_bytes(process.memory)),
                    Cell::from(process.status.clone()),
                ])
            })
            .collect();

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
                    .fg(self.theme.active_tab)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(self.block("Processes"))
    }

    fn process_stats(&self, snapshot: &Snapshot, filtered_count: usize) -> Paragraph<'static> {
        Paragraph::new(vec![
            kv_line(&self.theme, "Loaded", &snapshot.processes.len().to_string()),
            kv_line(&self.theme, "Filtered", &filtered_count.to_string()),
            kv_line(&self.theme, "Count", &snapshot.process_count.to_string()),
            kv_line(&self.theme, "Sort", process_sort_label(self.process_sort)),
        ])
        .block(self.block("Summary"))
    }

    fn process_memory_barchart<'a>(&self, processes: &[&'a ProcessRow]) -> BarChart<'a> {
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
            .block(self.block("Top Memory (MB)"))
            .data(&data)
            .bar_width(8)
            .bar_gap(1)
            .direction(Direction::Horizontal)
            .label_style(Style::default().fg(self.theme.text_secondary))
    }

    fn process_guidance(&self, snapshot: &Snapshot) -> List<'_> {
        let note = if snapshot.cpu_usage >= 90.0 {
            "CPU alert threshold exceeded"
        } else if percentage(snapshot.used_memory, snapshot.total_memory) >= 90.0 {
            "Memory alert threshold exceeded"
        } else {
            "No active alert threshold"
        };
        let note_color = status_color(
            &self.theme,
            (snapshot.cpu_usage as f64).max(percentage(snapshot.used_memory, snapshot.total_memory)),
        );

        let items = vec![
            ListItem::new(Span::styled(note, Style::default().fg(note_color))),
            ListItem::new(""),
            ListItem::new("`/` filters by process name"),
            ListItem::new("`s` cycles CPU → memory → name"),
            ListItem::new("Process actions are next phase"),
        ];

        List::new(items)
            .block(self.block("Notes"))
            .highlight_style(Style::default().fg(self.theme.brand))
    }

    fn interface_table(&self, offset: usize, height: usize) -> Table<'static> {
        let rows: Vec<Row> = self
            .interfaces
            .iter()
            .skip(offset.min(self.interfaces.len()))
            .take(height)
            .map(|iface| {
                Row::new(vec![
                    Cell::from(iface.name.clone()),
                    Cell::from(truncate(&iface.addresses, 28)),
                    Cell::from(format_rate(iface.rx_rate)),
                    Cell::from(format_rate(iface.tx_rate)),
                    Cell::from(commands::format_bytes(iface.total_rx)),
                    Cell::from(commands::format_bytes(iface.total_tx)),
                ])
            })
            .collect();

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
                    .fg(self.theme.active_tab)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(self.block("Interfaces"))
    }

    fn connection_table(&self, offset: usize, height: usize) -> Table<'static> {
        let rows: Vec<Row> = self
            .connections
            .iter()
            .skip(offset.min(self.connections.len()))
            .take(height)
            .map(|conn| {
                Row::new(vec![
                    Cell::from(conn.proto.clone()),
                    Cell::from(conn.state.clone()),
                    Cell::from(truncate(&conn.local, 24)),
                    Cell::from(truncate(&conn.remote, 24)),
                    Cell::from(truncate(&conn.process, 24)),
                ])
            })
            .collect();

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
                    .fg(self.theme.active_tab)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(self.block("Active Connections"))
    }

    fn disk_table<'a>(&self, disks: &'a [DiskRow], offset: usize, height: usize) -> Table<'a> {
        let rows: Vec<Row> = disks
            .iter()
            .skip(offset.min(disks.len()))
            .take(height)
            .map(|disk| {
                Row::new(vec![
                    Cell::from(truncate(&disk.mount, 26)),
                    Cell::from(disk.filesystem.clone()),
                    Cell::from(commands::format_bytes(disk.used)),
                    Cell::from(commands::format_bytes(disk.total)),
                    Cell::from(format!("{:.1}%", disk.usage)),
                ])
                .style(usage_style(&self.theme, disk.usage))
            })
            .collect();

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
                    .fg(self.theme.active_tab)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(self.block("Disk Usage"))
    }

    fn disk_stats(&self, snapshot: &Snapshot) -> Paragraph<'static> {
        let hot = snapshot
            .disks
            .iter()
            .filter(|disk| disk.usage >= 80.0)
            .count();
        Paragraph::new(vec![
            kv_line(&self.theme, "Mounted", &snapshot.disks.len().to_string()),
            kv_line(&self.theme, "80%+", &hot.to_string()),
            kv_line(&self.theme, "Worst", &worst_disk_mount(snapshot)),
        ])
        .block(self.block("Summary"))
    }

    fn disk_hotspots(&self, snapshot: &Snapshot) -> Paragraph<'static> {
        let mut lines = Vec::new();
        for disk in hottest_disks(snapshot, 5) {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>5.1}% ", disk.usage),
                    usage_style(&self.theme, disk.usage),
                ),
                Span::raw(truncate(&disk.mount, 20)),
            ]));
        }
        Paragraph::new(lines).block(self.block("Hotspots"))
    }

    fn disk_barchart(&self, snapshot: &Snapshot) -> BarChart<'_> {
        let data: Vec<(&str, u64)> = snapshot
            .disks
            .iter()
            .take(5)
            .map(|d| {
                let label = d.mount.rsplit('/').next().unwrap_or(&d.mount);
                let label_ref: &'static str = Box::leak(label.to_string().into_boxed_str());
                (label_ref, d.usage as u64)
            })
            .collect();

        BarChart::default()
            .block(self.block("Usage by Mount"))
            .data(&data)
            .max(100)
            .bar_width(7)
            .bar_gap(1)
            .direction(Direction::Horizontal)
            .label_style(Style::default().fg(self.theme.text_secondary))
    }

    fn disk_guidance(&self) -> List<'_> {
        let items = vec![
            ListItem::new("Per-partition usage is live"),
            ListItem::new("I/O speed is next"),
            ListItem::new("ncdu-style browsing planned"),
            ListItem::new("SMART health coming"),
        ];

        List::new(items)
            .block(self.block("Roadmap"))
            .highlight_style(Style::default().fg(self.theme.brand))
    }

    fn service_table<'a>(
        &self,
        services: &'a [ServiceRow],
        offset: usize,
        height: usize,
    ) -> Table<'a> {
        let rows: Vec<Row> = services
            .iter()
            .skip(offset.min(services.len()))
            .take(height)
            .map(|service| {
                Row::new(vec![
                    Cell::from(truncate(&service.name, 36)),
                    Cell::from(service.active.clone()),
                    Cell::from(service.sub.clone()),
                ])
                .style(service_row_style(&self.theme, service))
            })
            .collect();

        Table::new(
            rows,
            [
                Constraint::Percentage(60),
                Constraint::Length(12),
                Constraint::Length(12),
            ],
        )
        .header(
            Row::new(vec!["Name", "Active", "Sub"]).style(
                Style::default()
                    .fg(self.theme.active_tab)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(self.block("Services"))
    }

    fn service_stats(&self, snapshot: &Snapshot) -> Paragraph<'static> {
        Paragraph::new(vec![
            kv_line(&self.theme, "Rows", &snapshot.services.len().to_string()),
            kv_line(
                &self.theme,
                "Running",
                &snapshot
                    .service_summary
                    .map(|s| s.running.to_string())
                    .unwrap_or_else(|| "n/a".into()),
            ),
            kv_line(
                &self.theme,
                "Failed",
                &snapshot
                    .service_summary
                    .map(|s| s.failed.to_string())
                    .unwrap_or_else(|| "n/a".into()),
            ),
        ])
        .block(self.block("Summary"))
    }

    fn service_focus(&self, snapshot: &Snapshot) -> Paragraph<'static> {
        let mut lines = Vec::new();
        for service in snapshot.services.iter().take(5) {
            lines.push(Line::from(vec![
                Span::styled(
                    pad_status(&service.sub),
                    service_row_style(&self.theme, service),
                ),
                Span::raw(format!(" {}", truncate(&service.name, 18))),
            ]));
        }
        if lines.is_empty() {
            lines.push(Line::from("No service rows available"));
        }
        Paragraph::new(lines).block(self.block("Visible Services"))
    }

    fn service_guidance(&self, snapshot: &Snapshot) -> List<'_> {
        let headline = if snapshot.service_summary.is_some() {
            "systemd listing is live"
        } else {
            "systemd access blocked"
        };
        let items = vec![
            ListItem::new(headline),
            ListItem::new(""),
            ListItem::new("CLI start/stop/restart ready"),
            ListItem::new("Enable/disable next"),
            ListItem::new("Inline logs coming"),
        ];

        List::new(items)
            .block(self.block("Roadmap"))
            .highlight_style(Style::default().fg(self.theme.brand))
    }

    fn block<'a>(&self, title: &'a str) -> Block<'a> {
        Block::default()
            .title(Span::styled(
                format!(" {title} "),
                Style::default()
                    .fg(self.theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.border))
    }
}

fn push_history_value(history: &mut VecDeque<u64>, value: u64) {
    if history.len() >= HISTORY_CAPACITY {
        history.pop_front();
    }
    history.push_back(value);
}

fn kv_line(theme: &Theme, key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key}: "), Style::default().fg(theme.text_muted)),
        Span::raw(value.to_string()),
    ])
}

fn usage_style(theme: &Theme, usage: f64) -> Style {
    Style::default().fg(status_color(theme, usage))
}

fn status_color(theme: &Theme, value: f64) -> Color {
    if value >= 90.0 {
        theme.status_error
    } else if value >= 75.0 {
        theme.status_warn
    } else {
        theme.status_good
    }
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

fn service_row_style(theme: &Theme, service: &ServiceRow) -> Style {
    if service.active == "failed" {
        Style::default().fg(theme.status_error)
    } else if service.active == "active" {
        Style::default().fg(theme.status_good)
    } else {
        Style::default().fg(theme.text_secondary)
    }
}

fn disk_summary_color(theme: &Theme, snapshot: &Snapshot) -> Color {
    hottest_disks(snapshot, 1)
        .first()
        .map(|disk| status_color(theme, disk.usage))
        .unwrap_or(theme.text_secondary)
}

fn hottest_disks<'a>(snapshot: &'a Snapshot, limit: usize) -> Vec<&'a DiskRow> {
    let mut disks: Vec<_> = snapshot.disks.iter().collect();
    disks.sort_by(|a, b| b.usage.total_cmp(&a.usage));
    disks.into_iter().take(limit).collect()
}

fn worst_disk_mount(snapshot: &Snapshot) -> String {
    hottest_disks(snapshot, 1)
        .first()
        .map(|disk| disk.mount.clone())
        .unwrap_or_else(|| "n/a".into())
}

fn worst_disk_usage(snapshot: &Snapshot) -> String {
    hottest_disks(snapshot, 1)
        .first()
        .map(|disk| format!("{:.1}%", disk.usage))
        .unwrap_or_else(|| "n/a".into())
}

fn visible_rows(area: Rect, reserved: u16) -> usize {
    area.height.saturating_sub(reserved).max(1) as usize
}

fn percentage(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        used as f64 * 100.0 / total as f64
    }
}

fn format_rate(bytes_per_second: u64) -> String {
    format!("{}/s", commands::format_bytes(bytes_per_second))
}

fn process_sort_label(sort: ProcessSort) -> &'static str {
    match sort {
        ProcessSort::Cpu => "cpu",
        ProcessSort::Memory => "memory",
        ProcessSort::Name => "name",
    }
}

fn truncate(value: &str, max_len: usize) -> String {
    let count = value.chars().count();
    if count <= max_len {
        return value.to_string();
    }
    if max_len <= 1 {
        return "…".to_string();
    }
    let truncated: String = value.chars().take(max_len - 1).collect();
    format!("{truncated}…")
}

fn pad_status(value: &str) -> String {
    format!("{value:<8}")
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
