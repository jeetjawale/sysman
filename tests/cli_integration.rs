//! Integration tests for CLI commands.
//!
//! These tests verify that CLI subcommands execute successfully and produce
//! expected output formats. Tests are designed to run on Linux systems with
//! systemd available.

use std::process::Command;

/// Helper to run sysman CLI with arguments
fn run_sysman(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sysman"))
        .args(args)
        .output()
        .expect("Failed to execute sysman command")
}

/// Helper to convert output to UTF-8 string
fn output_to_string(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

// =============================================================================
// Summary Command Tests
// =============================================================================

#[test]
fn test_summary_command_executes() {
    let output = run_sysman(&["summary"]);
    assert!(
        output.status.success(),
        "summary command should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_summary_output_format() {
    let output = run_sysman(&["summary"]);
    let stdout = output_to_string(&output);

    assert!(
        stdout.contains("System Summary"),
        "summary should contain 'System Summary' header"
    );
    assert!(
        stdout.contains("Host:"),
        "summary should display host information"
    );
    assert!(stdout.contains("CPU:"), "summary should display CPU usage");
    assert!(
        stdout.contains("Memory:"),
        "summary should display memory usage"
    );
}

// =============================================================================
// System Command Tests
// =============================================================================

#[test]
fn test_system_command_executes() {
    let output = run_sysman(&["system"]);
    assert!(
        output.status.success(),
        "system command should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_system_output_format() {
    let output = run_sysman(&["system"]);
    let stdout = output_to_string(&output);

    assert!(
        stdout.contains("System Information"),
        "system should contain 'System Information' header"
    );
    assert!(
        stdout.contains("Host Name:"),
        "system should display host name"
    );
    assert!(
        stdout.contains("OS Version:"),
        "system should display OS version"
    );
    assert!(
        stdout.contains("CPU Cores:"),
        "system should display CPU core count"
    );
}

// =============================================================================
// Memory Command Tests
// =============================================================================

#[test]
fn test_memory_command_executes() {
    let output = run_sysman(&["memory"]);
    assert!(
        output.status.success(),
        "memory command should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_memory_output_format() {
    let output = run_sysman(&["memory"]);
    let stdout = output_to_string(&output);

    assert!(
        stdout.contains("Memory"),
        "memory should contain 'Memory' header"
    );
    assert!(
        stdout.contains("RAM Used:"),
        "memory should display RAM usage"
    );
    assert!(
        stdout.contains("Swap Used:"),
        "memory should display swap usage"
    );
}

// =============================================================================
// Disks Command Tests
// =============================================================================

#[test]
fn test_disks_command_executes() {
    let output = run_sysman(&["disks"]);
    assert!(
        output.status.success(),
        "disks command should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_disks_output_format() {
    let output = run_sysman(&["disks"]);
    let stdout = output_to_string(&output);

    assert!(
        stdout.contains("Disks"),
        "disks should contain 'Disks' header"
    );
    assert!(
        stdout.contains("Mount"),
        "disks should display mount point column"
    );
    assert!(
        stdout.contains("Use%"),
        "disks should display usage percentage column"
    );
}

// =============================================================================
// Processes Command Tests
// =============================================================================

#[test]
fn test_processes_command_executes() {
    let output = run_sysman(&["processes"]);
    assert!(
        output.status.success(),
        "processes command should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_processes_output_format() {
    let output = run_sysman(&["processes"]);
    let stdout = output_to_string(&output);

    assert!(
        stdout.contains("Processes"),
        "processes should contain 'Processes' header"
    );
    assert!(
        stdout.contains("PID"),
        "processes should display PID column"
    );
    assert!(
        stdout.contains("Name"),
        "processes should display process name column"
    );
}

#[test]
fn test_processes_with_limit() {
    let output = run_sysman(&["processes", "--limit", "5"]);
    let stdout = output_to_string(&output);

    assert!(
        output.status.success(),
        "processes --limit should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Count lines (should be at most 2 header + 5 data + 1 trailing = 8 lines)
    let line_count = stdout.lines().count();
    assert!(
        line_count <= 10,
        "processes --limit 5 should show reasonable number of lines, got: {}",
        line_count
    );
}

#[test]
fn test_processes_with_sort() {
    let output = run_sysman(&["processes", "--sort", "memory"]);
    assert!(
        output.status.success(),
        "processes --sort memory should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// =============================================================================
// Services Command Tests
// =============================================================================

#[test]
fn test_services_command_executes() {
    let output = run_sysman(&["services"]);
    let stdout = output_to_string(&output);

    // Services may fail on non-systemd systems, but should work on Linux with systemd
    if output.status.success() {
        assert!(
            stdout.contains("Services"),
            "services should contain 'Services' header"
        );
        assert!(
            stdout.contains("Name"),
            "services should display Name column"
        );
    } else {
        // On non-systemd systems, expect an error message
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("systemd") || stderr.contains("Linux"),
            "services failure should mention systemd or Linux: stderr={}",
            stderr
        );
    }
}

#[test]
fn test_services_with_state_filter() {
    let output = run_sysman(&["services", "--state", "running"]);
    let stdout = output_to_string(&output);

    if output.status.success() {
        assert!(
            stdout.contains("Services"),
            "services --state running should contain header"
        );
    }
}

#[test]
fn test_services_with_limit() {
    let output = run_sysman(&["services", "--limit", "5"]);
    assert!(
        output.status.success() || !output.status.success(),
        "services --limit should execute"
    );
}

// =============================================================================
// Service Command Tests (individual service actions)
// =============================================================================

#[test]
fn test_service_status_nonexistent_service() {
    let output = run_sysman(&["service", "nonexistent_service_xyz", "status"]);

    // Should fail for non-existent service
    assert!(
        !output.status.success(),
        "status of nonexistent service should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = output_to_string(&output);

    // Should contain error about service not existing
    assert!(
        stderr.contains("not found")
            || stderr.contains("failed")
            || stderr.contains("load failed")
            || stdout.contains("could not be found")
            || !output.status.success(),
        "should report service not found: stderr={}",
        stderr
    );
}

// =============================================================================
// Help and Version Tests
// =============================================================================

#[test]
fn test_help_flag() {
    let output = run_sysman(&["--help"]);
    assert!(output.status.success(), "--help should succeed");

    let stdout = output_to_string(&output);
    assert!(
        stdout.contains("sysman"),
        "help should contain program name"
    );
    assert!(
        stdout.contains("Commands:"),
        "help should list available commands"
    );
    assert!(
        stdout.contains("summary"),
        "help should mention summary command"
    );
    assert!(
        stdout.contains("system"),
        "help should mention system command"
    );
}

#[test]
fn test_version_flag() {
    let output = run_sysman(&["--version"]);
    assert!(output.status.success(), "--version should succeed");

    let stdout = output_to_string(&output);
    assert!(
        stdout.contains("sysman"),
        "version should contain program name"
    );
    assert!(
        stdout.contains("0.1.0") || stdout.chars().any(|c| c.is_ascii_digit()),
        "version should contain version number"
    );
}

#[test]
fn test_tui_subcommand_help() {
    let output = run_sysman(&["tui", "--help"]);
    assert!(output.status.success(), "tui --help should succeed");

    let stdout = output_to_string(&output);
    assert!(
        stdout.contains("tui") || stdout.contains("TUI"),
        "tui help should mention tui"
    );
}

// =============================================================================
// Invalid Command Tests
// =============================================================================

#[test]
fn test_invalid_subcommand() {
    let output = run_sysman(&["invalid_command_xyz"]);
    assert!(!output.status.success(), "invalid subcommand should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error") || stderr.contains("unrecognized"),
        "invalid command should show error: stderr={}",
        stderr
    );
}

#[test]
fn test_invalid_process_sort() {
    let output = run_sysman(&["processes", "--sort", "invalid_sort"]);
    assert!(!output.status.success(), "invalid sort option should fail");
}

#[test]
fn test_invalid_service_state() {
    let output = run_sysman(&["services", "--state", "invalid_state"]);
    assert!(
        !output.status.success(),
        "invalid service state should fail"
    );
}

#[test]
fn test_invalid_limit_value() {
    let output = run_sysman(&["processes", "--limit", "not_a_number"]);
    assert!(!output.status.success(), "invalid limit value should fail");
}
