use std::collections::HashMap;
use std::fs;
use std::process::Command as ProcessCommand;
use sysinfo::Disks;

/// A single disk/partition row for display.
#[derive(Debug, Clone)]
pub struct DiskRow {
    pub mount: String,
    pub filesystem: String,
    pub used: u64,
    pub total: u64,
    pub usage: f64,
    pub inode_used: Option<u64>,
    pub inode_total: Option<u64>,
    pub inode_usage: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct DiskIoRow {
    pub device: String,
    pub read_bps: u64,
    pub write_bps: u64,
}

#[derive(Debug, Clone, Default)]
pub struct SmartHealthRow {
    pub device: String,
    pub overall: String,
    pub temperature_c: Option<i32>,
    pub power_on_hours: Option<u64>,
}

pub type DiskIoCounters = HashMap<String, (u64, u64)>;

/// Collect all mounted disk partitions.
pub fn collect_disks() -> Vec<DiskRow> {
    let disks = Disks::new_with_refreshed_list();
    let inode_map = collect_inode_usage_map();
    disks
        .list()
        .iter()
        .map(|disk| {
            let total = disk.total_space();
            let used = total.saturating_sub(disk.available_space());
            let mount = disk.mount_point().display().to_string();
            let inode_data = inode_map.get(&mount).copied();
            DiskRow {
                mount,
                filesystem: disk.file_system().to_string_lossy().to_string(),
                used,
                total,
                usage: super::percentage(used, total),
                inode_used: inode_data.map(|(_, used, _)| used),
                inode_total: inode_data.map(|(total, _, _)| total),
                inode_usage: inode_data.map(|(_, _, usage)| usage),
            }
        })
        .collect()
}

fn collect_inode_usage_map() -> HashMap<String, (u64, u64, f64)> {
    if !cfg!(target_os = "linux") {
        return HashMap::new();
    }

    let output = ProcessCommand::new("df").args(["-Pi"]).output();
    let Ok(output) = output else {
        return HashMap::new();
    };
    if !output.status.success() {
        return HashMap::new();
    }

    let mut map = HashMap::new();
    for (index, line) in String::from_utf8_lossy(&output.stdout).lines().enumerate() {
        if index == 0 {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 6 {
            continue;
        }
        let inode_total = cols[1].parse::<u64>().ok();
        let inode_used = cols[2].parse::<u64>().ok();
        let inode_usage = cols[4].trim_end_matches('%').parse::<f64>().ok();
        let mount = cols[5].to_string();
        if let (Some(total), Some(used), Some(usage)) = (inode_total, inode_used, inode_usage) {
            map.insert(mount, (total, used, usage));
        }
    }
    map
}

pub fn collect_disk_io_rates(
    previous: &DiskIoCounters,
    elapsed_secs: f64,
) -> (Vec<DiskIoRow>, DiskIoCounters) {
    let Ok(contents) = fs::read_to_string("/proc/diskstats") else {
        return (Vec::new(), previous.clone());
    };

    let mut current: DiskIoCounters = HashMap::new();
    for line in contents.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 10 {
            continue;
        }
        let device = cols[2].to_string();
        // Keep only primary block devices to avoid noisy partition spam.
        if device.starts_with("loop") || device.starts_with("ram") || is_partition_name(&device) {
            continue;
        }
        let sectors_read = cols[5].parse::<u64>().unwrap_or(0);
        let sectors_written = cols[9].parse::<u64>().unwrap_or(0);
        current.insert(device, (sectors_read, sectors_written));
    }

    let mut rows = Vec::new();
    for (device, (read_sectors, write_sectors)) in &current {
        let (prev_read, prev_write) = previous.get(device).copied().unwrap_or((0, 0));
        let delta_read = read_sectors.saturating_sub(prev_read) * 512;
        let delta_write = write_sectors.saturating_sub(prev_write) * 512;
        let read_bps = (delta_read as f64 / elapsed_secs.max(1.0)) as u64;
        let write_bps = (delta_write as f64 / elapsed_secs.max(1.0)) as u64;
        rows.push(DiskIoRow {
            device: device.clone(),
            read_bps,
            write_bps,
        });
    }
    rows.sort_by(|a, b| {
        (b.read_bps + b.write_bps)
            .cmp(&(a.read_bps + a.write_bps))
            .then_with(|| a.device.cmp(&b.device))
    });
    (rows, current)
}

fn is_partition_name(name: &str) -> bool {
    // nvme0n1p2, mmcblk0p1, loop0p1
    if name.contains('p') && name.chars().last().is_some_and(|ch| ch.is_ascii_digit()) {
        return true;
    }
    // sda1, vda2, xvda3
    if name.chars().last().is_some_and(|ch| ch.is_ascii_digit())
        && (name.starts_with("sd") || name.starts_with("vd") || name.starts_with("xvd"))
    {
        return true;
    }
    false
}

pub fn collect_directory_sizes_with_depth(
    path: &str,
    depth: usize,
    limit: usize,
) -> Vec<(String, u64)> {
    let output = ProcessCommand::new("du")
        .args(["-xb", &format!("--max-depth={depth}"), path])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let mut rows = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((size_str, dir)) = line.split_once('\t') else {
            continue;
        };
        if dir == path {
            continue;
        }
        let size = size_str.trim().parse::<u64>().unwrap_or(0);
        rows.push((dir.to_string(), size));
    }
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.into_iter().take(limit).collect()
}

