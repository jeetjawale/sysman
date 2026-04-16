use crate::cli::{Command, ProcessSort, ServiceAction, ServiceState};
use anyhow::{Context, Result, anyhow, bail};
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fs;
use std::process::Command as ProcessCommand;
use sysinfo::{Disks, Pid, ProcessesToUpdate, System};

pub fn execute(command: Command) -> Result<()> {
    match command {
        Command::Tui => Ok(()),
        Command::Summary => print_summary(),
        Command::System => print_system(),
        Command::Memory => print_memory(),
        Command::Disks => print_disks(),
        Command::Processes { limit, sort } => print_processes(limit, sort),
        Command::Services { state, limit } => print_services(state, limit),
        Command::Service { name, action } => handle_service_action(&name, action),
    }
}

fn print_summary() -> Result<()> {
    let snapshot = collect_snapshot(ServiceState::Running, 10)?;

    println!("System Summary");
    println!("==============");
    println!("Host: {}", snapshot.host);
    println!("OS: {}", snapshot.os);
    println!("Kernel: {}", snapshot.kernel);
    println!("Uptime: {}", format_duration(snapshot.uptime));
    println!(
        "CPU: {:.1}% total usage across {} cores",
        snapshot.cpu_usage, snapshot.cpu_cores
    );
    println!(
        "Memory: {} / {} used",
        format_bytes(snapshot.used_memory),
        format_bytes(snapshot.total_memory)
    );
    println!(
        "Swap: {} / {} used",
        format_bytes(snapshot.used_swap),
        format_bytes(snapshot.total_swap)
    );
    match snapshot
        .disks
        .iter()
        .max_by(|a, b| a.usage.total_cmp(&b.usage))
    {
        Some(disk) => {
            println!(
                "Disks: {} mounted, busiest {} at {:.1}%",
                snapshot.disks.len(),
                disk.mount,
                disk.usage
            );
        }
        None => println!("Disks: {} mounted", snapshot.disks.len()),
    }

    if let Some(service_summary) = snapshot.service_summary {
        println!(
            "Services: {} running, {} failed",
            service_summary.running, service_summary.failed
        );
    } else {
        println!("Services: systemd data unavailable");
    }

    Ok(())
}

fn print_system() -> Result<()> {
    let mut system = System::new_all();
    system.refresh_all();

    println!("System Information");
    println!("==================");
    println!("Host Name: {}", host_name());
    println!(
        "OS Version: {}",
        System::long_os_version().unwrap_or_else(|| "unknown".into())
    );
    println!(
        "Distribution: {}",
        linux_distribution().unwrap_or_else(|| "unknown".into())
    );
    println!(
        "Kernel: {}",
        System::kernel_version().unwrap_or_else(|| "unknown".into())
    );
    println!("Architecture: {}", std::env::consts::ARCH);
    println!("Uptime: {}", format_duration(System::uptime()));
    println!("Boot Time: {}", System::boot_time());
    println!("CPU Cores: {}", system.cpus().len());
    println!("Process Count: {}", system.processes().len());

    Ok(())
}

fn print_memory() -> Result<()> {
    let mut system = System::new_all();
    system.refresh_memory();

    println!("Memory");
    println!("======");
    println!(
        "RAM Used: {} / {} ({:.1}%)",
        format_bytes(system.used_memory()),
        format_bytes(system.total_memory()),
        percentage(system.used_memory(), system.total_memory())
    );
    println!("RAM Available: {}", format_bytes(system.available_memory()));
    println!(
        "Swap Used: {} / {} ({:.1}%)",
        format_bytes(system.used_swap()),
        format_bytes(system.total_swap()),
        percentage(system.used_swap(), system.total_swap())
    );

    Ok(())
}

