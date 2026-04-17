# Sysman

`sysman` is a terminal system monitor with a TUI and CLI.

## Currently available

- Interactive TUI with tabs for dashboard, system, network, disks, processes, services, and help
- Live system vitals: CPU, memory, swap, disk usage, network throughput, uptime, host/OS info
- Process table with sorting and filtering
- Service listing on Linux `systemd` hosts
- CLI commands for summary, system, memory, disks, processes, services, and service actions

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
- Service management targets Linux hosts using `systemd`.
- `start`, `stop`, and `restart` may require elevated privileges.
