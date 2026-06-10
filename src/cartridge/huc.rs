use super::{
    CartridgeError, MemoryBankController, PersistentState, load_persistent_bytes,
    validate_persistent_length,
};

const ROM_BANK_SIZE: usize = 16 * 1024;
const RAM_BANK_SIZE: usize = 8 * 1024;

pub(super) struct Huc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank: u8,
    ram_bank: u8,
    infrared_mode: bool,
    infrared_transmitting: bool,
}

impl Huc1 {
    pub(super) fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            infrared_mode: false,
            infrared_transmitting: false,
        }
    }

    #[cfg(test)]
    pub(super) fn infrared_transmitting(&self) -> bool {
        self.infrared_transmitting
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
}

impl MemoryBankController for Huc1 {
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
            0x0000..=0x1fff => self.infrared_mode = value == 0x0e,
            0x2000..=0x3fff => self.rom_bank = value & 0x7f,
            0x4000..=0x5fff => self.ram_bank = value & 0x03,
            _ => {}
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        if self.infrared_mode {
            return if (0xa000..=0xbfff).contains(&address) {
                0xc0
            } else {
                0xff
            };
        }
        self.ram_index(address)
            .and_then(|index| self.ram.get(index))
            .copied()
            .unwrap_or(0xff)
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if self.infrared_mode {
            if (0xa000..=0xbfff).contains(&address) {
                self.infrared_transmitting = value & 0x01 != 0;
            }
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
        load_persistent_bytes(&mut self.ram, &state.ram, "HuC1 RAM")
    }
}

pub(super) struct Huc3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank: u8,
    ram_bank: u8,
    mode: u8,
    index: u8,
    mailbox: u8,
    registers: [u8; 256],
    infrared_transmitting: bool,
    subminute_seconds: u64,
}

impl Huc3 {
    pub(super) fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            mode: 0,
            index: 0,
            mailbox: 0,
            registers: [0; 256],
            infrared_transmitting: false,
            subminute_seconds: 0,
        }
    }

    #[cfg(test)]
    pub(super) fn register(&self, index: usize) -> u8 {
        self.registers.get(index).copied().unwrap_or(0xff)
    }

    pub(super) fn tick_rtc(&mut self, elapsed_seconds: u64) {
        let total_seconds = self.subminute_seconds + elapsed_seconds;
        let elapsed_minutes = total_seconds / 60;
        self.subminute_seconds = total_seconds % 60;
        if elapsed_minutes == 0 {
            return;
        }

        let minutes = self.registers[0x10] as u64
            | ((self.registers[0x11] as u64) << 4)
            | ((self.registers[0x12] as u64) << 8);
        let days = self.registers[0x13] as u64
            | ((self.registers[0x14] as u64) << 4)
            | ((self.registers[0x15] as u64) << 8);
        let total_minutes = minutes + elapsed_minutes;
        let new_minutes = total_minutes % 1440;
        let new_days = (days + total_minutes / 1440) & 0x0fff;
        Self::write_nibbles(&mut self.registers[0x10..=0x12], new_minutes);
        Self::write_nibbles(&mut self.registers[0x13..=0x15], new_days);
    }

    fn write_nibbles(target: &mut [u8], value: u64) {
        for (index, nibble) in target.iter_mut().enumerate() {
            *nibble = ((value >> (index * 4)) & 0x0f) as u8;
        }
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

    fn commit_mailbox(&mut self) {
        match self.mailbox & 0x70 {
            0x10 => {
                self.mailbox = (self.mailbox & 0xf0) | self.registers[self.index as usize] & 0x0f;
                self.index = self.index.wrapping_add(1);
            }
            0x30 => {
                self.registers[self.index as usize] = self.mailbox & 0x0f;
                self.index = self.index.wrapping_add(1);
            }
            0x40 => self.index = (self.index & 0xf0) | self.mailbox & 0x0f,
            0x50 => self.index = (self.index & 0x0f) | ((self.mailbox & 0x0f) << 4),
            0x60 => {
                match self.mailbox & 0x0f {
                    0x0 => self.registers.copy_within(0x10..0x16, 0),
                    0x1 => self.registers.copy_within(0..6, 0x10),
                    _ => {}
                }
                self.mailbox = 0xe1;
            }
            _ => {}
        }
    }
}

