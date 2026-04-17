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
}

#[derive(Debug, Clone, Default)]
pub struct DiskIoRow {
    pub device: String,
    pub read_bps: u64,
    pub write_bps: u64,
}

pub type DiskIoCounters = HashMap<String, (u64, u64)>;

/// Collect all mounted disk partitions.
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
                usage: super::percentage(used, total),
            }
        })
        .collect()
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

pub fn collect_directory_sizes(path: &str, limit: usize) -> Vec<(String, u64)> {
    let output = ProcessCommand::new("du")
        .args(["-xb", "--max-depth=1", path])
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
