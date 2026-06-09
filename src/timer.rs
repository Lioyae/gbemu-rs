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
