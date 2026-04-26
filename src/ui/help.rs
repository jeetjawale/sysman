use crate::app::App;
use crate::collectors::Snapshot;
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Paragraph, Wrap},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, _snapshot: &Snapshot) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(14),
            Constraint::Length(6),
        ])
        .split(area);

    let tab_lines = vec![
        Line::from(vec![
            Span::styled(
                "Tab Map",
                Style::default()
                    .fg(app.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  (Tab / h/l or 1..9,0,?)",
                Style::default().fg(app.theme.text_muted),
            ),
        ]),
        Line::from(""),
        help_entry(
            &app.theme,
            "1 Overview",
            "health score, alerts, top offenders",
        ),
        help_entry(&app.theme, "2 CPU", "per-core grid, freq/governor, thermal"),
        help_entry(&app.theme, "3 Memory", "PSI/page faults/leak suspects"),
        help_entry(
            &app.theme,
            "4 Processes",
            "sort/filter/tree/actions/details",
        ),
        help_entry(&app.theme, "5 Network", "ifaces/connections/tools/filter"),
        help_entry(&app.theme, "6 Disk", "I/O, inode+alerts, async explorer"),
        help_entry(&app.theme, "7 GPU", "telemetry + history + process table"),
        help_entry(
            &app.theme,
            "8 Services",
            "state filter, actions, logs, diagnostics",
        ),
        help_entry(
            &app.theme,
            "9 Logs",
            "source/level/regex/autoscroll/navigate",
        ),
        help_entry(
            &app.theme,
            "0 Hardware",
            "sensors, users/history, security snapshot",
        ),
        help_entry(&app.theme, "? Help", "this reference"),
    ];
    frame.render_widget(
        Paragraph::new(tab_lines)
            .block(widgets::block(&app.theme, "Tabs"))
            .wrap(Wrap { trim: false }),
        sections[0],
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(sections[1]);

    let deep_lines = vec![
        section_title(&app.theme, "Tab Deep Sections"),
        Line::from(""),
        key_line(
            &app.theme,
            "Overview",
            "cards + network chart + offenders panel",
        ),
        key_line(
            &app.theme,
            "CPU",
            "core grid + temp history + throttling state",
        ),
        key_line(&app.theme, "Memory", "RAM/swap trends + PSI + fault rates"),
        key_line(
            &app.theme,
            "Processes",
            "/ filter • s sort • p view • k/K kill • r renice • a pin",
        ),
        key_line(
            &app.theme,
            "Network",
            "c conn-state • k kill flow • b block IP • t DNS/ping/trace/http",
        ),
        key_line(
            &app.theme,
            "Disk",
            "f async scan • m depth • inode% + SMART + large files",
        ),
        key_line(
            &app.theme,
            "GPU",
            "util/VRAM/temp/power/fan history + GPU process VRAM table",
        ),
        key_line(
            &app.theme,
            "Services",
            "s state filter • u/i/o restart flow • e/d enable/disable • w/W mask",
        ),
        key_line(
            &app.theme,
            "Logs",
            "/ regex • v level • o source • a autoscroll • n/N match nav",
        ),
        key_line(
            &app.theme,
            "Hardware",
            "sensors/power/GPU + SSH/failed logins/firewall/SELinux/AppArmor",
        ),
    ];
    frame.render_widget(
        Paragraph::new(deep_lines)
            .block(widgets::block(&app.theme, "Per-Tab Reference"))
            .wrap(Wrap { trim: false }),
        body[0],
    );

    let config_lines = vec![
        section_title(&app.theme, "Config / Behavior"),
        Line::from(""),
        key_line(
            &app.theme,
            "Refresh",
            "Configurable; default 1s snapshot cycle",
        ),
        key_line(&app.theme, "History", "60 samples for trends/sparklines"),
        key_line(&app.theme, "Process rows", "top 200 processes"),
        key_line(&app.theme, "Service backend", "Linux + systemd required"),
        key_line(
            &app.theme,
            "Network tools",
            "DNS/ping/trace/http use host binaries",
        ),
        key_line(
            &app.theme,
            "Disk explorer",
            "background worker; status shown in sidebar/footer",
        ),
        key_line(
            &app.theme,
            "Security snapshot",
            "best-effort probes (ufw/firewalld/iptables, SELinux/AppArmor)",
        ),
        key_line(
            &app.theme,
            "Config file",
            "~/.config/sysman/config.toml (thresholds, colors)",
        ),
        Line::from(""),
        section_title(&app.theme, "Global Keys"),
        key_line(&app.theme, "Tab / h / l", "cycle / previous / next tab"),
        key_line(&app.theme, "j / k", "scroll down / up (vim-style)"),
        key_line(&app.theme, "gg / G", "top / bottom"),
        key_line(&app.theme, "R / q", "force refresh / quit"),
    ];
    frame.render_widget(
        Paragraph::new(config_lines)
            .block(widgets::block(&app.theme, "Config Reference"))
            .wrap(Wrap { trim: false }),
        body[1],
    );

    let footer_lines = vec![
        Line::from(vec![
            Span::styled(
                "Tip: ",
                Style::default()
                    .fg(app.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("each tab supports "),
            Span::styled("j/k gg/G", Style::default().fg(app.theme.status_info)),
            Span::raw(" navigation."),
        ]),
        Line::from(vec![
            Span::raw("Open "),
            Span::styled("footer hints", Style::default().fg(app.theme.status_info)),
            Span::raw(" in each tab for context-specific actions."),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(footer_lines)
            .block(widgets::block(&app.theme, "Quick Notes"))
            .wrap(Wrap { trim: false }),
        sections[2],
    );
}

fn help_entry<'a>(theme: &crate::theme::Theme, key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{key:<14}"), Style::default().fg(theme.status_info)),
        Span::raw(desc),
    ])
}

fn key_line<'a>(theme: &crate::theme::Theme, key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{key:<12}"),
            Style::default()
                .fg(theme.status_warn)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(desc),
    ])
}

fn section_title(theme: &crate::theme::Theme, text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default()
            .fg(theme.brand)
            .add_modifier(Modifier::BOLD),
    ))
}
