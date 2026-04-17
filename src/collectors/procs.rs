use crate::cli::ProcessSort;
use std::cmp::Reverse;
use sysinfo::{ProcessesToUpdate, System};

/// A single process row for display.
#[derive(Debug, Clone)]
pub struct ProcessRow {
    pub pid: String,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
    pub status: String,
}

/// Collect the top `limit` processes sorted by `sort`.
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
            pid: pid.to_string(),
            name: process.name().to_string_lossy().to_string(),
            cpu: process.cpu_usage(),
            memory: process.memory(),
            status: format!("{:?}", process.status()),
        })
        .collect()
}
