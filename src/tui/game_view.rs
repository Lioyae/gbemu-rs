use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::{
    app::App,
    ppu::framebuffer::{Framebuffer, Shade},
};

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(74),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);
    let screen_area = centered(162, 74, vertical[0]);
    let title = format!(
        " {} [{}] ",
        app.emulator().cartridge_header().title(),
        app.emulator().cartridge_header().cartridge_type()
    );
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(screen_area);
    frame.render_widget(block, screen_area);
    frame.render_widget(GameScreen(app.emulator().framebuffer()), inner);

    let state = if app.emulator().paused() {
        "已暂停"
    } else {
        "运行中"
    };
    frame.render_widget(
        Paragraph::new(format!("{state}  FPS: {:.1}", app.fps())).alignment(Alignment::Center),
        vertical[1],
    );
    frame.render_widget(
        Paragraph::new(
            "方向键: 十字键  Z:B  X:A  Enter:Start  Backspace:Select  Tab:调试  Space:暂停  Q:退出",
        )
        .alignment(Alignment::Center),
        vertical[2],
    );
}

struct GameScreen<'a>(&'a Framebuffer);

impl Widget for GameScreen<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        for y in 0..72usize {
            for x in 0..160usize {
                let cell = &mut buffer[(area.x + x as u16, area.y + y as u16)];
                cell.set_symbol("▀")
                    .set_fg(shade_color(self.0.pixel(x, y * 2)))
                    .set_bg(shade_color(self.0.pixel(x, y * 2 + 1)));
            }
        }
    }
}

fn shade_color(shade: Shade) -> Color {
    match shade {
        Shade::White => Color::Rgb(224, 248, 208),
        Shade::LightGray => Color::Rgb(136, 192, 112),
        Shade::DarkGray => Color::Rgb(52, 104, 86),
        Shade::Black => Color::Rgb(8, 24, 32),
    }
}

fn centered(width: u16, height: u16, area: Rect) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}
