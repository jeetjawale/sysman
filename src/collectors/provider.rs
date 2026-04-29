use anyhow::Result;
#[cfg(test)]
use anyhow::anyhow;
use std::process::Command as ProcessCommand;

pub trait CommandProvider: Send + Sync {
    fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput>;
}

#[derive(Debug, Clone, Default)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

pub struct RealProvider;

impl CommandProvider for RealProvider {
    fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput> {
        let mut out = ProcessCommand::new(command).args(args).output()?;

        // If the command failed with a permission-like error, try with sudo non-interactive.
        // List of common tools that often require root for full functionality.
        let priv_commands = [
            "smartctl",
            "journalctl",
            "ss",
            "docker",
            "podman",
            "dmesg",
            "getenforce",
            "aa-status",
            "ufw",
            "iptables",
            "firewall-cmd",
        ];

        if !out.status.success() && priv_commands.contains(&command) {
            // Attempt with sudo -n (non-interactive).
            // This only succeeds if the user has passwordless sudo for this command.
            let mut sudo_args = vec!["-n", command];
            sudo_args.extend_from_slice(args);
            if let Ok(sudo_out) = ProcessCommand::new("sudo").args(&sudo_args).output()
                && sudo_out.status.success()
            {
                out = sudo_out;
            }
        }

        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            success: out.status.success(),
        })
    }
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub enum MockBehavior {
    Success(CommandOutput),
    MissingBinary,
    ExitFailure { exit_code: i32, stderr: String },
    Timeout,
}

#[cfg(test)]
pub struct MockProvider {
    pub responses: std::collections::HashMap<String, MockBehavior>,
}

#[cfg(test)]
impl CommandProvider for MockProvider {
    fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput> {
        let key = format!("{} {}", command, args.join(" "));
        let behavior = self.responses.get(&key).cloned().ok_or_else(|| {
            anyhow!(
                "Mock response not found for: {} {}",
                command,
                args.join(" ")
            )
        })?;

        match behavior {
            MockBehavior::Success(output) => Ok(output),
            MockBehavior::MissingBinary => Err(anyhow!("Binary missing: {}", command)),
            MockBehavior::ExitFailure { exit_code: _, stderr } => Ok(CommandOutput {
                stdout: "".to_string(),
                stderr,
                success: false,
            }),
            MockBehavior::Timeout => Err(anyhow!("Command timed out: {}", command)),
        }
    }
}
