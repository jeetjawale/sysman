# Sysman

`sysman` is a terminal system monitor with a TUI and CLI.

## Currently available

- Interactive TUI with focused tabs: overview, cpu, memory, processes, network, disk, gpu, services, logs, hardware, and help
- Help tab now includes per-tab deep sections and a config/behavior reference
- Live system vitals: CPU (per-core history, freq/governor, ctx-switch rate, temp/throttle), memory (PSI, page-fault rates, leak-growth suspects), swap, disk usage, network throughput, uptime, host/OS info
- Process table with sorting/filtering, group-by (user/service/container), group separators, kill/renice/CPU pinning, per-PID open files/ports/cmdline/env/maps, and CPU history sparklines
- Network power tools: per-process bandwidth estimates, interface state/MAC/MTU, connection-state filters, open-ports panel, suspicious flags, kill/block actions, DNS+ping+traceroute+HTTP probe utility
- Disk power tools: inode visibility and critical badges, async multi-depth directory explorer with progress, large-file finder, and S.M.A.R.T. health status
- Service listing on Linux `systemd` hosts with state filters/counts, mask/unmask actions, logs, failure reason, and richer exit/runtime details
- Logs viewer tools: source selector, interactive level filtering, regex search/highlight with match navigation (n/N), autoscroll, and richer error-spike window detection
- Hardware info: CPU model/cache, temperatures, GPU details, battery/power status, users/login history, SSH sessions, failed logins, firewall, and SELinux/AppArmor snapshot
- GPU tab: device telemetry (utilization, VRAM, temperature, power, fan) with history graphs and per-process GPU memory table
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
