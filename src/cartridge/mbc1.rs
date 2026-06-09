use super::MemoryBankController;

const ROM_BANK_SIZE: usize = 16 * 1024;
const RAM_BANK_SIZE: usize = 8 * 1024;

pub(super) struct Mbc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank_low: u8,
    bank_high: u8,
    ram_enabled: bool,
    ram_banking_mode: bool,
}

impl Mbc1 {
    pub(super) fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size],
            rom_bank_low: 1,
            bank_high: 0,
            ram_enabled: false,
            ram_banking_mode: false,
        }
    }

    fn rom_bank_count(&self) -> usize {
        (self.rom.len() / ROM_BANK_SIZE).max(1)
    }

    fn lower_rom_bank(&self) -> usize {
        if self.ram_banking_mode {
            ((self.bank_high as usize) << 5) % self.rom_bank_count()
        } else {
            0
        }
    }

    fn upper_rom_bank(&self) -> usize {
        let bank = ((self.bank_high as usize) << 5) | self.rom_bank_low as usize;
        bank % self.rom_bank_count()
    }

    fn ram_bank(&self) -> usize {
        if self.ram_banking_mode {
            self.bank_high as usize
        } else {
            0
        }
    }

    fn read_rom_bank(&self, bank: usize, offset: usize) -> u8 {
        let index = bank
            .checked_mul(ROM_BANK_SIZE)
            .and_then(|start| start.checked_add(offset));
        index
            .and_then(|index| self.rom.get(index))
            .copied()
            .unwrap_or(0xff)
    }

    fn ram_index(&self, address: u16) -> Option<usize> {
        if !self.ram_enabled || !(0xa000..=0xbfff).contains(&address) {
            return None;
        }
        self.ram_bank()
            .checked_mul(RAM_BANK_SIZE)?
            .checked_add((address - 0xa000) as usize)
            .filter(|index| *index < self.ram.len())
    }
}

impl MemoryBankController for Mbc1 {
    fn read_rom(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3fff => self.read_rom_bank(self.lower_rom_bank(), address as usize),
            0x4000..=0x7fff => {
                self.read_rom_bank(self.upper_rom_bank(), (address - 0x4000) as usize)
            }
            _ => 0xff,
        }
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1fff => self.ram_enabled = value & 0x0f == 0x0a,
            0x2000..=0x3fff => {
                self.rom_bank_low = value & 0x1f;
                if self.rom_bank_low == 0 {
                    self.rom_bank_low = 1;
                }
            }
            0x4000..=0x5fff => self.bank_high = value & 0x03,
            0x6000..=0x7fff => self.ram_banking_mode = value & 0x01 != 0,
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

    const BANK_SIZE: usize = 16 * 1024;

    fn banked_rom(bank_count: usize) -> Vec<u8> {
        let mut rom = vec![0; bank_count * BANK_SIZE];
        for bank in 0..bank_count {
            rom[bank * BANK_SIZE..(bank + 1) * BANK_SIZE].fill(bank as u8);
        }
        rom
    }

    #[test]
    fn starts_with_bank_zero_and_bank_one() {
        let mbc = Mbc1::new(banked_rom(64), 0);

        assert_eq!(mbc.read_rom(0x0000), 0);
        assert_eq!(mbc.read_rom(0x3fff), 0);
        assert_eq!(mbc.read_rom(0x4000), 1);
        assert_eq!(mbc.read_rom(0x7fff), 1);
    }

    #[test]
    fn selects_low_five_rom_bank_bits() {
        let mut mbc = Mbc1::new(banked_rom(64), 0);

        mbc.write_rom(0x2000, 0x02);
        assert_eq!(mbc.read_rom(0x4000), 2);

        mbc.write_rom(0x2000, 0x1f);
        assert_eq!(mbc.read_rom(0x4000), 31);
    }

    #[test]
    fn remaps_forbidden_bank_numbers() {
        let mut mbc = Mbc1::new(banked_rom(64), 0);

        mbc.write_rom(0x2000, 0x00);
        assert_eq!(mbc.read_rom(0x4000), 1);

        mbc.write_rom(0x4000, 0x01);
        assert_eq!(mbc.read_rom(0x4000), 33);
    }

    #[test]
    fn combines_upper_bank_bits_in_rom_mode() {
        let mut mbc = Mbc1::new(banked_rom(128), 0);

        mbc.write_rom(0x2000, 0x03);
        mbc.write_rom(0x4000, 0x02);

        assert_eq!(mbc.read_rom(0x0000), 0);
        assert_eq!(mbc.read_rom(0x4000), 67);
    }

    #[test]
    fn applies_upper_bits_to_both_rom_regions_in_ram_mode() {
        let mut mbc = Mbc1::new(banked_rom(128), 0);
        mbc.write_rom(0x2000, 0x03);
        mbc.write_rom(0x4000, 0x02);
        mbc.write_rom(0x6000, 0x01);

        assert_eq!(mbc.read_rom(0x0000), 64);
        assert_eq!(mbc.read_rom(0x4000), 67);
    }

    #[test]
    fn keeps_ram_disabled_until_enable_value_is_written() {
        let mut mbc = Mbc1::new(banked_rom(4), 32 * 1024);

        mbc.write_ram(0xa000, 0x55);
        assert_eq!(mbc.read_ram(0xa000), 0xff);

        mbc.write_rom(0x0000, 0x0a);
        mbc.write_ram(0xa000, 0x55);
        assert_eq!(mbc.read_ram(0xa000), 0x55);

        mbc.write_rom(0x0000, 0x00);
        assert_eq!(mbc.read_ram(0xa000), 0xff);
    }

    #[test]
    fn isolates_ram_banks_in_ram_mode() {
        let mut mbc = Mbc1::new(banked_rom(4), 32 * 1024);
        mbc.write_rom(0x0000, 0x0a);
        mbc.write_rom(0x6000, 0x01);

        for bank in 0..4 {
            mbc.write_rom(0x4000, bank as u8);
            mbc.write_ram(0xa123, 0x10 + bank as u8);
        }

        for bank in 0..4 {
            mbc.write_rom(0x4000, bank as u8);
            assert_eq!(mbc.read_ram(0xa123), 0x10 + bank as u8);
        }
    }

    #[test]
    fn returns_ff_for_unavailable_ram_and_invalid_addresses() {
        let mut mbc = Mbc1::new(banked_rom(4), 0);
        mbc.write_rom(0x0000, 0x0a);

        assert_eq!(mbc.read_ram(0xa000), 0xff);
        assert_eq!(mbc.read_ram(0xc000), 0xff);
        assert_eq!(mbc.read_rom(0x8000), 0xff);
    }
}
