use crate::cli::{ProcessSort, ServiceState};
use crate::collectors::{self, ConnectionRow, ProcessRow, Snapshot};
use crate::theme::{Theme, default_theme};
use crate::ui;
use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Paragraph, Tabs, Wrap},
};
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::time::{Duration, Instant};
use sysinfo::Networks;

const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const TICK_RATE: Duration = Duration::from_millis(200);
pub(crate) const HISTORY_CAPACITY: usize = 60;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Dashboard,
    System,
    Processes,
    Network,
    Disks,
    Services,
    Help,
}

#[derive(Default)]
pub(crate) struct HistoryStore {
    pub cpu_total: VecDeque<u64>,
    pub memory_used: VecDeque<u64>,
    pub swap_used: VecDeque<u64>,
    pub network_rx: VecDeque<u64>,
    pub network_tx: VecDeque<u64>,
    pub per_core: Vec<VecDeque<u64>>,
    pub network_chart_rx: Vec<(f64, f64)>,
    pub network_chart_tx: Vec<(f64, f64)>,
}

#[derive(Clone)]
pub(crate) struct NetworkInterfaceView {
    pub name: String,
    pub addresses: String,
    pub rx_rate: u64,
    pub tx_rate: u64,
    pub total_rx: u64,
    pub total_tx: u64,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub(crate) struct App {
    pub active_tab: Tab,
    pub snapshot: Option<Snapshot>,
    pub service_error: Option<String>,
    pub status_line: String,
    pub last_refresh: Instant,
    pub process_scroll: usize,
    pub network_scroll: usize,
    pub connection_scroll: usize,
    pub disk_scroll: usize,
    pub service_scroll: usize,
    pub process_sort: ProcessSort,
    pub process_filter: String,
    pub filter_input: bool,
    pub pending_g: bool,
    pub theme: Theme,
    pub histories: HistoryStore,
    pub networks: Networks,
    pub interfaces: Vec<NetworkInterfaceView>,
    pub connections: Vec<ConnectionRow>,
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
        }
    }

    // -- Data refresh ------------------------------------------------------

    pub(crate) fn refresh(&mut self) {
        let elapsed = self.last_refresh.elapsed().as_secs_f64().max(1.0);

        match collectors::collect_snapshot(ServiceState::Running, 200) {
            Ok(mut snapshot) => {
                snapshot.processes =
                    collectors::procs::collect_processes(200, self.process_sort);
                self.service_error =
                    if snapshot.services.is_empty() && cfg!(target_os = "linux") {
                        Some(
                            "Service data unavailable in the current environment".into(),
                        )
                    } else {
                        None
                    };

                self.networks.refresh(true);
                let interface_addresses = collectors::netstat::collect_interface_addresses();
                self.interfaces =
                    self.collect_interface_views(&interface_addresses, elapsed);
                self.connections = collectors::netstat::collect_connections(200);
                self.push_histories(&snapshot);

                self.status_line = format!(
                    "{} | CPU {:.1}% | Mem {:.1}% | RX {} / TX {}",
                    snapshot.host,
                    snapshot.cpu_usage,
                    collectors::percentage(snapshot.used_memory, snapshot.total_memory),
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
            collectors::percentage(snapshot.used_memory, snapshot.total_memory).round() as u64,
        );
        push_history_value(
            &mut self.histories.swap_used,
            collectors::percentage(snapshot.used_swap, snapshot.total_swap).round() as u64,
        );
        push_history_value(&mut self.histories.network_rx, total_rx_kb);
        push_history_value(&mut self.histories.network_tx, total_tx_kb);

        // Chart data (owned for Chart widget)
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

    // -- Navigation --------------------------------------------------------

    pub(crate) fn cycle_process_sort(&mut self) {
        self.process_sort = match self.process_sort {
            ProcessSort::Cpu => ProcessSort::Memory,
            ProcessSort::Memory => ProcessSort::Name,
            ProcessSort::Name => ProcessSort::Cpu,
        };
    }

    pub(crate) fn next_tab(&mut self) {
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

    pub(crate) fn previous_tab(&mut self) {
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

    pub(crate) fn scroll_down(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = self.process_scroll.saturating_add(1),
            Tab::Network => self.connection_scroll = self.connection_scroll.saturating_add(1),
            Tab::Disks => self.disk_scroll = self.disk_scroll.saturating_add(1),
            Tab::Services => self.service_scroll = self.service_scroll.saturating_add(1),
            _ => {}
        }
    }

    pub(crate) fn scroll_up(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = self.process_scroll.saturating_sub(1),
            Tab::Network => self.connection_scroll = self.connection_scroll.saturating_sub(1),
            Tab::Disks => self.disk_scroll = self.disk_scroll.saturating_sub(1),
            Tab::Services => self.service_scroll = self.service_scroll.saturating_sub(1),
            _ => {}
        }
    }

    pub(crate) fn scroll_top(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = 0,
            Tab::Network => self.connection_scroll = 0,
            Tab::Disks => self.disk_scroll = 0,
            Tab::Services => self.service_scroll = 0,
            _ => {}
        }
    }

    pub(crate) fn scroll_bottom(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = usize::MAX / 4,
            Tab::Network => self.connection_scroll = usize::MAX / 4,
            Tab::Disks => self.disk_scroll = usize::MAX / 4,
            Tab::Services => self.service_scroll = usize::MAX / 4,
            _ => {}
        }
    }

    // -- Helpers -----------------------------------------------------------

    pub(crate) fn total_rx_rate(&self) -> u64 {
        self.interfaces.iter().map(|iface| iface.rx_rate).sum()
    }

    pub(crate) fn total_tx_rate(&self) -> u64 {
        self.interfaces.iter().map(|iface| iface.tx_rate).sum()
    }

    pub(crate) fn filtered_processes<'a>(
        &'a self,
        snapshot: &'a Snapshot,
    ) -> Vec<&'a ProcessRow> {
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

    // -- Drawing -----------------------------------------------------------

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let main_area = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(2),
            ])
            .split(area);

        frame.render_widget(self.tabs_widget(), main_area[0]);

        let content_area = main_area[1];
        let footer_area = main_area[2];

        if self.snapshot.is_some() {
            match self.active_tab {
                Tab::Dashboard => ui::dashboard::draw(frame, content_area, self),
                Tab::System => ui::system::draw(frame, content_area, self),
                Tab::Processes => ui::processes::draw(frame, content_area, self),
                Tab::Network => ui::network::draw(frame, content_area, self),
                Tab::Disks => ui::disks::draw(frame, content_area, self),
                Tab::Services => ui::services::draw(frame, content_area, self),
                Tab::Help => ui::help::draw(frame, content_area, self),
            }
        } else {
            frame.render_widget(
                Paragraph::new("No data loaded yet")
                    .block(ui::widgets::block(&self.theme, "Sysman"))
                    .style(Style::default().fg(self.theme.text_secondary))
                    .wrap(Wrap { trim: true }),
                content_area,
            );
        }

        ui::footer::draw(frame, footer_area, self);
    }

    fn tabs_widget(&self) -> Tabs<'static> {
        let titles: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("1", Style::default().fg(self.theme.status_info)),
                Span::raw(" Dashboard"),
            ]),
            Line::from(vec![
                Span::styled("2", Style::default().fg(self.theme.status_info)),
                Span::raw(" System"),
            ]),
            Line::from(vec![
                Span::styled("3", Style::default().fg(self.theme.status_info)),
                Span::raw(" Processes"),
            ]),
            Line::from(vec![
                Span::styled("4", Style::default().fg(self.theme.status_info)),
                Span::raw(" Network"),
            ]),
            Line::from(vec![
                Span::styled("5", Style::default().fg(self.theme.status_info)),
                Span::raw(" Disks"),
            ]),
            Line::from(vec![
                Span::styled("6", Style::default().fg(self.theme.status_info)),
                Span::raw(" Services"),
            ]),
            Line::from(vec![
                Span::styled("7", Style::default().fg(self.theme.status_info)),
                Span::raw(" Help"),
            ]),
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
            .divider(Span::styled(
                " | ",
                Style::default().fg(self.theme.border),
            ))
            .block(
                Block::default()
                    .title(Span::styled(
                        " Sysman ",
                        Style::default()
                            .fg(self.theme.brand)
                            .add_modifier(Modifier::BOLD),
                    ))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
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
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

fn push_history_value(history: &mut VecDeque<u64>, value: u64) {
    if history.len() >= HISTORY_CAPACITY {
        history.pop_front();
    }
    history.push_back(value);
}

fn format_rate(bytes_per_second: u64) -> String {
    format!("{}/s", collectors::format_bytes(bytes_per_second))
}
