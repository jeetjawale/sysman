use crate::app::App;
use crate::collectors::{self, Snapshot};
use crate::ui::widgets;
use ratatui::{prelude::*, widgets::Paragraph};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, snapshot: &Snapshot) {
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(8),
        ])
        .split(sections[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(10),
        ])
        .split(sections[1]);

    frame.render_widget(sensors_panel(app, snapshot), left[0]);
    frame.render_widget(power_panel(app, snapshot), left[1]);
    frame.render_widget(gpu_panel(app, snapshot), left[2]);
    frame.render_widget(users_panel(app, snapshot), right[0]);
    frame.render_widget(login_history_panel(app, snapshot), right[1]);
    frame.render_widget(security_snapshot_panel(app, snapshot), right[2]);
}

fn sensors_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines = vec![
        widgets::kv_line(
            &app.theme,
            "CPU Model",
            &collectors::truncate(&snapshot.hardware.cpu_model, 34),
        ),
        widgets::kv_line(&app.theme, "Arch", &snapshot.hardware.cpu_arch),
        widgets::kv_line(&app.theme, "Cache", &snapshot.hardware.cpu_cache),
    ];
    for row in snapshot.hardware.temperatures.iter().take(4) {
        lines.push(Line::from(collectors::truncate(row, 48)));
    }
    if snapshot.hardware.temperatures.is_empty() {
        lines.push(Line::from("No temperature data"));
    }
    Paragraph::new(lines).block(widgets::block(&app.theme, "Sensors"))
}

fn power_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    if snapshot.hardware.battery_info.is_empty() {
        lines.push(Line::from("No battery/power data"));
    } else {
        for row in snapshot.hardware.battery_info.iter().take(6) {
            lines.push(Line::from(collectors::truncate(row, 52)));
        }
    }
    Paragraph::new(lines).block(widgets::block(&app.theme, "Battery / Power"))
}

fn gpu_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    if snapshot.hardware.gpu_info.is_empty() {
        lines.push(Line::from("No GPU info"));
    } else {
        for row in snapshot.hardware.gpu_info.iter().take(6) {
            lines.push(Line::from(collectors::truncate(row, 56)));
        }
    }
    Paragraph::new(lines).block(widgets::block(&app.theme, "GPU"))
}

fn users_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    if snapshot.hardware.login_users.is_empty() {
        lines.push(Line::from("No active user sessions"));
    } else {
        for row in snapshot.hardware.login_users.iter().take(6) {
            lines.push(Line::from(collectors::truncate(row, 56)));
        }
    }
    Paragraph::new(lines).block(widgets::block(&app.theme, "Logged-in Users"))
}

fn login_history_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    if snapshot.hardware.login_history.is_empty() {
        lines.push(Line::from("No login history data"));
    } else {
        for row in snapshot.hardware.login_history.iter().take(10) {
            lines.push(Line::from(collectors::truncate(row, 80)));
        }
    }
    Paragraph::new(lines).block(widgets::block(&app.theme, "Login History"))
}

fn security_snapshot_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        "Security Modules",
        Style::default()
            .fg(app.theme.brand)
            .add_modifier(Modifier::BOLD),
    )));
    for row in snapshot.hardware.security_modules.iter().take(2) {
        lines.push(Line::from(collectors::truncate(row, 84)));
    }
    if snapshot.hardware.security_modules.is_empty() {
        lines.push(Line::from("No SELinux/AppArmor data"));
    }
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "Firewall",
        Style::default()
            .fg(app.theme.brand)
            .add_modifier(Modifier::BOLD),
    )));
    if snapshot.hardware.firewall_status.is_empty() {
        lines.push(Line::from("No firewall status data"));
    } else {
        for row in snapshot.hardware.firewall_status.iter().take(3) {
            lines.push(Line::from(collectors::truncate(row, 84)));
        }
    }
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "Active SSH Sessions",
        Style::default()
            .fg(app.theme.brand)
            .add_modifier(Modifier::BOLD),
    )));
    if snapshot.hardware.ssh_sessions.is_empty() {
        lines.push(Line::from("No active SSH sessions"));
    } else {
        for row in snapshot.hardware.ssh_sessions.iter().take(3) {
            lines.push(Line::from(collectors::truncate(row, 84)));
        }
    }
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "Failed Login Attempts",
        Style::default()
            .fg(app.theme.brand)
            .add_modifier(Modifier::BOLD),
    )));
    if snapshot.hardware.failed_logins.is_empty() {
        lines.push(Line::from("No recent failed logins"));
    } else {
        for row in snapshot.hardware.failed_logins.iter().take(3) {
            lines.push(Line::from(collectors::truncate(row, 84)));
        }
    }

    Paragraph::new(lines).block(widgets::block(&app.theme, "Security Snapshot"))
}
