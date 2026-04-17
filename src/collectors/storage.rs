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
