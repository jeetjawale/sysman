use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "sysman",
    version,
    about = "A system management CLI with an interactive terminal dashboard and one-shot inspection commands."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Launch the interactive TUI.
    Tui,
    /// Show a high-level system health summary.
    Summary,
    /// Show detailed host and OS information.
    System,
    /// Show memory and swap usage.
    Memory,
    /// Show mounted disks and capacity usage.
    Disks,
    /// Show top processes.
    Processes {
        /// Number of processes to display.
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
        /// Sort processes by the selected field.
        #[arg(short, long, value_enum, default_value_t = ProcessSort::Cpu)]
        sort: ProcessSort,
    },
    /// Inspect services through systemd on Linux hosts.
    Services {
        /// Filter by service state.
        #[arg(short, long, value_enum, default_value_t = ServiceState::Running)]
        state: ServiceState,
        /// Limit the number of rows shown.
        #[arg(short, long, default_value_t = 15)]
        limit: usize,
    },
    /// Perform an action on a service through systemctl.
    Service {
        /// Service name, for example ssh or docker.
        name: String,
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ProcessSort {
    Cpu,
    Memory,
    Pid,
    Name,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ServiceState {
    Running,
    Failed,
    All,
}

#[derive(Debug, Subcommand)]
pub enum ServiceAction {
    Status,
    Start,
    Stop,
    Restart,
}
