use crate::animation::AnimationManager;
use crate::cli::{ProcessSort, ServiceState};
use crate::collectors::{
    self, ConnectionRow, DiskIoCounters, DiskIoRow, ProcessNetRow, ProcessRow,
    ServiceFailureDetails, SmartHealthRow, Snapshot,
};
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
use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io;
use std::process::Command as ProcessCommand;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
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
        app.poll_background_jobs();
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
    Overview,
    Cpu,
    Memory,
    Processes,
    Containers,
    Network,
    Disk,
    Gpu,
    Services,
    Logs,
    Hardware,
    Help,
}

#[derive(Default)]
pub(crate) struct HistoryStore {
    pub cpu_total: VecDeque<u64>,
    pub cpu_temp: VecDeque<u64>,
    pub memory_used: VecDeque<u64>,
    pub swap_used: VecDeque<u64>,
    pub network_rx: VecDeque<u64>,
    pub network_tx: VecDeque<u64>,
    pub per_core: Vec<VecDeque<u64>>,
    pub network_chart_rx: Vec<(f64, f64)>,
    pub network_chart_tx: Vec<(f64, f64)>,
    pub process_cpu: HashMap<String, VecDeque<u64>>,
    pub gpu_util: VecDeque<u64>,
    pub gpu_vram: VecDeque<u64>,
    pub gpu_temp: VecDeque<u64>,
    pub gpu_power: VecDeque<u64>,
    pub gpu_fan: VecDeque<u64>,
}

