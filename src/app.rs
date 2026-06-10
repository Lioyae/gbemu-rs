use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use thiserror::Error;

use crate::{
    debugger::{DebugSnapshot, Debugger},
    emulator::{Emulator, EmulatorError},
    joypad::JoypadButton,
    save::{BatterySave, SaveError},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Game,
    Debugger,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExitConfirmation {
    #[default]
    None,
    Pending,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Emulator(#[from] EmulatorError),
    #[error(transparent)]
    Save(#[from] SaveError),
}

pub struct App {
    emulator: Emulator,
    rom_path: Option<PathBuf>,
    mode: ViewMode,
    should_quit: bool,
    exit_confirmation: ExitConfirmation,
    status_message: String,
    memory_start: u16,
    fps: f64,
    frames_since_measurement: u32,
    measurement_started: Instant,
}

impl App {
    pub fn new(emulator: Emulator) -> Self {
        Self {
            emulator,
            rom_path: None,
            mode: ViewMode::Game,
            should_quit: false,
            exit_confirmation: ExitConfirmation::None,
            status_message: "未执行存档操作".to_owned(),
            memory_start: 0xc000,
            fps: 0.0,
            frames_since_measurement: 0,
            measurement_started: Instant::now(),
        }
    }

    pub fn with_rom_path(emulator: Emulator, rom_path: PathBuf) -> Self {
        let mut app = Self::new(emulator);
        app.rom_path = Some(rom_path);
        app
    }

    pub fn emulator(&self) -> &Emulator {
        &self.emulator
    }

    pub fn emulator_mut(&mut self) -> &mut Emulator {
        &mut self.emulator
    }

    pub fn mode(&self) -> ViewMode {
        self.mode
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn exit_confirmation(&self) -> ExitConfirmation {
        self.exit_confirmation
    }

    pub fn status_message(&self) -> &str {
        &self.status_message
    }

    pub fn memory_start(&self) -> u16 {
        self.memory_start
    }

    pub fn fps(&self) -> f64 {
        self.fps
    }

    pub fn update(&mut self) -> Result<(), AppError> {
        let result = self.emulator.run_until_frame(80_000)?;
        if result.frame_ready {
            self.frames_since_measurement += 1;
            let elapsed = self.measurement_started.elapsed();
            if elapsed >= Duration::from_secs(1) {
                self.fps = f64::from(self.frames_since_measurement) / elapsed.as_secs_f64();
                self.frames_since_measurement = 0;
                self.measurement_started = Instant::now();
            }
        }
        Ok(())
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> Result<(), AppError> {
        if self.exit_confirmation == ExitConfirmation::Pending {
            if event.kind == KeyEventKind::Release {
                return Ok(());
            }
            return self.handle_exit_confirmation(event.code);
        }

        if event.kind != KeyEventKind::Release
            && event.modifiers.contains(KeyModifiers::CONTROL)
        {
            match event.code {
                KeyCode::Char('s' | 'S') => {
                    self.save_battery()?;
                    return Ok(());
                }
                KeyCode::Char('l' | 'L') => {
                    self.load_battery()?;
                    return Ok(());
                }
                _ => {}
            }
        }

        if let Some(button) = map_game_key(event.code) {
            match event.kind {
                KeyEventKind::Press | KeyEventKind::Repeat => {
                    self.emulator.set_button(button, true)
                }
                KeyEventKind::Release => self.emulator.set_button(button, false),
            }
            return Ok(());
        }

        if event.kind == KeyEventKind::Release {
            return Ok(());
        }
        match event.code {
            KeyCode::Char('q' | 'Q') | KeyCode::Esc => self.request_quit(),
            KeyCode::Tab => {
                self.mode = match self.mode {
                    ViewMode::Game => ViewMode::Debugger,
                    ViewMode::Debugger => ViewMode::Game,
                }
            }
            KeyCode::Char(' ') => {
                let paused = !self.emulator.paused();
                self.emulator.set_paused(paused);
            }
            KeyCode::Char('n' | 'N') if self.emulator.paused() => {
                self.emulator.step_instruction()?;
            }
            KeyCode::PageUp => self.memory_start = self.memory_start.wrapping_sub(0x0100),
            KeyCode::PageDown => self.memory_start = self.memory_start.wrapping_add(0x0100),
            _ => {}
        }
        Ok(())
    }

    fn request_quit(&mut self) {
        if self.emulator.cartridge_persistent_dirty() && self.rom_path.is_some() {
            self.exit_confirmation = ExitConfirmation::Pending;
        } else {
            self.should_quit = true;
        }
    }

    fn handle_exit_confirmation(&mut self, code: KeyCode) -> Result<(), AppError> {
        match code {
            KeyCode::Char('s' | 'S') => {
                self.save_battery()?;
                self.should_quit = true;
                self.exit_confirmation = ExitConfirmation::None;
            }
            KeyCode::Char('d' | 'D') => {
                self.should_quit = true;
                self.exit_confirmation = ExitConfirmation::None;
            }
            KeyCode::Esc | KeyCode::Char('c' | 'C') => {
                self.exit_confirmation = ExitConfirmation::None;
                self.status_message = "已取消退出".to_owned();
            }
            _ => {}
        }
        Ok(())
    }

    fn rom_path(&self) -> Result<&Path, SaveError> {
        self.rom_path.as_deref().ok_or(SaveError::MissingRomPath)
    }

    fn save_battery(&mut self) -> Result<(), AppError> {
        let rom_path = self.rom_path()?.to_owned();
        BatterySave::save(&mut self.emulator, &rom_path)?;
        self.status_message = format!("存档已保存：{}", rom_path.with_extension("sav").display());
        Ok(())
    }

    fn load_battery(&mut self) -> Result<(), AppError> {
        let rom_path = self.rom_path()?.to_owned();
        BatterySave::load(&mut self.emulator, &rom_path)?;
        self.status_message = format!("存档已加载：{}", rom_path.with_extension("sav").display());
        Ok(())
    }

    pub fn debug_snapshot(&self, memory_length: usize) -> DebugSnapshot {
        Debugger::snapshot(&self.emulator, self.memory_start, memory_length, 12)
    }
}

fn map_game_key(code: KeyCode) -> Option<JoypadButton> {
    match code {
        KeyCode::Right => Some(JoypadButton::Right),
        KeyCode::Left => Some(JoypadButton::Left),
        KeyCode::Up => Some(JoypadButton::Up),
        KeyCode::Down => Some(JoypadButton::Down),
        KeyCode::Char('x' | 'X') => Some(JoypadButton::A),
        KeyCode::Char('z' | 'Z') => Some(JoypadButton::B),
        KeyCode::Backspace => Some(JoypadButton::Select),
        KeyCode::Enter => Some(JoypadButton::Start),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    use super::*;

    fn test_app() -> App {
        let mut rom = vec![0; 32 * 1024];
        rom[0x100] = 0x00;
        rom[0x134..0x138].copy_from_slice(b"TEST");
        rom[0x147] = 0x00;
        rom[0x148] = 0x00;
        rom[0x149] = 0x00;
        App::new(Emulator::from_rom(rom).expect("测试 ROM 应加载成功"))
    }

    fn key(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind)
    }

    fn control_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::CONTROL, KeyEventKind::Press)
    }

    #[test]
    fn maps_game_keys_on_press_and_release() {
        let mut app = test_app();
        app.emulator_mut().write_memory(0xff00, 0x10);
        app.emulator_mut().write_memory(0xff0f, 0);

        app.handle_key(key(KeyCode::Char('x'), KeyEventKind::Press))
            .expect("按键应处理成功");
        assert_eq!(app.emulator().read_memory(0xff00) & 0x01, 0);
        assert_ne!(app.emulator().read_memory(0xff0f) & 0x10, 0);

        app.handle_key(key(KeyCode::Char('x'), KeyEventKind::Release))
            .expect("按键释放应处理成功");
        assert_ne!(app.emulator().read_memory(0xff00) & 0x01, 0);
    }

    #[test]
    fn switches_mode_and_toggles_pause() {
        let mut app = test_app();

        app.handle_key(key(KeyCode::Tab, KeyEventKind::Press))
            .expect("模式切换应成功");
        assert_eq!(app.mode(), ViewMode::Debugger);

        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Press))
            .expect("暂停切换应成功");
        assert!(app.emulator().paused());
    }

    #[test]
    fn steps_only_while_paused() {
        let mut app = test_app();

        app.handle_key(key(KeyCode::Char('n'), KeyEventKind::Press))
            .expect("运行时 N 应被忽略");
        assert_eq!(app.emulator().cpu().registers().pc, 0x0100);

        app.emulator_mut().set_paused(true);
        app.handle_key(key(KeyCode::Char('n'), KeyEventKind::Press))
            .expect("暂停时单步应成功");
        assert_eq!(app.emulator().cpu().registers().pc, 0x0101);
    }

    #[test]
    fn scrolls_debug_memory_by_one_page() {
        let mut app = test_app();
        assert_eq!(app.memory_start(), 0xc000);

        app.handle_key(key(KeyCode::PageDown, KeyEventKind::Press))
            .expect("向下翻页应成功");
        assert_eq!(app.memory_start(), 0xc100);

        app.handle_key(key(KeyCode::PageUp, KeyEventKind::Press))
            .expect("向上翻页应成功");
        assert_eq!(app.memory_start(), 0xc000);
    }

    #[test]
    fn q_and_escape_request_exit() {
        for code in [KeyCode::Char('q'), KeyCode::Esc] {
            let mut app = test_app();
            app.handle_key(key(code, KeyEventKind::Press))
                .expect("退出键应处理成功");
            assert!(app.should_quit());
        }
    }

    fn battery_app(rom_path: PathBuf) -> App {
        let mut rom = vec![0; 32 * 1024];
        rom[0x147] = 0x03;
        rom[0x148] = 0x00;
        rom[0x149] = 0x02;
        App::with_rom_path(
            Emulator::from_rom(rom).expect("电池 ROM 应加载成功"),
            rom_path,
        )
    }

    #[test]
    fn control_s_and_control_l_save_and_restore_battery_ram() {
        let directory = std::env::temp_dir().join(format!(
            "gbmeu-app-save-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("临时目录应创建成功");
        let rom_path = directory.join("game.gb");
        let mut app = battery_app(rom_path);
        app.emulator_mut().write_memory(0x0000, 0x0a);
        app.emulator_mut().write_memory(0xa000, 0x5a);
        app.handle_key(control_key(KeyCode::Char('s')))
            .expect("Ctrl+S 应保存成功");

        app.emulator_mut().write_memory(0xa000, 0x11);
        app.handle_key(control_key(KeyCode::Char('l')))
            .expect("Ctrl+L 应加载成功");
        assert_eq!(app.emulator().read_memory(0xa000), 0x5a);

        std::fs::remove_dir_all(directory).expect("临时目录应清理成功");
    }

    #[test]
    fn dirty_exit_supports_cancel_and_discard() {
        let mut app = battery_app(PathBuf::from("game.gb"));
        app.emulator_mut().write_memory(0xa000, 0x5a);
        app.handle_key(key(KeyCode::Char('q'), KeyEventKind::Press))
            .expect("退出请求应成功");
        assert_eq!(app.exit_confirmation(), ExitConfirmation::Pending);
        assert!(!app.should_quit());

        app.handle_key(key(KeyCode::Esc, KeyEventKind::Press))
            .expect("取消退出应成功");
        assert_eq!(app.exit_confirmation(), ExitConfirmation::None);

        app.handle_key(key(KeyCode::Char('q'), KeyEventKind::Press))
            .expect("退出请求应成功");
        app.handle_key(key(KeyCode::Char('d'), KeyEventKind::Press))
            .expect("放弃存档应成功");
        assert!(app.should_quit());
    }
}
