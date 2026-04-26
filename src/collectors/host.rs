use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;
use std::{cmp::Reverse, collections::HashMap};
use sysinfo::System;

#[derive(Debug, Clone, Default)]
pub struct CpuRuntimeInfo {
    pub current_freq_mhz: Option<u64>,
    pub governor: Option<String>,
    pub context_switches: Option<u64>,
    pub throttle_count: Option<u64>,
    pub temperature_c: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryPressureInfo {
    pub some_avg10: f64,
    pub some_avg60: f64,
    pub full_avg10: f64,
    pub full_avg60: f64,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryPageFaultInfo {
    pub minor: u64,
    pub major: u64,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryRuntimeInfo {
    pub pressure: Option<MemoryPressureInfo>,
    pub page_faults: Option<MemoryPageFaultInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct HardwareInfo {
    pub cpu_model: String,
    pub cpu_arch: String,
    pub cpu_cache: String,
    pub temperatures: Vec<String>,
    pub gpu_info: Vec<String>,
    pub battery_info: Vec<String>,
    pub login_users: Vec<String>,
    pub login_history: Vec<String>,
    pub ssh_sessions: Vec<String>,
    pub failed_logins: Vec<String>,
    pub firewall_status: Vec<String>,
    pub security_modules: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GpuRuntimeDevice {
    pub index: u32,
    pub uuid: Option<String>,
    pub name: String,
    pub utilization_pct: Option<f64>,
    pub memory_used_mib: Option<u64>,
    pub memory_total_mib: Option<u64>,
    pub temperature_c: Option<f64>,
    pub power_w: Option<f64>,
    pub fan_pct: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct GpuProcessRow {
    pub gpu_index: Option<u32>,
    pub pid: u32,
    pub process_name: String,
    pub used_memory_mib: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct GpuRuntimeInfo {
    pub backend: String,
    pub devices: Vec<GpuRuntimeDevice>,
    pub processes: Vec<GpuProcessRow>,
}

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

pub fn collect_hardware_info() -> HardwareInfo {
    HardwareInfo {
        cpu_model: cpu_model().unwrap_or_else(|| "unknown".into()),
        cpu_arch: std::env::consts::ARCH.into(),
        cpu_cache: cpu_cache().unwrap_or_else(|| "unknown".into()),
        temperatures: collect_temperatures(),
        gpu_info: collect_gpu_info(),
        battery_info: collect_battery_info(),
        login_users: collect_logged_in_users(),
        login_history: collect_login_history(),
        ssh_sessions: collect_ssh_sessions(),
        failed_logins: collect_failed_logins(),
        firewall_status: collect_firewall_status(),
        security_modules: collect_security_modules(),
    }
}

pub fn collect_cpu_runtime_info() -> CpuRuntimeInfo {
    CpuRuntimeInfo {
        current_freq_mhz: read_cpu_frequency_mhz(),
        governor: read_cpu_governor(),
        context_switches: read_context_switches(),
        throttle_count: read_cpu_throttle_count(),
        temperature_c: read_cpu_temperature_c(),
    }
}

pub fn collect_memory_runtime_info() -> MemoryRuntimeInfo {
    MemoryRuntimeInfo {
        pressure: read_memory_pressure(),
        page_faults: read_memory_page_faults(),
    }
}

pub fn collect_gpu_runtime_info() -> GpuRuntimeInfo {
    if command_exists("nvidia-smi")
        && let Some((devices, processes)) = collect_nvidia_gpu_runtime()
    {
        return GpuRuntimeInfo {
            backend: "nvidia-smi".into(),
            devices,
            processes,
        };
    }

    if command_exists("rocm-smi")
        && let Some(devices) = collect_rocm_gpu_runtime()
        && !devices.is_empty()
    {
        return GpuRuntimeInfo {
            backend: "rocm-smi".into(),
            devices,
            processes: Vec::new(), // ROCm per-process VRAM requires root; skip
        };
    }

    GpuRuntimeInfo {
        backend: "unavailable".into(),
        devices: Vec::new(),
        processes: Vec::new(),
    }
}

fn cpu_model() -> Option<String> {
    let content = fs::read_to_string("/proc/cpuinfo").ok()?;
    content.lines().find_map(|line| {
        line.split_once(':').and_then(|(k, v)| {
            (k.trim() == "model name")
                .then(|| v.trim().to_string())
                .filter(|v| !v.is_empty())
        })
    })
}

fn read_cpu_frequency_mhz() -> Option<u64> {
    let khz = read_u64_file("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
        .or_else(|| read_u64_file("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_cur_freq"))?;
    Some((khz / 1000).max(1))
}

fn read_cpu_governor() -> Option<String> {
    let value = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .ok()
        .map(|v| v.trim().to_string())?;
    (!value.is_empty()).then_some(value)
}

fn read_context_switches() -> Option<u64> {
    let content = fs::read_to_string("/proc/stat").ok()?;
    content.lines().find_map(|line| {
        line.strip_prefix("ctxt ")
            .and_then(|value| value.trim().parse::<u64>().ok())
    })
}

fn read_cpu_throttle_count() -> Option<u64> {
    let entries = fs::read_dir("/sys/devices/system/cpu").ok()?;
    let mut total = 0u64;
    let mut found = false;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("cpu")
            || name[3..].is_empty()
            || !name[3..].chars().all(|value| value.is_ascii_digit())
        {
            continue;
        }
        let throttle_dir = entry.path().join("thermal_throttle");
        for file in ["core_throttle_count", "package_throttle_count"] {
            if let Some(value) = read_u64_path(&throttle_dir.join(file)) {
                total = total.saturating_add(value);
                found = true;
            }
        }
    }

    found.then_some(total)
}

fn read_cpu_temperature_c() -> Option<f64> {
    let entries = fs::read_dir("/sys/class/thermal").ok()?;
    let mut fallback = None;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("thermal_zone") {
            continue;
        }
        let zone = entry.path();
        let Some(raw) = read_u64_path(&zone.join("temp")) else {
            continue;
        };
        let celsius = if raw > 1000 {
            raw as f64 / 1000.0
        } else {
            raw as f64
        };
        let zone_type = fs::read_to_string(zone.join("type"))
            .ok()
            .map(|value| value.to_lowercase())
            .unwrap_or_default();
        if zone_type.contains("cpu")
            || zone_type.contains("pkg")
            || zone_type.contains("x86_pkg_temp")
            || zone_type.contains("tctl")
            || zone_type.contains("soc")
        {
            return Some(celsius);
        }
        if fallback.is_none() {
            fallback = Some(celsius);
        }
    }

    fallback
}

fn cpu_cache() -> Option<String> {
    let content = fs::read_to_string("/proc/cpuinfo").ok()?;
    content.lines().find_map(|line| {
        line.split_once(':').and_then(|(k, v)| {
            (k.trim() == "cache size")
                .then(|| v.trim().to_string())
                .filter(|v| !v.is_empty())
        })
    })
}

fn collect_temperatures() -> Vec<String> {
    let mut rows = Vec::new();
    if command_exists("sensors")
        && let Ok(output) = ProcessCommand::new("sensors").output()
        && output.status.success()
    {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let trimmed = line.trim();
            if trimmed.contains("°C") && !trimmed.is_empty() {
                rows.push(trimmed.to_string());
            }
            if rows.len() >= 6 {
                break;
            }
        }
    }

    if rows.is_empty() {
        // Fallback to /sys thermal zones.
        let Ok(entries) = fs::read_dir("/sys/class/thermal") else {
            return rows;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("thermal_zone") {
                continue;
            }
            let base = entry.path();
            let temp = fs::read_to_string(base.join("temp")).ok();
            let zone_type = fs::read_to_string(base.join("type"))
                .ok()
                .map(|value| value.trim().to_string())
                .unwrap_or_else(|| name.to_string());
            if let Some(temp) = temp
                && let Ok(raw) = temp.trim().parse::<i64>()
            {
                let c = raw as f64 / 1000.0;
                rows.push(format!("{zone_type}: {c:.1}°C"));
            }
            if rows.len() >= 6 {
                break;
            }
        }
    }
    rows
}

fn collect_gpu_info() -> Vec<String> {
    if command_exists("nvidia-smi")
        && let Ok(output) = ProcessCommand::new("nvidia-smi")
            .args([
                "--query-gpu=name,temperature.gpu,utilization.gpu,memory.used,memory.total,power.draw",
                "--format=csv,noheader,nounits",
            ])
            .output()
        && output.status.success()
    {
        let mut rows = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines().take(4) {
            let cols: Vec<&str> = line.split(',').map(|part| part.trim()).collect();
            if cols.len() >= 6 {
                rows.push(format!(
                    "{} {}°C util {}% mem {}/{} MiB power {}W",
                    cols[0], cols[1], cols[2], cols[3], cols[4], cols[5]
                ));
            } else {
                rows.push(line.trim().to_string());
            }
        }
        return rows;
    }

    if command_exists("rocm-smi")
        && let Ok(output) = ProcessCommand::new("rocm-smi")
            .args([
                "--showproductname",
                "--showtemp",
                "--showuse",
                "--showpower",
            ])
            .output()
        && output.status.success()
    {
        return String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.contains("GPU") || line.contains("card"))
            .take(6)
            .map(|line| line.trim().to_string())
            .collect();
    }

    if command_exists("lspci")
        && let Ok(output) = ProcessCommand::new("lspci").output()
        && output.status.success()
    {
        let rows: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| {
                line.contains("VGA compatible controller") || line.contains("3D controller")
            })
            .take(4)
            .map(|line| line.trim().to_string())
            .collect();
        return rows;
    }

    Vec::new()
}

fn collect_nvidia_gpu_runtime() -> Option<(Vec<GpuRuntimeDevice>, Vec<GpuProcessRow>)> {
    let devices_output = ProcessCommand::new("nvidia-smi")
        .args([
            "--query-gpu=index,uuid,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,fan.speed",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !devices_output.status.success() {
        return None;
    }

    let mut devices = Vec::new();
    for line in String::from_utf8_lossy(&devices_output.stdout).lines() {
        let cols: Vec<&str> = line.split(',').map(|part| part.trim()).collect();
        if cols.len() < 9 {
            continue;
        }
        let index = cols[0].parse::<u32>().unwrap_or(0);
        devices.push(GpuRuntimeDevice {
            index,
            uuid: parse_optional_text(cols[1]),
            name: cols[2].to_string(),
            utilization_pct: parse_optional_f64(cols[3]),
            memory_used_mib: parse_optional_u64(cols[4]),
            memory_total_mib: parse_optional_u64(cols[5]),
            temperature_c: parse_optional_f64(cols[6]),
            power_w: parse_optional_f64(cols[7]),
            fan_pct: parse_optional_f64(cols[8]),
        });
    }

    let mut uuid_index_map = HashMap::new();
    for device in &devices {
        if let Some(uuid) = &device.uuid {
            uuid_index_map.insert(uuid.clone(), device.index);
        }
    }

    let proc_output = ProcessCommand::new("nvidia-smi")
        .args([
            "--query-compute-apps=gpu_uuid,pid,process_name,used_gpu_memory",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok();
    let mut processes = Vec::new();
    if let Some(output) = proc_output
        && output.status.success()
    {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let cols: Vec<&str> = line.split(',').map(|part| part.trim()).collect();
            if cols.len() < 4 {
                continue;
            }
            let pid = cols[1].parse::<u32>().unwrap_or(0);
            if pid == 0 {
                continue;
            }
            let used_memory_mib = parse_optional_u64(cols[3]);
            processes.push(GpuProcessRow {
                gpu_index: uuid_index_map.get(cols[0]).copied(),
                pid,
                process_name: parse_optional_text(cols[2]).unwrap_or_else(|| "-".into()),
                used_memory_mib,
            });
        }
    }
    if processes.is_empty() {
        processes.extend(collect_nvidia_pmon_processes());
    }
    processes.sort_by_key(|row| Reverse(row.used_memory_mib.unwrap_or(0)));

    Some((devices, processes))
}

fn collect_nvidia_pmon_processes() -> Vec<GpuProcessRow> {
    let output = ProcessCommand::new("nvidia-smi")
        .args(["pmon", "-c", "1"])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let mut rows = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 3 {
            continue;
        }
        let Ok(gpu_index) = cols[0].parse::<u32>() else {
            continue;
        };
        let Ok(pid) = cols[1].parse::<u32>() else {
            continue;
        };
        if pid == 0 {
            continue;
        }
        let process_name = read_process_name_by_pid(pid)
            .unwrap_or_else(|| cols.last().unwrap_or(&"-").to_string());
        rows.push(GpuProcessRow {
            gpu_index: Some(gpu_index),
            pid,
            process_name,
            used_memory_mib: None,
        });
    }
    rows
}

fn collect_battery_info() -> Vec<String> {
    let mut rows = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/power_supply") else {
        return rows;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let supply_type = fs::read_to_string(path.join("type"))
            .ok()
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        if supply_type != "Battery" {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let status = fs::read_to_string(path.join("status"))
            .ok()
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|| "unknown".into());
        let capacity = fs::read_to_string(path.join("capacity"))
            .ok()
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|| "?".into());

        let power_mw = fs::read_to_string(path.join("power_now"))
            .ok()
            .and_then(|value| value.trim().parse::<f64>().ok().map(|v| v / 1_000_000.0))
            .or_else(|| {
                fs::read_to_string(path.join("current_now"))
                    .ok()
                    .and_then(|value| value.trim().parse::<f64>().ok().map(|v| v / 1_000_000.0))
            });

        let line = if let Some(power) = power_mw {
            format!("{name}: {status} {capacity}% {power:.1}W")
        } else {
            format!("{name}: {status} {capacity}%")
        };
        rows.push(line);
    }
    rows
}

fn collect_logged_in_users() -> Vec<String> {
    if !command_exists("who") {
        return Vec::new();
    }
    let Ok(output) = ProcessCommand::new("who").output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .take(6)
        .map(|line| line.trim().to_string())
        .collect()
}

fn collect_login_history() -> Vec<String> {
    if !command_exists("last") {
        return Vec::new();
    }
    let Ok(output) = ProcessCommand::new("last").args(["-n", "6"]).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.contains("wtmp begins"))
        .take(6)
        .map(|line| line.trim().to_string())
        .collect()
}

fn collect_ssh_sessions() -> Vec<String> {
    let mut rows = Vec::new();
    if command_exists("ss")
        && let Ok(output) = ProcessCommand::new("ss").args(["-tnp"]).output()
        && output.status.success()
    {
        rows.extend(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| line.contains("ESTAB") && line.contains(":22"))
                .take(6)
                .map(|line| line.trim().to_string()),
        );
    }

    if rows.is_empty()
        && command_exists("who")
        && let Ok(output) = ProcessCommand::new("who").output()
        && output.status.success()
    {
        rows.extend(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| line.contains("pts/") && line.contains('('))
                .take(6)
                .map(|line| line.trim().to_string()),
        );
    }

