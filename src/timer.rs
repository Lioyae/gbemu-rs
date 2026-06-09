pub struct Timer {
    divider: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    reload_delay: Option<u8>,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            divider: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            reload_delay: None,
        }
    }

    pub fn read(&self, address: u16) -> u8 {
        match address {
            0xff04 => (self.divider >> 8) as u8,
            0xff05 => self.tima,
            0xff06 => self.tma,
            0xff07 => self.tac | 0xf8,
            _ => 0xff,
        }
    }

    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0xff04 => {
                let old_signal = self.timer_signal();
                self.divider = 0;
                self.apply_falling_edge(old_signal);
            }
            0xff05 => {
                self.tima = value;
                self.reload_delay = None;
            }
            0xff06 => self.tma = value,
            0xff07 => {
                let old_signal = self.timer_signal();
                self.tac = value & 0x07;
                self.apply_falling_edge(old_signal);
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, cycles: u16) -> bool {
        let mut request_interrupt = false;
        for _ in 0..cycles {
            if let Some(delay) = self.reload_delay {
                if delay == 1 {
                    self.tima = self.tma;
                    self.reload_delay = None;
                    request_interrupt = true;
                } else {
                    self.reload_delay = Some(delay - 1);
                }
            }

            let old_signal = self.timer_signal();
            self.divider = self.divider.wrapping_add(1);
            self.apply_falling_edge(old_signal);
        }
        request_interrupt
    }

    fn timer_signal(&self) -> bool {
        if self.tac & 0x04 == 0 {
            return false;
        }
        let bit = match self.tac & 0x03 {
            0 => 9,
            1 => 3,
            2 => 5,
            3 => 7,
            _ => unreachable!("TAC 频率索引始终为 0..=3"),
        };
        self.divider & (1 << bit) != 0
    }

    fn apply_falling_edge(&mut self, old_signal: bool) {
        if old_signal && !self.timer_signal() && self.reload_delay.is_none() {
            let (value, overflow) = self.tima.overflowing_add(1);
            self.tima = value;
            if overflow {
                self.reload_delay = Some(4);
            }
        }
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divider_exposes_upper_counter_byte() {
        let mut timer = Timer::new();

        timer.tick(255);
        assert_eq!(timer.read(0xff04), 0x00);
        timer.tick(1);
        assert_eq!(timer.read(0xff04), 0x01);
    }

    #[test]
    fn selected_frequencies_increment_on_falling_edge() {
        let cases = [(0x04, 1024), (0x05, 16), (0x06, 64), (0x07, 256)];

        for (tac, period) in cases {
            let mut timer = Timer::new();
            timer.write(0xff07, tac);

            timer.tick(period - 1);
            assert_eq!(timer.read(0xff05), 0, "TAC 0x{tac:02x}");
            timer.tick(1);
            assert_eq!(timer.read(0xff05), 1, "TAC 0x{tac:02x}");
        }
    }

    #[test]
    fn resetting_divider_can_create_timer_edge() {
        let mut timer = Timer::new();
        timer.write(0xff07, 0x05);
        timer.tick(8);

        timer.write(0xff04, 0xff);

        assert_eq!(timer.read(0xff04), 0);
        assert_eq!(timer.read(0xff05), 1);
    }

    #[test]
    fn overflow_reloads_tma_after_four_cycles_and_requests_interrupt() {
        let mut timer = Timer::new();
        timer.write(0xff06, 0x42);
        timer.write(0xff05, 0xff);
        timer.write(0xff07, 0x05);

        assert!(!timer.tick(16));
        assert_eq!(timer.read(0xff05), 0x00);
        assert!(!timer.tick(3));
        assert_eq!(timer.read(0xff05), 0x00);
        assert!(timer.tick(1));
        assert_eq!(timer.read(0xff05), 0x42);
    }

    #[test]
    fn writing_tima_during_reload_delay_cancels_reload() {
        let mut timer = Timer::new();
        timer.write(0xff06, 0x42);
        timer.write(0xff05, 0xff);
        timer.write(0xff07, 0x05);
        timer.tick(16);

        timer.write(0xff05, 0x77);

        assert!(!timer.tick(4));
        assert_eq!(timer.read(0xff05), 0x77);
    }
}
