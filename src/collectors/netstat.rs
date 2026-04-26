use super::CommandProvider;
use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// A single network connection row for display.
#[derive(Debug, Clone)]
pub struct ConnectionRow {
    pub proto: String,
    pub state: String,
    pub local: String,
    pub remote: String,
    pub process_name: String,
    pub pid: Option<u32>,
    pub remote_ip: String,
    pub remote_port: Option<u16>,
    pub suspicious: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProcessNetRow {
    pub pid: u32,
    pub process: String,
    pub rx_bps: u64,
    pub tx_bps: u64,
    pub connections: usize,
}

#[derive(Debug, Clone, Default)]
pub struct InterfaceLinkDetails {
    pub state: String,
    pub mac: String,
    pub mtu: Option<u32>,
}

/// Collect active TCP/UDP connections via `ss`.
pub fn collect_connections(provider: &dyn CommandProvider, limit: usize) -> Vec<ConnectionRow> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }

    let output = provider.run("ss", &["-tunapH"]);
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.success {
        return Vec::new();
    }

    output
        .stdout
        .lines()
        .filter_map(parse_connection_line)
        .take(limit)
        .collect()
}

pub fn collect_process_bandwidth(
    provider: &dyn CommandProvider,
    previous: &HashMap<u32, (u64, u64)>,
    elapsed_secs: f64,
    limit: usize,
) -> (Vec<ProcessNetRow>, HashMap<u32, (u64, u64)>) {
    if !cfg!(target_os = "linux") {
        return (Vec::new(), previous.clone());
    }

    let output = provider.run("ss", &["-tinapH"]);
    let Ok(output) = output else {
        return (Vec::new(), previous.clone());
    };
    if !output.success {
        return (Vec::new(), previous.clone());
    }

    let mut current_totals: HashMap<u32, (u64, u64)> = HashMap::new();
    let mut names: HashMap<u32, String> = HashMap::new();
    let mut connection_counts: HashMap<u32, usize> = HashMap::new();
    let mut pending_pid: Option<u32> = None;

    for line in output.stdout.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(pid) = pending_pid {
                let sent = parse_counter(line, "bytes_sent:");
                let recv = parse_counter(line, "bytes_received:");
                if sent.is_some() || recv.is_some() {
                    let entry = current_totals.entry(pid).or_insert((0, 0));
                    entry.0 = entry.0.saturating_add(sent.unwrap_or(0));
                    entry.1 = entry.1.saturating_add(recv.unwrap_or(0));
                }
            }
            continue;
        }

        pending_pid = parse_process_info(line).and_then(|(_, pid)| pid);
        if let Some((process, Some(pid))) = parse_process_info(line) {
            names.entry(pid).or_insert(process);
            *connection_counts.entry(pid).or_insert(0) += 1;
        }
    }

    let mut rows: Vec<ProcessNetRow> = current_totals
        .iter()
        .map(|(pid, (tx_total, rx_total))| {
            let (prev_tx, prev_rx) = previous.get(pid).copied().unwrap_or((*tx_total, *rx_total));
            let tx_bps =
                (tx_total.saturating_sub(prev_tx) as f64 / elapsed_secs.max(1.0)).round() as u64;
            let rx_bps =
                (rx_total.saturating_sub(prev_rx) as f64 / elapsed_secs.max(1.0)).round() as u64;
            ProcessNetRow {
                pid: *pid,
                process: names
                    .get(pid)
                    .cloned()
                    .unwrap_or_else(|| format!("pid-{pid}")),
                rx_bps,
                tx_bps,
                connections: connection_counts.get(pid).copied().unwrap_or(0),
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        (b.rx_bps + b.tx_bps)
            .cmp(&(a.rx_bps + a.tx_bps))
            .then_with(|| a.process.cmp(&b.process))
    });
    rows.truncate(limit);

    (rows, current_totals)
}

