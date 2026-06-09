use crate::{
    cartridge::Cartridge,
    cpu::Memory,
    joypad::{Joypad, JoypadButton},
    timer::Timer,
};

const VRAM_SIZE: usize = 0x2000;
const WRAM_SIZE: usize = 0x2000;
const OAM_SIZE: usize = 0x00a0;
const IO_SIZE: usize = 0x0080;
const HRAM_SIZE: usize = 0x007f;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Interrupt {
    VBlank = 0x01,
    LcdStat = 0x02,
    Timer = 0x04,
    Serial = 0x08,
    Joypad = 0x10,
}

pub struct Bus {
    cartridge: Cartridge,
    vram: [u8; VRAM_SIZE],
    wram: [u8; WRAM_SIZE],
    oam: [u8; OAM_SIZE],
    io: [u8; IO_SIZE],
    hram: [u8; HRAM_SIZE],
    timer: Timer,
    joypad: Joypad,
    interrupt_flags: u8,
    interrupt_enable: u8,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            cartridge,
            vram: [0; VRAM_SIZE],
            wram: [0; WRAM_SIZE],
            oam: [0; OAM_SIZE],
            io: [0; IO_SIZE],
            hram: [0; HRAM_SIZE],
            timer: Timer::new(),
            joypad: Joypad::new(),
            interrupt_flags: 0xe1,
            interrupt_enable: 0,
        }
    }

    pub fn request_interrupt(&mut self, interrupt: Interrupt) {
        self.interrupt_flags |= interrupt as u8;
    }

    pub fn tick(&mut self, cycles: u16) {
        if self.timer.tick(cycles) {
            self.request_interrupt(Interrupt::Timer);
        }
    }

    pub fn set_button(&mut self, button: JoypadButton, pressed: bool) {
        if self.joypad.set_button(button, pressed) {
            self.request_interrupt(Interrupt::Joypad);
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x7fff => self.cartridge.read_rom(address),
            0x8000..=0x9fff => self.vram[(address - 0x8000) as usize],
            0xa000..=0xbfff => self.cartridge.read_ram(address),
            0xc000..=0xdfff => self.wram[(address - 0xc000) as usize],
            0xe000..=0xfdff => self.wram[(address - 0xe000) as usize],
            0xfe00..=0xfe9f => self.oam[(address - 0xfe00) as usize],
            0xfea0..=0xfeff => 0xff,
            0xff00 => self.joypad.read(),
            0xff04..=0xff07 => self.timer.read(address),
            0xff0f => self.interrupt_flags,
            0xff00..=0xff7f => self.io[(address - 0xff00) as usize],
            0xff80..=0xfffe => self.hram[(address - 0xff80) as usize],
            0xffff => self.interrupt_enable,
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x7fff => self.cartridge.write_rom(address, value),
            0x8000..=0x9fff => self.vram[(address - 0x8000) as usize] = value,
            0xa000..=0xbfff => self.cartridge.write_ram(address, value),
            0xc000..=0xdfff => self.wram[(address - 0xc000) as usize] = value,
            0xe000..=0xfdff => self.wram[(address - 0xe000) as usize] = value,
            0xfe00..=0xfe9f => self.oam[(address - 0xfe00) as usize] = value,
            0xfea0..=0xfeff => {}
            0xff00 => {
                if self.joypad.write(value) {
                    self.request_interrupt(Interrupt::Joypad);
                }
            }
            0xff04..=0xff07 => self.timer.write(address, value),
            0xff0f => self.interrupt_flags = value | 0xe0,
            0xff00..=0xff7f => self.io[(address - 0xff00) as usize] = value,
            0xff80..=0xfffe => self.hram[(address - 0xff80) as usize] = value,
            0xffff => self.interrupt_enable = value,
        }
    }
}

impl Memory for Bus {
    fn read8(&self, address: u16) -> u8 {
        self.read_byte(address)
    }

    fn write8(&mut self, address: u16, value: u8) {
        self.write_byte(address, value);
    }
}

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

    #[test]
    fn routes_timer_registers_and_requests_timer_interrupt() {
        let mut bus = test_bus();
        bus.write8(0xff06, 0x42);
        bus.write8(0xff05, 0xff);
        bus.write8(0xff07, 0x05);

        bus.tick(20);

        assert_eq!(bus.read8(0xff05), 0x42);
        assert_ne!(bus.read8(0xff0f) & Interrupt::Timer as u8, 0);
    }

    #[test]
    fn routes_joypad_and_requests_joypad_interrupt() {
        let mut bus = test_bus();
        bus.write8(0xff00, 0x10);

        bus.set_button(crate::joypad::JoypadButton::A, true);

        assert_eq!(bus.read8(0xff00) & 0x0f, 0x0e);
        assert_ne!(bus.read8(0xff0f) & Interrupt::Joypad as u8, 0);
    }

    #[test]
    fn dma_copies_one_hundred_sixty_bytes_in_six_hundred_forty_cycles() {
        let mut bus = test_bus();
        for offset in 0..0x00a0 {
            bus.write8(0xc000 + offset, offset as u8);
        }

        bus.write8(0xff46, 0xc0);
        assert!(bus.dma_active());
        bus.tick(639);
        assert!(bus.dma_active());
        bus.tick(1);
        assert!(!bus.dma_active());

        for offset in 0..0x00a0 {
            assert_eq!(bus.read8(0xfe00 + offset), offset as u8);
        }
    }

    #[test]
    fn dma_blocks_cpu_bus_except_high_ram() {
        let mut bus = test_bus();
        bus.write8(0xc000, 0x11);
        bus.write8(0xff46, 0xc0);

        assert_eq!(bus.read8(0xc000), 0xff);
        bus.write8(0xc000, 0x22);
        bus.write8(0xff80, 0x33);
        assert_eq!(bus.read8(0xff80), 0x33);

        bus.tick(640);
        assert_eq!(bus.read8(0xc000), 0x11);
    }
}
