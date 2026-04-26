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
        let output = ProcessCommand::new(command)
            .args(args)
            .output()?;

        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            success: output.status.success(),
        })
    }
}

#[cfg(test)]
pub struct MockProvider {
    pub responses: std::collections::HashMap<String, CommandOutput>,
}

#[cfg(test)]
impl CommandProvider for MockProvider {
    fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput> {
        let key = format!("{} {}", command, args.join(" "));
        self.responses
            .get(&key)
            .cloned()
            .ok_or_else(|| anyhow!("Mock response not found for: {} {}", command, args.join(" ")))
    }
}
