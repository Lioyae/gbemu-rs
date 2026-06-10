use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{App, ExitConfirmation};

pub fn render(frame: &mut Frame, app: &App) {
    if app.exit_confirmation() != ExitConfirmation::Pending {
        return;
    }

    let area = centered(62, 7, frame.area());
    frame.render_widget(Clear, area);
    let text = vec![
        Line::from("卡带存档尚未保存，确定退出吗？"),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "S 保存并退出",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled("D 放弃并退出", Style::default().fg(Color::Red)),
            Span::raw("    Esc 取消"),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(" 退出确认 ")),
        area,
    );
}

fn centered(width: u16, height: u16, area: Rect) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
