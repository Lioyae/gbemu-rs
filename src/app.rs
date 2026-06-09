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
}
