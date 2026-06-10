use super::MemoryBankController;

const ROM_BANK_SIZE: usize = 16 * 1024;
const RAM_BANK_SIZE: usize = 8 * 1024;

pub(super) struct Mbc5 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank: u16,
    ram_bank: u8,
    ram_enabled: bool,
    has_rumble: bool,
    rumble_active: bool,
}

impl Mbc5 {
    pub(super) fn new(rom: Vec<u8>, ram_size: usize, has_rumble: bool) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
            has_rumble,
            rumble_active: false,
        }
    }

    pub(super) fn rumble_active(&self) -> bool {
        self.rumble_active
    }

    fn read_rom_bank(&self, bank: usize, offset: usize) -> u8 {
        let bank_count = (self.rom.len() / ROM_BANK_SIZE).max(1);
        let index = (bank % bank_count) * ROM_BANK_SIZE + offset;
        self.rom.get(index).copied().unwrap_or(0xff)
    }

    fn ram_index(&self, address: u16) -> Option<usize> {
        if !self.ram_enabled {
            return None;
        }
        let offset = (address as usize).checked_sub(0xa000)?;
        (offset < RAM_BANK_SIZE)
            .then_some(self.ram_bank as usize * RAM_BANK_SIZE + offset)
            .filter(|index| *index < self.ram.len())
    }
}

impl MemoryBankController for Mbc5 {
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
            0x0000..=0x1fff => self.ram_enabled = value & 0x0f == 0x0a,
            0x2000..=0x2fff => self.rom_bank = (self.rom_bank & 0x0100) | value as u16,
            0x3000..=0x3fff => {
                self.rom_bank = (self.rom_bank & 0x00ff) | (((value & 0x01) as u16) << 8);
            }
            0x4000..=0x5fff => {
                if self.has_rumble {
                    self.rumble_active = value & 0x08 != 0;
                    self.ram_bank = value & 0x07;
                } else {
                    self.ram_bank = value & 0x0f;
                }
            }
            _ => {}
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        self.ram_index(address)
            .and_then(|index| self.ram.get(index))
            .copied()
            .unwrap_or(0xff)
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if let Some(index) = self.ram_index(address)
            && let Some(byte) = self.ram.get_mut(index)
        {
            *byte = value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::MemoryBankController;

    fn banked_rom(bank_count: usize) -> Vec<u8> {
        let mut rom = vec![0; bank_count * 16 * 1024];
        for bank in 0..bank_count {
            rom[bank * 0x4000] = bank as u8;
            rom[bank * 0x4000 + 1] = (bank >> 8) as u8;
        }
        rom
    }

    #[test]
    fn selects_all_nine_rom_bank_bits() {
        let mut mbc = Mbc5::new(banked_rom(0x102), 0, false);

        mbc.write_rom(0x2000, 0x01);
        mbc.write_rom(0x3000, 0x01);

        assert_eq!(mbc.read_rom(0x4000), 0x01);
        assert_eq!(mbc.read_rom(0x4001), 0x01);
    }

    #[test]
    fn isolates_rumble_bit_from_ram_bank() {
        let mut mbc = Mbc5::new(banked_rom(2), 64 * 1024, true);
        mbc.write_rom(0x0000, 0x0a);

        mbc.write_rom(0x4000, 0x0a);
        assert!(mbc.rumble_active());
        mbc.write_ram(0xa000, 0x22);

        mbc.write_rom(0x4000, 0x02);
        assert!(!mbc.rumble_active());
        assert_eq!(mbc.read_ram(0xa000), 0x22);
    }
}
