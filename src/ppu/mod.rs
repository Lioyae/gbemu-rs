pub mod framebuffer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_in_oam_scan_with_post_boot_registers() {
        let ppu = Ppu::post_boot();

        assert_eq!(ppu.read_register(0xff40), 0x91);
        assert_eq!(ppu.read_register(0xff41) & 0x03, LcdMode::OamScan as u8);
        assert_eq!(ppu.read_register(0xff44), 0);
        assert_eq!(ppu.read_register(0xff47), 0xfc);
    }

    #[test]
    fn advances_through_visible_scanline_modes() {
        let mut ppu = Ppu::post_boot();

        assert_eq!(ppu.tick(79), PpuEvents::default());
        assert_eq!(ppu.mode(), LcdMode::OamScan);
        assert_eq!(ppu.tick(1), PpuEvents::default());
        assert_eq!(ppu.mode(), LcdMode::Drawing);

        assert_eq!(ppu.tick(171), PpuEvents::default());
        assert_eq!(ppu.mode(), LcdMode::Drawing);
        assert_eq!(ppu.tick(1), PpuEvents::default());
        assert_eq!(ppu.mode(), LcdMode::HBlank);

        assert_eq!(ppu.tick(203), PpuEvents::default());
        assert_eq!(ppu.mode(), LcdMode::HBlank);
        assert_eq!(ppu.tick(1), PpuEvents::default());
        assert_eq!(ppu.mode(), LcdMode::OamScan);
        assert_eq!(ppu.read_register(0xff44), 1);
    }

    #[test]
    fn enters_vblank_on_line_144_and_wraps_after_line_153() {
        let mut ppu = Ppu::post_boot();

        let events = ppu.tick(456 * 144);
        assert!(events.vblank_interrupt);
        assert!(events.frame_ready);
        assert_eq!(ppu.mode(), LcdMode::VBlank);
        assert_eq!(ppu.read_register(0xff44), 144);

        ppu.tick(456 * 10);
        assert_eq!(ppu.mode(), LcdMode::OamScan);
        assert_eq!(ppu.read_register(0xff44), 0);
    }

    #[test]
    fn blocks_cpu_vram_and_oam_access_in_restricted_modes() {
        let mut ppu = Ppu::post_boot();

        ppu.write_vram(0x8000, 0x11);
        ppu.write_oam(0xfe00, 0x22);
        assert_eq!(ppu.read_vram(0x8000), 0x11);
        assert_eq!(ppu.read_oam(0xfe00), 0xff);

        ppu.tick(80);
        assert_eq!(ppu.mode(), LcdMode::Drawing);
        assert_eq!(ppu.read_vram(0x8000), 0xff);
        assert_eq!(ppu.read_oam(0xfe00), 0xff);
        ppu.write_vram(0x8000, 0x33);

        ppu.tick(172);
        assert_eq!(ppu.mode(), LcdMode::HBlank);
        assert_eq!(ppu.read_vram(0x8000), 0x11);
        assert_eq!(ppu.read_oam(0xfe00), 0x00);
    }

    #[test]
    fn disabling_lcd_resets_line_and_allows_memory_access() {
        let mut ppu = Ppu::post_boot();
        ppu.tick(500);

        ppu.write_register(0xff40, 0x11);

        assert_eq!(ppu.mode(), LcdMode::HBlank);
        assert_eq!(ppu.read_register(0xff44), 0);
        ppu.write_vram(0x8000, 0x44);
        ppu.write_oam(0xfe00, 0x55);
        assert_eq!(ppu.read_vram(0x8000), 0x44);
        assert_eq!(ppu.read_oam(0xfe00), 0x55);
        assert_eq!(ppu.tick(1000), PpuEvents::default());
        assert_eq!(ppu.read_register(0xff44), 0);
    }

    #[test]
    fn lyc_sets_coincidence_and_stat_interrupt_on_rising_edge() {
        let mut ppu = Ppu::post_boot();
        ppu.write_register(0xff45, 1);
        ppu.write_register(0xff41, 0x40);

        let events = ppu.tick(456);

        assert_ne!(ppu.read_register(0xff41) & 0x04, 0);
        assert!(events.stat_interrupt);
        assert!(!ppu.tick(1).stat_interrupt);
    }

    #[test]
    fn stat_mode_sources_request_only_on_signal_rising_edge() {
        let mut ppu = Ppu::post_boot();
        ppu.write_register(0xff41, 0x08);

        let events = ppu.tick(252);
        assert!(events.stat_interrupt);
        assert!(!ppu.tick(1).stat_interrupt);
    }
}