/// Collect interface → IP address mappings via `ip addr show`.
pub fn collect_interface_addresses(
    provider: &dyn CommandProvider,
) -> BTreeMap<String, Vec<String>> {
    if !cfg!(target_os = "linux") {
        return BTreeMap::new();
    }

    let output = provider.run("ip", &["-o", "addr", "show"]);
    let Ok(output) = output else {
        return BTreeMap::new();
    };
    if !output.success {
        return BTreeMap::new();
    }

    let mut interfaces: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in output.stdout.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 4 {
            continue;
        }
        let name = cols[1]
            .trim_end_matches(':')
            .split('@')
            .next()
            .unwrap_or(cols[1])
            .to_string();
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

pub fn collect_interface_link_details(
    provider: &dyn CommandProvider,
) -> BTreeMap<String, InterfaceLinkDetails> {
    if !cfg!(target_os = "linux") {
        return BTreeMap::new();
    }

    let output = provider.run("ip", &["-o", "link", "show"]);
    let Ok(output) = output else {
        return BTreeMap::new();
    };
    if !output.success {
        return BTreeMap::new();
    }

    let mut details = BTreeMap::new();
    for line in output.stdout.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 4 {
            continue;
        }
        let name = cols[1]
            .trim_end_matches(':')
            .split('@')
            .next()
            .unwrap_or(cols[1])
            .to_string();
        let mtu = cols
            .windows(2)
            .find(|w| w[0] == "mtu")
            .and_then(|w| w[1].parse::<u32>().ok());
        let state = cols
            .windows(2)
            .find(|w| w[0] == "state")
            .map(|w| w[1].to_string())
            .unwrap_or_else(|| "UNKNOWN".into());
        let mac = cols
            .windows(2)
            .find(|w| w[0].starts_with("link/"))
            .map(|w| w[1].to_string())
            .filter(|value| value != "00:00:00:00:00:00")
            .unwrap_or_else(|| "-".into());
        details.insert(name, InterfaceLinkDetails { state, mac, mtu });
    }
    details
}

fn parse_connection_line(line: &str) -> Option<ConnectionRow> {
    let cols: Vec<&str> = line.split_whitespace().collect();
    if cols.len() < 6 {
        return None;
    }

    let (process_name, pid) = parse_process_info(line).unwrap_or_else(|| ("-".into(), None));
    let (remote_ip, remote_port) = split_endpoint(cols[5]);
    let suspicious = suspicious_reason(cols[1], &remote_ip, remote_port);

    Some(ConnectionRow {
        proto: cols[0].to_string(),
        state: cols[1].to_string(),
        local: cols[4].to_string(),
        remote: cols[5].to_string(),
        process_name,
        pid,
        remote_ip,
        remote_port,
        suspicious,
    })
}

pub fn kill_connection(
    provider: &dyn CommandProvider,
    conn: &ConnectionRow,
) -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("connection actions are currently supported on Linux hosts only".into());
    }

    if conn.proto.starts_with("tcp")
        && !conn.remote_ip.is_empty()
        && conn.remote_ip != "-"
        && let Some(port) = conn.remote_port
    {
        let kill_output = provider.run(
            "ss",
            &[
                "-K",
                "dst",
                &conn.remote_ip,
                "dport",
                "=",
                &port.to_string(),
            ],
        );
        if let Ok(output) = kill_output
            && output.success
        {
            return Ok(format!("Killed TCP flow to {}:{port}", conn.remote_ip));
        }
    }

    let Some(pid) = conn.pid else {
        return Err("No owning PID found for selected connection".into());
    };
    let output = provider
        .run("kill", &["-KILL", &pid.to_string()])
        .map_err(|error| error.to_string())?;
    if output.success {
        Ok(format!("Killed connection owner PID {pid}"))
    } else {
        let stderr = output.stderr.trim().to_string();
        Err(if stderr.is_empty() {
            format!("Failed to kill PID {pid}")
        } else {
            format!("Failed to kill PID {pid}: {stderr}")
        })
    }
}

