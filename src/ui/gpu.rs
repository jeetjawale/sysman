use crate::app::App;
use crate::collectors::{self, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Cell, Row, Sparkline, Table},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, snapshot: &Snapshot) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(10)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(16),
            Constraint::Percentage(16),
            Constraint::Percentage(17),
            Constraint::Percentage(17),
            Constraint::Percentage(17),
            Constraint::Percentage(17),
        ])
        .split(sections[0]);

    let devices = &snapshot.gpu_runtime.devices;
    let backend = &snapshot.gpu_runtime.backend;
    let device_count = devices.len();
    let util_now = app.histories.gpu_util.back().copied().unwrap_or(0);
    let vram_now = app.histories.gpu_vram.back().copied().unwrap_or(0);
    let temp_now = app.histories.gpu_temp.back().copied().unwrap_or(0);
    let power_now = app.histories.gpu_power.back().copied().unwrap_or(0);
    let fan_now = app.histories.gpu_fan.back().copied().unwrap_or(0);

    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "GPU",
            &device_count.to_string(),
            backend,
            if device_count > 0 {
                app.theme.status_good
            } else {
                app.theme.status_warn
            },
            app.animation_frame,
        ),
        top[0],
    );
    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Util",
            &format!("{util_now}%"),
            "avg",
            widgets::status_color(&app.theme, util_now as f64),
            app.animation_frame,
        ),
        top[1],
    );
    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "VRAM",
            &format!("{vram_now}%"),
            "avg",
            widgets::status_color(&app.theme, vram_now as f64),
            app.animation_frame,
        ),
        top[2],
    );
    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Temp",
            &format!("{temp_now}°C"),
            "avg",
            widgets::status_color(&app.theme, temp_now as f64),
            app.animation_frame,
        ),
        top[3],
    );
    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Power",
            &format!("{power_now}W"),
            "avg",
            app.theme.status_info,
            app.animation_frame,
        ),
        top[4],
    );
    frame.render_widget(
        widgets::metric_card(
            &app.theme,
            "Fan",
            &format!("{fan_now}%"),
            "avg",
            widgets::status_color(&app.theme, fan_now as f64),
            app.animation_frame,
        ),
        top[5],
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(sections[1]);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(body[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
        ])
        .split(body[1]);

    frame.render_widget(gpu_device_table(app, snapshot), left[0]);
    frame.render_widget(gpu_process_table(app, snapshot), left[1]);
    frame.render_widget(
        history_panel(
            app,
            "Utilization %",
            &app.histories.gpu_util,
            app.theme.status_info,
            100,
        ),
        right[0],
    );
    frame.render_widget(
        history_panel(
            app,
            "VRAM %",
            &app.histories.gpu_vram,
            app.theme.status_warn,
            100,
        ),
        right[1],
    );
    frame.render_widget(
        history_panel(
            app,
            "Temp °C",
            &app.histories.gpu_temp,
            app.theme.status_error,
            120,
        ),
        right[2],
    );
    frame.render_widget(
        history_panel(
            app,
            "Power W",
            &app.histories.gpu_power,
            app.theme.brand,
            dynamic_max(&app.histories.gpu_power, 100),
        ),
        right[3],
    );
    frame.render_widget(
        history_panel(
            app,
            "Fan %",
            &app.histories.gpu_fan,
            app.theme.status_good,
            100,
        ),
        right[4],
    );
}

fn gpu_device_table(app: &App, snapshot: &Snapshot) -> Table<'static> {
    let rows: Vec<Row> = snapshot
        .gpu_runtime
        .devices
        .iter()
        .enumerate()
        .map(|(idx, gpu)| {
            let row_style = if idx % 2 == 0 {
                Style::default()
            } else {
                Style::default().bg(app.theme.alt_row_bg)
            };
            let mem = match (gpu.memory_used_mib, gpu.memory_total_mib) {
                (Some(used), Some(total)) => format!("{used}/{total} MiB"),
                _ => "-".into(),
            };
            Row::new(vec![
                Cell::from(gpu.index.to_string()),
                Cell::from(collectors::truncate(&gpu.name, 22)),
                Cell::from(
                    gpu.utilization_pct
                        .map(|value| format!("{value:.0}%"))
                        .unwrap_or_else(|| "-".into()),
                ),
                Cell::from(mem),
                Cell::from(
                    gpu.temperature_c
                        .map(|value| format!("{value:.0}°C"))
                        .unwrap_or_else(|| "-".into()),
                ),
                Cell::from(
                    gpu.power_w
                        .map(|value| format!("{value:.0}W"))
                        .unwrap_or_else(|| "-".into()),
                ),
                Cell::from(
                    gpu.fan_pct
                        .map(|value| format!("{value:.0}%"))
                        .unwrap_or_else(|| "-".into()),
                ),
            ])
            .style(row_style)
        })
        .collect();

    Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Percentage(34),
            Constraint::Length(8),
            Constraint::Length(16),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["GPU", "Name", "Util", "VRAM", "Temp", "Power", "Fan"]).style(
            Style::default()
                .fg(app.theme.active_tab)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(widgets::active_block(
        &app.theme,
        "GPU Telemetry",
        app.animation_frame,
    ))
}

fn gpu_process_table(app: &App, snapshot: &Snapshot) -> Table<'static> {
    let rows: Vec<Row> = if snapshot.gpu_runtime.processes.is_empty() {
        vec![Row::new(vec![
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("No active GPU processes"),
            Cell::from("-"),
        ])]
    } else {
        snapshot
            .gpu_runtime
            .processes
            .iter()
            .take(12)
            .map(|proc_row| {
                Row::new(vec![
                    Cell::from(
                        proc_row
                            .gpu_index
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".into()),
                    ),
                    Cell::from(proc_row.pid.to_string()),
                    Cell::from(collectors::truncate(&proc_row.process_name, 20)),
                    Cell::from(
                        proc_row
                            .used_memory_mib
                            .map(|value| format!("{value} MiB"))
                            .unwrap_or_else(|| "-".into()),
                    ),
                ])
            })
            .collect()
    };

    Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Percentage(50),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new(vec!["GPU", "PID", "Process", "VRAM"]).style(
            Style::default()
                .fg(app.theme.active_tab)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(widgets::block(&app.theme, "GPU Processes"))
}

fn history_panel<'a>(
    app: &App,
    title: &'a str,
    history: &std::collections::VecDeque<u64>,
    color: Color,
    max: u64,
) -> Sparkline<'a> {
    let data: Vec<u64> = history.iter().copied().collect();
    Sparkline::default()
        .block(widgets::surface_block(&app.theme, title))
        .style(Style::default().fg(color).bg(app.theme.alt_row_bg))
        .data(&data)
        .max(max.max(1))
        .direction(ratatui::widgets::RenderDirection::LeftToRight)
}

fn dynamic_max(history: &std::collections::VecDeque<u64>, floor: u64) -> u64 {
    history.iter().copied().max().unwrap_or(floor).max(floor)
}
