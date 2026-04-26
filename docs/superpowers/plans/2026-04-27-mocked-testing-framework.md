# Mocked Testing Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decouple system collectors from direct shell command execution using a mockable `CommandProvider` trait, enabling platform-independent testing and CI validation.

**Architecture:** Introduce a `CommandProvider` trait and two implementations: `RealProvider` (invokes `std::process::Command`) and `MockProvider` (returns canned responses). Refactor all collector functions to accept a `&dyn CommandProvider`.

**Tech Stack:** Rust, standard library (`std::process::Command`), `cargo test`.

---

## File Mapping
- `src/collectors/provider.rs`: New file for trait and implementations.
- `src/collectors/mod.rs`: Register new module, update `collect_snapshot`.
- `src/collectors/logs.rs`: Update utility functions to use provider.
- `src/collectors/storage.rs`: Update `collect_disks`, `collect_large_files`, etc.
- `src/collectors/host.rs`: Update GPU, sensors, and security module collectors.
- `src/collectors/netstat.rs`: Update connection and bandwidth collectors.
- `src/collectors/procs.rs`: Update port and group collectors.
- `src/collectors/systemd.rs`: Update `systemctl` and `journalctl` wrappers.
- `src/collectors/containers.rs`: Update Docker/Podman collectors.
- `src/app.rs`: Store and pass the provider to the refresh loop.

---

## Tasks

### Task 1: Foundation (Trait & Providers)

**Files:**
- Create: `src/collectors/provider.rs`
- Modify: `src/collectors/mod.rs`

- [ ] **Step 1: Create the provider module**

```rust
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
```

- [ ] **Step 2: Register module in `src/collectors/mod.rs`**

Add `pub mod provider;` and update re-exports.

- [ ] **Step 3: Commit**

```bash
git add src/collectors/provider.rs src/collectors/mod.rs
git commit -m "feat: add CommandProvider trait and RealProvider implementation"
```

### Task 2: Refactor Logs & Base Utils

**Files:**
- Modify: `src/collectors/logs.rs`
- Modify: `src/collectors/mod.rs`

- [ ] **Step 1: Update `logs.rs` utility**

Change `run_lines` to accept `provider: &dyn CommandProvider`.

```rust
fn run_lines(provider: &dyn CommandProvider, command: &str, args: &[&str]) -> Vec<String> {
    let Ok(output) = provider.run(command, args) else {
        return Vec::new();
    };
    if !output.success {
        return Vec::new();
    }
    output.stdout.lines().map(|line| line.to_string()).collect()
}
```

Update all public `collect_*` functions in `logs.rs` to take the provider.

- [ ] **Step 2: Update `collect_snapshot` signature**

Modify `collect_snapshot` to accept `provider: &dyn CommandProvider`.

- [ ] **Step 3: Commit**

```bash
git add src/collectors/logs.rs src/collectors/mod.rs
git commit -m "refactor: update logs collector to use CommandProvider"
```

### Task 3: Refactor Systemd & Container Collectors

**Files:**
- Modify: `src/collectors/systemd.rs`
- Modify: `src/collectors/containers.rs`

- [ ] **Step 1: Update `systemd.rs`**

Update `run_systemctl`, `collect_service_logs`, and `collect_service_failure_details` to use the provider.

- [ ] **Step 2: Update `containers.rs`**

Update `collect_docker`, `collect_podman`, `act_on_container`, and `get_container_logs`.

- [ ] **Step 3: Commit**

```bash
git add src/collectors/systemd.rs src/collectors/containers.rs
git commit -m "refactor: update systemd and container collectors to use CommandProvider"
```

### Task 4: Refactor Host & Storage Collectors

**Files:**
- Modify: `src/collectors/host.rs`
- Modify: `src/collectors/storage.rs`

- [ ] **Step 1: Update `host.rs`**

Update all GPU, sensors, and CLI-based collectors (who, last, ss, journalctl, firewall, etc.).

- [ ] **Step 2: Update `storage.rs`**

Update `collect_inode_usage_map`, `collect_large_files`, and `collect_smart_health`.

- [ ] **Step 3: Commit**

```bash
git add src/collectors/host.rs src/collectors/storage.rs
git commit -m "refactor: update host and storage collectors to use CommandProvider"
```

### Task 5: App Integration & Final Verification

**Files:**
- Modify: `src/app.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs` (if necessary)

- [ ] **Step 1: Update `App` struct**

Add `pub provider: Box<dyn CommandProvider>` to `App`.

- [ ] **Step 2: Update `refresh` loop**

Pass `app.provider.as_ref()` to `collect_snapshot`.

- [ ] **Step 3: Run full verification**

```bash
cargo check
cargo test
```

- [ ] **Step 4: Commit**

```bash
git add src/app.rs src/lib.rs
git commit -m "feat: integrate CommandProvider into App refresh loop"
```