#[derive(Clone)]
pub(crate) struct NetworkInterfaceView {
    pub name: String,
    pub addresses: String,
    pub state: String,
    pub mac: String,
    pub mtu: String,
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
    Service,
    Container,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogLevelFilter {
    All,
    Error,
    Warn,
    Info,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogSourceFilter {
    All,
    Journal,
    Syslog,
    Dmesg,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionStateFilter {
    All,
    Established,
    Listen,
    Closing,
    Suspicious,
}

#[derive(Clone, Copy)]
pub(crate) struct ProcessViewRow<'a> {
    pub process: &'a ProcessRow,
    pub depth: usize,
}

#[derive(Clone)]
pub(crate) struct MemoryLeakSuspect {
    pub pid: String,
    pub name: String,
    pub current_memory: u64,
    pub growth_rate: u64,
    pub streak: u8,
}

enum DiskScanEvent {
    Progress(String),
    Complete {
        mount: String,
        dir_rows: Vec<(String, u64)>,
        large_rows: Vec<(String, u64)>,
    },
    Failed(String),
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
    pub service_failure_details: Option<ServiceFailureDetails>,
    pub service_failure_error: Option<String>,
    pub status_line: String,
    pub last_refresh: Instant,
    pub last_tick: Instant,
    pub process_scroll: usize,
    pub container_scroll: usize,
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
    pub pin_input: bool,
    pub pin_core_value: String,
    pub network_tool_input: bool,
    pub network_tool_value: String,
    pub logs_regex_input: bool,
    pub logs_query: String,
    pub logs_level_filter: LogLevelFilter,
    pub logs_source_filter: LogSourceFilter,
    pub logs_autoscroll: bool,
    pub connection_state_filter: ConnectionStateFilter,
    pub service_state_filter: ServiceState,
    pub pending_g: bool,
    pub theme: Theme,
    pub histories: HistoryStore,
    pub networks: Networks,
    pub interfaces: Vec<NetworkInterfaceView>,
    pub connections: Vec<ConnectionRow>,
    pub network_process_rows: Vec<ProcessNetRow>,
    pub network_process_counters: HashMap<u32, (u64, u64)>,
    pub disk_io_rows: Vec<DiskIoRow>,
    pub disk_io_counters: DiskIoCounters,
    pub dir_scan_rows: Vec<(String, u64)>,
    pub dir_scan_target: Option<String>,
    pub dir_scan_depth: usize,
    pub large_file_rows: Vec<(String, u64)>,
    pub disk_scan_in_progress: bool,
    pub disk_scan_progress: Option<String>,
    pub disk_scan_started_at: Option<Instant>,
    pub smart_health_rows: Vec<SmartHealthRow>,
    pub logs_journal: Vec<String>,
    pub logs_syslog: Vec<String>,
    pub logs_dmesg: Vec<String>,
    /// Pre-computed spike message shown in the logs UI header.
    pub error_spike: Option<String>,
    /// Rolling per-refresh error counts (one entry per second tick, capacity 60).
    pub error_count_history: VecDeque<u32>,
    pub process_open_files: Vec<String>,
    pub process_open_ports: Vec<String>,
    pub process_detail_error: Option<String>,
    pub process_cmdline: Option<String>,
    pub process_environ: Vec<String>,
    pub process_maps: Vec<String>,
    pub network_tool_output: Vec<String>,
    pub animation_frame: u32,
    pub is_loading: bool,
    pub anim_manager: AnimationManager,
    pub tab_transition: Option<(Tab, u32)>, // (from_tab, start_frame)
    sys: sysinfo::System,
    // Cache for BarChart labels to avoid Box::leak
    pub process_chart_labels: Vec<String>,
    pub disk_chart_labels: Vec<String>,
    pub context_switch_rate: Option<u64>,
    pub throttle_events_delta: Option<u64>,
    pub memory_page_fault_rate: Option<u64>,
    pub memory_major_fault_rate: Option<u64>,
    pub memory_leak_suspects: Vec<MemoryLeakSuspect>,
    last_context_switches: Option<u64>,
    last_throttle_events: Option<u64>,
    last_page_faults: Option<u64>,
    last_major_faults: Option<u64>,
    process_memory_baseline: HashMap<String, u64>,
    process_growth_streaks: HashMap<String, u8>,
    disk_scan_receiver: Option<Receiver<DiskScanEvent>>,
}

impl App {
    fn new() -> Self {
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
        sys.refresh_cpu_usage();
        Self {
            active_tab: Tab::Overview,
            snapshot: None,
            service_error: None,
            service_logs: Vec::new(),
            service_logs_error: None,
            service_failure_details: None,
            service_failure_error: None,
            status_line: "Loading system snapshot...".into(),
            last_refresh: Instant::now() - REFRESH_INTERVAL,
            last_tick: Instant::now(),
            process_scroll: 0,
            container_scroll: 0,
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
            pin_input: false,
            pin_core_value: String::new(),
            network_tool_input: false,
            network_tool_value: String::new(),
            logs_regex_input: false,
            logs_query: String::new(),
            logs_level_filter: LogLevelFilter::All,
            logs_source_filter: LogSourceFilter::All,
            logs_autoscroll: true,
            connection_state_filter: ConnectionStateFilter::All,
            service_state_filter: ServiceState::Running,
            pending_g: false,
            theme: default_theme(),
            histories: HistoryStore::default(),
            networks: Networks::new_with_refreshed_list(),
            interfaces: Vec::new(),
            connections: Vec::new(),
            network_process_rows: Vec::new(),
            network_process_counters: HashMap::new(),
            disk_io_rows: Vec::new(),
            disk_io_counters: DiskIoCounters::default(),
            dir_scan_rows: Vec::new(),
            dir_scan_target: None,
            dir_scan_depth: 2,
            large_file_rows: Vec::new(),
            disk_scan_in_progress: false,
            disk_scan_progress: None,
            disk_scan_started_at: None,
            smart_health_rows: Vec::new(),
            logs_journal: Vec::new(),
            logs_syslog: Vec::new(),
            logs_dmesg: Vec::new(),
            error_spike: None,
            error_count_history: VecDeque::new(),
            process_open_files: Vec::new(),
            process_open_ports: Vec::new(),
            process_detail_error: None,
            process_cmdline: None,
            process_environ: Vec::new(),
            process_maps: Vec::new(),
            network_tool_output: Vec::new(),
            animation_frame: 0,
            is_loading: true,
            anim_manager: AnimationManager::new(),
            tab_transition: None,
            sys,
            process_chart_labels: Vec::new(),
            disk_chart_labels: Vec::new(),
            context_switch_rate: None,
            throttle_events_delta: None,
            memory_page_fault_rate: None,
            memory_major_fault_rate: None,
            memory_leak_suspects: Vec::new(),
            last_context_switches: None,
            last_throttle_events: None,
            last_page_faults: None,
            last_major_faults: None,
            process_memory_baseline: HashMap::new(),
            process_growth_streaks: HashMap::new(),
            disk_scan_receiver: None,
        }
    }

    // -- Data refresh ------------------------------------------------------

    pub(crate) fn refresh(&mut self) {
        let elapsed = self.last_refresh.elapsed().as_secs_f64().max(1.0);

        match collectors::collect_snapshot(&mut self.sys, self.service_state_filter, 200) {
            Ok(mut snapshot) => {
                snapshot.processes =
                    collectors::procs::collect_processes(&self.sys, 200, self.process_sort);
                self.service_error =
                    if snapshot.service_summary.is_none() && cfg!(target_os = "linux") {
                        Some("Service data unavailable in the current environment".into())
                    } else {
                        None
                    };

                self.networks.refresh(true);
                let interface_addresses = collectors::netstat::collect_interface_addresses();
                let interface_link_details = collectors::netstat::collect_interface_link_details();
                self.interfaces = self.collect_interface_views(
                    &interface_addresses,
                    &interface_link_details,
                    elapsed,
                );
                self.connections = collectors::netstat::collect_connections(200);
                let max_conn_scroll = self.filtered_connections().len().saturating_sub(1);
                self.connection_scroll = self.connection_scroll.min(max_conn_scroll);
                let (process_rows, process_counters) =
                    collectors::netstat::collect_process_bandwidth(
                        &self.network_process_counters,
                        elapsed,
                        12,
                    );
                self.network_process_rows = process_rows;
                self.network_process_counters = process_counters;
                let (io_rows, io_counters) =
                    collectors::storage::collect_disk_io_rates(&self.disk_io_counters, elapsed);
                self.disk_io_rows = io_rows;
                self.disk_io_counters = io_counters;
                self.smart_health_rows = collectors::storage::collect_smart_health(8);
                self.push_histories(&snapshot, elapsed);

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
                    self.refresh_selected_service_failure_details();
                }
                if self.active_tab == Tab::Logs || self.active_tab == Tab::Overview {
                    self.refresh_logs_view();
                }
                if self.active_tab == Tab::Processes {
                    self.refresh_selected_process_details();
                }
            }
            Err(error) => self.status_line = format!("Refresh failed: {error}"),
        }

        self.last_refresh = Instant::now();
    }

    fn collect_interface_views(
        &self,
        addresses: &BTreeMap<String, Vec<String>>,
        link_details: &BTreeMap<String, collectors::netstat::InterfaceLinkDetails>,
        elapsed: f64,
    ) -> Vec<NetworkInterfaceView> {
        let mut interfaces: Vec<_> = self
            .networks
            .iter()
            .map(|(name, data)| {
                let details = link_details.get(name).cloned().unwrap_or_default();
                NetworkInterfaceView {
                    name: name.clone(),
                    addresses: addresses
                        .get(name)
                        .map(|list| list.join(", "))
                        .unwrap_or_else(|| "-".into()),
                    state: details.state,
                    mac: details.mac,
                    mtu: details
                        .mtu
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".into()),
                    rx_rate: (data.received() as f64 / elapsed) as u64,
                    tx_rate: (data.transmitted() as f64 / elapsed) as u64,
                    total_rx: data.total_received(),
                    total_tx: data.total_transmitted(),
                }
            })
            .collect();

        interfaces.sort_by(|a, b| {
            (b.rx_rate + b.tx_rate)
                .cmp(&(a.rx_rate + a.tx_rate))
                .then_with(|| a.name.cmp(&b.name))
        });
        interfaces
    }

    fn push_histories(&mut self, snapshot: &Snapshot, elapsed: f64) {
        let total_rx_kb = self.total_rx_rate() / 1024;
        let total_tx_kb = self.total_tx_rate() / 1024;

        push_history_value(
            &mut self.histories.cpu_total,
            snapshot.cpu_usage.round() as u64,
        );
        if let Some(cpu_temp) = snapshot.cpu_runtime.temperature_c {
            push_history_value(&mut self.histories.cpu_temp, cpu_temp.round() as u64);
        }
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

        let gpu_devices = &snapshot.gpu_runtime.devices;
        if !gpu_devices.is_empty() {
            if let Some(util) = average_gpu_metric(gpu_devices, |device| device.utilization_pct) {
                push_history_value(&mut self.histories.gpu_util, util.round() as u64);
            }
            if let Some(vram_pct) = average_gpu_metric(gpu_devices, |device| {
                match (device.memory_used_mib, device.memory_total_mib) {
                    (Some(used), Some(total)) if total > 0 => {
                        Some((used as f64 * 100.0 / total as f64).clamp(0.0, 100.0))
                    }
                    _ => None,
                }
            }) {
                push_history_value(&mut self.histories.gpu_vram, vram_pct.round() as u64);
            }
            if let Some(temp) = average_gpu_metric(gpu_devices, |device| device.temperature_c) {
                push_history_value(&mut self.histories.gpu_temp, temp.round() as u64);
            }
            if let Some(power) = average_gpu_metric(gpu_devices, |device| device.power_w) {
                push_history_value(&mut self.histories.gpu_power, power.round() as u64);
            }
            if let Some(fan) = average_gpu_metric(gpu_devices, |device| device.fan_pct) {
                push_history_value(&mut self.histories.gpu_fan, fan.round() as u64);
            }
        }

        if let Some(current) = snapshot.cpu_runtime.context_switches {
            self.context_switch_rate = self.last_context_switches.map(|previous| {
                let delta = current.saturating_sub(previous);
                (delta as f64 / elapsed.max(1.0)).round() as u64
            });
            self.last_context_switches = Some(current);
        } else {
            self.context_switch_rate = None;
            self.last_context_switches = None;
        }

        if let Some(current) = snapshot.cpu_runtime.throttle_count {
            self.throttle_events_delta = self
                .last_throttle_events
                .map(|previous| current.saturating_sub(previous));
            self.last_throttle_events = Some(current);
        } else {
            self.throttle_events_delta = None;
            self.last_throttle_events = None;
        }

        self.update_memory_diagnostics(snapshot, elapsed);
    }

    fn update_memory_diagnostics(&mut self, snapshot: &Snapshot, elapsed: f64) {
        if let Some(page_faults) = &snapshot.memory_runtime.page_faults {
            self.memory_page_fault_rate = self.last_page_faults.map(|previous| {
                let delta = page_faults.minor.saturating_sub(previous);
                (delta as f64 / elapsed.max(1.0)).round() as u64
            });
            self.memory_major_fault_rate = self.last_major_faults.map(|previous| {
                let delta = page_faults.major.saturating_sub(previous);
                (delta as f64 / elapsed.max(1.0)).round() as u64
            });
            self.last_page_faults = Some(page_faults.minor);
            self.last_major_faults = Some(page_faults.major);
        } else {
            self.memory_page_fault_rate = None;
            self.memory_major_fault_rate = None;
            self.last_page_faults = None;
            self.last_major_faults = None;
        }

        let mut current_memory_map = HashMap::new();
        let mut leak_rows = Vec::new();

        for process in &snapshot.processes {
            let pid = process.pid.clone();
            let current = process.memory;
            current_memory_map.insert(pid.clone(), current);

            let growth = self
                .process_memory_baseline
                .get(&pid)
                .copied()
                .map(|previous| current.saturating_sub(previous))
                .unwrap_or(0);

            let streak = self.process_growth_streaks.entry(pid.clone()).or_insert(0);
            if growth >= 1_048_576 {
                *streak = streak.saturating_add(1);
            } else {
                *streak = 0;
            }

            let growth_rate = (growth as f64 / elapsed.max(1.0)).round() as u64;
            if *streak >= 3 && growth_rate >= 512 * 1024 {
                leak_rows.push(MemoryLeakSuspect {
                    pid,
                    name: process.name.clone(),
                    current_memory: current,
                    growth_rate,
                    streak: *streak,
                });
            }
        }

        self.process_memory_baseline = current_memory_map;
        self.process_growth_streaks
            .retain(|pid, _| self.process_memory_baseline.contains_key(pid));
        leak_rows.sort_by(|a, b| {
            b.growth_rate
                .cmp(&a.growth_rate)
                .then_with(|| b.current_memory.cmp(&a.current_memory))
        });
        self.memory_leak_suspects = leak_rows.into_iter().take(6).collect();
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
            ProcessViewMode::User => ProcessViewMode::Service,
            ProcessViewMode::Service => ProcessViewMode::Container,
            ProcessViewMode::Container => ProcessViewMode::Flat,
        };
        self.process_scroll = 0;
        self.refresh_selected_process_details();
    }

