use super::CommandProvider;
use crate::cli::ProcessSort;
use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::fs;
use sysinfo::System;

/// A single process row for display.
#[derive(Debug, Clone)]
pub struct ProcessRow {
    pub pid: String,
    pub parent_pid: Option<String>,
    pub user: String,
    pub service_group: String,
    pub container_group: String,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
    pub status: String,
    /// Non-None when the process looks suspicious, contains a short reason string.
    pub suspicious: Option<String>,
}

pub struct ProcessDetails {
    pub cmdline: String,
    pub environ: Vec<String>,
    pub maps: Vec<String>,
}

/// Collect the top `limit` processes sorted by `sort`.
pub fn collect_processes(system: &System, limit: usize, sort: ProcessSort) -> Vec<ProcessRow> {
    let mut processes: Vec<_> = system.processes().iter().collect();
    match sort {
        ProcessSort::Cpu => processes.sort_by(|a, b| {
            b.1.cpu_usage()
                .total_cmp(&a.1.cpu_usage())
                .then_with(|| a.1.name().cmp(b.1.name()))
        }),
        ProcessSort::Memory => processes.sort_by_key(|(_, process)| Reverse(process.memory())),
        ProcessSort::Pid => processes.sort_by_key(|(pid, _)| pid.as_u32()),
        ProcessSort::Name => processes.sort_by(|a, b| a.1.name().cmp(b.1.name())),
    }

    processes
        .into_iter()
        .take(limit)
        .map(|(pid, process)| {
            let pid_u32 = pid.as_u32();
            let (service_group, container_group) = linux_process_groups(pid_u32);
            let name = process.name().to_string_lossy().to_string();
            let suspicious = check_suspicious_process(pid_u32, &name);
            ProcessRow {
                pid: pid.to_string(),
                parent_pid: process.parent().map(|ppid| ppid.to_string()),
                user: process
                    .user_id()
                    .map(|uid| format!("{uid:?}"))
                    .unwrap_or_else(|| "-".into()),
                service_group,
                container_group,
                name,
                cpu: process.cpu_usage(),
                memory: process.memory(),
                status: format!("{:?}", process.status()),
                suspicious,
            }
        })
        .collect()
}

pub fn collect_open_files(pid: u32, limit: usize) -> Result<Vec<String>, String> {
    let path = format!("/proc/{pid}/fd");
    let entries = fs::read_dir(path).map_err(|error| error.to_string())?;

    let mut set = BTreeSet::new();
    for entry in entries.flatten() {
        if let Ok(target) = fs::read_link(entry.path()) {
            let value = target.to_string_lossy().to_string();
            if value.is_empty() {
                continue;
            }
            set.insert(value);
        }
    }
    Ok(set.into_iter().take(limit).collect())
}

pub fn collect_open_ports(provider: &dyn CommandProvider, pid: u32, limit: usize) -> Vec<String> {
    let output = provider.run("ss", &["-tunapH"]);
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.success {
        return Vec::new();
    }

    let needle = format!("pid={pid},");
    let mut rows = Vec::new();
    for line in output.stdout.lines() {
        if !line.contains(&needle) {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 6 {
            rows.push(line.to_string());
            continue;
        }
        rows.push(format!(
            "{} {} {} -> {}",
            cols[0], cols[1], cols[4], cols[5]
        ));
    }
    rows.into_iter().take(limit).collect()
}

pub fn collect_process_details(pid: u32, limit: usize) -> ProcessDetails {
    ProcessDetails {
        cmdline: read_cmdline(pid),
        environ: read_environ(pid, limit),
        maps: read_maps(pid, limit),
    }
}

fn read_cmdline(pid: u32) -> String {
    if let Ok(content) = fs::read(format!("/proc/{pid}/cmdline")) {
        return String::from_utf8_lossy(&content)
            .split('\0')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }
    "-".into()
}

fn read_environ(pid: u32, limit: usize) -> Vec<String> {
    let mut vars = Vec::new();
    if let Ok(content) = fs::read_to_string(format!("/proc/{pid}/environ")) {
        for line in content.split('\0').filter(|s| !s.is_empty()).take(limit) {
            vars.push(line.to_string());
        }
    }
    vars
}

fn read_maps(pid: u32, limit: usize) -> Vec<String> {
    let mut regions = Vec::new();
    if let Ok(content) = fs::read_to_string(format!("/proc/{pid}/maps")) {
        for line in content.lines().take(limit) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 6 {
                regions.push(format!("{} {}", parts[0], parts[5]));
            }
        }
    }
    regions
}

