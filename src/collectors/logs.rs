use super::CommandProvider;

fn run_lines(provider: &dyn CommandProvider, command: &str, args: &[&str]) -> Vec<String> {
    let Ok(output) = provider.run(command, args) else {
        return Vec::new();
    };
    if !output.success {
        return Vec::new();
    }
    output.stdout
        .lines()
        .map(|line| line.to_string())
        .collect()
}

pub fn collect_journal_lines(provider: &dyn CommandProvider, limit: usize) -> Vec<String> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }
    run_lines(
        provider,
        "journalctl",
        &["-n", &limit.to_string(), "--no-pager", "--output=short"],
    )
}

pub fn collect_syslog_lines(provider: &dyn CommandProvider, limit: usize) -> Vec<String> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }
    let mut lines = run_lines(provider, "tail", &["-n", &limit.to_string(), "/var/log/syslog"]);
    if lines.is_empty() {
        lines = run_lines(provider, "tail", &["-n", &limit.to_string(), "/var/log/messages"]);
    }
    lines
}

pub fn collect_dmesg_lines(provider: &dyn CommandProvider, limit: usize) -> Vec<String> {
    run_lines(
        provider,
        "dmesg",
        &["--ctime", "--nopager", "--level=err,warn,info"],
    )
    .into_iter()
    .rev()
    .take(limit)
    .collect::<Vec<_>>()
    .into_iter()
    .rev()
    .collect()
}
