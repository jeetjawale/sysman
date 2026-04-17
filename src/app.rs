use crate::animation::AnimationManager;
use crate::cli::{ProcessSort, ServiceState};
use crate::collectors::{self, ConnectionRow, DiskIoCounters, DiskIoRow, ProcessRow, Snapshot};
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
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io;
use std::process::Command as ProcessCommand;
use std::time::{Duration, Instant};
use sysinfo::Networks;

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
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

        if event::poll(TICK_RATE)?
            && let Event::Key(key) = event::read()?
            && app.handle_key(key)
        {
            break;
        }

        // Tick animation frame
        app.animation_frame = (app.animation_frame + 1) % 60;
        app.last_tick = Instant::now();

        if app.last_refresh.elapsed() >= REFRESH_INTERVAL {
            app.refresh();
            app.is_loading = false;
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
    Logs,
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessViewMode {
    Flat,
    Tree,
    User,
}

#[derive(Clone, Copy)]
pub(crate) struct ProcessViewRow<'a> {
    pub process: &'a ProcessRow,
    pub depth: usize,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub(crate) struct App {
    pub active_tab: Tab,
    pub snapshot: Option<Snapshot>,
    pub service_error: Option<String>,
    pub service_logs: Vec<String>,
    pub service_logs_error: Option<String>,
    pub status_line: String,
    pub last_refresh: Instant,
    pub last_tick: Instant,
    pub process_scroll: usize,
    pub network_scroll: usize,
    pub connection_scroll: usize,
    pub disk_scroll: usize,
    pub service_scroll: usize,
    pub logs_scroll: usize,
    pub process_sort: ProcessSort,
    pub process_view: ProcessViewMode,
    pub process_filter: String,
    pub filter_input: bool,
    pub renice_input: bool,
    pub renice_value: String,
    pub pending_g: bool,
    pub theme: Theme,
    pub histories: HistoryStore,
    pub networks: Networks,
    pub interfaces: Vec<NetworkInterfaceView>,
    pub connections: Vec<ConnectionRow>,
    pub disk_io_rows: Vec<DiskIoRow>,
    pub disk_io_counters: DiskIoCounters,
    pub dir_scan_rows: Vec<(String, u64)>,
    pub dir_scan_target: Option<String>,
    pub logs_journal: Vec<String>,
    pub logs_syslog: Vec<String>,
    pub logs_dmesg: Vec<String>,
    pub animation_frame: u32,
    pub is_loading: bool,
    pub anim_manager: AnimationManager,
    pub tab_transition: Option<(Tab, u32)>, // (from_tab, start_frame)
    sys: sysinfo::System,
    // Cache for BarChart labels to avoid Box::leak
    pub process_chart_labels: Vec<String>,
    pub disk_chart_labels: Vec<String>,
}

impl App {
    fn new() -> Self {
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
        sys.refresh_cpu_usage();
        Self {
            active_tab: Tab::Dashboard,
            snapshot: None,
            service_error: None,
            service_logs: Vec::new(),
            service_logs_error: None,
            status_line: "Loading system snapshot...".into(),
            last_refresh: Instant::now() - REFRESH_INTERVAL,
            last_tick: Instant::now(),
            process_scroll: 0,
            network_scroll: 0,
            connection_scroll: 0,
            disk_scroll: 0,
            service_scroll: 0,
            logs_scroll: 0,
            process_sort: ProcessSort::Cpu,
            process_view: ProcessViewMode::Flat,
            process_filter: String::new(),
            filter_input: false,
            renice_input: false,
            renice_value: String::new(),
            pending_g: false,
            theme: default_theme(),
            histories: HistoryStore::default(),
            networks: Networks::new_with_refreshed_list(),
            interfaces: Vec::new(),
            connections: Vec::new(),
            disk_io_rows: Vec::new(),
            disk_io_counters: DiskIoCounters::default(),
            dir_scan_rows: Vec::new(),
            dir_scan_target: None,
            logs_journal: Vec::new(),
            logs_syslog: Vec::new(),
            logs_dmesg: Vec::new(),
            animation_frame: 0,
            is_loading: true,
            anim_manager: AnimationManager::new(),
            tab_transition: None,
            sys,
            process_chart_labels: Vec::new(),
            disk_chart_labels: Vec::new(),
        }
    }

    // -- Data refresh ------------------------------------------------------

    pub(crate) fn refresh(&mut self) {
        let elapsed = self.last_refresh.elapsed().as_secs_f64().max(1.0);

        match collectors::collect_snapshot(&mut self.sys, ServiceState::Running, 200) {
            Ok(mut snapshot) => {
                snapshot.processes =
                    collectors::procs::collect_processes(&self.sys, 200, self.process_sort);
                self.service_error = if snapshot.services.is_empty() && cfg!(target_os = "linux") {
                    Some("Service data unavailable in the current environment".into())
                } else {
                    None
                };

                self.networks.refresh(true);
                let interface_addresses = collectors::netstat::collect_interface_addresses();
                self.interfaces = self.collect_interface_views(&interface_addresses, elapsed);
                self.connections = collectors::netstat::collect_connections(200);
                let (io_rows, io_counters) =
                    collectors::storage::collect_disk_io_rates(&self.disk_io_counters, elapsed);
                self.disk_io_rows = io_rows;
                self.disk_io_counters = io_counters;
                self.push_histories(&snapshot);

                self.status_line = format!(
                    "{} | CPU {:.1}% | Mem {:.1}% | RX {} / TX {}",
                    snapshot.host,
                    snapshot.cpu_usage,
                    collectors::percentage(snapshot.used_memory, snapshot.total_memory),
                    ui::widgets::format_rate(self.total_rx_rate()),
                    ui::widgets::format_rate(self.total_tx_rate()),
                );
                self.snapshot = Some(snapshot);
                if self.active_tab == Tab::Services {
                    self.refresh_selected_service_logs();
                }
                if self.active_tab == Tab::Logs {
                    self.refresh_logs_view();
                }
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
            ProcessSort::Memory => ProcessSort::Pid,
            ProcessSort::Pid => ProcessSort::Name,
            ProcessSort::Name => ProcessSort::Cpu,
        };
    }

    pub(crate) fn cycle_process_view(&mut self) {
        self.process_view = match self.process_view {
            ProcessViewMode::Flat => ProcessViewMode::Tree,
            ProcessViewMode::Tree => ProcessViewMode::User,
            ProcessViewMode::User => ProcessViewMode::Flat,
        };
        self.process_scroll = 0;
    }

    pub(crate) fn next_tab(&mut self) {
        let from = self.active_tab;
        self.active_tab = match self.active_tab {
            Tab::Dashboard => Tab::System,
            Tab::System => Tab::Processes,
            Tab::Processes => Tab::Network,
            Tab::Network => Tab::Disks,
            Tab::Disks => Tab::Services,
            Tab::Services => Tab::Logs,
            Tab::Logs => Tab::Help,
            Tab::Help => Tab::Dashboard,
        };
        self.tab_transition = Some((from, self.animation_frame));
        self.anim_manager.start("tab_slide", 0.0, 1.0, 200);
        if self.active_tab == Tab::Services {
            self.refresh_selected_service_logs();
        }
        if self.active_tab == Tab::Logs {
            self.refresh_logs_view();
        }
    }

    pub(crate) fn previous_tab(&mut self) {
        let from = self.active_tab;
        self.active_tab = match self.active_tab {
            Tab::Dashboard => Tab::Help,
            Tab::System => Tab::Dashboard,
            Tab::Processes => Tab::System,
            Tab::Network => Tab::Processes,
            Tab::Disks => Tab::Network,
            Tab::Services => Tab::Disks,
            Tab::Logs => Tab::Services,
            Tab::Help => Tab::Logs,
        };
        self.tab_transition = Some((from, self.animation_frame));
        self.anim_manager.start("tab_slide", 0.0, 1.0, 200);
        if self.active_tab == Tab::Services {
            self.refresh_selected_service_logs();
        }
        if self.active_tab == Tab::Logs {
            self.refresh_logs_view();
        }
    }

    pub(crate) fn scroll_down(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = self.process_scroll.saturating_add(1),
            Tab::Network => self.connection_scroll = self.connection_scroll.saturating_add(1),
            Tab::Disks => self.disk_scroll = self.disk_scroll.saturating_add(1),
            Tab::Services => {
                self.service_scroll = self.service_scroll.saturating_add(1);
                self.refresh_selected_service_logs();
            }
            Tab::Logs => self.logs_scroll = self.logs_scroll.saturating_add(1),
            _ => {}
        }
    }

    pub(crate) fn scroll_up(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = self.process_scroll.saturating_sub(1),
            Tab::Network => self.connection_scroll = self.connection_scroll.saturating_sub(1),
            Tab::Disks => self.disk_scroll = self.disk_scroll.saturating_sub(1),
            Tab::Services => {
                self.service_scroll = self.service_scroll.saturating_sub(1);
                self.refresh_selected_service_logs();
            }
            Tab::Logs => self.logs_scroll = self.logs_scroll.saturating_sub(1),
            _ => {}
        }
    }

    pub(crate) fn scroll_top(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = 0,
            Tab::Network => self.connection_scroll = 0,
            Tab::Disks => self.disk_scroll = 0,
            Tab::Services => {
                self.service_scroll = 0;
                self.refresh_selected_service_logs();
            }
            Tab::Logs => self.logs_scroll = 0,
            _ => {}
        }
    }

    pub(crate) fn scroll_bottom(&mut self) {
        match self.active_tab {
            Tab::Processes => self.process_scroll = usize::MAX / 4,
            Tab::Network => self.connection_scroll = usize::MAX / 4,
            Tab::Disks => self.disk_scroll = usize::MAX / 4,
            Tab::Services => {
                self.service_scroll = usize::MAX / 4;
                self.refresh_selected_service_logs();
            }
            Tab::Logs => self.logs_scroll = usize::MAX / 4,
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

    pub(crate) fn filtered_processes<'a>(&'a self, snapshot: &'a Snapshot) -> Vec<&'a ProcessRow> {
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

    pub(crate) fn process_view_rows<'a>(
        &'a self,
        snapshot: &'a Snapshot,
    ) -> Vec<ProcessViewRow<'a>> {
        let filtered = self.filtered_processes(snapshot);
        match self.process_view {
            ProcessViewMode::Flat => filtered
                .into_iter()
                .map(|process| ProcessViewRow { process, depth: 0 })
                .collect(),
            ProcessViewMode::User => {
                let mut grouped = filtered;
                grouped.sort_by(|a, b| {
                    a.user
                        .cmp(&b.user)
                        .then_with(|| b.cpu.total_cmp(&a.cpu))
                        .then_with(|| a.name.cmp(&b.name))
                });
                grouped
                    .into_iter()
                    .map(|process| ProcessViewRow { process, depth: 0 })
                    .collect()
            }
            ProcessViewMode::Tree => self.process_tree_rows(filtered),
        }
    }

    fn process_tree_rows<'a>(&'a self, filtered: Vec<&'a ProcessRow>) -> Vec<ProcessViewRow<'a>> {
        fn sort_processes(processes: &mut [&ProcessRow], sort: ProcessSort) {
            match sort {
                ProcessSort::Cpu => processes
                    .sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then_with(|| a.name.cmp(&b.name))),
                ProcessSort::Memory => processes.sort_by(|a, b| b.memory.cmp(&a.memory)),
                ProcessSort::Pid => processes.sort_by_key(|p| p.pid.parse::<u32>().unwrap_or(0)),
                ProcessSort::Name => processes.sort_by(|a, b| a.name.cmp(&b.name)),
            }
        }

        fn visit<'a>(
            process: &'a ProcessRow,
            depth: usize,
            out: &mut Vec<ProcessViewRow<'a>>,
            children: &HashMap<String, Vec<&'a ProcessRow>>,
            visited: &mut HashSet<String>,
            sort: ProcessSort,
        ) {
            if !visited.insert(process.pid.clone()) {
                return;
            }
            out.push(ProcessViewRow { process, depth });
            if let Some(kids) = children.get(&process.pid) {
                let mut ordered = kids.clone();
                sort_processes(&mut ordered, sort);
                for child in ordered {
                    visit(child, depth + 1, out, children, visited, sort);
                }
            }
        }

        let mut children: HashMap<String, Vec<&ProcessRow>> = HashMap::new();
        let mut known_pids = HashSet::new();
        for process in &filtered {
            known_pids.insert(process.pid.clone());
        }
        for process in &filtered {
            if let Some(parent) = &process.parent_pid {
                children.entry(parent.clone()).or_default().push(*process);
            }
        }

        let mut roots: Vec<&ProcessRow> = filtered
            .iter()
            .copied()
            .filter(|process| match &process.parent_pid {
                Some(parent) => !known_pids.contains(parent),
                None => true,
            })
            .collect();
        sort_processes(&mut roots, self.process_sort);

        let mut out = Vec::with_capacity(filtered.len());
        let mut visited = HashSet::new();
        for root in roots {
            visit(
                root,
                0,
                &mut out,
                &children,
                &mut visited,
                self.process_sort,
            );
        }

        for process in filtered {
            if visited.insert(process.pid.clone()) {
                out.push(ProcessViewRow { process, depth: 0 });
            }
        }

        out
    }

    pub(crate) fn selected_process<'a>(&'a self, snapshot: &'a Snapshot) -> Option<&'a ProcessRow> {
        let rows = self.process_view_rows(snapshot);
        if rows.is_empty() {
            return None;
        }
        let index = self.process_scroll.min(rows.len().saturating_sub(1));
        rows.get(index).map(|row| row.process)
    }

    pub(crate) fn kill_selected_process(&mut self, force: bool) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.status_line = "No snapshot loaded".into();
            return;
        };
        let Some(process) = self.selected_process(snapshot) else {
            self.status_line = "No process selected".into();
            return;
        };
        let pid = process.pid.clone();
        let signal = if force { "-KILL" } else { "-TERM" };
        let action = if force { "SIGKILL" } else { "SIGTERM" };
        let output = ProcessCommand::new("kill").args([signal, &pid]).output();

        match output {
            Ok(output) if output.status.success() => {
                self.status_line = format!("{action} sent to PID {pid}");
                self.refresh();
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                self.status_line = if stderr.is_empty() {
                    format!("Failed to send {action} to PID {pid}")
                } else {
                    format!("{action} failed for PID {pid}: {stderr}")
                };
            }
            Err(error) => {
                self.status_line = format!("Failed to run kill for PID {pid}: {error}");
            }
        }
    }

    pub(crate) fn apply_renice_selected(&mut self) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.status_line = "No snapshot loaded".into();
            return;
        };
        let Some(process) = self.selected_process(snapshot) else {
            self.status_line = "No process selected".into();
            return;
        };
        let value = self.renice_value.trim();
        let Ok(nice) = value.parse::<i32>() else {
            self.status_line = format!("Invalid nice value: {value}");
            return;
        };
        if !(-20..=19).contains(&nice) {
            self.status_line = "Nice value must be in range -20..19".into();
            return;
        }

        let pid = process.pid.clone();
        let output = ProcessCommand::new("renice")
            .args([nice.to_string(), "-p".into(), pid.clone()])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                self.status_line = format!("Set PID {pid} nice to {nice}");
                self.refresh();
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                self.status_line = if stderr.is_empty() {
                    format!("Renice failed for PID {pid}")
                } else {
                    format!("Renice failed for PID {pid}: {stderr}")
                };
            }
            Err(error) => {
                self.status_line = format!("Failed to run renice for PID {pid}: {error}");
            }
        }
    }

    pub(crate) fn process_view_label(&self) -> &'static str {
        match self.process_view {
            ProcessViewMode::Flat => "flat",
            ProcessViewMode::Tree => "tree",
            ProcessViewMode::User => "user",
        }
    }

    pub(crate) fn selected_service_name(&self, snapshot: &Snapshot) -> Option<String> {
        if snapshot.services.is_empty() {
            return None;
        }
        let index = self
            .service_scroll
            .min(snapshot.services.len().saturating_sub(1));
        snapshot
            .services
            .get(index)
            .map(|service| service.name.clone())
    }

    pub(crate) fn refresh_selected_service_logs(&mut self) {
        self.service_logs.clear();
        self.service_logs_error = None;

        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        let Some(service) = self.selected_service_name(snapshot) else {
            return;
        };

        match collectors::systemd::collect_service_logs(&service, 8) {
            Ok(lines) => self.service_logs = lines,
            Err(error) => self.service_logs_error = Some(error.to_string()),
        }
    }

    pub(crate) fn refresh_logs_view(&mut self) {
        self.logs_journal = collectors::logs::collect_journal_lines(20);
        self.logs_syslog = collectors::logs::collect_syslog_lines(20);
        self.logs_dmesg = collectors::logs::collect_dmesg_lines(20);
    }

    pub(crate) fn scan_selected_disk_dirs(&mut self) {
        self.dir_scan_rows.clear();
        self.dir_scan_target = None;
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.status_line = "No snapshot loaded".into();
            return;
        };
        if snapshot.disks.is_empty() {
            self.status_line = "No disk selected".into();
            return;
        }
        let index = self.disk_scroll.min(snapshot.disks.len().saturating_sub(1));
        let mount = snapshot.disks[index].mount.clone();
        self.dir_scan_rows = collectors::storage::collect_directory_sizes(&mount, 8);
        self.dir_scan_target = Some(mount.clone());
        self.status_line = if self.dir_scan_rows.is_empty() {
            format!("No directory data for {mount}")
        } else {
            format!("Directory scan complete for {mount}")
        };
    }

    pub(crate) fn act_on_selected_service(&mut self, action: &str) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.status_line = "No snapshot loaded".into();
            return;
        };
        let Some(service) = self.selected_service_name(snapshot) else {
            self.status_line = "No service selected".into();
            return;
        };

        if let Err(error) = collectors::systemd::ensure_linux_systemd() {
            self.status_line = format!("{error}");
            return;
        }

        match collectors::systemd::run_systemctl(&[action, &service]) {
            Ok(_) => {
                self.status_line = format!("service {service}: {action} OK");
                self.refresh();
            }
            Err(error) => {
                self.status_line = format!("service {service}: {action} failed ({error})");
            }
        }
    }

    // -- Drawing -----------------------------------------------------------

    fn draw(&mut self, frame: &mut Frame) {
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

        if let Some(snapshot) = self.snapshot.clone() {
            match self.active_tab {
                Tab::Dashboard => ui::dashboard::draw(frame, content_area, self, &snapshot),
                Tab::System => ui::system::draw(frame, content_area, self, &snapshot),
                Tab::Processes => ui::processes::draw(frame, content_area, self, &snapshot),
                Tab::Network => ui::network::draw(frame, content_area, self, &snapshot),
                Tab::Disks => ui::disks::draw(frame, content_area, self, &snapshot),
                Tab::Services => ui::services::draw(frame, content_area, self, &snapshot),
                Tab::Logs => ui::logs::draw(frame, content_area, self, &snapshot),
                Tab::Help => ui::help::draw(frame, content_area, self, &snapshot),
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
                Span::raw(" Logs"),
            ]),
            Line::from(vec![
                Span::styled("8", Style::default().fg(self.theme.status_info)),
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
            Tab::Logs => 6,
            Tab::Help => 7,
        };

        Tabs::new(titles)
            .select(selected)
            .divider(Span::styled(" | ", Style::default().fg(self.theme.border)))
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
