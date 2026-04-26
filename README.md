# Sysman

[![CI](https://github.com/jeetjawale/sysman/actions/workflows/ci.yml/badge.svg)](https://github.com/jeetjawale/sysman/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org)

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

## Installation

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- `lm-sensors` (for temperature monitoring)
- `smartmontools` (for S.M.A.R.T. disk health)
- `docker` / `podman` (for container management)

### Build from source
```bash
git clone https://github.com/jeetjawale/sysman
cd sysman
cargo build --release
```

### Install to path
```bash
cargo install --path .
```

## Usage

```bash
sysman         # Launch interactive TUI
sysman summary # Show high-level health
sysman system  # Show host/OS info
```

## Configuration

`sysman` loads settings from `~/.config/sysman/config.toml` (or `$XDG_CONFIG_HOME/sysman/config.toml`).

### Default Config
```toml
refresh_rate_ms = 1000

[thresholds]
cpu_high = 85.0
mem_high = 90.0
disk_high = 95.0

[theme]
brand_color = "#8B5CF6" # Modern Purple
```

- **refresh_rate_ms**: How often the UI updates.
- **thresholds**: Set your own limits for critical resource alerts.
- **brand_color**: Customize the accent color of the TUI (Hex format).

## Screenshots

![Dashboard Placeholder](https://via.placeholder.com/800x450.png?text=Sysman+Dashboard+TUI)
*Run `sysman` to see the live interactive dashboard.*

## Notes

- **Linux-First**: `sysman` is optimized for Linux. Some features (Services, GPU, Containers) require specific system tools or drivers.
- **Permissions**: Container lifecycle actions and some hardware stats may require `sudo` or group membership (e.g., `docker` group).

## License

MIT License - see [LICENSE](LICENSE) for details.