pub fn collect_large_files(path: &str, limit: usize) -> Vec<(String, u64)> {
    let output = ProcessCommand::new("sh")
        .args([
            "-c",
            "find \"$1\" -xdev -type f -printf '%s\\t%p\\n' 2>/dev/null | sort -nr | head -n \"$2\"",
            "--", // $0
            path, // $1
            &limit.to_string(), // $2
        ])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let mut rows = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((size_str, file)) = line.split_once('\t') else {
            continue;
        };
        if let Ok(size) = size_str.trim().parse::<u64>() {
            rows.push((file.to_string(), size));
        }
    }
    rows
}

pub fn collect_smart_health(limit: usize) -> Vec<SmartHealthRow> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }
    if !command_exists("smartctl") {
        return Vec::new();
    }

    let scan = ProcessCommand::new("smartctl")
        .args(["--scan-open"])
        .output();
    let Ok(scan) = scan else {
        return Vec::new();
    };
    if !scan.status.success() {
        return Vec::new();
    }

    let mut rows = Vec::new();
    for line in String::from_utf8_lossy(&scan.stdout).lines().take(limit) {
        let device = line.split_whitespace().next().unwrap_or("");
        if device.is_empty() {
            continue;
        }
        rows.push(read_single_smart_health(device));
    }
    rows
}

fn read_single_smart_health(device: &str) -> SmartHealthRow {
    let output = ProcessCommand::new("smartctl")
        .args(["-H", "-A", device])
        .output();

    let mut row = SmartHealthRow {
        device: device.to_string(),
        overall: "unknown".into(),
        temperature_c: None,
        power_on_hours: None,
    };

    let Ok(output) = output else {
        row.overall = "unavailable".into();
        return row;
    };
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("overall-health") || lower.contains("smart health status") {
            row.overall = line
                .split(':')
                .next_back()
                .map(|value| value.trim().to_string())
                .unwrap_or_else(|| line.trim().to_string());
        } else if lower.contains("temperature_celsius") || lower.contains("temperature:") {
            let value = line
                .split_whitespace()
                .rev()
                .find_map(|part| part.parse::<i32>().ok());
            if value.is_some() {
                row.temperature_c = value;
            }
        } else if lower.contains("power_on_hours") || lower.contains("power on hours") {
            let value = line
                .split_whitespace()
                .rev()
                .find_map(|part| part.parse::<u64>().ok());
            if value.is_some() {
                row.power_on_hours = value;
            }
        }
    }

    if row.overall == "unknown" {
        row.overall = if output.status.success() {
            "available".into()
        } else {
            "failed".into()
        };
    }
    row
}

fn command_exists(binary: &str) -> bool {
    ProcessCommand::new("which")
        .arg(binary)
        .output()
        .is_ok_and(|output| output.status.success())
}
