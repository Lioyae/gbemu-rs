use super::{
    CartridgeError, MemoryBankController, PersistentState, load_persistent_bytes,
};

const ROM_BANK_SIZE: usize = 16 * 1024;
const RAM_BANK_SIZE: usize = 8 * 1024;
const CAMERA_RAM_SIZE: usize = 128 * 1024;
const CAMERA_REGISTER_COUNT: usize = 0x36;
const IMAGE_OFFSET: usize = 0x100;
const IMAGE_SIZE: usize = 128 * 112 / 4;

pub(super) struct Camera {
    rom: Vec<u8>,
    ram: Vec<u8>,
    registers: [u8; CAMERA_REGISTER_COUNT],
    rom_bank: u8,
    ram_bank: u8,
    ram_writable: bool,
    registers_active: bool,
}

impl Camera {
    pub(super) fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size.max(CAMERA_RAM_SIZE)],
            registers: [0; CAMERA_REGISTER_COUNT],
            rom_bank: 1,
            ram_bank: 0,
            ram_writable: false,
            registers_active: false,
        }
    }

    #[cfg(test)]
    pub(super) fn register(&self, index: usize) -> u8 {
        self.registers.get(index).copied().unwrap_or(0)
    }

    #[cfg(test)]
    pub(super) fn ram(&self) -> &[u8] {
        &self.ram
    }

    fn read_rom_bank(&self, bank: usize, offset: usize) -> u8 {
        let bank_count = (self.rom.len() / ROM_BANK_SIZE).max(1);
        self.rom
            .get(bank % bank_count * ROM_BANK_SIZE + offset)
            .copied()
            .unwrap_or(0xff)
    }

    fn ram_index(&self, address: u16) -> Option<usize> {
        let offset = (address as usize).checked_sub(0xa000)?;
        (offset < RAM_BANK_SIZE)
            .then_some(self.ram_bank as usize * RAM_BANK_SIZE + offset)
            .filter(|index| *index < self.ram.len())
    }

    fn capture_placeholder(&mut self) {
        let end = (IMAGE_OFFSET + IMAGE_SIZE).min(self.ram.len());
        for (index, byte) in self.ram[IMAGE_OFFSET..end].iter_mut().enumerate() {
            *byte = if index & 1 == 0 { 0xaa } else { 0x55 };
        }
    }
}

impl MemoryBankController for Camera {
    fn read_rom(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3fff => self.read_rom_bank(0, address as usize),
            0x4000..=0x7fff => {
                self.read_rom_bank(self.rom_bank as usize, address as usize - 0x4000)
            }
            _ => 0xff,
        }
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1fff => self.ram_writable = value & 0x0f == 0x0a,
            0x2000..=0x3fff => self.rom_bank = value & 0x3f,
            0x4000..=0x5fff => {
                self.registers_active = value & 0x10 != 0;
                if !self.registers_active {
                    self.ram_bank = value & 0x0f;
                }
            }
            _ => {}
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !(0xa000..=0xbfff).contains(&address) {
            return 0xff;
        }
        if self.registers_active {
            return if address & 0x007f == 0 {
                self.registers[0] & 0x07
            } else {
                0
            };
        }
        self.ram_index(address)
            .and_then(|index| self.ram.get(index))
            .copied()
            .unwrap_or(0xff)
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !(0xa000..=0xbfff).contains(&address) {
            return;
        }
        if self.registers_active {
            let register = (address & 0x007f) as usize;
            if register >= self.registers.len() {
                return;
            }
            if register == 0 && value & 0x01 != 0 {
                self.registers[0] = value & 0x07;
                self.capture_placeholder();
                self.registers[0] &= 0x06;
            } else {
                self.registers[register] = value;
            }
            return;
        }
        if !self.ram_writable {
            return;
        }
        if let Some(index) = self.ram_index(address)
            && let Some(byte) = self.ram.get_mut(index)
        {
            *byte = value;
        }
    }

    fn persistent_state(&self) -> PersistentState {
        PersistentState {
            ram: self.ram.clone(),
            ..PersistentState::default()
        }
    }

    fn load_persistent_state(
        &mut self,
        state: &PersistentState,
    ) -> Result<(), CartridgeError> {
        load_persistent_bytes(&mut self.ram, &state.ram, "Camera RAM")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::MemoryBankController;

    fn banked_rom() -> Vec<u8> {
        let mut rom = vec![0; 64 * 16 * 1024];
        for bank in 0..64 {
            rom[bank * 0x4000] = bank as u8;
        }
        rom
    }

    #[test]
    fn switches_rom_and_ram_banks() {
        let mut camera = Camera::new(banked_rom(), 128 * 1024);
        camera.write_rom(0x2000, 0x03);
        assert_eq!(camera.read_rom(0x4000), 3);

        camera.write_rom(0x0000, 0x0a);
        camera.write_rom(0x4000, 0x02);
        camera.write_ram(0xa000, 0x22);
        camera.write_rom(0x4000, 0x01);
        camera.write_ram(0xa000, 0x11);
        camera.write_rom(0x4000, 0x02);
        assert_eq!(camera.read_ram(0xa000), 0x22);
    }

    #[test]
    fn camera_registers_are_mirrored_every_one_hundred_twenty_eight_bytes() {
        let mut camera = Camera::new(banked_rom(), 128 * 1024);
        camera.write_rom(0x4000, 0x10);
        camera.write_ram(0xa081, 0x77);

        assert_eq!(camera.register(1), 0x77);
        assert_eq!(camera.read_ram(0xa001), 0x00);
    }

    #[test]
    fn capture_generates_deterministic_gray_tile_data() {
        let mut camera = Camera::new(banked_rom(), 128 * 1024);
        camera.write_rom(0x4000, 0x10);
        camera.write_ram(0xa000, 0x01);
        assert_eq!(camera.read_ram(0xa000), 0x00);

        camera.write_rom(0x4000, 0x00);
        assert_eq!(&camera.ram()[0x100..0x104], &[0xaa, 0x55, 0xaa, 0x55]);
    }
}