    rows
}

fn collect_failed_logins() -> Vec<String> {
    let mut rows = Vec::new();
    if command_exists("journalctl")
        && let Ok(output) = ProcessCommand::new("journalctl")
            .args(["-n", "250", "--no-pager", "--output=short"])
            .output()
        && output.status.success()
    {
        rows.extend(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| {
                    let lower = line.to_ascii_lowercase();
                    lower.contains("failed password")
                        || lower.contains("authentication failure")
                        || lower.contains("invalid user")
                })
                .rev()
                .take(6)
                .map(|line| line.trim().to_string()),
        );
    }

    if rows.is_empty() {
        for path in ["/var/log/auth.log", "/var/log/secure"] {
            if let Ok(content) = fs::read_to_string(path) {
                rows.extend(
                    content
                        .lines()
                        .filter(|line| {
                            let lower = line.to_ascii_lowercase();
                            lower.contains("failed password")
                                || lower.contains("authentication failure")
                                || lower.contains("invalid user")
                        })
                        .rev()
                        .take(6)
                        .map(|line| line.trim().to_string()),
                );
                if !rows.is_empty() {
                    break;
                }
            }
        }
    }

    rows
}

fn collect_firewall_status() -> Vec<String> {
    if command_exists("ufw")
        && let Ok(output) = ProcessCommand::new("ufw").arg("status").output()
        && output.status.success()
    {
        return String::from_utf8_lossy(&output.stdout)
            .lines()
            .take(4)
            .map(|line| line.trim().to_string())
            .collect();
    }

    if command_exists("firewall-cmd")
        && let Ok(state) = ProcessCommand::new("firewall-cmd").arg("--state").output()
        && state.status.success()
    {
        let mut rows = vec![format!(
            "firewalld: {}",
            String::from_utf8_lossy(&state.stdout).trim()
        )];
        if let Ok(zones) = ProcessCommand::new("firewall-cmd")
            .args(["--get-active-zones"])
            .output()
            && zones.status.success()
        {
            rows.extend(
                String::from_utf8_lossy(&zones.stdout)
                    .lines()
                    .take(3)
                    .map(|line| line.trim().to_string()),
            );
        }
        return rows;
    }

    if command_exists("iptables")
        && let Ok(output) = ProcessCommand::new("iptables")
            .args(["-L", "INPUT", "-n"])
            .output()
        && output.status.success()
    {
        return String::from_utf8_lossy(&output.stdout)
            .lines()
            .take(4)
            .map(|line| line.trim().to_string())
            .collect();
    }

    Vec::new()
}

