use crate::{
    boot::BootRom,
    cartridge::{Cartridge, CartridgeError, CartridgeHeader, PersistentState},
    cpu::Memory,
    joypad::{Joypad, JoypadButton},
    model::HardwareModel,
    ppu::{Ppu, framebuffer::Framebuffer},
    serial::Serial,
    timer::Timer,
};
use serde::{Deserialize, Serialize};

const WRAM_SIZE: usize = 0x8000;
const OAM_SIZE: u16 = 0x00a0;
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

#[derive(Serialize, Deserialize)]
pub struct Bus {
    cartridge: Cartridge,
    wram: Vec<u8>,
    io: Vec<u8>,
    hram: Vec<u8>,
    timer: Timer,
    serial: Serial,
    joypad: Joypad,
    ppu: Ppu,
    interrupt_flags: u8,
    interrupt_enable: u8,
    dma_source: u16,
    dma_index: u16,
    dma_cycle: u8,
    dma_active: bool,
    frame_ready: bool,
    model: HardwareModel,
    double_speed: bool,
    speed_switch_armed: bool,
    wram_bank: u8,
    hdma_source: u16,
    hdma_destination: u16,
    hdma_blocks_remaining: u8,
    hdma_active: bool,
    hdma_hblank: bool,
    boot_rom: Option<BootRom>,
    boot_rom_enabled: bool,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Self::with_model(cartridge, HardwareModel::Dmg)
    }

    pub fn with_model(cartridge: Cartridge, model: HardwareModel) -> Self {
        Self::with_model_and_boot_rom(cartridge, model, None)
    }

    pub fn with_model_and_boot_rom(
        cartridge: Cartridge,
        model: HardwareModel,
        boot_rom: Option<BootRom>,
    ) -> Self {
        let boot_rom_enabled = boot_rom.is_some();
        Self {
            cartridge,
            wram: vec![0; WRAM_SIZE],
            io: vec![0; IO_SIZE],
            hram: vec![0; HRAM_SIZE],
            timer: Timer::new(),
            serial: Serial::new(model),
            joypad: Joypad::new(),
            ppu: if boot_rom_enabled {
                Ppu::power_on_for_model(model)
            } else {
                Ppu::post_boot_for_model(model)
            },
            interrupt_flags: 0xe1,
            interrupt_enable: 0,
            dma_source: 0,
            dma_index: 0,
            dma_cycle: 0,
            dma_active: false,
            frame_ready: false,
            model,
            double_speed: false,
            speed_switch_armed: false,
            wram_bank: 1,
            hdma_source: 0,
            hdma_destination: 0x8000,
            hdma_blocks_remaining: 0,
            hdma_active: false,
            hdma_hblank: false,
            boot_rom,
            boot_rom_enabled,
        }
    }

    pub fn request_interrupt(&mut self, interrupt: Interrupt) {
        self.interrupt_flags |= interrupt as u8;
    }

    pub fn tick(&mut self, cycles: u32) {
        if self.timer.tick(cycles) {
            self.request_interrupt(Interrupt::Timer);
        }
        if self.serial.tick(cycles) {
            self.request_interrupt(Interrupt::Serial);
        }
        let ppu_cycles = if self.double_speed {
            cycles / 2
        } else {
            cycles
        };
        self.cartridge.tick(ppu_cycles);
        let ppu_events = self.ppu.tick(ppu_cycles);
        if ppu_events.vblank_interrupt {
            self.request_interrupt(Interrupt::VBlank);
        }
        if ppu_events.stat_interrupt {
            self.request_interrupt(Interrupt::LcdStat);
        }
        if ppu_events.hblank_started && self.hdma_active && self.hdma_hblank {
            self.transfer_hdma_block();
        }
        self.frame_ready |= ppu_events.frame_ready;
        for _ in 0..cycles {
            self.tick_dma();
        }
    }

    pub fn set_button(&mut self, button: JoypadButton, pressed: bool) {
        if self.joypad.set_button(button, pressed) {
            self.request_interrupt(Interrupt::Joypad);
        }
    }

    pub fn dma_active(&self) -> bool {
        self.dma_active
    }

    pub fn framebuffer(&self) -> &Framebuffer {
        self.ppu.framebuffer()
    }

    pub fn cartridge_header(&self) -> &CartridgeHeader {
        self.cartridge.header()
    }

    pub fn cartridge_persistent_state(&self) -> PersistentState {
        self.cartridge.persistent_state()
    }

    pub fn load_cartridge_persistent_state(
        &mut self,
        state: &PersistentState,
    ) -> Result<(), CartridgeError> {
        self.cartridge.load_persistent_state(state)
    }

    pub fn cartridge_persistent_dirty(&self) -> bool {
        self.cartridge.persistent_dirty()
    }

    pub fn clear_cartridge_persistent_dirty(&mut self) {
        self.cartridge.clear_persistent_dirty();
    }

    pub fn advance_cartridge_rtc(&mut self, elapsed_seconds: u64) {
        self.cartridge.advance_rtc(elapsed_seconds);
    }

    pub fn model(&self) -> HardwareModel {
        self.model
    }

    pub fn double_speed(&self) -> bool {
        self.double_speed
    }

    pub fn peek_byte(&self, address: u16) -> u8 {
        match address {
            0x8000..=0x9fff => self.ppu.read_vram_raw(address),
            0xfe00..=0xfe9f => self.ppu.read_oam_raw(address),
            _ => self.read_unrestricted(address),
        }
    }

    pub fn lcd_mode(&self) -> u8 {
        self.ppu.mode() as u8
    }

    pub fn ly(&self) -> u8 {
        self.ppu.read_register(0xff44)
    }

    pub fn take_frame_ready(&mut self) -> bool {
        std::mem::take(&mut self.frame_ready)
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        if self.dma_active && !(0xff80..=0xfffe).contains(&address) {
            return 0xff;
        }
        self.read_unrestricted(address)
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        if self.dma_active && !(0xff80..=0xfffe).contains(&address) {
            return;
        }
        self.write_unrestricted(address, value);
    }

    fn read_unrestricted(&self, address: u16) -> u8 {
        if self.boot_rom_enabled
            && let Some(value) = self.boot_rom.as_ref().and_then(|boot| boot.read(address))
        {
            return value;
        }

        match address {
            0x0000..=0x7fff => self.cartridge.read_rom(address),
            0x8000..=0x9fff => self.ppu.read_vram(address),
            0xa000..=0xbfff => self.cartridge.read_ram(address),
            0xc000..=0xdfff => self.read_wram(address),
            0xe000..=0xfdff => self.read_wram(address - 0x2000),
            0xfe00..=0xfe9f => self.ppu.read_oam(address),
            0xfea0..=0xfeff => 0xff,
            0xff00 => self.joypad.read(),
            0xff01..=0xff02 => self.serial.read(address),
            0xff03 => self.io[(address - 0xff00) as usize],
            0xff04..=0xff07 => self.timer.read(address),
            0xff08..=0xff0e => self.io[(address - 0xff00) as usize],
            0xff0f => self.interrupt_flags,
            0xff10..=0xff3f => self.io[(address - 0xff00) as usize],
            0xff40..=0xff4b => self.ppu.read_register(address),
            0xff4d if self.model == HardwareModel::Cgb => {
                0x7e | (u8::from(self.double_speed) << 7) | u8::from(self.speed_switch_armed)
            }
            0xff4d => 0xff,
            0xff4f if self.model == HardwareModel::Cgb => 0xfe | self.ppu.vram_bank(),
            0xff50 => u8::from(!self.boot_rom_enabled),
            0xff51..=0xff54 if self.model == HardwareModel::Cgb => 0xff,
            0xff55 if self.model == HardwareModel::Cgb => self.hdma_status(),
            0xff56 if self.model == HardwareModel::Cgb => {
                (self.io[(address - 0xff00) as usize] & 0xc1) | 0x3e
            }
            0xff56 => 0xff,
            0xff68..=0xff6b if self.model == HardwareModel::Cgb => self.ppu.read_register(address),
            0xff70 if self.model == HardwareModel::Cgb => 0xf8 | self.wram_bank,
            0xff4c..=0xff7f => self.io[(address - 0xff00) as usize],
            0xff80..=0xfffe => self.hram[(address - 0xff80) as usize],
            0xffff => self.interrupt_enable,
        }
    }

    fn write_unrestricted(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x7fff => self.cartridge.write_rom(address, value),
            0x8000..=0x9fff => self.ppu.write_vram(address, value),
            0xa000..=0xbfff => self.cartridge.write_ram(address, value),
            0xc000..=0xdfff => self.write_wram(address, value),
            0xe000..=0xfdff => self.write_wram(address - 0x2000, value),
            0xfe00..=0xfe9f => self.ppu.write_oam(address, value),
            0xfea0..=0xfeff => {}
            0xff00 => {
                if self.joypad.write(value) {
                    self.request_interrupt(Interrupt::Joypad);
                }
            }
            0xff01..=0xff02 => self.serial.write(address, value),
            0xff03 => self.io[(address - 0xff00) as usize] = value,
            0xff04..=0xff07 => self.timer.write(address, value),
            0xff08..=0xff0e => self.io[(address - 0xff00) as usize] = value,
            0xff0f => self.interrupt_flags = value | 0xe0,
            0xff10..=0xff3f => self.io[(address - 0xff00) as usize] = value,
            0xff40..=0xff45 => self.ppu.write_register(address, value),
            0xff46 => {
                self.ppu.write_register(address, value);
                self.dma_source = (value as u16) << 8;
                self.dma_index = 0;
                self.dma_cycle = 0;
                self.dma_active = true;
            }
            0xff47..=0xff4b => self.ppu.write_register(address, value),
            0xff4d if self.model == HardwareModel::Cgb => {
                self.speed_switch_armed = value & 0x01 != 0;
            }
            0xff4d => {}
            0xff4f if self.model == HardwareModel::Cgb => self.ppu.set_vram_bank(value),
            0xff50 if value != 0 => self.boot_rom_enabled = false,
            0xff51 if self.model == HardwareModel::Cgb => {
                self.hdma_source = (u16::from(value) << 8) | (self.hdma_source & 0x00f0);
            }
            0xff52 if self.model == HardwareModel::Cgb => {
                self.hdma_source = (self.hdma_source & 0xff00) | u16::from(value & 0xf0);
            }
            0xff53 if self.model == HardwareModel::Cgb => {
                self.hdma_destination =
                    0x8000 | (u16::from(value & 0x1f) << 8) | (self.hdma_destination & 0x00f0);
            }
            0xff54 if self.model == HardwareModel::Cgb => {
                self.hdma_destination =
                    (self.hdma_destination & 0x1f00) | 0x8000 | u16::from(value & 0xf0);
            }
            0xff55 if self.model == HardwareModel::Cgb => self.start_hdma(value),
            0xff56 if self.model == HardwareModel::Cgb => {
                self.io[(address - 0xff00) as usize] = value & 0xc1;
            }
            0xff56 => {}
            0xff68..=0xff6b if self.model == HardwareModel::Cgb => {
                self.ppu.write_register(address, value);
            }
            0xff70 if self.model == HardwareModel::Cgb => {
                self.wram_bank = value & 0x07;
                if self.wram_bank == 0 {
                    self.wram_bank = 1;
                }
            }
            0xff4c..=0xff7f => self.io[(address - 0xff00) as usize] = value,
            0xff80..=0xfffe => self.hram[(address - 0xff80) as usize] = value,
            0xffff => self.interrupt_enable = value,
        }
    }

    fn tick_dma(&mut self) {
        if !self.dma_active {
            return;
        }
        self.dma_cycle += 1;
        if self.dma_cycle < 4 {
            return;
        }
        self.dma_cycle = 0;

        let source = self.dma_source.wrapping_add(self.dma_index);
        let value = self.read_dma_source(source);
        self.ppu.write_oam_raw(0xfe00 + self.dma_index, value);
        self.dma_index += 1;
        if self.dma_index == OAM_SIZE {
            self.dma_active = false;
        }
    }

    fn read_dma_source(&self, address: u16) -> u8 {
        match address {
            0x8000..=0x9fff => self.ppu.read_vram_raw(address),
            0xfe00..=0xfe9f => self.ppu.read_oam_raw(address),
            _ => self.read_unrestricted(address),
        }
    }

    fn wram_index(&self, address: u16) -> usize {
        match address {
            0xc000..=0xcfff => (address - 0xc000) as usize,
            0xd000..=0xdfff => usize::from(self.wram_bank) * 0x1000 + (address - 0xd000) as usize,
            _ => unreachable!("WRAM 地址必须位于 C000-DFFF"),
        }
    }

    fn read_wram(&self, address: u16) -> u8 {
        self.wram[self.wram_index(address)]
    }

    fn write_wram(&mut self, address: u16, value: u8) {
        let index = self.wram_index(address);
        self.wram[index] = value;
    }

    fn hdma_status(&self) -> u8 {
        if self.hdma_blocks_remaining == 0 {
            return 0xff;
        }
        let remaining = self.hdma_blocks_remaining - 1;
        if self.hdma_active {
            remaining
        } else {
            0x80 | remaining
        }
    }

    fn start_hdma(&mut self, value: u8) {
        if self.hdma_active && self.hdma_hblank && value & 0x80 == 0 {
            self.hdma_active = false;
            return;
        }

        self.hdma_blocks_remaining = (value & 0x7f) + 1;
        self.hdma_hblank = value & 0x80 != 0;
        self.hdma_active = true;
        if !self.hdma_hblank {
            while self.hdma_active {
                self.transfer_hdma_block();
            }
        }
    }

    fn transfer_hdma_block(&mut self) {
        for offset in 0..0x10u16 {
            let value = self.read_dma_source(self.hdma_source.wrapping_add(offset));
            self.ppu
                .write_vram_raw(self.hdma_destination.wrapping_add(offset), value);
        }
        self.hdma_source = self.hdma_source.wrapping_add(0x10);
        self.hdma_destination = 0x8000 | (self.hdma_destination.wrapping_add(0x10) & 0x1ff0);
        self.hdma_blocks_remaining -= 1;
        if self.hdma_blocks_remaining == 0 {
            self.hdma_active = false;
            self.hdma_hblank = false;
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

    fn tick(&mut self, cycles: u8) {
        Bus::tick(self, u32::from(cycles));
    }

    fn stop(&mut self) -> bool {
        if self.model != HardwareModel::Cgb || !self.speed_switch_armed {
            return false;
        }
        self.double_speed = !self.double_speed;
        self.speed_switch_armed = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::{cartridge::Cartridge, cpu::Memory, model::HardwareModel};

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

    fn cgb_bus() -> Bus {
        let mut rom = vec![0; 32 * 1024];
        rom[0x134..0x137].copy_from_slice(b"CGB");
        rom[0x143] = 0x80;
        rom[0x147] = 0x00;
        rom[0x148] = 0x00;
        rom[0x149] = 0x00;
        Bus::with_model(
            Cartridge::from_bytes(rom).expect("测试卡带应有效"),
            HardwareModel::Cgb,
        )
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
        bus.write8(0xff40, 0x00);

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
    fn dmg_rejects_cgb_speed_switch_register() {
        let mut bus = test_bus();

        assert_eq!(bus.read8(0xff4d), 0xff);
        bus.write8(0xff4d, 0x01);
        assert_eq!(bus.read8(0xff4d), 0xff);
    }

    #[test]
    fn cgb_key1_arms_and_switches_double_speed() {
        let mut bus = cgb_bus();

        assert_eq!(bus.read8(0xff4d), 0x7e);
        bus.write8(0xff4d, 0x01);
        assert_eq!(bus.read8(0xff4d), 0x7f);
        assert!(Memory::stop(&mut bus));
        assert_eq!(bus.read8(0xff4d), 0xfe);
        assert!(bus.double_speed());
    }

    #[test]
    fn cgb_switches_wram_banks_and_maps_zero_to_one() {
        let mut bus = cgb_bus();

        bus.write8(0xff70, 0x02);
        bus.write8(0xd000, 0x22);
        bus.write8(0xff70, 0x03);
        bus.write8(0xd000, 0x33);
        assert_eq!(bus.read8(0xd000), 0x33);

        bus.write8(0xff70, 0x02);
        assert_eq!(bus.read8(0xd000), 0x22);
        bus.write8(0xff70, 0x00);
        assert_eq!(bus.read8(0xff70) & 0x07, 1);
    }

    #[test]
    fn cgb_switches_cpu_visible_vram_bank() {
        let mut bus = cgb_bus();
        bus.write8(0xff40, 0x00);

        bus.write8(0xff4f, 0x00);
        bus.write8(0x8000, 0x11);
        bus.write8(0xff4f, 0x01);
        bus.write8(0x8000, 0x22);
        assert_eq!(bus.read8(0x8000), 0x22);

        bus.write8(0xff4f, 0x00);
        assert_eq!(bus.read8(0x8000), 0x11);
    }

    #[test]
    fn cgb_gdma_copies_requested_blocks_to_vram() {
        let mut bus = cgb_bus();
        bus.write8(0xff40, 0x00);
        for offset in 0..32u16 {
            bus.write8(0xc000 + offset, offset as u8);
        }

        bus.write8(0xff51, 0xc0);
        bus.write8(0xff52, 0x00);
        bus.write8(0xff53, 0x00);
        bus.write8(0xff54, 0x00);
        bus.write8(0xff55, 0x01);

        for offset in 0..32u16 {
            assert_eq!(bus.read8(0x8000 + offset), offset as u8);
        }
        assert_eq!(bus.read8(0xff55), 0xff);
    }

    #[test]
    fn cgb_hdma_copies_one_block_per_hblank() {
        let mut bus = cgb_bus();
        for offset in 0..32u16 {
            bus.write8(0xc000 + offset, 0x80 | offset as u8);
        }
        bus.write8(0xff51, 0xc0);
        bus.write8(0xff52, 0x00);
        bus.write8(0xff53, 0x00);
        bus.write8(0xff54, 0x00);
        bus.write8(0xff55, 0x81);

        bus.tick(252);
        assert_eq!(bus.peek_byte(0x800f), 0x8f);
        assert_eq!(bus.peek_byte(0x8010), 0x00);

        bus.tick(456);
        assert_eq!(bus.peek_byte(0x801f), 0x9f);
        assert_eq!(bus.read8(0xff55), 0xff);
    }

    #[test]
    fn cgb_hdma_address_registers_are_write_only() {
        let mut bus = cgb_bus();

        bus.write8(0xff51, 0xc1);
        bus.write8(0xff52, 0x2f);
        bus.write8(0xff53, 0x9f);
        bus.write8(0xff54, 0x3f);

        assert_eq!(bus.read8(0xff51), 0xff);
        assert_eq!(bus.read8(0xff52), 0xff);
        assert_eq!(bus.read8(0xff53), 0xff);
        assert_eq!(bus.read8(0xff54), 0xff);
    }

    #[test]
    fn cgb_can_cancel_hblank_dma_without_copying_remaining_blocks() {
        let mut bus = cgb_bus();
        for offset in 0..32u16 {
            bus.write8(0xc000 + offset, 0x40 | offset as u8);
        }
        bus.write8(0xff51, 0xc0);
        bus.write8(0xff52, 0x00);
        bus.write8(0xff53, 0x00);
        bus.write8(0xff54, 0x00);
        bus.write8(0xff55, 0x81);

        bus.tick(252);
        assert_eq!(bus.read8(0xff55), 0x00);
        bus.write8(0xff55, 0x00);
        assert_eq!(bus.read8(0xff55), 0x80);

        bus.tick(456);
        assert_eq!(bus.peek_byte(0x800f), 0x4f);
        assert_eq!(bus.peek_byte(0x8010), 0x00);
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
    fn internal_serial_transfer_completes_and_requests_interrupt() {
        let mut bus = test_bus();
        bus.write8(0xff0f, 0x00);
        bus.write8(0xff01, 0x55);
        bus.write8(0xff02, 0x81);

        bus.tick(4_095);
        assert_ne!(bus.read8(0xff02) & 0x80, 0);
        assert_eq!(bus.read8(0xff0f) & Interrupt::Serial as u8, 0);

        bus.tick(1);
        assert_eq!(bus.read8(0xff02) & 0x80, 0);
        assert_eq!(bus.read8(0xff01), 0xff);
        assert_ne!(bus.read8(0xff0f) & Interrupt::Serial as u8, 0);
    }

    #[test]
    fn external_serial_clock_waits_for_a_link_peer() {
        let mut bus = test_bus();
        bus.write8(0xff0f, 0x00);
        bus.write8(0xff01, 0x55);
        bus.write8(0xff02, 0x80);

        bus.tick(10_000);

        assert_ne!(bus.read8(0xff02) & 0x80, 0);
        assert_eq!(bus.read8(0xff01), 0x55);
        assert_eq!(bus.read8(0xff0f) & Interrupt::Serial as u8, 0);
    }

    #[test]
    fn cgb_fast_serial_transfer_completes_in_one_hundred_twenty_eight_cycles() {
        let mut bus = cgb_bus();
        bus.write8(0xff0f, 0x00);
        bus.write8(0xff01, 0x00);
        bus.write8(0xff02, 0x83);

        bus.tick(127);
        assert_ne!(bus.read8(0xff02) & 0x80, 0);

        bus.tick(1);
        assert_eq!(bus.read8(0xff02) & 0x80, 0);
        assert_eq!(bus.read8(0xff01), 0xff);
        assert_ne!(bus.read8(0xff0f) & Interrupt::Serial as u8, 0);
    }

    #[test]
    fn cgb_infrared_port_masks_control_bits_and_reports_no_signal() {
        let mut bus = cgb_bus();

        assert_eq!(bus.read8(0xff56), 0x3e);
        bus.write8(0xff56, 0xff);
        assert_eq!(bus.read8(0xff56), 0xff);
        bus.write8(0xff56, 0x01);
        assert_eq!(bus.read8(0xff56), 0x3f);
    }

    #[test]
    fn dmg_does_not_expose_cgb_infrared_port() {
        let mut bus = test_bus();

        bus.write8(0xff56, 0x01);

        assert_eq!(bus.read8(0xff56), 0xff);
    }

    #[test]
    fn dma_copies_one_hundred_sixty_bytes_in_six_hundred_forty_cycles() {
        let mut bus = test_bus();
        bus.write8(0xff40, 0x00);
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

    #[test]
    fn routes_ppu_memory_and_registers() {
        let mut bus = test_bus();
        bus.write8(0xff40, 0x00);
        bus.write8(0x8000, 0x12);
        bus.write8(0xfe00, 0x34);
        bus.write8(0xff42, 0x56);

        assert_eq!(bus.read8(0x8000), 0x12);
        assert_eq!(bus.read8(0xfe00), 0x34);
        assert_eq!(bus.read8(0xff42), 0x56);
    }

    #[test]
    fn ppu_tick_requests_vblank_and_stat_interrupts() {
        let mut bus = test_bus();
        bus.write8(0xff0f, 0x00);
        bus.write8(0xff41, 0x10);

        bus.tick(456 * 144);

        assert_ne!(bus.read8(0xff0f) & Interrupt::VBlank as u8, 0);
        assert_ne!(bus.read8(0xff0f) & Interrupt::LcdStat as u8, 0);
        assert!(bus.take_frame_ready());
        assert!(!bus.take_frame_ready());
    }
}
