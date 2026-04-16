# Sysman

`sysman` is a system management CLI for inspecting machine health from the terminal. It now ships with an interactive TUI inspired by `netwatch-main`, but focused on host state instead of network diagnostics.

## Features

- Interactive TUI dashboard with tab navigation and periodic refresh
- System summary with host, OS, uptime, CPU, memory, swap, and disk pressure
- Detailed system information view
- Memory and swap inspection
- Mounted disk usage reporting
- Top process listing sorted by CPU, memory, or name
- Linux `systemd` service listing and service actions

## Usage

```bash
cargo run
cargo run -- tui
cargo run -- summary
cargo run -- system
cargo run -- memory
cargo run -- disks
cargo run -- processes --limit 15 --sort memory
cargo run -- services --state failed
cargo run -- service ssh status
```

## Commands

```text
sysman [COMMAND]

Commands:
  tui        Launch the interactive TUI
  summary    Show a high-level system health summary
  system     Show detailed host and OS information
  memory     Show memory and swap usage
  disks      Show mounted disks and capacity usage
  processes  Show top processes
  services   Inspect services through systemd on Linux hosts
  service    Perform an action on a service through systemctl
```

## Notes

- Running `sysman` with no command launches the TUI.
- Service management currently targets Linux hosts that use `systemd`.
- `start`, `stop`, and `restart` actions may require elevated privileges depending on the target service.