fn collect_security_modules() -> Vec<String> {
    let mut rows = Vec::new();

    let selinux = if let Ok(value) = fs::read_to_string("/sys/fs/selinux/enforce") {
        let mode = if value.trim() == "1" {
            "enforcing"
        } else {
            "permissive"
        };
        Some(format!("SELinux: {mode}"))
    } else if command_exists("getenforce") {
        ProcessCommand::new("getenforce")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                format!(
                    "SELinux: {}",
                    String::from_utf8_lossy(&output.stdout).trim()
                )
            })
    } else {
        None
    };
    rows.push(selinux.unwrap_or_else(|| "SELinux: unavailable".into()));

    let apparmor = if let Ok(value) = fs::read_to_string("/sys/module/apparmor/parameters/enabled")
    {
        if value.trim().eq_ignore_ascii_case("y") {
            let profiles = fs::read_to_string("/sys/kernel/security/apparmor/profiles")
                .ok()
                .map(|content| content.lines().count())
                .unwrap_or(0);
            format!("AppArmor: enabled ({profiles} profiles)")
        } else {
            "AppArmor: disabled".into()
        }
    } else if command_exists("aa-status") {
        ProcessCommand::new("aa-status")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .find(|line| line.contains("profiles are loaded"))
                    .map(|line| format!("AppArmor: {}", line.trim()))
            })
            .unwrap_or_else(|| "AppArmor: unavailable".into())
    } else {
        "AppArmor: unavailable".into()
    };
    rows.push(apparmor);

    rows
}

