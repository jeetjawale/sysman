use crate::cli::ServiceState;
use anyhow::{Context, Result, anyhow, bail};
use std::process::Command as ProcessCommand;

/// A single systemd service row for display.
#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub name: String,
    pub active: String,
    pub sub: String,
}

/// Aggregate service counts.
#[derive(Debug, Clone, Copy)]
pub struct ServiceSummary {
    pub running: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ServiceStateCounts {
    pub running: usize,
    pub failed: usize,
    pub inactive: usize,
    pub activating: usize,
    pub deactivating: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ServiceFailureDetails {
    pub result: String,
    pub exec_main_status: Option<i32>,
    pub exec_main_code: String,
    pub status_text: String,
    pub last_error: String,
    pub active_state: String,
    pub sub_state: String,
    pub unit_file_state: String,
    pub main_pid: Option<u32>,
    pub tasks_current: Option<u32>,
    pub memory_current: Option<u64>,
    pub n_restarts: Option<u64>,
}

/// Collect systemd services filtered by `state`.
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

/// Count running and failed systemd services.
pub fn count_systemd_services() -> Result<(usize, usize)> {
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

pub fn count_service_states() -> Result<ServiceStateCounts> {
    Ok(ServiceStateCounts {
        running: count_services_by_state("running")?,
        failed: count_services_by_state("failed")?,
        inactive: count_services_by_state("inactive")?,
        activating: count_services_by_state("activating")?,
        deactivating: count_services_by_state("deactivating")?,
    })
}

/// Ensure we're on a Linux host with systemd.
pub fn ensure_linux_systemd() -> Result<()> {
    if !cfg!(target_os = "linux") {
        bail!("service management is currently supported on Linux hosts only");
    }
    Ok(())
}

/// Run a systemctl command and return stdout as a string.
pub fn run_systemctl(args: &[&str]) -> Result<String> {
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

/// Collect recent journal lines for a service.
pub fn collect_service_logs(service: &str, lines: usize) -> Result<Vec<String>> {
    ensure_linux_systemd()?;
    let output = ProcessCommand::new("journalctl")
        .args([
            "-u",
            service,
            "-n",
            &lines.to_string(),
            "--no-pager",
            "--output=short",
        ])
        .output()
        .with_context(|| format!("failed to invoke journalctl for service {service}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| line.to_string())
            .collect())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(anyhow!(
            "journalctl -u {} failed: {}",
            service,
            if stderr.is_empty() {
                "unknown error"
            } else {
                &stderr
            }
        ))
    }
}

pub fn collect_service_failure_details(service: &str) -> Result<ServiceFailureDetails> {
    ensure_linux_systemd()?;

    let output = run_systemctl(&[
        "show",
        service,
        "--no-pager",
        "--property=Result,ExecMainStatus,ExecMainCode,StatusText,ActiveState,SubState,UnitFileState,MainPID,TasksCurrent,MemoryCurrent,NRestarts",
    ])?;

    let mut details = ServiceFailureDetails::default();
    for line in output.lines() {
        if let Some(value) = line.strip_prefix("Result=") {
            details.result = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("ExecMainStatus=") {
            details.exec_main_status = value.trim().parse::<i32>().ok();
        } else if let Some(value) = line.strip_prefix("ExecMainCode=") {
            details.exec_main_code = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("StatusText=") {
            details.status_text = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("ActiveState=") {
            details.active_state = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("SubState=") {
            details.sub_state = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("UnitFileState=") {
            details.unit_file_state = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("MainPID=") {
            details.main_pid = value.trim().parse::<u32>().ok().filter(|pid| *pid > 0);
        } else if let Some(value) = line.strip_prefix("TasksCurrent=") {
            details.tasks_current = value.trim().parse::<u32>().ok();
        } else if let Some(value) = line.strip_prefix("MemoryCurrent=") {
            details.memory_current = value.trim().parse::<u64>().ok().filter(|v| *v > 0);
        } else if let Some(value) = line.strip_prefix("NRestarts=") {
            details.n_restarts = value.trim().parse::<u64>().ok();
        }
    }

    // Best-effort reason line from latest error/critical journal entries.
    let reason_output = ProcessCommand::new("journalctl")
        .args([
            "-u",
            service,
            "-n",
            "30",
            "--no-pager",
            "--output=short",
            "--priority=0..3",
        ])
        .output()
        .with_context(|| format!("failed to invoke journalctl for service {service}"))?;
    if reason_output.status.success() {
        details.last_error = String::from_utf8_lossy(&reason_output.stdout)
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .unwrap_or_default();
    }

    Ok(details)
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

fn count_nonempty_lines(text: &str) -> usize {
    text.lines().filter(|line| !line.trim().is_empty()).count()
}

fn count_services_by_state(state: &str) -> Result<usize> {
    let output = run_systemctl(&[
        "list-units",
        "--type=service",
        "--state",
        state,
        "--all",
        "--no-legend",
        "--no-pager",
    ])?;
    Ok(count_nonempty_lines(&output))
}
