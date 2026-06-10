use super::{
    CartridgeError, MemoryBankController, PersistentState, load_persistent_bytes,
};

const ROM_BANK_SIZE: usize = 16 * 1024;
const RAM_SIZE: usize = 512;

pub(super) struct Mbc2 {
    rom: Vec<u8>,
    ram: [u8; RAM_SIZE],
    rom_bank: u8,
    ram_enabled: bool,
}

impl Mbc2 {
    pub(super) fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            ram: [0; RAM_SIZE],
            rom_bank: 1,
            ram_enabled: false,
        }
    }

    fn read_rom_bank(&self, bank: usize, offset: usize) -> u8 {
        let bank_count = (self.rom.len() / ROM_BANK_SIZE).max(1);
        let index = (bank % bank_count) * ROM_BANK_SIZE + offset;
        self.rom.get(index).copied().unwrap_or(0xff)
    }

    fn ram_index(address: u16) -> Option<usize> {
        (0xa000..=0xbfff)
            .contains(&address)
            .then_some((address as usize - 0xa000) & 0x01ff)
    }
}

impl MemoryBankController for Mbc2 {
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
        if address > 0x3fff {
            return;
        }

        if address & 0x0100 == 0 {
            self.ram_enabled = value & 0x0f == 0x0a;
        } else {
            self.rom_bank = value & 0x0f;
            if self.rom_bank == 0 {
                self.rom_bank = 1;
            }
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled {
            return 0xff;
        }

        Self::ram_index(address)
            .map(|index| 0xf0 | self.ram[index])
            .unwrap_or(0xff)
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }

        if let Some(index) = Self::ram_index(address) {
            self.ram[index] = value & 0x0f;
        }
    }

    fn persistent_state(&self) -> PersistentState {
        PersistentState {
            ram: self.ram.to_vec(),
            ..PersistentState::default()
        }
    }

    fn load_persistent_state(
        &mut self,
        state: &PersistentState,
    ) -> Result<(), CartridgeError> {
        load_persistent_bytes(&mut self.ram, &state.ram, "MBC2 RAM")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::MemoryBankController;

    fn banked_rom() -> Vec<u8> {
        let mut rom = vec![0; 16 * 16 * 1024];
        for bank in 0..16 {
            rom[bank * 0x4000..(bank + 1) * 0x4000].fill(bank as u8);
        }
        rom
    }

    #[test]
    fn address_bit_eight_selects_ram_enable_or_rom_bank() {
        let mut mbc = Mbc2::new(banked_rom());

        mbc.write_rom(0x0100, 0x0a);
        mbc.write_ram(0xa000, 0x05);
        assert_eq!(mbc.read_ram(0xa000), 0xff);

        mbc.write_rom(0x0000, 0x0a);
        mbc.write_ram(0xa000, 0x05);
        assert_eq!(mbc.read_ram(0xa000), 0xf5);

        mbc.write_rom(0x2100, 0x03);
        assert_eq!(mbc.read_rom(0x4000), 0x03);
    }

    #[test]
    fn mirrors_five_hundred_twelve_nibbles() {
        let mut mbc = Mbc2::new(banked_rom());
        mbc.write_rom(0x0000, 0x0a);

        mbc.write_ram(0xa1ff, 0xab);

        assert_eq!(mbc.read_ram(0xa1ff), 0xfb);
        assert_eq!(mbc.read_ram(0xa3ff), 0xfb);
    }
}