fn command_exists(binary: &str) -> bool {
    ProcessCommand::new("which")
        .arg(binary)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn read_memory_pressure() -> Option<MemoryPressureInfo> {
    let content = fs::read_to_string("/proc/pressure/memory").ok()?;
    let mut pressure = MemoryPressureInfo::default();
    let mut has_some = false;
    let mut has_full = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("some ") {
            pressure.some_avg10 = parse_psi_avg(line, "avg10")?;
            pressure.some_avg60 = parse_psi_avg(line, "avg60")?;
            has_some = true;
        } else if line.starts_with("full ") {
            pressure.full_avg10 = parse_psi_avg(line, "avg10")?;
            pressure.full_avg60 = parse_psi_avg(line, "avg60")?;
            has_full = true;
        }
    }

    (has_some || has_full).then_some(pressure)
}

fn parse_psi_avg(line: &str, metric: &str) -> Option<f64> {
    let prefix = format!("{metric}=");
    line.split_whitespace().find_map(|part| {
        part.strip_prefix(&prefix)
            .and_then(|value| value.parse::<f64>().ok())
    })
}

fn read_memory_page_faults() -> Option<MemoryPageFaultInfo> {
    let content = fs::read_to_string("/proc/vmstat").ok()?;
    let mut minor = None;
    let mut major = None;

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("pgfault ") {
            minor = value.trim().parse::<u64>().ok();
        } else if let Some(value) = line.strip_prefix("pgmajfault ") {
            major = value.trim().parse::<u64>().ok();
        }
    }

    Some(MemoryPageFaultInfo {
        minor: minor?,
        major: major?,
    })
}

