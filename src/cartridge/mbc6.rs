use super::{
    CartridgeError, MemoryBankController, PersistentState, validate_persistent_length,
};

const ROM_BANK_SIZE: usize = 8 * 1024;
const RAM_BANK_SIZE: usize = 4 * 1024;
const FLASH_SIZE: usize = 1024 * 1024;
const FLASH_SECTOR_SIZE: usize = 128 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum FlashCommand {
    #[default]
    Idle,
    UnlockOne,
    UnlockTwo,
    Program,
    EraseUnlockOne,
    EraseUnlockTwo,
    EraseCommand,
}

pub(super) struct Mbc6 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    flash: Vec<u8>,
    ram_enabled: bool,
    ram_bank_a: u8,
    ram_bank_b: u8,
    flash_enabled: bool,
    flash_sector_zero_writable: bool,
    bank_a: u8,
    bank_b: u8,
    flash_a: bool,
    flash_b: bool,
    flash_command: FlashCommand,
}

impl Mbc6 {
    pub(super) fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size],
            flash: vec![0xff; FLASH_SIZE],
            ram_enabled: false,
            ram_bank_a: 0,
            ram_bank_b: 0,
            flash_enabled: false,
            flash_sector_zero_writable: false,
            bank_a: 0,
            bank_b: 0,
            flash_a: false,
            flash_b: false,
            flash_command: FlashCommand::Idle,
        }
    }

    fn read_bank(&self, bank: u8, offset: usize, flash_selected: bool) -> u8 {
        let index = bank as usize * ROM_BANK_SIZE + offset;
        if flash_selected {
            if !self.flash_enabled {
                return 0xff;
            }
            self.flash.get(index).copied().unwrap_or(0xff)
        } else {
            self.rom.get(index).copied().unwrap_or(0xff)
        }
    }

    fn flash_address(&self, address: u16) -> Option<usize> {
        match address {
            0x4000..=0x5fff if self.flash_a => {
                Some(self.bank_a as usize * ROM_BANK_SIZE + address as usize - 0x4000)
            }
            0x6000..=0x7fff if self.flash_b => {
                Some(self.bank_b as usize * ROM_BANK_SIZE + address as usize - 0x6000)
            }
            _ => None,
        }
    }

    fn flash_address_writable(&self, address: usize) -> bool {
        address >= FLASH_SECTOR_SIZE || self.flash_sector_zero_writable
    }

    fn write_flash(&mut self, address: usize, value: u8) {
        if !self.flash_enabled {
            return;
        }

        self.flash_command = match self.flash_command {
            FlashCommand::Idle if address == 0x5555 && value == 0xaa => FlashCommand::UnlockOne,
            FlashCommand::UnlockOne if address == 0x2aaa && value == 0x55 => {
                FlashCommand::UnlockTwo
            }
            FlashCommand::UnlockTwo if address == 0x5555 && value == 0xa0 => {
                FlashCommand::Program
            }
            FlashCommand::UnlockTwo if address == 0x5555 && value == 0x80 => {
                FlashCommand::EraseUnlockOne
            }
            FlashCommand::Program => {
                if self.flash_address_writable(address)
                    && let Some(byte) = self.flash.get_mut(address)
                {
                    *byte &= value;
                }
                FlashCommand::Idle
            }
            FlashCommand::EraseUnlockOne if address == 0x5555 && value == 0xaa => {
                FlashCommand::EraseUnlockTwo
            }
            FlashCommand::EraseUnlockTwo if address == 0x2aaa && value == 0x55 => {
                FlashCommand::EraseCommand
            }
            FlashCommand::EraseCommand if address == 0x5555 && value == 0x10 => {
                let writable_from = if self.flash_sector_zero_writable {
                    0
                } else {
                    FLASH_SECTOR_SIZE
                };
                self.flash[writable_from..].fill(0xff);
                FlashCommand::Idle
            }
            FlashCommand::EraseCommand if value == 0x30 => {
                let sector_start = address / FLASH_SECTOR_SIZE * FLASH_SECTOR_SIZE;
                if self.flash_address_writable(sector_start) {
                    let sector_end = (sector_start + FLASH_SECTOR_SIZE).min(self.flash.len());
                    self.flash[sector_start..sector_end].fill(0xff);
                }
                FlashCommand::Idle
            }
            _ => FlashCommand::Idle,
        };
    }

    fn ram_index(&self, address: u16) -> Option<usize> {
        if !self.ram_enabled {
            return None;
        }
        let (bank, offset) = match address {
            0xa000..=0xafff => (self.ram_bank_a, address as usize - 0xa000),
            0xb000..=0xbfff => (self.ram_bank_b, address as usize - 0xb000),
            _ => return None,
        };
        Some(bank as usize * RAM_BANK_SIZE + offset).filter(|index| *index < self.ram.len())
    }
}

