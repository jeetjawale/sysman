use crate::app::App;
use crate::collectors::{self, Snapshot};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Paragraph, Sparkline},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, snapshot: &Snapshot) {

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(6),
            Constraint::Min(10),
        ])
        .split(area);

    // -- Top: identity + runtime -------------------------------------------
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(sections[0]);

    frame.render_widget(system_identity(app, snapshot), top[0]);
    frame.render_widget(system_runtime(app, snapshot), top[1]);

    // -- Resource gauges ----------------------------------------------------
    let gauges = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(sections[1]);

    frame.render_widget(
        widgets::gauge_card(
            &app.theme,
            "CPU",
            snapshot.cpu_usage as f64,
            &format!("{:.1}% total", snapshot.cpu_usage),
            app.theme.status_info,
            app.animation_frame,
        ),
        gauges[0],
    );
    frame.render_widget(
        widgets::gauge_card(
            &app.theme,
            "Memory",
            collectors::percentage(snapshot.used_memory, snapshot.total_memory),
            &format!(
                "{} / {}",
                collectors::format_bytes(snapshot.used_memory),
                collectors::format_bytes(snapshot.total_memory)
            ),
            app.theme.status_warn,
            app.animation_frame,
        ),
        gauges[1],
    );
    frame.render_widget(
        widgets::gauge_card(
            &app.theme,
            "Swap",
            collectors::percentage(snapshot.used_swap, snapshot.total_swap),
            &format!(
                "{} / {}",
                collectors::format_bytes(snapshot.used_swap),
                collectors::format_bytes(snapshot.total_swap)
            ),
            app.theme.status_error,
            app.animation_frame,
        ),
        gauges[2],
    );

    // -- Sparkline trends ---------------------------------------------------
    let trends = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(sections[2]);

    frame.render_widget(
        widgets::spark_panel(
            &app.theme,
            "CPU History",
            &app.histories.cpu_total,
            app.theme.status_info,
            app.animation_frame,
        ),
        trends[0],
    );
    frame.render_widget(
        widgets::spark_panel(
            &app.theme,
            "Memory History",
            &app.histories.memory_used,
            app.theme.status_warn,
            app.animation_frame,
        ),
        trends[1],
    );
    frame.render_widget(
        widgets::spark_panel(
            &app.theme,
            "Swap History",
            &app.histories.swap_used,
            app.theme.status_error,
            app.animation_frame,
        ),
        trends[2],
    );

    // -- Bottom: per-core sparkline grid -----------------------------------
    render_per_core_grid(frame, sections[3], app, snapshot);
}

// ---------------------------------------------------------------------------
// System tab sub-widgets
// ---------------------------------------------------------------------------

fn system_identity(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    Paragraph::new(vec![
        widgets::kv_line(&app.theme, "Host", &snapshot.host),
        widgets::kv_line(&app.theme, "Distribution", &snapshot.distro),
        widgets::kv_line(&app.theme, "Kernel", &snapshot.kernel),
    ])
    .block(widgets::block(&app.theme, "Identity"))
}

fn system_runtime(app: &App, snapshot: &Snapshot) -> Paragraph<'static> {
    Paragraph::new(vec![
        widgets::kv_line(
            &app.theme,
            "Uptime",
            &collectors::format_duration(snapshot.uptime),
        ),
        widgets::kv_line(&app.theme, "Boot Time", &snapshot.boot_time.to_string()),
        widgets::kv_line(&app.theme, "Load Avg", &snapshot.load_average),
    ])
    .block(widgets::block(&app.theme, "Runtime"))
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
