use crate::app::{App, Tab};
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App) {
    let footer = if app.logs_regex_input {
        Line::from(vec![
            Span::styled("LOG REGEX: ", Style::default().fg(app.theme.status_warn)),
            Span::raw(app.logs_query.clone()),
            Span::styled(
                "  |  Enter to apply  Esc to clear",
                Style::default().fg(app.theme.text_muted),
            ),
        ])
    } else if app.network_tool_input {
        Line::from(vec![
            Span::styled("NET TARGET: ", Style::default().fg(app.theme.status_warn)),
            Span::raw(app.network_tool_value.clone()),
            Span::styled(
                "  |  Enter: DNS+Ping+Trace+HTTP  Esc: cancel",
                Style::default().fg(app.theme.text_muted),
            ),
        ])
    } else if app.pin_input {
        Line::from(vec![
            Span::styled("PIN CORE: ", Style::default().fg(app.theme.status_warn)),
            Span::raw(app.pin_core_value.clone()),
            Span::styled(
                "  |  Enter to apply  Esc to cancel",
                Style::default().fg(app.theme.text_muted),
            ),
        ])
    } else if app.renice_input {
        Line::from(vec![
            Span::styled("RENICE: ", Style::default().fg(app.theme.status_warn)),
            Span::raw(app.renice_value.clone()),
            Span::styled(
                "  |  Enter to apply  Esc to cancel  (-20..19)",
                Style::default().fg(app.theme.text_muted),
            ),
        ])
    } else if app.filter_input {
        Line::from(vec![
            Span::styled("FILTER: ", Style::default().fg(app.theme.status_warn)),
            Span::raw(app.process_filter.clone()),
            Span::styled(
                "  |  Enter to apply  Esc to cancel",
                Style::default().fg(app.theme.text_muted),
            ),
        ])
    } else {
        let refresh_indicator = if app.is_loading {
            format!("{} ", widgets::spinner_char(app.animation_frame))
        } else {
            String::new()
        };

        let tab_hints = match app.active_tab {
            Tab::Overview => "",
            Tab::Cpu => "",
            Tab::Memory => "",
            Tab::Processes => " j/k gg/G / s p x z n a",
            Tab::Network => " j/k gg/G c filter x kill b block t tools",
            Tab::Disk => " j/k gg/G f async-scan m depth",
            Tab::Gpu => "",
            Tab::Services => " j/k gg/G s filter u/i/o e/d w/W",
            Tab::Logs => " j/k gg/G / regex v level o source a auto n/N match",
            Tab::Hardware => "",
            Tab::Help => "",
        };

        Line::from(vec![
            Span::styled(
                format!("{}{}  ", refresh_indicator, app.status_line),
                Style::default().fg(app.theme.text_primary),
            ),
            Span::styled("◆", Style::default().fg(app.theme.divider)),
            Span::styled(
                format!(" {:<3} ", app.animation_frame % 60),
                Style::default().fg(app.theme.text_dim),
            ),
            Span::styled("◆", Style::default().fg(app.theme.divider)),
            Span::raw(" "),
            Span::styled(
                "1-9,0",
                Style::default()
                    .fg(app.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" tab ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                "h/l",
                Style::default()
                    .fg(app.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" nav ", Style::default().fg(app.theme.text_muted)),
            Span::styled(tab_hints, Style::default().fg(app.theme.text_muted)),
            Span::styled(
                "r",
                Style::default()
                    .fg(app.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ref", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                " ?",
                Style::default()
                    .fg(app.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" help ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                "q",
                Style::default()
                    .fg(app.theme.status_error)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" quit", Style::default().fg(app.theme.text_muted)),
        ])
    };

    frame.render_widget(
        Paragraph::new(footer)
            .style(Style::default().fg(app.theme.text_secondary))
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(app.theme.border)),
            ),
        area,
    );
}
