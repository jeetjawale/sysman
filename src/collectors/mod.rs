pub mod host;
pub mod netstat;
pub mod procs;
pub mod storage;
pub mod systemd;

// Re-export core types
pub use storage::DiskRow;
pub use netstat::ConnectionRow;
pub use procs::ProcessRow;
pub use systemd::{ServiceRow, ServiceSummary};

use crate::cli::{ProcessSort, ServiceState};
use anyhow::Result;
use sysinfo::{ProcessesToUpdate, System};

/// Complete system snapshot for TUI display.
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

/// Collect a full system snapshot for display.
pub fn collect_snapshot(sys: &mut System, service_state: ServiceState, process_limit: usize) -> Result<Snapshot> {
     sys.refresh_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.refresh_cpu_usage();

    let disks = storage::collect_disks();
    let processes = procs::collect_processes(&sys, process_limit, ProcessSort::Cpu);
    let services = systemd::collect_services(service_state, 50).unwrap_or_default();
    let service_summary = if cfg!(target_os = "linux") {
        systemd::count_systemd_services()
            .ok()
            .map(|(running, failed)| ServiceSummary { running, failed })
    } else {
        None
    };
    let load = System::load_average();

    Ok(Snapshot {
        host: host::host_name(),
        os: host::os_label(),
        kernel: System::kernel_version().unwrap_or_else(|| "unknown".into()),
        distro: host::linux_distribution().unwrap_or_else(|| "unknown".into()),
        uptime: System::uptime(),
        boot_time: System::boot_time(),
        cpu_usage: sys.global_cpu_usage(),
        cpu_cores: sys.cpus().len(),
        used_memory: sys.used_memory(),
        total_memory: sys.total_memory(),
        available_memory: sys.available_memory(),
        cached_memory: host::linux_cached_memory().unwrap_or(0),
        used_swap: sys.used_swap(),
        total_swap: sys.total_swap(),
        process_count: sys.processes().len(),
        load_average: format!("{:.2} / {:.2} / {:.2}", load.one, load.five, load.fifteen),
        cpu_per_core: sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect(),
        disks,
        processes,
        services,
        service_summary,
    })
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

/// Format byte count as human-readable string (e.g. "1.5 GB").
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

/// Format seconds as "Xd Yh Zm" duration string.
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

/// Compute usage percentage safely (handles zero total).
pub fn percentage(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        used as f64 * 100.0 / total as f64
    }
}

/// Truncate string with "…" suffix if too long.
pub fn truncate(value: &str, max_len: usize) -> String {
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
    use super::*;

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