pub fn block_ip(provider: &dyn CommandProvider, ip: &str) -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("IP blocking is currently supported on Linux hosts only".into());
    }
    if ip.is_empty() || ip == "-" {
        return Err("No remote IP found for selected connection".into());
    }

    if command_exists(provider, "ufw") {
        let output = provider
            .run("ufw", &["--force", "deny", "from", ip])
            .map_err(|error| error.to_string())?;
        if output.success {
            return Ok(format!("Blocked {ip} via ufw deny"));
        }
        let stderr = output.stderr.trim().to_string();
        return Err(if stderr.is_empty() {
            format!("ufw deny failed for {ip}")
        } else {
            format!("ufw deny failed for {ip}: {stderr}")
        });
    }

    let input = provider
        .run("iptables", &["-I", "INPUT", "-s", ip, "-j", "DROP"])
        .map_err(|error| error.to_string())?;
    if !input.success {
        let stderr = input.stderr.trim().to_string();
        return Err(if stderr.is_empty() {
            format!("iptables INPUT rule failed for {ip}")
        } else {
            format!("iptables INPUT rule failed for {ip}: {stderr}")
        });
    }
    let output = provider
        .run("iptables", &["-I", "OUTPUT", "-d", ip, "-j", "DROP"])
        .map_err(|error| error.to_string())?;
    if output.success {
        Ok(format!("Blocked {ip} via iptables"))
    } else {
        let stderr = output.stderr.trim().to_string();
        Err(if stderr.is_empty() {
            format!("iptables OUTPUT rule failed for {ip}")
        } else {
            format!("iptables OUTPUT rule failed for {ip}: {stderr}")
        })
    }
}

pub fn run_dns_lookup(provider: &dyn CommandProvider, target: &str, limit: usize) -> Vec<String> {
    if target.trim().is_empty() {
        return vec!["Empty lookup target".into()];
    }
    let getent = provider.run("getent", &["ahosts", target]);
    if let Ok(output) = getent
        && output.success
    {
        return output
            .stdout
            .lines()
            .take(limit)
            .map(|line| line.to_string())
            .collect();
    }

    let nslookup = provider.run("nslookup", &[target]);
    let Ok(output) = nslookup else {
        return vec!["DNS lookup command unavailable".into()];
    };
    if !output.success {
        let stderr = output.stderr.trim().to_string();
        return vec![if stderr.is_empty() {
            "DNS lookup failed".into()
        } else {
            format!("DNS lookup failed: {stderr}")
        }];
    }
    output
        .stdout
        .lines()
        .take(limit)
        .map(|line| line.to_string())
        .collect()
}

pub fn run_ping(provider: &dyn CommandProvider, target: &str, limit: usize) -> Vec<String> {
    if target.trim().is_empty() {
        return vec!["Empty ping target".into()];
    }
    let output = provider.run("ping", &["-c", "2", "-W", "1", target]);
    let Ok(output) = output else {
        return vec!["Ping command unavailable".into()];
    };

    if output.success {
        output
            .stdout
            .lines()
            .take(limit)
            .map(|line| line.to_string())
            .collect()
    } else {
        let stderr = output.stderr.trim().to_string();
        vec![if stderr.is_empty() {
            "Ping failed".into()
        } else {
            format!("Ping failed: {stderr}")
        }]
    }
}

pub fn run_traceroute(provider: &dyn CommandProvider, target: &str, limit: usize) -> Vec<String> {
    if target.trim().is_empty() {
        return vec!["Empty traceroute target".into()];
    }

    let tracepath = provider.run("tracepath", &["-n", target]);
    if let Ok(output) = tracepath
        && output.success
    {
        return output
            .stdout
            .lines()
            .take(limit)
            .map(|line| line.to_string())
            .collect();
    }

    let traceroute = provider.run("traceroute", &["-n", "-m", "6", target]);
    let Ok(output) = traceroute else {
        return vec!["Traceroute command unavailable".into()];
    };
    if output.success {
        output
            .stdout
            .lines()
            .take(limit)
            .map(|line| line.to_string())
            .collect()
    } else {
        let stderr = output.stderr.trim().to_string();
        vec![if stderr.is_empty() {
            "Traceroute failed".into()
        } else {
            format!("Traceroute failed: {stderr}")
        }]
    }
}