fn print_disks() -> Result<()> {
    let disks = Disks::new_with_refreshed_list();
    println!("Disks");
    println!("=====");
    println!(
        "{:<20} {:<10} {:>12} {:>12} {:>8}",
        "Mount", "FS", "Used", "Total", "Use%"
    );

    for disk in disks.list() {
        let total = disk.total_space();
        let available = disk.available_space();
        let used = total.saturating_sub(available);
        let fs = disk.file_system().to_string_lossy();
        println!(
            "{:<20} {:<10} {:>12} {:>12} {:>7.1}",
            disk.mount_point().display(),
            fs,
            format_bytes(used),
            format_bytes(total),
            percentage(used, total)
        );
    }

    Ok(())
}

fn print_processes(limit: usize, sort: ProcessSort) -> Result<()> {
    let processes = collect_processes(limit, sort);

    println!("Processes");
    println!("=========");
    println!(
        "{:<8} {:<28} {:>8} {:>12} {:>10}",
        "PID", "Name", "CPU%", "Memory", "Status"
    );

    for process in processes {
        println!(
            "{:<8} {:<28} {:>8.1} {:>12} {:>10}",
            process.pid,
            truncate(&process.name, 28),
            process.cpu,
            format_bytes(process.memory),
            process.status
        );
    }

    Ok(())
}

fn print_services(state: ServiceState, limit: usize) -> Result<()> {
    let services = collect_services(state, limit)?;

    println!("Services");
    println!("========");
    println!("{:<40} {:<12} {:<12}", "Name", "Active", "Sub");

    for service in services {
        println!(
            "{:<40} {:<12} {:<12}",
            service.name, service.active, service.sub
        );
    }

    Ok(())
}

fn handle_service_action(name: &str, action: ServiceAction) -> Result<()> {
    ensure_linux_systemd()?;

    let action_name = match action {
        ServiceAction::Status => "status",
        ServiceAction::Start => "start",
        ServiceAction::Stop => "stop",
        ServiceAction::Restart => "restart",
    };

    let output = run_systemctl(&[action_name, name])?;
    println!("{output}");
    Ok(())
}

fn ensure_linux_systemd() -> Result<()> {
    if !cfg!(target_os = "linux") {
        bail!("service management is currently supported on Linux hosts only");
    }

    Ok(())
}

fn run_systemctl(args: &[&str]) -> Result<String> {
    let output = ProcessCommand::new("systemctl")
        .args(args)
        .output()
        .with_context(|| format!("failed to invoke systemctl with args: {}", args.join(" ")))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(anyhow!(
            "systemctl {} failed: {}",
            args.join(" "),
            if stderr.is_empty() {
                "unknown error"
            } else {
                &stderr
            }
        ))
    }
}

fn count_systemd_services() -> Result<(usize, usize)> {
    let running = run_systemctl(&[
        "list-units",
        "--type=service",
        "--state=running",
        "--no-legend",
        "--no-pager",
    ])?;
    let failed = run_systemctl(&[
        "list-units",
        "--type=service",
        "--state=failed",
        "--no-legend",
        "--no-pager",
    ])?;

    Ok((
        count_nonempty_lines(&running),
        count_nonempty_lines(&failed),
    ))
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub host: String,
    pub os: String,
    pub kernel: String,
    pub distro: String,
    pub uptime: u64,
    pub boot_time: u64,
    pub cpu_usage: f32,
    pub cpu_cores: usize,
    pub used_memory: u64,
    pub total_memory: u64,
    pub available_memory: u64,
    pub cached_memory: u64,
    pub used_swap: u64,
    pub total_swap: u64,
    pub process_count: usize,
    pub load_average: String,
    pub cpu_per_core: Vec<f32>,
    pub disks: Vec<DiskRow>,
    pub processes: Vec<ProcessRow>,
    pub services: Vec<ServiceRow>,
    pub service_summary: Option<ServiceSummary>,
}

#[derive(Debug, Clone)]
pub struct DiskRow {
    pub mount: String,
    pub filesystem: String,
    pub used: u64,
    pub total: u64,
    pub usage: f64,
}

