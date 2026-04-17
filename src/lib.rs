mod app;
mod cli;
pub(crate) mod collectors;
mod commands;
mod event;
mod theme;
mod ui;

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