pub fn run_http_probe(provider: &dyn CommandProvider, target: &str, limit: usize) -> Vec<String> {
    if target.trim().is_empty() {
        return vec!["Empty HTTP probe target".into()];
    }

    let mut host = target.trim().to_string();
    if !host.starts_with("http://") && !host.starts_with("https://") {
        host = format!("http://{host}");
    }

    let output = provider.run(
        "curl",
        &[
            "-sS",
            "-o",
            "/dev/null",
            "-L",
            "--max-time",
            "3",
            "-w",
            "code=%{http_code} ip=%{remote_ip} connect=%{time_connect}s total=%{time_total}s",
            &host,
        ],
    );
    let Ok(output) = output else {
        return vec!["HTTP probe command unavailable (curl)".into()];
    };
    if output.success {
        output
            .stdout
            .lines()
            .take(limit)
            .map(|line| line.to_string())
            .collect()
    } else {
        let stderr = output.stderr.trim().to_string();
        vec![if stderr.is_empty() {
            "HTTP probe failed".into()
        } else {
            format!("HTTP probe failed: {stderr}")
        }]
    }
}

fn parse_process_info(line: &str) -> Option<(String, Option<u32>)> {
    let users_part = line.split("users:((").nth(1)?;
    let process_name = users_part
        .strip_prefix('"')
        .and_then(|rest| rest.split('"').next())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".into());
    let pid = users_part.split("pid=").nth(1).and_then(|rest| {
        rest.split(|ch: char| !ch.is_ascii_digit())
            .next()
            .and_then(|value| value.parse::<u32>().ok())
    });
    Some((process_name, pid))
}

fn split_endpoint(endpoint: &str) -> (String, Option<u16>) {
    if endpoint.is_empty() || endpoint == "*" || endpoint == "*:*" {
        return ("-".into(), None);
    }
    let Some((ip_raw, port_raw)) = endpoint.rsplit_once(':') else {
        return (endpoint.to_string(), None);
    };
    let mut ip = ip_raw.trim_matches('[').trim_matches(']').to_string();
    if ip == "*" {
        ip = "-".into();
    }
    let port = if port_raw == "*" {
        None
    } else {
        port_raw.parse::<u16>().ok()
    };
    (ip, port)
}

fn suspicious_reason(state: &str, remote_ip: &str, remote_port: Option<u16>) -> Option<String> {
    if state == "SYN-SENT" || state == "SYN-RECV" {
        return Some("SYN handshake pending".into());
    }
    let port = remote_port?;
    if [23u16, 2323, 4444, 5555, 6667, 31337].contains(&port) {
        return Some(format!("Remote port {port} is high-risk"));
    }
    if state == "ESTAB" && is_public_ip(remote_ip) && ![80u16, 443, 53, 123, 22].contains(&port) {
        return Some("External connection on uncommon port".into());
    }
    None
}

fn is_public_ip(ip: &str) -> bool {
    let Ok(addr) = ip.parse::<IpAddr>() else {
        return false;
    };
    match addr {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified())
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    !(ip.is_loopback()
        || ip.is_multicast()
        || ip.is_unspecified()
        || ip.is_unicast_link_local()
        || ip.is_unique_local())
}

fn parse_counter(line: &str, marker: &str) -> Option<u64> {
    line.split(marker)
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
}

fn command_exists(provider: &dyn CommandProvider, bin: &str) -> bool {
    provider
        .run("which", &[bin])
        .is_ok_and(|output| output.success)
}