fn linux_process_groups(pid: u32) -> (String, String) {
    if !cfg!(target_os = "linux") {
        return ("-".into(), "-".into());
    }

    let path = format!("/proc/{pid}/cgroup");
    let Ok(content) = fs::read_to_string(path) else {
        return ("-".into(), "-".into());
    };

    let mut service = None;
    let mut container = None;

    for line in content.lines() {
        let mut parts = line.splitn(3, ':');
        let _hier = parts.next();
        let _controllers = parts.next();
        let Some(cgroup_path) = parts.next() else {
            continue;
        };

        let tokens: Vec<&str> = cgroup_path
            .split('/')
            .filter(|token| !token.is_empty())
            .collect();
        for token in &tokens {
            if service.is_none() && token.ends_with(".service") {
                service = Some((*token).to_string());
            }

            if container.is_none() && (token.starts_with("docker-") || token.starts_with("crio-")) {
                let trimmed = token
                    .trim_start_matches("docker-")
                    .trim_start_matches("crio-")
                    .trim_end_matches(".scope");
                container = Some(short_container_id(trimmed));
            }

            if container.is_none()
                && (token.contains("kubepods")
                    || token.contains("docker")
                    || token.contains("podman"))
            {
                // Look for a 64-char hex string in the token
                if let Some(pos) = token.find(|c: char| c.is_ascii_hexdigit()) {
                    let potential_id = &token[pos..];
                    if potential_id.len() >= 12
                        && potential_id.chars().take(12).all(|c| c.is_ascii_hexdigit())
                    {
                        container = Some(short_container_id(&potential_id[..12]));
                    }
                }
            }

            if container.is_none() && is_probable_container_id(token) {
                container = Some(short_container_id(token));
            }
        }
    }

    (
        service.unwrap_or_else(|| "-".into()),
        container.unwrap_or_else(|| "-".into()),
    )
}

pub fn find_top_offenders(processes: &[ProcessRow]) -> (Option<ProcessRow>, Option<ProcessRow>) {
    let cpu = processes
        .iter()
        .max_by(|a, b| a.cpu.total_cmp(&b.cpu))
        .cloned();
    let mem = processes.iter().max_by_key(|p| p.memory).cloned();
    (cpu, mem)
}

fn is_probable_container_id(token: &str) -> bool {
    let len = token.len();
    len >= 12 && token.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn short_container_id(value: &str) -> String {
    value.chars().take(12).collect()
}

// ---------------------------------------------------------------------------
// Suspicious process heuristics
// ---------------------------------------------------------------------------

/// Check a process for suspicious indicators via /proc only (no subprocesses).
///
/// Returns `Some(reason)` for the first matching heuristic, `None` if clean.
pub fn check_suspicious_process(pid: u32, name: &str) -> Option<String> {
    let exe_path = fs::read_link(format!("/proc/{pid}/exe"));

    // 1. Exe has been deleted from disk since the process started.
    if let Ok(ref path) = exe_path {
        let path_str = path.to_string_lossy();
        if path_str.ends_with(" (deleted)") {
            return Some("exe deleted".into());
        }
    }

    // 2. Process was launched from a world-writable / temp directory.
    if let Ok(ref path) = exe_path {
        let path_str = path.to_string_lossy();
        for prefix in &["/tmp/", "/dev/shm/", "/var/tmp/", "/run/shm/"] {
            if path_str.starts_with(prefix) {
                return Some(format!("launched from {}", prefix.trim_end_matches('/')));
            }
        }
    }

    // 3. Process name mismatches the exe basename (masquerading).
    //    Skip kernel threads (no exe) and short names (common false positives).
    if name.len() >= 4
        && let Ok(ref path) = exe_path
        && let Some(exe_base) = path.file_name()
    {
        let exe_name = exe_base.to_string_lossy();
        // Allow prefix match (e.g. "python" vs "python3.11")
        if !exe_name.starts_with(name) && !name.starts_with(exe_name.as_ref()) {
            return Some(format!("name '{}' != exe '{}'", name, exe_name));
        }
    }

    None
}
