use super::MemoryBankController;

const ROM_BANK_SIZE: usize = 16 * 1024;
const RAM_BANK_SIZE: usize = 8 * 1024;

pub(super) struct Mmm01 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    mapped: bool,
    ram_enabled: bool,
    ram_bank_mask: u8,
    rom_bank_low: u8,
    rom_bank_mid: u8,
    ram_bank_low: u8,
    ram_bank_high: u8,
    rom_bank_high: u8,
    mode_write_locked: bool,
    banking_mode: bool,
    rom_bank_mask: u8,
    multiplex: bool,
}

impl Mmm01 {
    pub(super) fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size],
            mapped: false,
            ram_enabled: false,
            ram_bank_mask: 0,
            rom_bank_low: 0,
            rom_bank_mid: 0,
            ram_bank_low: 0,
            ram_bank_high: 0,
            rom_bank_high: 0,
            mode_write_locked: false,
            banking_mode: false,
            rom_bank_mask: 0,
            multiplex: false,
        }
    }

    fn rom_bank_count(&self) -> usize {
        (self.rom.len() / ROM_BANK_SIZE).max(1)
    }

    fn menu_bank(&self, upper_region: bool) -> usize {
        self.rom_bank_count()
            .saturating_sub(if upper_region { 1 } else { 2 })
    }

    fn fixed_rom_prefix(&self) -> usize {
        (self.rom_bank_high as usize) << 7
    }

    fn lower_rom_bank(&self) -> usize {
        if !self.mapped {
            return self.menu_bank(false);
        }

        let low = (self.rom_bank_low & self.rom_bank_mask) as usize;
        let middle = if self.multiplex {
            if self.banking_mode {
                self.ram_bank_low
            } else {
                self.ram_bank_low & self.ram_bank_mask
            }
        } else {
            self.rom_bank_mid
        };
        self.fixed_rom_prefix() | ((middle as usize) << 5) | low
    }

    fn upper_rom_bank(&self) -> usize {
        if !self.mapped {
            return self.menu_bank(true);
        }

        let mut low = self.rom_bank_low;
        if low & !self.rom_bank_mask & 0x1f == 0 {
            low |= 1;
        }
        let middle = if self.multiplex {
            self.ram_bank_low
        } else {
            self.rom_bank_mid
        };
        self.fixed_rom_prefix() | ((middle as usize) << 5) | low as usize
    }

    fn selected_ram_bank(&self) -> usize {
        let low = if self.multiplex {
            self.rom_bank_mid
        } else if self.banking_mode {
            self.ram_bank_low
        } else {
            self.ram_bank_low & self.ram_bank_mask
        };
        ((self.ram_bank_high as usize) << 2) | low as usize
    }

    fn read_rom_bank(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.rom_bank_count();
        self.rom
            .get(bank * ROM_BANK_SIZE + offset)
            .copied()
            .unwrap_or(0xff)
    }

    fn ram_index(&self, address: u16) -> Option<usize> {
        if !self.ram_enabled {
            return None;
        }
        let offset = (address as usize).checked_sub(0xa000)?;
        (offset < RAM_BANK_SIZE)
            .then_some(self.selected_ram_bank() * RAM_BANK_SIZE + offset)
            .filter(|index| *index < self.ram.len())
    }

    fn update_masked_register(register: &mut u8, value: u8, mask: u8, width_mask: u8) {
        let mask = mask & width_mask;
        *register = (*register & mask) | (value & !mask & width_mask);
    }
}

impl MemoryBankController for Mmm01 {
    fn read_rom(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3fff => self.read_rom_bank(self.lower_rom_bank(), address as usize),
            0x4000..=0x7fff => {
                self.read_rom_bank(self.upper_rom_bank(), address as usize - 0x4000)
            }
            _ => 0xff,
        }
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1fff => {
                self.ram_enabled = value & 0x0f == 0x0a;
                if !self.mapped {
                    self.ram_bank_mask = (value >> 4) & 0x03;
                    if value & 0x40 != 0 {
                        self.mapped = true;
                    }
                }
            }
            0x2000..=0x3fff => {
                Self::update_masked_register(
                    &mut self.rom_bank_low,
                    value,
                    self.rom_bank_mask,
                    0x1f,
                );
                if !self.mapped {
                    self.rom_bank_mid = (value >> 5) & 0x03;
                }
            }
            0x4000..=0x5fff => {
                Self::update_masked_register(
                    &mut self.ram_bank_low,
                    value,
                    self.ram_bank_mask,
                    0x03,
                );
                if !self.mapped {
                    self.ram_bank_high = (value >> 2) & 0x03;
                    self.rom_bank_high = (value >> 4) & 0x03;
                    self.mode_write_locked = value & 0x40 != 0;
                }
            }
            0x6000..=0x7fff => {
                if !self.mode_write_locked {
                    self.banking_mode = value & 0x01 != 0;
                }
                if !self.mapped {
                    self.rom_bank_mask = ((value >> 1) & 0x1f) & 0x1e;
                    self.multiplex = value & 0x40 != 0;
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
    fn starts_with_last_thirty_two_kib_menu_mapped() {
        let mbc = Mmm01::new(banked_rom(32), 0);

        assert_eq!(mbc.read_rom(0x0000), 30);
        assert_eq!(mbc.read_rom(0x4000), 31);
    }

    #[test]
    fn mapping_enable_selects_game_and_cannot_be_reversed() {
        let mut mbc = Mmm01::new(banked_rom(32), 0);
        mbc.write_rom(0x2000, 0x03);
        mbc.write_rom(0x0000, 0x40);

        assert_eq!(mbc.read_rom(0x0000), 0);
        assert_eq!(mbc.read_rom(0x4000), 3);

        mbc.write_rom(0x0000, 0x00);
        assert_eq!(mbc.read_rom(0x4000), 3);
    }

    #[test]
    fn rom_mask_preserves_game_selection_bits_after_mapping() {
        let mut mbc = Mmm01::new(banked_rom(64), 0);
        mbc.write_rom(0x2000, 0x11);
        mbc.write_rom(0x6000, 0x20);
        mbc.write_rom(0x0000, 0x40);

        mbc.write_rom(0x2000, 0x02);

        assert_eq!(mbc.read_rom(0x4000), 18);
    }

    #[test]
    fn switches_ram_with_game_and_local_bank_bits() {
        let mut mbc = Mmm01::new(banked_rom(32), 128 * 1024);
        mbc.write_rom(0x4000, 0x04);
        mbc.write_rom(0x0000, 0x4a);

        mbc.write_ram(0xa000, 0x11);
        mbc.write_rom(0x6000, 0x01);
        mbc.write_rom(0x4000, 0x05);
        mbc.write_ram(0xa000, 0x22);
        mbc.write_rom(0x4000, 0x04);

        assert_eq!(mbc.read_ram(0xa000), 0x11);
    }
}
