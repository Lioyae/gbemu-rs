pub mod framebuffer;

use framebuffer::{Framebuffer, Shade};

const VRAM_BANK_SIZE: usize = 0x2000;
const VRAM_SIZE: usize = VRAM_BANK_SIZE * 2;
const OAM_SIZE: usize = 0x00a0;
const OAM_SCAN_END: u16 = 80;
const DRAWING_END: u16 = 252;
const SCANLINE_END: u16 = 456;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum LcdMode {
    #[default]
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Drawing = 3,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PpuEvents {
    pub vblank_interrupt: bool,
    pub stat_interrupt: bool,
    pub frame_ready: bool,
}

pub struct Ppu {
    vram: [u8; VRAM_SIZE],
    vram_bank: u8,
    oam: [u8; OAM_SIZE],
    framebuffer: Framebuffer,
    lcdc: u8,
    stat: u8,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    dma: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,
    dot: u16,
    mode: LcdMode,
    stat_line: bool,
    pending_stat_interrupt: bool,
}

impl Ppu {
    pub fn post_boot() -> Self {
        Self {
            vram: [0; VRAM_SIZE],
            vram_bank: 0,
            oam: [0; OAM_SIZE],
            framebuffer: Framebuffer::new(),
            lcdc: 0x91,
            stat: 0x00,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            dma: 0xff,
            bgp: 0xfc,
            obp0: 0xff,
            obp1: 0xff,
            wy: 0,
            wx: 0,
            dot: 0,
            mode: LcdMode::OamScan,
            stat_line: false,
            pending_stat_interrupt: false,
        }
    }

    pub fn mode(&self) -> LcdMode {
        self.mode
    }

    pub fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    pub fn read_vram(&self, address: u16) -> u8 {
        if self.lcd_enabled() && self.mode == LcdMode::Drawing {
            return 0xff;
        }
        self.read_vram_raw(address)
    }

    pub fn write_vram(&mut self, address: u16, value: u8) {
        if self.lcd_enabled() && self.mode == LcdMode::Drawing {
            return;
        }
        let index = self.vram_index(self.vram_bank, address);
        if let Some(byte) = self.vram.get_mut(index) {
            *byte = value;
        }
    }

    pub fn set_vram_bank(&mut self, bank: u8) {
        self.vram_bank = bank & 0x01;
    }

    pub fn vram_bank(&self) -> u8 {
        self.vram_bank
    }

    pub fn read_oam(&self, address: u16) -> u8 {
        if self.lcd_enabled() && matches!(self.mode, LcdMode::OamScan | LcdMode::Drawing) {
            return 0xff;
        }
        self.read_oam_raw(address)
    }

    pub fn write_oam(&mut self, address: u16, value: u8) {
        if self.lcd_enabled() && matches!(self.mode, LcdMode::OamScan | LcdMode::Drawing) {
            return;
        }
        self.write_oam_raw(address, value);
    }

    pub(crate) fn read_vram_raw(&self, address: u16) -> u8 {
        self.read_vram_bank(self.vram_bank, address)
    }

    fn read_vram_bank(&self, bank: u8, address: u16) -> u8 {
        self.vram
            .get(self.vram_index(bank, address))
            .copied()
            .unwrap_or(0xff)
    }

    fn vram_index(&self, bank: u8, address: u16) -> usize {
        usize::from(bank & 0x01) * VRAM_BANK_SIZE + address.wrapping_sub(0x8000) as usize
    }

    pub(crate) fn read_oam_raw(&self, address: u16) -> u8 {
        self.oam
            .get(address.wrapping_sub(0xfe00) as usize)
            .copied()
            .unwrap_or(0xff)
    }

    pub(crate) fn write_oam_raw(&mut self, address: u16, value: u8) {
        if let Some(byte) = self.oam.get_mut(address.wrapping_sub(0xfe00) as usize) {
            *byte = value;
        }
    }

    pub fn read_register(&self, address: u16) -> u8 {
        match address {
            0xff40 => self.lcdc,
            0xff41 => 0x80 | self.stat | self.coincidence_bit() | self.mode as u8,
            0xff42 => self.scy,
            0xff43 => self.scx,
            0xff44 => self.ly,
            0xff45 => self.lyc,
            0xff46 => self.dma,
            0xff47 => self.bgp,
            0xff48 => self.obp0,
            0xff49 => self.obp1,
            0xff4a => self.wy,
            0xff4b => self.wx,
            _ => 0xff,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address {
            0xff40 => self.write_lcdc(value),
            0xff41 => self.stat = value & 0x78,
            0xff42 => self.scy = value,
            0xff43 => self.scx = value,
            0xff44 => {}
            0xff45 => self.lyc = value,
            0xff46 => self.dma = value,
            0xff47 => self.bgp = value,
            0xff48 => self.obp0 = value,
            0xff49 => self.obp1 = value,
            0xff4a => self.wy = value,
            0xff4b => self.wx = value,
            _ => {}
        }
        self.update_stat_line();
    }

    pub fn tick(&mut self, cycles: u32) -> PpuEvents {
        let mut events = PpuEvents {
            stat_interrupt: std::mem::take(&mut self.pending_stat_interrupt),
            ..PpuEvents::default()
        };
        if !self.lcd_enabled() {
            return events;
        }

        for _ in 0..cycles {
            self.dot += 1;
            match self.mode {
                LcdMode::OamScan if self.dot == OAM_SCAN_END => {
                    self.mode = LcdMode::Drawing;
                    self.update_stat_event(&mut events);
                }
                LcdMode::Drawing if self.dot == DRAWING_END => {
                    self.render_scanline();
                    self.mode = LcdMode::HBlank;
                    self.update_stat_event(&mut events);
                }
                LcdMode::HBlank if self.dot == SCANLINE_END => {
                    self.dot = 0;
                    self.ly = self.ly.wrapping_add(1);
                    if self.ly == 144 {
                        self.mode = LcdMode::VBlank;
                        events.vblank_interrupt = true;
                        events.frame_ready = true;
                    } else {
                        self.mode = LcdMode::OamScan;
                    }
                    self.update_stat_event(&mut events);
                }
                LcdMode::VBlank if self.dot == SCANLINE_END => {
                    self.dot = 0;
                    self.ly = self.ly.wrapping_add(1);
                    if self.ly > 153 {
                        self.ly = 0;
                        self.mode = LcdMode::OamScan;
                    }
                    self.update_stat_event(&mut events);
                }
                _ => {}
            }
        }
        events
    }

    fn lcd_enabled(&self) -> bool {
        self.lcdc & 0x80 != 0
    }

    fn write_lcdc(&mut self, value: u8) {
        let was_enabled = self.lcd_enabled();
        self.lcdc = value;
        match (was_enabled, self.lcd_enabled()) {
            (true, false) => {
                self.dot = 0;
                self.ly = 0;
                self.mode = LcdMode::HBlank;
                self.stat_line = false;
                self.pending_stat_interrupt = false;
                self.framebuffer.clear(Shade::White);
            }
            (false, true) => {
                self.dot = 0;
                self.ly = 0;
                self.mode = LcdMode::OamScan;
            }
            _ => {}
        }
    }

    fn coincidence_bit(&self) -> u8 {
        if self.ly == self.lyc { 0x04 } else { 0 }
    }

    fn stat_signal(&self) -> bool {
        if !self.lcd_enabled() {
            return false;
        }
        (self.stat & 0x40 != 0 && self.ly == self.lyc)
            || (self.stat & 0x20 != 0 && self.mode == LcdMode::OamScan)
            || (self.stat & 0x10 != 0 && self.mode == LcdMode::VBlank)
            || (self.stat & 0x08 != 0 && self.mode == LcdMode::HBlank)
    }

    fn update_stat_line(&mut self) {
        let signal = self.stat_signal();
        if signal && !self.stat_line {
            self.pending_stat_interrupt = true;
        }
        self.stat_line = signal;
    }

    fn update_stat_event(&mut self, events: &mut PpuEvents) {
        self.update_stat_line();
        if self.pending_stat_interrupt {
            events.stat_interrupt = true;
            self.pending_stat_interrupt = false;
        }
    }

    fn render_scanline(&mut self) {
        if self.ly >= 144 {
            return;
        }

        let mut background_colors = [0u8; 160];
        for (x, background_color) in background_colors.iter_mut().enumerate() {
            let color = self.background_or_window_color(x as u8);
            *background_color = color;
            self.framebuffer
                .set_pixel(x, self.ly as usize, palette_shade(self.bgp, color));
        }

        if self.lcdc & 0x02 != 0 {
            self.render_sprites(&background_colors);
        }
    }

    fn background_or_window_color(&self, x: u8) -> u8 {
        if self.lcdc & 0x01 == 0 {
            return 0;
        }

        let window_visible =
            self.lcdc & 0x20 != 0 && self.ly >= self.wy && u16::from(x) + 7 >= u16::from(self.wx);
        let (map_base, pixel_x, pixel_y) = if window_visible {
            let base = if self.lcdc & 0x40 != 0 {
                0x9c00
            } else {
                0x9800
            };
            (
                base,
                x.wrapping_add(7).wrapping_sub(self.wx),
                self.ly.wrapping_sub(self.wy),
            )
        } else {
            let base = if self.lcdc & 0x08 != 0 {
                0x9c00
            } else {
                0x9800
            };
            (
                base,
                x.wrapping_add(self.scx),
                self.ly.wrapping_add(self.scy),
            )
        };

        let tile_x = u16::from(pixel_x / 8);
        let tile_y = u16::from(pixel_y / 8);
        let map_address = map_base + tile_y * 32 + tile_x;
        let tile_number = self.read_vram_bank(0, map_address);
        let tile_address = if self.lcdc & 0x10 != 0 {
            0x8000 + u16::from(tile_number) * 16
        } else {
            (0x9000i32 + i32::from(tile_number as i8) * 16) as u16
        };
        self.tile_color(tile_address, pixel_x % 8, pixel_y % 8)
    }

    fn render_sprites(&mut self, background_colors: &[u8; 160]) {
        let sprite_height = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
        let mut sprites = Vec::with_capacity(10);

        for index in 0..40usize {
            let start = index * 4;
            let raw_y = self.oam[start];
            let top = i16::from(raw_y) - 16;
            let line = i16::from(self.ly);
            if line >= top && line < top + sprite_height && sprites.len() < 10 {
                sprites.push((
                    index,
                    self.oam[start + 1],
                    raw_y,
                    self.oam[start + 2],
                    self.oam[start + 3],
                ));
            }
        }
        sprites.sort_by_key(|(index, raw_x, _, _, _)| (*raw_x, *index));

        for (x, background_color) in background_colors.iter().enumerate() {
            for (_, raw_x, raw_y, tile_number, attributes) in &sprites {
                let left = i16::from(*raw_x) - 8;
                let local_x = x as i16 - left;
                if !(0..8).contains(&local_x) {
                    continue;
                }

                let top = i16::from(*raw_y) - 16;
                let mut local_y = i16::from(self.ly) - top;
                if attributes & 0x40 != 0 {
                    local_y = sprite_height - 1 - local_y;
                }
                let pixel_x = if attributes & 0x20 != 0 {
                    7 - local_x as u8
                } else {
                    local_x as u8
                };
                let (tile, row) = if sprite_height == 16 {
                    (
                        (tile_number & 0xfe).wrapping_add((local_y / 8) as u8),
                        (local_y % 8) as u8,
                    )
                } else {
                    (*tile_number, local_y as u8)
                };
                let color = self.tile_color(0x8000 + u16::from(tile) * 16, pixel_x, row);
                if color == 0 {
                    continue;
                }

                if attributes & 0x80 == 0 || *background_color == 0 {
                    let palette = if attributes & 0x10 != 0 {
                        self.obp1
                    } else {
                        self.obp0
                    };
                    self.framebuffer
                        .set_pixel(x, self.ly as usize, palette_shade(palette, color));
                }
                break;
            }
        }
    }

    fn tile_color(&self, tile_address: u16, x: u8, y: u8) -> u8 {
        let row_address = tile_address + u16::from(y) * 2;
        let low = self.read_vram_bank(0, row_address);
        let high = self.read_vram_bank(0, row_address + 1);
        let bit = 7 - x;
        ((high >> bit) & 1) << 1 | ((low >> bit) & 1)
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::post_boot()
    }
}

fn palette_shade(palette: u8, color: u8) -> Shade {
    Shade::from_color((palette >> (color * 2)) & 0x03)
}

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

    fn set_tile_row(ppu: &mut Ppu, tile_address: u16, row: u8, low: u8, high: u8) {
        let address = tile_address + u16::from(row) * 2;
        ppu.vram[(address - 0x8000) as usize] = low;
        ppu.vram[(address - 0x8000 + 1) as usize] = high;
    }

    #[test]
    fn renders_background_with_unsigned_tiles_scroll_and_palette() {
        let mut ppu = Ppu::post_boot();
        ppu.write_register(0xff47, 0xe4);
        ppu.write_register(0xff43, 7);
        ppu.vram[(0x9800 - 0x8000) as usize] = 0;
        ppu.vram[(0x9801 - 0x8000) as usize] = 1;
        set_tile_row(&mut ppu, 0x8000, 0, 0x01, 0x00);
        set_tile_row(&mut ppu, 0x8010, 0, 0x80, 0x80);

        ppu.tick(252);

        assert_eq!(ppu.framebuffer().pixel(0, 0), Shade::LightGray);
        assert_eq!(ppu.framebuffer().pixel(1, 0), Shade::Black);
    }

    #[test]
    fn renders_background_with_signed_tile_addressing() {
        let mut ppu = Ppu::post_boot();
        ppu.write_register(0xff40, 0x81);
        ppu.write_register(0xff47, 0xe4);
        ppu.vram[(0x9800 - 0x8000) as usize] = 0xff;
        set_tile_row(&mut ppu, 0x8ff0, 0, 0x80, 0x00);

        ppu.tick(252);

        assert_eq!(ppu.framebuffer().pixel(0, 0), Shade::LightGray);
    }

    #[test]
    fn window_replaces_background_from_wx_minus_seven() {
        let mut ppu = Ppu::post_boot();
        ppu.write_register(0xff40, 0xf1);
        ppu.write_register(0xff47, 0xe4);
        ppu.write_register(0xff4a, 0);
        ppu.write_register(0xff4b, 10);
        ppu.vram[(0x9800 - 0x8000) as usize] = 0;
        ppu.vram[(0x9c00 - 0x8000) as usize] = 1;
        set_tile_row(&mut ppu, 0x8000, 0, 0x00, 0x00);
        set_tile_row(&mut ppu, 0x8010, 0, 0xff, 0x00);

        ppu.tick(252);

        assert_eq!(ppu.framebuffer().pixel(2, 0), Shade::White);
        assert_eq!(ppu.framebuffer().pixel(3, 0), Shade::LightGray);
    }

    #[test]
    fn sprite_uses_transparency_flip_palette_and_background_priority() {
        let mut ppu = Ppu::post_boot();
        ppu.write_register(0xff40, 0x93);
        ppu.write_register(0xff47, 0xe4);
        ppu.write_register(0xff48, 0xe4);
        ppu.write_register(0xff49, 0x1b);
        ppu.vram[(0x9800 - 0x8000) as usize] = 0;
        set_tile_row(&mut ppu, 0x8000, 0, 0x40, 0x00);
        set_tile_row(&mut ppu, 0x8010, 7, 0x03, 0x00);

        ppu.oam[0..4].copy_from_slice(&[16, 8, 1, 0xf0]);
        ppu.tick(252);

        assert_eq!(ppu.framebuffer().pixel(0, 0), Shade::DarkGray);
        assert_eq!(ppu.framebuffer().pixel(1, 0), Shade::LightGray);
    }

    #[test]
    fn lower_x_sprite_has_priority_and_only_first_ten_are_selected() {
        let mut ppu = Ppu::post_boot();
        ppu.write_register(0xff40, 0x93);
        ppu.write_register(0xff47, 0xe4);
        ppu.write_register(0xff48, 0xe4);
        set_tile_row(&mut ppu, 0x8010, 0, 0xff, 0x00);
        set_tile_row(&mut ppu, 0x8020, 0, 0xff, 0xff);

        ppu.oam[0..4].copy_from_slice(&[16, 10, 2, 0]);
        ppu.oam[4..8].copy_from_slice(&[16, 9, 1, 0]);
        for index in 2..10 {
            let start = index * 4;
            ppu.oam[start..start + 4].copy_from_slice(&[16, 160, 1, 0]);
        }
        ppu.oam[40..44].copy_from_slice(&[16, 8, 2, 0]);

        ppu.tick(252);

        assert_eq!(ppu.framebuffer().pixel(2, 0), Shade::LightGray);
        assert_eq!(ppu.framebuffer().pixel(0, 0), Shade::White);
    }

    #[test]
    fn eight_by_sixteen_sprite_ignores_tile_low_bit() {
        let mut ppu = Ppu::post_boot();
        ppu.write_register(0xff40, 0x97);
        ppu.write_register(0xff48, 0xe4);
        set_tile_row(&mut ppu, 0x8020, 0, 0x80, 0x00);
        set_tile_row(&mut ppu, 0x8030, 0, 0x00, 0x80);
        ppu.oam[0..4].copy_from_slice(&[16, 8, 3, 0]);

        ppu.tick(252);

        assert_eq!(ppu.framebuffer().pixel(0, 0), Shade::LightGray);
    }
}
