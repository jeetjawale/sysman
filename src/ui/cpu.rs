use crate::app::App;
use crate::collectors::Snapshot;
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Paragraph, Sparkline},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, snapshot: &Snapshot) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Min(10),
        ])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(23),
            Constraint::Percentage(23),
            Constraint::Percentage(24),
        ])
        .split(sections[0]);

    frame.render_widget(
        widgets::gauge_card(
            &app.theme,
            "CPU Usage",
            snapshot.cpu_usage as f64,
            &format!("{:.1}% total", snapshot.cpu_usage),
            app.theme.status_info,
            app.animation_frame,
        ),
        top[0],
    );
    frame.render_widget(frequency_panel(app, snapshot), top[1]);
    frame.render_widget(scheduler_panel(app, snapshot), top[2]);
    frame.render_widget(thermal_panel(app, snapshot), top[3]);

    let trends = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(sections[1]);
    frame.render_widget(
        widgets::spark_panel(
            &app.theme,
            "CPU % History",
            &app.histories.cpu_total,
            app.theme.status_info,
            app.animation_frame,
        ),
        trends[0],
    );
    frame.render_widget(temperature_history_panel(app), trends[1]);

    render_per_core_grid(frame, sections[2], app, snapshot);
}

fn frequency_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let freq = snapshot
        .cpu_runtime
        .current_freq_mhz
        .map(|value| format!("{value} MHz"))
        .unwrap_or_else(|| "n/a".into());
    let governor = snapshot
        .cpu_runtime
        .governor
        .as_deref()
        .unwrap_or("n/a")
        .to_string();
    let lines = vec![
        widgets::kv_line(&app.theme, "Governor", &governor),
        widgets::kv_line(&app.theme, "Current Freq", &freq),
        widgets::kv_line(&app.theme, "Cores", &snapshot.cpu_cores.to_string()),
    ];
    Paragraph::new(lines).block(widgets::block(&app.theme, "Frequency"))
}

fn scheduler_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let context_total = snapshot
        .cpu_runtime
        .context_switches
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".into());
    let context_rate = app
        .context_switch_rate
        .map(|value| format!("{value}/s"))
        .unwrap_or_else(|| "n/a".into());
    let lines = vec![
        widgets::kv_line(&app.theme, "Ctx Switches", &context_total),
        widgets::kv_line(&app.theme, "Rate", &context_rate),
        widgets::kv_line(&app.theme, "Load Avg", &snapshot.load_average),
    ];
    Paragraph::new(lines).block(widgets::block(&app.theme, "Scheduler"))
}

fn thermal_panel(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    let temp = snapshot.cpu_runtime.temperature_c.unwrap_or(0.0);
    let temp_color = if snapshot.cpu_runtime.temperature_c.is_some() {
        widgets::status_color(&app.theme, temp)
    } else {
        app.theme.text_muted
    };
    let throttle_total = snapshot
        .cpu_runtime
        .throttle_count
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".into());
    let throttle_delta = app
        .throttle_events_delta
        .map(|value| format!("+{value}"))
        .unwrap_or_else(|| "n/a".into());
    let throttle_state = if app.throttle_events_delta.unwrap_or(0) > 0 {
        ("Active throttling", app.theme.status_error)
    } else if snapshot.cpu_runtime.throttle_count.unwrap_or(0) > 0 {
        ("Past throttling", app.theme.status_warn)
    } else {
        ("No throttling", app.theme.status_good)
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("CPU Temp: ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                if snapshot.cpu_runtime.temperature_c.is_some() {
                    format!("{temp:.1}°C")
                } else {
                    "n/a".into()
                },
                Style::default().fg(temp_color),
            ),
        ]),
        widgets::kv_line(&app.theme, "Throttle Total", &throttle_total),
        widgets::kv_line(&app.theme, "Throttle Delta", &throttle_delta),
        Line::from(vec![
            Span::styled("State: ", Style::default().fg(app.theme.text_muted)),
            Span::styled(throttle_state.0, Style::default().fg(throttle_state.1)),
        ]),
    ];
    Paragraph::new(lines).block(widgets::block(&app.theme, "Thermal / Throttle"))
}

fn temperature_history_panel(app: &App) -> Sparkline<'static> {
    let data: Vec<u64> = app.histories.cpu_temp.iter().copied().collect();
    let current = app.histories.cpu_temp.back().copied().unwrap_or(0) as f64;
    let color = if app.histories.cpu_temp.is_empty() {
        app.theme.text_muted
    } else {
        widgets::status_color(&app.theme, current)
    };
    Sparkline::default()
        .block(widgets::surface_block(&app.theme, "CPU Temp History"))
        .style(Style::default().fg(color).bg(app.theme.alt_row_bg))
        .data(&data)
        .max(120)
        .direction(ratatui::widgets::RenderDirection::LeftToRight)
}

fn render_per_core_grid(frame: &mut Frame, area: Rect, app: &App, snapshot: &Snapshot) {
    let core_count = snapshot.cpu_per_core.len().max(1);
    let columns = 4usize.min(core_count);
    let rows = core_count.div_ceil(columns);

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Ratio(1, rows as u32); rows])
        .split(area);

    for (row_index, row_area) in vertical.iter().enumerate() {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Ratio(1, columns as u32); columns])
            .split(*row_area);

        for (col_index, cell) in horizontal.iter().enumerate() {
            let core_index = row_index * columns + col_index;
            if core_index >= core_count {
                continue;
            }
            let history = app
                .histories
                .per_core
                .get(core_index)
                .cloned()
                .unwrap_or_default();
            let data: Vec<u64> = history.iter().copied().collect();
            frame.render_widget(
                Sparkline::default()
                    .block(widgets::block(
                        &app.theme,
                        &format!(
                            "Core {}  {:.1}%",
                            core_index, snapshot.cpu_per_core[core_index]
                        ),
                    ))
                    .style(Style::default().fg(widgets::status_color(
                        &app.theme,
                        snapshot.cpu_per_core[core_index] as f64,
                    )))
                    .data(&data)
                    .max(100),
                *cell,
            );
        }
    }
}
