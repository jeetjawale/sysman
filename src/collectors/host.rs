use std::fs;
use sysinfo::System;

/// System hostname.
pub fn host_name() -> String {
    System::host_name().unwrap_or_else(|| "unknown".into())
}

/// "Linux 6.x" style OS label.
pub fn os_label() -> String {
    let name = System::name().unwrap_or_else(|| "unknown".into());
    let version = System::os_version().unwrap_or_else(|| "unknown".into());
    format!("{name} {version}")
}

/// Parse PRETTY_NAME from /etc/os-release.
pub fn linux_distribution() -> Option<String> {
    let content = fs::read_to_string("/etc/os-release").ok()?;
    let pretty_name = content.lines().find_map(|line| {
        line.strip_prefix("PRETTY_NAME=")
            .map(|value| value.trim_matches('"').to_string())
    })?;
    Some(pretty_name)
}

/// Parse "Cached:" from /proc/meminfo (bytes).
pub fn linux_cached_memory() -> Option<u64> {
    let content = fs::read_to_string("/proc/meminfo").ok()?;
    content.lines().find_map(|line| {
        let value = line.strip_prefix("Cached:")?;
        let kb = value.split_whitespace().next()?.parse::<u64>().ok()?;
        Some(kb * 1024)
    })
}
