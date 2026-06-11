pub mod debug_view;
pub mod dialog;
pub mod game_view;
mod launcher;

pub use launcher::select_rom;

use std::{
    io::{self, Stdout},
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, SetSize, disable_raw_mode, enable_raw_mode,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};
use thiserror::Error;

use crate::{
    app::{App, ViewMode},
    emulator::Emulator,
};

const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706);

#[derive(Debug, Error)]
pub enum TuiError {
    #[error(
        "终端尺寸不足：当前 {actual_width}×{actual_height}，{mode}模式至少需要 {required_width}×{required_height}"
    )]
    TerminalTooSmall {
        actual_width: u16,
        actual_height: u16,
        required_width: u16,
        required_height: u16,
        mode: &'static str,
    },
}

pub fn run(emulator: Emulator, rom_path: PathBuf) -> Result<()> {
    prepare_terminal_for_mode(ViewMode::Game);
    let _guard = TerminalGuard::enter().context("无法初始化终端")?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).context("无法创建终端绘制后端")?;
    terminal.clear().context("无法清空终端")?;
    let mut app = App::with_rom_path(emulator, rom_path);

    while !app.should_quit() {
        let frame_started = Instant::now();
        if !app.emulator().paused() {
            app.update().context("模拟器运行失败")?;
        }

        let size = terminal.size().context("无法读取终端尺寸")?;
        validate_size(size.width, size.height, app.mode())?;
        terminal
            .draw(|frame| {
                match app.mode() {
                    ViewMode::Game => game_view::render(frame, &app),
                    ViewMode::Debugger => debug_view::render(frame, &app),
                }
                dialog::render(frame, &app);
            })
            .context("终端绘制失败")?;

        let wait = FRAME_DURATION.saturating_sub(frame_started.elapsed());
        if event::poll(wait).context("终端事件轮询失败")?
            && let Event::Key(key) = event::read().context("无法读取终端事件")?
        {
            app.handle_key(key).context("无法处理终端按键")?;
        }
    }
    Ok(())
}

pub fn validate_size(width: u16, height: u16, mode: ViewMode) -> Result<(), TuiError> {
    let (required_width, required_height, mode_name) = required_size(mode);
    if width < required_width || height < required_height {
        return Err(TuiError::TerminalTooSmall {
            actual_width: width,
            actual_height: height,
            required_width,
            required_height,
            mode: mode_name,
        });
    }
    Ok(())
}

fn required_size(mode: ViewMode) -> (u16, u16, &'static str) {
    match mode {
        ViewMode::Game => (162, 76, "游戏"),
        ViewMode::Debugger => (120, 36, "调试"),
    }
}

fn resize_target(width: u16, height: u16, mode: ViewMode) -> Option<(u16, u16)> {
    let (required_width, required_height, _) = required_size(mode);
    if width >= required_width && height >= required_height {
        return None;
    }

    Some((width.max(required_width), height.max(required_height)))
}

fn prepare_terminal_for_mode(mode: ViewMode) {
    let Ok((width, height)) = crossterm::terminal::size() else {
        return;
    };
    let Some((target_width, target_height)) = resize_target(width, height, mode) else {
        return;
    };

    let _ = execute!(io::stdout(), SetSize(target_width, target_height));
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    }
}

pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_terminal_size_for_each_mode() {
        assert!(validate_size(162, 76, crate::app::ViewMode::Game).is_ok());
        assert!(validate_size(161, 76, crate::app::ViewMode::Game).is_err());
        assert!(validate_size(120, 36, crate::app::ViewMode::Debugger).is_ok());
        assert!(validate_size(119, 36, crate::app::ViewMode::Debugger).is_err());
    }

    #[test]
    fn requests_only_missing_terminal_dimensions() {
        assert_eq!(
            resize_target(120, 30, crate::app::ViewMode::Game),
            Some((162, 76))
        );
        assert_eq!(
            resize_target(200, 30, crate::app::ViewMode::Game),
            Some((200, 76))
        );
        assert_eq!(resize_target(200, 80, crate::app::ViewMode::Game), None);
    }
}
