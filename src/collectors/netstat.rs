use std::collections::BTreeMap;
use std::process::Command as ProcessCommand;

/// A single network connection row for display.
#[derive(Debug, Clone)]
pub struct ConnectionRow {
    pub proto: String,
    pub state: String,
    pub local: String,
    pub remote: String,
    pub process: String,
}

/// Collect active TCP/UDP connections via `ss`.
pub fn collect_connections(limit: usize) -> Vec<ConnectionRow> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }

    let output = ProcessCommand::new("ss").args(["-tunapH"]).output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_connection_line)
        .take(limit)
        .collect()
}

/// Collect interface → IP address mappings via `ip addr show`.
pub fn collect_interface_addresses() -> BTreeMap<String, Vec<String>> {
    if !cfg!(target_os = "linux") {
        return BTreeMap::new();
    }

    let output = ProcessCommand::new("ip")
        .args(["-o", "addr", "show"])
        .output();
    let Ok(output) = output else {
        return BTreeMap::new();
    };
    if !output.status.success() {
        return BTreeMap::new();
    }

    let mut interfaces: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 4 {
            continue;
        }
        let name = cols[1].trim_end_matches(':').to_string();
        let family = cols[2];
        let address = cols[3];
        if family == "inet" || family == "inet6" {
            interfaces
                .entry(name)
                .or_default()
                .push(address.to_string());
        }
    }
    interfaces
}

fn parse_connection_line(line: &str) -> Option<ConnectionRow> {
    let cols: Vec<&str> = line.split_whitespace().collect();
    if cols.len() < 6 {
        return None;
    }

    let process = cols
        .iter()
        .find(|col| col.contains("users:("))
        .map(|value| (*value).to_string())
        .unwrap_or_else(|| "-".into());

    Some(ConnectionRow {
        proto: cols[0].to_string(),
        state: cols[1].to_string(),
        local: cols[4].to_string(),
        remote: cols[5].to_string(),
        process,
    })
}
