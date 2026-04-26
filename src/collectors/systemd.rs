use crate::cli::ServiceState;
use crate::collectors::CommandProvider;
use anyhow::{Result, anyhow, bail};

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
pub fn collect_services(
    provider: &dyn CommandProvider,
    state: ServiceState,
    limit: usize,
) -> Result<Vec<ServiceRow>> {
    ensure_linux_systemd()?;

    let lines = run_systemctl(
        provider,
        &[
            "list-units",
            "--type=service",
            "--all",
            "--no-legend",
            "--no-pager",
        ],
    )?;

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
pub fn count_systemd_services(provider: &dyn CommandProvider) -> Result<(usize, usize)> {
    let running = run_systemctl(
        provider,
        &[
            "list-units",
            "--type=service",
            "--state=running",
            "--no-legend",
            "--no-pager",
        ],
    )?;
    let failed = run_systemctl(
        provider,
        &[
            "list-units",
            "--type=service",
            "--state=failed",
            "--no-legend",
            "--no-pager",
        ],
    )?;

    Ok((
        count_nonempty_lines(&running),
        count_nonempty_lines(&failed),
    ))
}

pub fn count_service_states(provider: &dyn CommandProvider) -> Result<ServiceStateCounts> {
    Ok(ServiceStateCounts {
        running: count_services_by_state(provider, "running")?,
        failed: count_services_by_state(provider, "failed")?,
        inactive: count_services_by_state(provider, "inactive")?,
        activating: count_services_by_state(provider, "activating")?,
        deactivating: count_services_by_state(provider, "deactivating")?,
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
pub fn run_systemctl(provider: &dyn CommandProvider, args: &[&str]) -> Result<String> {
    let output = provider
        .run("systemctl", args)
        .map_err(|e| anyhow!("failed to invoke systemctl with args: {}: {}", args.join(" "), e))?;

    if output.success {
        Ok(output.stdout.trim().to_string())
    } else {
        let stderr = output.stderr.trim().to_string();
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
pub fn collect_service_logs(
    provider: &dyn CommandProvider,
    service: &str,
    lines: usize,
) -> Result<Vec<String>> {
    ensure_linux_systemd()?;
    let output = provider
        .run(
            "journalctl",
            &[
                "-u",
                service,
                "-n",
                &lines.to_string(),
                "--no-pager",
                "--output=short",
            ],
        )
        .map_err(|e| anyhow!("failed to invoke journalctl for service {service}: {e}"))?;

    if output.success {
        Ok(output
            .stdout
            .lines()
            .map(|line| line.to_string())
            .collect())
    } else {
        let stderr = output.stderr.trim().to_string();
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

pub fn collect_service_failure_details(
    provider: &dyn CommandProvider,
    service: &str,
) -> Result<ServiceFailureDetails> {
    ensure_linux_systemd()?;

    let output = run_systemctl(
        provider,
        &[
            "show",
            service,
            "--no-pager",
            "--property=Result,ExecMainStatus,ExecMainCode,StatusText,ActiveState,SubState,UnitFileState,MainPID,TasksCurrent,MemoryCurrent,NRestarts",
        ],
    )?;

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
    let reason_output = provider
        .run(
            "journalctl",
            &[
                "-u",
                service,
                "-n",
                "30",
                "--no-pager",
                "--output=short",
                "--priority=0..3",
            ],
        )
        .map_err(|e| anyhow!("failed to invoke journalctl for service {service}: {e}"))?;

    if reason_output.success {
        details.last_error = reason_output
            .stdout
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

fn count_services_by_state(provider: &dyn CommandProvider, state: &str) -> Result<usize> {
    let output = run_systemctl(
        provider,
        &[
            "list-units",
            "--type=service",
            "--state",
            state,
            "--all",
            "--no-legend",
            "--no-pager",
        ],
    )?;
    Ok(count_nonempty_lines(&output))
}

