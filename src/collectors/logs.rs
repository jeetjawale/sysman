use super::CommandProvider;

fn run_lines(provider: &dyn CommandProvider, command: &str, args: &[&str]) -> Vec<String> {
    let Ok(output) = provider.run(command, args) else {
        return Vec::new();
    };
    if !output.success {
        return Vec::new();
    }
    output.stdout.lines().map(|line| line.to_string()).collect()
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
    let mut lines = run_lines(
        provider,
        "tail",
        &["-n", &limit.to_string(), "/var/log/syslog"],
    );
    if lines.is_empty() {
        lines = run_lines(
            provider,
            "tail",
            &["-n", &limit.to_string(), "/var/log/messages"],
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::provider::{CommandOutput, MockProvider};
    use std::collections::HashMap;

    #[test]
    fn test_collect_journal_lines_mock() {
        let mut responses = HashMap::new();
        responses.insert(
            "journalctl -n 10 --no-pager --output=short".to_string(),
            crate::collectors::provider::MockBehavior::Success(CommandOutput {
                stdout: "line 1\nline 2\nline 3".to_string(),
                stderr: "".to_string(),
                success: true,
            }),
        );

        let mock_provider = MockProvider { responses };
        let lines = collect_journal_lines(&mock_provider, 10);

        #[cfg(target_os = "linux")]
        {
            assert_eq!(lines.len(), 3);
            assert_eq!(lines[0], "line 1");
            assert_eq!(lines[1], "line 2");
            assert_eq!(lines[2], "line 3");
        }

        #[cfg(not(target_os = "linux"))]
        {
            assert!(lines.is_empty());
        }
    }
}
