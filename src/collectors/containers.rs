use anyhow::Result;
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct ContainerRow {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub cpu: String,
    pub memory: String,
    pub net_io: String,
    pub block_io: String,
    pub pids: u32,
}

pub fn collect_containers() -> Vec<ContainerRow> {
    // Try Docker first, then Podman
    if let Ok(rows) = collect_docker() {
        return rows;
    }
    collect_podman().unwrap_or_default()
}

fn collect_docker() -> Result<Vec<ContainerRow>> {
    let output = Command::new("docker")
        .args([
            "stats",
            "--no-stream",
            "--format",
            "{{.ID}}|{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}|{{.NetIO}}|{{.BlockIO}}|{{.PIDs}}",
        ])
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Docker stats failed"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut rows = Vec::new();

    // Also get images from 'docker ps' because 'stats' doesn't show them clearly
    let ps_output = Command::new("docker")
        .args(["ps", "--format", "{{.ID}}|{{.Image}}|{{.Status}}"])
        .output()?;
    let ps_stdout = String::from_utf8_lossy(&ps_output.stdout);
    let mut ps_info = std::collections::HashMap::new();
    for line in ps_stdout.lines() {
        let cols: Vec<&str> = line.split('|').collect();
        if cols.len() >= 3 {
            ps_info.insert(
                cols[0].to_string(),
                (cols[1].to_string(), cols[2].to_string()),
            );
        }
    }

    for line in stdout.lines() {
        let cols: Vec<&str> = line.split('|').collect();
        if cols.len() >= 7 {
            let id = cols[0].to_string();
            let (image, status) = ps_info
                .get(&id)
                .cloned()
                .unwrap_or(("-".into(), "-".into()));
            rows.push(ContainerRow {
                id,
                name: cols[1].into(),
                image,
                status,
                cpu: cols[2].into(),
                memory: cols[3].into(),
                net_io: cols[4].into(),
                block_io: cols[5].into(),
                pids: cols[6].parse().unwrap_or(0),
            });
        }
    }

    Ok(rows)
}

fn collect_podman() -> Result<Vec<ContainerRow>> {
    let output = Command::new("podman")
        .args([
            "stats",
            "--no-stream",
            "--format",
            "{{.ID}}|{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}|{{.NetIO}}|{{.BlockIO}}|{{.PIDs}}",
        ])
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Podman stats failed"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut rows = Vec::new();

    let ps_output = Command::new("podman")
        .args(["ps", "--format", "{{.ID}}|{{.Image}}|{{.Status}}"])
        .output()?;
    let ps_stdout = String::from_utf8_lossy(&ps_output.stdout);
    let mut ps_info = std::collections::HashMap::new();
    for line in ps_stdout.lines() {
        let cols: Vec<&str> = line.split('|').collect();
        if cols.len() >= 3 {
            ps_info.insert(
                cols[0].to_string(),
                (cols[1].to_string(), cols[2].to_string()),
            );
        }
    }

    for line in stdout.lines() {
        let cols: Vec<&str> = line.split('|').collect();
        if cols.len() >= 7 {
            let id = cols[0].to_string();
            let (image, status) = ps_info
                .get(&id)
                .cloned()
                .unwrap_or(("-".into(), "-".into()));
            rows.push(ContainerRow {
                id,
                name: cols[1].into(),
                image,
                status,
                cpu: cols[2].into(),
                memory: cols[3].into(),
                net_io: cols[4].into(),
                block_io: cols[5].into(),
                pids: cols[6].parse().unwrap_or(0),
            });
        }
    }

    Ok(rows)
}

pub fn act_on_container(id: &str, action: &str) -> Result<()> {
    // Try Docker then Podman
    let mut cmd = Command::new("docker");
    cmd.args([action, id]);
    if let Ok(output) = cmd.output()
        && output.status.success()
    {
        return Ok(());
    }

    let mut cmd = Command::new("podman");
    cmd.args([action, id]);
    let output = cmd.output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to {} container {}: {}",
            action,
            id,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

pub fn get_container_logs(id: &str, limit: usize) -> Result<Vec<String>> {
    let mut cmd = Command::new("docker");
    cmd.args(["logs", "--tail", &limit.to_string(), id]);
    if let Ok(output) = cmd.output()
        && output.status.success()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Ok(stdout.lines().map(|s| s.to_string()).collect());
    }

    let mut cmd = Command::new("podman");
    cmd.args(["logs", "--tail", &limit.to_string(), id]);
    let output = cmd.output()?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    } else {
        Err(anyhow::anyhow!(
            "Failed to get logs for container {}: {}",
            id,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
