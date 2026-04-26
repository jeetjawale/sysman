use std::process::Command as ProcessCommand;

pub trait CommandProvider: Send + Sync {
    fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput, String>;
}

#[derive(Debug, Clone, Default)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

pub struct RealProvider;

impl CommandProvider for RealProvider {
    fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput, String> {
        let output = ProcessCommand::new(command)
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;

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
    fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput, String> {
        let key = format!("{} {}", command, args.join(" "));
        Ok(self.responses.get(&key).cloned().unwrap_or_default())
    }
}