    pub(crate) fn next_tab(&mut self) {
        let from = self.active_tab;
        self.active_tab = match self.active_tab {
            Tab::Overview => Tab::Cpu,
            Tab::Cpu => Tab::Memory,
            Tab::Memory => Tab::Processes,
            Tab::Processes => Tab::Containers,
            Tab::Containers => Tab::Network,
            Tab::Network => Tab::Disk,
            Tab::Disk => Tab::Gpu,
            Tab::Gpu => Tab::Services,
            Tab::Services => Tab::Logs,
            Tab::Logs => Tab::Hardware,
            Tab::Hardware => Tab::Help,
            Tab::Help => Tab::Overview,
        };
        self.tab_transition = Some((from, self.animation_frame));
        self.anim_manager.start("tab_slide", 0.0, 1.0, 200);
        if self.active_tab == Tab::Services {
            self.refresh_selected_service_logs();
            self.refresh_selected_service_failure_details();
        }
        if self.active_tab == Tab::Processes {
            self.refresh_selected_process_details();
        }
        if self.active_tab == Tab::Logs {
            self.refresh_logs_view();
        }
    }

    pub(crate) fn previous_tab(&mut self) {
        let from = self.active_tab;
        self.active_tab = match self.active_tab {
            Tab::Overview => Tab::Help,
            Tab::Cpu => Tab::Overview,
            Tab::Memory => Tab::Cpu,
            Tab::Processes => Tab::Memory,
            Tab::Containers => Tab::Processes,
            Tab::Network => Tab::Containers,
            Tab::Disk => Tab::Network,
            Tab::Gpu => Tab::Disk,
            Tab::Services => Tab::Gpu,
            Tab::Logs => Tab::Services,
            Tab::Hardware => Tab::Logs,
            Tab::Help => Tab::Hardware,
        };
        self.tab_transition = Some((from, self.animation_frame));
        self.anim_manager.start("tab_slide", 0.0, 1.0, 200);
        if self.active_tab == Tab::Services {
            self.refresh_selected_service_logs();
            self.refresh_selected_service_failure_details();
        }
        if self.active_tab == Tab::Processes {
            self.refresh_selected_process_details();
        }
        if self.active_tab == Tab::Logs {
            self.refresh_logs_view();
        }
    }

