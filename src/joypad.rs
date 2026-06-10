use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum JoypadButton {
    Right = 0,
    Left = 1,
    Up = 2,
    Down = 3,
    A = 4,
    B = 5,
    Select = 6,
    Start = 7,
}

#[derive(Serialize, Deserialize)]
pub struct Joypad {
    selection: u8,
    pressed: u8,
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            selection: 0x30,
            pressed: 0,
        }
    }

    pub fn read(&self) -> u8 {
        0xc0 | self.selection | self.output_lines()
    }

    pub fn write(&mut self, value: u8) -> bool {
        let old_lines = self.output_lines();
        self.selection = value & 0x30;
        falling_edge(old_lines, self.output_lines())
    }

    pub fn set_button(&mut self, button: JoypadButton, pressed: bool) -> bool {
        let old_lines = self.output_lines();
        let mask = 1 << button as u8;
        if pressed {
            self.pressed |= mask;
        } else {
            self.pressed &= !mask;
        }
        falling_edge(old_lines, self.output_lines())
    }

    fn output_lines(&self) -> u8 {
        let mut lines = 0x0f;
        if self.selection & 0x10 == 0 {
            lines &= !(self.pressed & 0x0f);
        }
        if self.selection & 0x20 == 0 {
            lines &= !((self.pressed >> 4) & 0x0f);
        }
        lines
    }
}

impl Default for Joypad {
    fn default() -> Self {
        Self::new()
    }
}

fn falling_edge(old_lines: u8, new_lines: u8) -> bool {
    old_lines & !new_lines & 0x0f != 0
}

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