#[derive(Debug, Clone)]
pub struct ProcessRow {
    pub pid: String,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub name: String,
    pub active: String,
    pub sub: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceSummary {
    pub running: usize,
    pub failed: usize,
}

#[derive(Debug, Clone)]
pub struct ConnectionRow {
    pub proto: String,
    pub state: String,
    pub local: String,
    pub remote: String,
    pub process: String,
}

pub fn collect_snapshot(service_state: ServiceState, process_limit: usize) -> Result<Snapshot> {
    let mut system = System::new_all();
    system.refresh_all();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let disks = collect_disks();
    let processes = collect_processes(process_limit, ProcessSort::Cpu);
    let services = collect_services(service_state, 50).unwrap_or_default();
    let service_summary = if cfg!(target_os = "linux") {
        count_systemd_services()
            .ok()
            .map(|(running, failed)| ServiceSummary { running, failed })
    } else {
        None
    };
    let load = System::load_average();

    Ok(Snapshot {
        host: host_name(),
        os: os_label(),
        kernel: System::kernel_version().unwrap_or_else(|| "unknown".into()),
        distro: linux_distribution().unwrap_or_else(|| "unknown".into()),
        uptime: System::uptime(),
        boot_time: System::boot_time(),
        cpu_usage: system.global_cpu_usage(),
        cpu_cores: system.cpus().len(),
        used_memory: system.used_memory(),
        total_memory: system.total_memory(),
        available_memory: system.available_memory(),
        cached_memory: linux_cached_memory().unwrap_or(0),
        used_swap: system.used_swap(),
        total_swap: system.total_swap(),
        process_count: system.processes().len(),
        load_average: format!("{:.2} / {:.2} / {:.2}", load.one, load.five, load.fifteen),
        cpu_per_core: system.cpus().iter().map(|cpu| cpu.cpu_usage()).collect(),
        disks,
        processes,
        services,
        service_summary,
    })
}

pub fn collect_disks() -> Vec<DiskRow> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .map(|disk| {
            let total = disk.total_space();
            let used = total.saturating_sub(disk.available_space());
            DiskRow {
                mount: disk.mount_point().display().to_string(),
                filesystem: disk.file_system().to_string_lossy().to_string(),
                used,
                total,
                usage: percentage(used, total),
            }
        })
        .collect()
}

pub fn collect_processes(limit: usize, sort: ProcessSort) -> Vec<ProcessRow> {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let mut processes: Vec<_> = system.processes().iter().collect();
    match sort {
        ProcessSort::Cpu => processes.sort_by(|a, b| {
            b.1.cpu_usage()
                .total_cmp(&a.1.cpu_usage())
                .then_with(|| a.1.name().cmp(b.1.name()))
        }),
        ProcessSort::Memory => processes.sort_by_key(|(_, process)| Reverse(process.memory())),
        ProcessSort::Name => processes.sort_by(|a, b| a.1.name().cmp(b.1.name())),
    }

    processes
        .into_iter()
        .take(limit)
        .map(|(pid, process)| ProcessRow {
            pid: format_pid(*pid),
            name: process.name().to_string_lossy().to_string(),
            cpu: process.cpu_usage(),
            memory: process.memory(),
            status: format!("{:?}", process.status()),
        })
        .collect()
}

pub fn collect_services(state: ServiceState, limit: usize) -> Result<Vec<ServiceRow>> {
    ensure_linux_systemd()?;

    let lines = run_systemctl(&[
        "list-units",
        "--type=service",
        "--all",
        "--no-legend",
        "--no-pager",
    ])?;

    let services = lines
        .lines()
        .filter_map(parse_service_line)
        .filter(|service| match state {
            ServiceState::Running => service.active == "active" && service.sub == "running",
            ServiceState::Failed => service.active == "failed",
            ServiceState::All => true,
        })
        .take(limit)
        .collect();

    Ok(services)
}

