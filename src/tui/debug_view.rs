use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{app::App, cpu::registers::Flag};

pub fn render(frame: &mut Frame, app: &App) {
    let snapshot = app.debug_snapshot(128);
    let area = frame.area();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(33),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(36), Constraint::Min(80)])
        .split(vertical[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(17), Constraint::Min(16)])
        .split(columns[1]);

    let registers = &snapshot.registers;
    let flags = format!(
        "{}{}{}{}",
        flag(registers.flag(Flag::Zero), 'Z'),
        flag(registers.flag(Flag::Subtract), 'N'),
        flag(registers.flag(Flag::HalfCarry), 'H'),
        flag(registers.flag(Flag::Carry), 'C')
    );
    let cpu_text = format!(
        "AF  {:04X}    BC  {:04X}\nDE  {:04X}    HL  {:04X}\nSP  {:04X}    PC  {:04X}\n\n标志  {flags}\nIME   {}    HALT  {}\nIE    {:02X}    IF    {:02X}\nLCD 模式 {}    LY {:3}",
        registers.af(),
        registers.bc(),
        registers.de(),
        registers.hl(),
        registers.sp,
        registers.pc,
        on_off(snapshot.ime),
        on_off(snapshot.halted),
        snapshot.interrupt_enable,
        snapshot.interrupt_flags,
        snapshot.lcd_mode,
        snapshot.ly,
    );
    frame.render_widget(
        Paragraph::new(cpu_text).block(Block::default().borders(Borders::ALL).title(" CPU 状态 ")),
        columns[0],
    );

    let disassembly: Vec<Line<'static>> = snapshot
        .disassembly
        .iter()
        .map(|instruction| {
            let current = instruction.address == registers.pc;
            let marker = if current { ">" } else { " " };
            let bytes = instruction
                .bytes
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            Line::from(vec![
                Span::styled(
                    format!("{marker} {:04X}  {bytes:<8} ", instruction.address),
                    if current {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                ),
                Span::raw(instruction.text.clone()),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(disassembly)
            .block(Block::default().borders(Borders::ALL).title(" 反汇编 ")),
        right[0],
    );

    let memory_lines: Vec<Line<'static>> = snapshot
        .memory
        .chunks(16)
        .map(|chunk| {
            let address = chunk.first().map(|(address, _)| *address).unwrap_or(0);
            let bytes = chunk
                .iter()
                .map(|(_, byte)| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            Line::raw(format!("{address:04X}: {bytes}"))
        })
        .collect();
    frame.render_widget(
        Paragraph::new(memory_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" 内存 {:04X} ", app.memory_start())),
        ),
        right[1],
    );

    let state = if app.emulator().paused() {
        "已暂停"
    } else {
        "运行中"
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{state}  ROM: {}  FPS: {:.1}",
            app.emulator().cartridge_header().title(),
            app.fps()
        )),
        vertical[1],
    );
    frame.render_widget(
        Paragraph::new(
            "Tab:游戏  Space:暂停/继续  N:单步  PageUp/PageDown:内存翻页  Q/Esc:退出",
        ),
        vertical[2],
    );
}

fn flag(enabled: bool, name: char) -> char {
    if enabled { name } else { '-' }
}

fn on_off(enabled: bool) -> &'static str {
    if enabled { "开" } else { "关" }
}
