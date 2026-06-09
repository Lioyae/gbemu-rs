#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_high_lines_when_no_group_is_selected() {
        let joypad = Joypad::new();

        assert_eq!(joypad.read(), 0xff);
    }

    #[test]
    fn selects_direction_and_action_groups() {
        let mut joypad = Joypad::new();
        joypad.set_button(JoypadButton::Right, true);
        joypad.set_button(JoypadButton::A, true);

        joypad.write(0x20);
        assert_eq!(joypad.read() & 0x0f, 0x0e);

        joypad.write(0x10);
        assert_eq!(joypad.read() & 0x0f, 0x0e);
    }

    #[test]
    fn maps_all_buttons_to_low_active_lines() {
        let cases = [
            (JoypadButton::Right, 0x20, 0),
            (JoypadButton::Left, 0x20, 1),
            (JoypadButton::Up, 0x20, 2),
            (JoypadButton::Down, 0x20, 3),
            (JoypadButton::A, 0x10, 0),
            (JoypadButton::B, 0x10, 1),
            (JoypadButton::Select, 0x10, 2),
            (JoypadButton::Start, 0x10, 3),
        ];

        for (button, selection, bit) in cases {
            let mut joypad = Joypad::new();
            joypad.write(selection);
            assert!(joypad.set_button(button, true));
            assert_eq!(joypad.read() & (1 << bit), 0);
            assert!(!joypad.set_button(button, true));
            assert!(!joypad.set_button(button, false));
        }
    }

    #[test]
    fn selecting_a_pressed_group_requests_interrupt() {
        let mut joypad = Joypad::new();
        assert!(!joypad.set_button(JoypadButton::A, true));

        assert!(joypad.write(0x10));
    }
}