fn read_u64_file(path: &str) -> Option<u64> {
    read_u64_path(Path::new(path))
}

fn read_u64_path(path: &Path) -> Option<u64> {
    fs::read_to_string(path)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
}

fn read_process_name_by_pid(pid: u32) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

fn parse_optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("n/a") || trimmed == "-" {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_optional_u64(value: &str) -> Option<u64> {
    let cleaned = value
        .trim()
        .trim_end_matches("MiB")
        .trim_end_matches("W")
        .trim();
    if cleaned.is_empty() || cleaned.eq_ignore_ascii_case("n/a") || cleaned == "-" {
        None
    } else {
        cleaned.parse::<u64>().ok()
    }
}

fn parse_optional_f64(value: &str) -> Option<f64> {
    let cleaned = value
        .trim()
        .trim_end_matches('%')
        .trim_end_matches('W')
        .trim();
    if cleaned.is_empty() || cleaned.eq_ignore_ascii_case("n/a") || cleaned == "-" {
        None
    } else {
        cleaned.parse::<f64>().ok()
    }
}

// ---------------------------------------------------------------------------
// ROCm GPU runtime collector
// ---------------------------------------------------------------------------

/// Try to collect live AMD GPU telemetry via `rocm-smi`.
///
/// Attempts JSON mode first (ROCm 5+), falls back to individual `--show*` flags.
/// Returns `None` only when rocm-smi produces no usable output at all.
fn collect_rocm_gpu_runtime() -> Option<Vec<GpuRuntimeDevice>> {
    // --- Strategy 1: rocm-smi --json (ROCm ≥ 5.0) ---
    if let Some(devices) = collect_rocm_json() {
        return Some(devices);
    }
    // --- Strategy 2: rocm-smi individual flags (older ROCm / plain text) ---
    collect_rocm_plain_text()
}

/// Parse `rocm-smi --json` output.
///
/// JSON structure:
/// ```json
/// {
///   "card0": {
///     "Card series": "Radeon RX 7900 XTX",
///     "GPU use (%)": "42",
///     "Temperature (Sensor edge) (C)": "67.0",
///     "Average Graphics Package Power (W)": "210.0",
///     "Fan speed (%)": "55",
///     "VRAM Total Memory (B)": "25753026560",
///     "VRAM Total Used Memory (B)": "4294967296"
///   },
///   "card1": { ... }
/// }
/// ```
fn collect_rocm_json() -> Option<Vec<GpuRuntimeDevice>> {
    let output = ProcessCommand::new("rocm-smi")
        .args([
            "--showproductname",
            "--showuse",
            "--showtemp",
            "--showmeminfo",
            "vram",
            "--showpower",
            "--showfan",
            "--json",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // Minimal JSON parser — avoids adding a `serde_json` dependency.
    // We only need to extract string values from a flat two-level object.
    rocm_parse_json(&text)
}

/// Parse the two-level `{"cardN": {"key": "value", ...}}` JSON that rocm-smi emits.
/// Uses a hand-rolled parser so we don't need serde_json.
fn rocm_parse_json(text: &str) -> Option<Vec<GpuRuntimeDevice>> {
    // Split on top-level `"cardN":` keys.
    let mut devices: Vec<GpuRuntimeDevice> = Vec::new();
    // Find all "card<N>" blocks. We look for `"card` as a marker.
    let mut remaining = text;
    while let Some(card_start) = remaining.find("\"card") {
        remaining = &remaining[card_start + 1..];
        // Extract the card name to get the index
        let name_end = remaining.find('"').unwrap_or(0);
        let card_name = &remaining[..name_end]; // e.g. "card0"
        
        let Ok(idx) = card_name.trim_start_matches("card").parse::<u32>() else {
            continue; // Skip if it's an internal key like "card_info"
        };

        // Find the opening brace of the card object
        let brace_start = match remaining.find('{') {
            Some(pos) => pos,
            None => break,
        };
        // Find the matching closing brace
        let obj_text = &remaining[brace_start..];
        let brace_end = find_matching_brace(obj_text).unwrap_or(obj_text.len());
        let card_obj = &obj_text[1..brace_end]; // strip outer { }

        let get = |key: &str| -> Option<String> {
            // Find `"key": "value"` in card_obj
            let needle = format!("\"{key}\"");
            let pos = card_obj.find(&needle)?;
            let after_key = &card_obj[pos + needle.len()..];
            let colon = after_key.find(':')? + 1;
            let after_colon = after_key[colon..].trim();
            if let Some(inner) = after_colon.strip_prefix('"') {
                let end = inner.find('"').unwrap_or(inner.len());
                Some(inner[..end].to_string())
            } else {
                // numeric (no quotes)
                let end = after_colon
                    .find(|ch: char| !ch.is_ascii_digit() && ch != '.' && ch != '-')
                    .unwrap_or(after_colon.len());
                Some(after_colon[..end].to_string())
            }
        };

        // Product name: try several key variants
        let name = get("Card series")
            .or_else(|| get("Card model"))
            .or_else(|| get("GPU ID"))
            .unwrap_or_else(|| format!("AMD GPU {idx}"));

        let utilization_pct = get("GPU use (%)").as_deref().and_then(parse_optional_f64);

        // Temperature: try edge sensor first, then junction
        let temperature_c = get("Temperature (Sensor edge) (C)")
            .or_else(|| get("Temperature (Sensor junction) (C)"))
            .or_else(|| get("Temperature (Sensor mem) (C)"))
            .as_deref()
            .and_then(parse_optional_f64);

        let power_w = get("Average Graphics Package Power (W)")
            .or_else(|| get("Current Socket Graphics Package Power (W)"))
            .as_deref()
            .and_then(parse_optional_f64);

        let fan_pct = get("Fan speed (%)").as_deref().and_then(parse_optional_f64);

        // VRAM in bytes → convert to MiB
        let memory_total_mib = get("VRAM Total Memory (B)")
            .as_deref()
            .and_then(|v| v.parse::<u64>().ok())
            .map(|b| b / (1024 * 1024));
        let memory_used_mib = get("VRAM Total Used Memory (B)")
            .as_deref()
            .and_then(|v| v.parse::<u64>().ok())
            .map(|b| b / (1024 * 1024));

        devices.push(GpuRuntimeDevice {
            index: idx,
            uuid: None,
            name,
            utilization_pct,
            memory_used_mib,
            memory_total_mib,
            temperature_c,
            power_w,
            fan_pct,
        });

        remaining = &remaining[brace_start + brace_end..];
    }
    if devices.is_empty() {
        None
    } else {
        Some(devices)
    }
}

/// Find the index of the `}` that closes the opening `{` at position 0.
fn find_matching_brace(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, ch) in text.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Fallback: run rocm-smi with individual flags and parse the plain-text output.
///
/// Example output format:
/// ```text
/// ======================== ROCm System Management Interface ========================
/// ================================= Concise Info ==================================
/// GPU  Temp   AvgPwr  SCLK     MCLK     Fan     Perf  PwrCap  VRAM%  GPU%
/// 0    67.0c  210.0W  1900Mhz  1249Mhz  55.0%   auto  300.0W  16%    42%
/// ```
fn collect_rocm_plain_text() -> Option<Vec<GpuRuntimeDevice>> {
    // Use the concise output (always available, no flags needed in old versions)
    let output = ProcessCommand::new("rocm-smi")
        .args([
            "--showproductname",
            "--showuse",
            "--showtemp",
            "--showpower",
            "--showfan",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut gpu_blocks: HashMap<u32, Vec<(String, String)>> = HashMap::new();

    for line in text.lines() {
        let trimmed = line.trim();
        // Lines like "GPU[0]\t\t: key: value" or "GPU[0] : key value"
        if trimmed.starts_with("GPU[")
            && let Some(bracket_end) = trimmed.find(']')
            && let Ok(idx) = trimmed[4..bracket_end].parse::<u32>()
        {
            // Extract the key-value after the colon following the GPU[N]
            if let Some(rest) = trimmed[bracket_end + 1..].split_once(':').map(|x| x.1) {
                let rest = rest.trim();
                // The rest may be "key: value" or just "key value"
                if let Some((k, v)) = rest.split_once(':') {
                    gpu_blocks
                        .entry(idx)
                        .or_default()
                        .push((k.trim().to_string(), v.trim().to_string()));
                }
            }
        }
    }

    if gpu_blocks.is_empty() {
        // Last resort: try the concise table format
        return collect_rocm_concise(&text);
    }

    let mut devices: Vec<GpuRuntimeDevice> = gpu_blocks
        .iter()
        .map(|(&idx, kvs)| {
            let get = |key: &str| -> Option<String> {
                kvs.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(key))
                    .map(|(_, v)| v.clone())
            };
            let name = get("Card series")
                .or_else(|| get("Card model"))
                .unwrap_or_else(|| format!("AMD GPU {idx}"));
            GpuRuntimeDevice {
                index: idx,
                uuid: None,
                name,
                utilization_pct: get("GPU use (%)").as_deref().and_then(parse_optional_f64),
                temperature_c: get("Temperature (Sensor edge) (C)")
                    .or_else(|| get("Temperature (Sensor junction) (C)"))
                    .as_deref()
                    .and_then(parse_optional_f64),
                power_w: get("Average Graphics Package Power (W)")
                    .as_deref()
                    .and_then(parse_optional_f64),
                fan_pct: get("Fan speed (%)").as_deref().and_then(parse_optional_f64),
                memory_used_mib: None,
                memory_total_mib: None,
            }
        })
        .collect();
    devices.sort_by_key(|d| d.index);
    if devices.is_empty() {
        None
    } else {
        Some(devices)
    }
}

/// Parse the compact table that `rocm-smi` alone (no flags) prints.
///
/// Example output format:
/// ```text
/// GPU  Temp   AvgPwr  SCLK  MCLK  Fan   Perf  PwrCap  VRAM%  GPU%
/// 0    67.0c  210W    ...   ...   55%   auto  300W    16%    42%
/// ```
fn collect_rocm_concise(text: &str) -> Option<Vec<GpuRuntimeDevice>> {
    let mut header_line: Option<Vec<&str>> = None;
    let mut devices = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('=') {
            continue;
        }
        let cols: Vec<&str> = trimmed.split_whitespace().collect();
        if cols.first() == Some(&"GPU") {
            header_line = Some(cols);
            continue;
        }
        if let Some(ref headers) = header_line
            && let Some(&gpu_str) = cols.first()
            && let Ok(idx) = gpu_str.parse::<u32>()
        {
            let col = |name: &str| -> Option<&str> {
                headers
                    .iter()
                    .position(|h| h.eq_ignore_ascii_case(name))
                    .and_then(|i| cols.get(i))
                    .copied()
            };
            let strip_suffix = |s: &str| -> Option<f64> { parse_optional_f64(s) };
            devices.push(GpuRuntimeDevice {
                index: idx,
                uuid: None,
                name: format!("AMD GPU {idx}"),
                utilization_pct: col("GPU%").and_then(strip_suffix),
                temperature_c: col("Temp").and_then(strip_suffix),
                power_w: col("AvgPwr").and_then(strip_suffix),
                fan_pct: col("Fan").and_then(strip_suffix),
                memory_used_mib: None,
                memory_total_mib: None,
            });
        }
    }
    if devices.is_empty() {
        None
    } else {
        Some(devices)
    }
}
