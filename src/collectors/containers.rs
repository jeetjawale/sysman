use crate::collectors::CommandProvider;
use anyhow::Result;

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

pub fn collect_containers(provider: &dyn CommandProvider) -> Vec<ContainerRow> {
    // Try Docker first, then Podman
    if let Ok(rows) = collect_docker(provider) {
        return rows;
    }
    collect_podman(provider).unwrap_or_default()
}

fn collect_docker(provider: &dyn CommandProvider) -> Result<Vec<ContainerRow>> {
    let output = provider
        .run(
            "docker",
            &[
                "stats",
                "--no-stream",
                "--format",
                "{{.ID}}|{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}|{{.NetIO}}|{{.BlockIO}}|{{.PIDs}}",
            ],
        )
        .map_err(|e| anyhow::anyhow!("Docker stats command failed: {}", e))?;

    if !output.success {
        return Err(anyhow::anyhow!("Docker stats failed: {}", output.stderr));
    }

    let stdout = output.stdout;
    let mut rows = Vec::new();

    // Also get images from 'docker ps' because 'stats' doesn't show them clearly
    let ps_output = provider
        .run(
            "docker",
            &["ps", "--format", "{{.ID}}|{{.Image}}|{{.Status}}"],
        )
        .map_err(|e| anyhow::anyhow!("Docker ps command failed: {}", e))?;
    let ps_stdout = ps_output.stdout;
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

fn collect_podman(provider: &dyn CommandProvider) -> Result<Vec<ContainerRow>> {
    let output = provider
        .run(
            "podman",
            &[
                "stats",
                "--no-stream",
                "--format",
                "{{.ID}}|{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}|{{.NetIO}}|{{.BlockIO}}|{{.PIDs}}",
            ],
        )
        .map_err(|e| anyhow::anyhow!("Podman stats command failed: {}", e))?;

    if !output.success {
        return Err(anyhow::anyhow!("Podman stats failed: {}", output.stderr));
    }

    let stdout = output.stdout;
    let mut rows = Vec::new();

    let ps_output = provider
        .run(
            "podman",
            &["ps", "--format", "{{.ID}}|{{.Image}}|{{.Status}}"],
        )
        .map_err(|e| anyhow::anyhow!("Podman ps command failed: {}", e))?;
    let ps_stdout = ps_output.stdout;
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

pub fn act_on_container(provider: &dyn CommandProvider, id: &str, action: &str) -> Result<()> {
    // Try Docker then Podman
    if let Ok(output) = provider.run("docker", &[action, id])
        && output.success
    {
        return Ok(());
    }

    let output = provider
        .run("podman", &[action, id])
        .map_err(|e| anyhow::anyhow!("Podman command failed: {}", e))?;

    if output.success {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to {} container {}: {}",
            action,
            id,
            output.stderr.trim()
        ))
    }
}

pub fn get_container_logs(
    provider: &dyn CommandProvider,
    id: &str,
    limit: usize,
) -> Result<Vec<String>> {
    if let Ok(output) = provider.run("docker", &["logs", "--tail", &limit.to_string(), id])
        && output.success
    {
        let stdout = output.stdout;
        return Ok(stdout.lines().map(|s| s.to_string()).collect());
    }

    let output = provider
        .run("podman", &["logs", "--tail", &limit.to_string(), id])
        .map_err(|e| anyhow::anyhow!("Podman command failed: {}", e))?;

    if output.success {
        let stdout = output.stdout;
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    } else {
        Err(anyhow::anyhow!(
            "Failed to get logs for container {}: {}",
            id,
            output.stderr.trim()
        ))
    }
}