    pub(crate) fn scroll_down(&mut self) {
        match self.active_tab {
            Tab::Processes => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    let rows = self.process_view_rows(snapshot);
                    let max_scroll = rows.len().saturating_sub(1);
                    self.process_scroll = (self.process_scroll + 1).min(max_scroll);
                    self.refresh_selected_process_details();
                }
            }
            Tab::Containers => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    let max_scroll = snapshot.containers.len().saturating_sub(1);
                    self.container_scroll = (self.container_scroll + 1).min(max_scroll);
                }
            }
            Tab::Network => {
                let max_scroll = self.filtered_connections().len().saturating_sub(1);
                self.connection_scroll = (self.connection_scroll + 1).min(max_scroll);
            }
            Tab::Disk => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    let max_scroll = snapshot.disks.len().saturating_sub(1);
                    self.disk_scroll = (self.disk_scroll + 1).min(max_scroll);
                }
            }
            Tab::Services => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    let max_scroll = snapshot.services.len().saturating_sub(1);
                    self.service_scroll = (self.service_scroll + 1).min(max_scroll);
                    self.refresh_selected_service_logs();
                    self.refresh_selected_service_failure_details();
                }
            }
            Tab::Logs => {
                self.logs_autoscroll = false;
                self.logs_scroll = self.logs_scroll.saturating_add(1);
            }
            _ => {}
        }
    }

    pub(crate) fn scroll_up(&mut self) {
        match self.active_tab {
            Tab::Processes => {
                self.process_scroll = self.process_scroll.saturating_sub(1);
                self.refresh_selected_process_details();
            }
            Tab::Containers => self.container_scroll = self.container_scroll.saturating_sub(1),
            Tab::Network => self.connection_scroll = self.connection_scroll.saturating_sub(1),
            Tab::Disk => self.disk_scroll = self.disk_scroll.saturating_sub(1),
            Tab::Services => {
                self.service_scroll = self.service_scroll.saturating_sub(1);
                self.refresh_selected_service_logs();
                self.refresh_selected_service_failure_details();
            }
            Tab::Logs => {
                self.logs_autoscroll = false;
                self.logs_scroll = self.logs_scroll.saturating_sub(1);
            }
            _ => {}
        }
    }

    pub(crate) fn scroll_top(&mut self) {
        match self.active_tab {
            Tab::Processes => {
                self.process_scroll = 0;
                self.refresh_selected_process_details();
            }
            Tab::Containers => self.container_scroll = 0,
            Tab::Network => self.connection_scroll = 0,
            Tab::Disk => self.disk_scroll = 0,
            Tab::Services => {
                self.service_scroll = 0;
                self.refresh_selected_service_logs();
                self.refresh_selected_service_failure_details();
            }
            Tab::Logs => {
                self.logs_autoscroll = false;
                self.logs_scroll = 0;
            }
            _ => {}
        }
    }

    pub(crate) fn scroll_bottom(&mut self) {
        match self.active_tab {
            Tab::Processes => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    let rows = self.process_view_rows(snapshot);
                    self.process_scroll = rows.len().saturating_sub(1);
                    self.refresh_selected_process_details();
                }
            }
            Tab::Containers => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    self.container_scroll = snapshot.containers.len().saturating_sub(1);
                }
            }
            Tab::Network => {
                self.connection_scroll = self.filtered_connections().len().saturating_sub(1);
            }
            Tab::Disk => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    self.disk_scroll = snapshot.disks.len().saturating_sub(1);
                }
            }
            Tab::Services => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    self.service_scroll = snapshot.services.len().saturating_sub(1);
                    self.refresh_selected_service_logs();
                    self.refresh_selected_service_failure_details();
                }
            }
            Tab::Logs => {
                self.logs_autoscroll = true;
                self.logs_scroll = usize::MAX / 4;
            }
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
            ProcessViewMode::Service => {
                let mut grouped = filtered;
                grouped.sort_by(|a, b| {
                    a.service_group
                        .cmp(&b.service_group)
                        .then_with(|| b.cpu.total_cmp(&a.cpu))
                        .then_with(|| a.name.cmp(&b.name))
                });
                grouped
                    .into_iter()
                    .map(|process| ProcessViewRow { process, depth: 0 })
                    .collect()
            }
            ProcessViewMode::Container => {
                let mut grouped = filtered;
                grouped.sort_by(|a, b| {
                    a.container_group
                        .cmp(&b.container_group)
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

    pub(crate) fn apply_pin_selected(&mut self) {
        if !cfg!(target_os = "linux") {
            self.status_line = "CPU affinity is currently supported on Linux only".into();
            return;
        }

        let Some(snapshot) = self.snapshot.as_ref() else {
            self.status_line = "No snapshot loaded".into();
            return;
        };
        let Some(process) = self.selected_process(snapshot) else {
            self.status_line = "No process selected".into();
            return;
        };

        let value = self.pin_core_value.trim();
        let Ok(core) = value.parse::<usize>() else {
            self.status_line = format!("Invalid CPU core: {value}");
            return;
        };
        if core >= snapshot.cpu_cores {
            self.status_line = format!(
                "CPU core out of range: 0..{}",
                snapshot.cpu_cores.saturating_sub(1)
            );
            return;
        }

        let pid = process.pid.clone();
        let output = ProcessCommand::new("taskset")
            .args(["-cp", &core.to_string(), &pid])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                self.status_line = format!("Pinned PID {pid} to CPU core {core}");
                self.refresh();
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                self.status_line = if stderr.is_empty() {
                    format!("CPU pin failed for PID {pid}")
                } else {
                    format!("CPU pin failed for PID {pid}: {stderr}")
                };
            }
            Err(error) => {
                self.status_line = format!("Failed to run taskset for PID {pid}: {error}");
            }
        }
    }

    pub(crate) fn process_view_label(&self) -> &'static str {
        match self.process_view {
            ProcessViewMode::Flat => "flat",
            ProcessViewMode::Tree => "tree",
            ProcessViewMode::User => "user",
            ProcessViewMode::Service => "service",
            ProcessViewMode::Container => "container",
        }
    }

    pub(crate) fn process_group_label<'a>(&self, process: &'a ProcessRow) -> &'a str {
        match self.process_view {
            ProcessViewMode::User => &process.user,
            ProcessViewMode::Service => &process.service_group,
            ProcessViewMode::Container => &process.container_group,
            _ => "-",
        }
    }

    pub(crate) fn selected_connection(&self) -> Option<&ConnectionRow> {
        let filtered = self.filtered_connections();
        if filtered.is_empty() {
            return None;
        }
        let index = self.connection_scroll.min(filtered.len().saturating_sub(1));
        filtered.get(index).copied()
    }

    pub(crate) fn filtered_connections(&self) -> Vec<&ConnectionRow> {
        self.connections
            .iter()
            .filter(|conn| self.connection_matches(conn))
            .collect()
    }

    pub(crate) fn connection_state_filter_label(&self) -> &'static str {
        match self.connection_state_filter {
            ConnectionStateFilter::All => "all",
            ConnectionStateFilter::Established => "established",
            ConnectionStateFilter::Listen => "listen",
            ConnectionStateFilter::Closing => "closing",
            ConnectionStateFilter::Suspicious => "suspicious",
        }
    }

    pub(crate) fn cycle_connection_state_filter(&mut self) {
        self.connection_state_filter = match self.connection_state_filter {
            ConnectionStateFilter::All => ConnectionStateFilter::Established,
            ConnectionStateFilter::Established => ConnectionStateFilter::Listen,
            ConnectionStateFilter::Listen => ConnectionStateFilter::Closing,
            ConnectionStateFilter::Closing => ConnectionStateFilter::Suspicious,
            ConnectionStateFilter::Suspicious => ConnectionStateFilter::All,
        };
        self.connection_scroll = 0;
    }

    fn connection_matches(&self, conn: &ConnectionRow) -> bool {
        match self.connection_state_filter {
            ConnectionStateFilter::All => true,
            ConnectionStateFilter::Established => {
                matches!(conn.state.as_str(), "ESTAB" | "ESTABLISHED")
            }
            ConnectionStateFilter::Listen => conn.state == "LISTEN",
            ConnectionStateFilter::Closing => matches!(
                conn.state.as_str(),
                "TIME_WAIT"
                    | "CLOSE_WAIT"
                    | "CLOSING"
                    | "SYN-SENT"
                    | "SYN-RECV"
                    | "FIN-WAIT-1"
                    | "FIN-WAIT-2"
                    | "LAST_ACK"
                    | "CLOSED"
            ),
            ConnectionStateFilter::Suspicious => conn.suspicious.is_some(),
        }
    }

    pub(crate) fn kill_selected_connection(&mut self) {
        let Some(conn) = self.selected_connection() else {
            self.status_line = "No connection selected".into();
            return;
        };
        match collectors::netstat::kill_connection(conn) {
            Ok(msg) => {
                self.status_line = msg;
                self.refresh();
            }
            Err(error) => {
                self.status_line = error;
            }
        }
    }

    pub(crate) fn block_selected_remote_ip(&mut self) {
        let Some(conn) = self.selected_connection() else {
            self.status_line = "No connection selected".into();
            return;
        };
        match collectors::netstat::block_ip(&conn.remote_ip) {
            Ok(msg) => {
                self.status_line = msg;
                self.refresh();
            }
            Err(error) => {
                self.status_line = error;
            }
        }
    }

    pub(crate) fn run_network_tools(&mut self) {
        let target = self.network_tool_value.trim();
        if target.is_empty() {
            self.status_line = "Network tools target is empty".into();
            return;
        }

        self.network_tool_output.clear();
        self.network_tool_output.push(format!("Target: {target}"));
        self.network_tool_output.push(String::new());
        self.network_tool_output.push("DNS".into());
        self.network_tool_output.extend(
            collectors::netstat::run_dns_lookup(target, 6)
                .into_iter()
                .map(|line| format!("  {line}")),
        );
        self.network_tool_output.push(String::new());
        self.network_tool_output.push("Ping".into());
        self.network_tool_output.extend(
            collectors::netstat::run_ping(target, 6)
                .into_iter()
                .map(|line| format!("  {line}")),
        );
        self.network_tool_output.push(String::new());
        self.network_tool_output.push("Traceroute".into());
        self.network_tool_output.extend(
            collectors::netstat::run_traceroute(target, 6)
                .into_iter()
                .map(|line| format!("  {line}")),
        );
        self.network_tool_output.push(String::new());
        self.network_tool_output.push("HTTP probe".into());
        self.network_tool_output.extend(
            collectors::netstat::run_http_probe(target, 4)
                .into_iter()
                .map(|line| format!("  {line}")),
        );
        self.status_line = format!("Network tools refreshed for {target}");
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

    pub(crate) fn refresh_selected_service_failure_details(&mut self) {
        self.service_failure_details = None;
        self.service_failure_error = None;

        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        let Some(service) = self.selected_service_name(snapshot) else {
            return;
        };

        match collectors::systemd::collect_service_failure_details(&service) {
            Ok(details) => self.service_failure_details = Some(details),
            Err(error) => self.service_failure_error = Some(error.to_string()),
        }
    }

    pub(crate) fn refresh_logs_view(&mut self) {
        self.logs_journal = collectors::logs::collect_journal_lines(200);
        self.logs_syslog = collectors::logs::collect_syslog_lines(200);
        self.logs_dmesg = collectors::logs::collect_dmesg_lines(200);
        self.update_error_spike();
        if self.logs_autoscroll {
            self.logs_scroll = usize::MAX / 4;
        }
    }

    /// Push the current error count into the rolling history and compute the spike status.
    ///
    /// History is a ring buffer of `HISTORY_CAPACITY` slots (one per 1-second refresh).
    /// A spike is declared when the **recent half** of the history has ≥2× the error
    /// rate of the **older half**, with a minimum of 3 errors in the recent window.
    fn update_error_spike(&mut self) {
        let count = self
            .logs_journal
            .iter()
            .chain(self.logs_syslog.iter())
            .chain(self.logs_dmesg.iter())
            .filter(|line| is_error_line(line))
            .count() as u32;

        if self.error_count_history.len() >= HISTORY_CAPACITY {
            self.error_count_history.pop_front();
        }
        self.error_count_history.push_back(count);

        let len = self.error_count_history.len();
        // Need at least 4 slots before comparing halves.
        if len < 4 {
            self.error_spike = None;
            return;
        }

        let half = len / 2;
        let recent: u32 = self.error_count_history.iter().rev().take(half).sum();
        let older: u32 = self.error_count_history.iter().rev().skip(half).take(half).sum();

        let spike_threshold = (older as f64 * 2.0).max(3.0);
        self.error_spike = if recent as f64 >= spike_threshold {
            Some(format!(
                "⚠ error spike: {} errs (recent {}) vs {} errs (prev {})",
                recent,
                half,
                older,
                half,
            ))
        } else {
            None
        };
    }

    pub(crate) fn cycle_logs_level_filter(&mut self) {
        self.logs_level_filter = match self.logs_level_filter {
            LogLevelFilter::All => LogLevelFilter::Error,
            LogLevelFilter::Error => LogLevelFilter::Warn,
            LogLevelFilter::Warn => LogLevelFilter::Info,
            LogLevelFilter::Info => LogLevelFilter::All,
        };
        self.logs_scroll = 0;
    }

    pub(crate) fn cycle_service_state_filter(&mut self) {
        self.service_state_filter = match self.service_state_filter {
            ServiceState::Running => ServiceState::Failed,
            ServiceState::Failed => ServiceState::All,
            ServiceState::All => ServiceState::Running,
        };
        self.service_scroll = 0;
        self.refresh();
        self.refresh_selected_service_logs();
        self.refresh_selected_service_failure_details();
    }

    pub(crate) fn service_state_filter_label(&self) -> &'static str {
        match self.service_state_filter {
            ServiceState::Running => "running",
            ServiceState::Failed => "failed",
            ServiceState::All => "all",
        }
    }

    pub(crate) fn logs_level_label(&self) -> &'static str {
        match self.logs_level_filter {
            LogLevelFilter::All => "all",
            LogLevelFilter::Error => "error",
            LogLevelFilter::Warn => "warn",
            LogLevelFilter::Info => "info",
        }
    }

    pub(crate) fn cycle_logs_source_filter(&mut self) {
        self.logs_source_filter = match self.logs_source_filter {
            LogSourceFilter::All => LogSourceFilter::Journal,
            LogSourceFilter::Journal => LogSourceFilter::Syslog,
            LogSourceFilter::Syslog => LogSourceFilter::Dmesg,
            LogSourceFilter::Dmesg => LogSourceFilter::All,
        };
        self.logs_scroll = 0;
    }

    pub(crate) fn logs_source_label(&self) -> &'static str {
        match self.logs_source_filter {
            LogSourceFilter::All => "all",
            LogSourceFilter::Journal => "journal",
            LogSourceFilter::Syslog => "syslog",
            LogSourceFilter::Dmesg => "dmesg",
        }
    }

    pub(crate) fn toggle_logs_autoscroll(&mut self) {
        self.logs_autoscroll = !self.logs_autoscroll;
        if self.logs_autoscroll {
            self.logs_scroll = usize::MAX / 4;
        }
    }

    pub(crate) fn navigate_logs_match(&mut self, forward: bool) {
        let query = self.logs_query.trim();
        if query.is_empty() {
            self.status_line = "Log regex is empty".into();
            return;
        }
        let Ok(regex) = Regex::new(query) else {
            self.status_line = "Invalid log regex".into();
            return;
        };

        let lines = self.filtered_log_lines_for_navigation();
        if lines.is_empty() {
            self.status_line = "No logs in current source/filter".into();
            return;
        }

        let matches: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter_map(|(idx, line)| regex.is_match(line).then_some(idx))
            .collect();
        if matches.is_empty() {
            self.status_line = "No regex matches found".into();
            return;
        }

        let current = self.logs_scroll.min(lines.len().saturating_sub(1));
        let target = if forward {
            matches
                .iter()
                .copied()
                .find(|idx| *idx > current)
                .unwrap_or(matches[0])
        } else {
            matches
                .iter()
                .copied()
                .rev()
                .find(|idx| *idx < current)
                .unwrap_or(*matches.last().unwrap())
        };
        self.logs_autoscroll = false;
        self.logs_scroll = target;
        let match_pos = matches
            .iter()
            .position(|idx| *idx == target)
            .map(|i| i + 1)
            .unwrap_or(1);
        self.status_line = format!(
            "Log match {match_pos}/{} in {}",
            matches.len(),
            self.logs_source_label()
        );
    }

    fn filtered_log_lines_for_navigation(&self) -> Vec<&String> {
        let iter: Box<dyn Iterator<Item = &String> + '_> = match self.logs_source_filter {
            LogSourceFilter::All => Box::new(
                self.logs_journal
                    .iter()
                    .chain(self.logs_syslog.iter())
                    .chain(self.logs_dmesg.iter()),
            ),
            LogSourceFilter::Journal => Box::new(self.logs_journal.iter()),
            LogSourceFilter::Syslog => Box::new(self.logs_syslog.iter()),
            LogSourceFilter::Dmesg => Box::new(self.logs_dmesg.iter()),
        };
        iter.filter(|line| logs_matches_level(line, self.logs_level_filter))
            .collect()
    }

    pub(crate) fn refresh_selected_process_details(&mut self) {
        self.process_open_files.clear();
        self.process_open_ports.clear();
        self.process_detail_error = None;
        self.process_cmdline = None;
        self.process_environ.clear();
        self.process_maps.clear();

        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        let Some(process) = self.selected_process(snapshot) else {
            return;
        };
        let Ok(pid) = process.pid.parse::<u32>() else {
            self.process_detail_error = Some("Unable to parse selected PID".into());
            return;
        };
        let process_pid = process.pid.clone();

        match collectors::procs::collect_open_files(pid, 8) {
            Ok(files) => self.process_open_files = files,
            Err(error) => {
                self.process_detail_error = Some(format!("Open files unavailable: {error}"));
            }
        }
        self.process_open_ports = collectors::procs::collect_open_ports(pid, 8);

        let details = collectors::procs::collect_process_details(pid, 3);
        self.process_cmdline = Some(details.cmdline);
        self.process_environ = details.environ;
        self.process_maps = details.maps;

        if let Some(cpu_usage) = snapshot
            .processes
            .iter()
            .find(|p| p.pid == process_pid)
            .map(|p| p.cpu as u64)
        {
            self.histories
                .process_cpu
                .entry(process_pid.clone())
                .or_insert_with(VecDeque::new)
                .push_back(cpu_usage);

            let history = self.histories.process_cpu.get_mut(&process_pid).unwrap();
            if history.len() > HISTORY_CAPACITY {
                history.pop_front();
            }
        }
    }

    pub(crate) fn scan_selected_disk_dirs(&mut self) {
        if self.disk_scan_in_progress {
            self.status_line = "Disk scan already running".into();
            return;
        }
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
        self.dir_scan_target = Some(mount.clone());
        self.disk_scan_in_progress = true;
        self.disk_scan_progress = Some("queued".into());
        self.disk_scan_started_at = Some(Instant::now());

        let depth = self.dir_scan_depth;
        let (sender, receiver) = mpsc::channel::<DiskScanEvent>();
        self.disk_scan_receiver = Some(receiver);
        self.status_line = format!("Starting async disk scan for {mount} (depth {depth})");
        thread::spawn(move || {
            let _ = sender.send(DiskScanEvent::Progress("scanning directories".into()));
            let dir_rows =
                collectors::storage::collect_directory_sizes_with_depth(&mount, depth, 12);
            let _ = sender.send(DiskScanEvent::Progress("scanning large files".into()));
            let large_rows = collectors::storage::collect_large_files(&mount, 8);
            if dir_rows.is_empty() && large_rows.is_empty() {
                let _ = sender.send(DiskScanEvent::Failed(format!(
                    "No explorer data for {mount}"
                )));
            } else {
                let _ = sender.send(DiskScanEvent::Complete {
                    mount,
                    dir_rows,
                    large_rows,
                });
            }
        });
    }

    pub(crate) fn cycle_disk_scan_depth(&mut self) {
        self.dir_scan_depth = match self.dir_scan_depth {
            1 => 2,
            2 => 3,
            _ => 1,
        };
        self.scan_selected_disk_dirs();
    }

    pub(crate) fn poll_background_jobs(&mut self) {
        let Some(receiver) = self.disk_scan_receiver.take() else {
            return;
        };

        let mut keep_receiver = true;
        loop {
            match receiver.try_recv() {
                Ok(DiskScanEvent::Progress(progress)) => {
                    self.disk_scan_progress = Some(progress);
                }
                Ok(DiskScanEvent::Complete {
                    mount,
                    dir_rows,
                    large_rows,
                }) => {
                    self.dir_scan_target = Some(mount.clone());
                    self.dir_scan_rows = dir_rows;
                    self.large_file_rows = large_rows;
                    self.disk_scan_in_progress = false;
                    self.disk_scan_progress = None;
                    self.disk_scan_started_at = None;
                    self.status_line = format!(
                        "Disk scan complete for {mount} (depth {}, {} dirs, {} files)",
                        self.dir_scan_depth,
                        self.dir_scan_rows.len(),
                        self.large_file_rows.len()
                    );
                    keep_receiver = false;
                }
                Ok(DiskScanEvent::Failed(error)) => {
                    self.disk_scan_in_progress = false;
                    self.disk_scan_progress = None;
                    self.disk_scan_started_at = None;
                    self.status_line = error;
                    keep_receiver = false;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.disk_scan_in_progress = false;
                    self.disk_scan_progress = None;
                    self.disk_scan_started_at = None;
                    self.status_line = "Disk scan worker disconnected".into();
                    keep_receiver = false;
                    break;
                }
            }
        }

        if keep_receiver {
            self.disk_scan_receiver = Some(receiver);
        }
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
                Tab::Overview => ui::dashboard::draw(frame, content_area, self, &snapshot),
                Tab::Cpu => ui::cpu::draw(frame, content_area, self, &snapshot),
                Tab::Memory => ui::memory::draw(frame, content_area, self, &snapshot),
                Tab::Processes => {
                    ui::processes::draw(frame, content_area, self, &snapshot)
                }
                Tab::Containers => {
                    ui::containers::draw(frame, content_area, self, &snapshot)
                }
                Tab::Network => ui::network::draw(frame, content_area, self, &snapshot),
                Tab::Disk => ui::disks::draw(frame, content_area, self, &snapshot),
                Tab::Gpu => ui::gpu::draw(frame, content_area, self, &snapshot),
                Tab::Services => ui::services::draw(frame, content_area, self, &snapshot),
                Tab::Logs => ui::logs::draw(frame, content_area, self, &snapshot),
                Tab::Hardware => ui::hardware::draw(frame, content_area, self, &snapshot),
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
                Span::raw(" Overview"),
            ]),
            Line::from(vec![
                Span::styled("2", Style::default().fg(self.theme.status_info)),
                Span::raw(" CPU"),
            ]),
            Line::from(vec![
                Span::styled("3", Style::default().fg(self.theme.status_info)),
                Span::raw(" Memory"),
            ]),
            Line::from(vec![
                Span::styled("4", Style::default().fg(self.theme.status_info)),
                Span::raw(" Processes"),
            ]),
            Line::from(vec![
                Span::styled("C", Style::default().fg(self.theme.status_info)),
                Span::raw(" Containers"),
            ]),
            Line::from(vec![
                Span::styled("5", Style::default().fg(self.theme.status_info)),
                Span::raw(" Network"),
            ]),
            Line::from(vec![
                Span::styled("6", Style::default().fg(self.theme.status_info)),
                Span::raw(" Disk"),
            ]),
            Line::from(vec![
                Span::styled("7", Style::default().fg(self.theme.status_info)),
                Span::raw(" GPU"),
            ]),
            Line::from(vec![
                Span::styled("8", Style::default().fg(self.theme.status_info)),
                Span::raw(" Services"),
            ]),
            Line::from(vec![
                Span::styled("9", Style::default().fg(self.theme.status_info)),
                Span::raw(" Logs"),
            ]),
            Line::from(vec![
                Span::styled("0", Style::default().fg(self.theme.status_info)),
                Span::raw(" Hardware"),
            ]),
            Line::from(vec![
                Span::styled("?", Style::default().fg(self.theme.status_info)),
                Span::raw(" Help"),
            ]),
        ];
        let selected = match self.active_tab {
            Tab::Overview => 0,
            Tab::Cpu => 1,
            Tab::Memory => 2,
            Tab::Processes => 3,
            Tab::Containers => 4,
            Tab::Network => 5,
            Tab::Disk => 6,
            Tab::Gpu => 7,
            Tab::Services => 8,
            Tab::Logs => 9,
            Tab::Hardware => 10,
            Tab::Help => 11,
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

fn average_gpu_metric(
    devices: &[collectors::host::GpuRuntimeDevice],
    metric: impl Fn(&collectors::host::GpuRuntimeDevice) -> Option<f64>,
) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0usize;
    for device in devices {
        if let Some(value) = metric(device) {
            total += value;
            count += 1;
        }
    }
    (count > 0).then_some(total / count as f64)
}

fn logs_matches_level(line: &str, level: LogLevelFilter) -> bool {
    let lower = line.to_ascii_lowercase();
    let is_error = lower.contains("error")
        || lower.contains(" err ")
        || lower.contains("failed")
        || lower.contains("panic")
        || lower.contains("fatal")
        || lower.contains("crit");
    let is_warn = lower.contains("warn");
    let is_info = lower.contains("info");

    match level {
        LogLevelFilter::All => true,
        LogLevelFilter::Error => is_error,
        LogLevelFilter::Warn => is_warn,
        LogLevelFilter::Info => is_info,
    }
}

/// Returns true if a log line looks like an error. Shared by spike detection and the UI.
pub(crate) fn is_error_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("error")
        || lower.contains(" err ")
        || lower.contains("failed")
        || lower.contains("panic")
        || lower.contains("fatal")
        || lower.contains("crit")
}
