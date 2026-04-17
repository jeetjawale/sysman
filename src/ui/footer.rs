use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App) {
    let footer = if app.filter_input {
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

        Line::from(vec![
            Span::styled(
                format!("{}{}  ", refresh_indicator, app.status_line),
                Style::default().fg(app.theme.text_primary),
            ),
            Span::styled(
                "◆",
                Style::default().fg(app.theme.divider),
            ),
            Span::styled(
                format!(" {:<3} ", app.animation_frame % 60),
                Style::default().fg(app.theme.text_dim),
            ),
            Span::styled(
                "◆",
                Style::default().fg(app.theme.divider),
            ),
            Span::raw(" "),
            Span::styled(
                "1-7",
                Style::default().fg(app.theme.brand).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" tabs ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                "h/l",
                Style::default().fg(app.theme.brand).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" nav ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                "j/k",
                Style::default().fg(app.theme.brand).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" scroll ", Style::default().fg(app.theme.text_muted)),
            Span::styled(
                "q",
                Style::default().fg(app.theme.status_error).add_modifier(Modifier::BOLD),
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
