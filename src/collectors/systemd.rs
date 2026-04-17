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
