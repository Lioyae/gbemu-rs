#[cfg(test)]
mod tests {
    use crate::{cartridge::Cartridge, cpu::Memory};

    use super::*;

    fn test_bus() -> Bus {
        let mut rom = vec![0; 32 * 1024];
        rom[0x134..0x138].copy_from_slice(b"TEST");
        rom[0x147] = 0x00;
        rom[0x148] = 0x00;
        rom[0x149] = 0x00;
        rom[0x0150] = 0x42;
        Bus::new(Cartridge::from_bytes(rom).expect("测试卡带应有效"))
    }

    #[test]
    fn routes_cartridge_and_video_memory() {
        let mut bus = test_bus();

        assert_eq!(bus.read8(0x0150), 0x42);
        bus.write8(0x0150, 0xaa);
        assert_eq!(bus.read8(0x0150), 0x42);

        bus.write8(0x8000, 0x11);
        bus.write8(0x9fff, 0x22);
        assert_eq!(bus.read8(0x8000), 0x11);
        assert_eq!(bus.read8(0x9fff), 0x22);
    }

    #[test]
    fn mirrors_work_ram_into_echo_region() {
        let mut bus = test_bus();

        bus.write8(0xc123, 0x5a);
        assert_eq!(bus.read8(0xe123), 0x5a);

        bus.write8(0xfdff, 0xa5);
        assert_eq!(bus.read8(0xddff), 0xa5);
    }

    #[test]
    fn maps_oam_and_rejects_unusable_region() {
        let mut bus = test_bus();

        bus.write8(0xfe00, 0x33);
        bus.write8(0xfe9f, 0x44);
        bus.write8(0xfea0, 0x55);

        assert_eq!(bus.read8(0xfe00), 0x33);
        assert_eq!(bus.read8(0xfe9f), 0x44);
        assert_eq!(bus.read8(0xfea0), 0xff);
        assert_eq!(bus.read8(0xfeff), 0xff);
    }

    #[test]
    fn maps_io_high_ram_and_interrupt_registers() {
        let mut bus = test_bus();

        bus.write8(0xff01, 0x77);
        bus.write8(0xff80, 0x88);
        bus.write8(0xfffe, 0x99);
        bus.write8(0xffff, 0x1f);

        assert_eq!(bus.read8(0xff01), 0x77);
        assert_eq!(bus.read8(0xff80), 0x88);
        assert_eq!(bus.read8(0xfffe), 0x99);
        assert_eq!(bus.read8(0xffff), 0x1f);
    }

    #[test]
    fn requests_interrupt_without_clearing_existing_bits() {
        let mut bus = test_bus();
        bus.write8(0xff0f, 0xe1);

        bus.request_interrupt(Interrupt::Timer);
        bus.request_interrupt(Interrupt::Joypad);

        assert_eq!(bus.read8(0xff0f), 0xf5);
    }

    #[test]
    fn word_access_wraps_at_end_of_address_space() {
        let mut bus = test_bus();

        bus.write16(0xffff, 0x1234);

        assert_eq!(bus.read8(0xffff), 0x34);
        assert_eq!(bus.read8(0x0000), 0x00);
        assert_eq!(bus.read16(0xffff), 0x0034);
    }
}
