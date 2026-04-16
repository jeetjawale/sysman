mod app;
mod cli;
mod commands;
mod theme;

use anyhow::Result;
use clap::Parser;
use cli::Command;

pub fn run() -> Result<()> {
    let cli = cli::Cli::parse();
    match cli.command {
        None | Some(Command::Tui) => app::run(),
        Some(command) => commands::execute(command),
    }
}