impl MemoryBankController for Huc3 {
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
            0x0000..=0x1fff => self.mode = value & 0x0f,
            0x2000..=0x3fff => self.rom_bank = value & 0x7f,
            0x4000..=0x5fff => self.ram_bank = value & 0x03,
            _ => {}
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !(0xa000..=0xbfff).contains(&address) {
            return 0xff;
        }
        match self.mode {
            0x00 | 0x0a => self
                .ram_index(address)
                .and_then(|index| self.ram.get(index))
                .copied()
                .unwrap_or(0xff),
            0x0b | 0x0c => 0x80 | self.mailbox,
            0x0d => 0x81,
            0x0e => 0xc0,
            _ => 0xff,
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !(0xa000..=0xbfff).contains(&address) {
            return;
        }
        match self.mode {
            0x0a => {
                if let Some(index) = self.ram_index(address)
                    && let Some(byte) = self.ram.get_mut(index)
                {
                    *byte = value;
                }
            }
            0x0b => self.mailbox = 0x80 | (value & 0x7f),
            0x0d if value & 0x01 == 0 => self.commit_mailbox(),
            0x0e => self.infrared_transmitting = value & 0x01 != 0,
            _ => {}
        }
    }

    fn persistent_state(&self) -> PersistentState {
        let mut rtc = self.registers.to_vec();
        rtc.extend_from_slice(&self.subminute_seconds.to_le_bytes());
        PersistentState {
            ram: self.ram.clone(),
            rtc,
            ..PersistentState::default()
        }
    }

    fn load_persistent_state(
        &mut self,
        state: &PersistentState,
    ) -> Result<(), CartridgeError> {
        const RTC_SIZE: usize = 256 + 8;
        validate_persistent_length(self.ram.len(), state.ram.len(), "HuC3 RAM")?;
        validate_persistent_length(RTC_SIZE, state.rtc.len(), "HuC3 RTC")?;

        self.ram.copy_from_slice(&state.ram);
        self.registers.copy_from_slice(&state.rtc[..256]);
        self.subminute_seconds =
            u64::from_le_bytes(state.rtc[256..264].try_into().expect("RTC 长度已校验"));
        Ok(())
    }

    fn tick_rtc(&mut self, elapsed_seconds: u64) {
        Huc3::tick_rtc(self, elapsed_seconds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::MemoryBankController;

    fn banked_rom() -> Vec<u8> {
        let mut rom = vec![0; 128 * 16 * 1024];
        for bank in 0..128 {
            rom[bank * 0x4000] = bank as u8;
        }
        rom
    }

    #[test]
    fn huc1_banks_rom_and_ram_without_ram_enable() {
        let mut huc = Huc1::new(banked_rom(), 32 * 1024);
        huc.write_rom(0x2000, 0x22);
        huc.write_rom(0x4000, 0x02);
        huc.write_ram(0xa000, 0x22);
        huc.write_rom(0x4000, 0x01);
        huc.write_ram(0xa000, 0x11);

        assert_eq!(huc.read_rom(0x4000), 0x22);
        assert_eq!(huc.read_ram(0xa000), 0x11);
    }

    #[test]
    fn huc1_infrared_defaults_to_no_received_signal() {
        let mut huc = Huc1::new(banked_rom(), 32 * 1024);
        huc.write_rom(0x0000, 0x0e);
        huc.write_ram(0xa000, 0x01);

        assert!(huc.infrared_transmitting());
        assert_eq!(huc.read_ram(0xa000), 0xc0);
    }

    fn huc3_command(huc: &mut Huc3, command: u8) {
        huc.write_rom(0x0000, 0x0b);
        huc.write_ram(0xa000, command);
        huc.write_rom(0x0000, 0x0d);
        huc.write_ram(0xa000, 0xfe);
    }

    #[test]
    fn huc3_mailbox_writes_and_reads_nibbles() {
        let mut huc = Huc3::new(banked_rom(), 32 * 1024);
        huc3_command(&mut huc, 0x42);
        huc3_command(&mut huc, 0x51);
        huc3_command(&mut huc, 0x3a);

        huc3_command(&mut huc, 0x42);
        huc3_command(&mut huc, 0x51);
        huc3_command(&mut huc, 0x10);
        huc.write_rom(0x0000, 0x0c);

        assert_eq!(huc.read_ram(0xa000), 0x9a);
    }

    #[test]
    fn huc3_advances_rtc_minutes_and_days() {
        let mut huc = Huc3::new(banked_rom(), 32 * 1024);
        huc.tick_rtc(1441 * 60);

        assert_eq!(huc.register(0x10), 1);
        assert_eq!(huc.register(0x13), 1);
    }
}