impl MemoryBankController for Mbc6 {
    fn read_rom(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3fff => self.rom.get(address as usize).copied().unwrap_or(0xff),
            0x4000..=0x5fff => {
                self.read_bank(self.bank_a, address as usize - 0x4000, self.flash_a)
            }
            0x6000..=0x7fff => {
                self.read_bank(self.bank_b, address as usize - 0x6000, self.flash_b)
            }
            _ => 0xff,
        }
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        if let Some(flash_address) = self.flash_address(address) {
            self.write_flash(flash_address, value);
            return;
        }

        match address {
            0x0000..=0x03ff => self.ram_enabled = value & 0x0f == 0x0a,
            0x0400..=0x07ff => self.ram_bank_a = value & 0x07,
            0x0800..=0x0bff => self.ram_bank_b = value & 0x07,
            0x0c00..=0x0fff => self.flash_enabled = value & 0x01 != 0,
            0x1000 => self.flash_sector_zero_writable = value & 0x01 != 0,
            0x2000..=0x27ff => self.bank_a = value & 0x7f,
            0x2800..=0x2fff => self.flash_a = value & 0x08 != 0,
            0x3000..=0x37ff => self.bank_b = value & 0x7f,
            0x3800..=0x3fff => self.flash_b = value & 0x08 != 0,
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

    fn persistent_state(&self) -> PersistentState {
        PersistentState {
            ram: self.ram.clone(),
            flash: self.flash.clone(),
            ..PersistentState::default()
        }
    }

    fn load_persistent_state(
        &mut self,
        state: &PersistentState,
    ) -> Result<(), CartridgeError> {
        validate_persistent_length(self.ram.len(), state.ram.len(), "MBC6 RAM")?;
        validate_persistent_length(self.flash.len(), state.flash.len(), "MBC6 Flash")?;
        self.ram.copy_from_slice(&state.ram);
        self.flash.copy_from_slice(&state.flash);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::MemoryBankController;

    fn banked_rom() -> Vec<u8> {
        let mut rom = vec![0; 128 * 8 * 1024];
        for bank in 0..128 {
            rom[bank * 0x2000] = bank as u8;
        }
        rom
    }

    #[test]
    fn independently_selects_two_eight_kib_rom_banks() {
        let mut mbc = Mbc6::new(banked_rom(), 32 * 1024);
        mbc.write_rom(0x2000, 0x12);
        mbc.write_rom(0x3000, 0x34);

        assert_eq!(mbc.read_rom(0x4000), 0x12);
        assert_eq!(mbc.read_rom(0x6000), 0x34);
    }

    #[test]
    fn independently_selects_two_four_kib_ram_banks() {
        let mut mbc = Mbc6::new(banked_rom(), 32 * 1024);
        mbc.write_rom(0x0000, 0x0a);
        mbc.write_rom(0x0400, 0x03);
        mbc.write_rom(0x0800, 0x05);
        mbc.write_ram(0xa000, 0x33);
        mbc.write_ram(0xb000, 0x55);

        assert_eq!(mbc.read_ram(0xa000), 0x33);
        assert_eq!(mbc.read_ram(0xb000), 0x55);

        mbc.write_rom(0x0400, 0x05);
        assert_eq!(mbc.read_ram(0xa000), 0x55);
    }

    #[test]
    fn selects_flash_independently_for_each_rom_window() {
        let mut mbc = Mbc6::new(banked_rom(), 32 * 1024);
        mbc.write_rom(0x2000, 0x12);
        mbc.write_rom(0x3000, 0x34);
        mbc.write_rom(0x2800, 0x08);

        assert_eq!(mbc.read_rom(0x4000), 0xff);
        assert_eq!(mbc.read_rom(0x6000), 0x34);
    }
}