pub fn collect_connections(limit: usize) -> Vec<ConnectionRow> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }

    let output = ProcessCommand::new("ss").args(["-tunapH"]).output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_connection_line)
        .take(limit)
        .collect()
}

pub fn collect_interface_addresses() -> BTreeMap<String, Vec<String>> {
    if !cfg!(target_os = "linux") {
        return BTreeMap::new();
    }

    let output = ProcessCommand::new("ip")
        .args(["-o", "addr", "show"])
        .output();
    let Ok(output) = output else {
        return BTreeMap::new();
    };
    if !output.status.success() {
        return BTreeMap::new();
    }

    let mut interfaces: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 4 {
            continue;
        }
        let name = cols[1].trim_end_matches(':').to_string();
        let family = cols[2];
        let address = cols[3];
        if family == "inet" || family == "inet6" {
            interfaces
                .entry(name)
                .or_default()
                .push(address.to_string());
        }
    }
    interfaces
}

fn parse_service_line(line: &str) -> Option<ServiceRow> {
    if line.trim().is_empty() {
        return None;
    }
    let cols: Vec<&str> = line.split_whitespace().collect();
    if cols.len() < 4 {
        return None;
    }
    Some(ServiceRow {
        name: cols[0].to_string(),
        active: cols[2].to_string(),
        sub: cols[3].to_string(),
    })
}

fn parse_connection_line(line: &str) -> Option<ConnectionRow> {
    let cols: Vec<&str> = line.split_whitespace().collect();
    if cols.len() < 6 {
        return None;
    }

    let process = cols
        .iter()
        .find(|col| col.contains("users:("))
        .map(|value| (*value).to_string())
        .unwrap_or_else(|| "-".into());

    Some(ConnectionRow {
        proto: cols[0].to_string(),
        state: cols[1].to_string(),
        local: cols[4].to_string(),
        remote: cols[5].to_string(),
        process,
    })
}

fn count_nonempty_lines(text: &str) -> usize {
    text.lines().filter(|line| !line.trim().is_empty()).count()
}

fn host_name() -> String {
    System::host_name().unwrap_or_else(|| "unknown".into())
}

fn os_label() -> String {
    let name = System::name().unwrap_or_else(|| "unknown".into());
    let version = System::os_version().unwrap_or_else(|| "unknown".into());
    format!("{name} {version}")
}

fn linux_distribution() -> Option<String> {
    let content = fs::read_to_string("/etc/os-release").ok()?;
    let pretty_name = content.lines().find_map(|line| {
        line.strip_prefix("PRETTY_NAME=")
            .map(|value| value.trim_matches('"').to_string())
    })?;
    Some(pretty_name)
}

fn linux_cached_memory() -> Option<u64> {
    let content = fs::read_to_string("/proc/meminfo").ok()?;
    content.lines().find_map(|line| {
        let value = line.strip_prefix("Cached:")?;
        let kb = value.split_whitespace().next()?.parse::<u64>().ok()?;
        Some(kb * 1024)
    })
}

fn format_pid(pid: Pid) -> String {
    pid.to_string()
}

fn percentage(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        used as f64 * 100.0 / total as f64
    }
}

pub fn format_duration(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
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

#[cfg(test)]
mod tests {
    use super::{format_bytes, format_duration, percentage, truncate};

    #[test]
    fn formats_bytes_in_human_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn formats_duration() {
        assert_eq!(format_duration(59), "0m");
        assert_eq!(format_duration(3_661), "1h 1m");
        assert_eq!(format_duration(90_061), "1d 1h 1m");
    }

    #[test]
    fn computes_percentages_safely() {
        assert_eq!(percentage(10, 0), 0.0);
        assert_eq!(percentage(50, 200), 25.0);
    }

    #[test]
    fn truncates_long_values() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdefghijkl", 6), "abcde…");
    }
}
