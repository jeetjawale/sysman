# Mocked Testing Framework Design

## 1. Overview
The `sysman` collectors currently rely on direct calls to `std::process::Command`, making them difficult to test in isolation or in CI environments without specific system tools (like `nvidia-smi` or `docker`). 

This design introduces a **Mockable Command Runner** using trait-based injection. This allows us to simulate system responses, verify error handling, and run tests on any platform.

## 2. Architecture

### Core Trait: `CommandProvider`
We will define a central trait that abstracts the "run a command and get output" behavior.

```rust
pub trait CommandProvider: Send + Sync {
    fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput, String>;
}

pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}
```

### Implementations
1. **`RealCommandProvider`**: The production implementation using `std::process::Command`.
2. **`MockCommandProvider`**: The test implementation that holds a map of `(command, args)` to `CommandOutput`.

## 3. Impacted Components
All modules in `src/collectors/*.rs` will be refactored to accept a `&dyn CommandProvider`.

### Example Refactoring (`src/collectors/logs.rs`):

**Before:**
```rust
fn run_lines(command: &str, args: &[&str]) -> Vec<String> {
    let output = ProcessCommand::new(command).args(args).output();
    // ...
}
```

**After:**
```rust
fn run_lines(provider: &dyn CommandProvider, command: &str, args: &[&str]) -> Vec<String> {
    let output = provider.run(command, args);
    // ...
}
```

### Top-Level Snapshot
The `collect_snapshot` function in `src/collectors/mod.rs` will be updated to take a `provider: &dyn CommandProvider`, which it will pass down to all sub-collectors.

## 4. Implementation Strategy

1. **Phase 1: Foundation**: Create `src/collectors/provider.rs` with the trait and basic implementations.
2. **Phase 2: Core Migration**: Update `logs.rs` and `mod.rs` as a "pilot" for the new pattern.
3. **Phase 3: Global Rollout**: Progressively update `host.rs`, `storage.rs`, `netstat.rs`, `procs.rs`, `systemd.rs`, and `containers.rs`.
4. **Phase 4: Testing**: Add unit tests for each collector using the `MockCommandProvider` to simulate various scenarios (e.g., failed `docker stats`, empty `df` output, etc.).

## 5. Success Criteria
- `cargo test` runs and passes on non-Linux systems (like macOS or CI environments).
- Each collector has at least one unit test that uses a mocked response.
- No performance regression in production (TUI refresh rate stays stable).

---

Does this architecture look right so far? I'll cover the detailed data flow and testing examples in the next section once approved.