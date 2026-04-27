use crate::cli::{Command, ProcessSort, ServiceAction, ServiceState};
use crate::collectors::{self, RealProvider};
use anyhow::Result;
use sysinfo::System;

pub fn execute(command: Command) -> Result<()> {
    match command {
        Command::Tui => Ok(()),
        Command::Summary => print_summary(),
        Command::System => print_system(),
        Command::Memory => print_memory(),
        Command::Disks => print_disks(),
        Command::Processes { limit, sort } => print_processes(limit, sort),
        Command::Services { state, limit } => print_services(state, limit),
        Command::Service { name, action } => handle_service_action(&name, action),
    }
}

fn print_summary() -> Result<()> {
    let mut sys = System::new_all();
    sys.refresh_all();
    let provider = RealProvider;
    let filter = collectors::CollectionFilter {
        medium_lane: true,
        slow_lane: true,
        ..Default::default()
    };
    let snapshot =
        collectors::collect_snapshot(&mut sys, &provider, ServiceState::Running, 10, filter, None)?;

    println!("System Summary");
    println!("==============");
    println!("Host: {}", snapshot.host);
    println!("OS: {}", snapshot.os);
    println!("Kernel: {}", snapshot.kernel);
    println!("Uptime: {}", collectors::format_duration(snapshot.uptime));
    println!(
        "CPU: {:.1}% total usage across {} cores",
        snapshot.cpu_usage, snapshot.cpu_cores
    );
    println!(
        "Memory: {} / {} used",
        collectors::format_bytes(snapshot.used_memory),
        collectors::format_bytes(snapshot.total_memory)
    );
    println!(
        "Swap: {} / {} used",
        collectors::format_bytes(snapshot.used_swap),
        collectors::format_bytes(snapshot.total_swap)
    );
    match snapshot
        .disks
        .iter()
        .max_by(|a, b| a.usage.total_cmp(&b.usage))
    {
        Some(disk) => {
            println!(
                "Disks: {} mounted, busiest {} at {:.1}%",
                snapshot.disks.len(),
                disk.mount,
                disk.usage
            );
        }
        None => println!("Disks: {} mounted", snapshot.disks.len()),
    }

    if let Some(service_summary) = snapshot.service_summary {
        println!(
            "Services: {} running, {} failed",
            service_summary.running, service_summary.failed
        );
    } else {
        println!("Services: systemd data unavailable");
    }

    Ok(())
}

fn print_system() -> Result<()> {
    let mut system = System::new_all();
    system.refresh_all();
    let provider = RealProvider;
    let hardware = collectors::host::collect_hardware_info(&provider);

    println!("System Information");
    println!("==================");
    println!("Host Name: {}", collectors::host::host_name());
    println!(
        "OS Version: {}",
        System::long_os_version().unwrap_or_else(|| "unknown".into())
    );
    println!(
        "Distribution: {}",
        collectors::host::linux_distribution().unwrap_or_else(|| "unknown".into())
    );
    println!(
        "Kernel: {}",
        System::kernel_version().unwrap_or_else(|| "unknown".into())
    );
    println!("Architecture: {}", std::env::consts::ARCH);
    println!("Uptime: {}", collectors::format_duration(System::uptime()));
    println!("Boot Time: {}", System::boot_time());
    println!("CPU Cores: {}", system.cpus().len());
    println!("Process Count: {}", system.processes().len());
    println!("CPU Model: {}", hardware.cpu_model);
    println!("CPU Cache: {}", hardware.cpu_cache);
    if let Some(temp) = hardware.temperatures.first() {
        println!("Temperature: {temp}");
    }
    if let Some(gpu) = hardware.gpu_info.first() {
        println!("GPU: {gpu}");
    }
    if let Some(battery) = hardware.battery_info.first() {
        println!("Battery: {battery}");
    }
    if let Some(user) = hardware.login_users.first() {
        println!("Logged-in User: {user}");
    }
    if let Some(login) = hardware.login_history.first() {
        println!("Last Login: {login}");
    }
    if let Some(ssh) = hardware.ssh_sessions.first() {
        println!("SSH Session: {ssh}");
    }
    if let Some(failed) = hardware.failed_logins.first() {
        println!("Failed Login: {failed}");
    }
    if let Some(firewall) = hardware.firewall_status.first() {
        println!("Firewall: {firewall}");
    }
    if let Some(module) = hardware.security_modules.first() {
        println!("Security Module: {module}");
    }

    Ok(())
}

fn print_memory() -> Result<()> {
    let mut system = System::new_all();
    system.refresh_memory();

    println!("Memory");
    println!("======");
    println!(
        "RAM Used: {} / {} ({:.1}%)",
        collectors::format_bytes(system.used_memory()),
        collectors::format_bytes(system.total_memory()),
        collectors::percentage(system.used_memory(), system.total_memory())
    );
    println!(
        "RAM Available: {}",
        collectors::format_bytes(system.available_memory())
    );
    println!(
        "Swap Used: {} / {} ({:.1}%)",
        collectors::format_bytes(system.used_swap()),
        collectors::format_bytes(system.total_swap()),
        collectors::percentage(system.used_swap(), system.total_swap())
    );

    Ok(())
}

fn print_disks() -> Result<()> {
    let provider = RealProvider;
    let disks = collectors::storage::collect_disks(&provider);
    println!("Disks");
    println!("=====");
    println!(
        "{:<20} {:<10} {:>12} {:>12} {:>8}",
        "Mount", "FS", "Used", "Total", "Use%"
    );

    for disk in &disks {
        println!(
            "{:<20} {:<10} {:>12} {:>12} {:>7.1}",
            disk.mount,
            disk.filesystem,
            collectors::format_bytes(disk.used),
            collectors::format_bytes(disk.total),
            disk.usage
        );
    }

    Ok(())
}

fn print_processes(limit: usize, sort: ProcessSort) -> Result<()> {
    let sys = sysinfo::System::new_all();
    let processes = collectors::procs::collect_processes(&sys, limit, sort);

    println!("Processes");
    println!("=========");
    println!(
        "{:<8} {:<28} {:>8} {:>12} {:>10}",
        "PID", "Name", "CPU%", "Memory", "Status"
    );

    for process in processes {
        println!(
            "{:<8} {:<28} {:>8.1} {:>12} {:>10}",
            process.pid,
            collectors::truncate(&process.name, 28),
            process.cpu,
            collectors::format_bytes(process.memory),
            process.status
        );
    }

    Ok(())
}

fn print_services(state: ServiceState, limit: usize) -> Result<()> {
    let provider = RealProvider;
    let services = collectors::systemd::collect_services(&provider, state, limit)?;

    println!("Services");
    println!("========");
    println!("{:<40} {:<12} {:<12}", "Name", "Active", "Sub");

    for service in services {
        println!(
            "{:<40} {:<12} {:<12}",
            service.name, service.active, service.sub
        );
    }

    Ok(())
}

fn handle_service_action(name: &str, action: ServiceAction) -> Result<()> {
    collectors::systemd::ensure_linux_systemd()?;

    let action_name = match action {
        ServiceAction::Status => "status",
        ServiceAction::Start => "start",
        ServiceAction::Stop => "stop",
        ServiceAction::Restart => "restart",
    };

    let provider = RealProvider;
    let output = collectors::systemd::run_systemctl(&provider, &[action_name, name])?;
    println!("{output}");
    Ok(())
}
