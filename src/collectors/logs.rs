use std::process::Command as ProcessCommand;

fn run_lines(command: &str, args: &[&str]) -> Vec<String> {
    let output = ProcessCommand::new(command).args(args).output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.to_string())
        .collect()
}

pub fn collect_journal_lines(limit: usize) -> Vec<String> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }
    run_lines(
        "journalctl",
        &["-n", &limit.to_string(), "--no-pager", "--output=short"],
    )
}

pub fn collect_syslog_lines(limit: usize) -> Vec<String> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }
    let mut lines = run_lines("tail", &["-n", &limit.to_string(), "/var/log/syslog"]);
    if lines.is_empty() {
        lines = run_lines("tail", &["-n", &limit.to_string(), "/var/log/messages"]);
    }
    lines
}

pub fn collect_dmesg_lines(limit: usize) -> Vec<String> {
    run_lines("dmesg", &["--ctime", "--nopager", "--level=err,warn,info"])
        .into_iter()
        .rev()
        .take(limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}
