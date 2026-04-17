use crate::app::App;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
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
        Line::from(vec![
            Span::styled(
                app.status_line.clone(),
                Style::default().fg(app.theme.text_primary),
            ),
            Span::styled(
                "  |  1-7 tabs  h/l nav  j/k scroll  / filter  s sort  r refresh  q quit",
                Style::default().fg(app.theme.text_secondary),
            ),
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
