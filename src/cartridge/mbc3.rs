use super::{
    CartridgeError, MemoryBankController, PersistentState, validate_persistent_length,
};

const ROM_BANK_SIZE: usize = 16 * 1024;
const RAM_BANK_SIZE: usize = 8 * 1024;
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;
const RTC_DAY_PERIOD: u64 = 512;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Rtc {
    seconds: u8,
    minutes: u8,
    hours: u8,
    days: u16,
    halted: bool,
    carry: bool,
}

impl Rtc {
    const PERSISTENT_SIZE: usize = 8;

    fn tick(&mut self, elapsed_seconds: u64) {
        if self.halted || elapsed_seconds == 0 {
            return;
        }

        let seconds_today =
            self.hours as u64 * 3600 + self.minutes as u64 * 60 + self.seconds as u64;
        let total_seconds = seconds_today + elapsed_seconds;
        let elapsed_days = total_seconds / SECONDS_PER_DAY;
        let seconds_today = total_seconds % SECONDS_PER_DAY;
        let total_days = self.days as u64 + elapsed_days;

        self.hours = (seconds_today / 3600) as u8;
        self.minutes = ((seconds_today / 60) % 60) as u8;
        self.seconds = (seconds_today % 60) as u8;
        if total_days >= RTC_DAY_PERIOD {
            self.carry = true;
        }
        self.days = (total_days % RTC_DAY_PERIOD) as u16;
    }

    fn read_register(self, register: u8) -> u8 {
        match register {
            0x08 => self.seconds,
            0x09 => self.minutes,
            0x0a => self.hours,
            0x0b => self.days as u8,
            0x0c => {
                ((self.days >> 8) as u8 & 0x01)
                    | ((self.halted as u8) << 6)
                    | ((self.carry as u8) << 7)
            }
            _ => 0xff,
        }
    }

    fn write_register(&mut self, register: u8, value: u8) {
        match register {
            0x08 => self.seconds = value % 60,
            0x09 => self.minutes = value % 60,
            0x0a => self.hours = value % 24,
            0x0b => self.days = (self.days & 0x0100) | value as u16,
            0x0c => {
                self.days = (self.days & 0x00ff) | (((value & 0x01) as u16) << 8);
                self.halted = value & 0x40 != 0;
                self.carry = value & 0x80 != 0;
            }
            _ => {}
        }
    }

    fn persistent_bytes(self) -> [u8; Self::PERSISTENT_SIZE] {
        let [day_low, day_high] = self.days.to_le_bytes();
        [
            self.seconds,
            self.minutes,
            self.hours,
            day_low,
            day_high,
            self.halted as u8,
            self.carry as u8,
            0,
        ]
    }

    fn from_persistent_bytes(bytes: &[u8]) -> Self {
        Self {
            seconds: bytes[0] % 60,
            minutes: bytes[1] % 60,
            hours: bytes[2] % 24,
            days: u16::from_le_bytes([bytes[3], bytes[4]]) & 0x01ff,
            halted: bytes[5] != 0,
            carry: bytes[6] != 0,
        }
    }
}

pub(super) struct Mbc3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank: u8,
    ram_rtc_selector: u8,
    ram_rtc_enabled: bool,
    has_rtc: bool,
    latch_value: u8,
    rtc: Rtc,
    latched_rtc: Rtc,
}

impl Mbc3 {
    pub(super) fn new(rom: Vec<u8>, ram_size: usize, has_rtc: bool) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size],
            rom_bank: 1,
            ram_rtc_selector: 0,
            ram_rtc_enabled: false,
            has_rtc,
            latch_value: 0xff,
            rtc: Rtc::default(),
            latched_rtc: Rtc::default(),
        }
    }

    pub(super) fn tick_rtc(&mut self, elapsed_seconds: u64) {
        if self.has_rtc {
            self.rtc.tick(elapsed_seconds);
        }
    }

    fn read_rom_bank(&self, bank: usize, offset: usize) -> u8 {
        let bank_count = (self.rom.len() / ROM_BANK_SIZE).max(1);
        let index = (bank % bank_count) * ROM_BANK_SIZE + offset;
        self.rom.get(index).copied().unwrap_or(0xff)
    }

    fn ram_index(&self, address: u16) -> Option<usize> {
        if !self.ram_rtc_enabled || self.ram_rtc_selector > 0x03 {
            return None;
        }
        let offset = (address as usize).checked_sub(0xa000)?;
        (offset < RAM_BANK_SIZE)
            .then_some(self.ram_rtc_selector as usize * RAM_BANK_SIZE + offset)
            .filter(|index| *index < self.ram.len())
    }
}

impl MemoryBankController for Mbc3 {
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
            0x0000..=0x1fff => self.ram_rtc_enabled = value & 0x0f == 0x0a,
            0x2000..=0x3fff => {
                self.rom_bank = value & 0x7f;
                if self.rom_bank == 0 {
                    self.rom_bank = 1;
                }
            }
            0x4000..=0x5fff => self.ram_rtc_selector = value,
            0x6000..=0x7fff => {
                if self.latch_value == 0 && value == 1 && self.has_rtc {
                    self.latched_rtc = self.rtc;
                }
                self.latch_value = value;
            }
            _ => {}
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_rtc_enabled || !(0xa000..=0xbfff).contains(&address) {
            return 0xff;
        }

        match self.ram_rtc_selector {
            0x00..=0x03 => self
                .ram_index(address)
                .and_then(|index| self.ram.get(index))
                .copied()
                .unwrap_or(0xff),
            0x08..=0x0c if self.has_rtc => {
                self.latched_rtc.read_register(self.ram_rtc_selector)
            }
            _ => 0xff,
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_rtc_enabled || !(0xa000..=0xbfff).contains(&address) {
            return;
        }

        match self.ram_rtc_selector {
            0x00..=0x03 => {
                if let Some(index) = self.ram_index(address)
                    && let Some(byte) = self.ram.get_mut(index)
                {
                    *byte = value;
                }
            }
            0x08..=0x0c if self.has_rtc => {
                self.rtc.write_register(self.ram_rtc_selector, value);
            }
            _ => {}
        }
    }

    fn persistent_state(&self) -> PersistentState {
        PersistentState {
            ram: self.ram.clone(),
            rtc: self
                .has_rtc
                .then(|| self.rtc.persistent_bytes().to_vec())
                .unwrap_or_default(),
            ..PersistentState::default()
        }
    }

    fn load_persistent_state(
        &mut self,
        state: &PersistentState,
    ) -> Result<(), CartridgeError> {
        validate_persistent_length(self.ram.len(), state.ram.len(), "RAM")?;
        let rtc_size = if self.has_rtc {
            Rtc::PERSISTENT_SIZE
        } else {
            0
        };
        validate_persistent_length(rtc_size, state.rtc.len(), "MBC3 RTC")?;

        self.ram.copy_from_slice(&state.ram);
        if self.has_rtc {
            self.rtc = Rtc::from_persistent_bytes(&state.rtc);
            self.latched_rtc = self.rtc;
        }
        Ok(())
    }

    fn tick_rtc(&mut self, elapsed_seconds: u64) {
        Mbc3::tick_rtc(self, elapsed_seconds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::MemoryBankController;

    fn banked_rom() -> Vec<u8> {
        let mut rom = vec![0; 8 * 16 * 1024];
        for bank in 0..8 {
            rom[bank * 0x4000..(bank + 1) * 0x4000].fill(bank as u8);
        }
        rom
    }

    #[test]
    fn switches_rom_and_ram_banks() {
        let mut mbc = Mbc3::new(banked_rom(), 32 * 1024, false);
        mbc.write_rom(0x0000, 0x0a);
        mbc.write_rom(0x2000, 0x03);
        assert_eq!(mbc.read_rom(0x4000), 3);

        mbc.write_rom(0x4000, 0x02);
        mbc.write_ram(0xa000, 0x22);
        mbc.write_rom(0x4000, 0x01);
        mbc.write_ram(0xa000, 0x11);
        mbc.write_rom(0x4000, 0x02);
        assert_eq!(mbc.read_ram(0xa000), 0x22);
    }

    #[test]
    fn latches_rtc_on_zero_to_one_transition() {
        let mut mbc = Mbc3::new(banked_rom(), 0, true);
        mbc.write_rom(0x0000, 0x0a);
        mbc.tick_rtc(61);
        mbc.write_rom(0x6000, 0x00);
        mbc.write_rom(0x6000, 0x01);

        mbc.write_rom(0x4000, 0x08);
        assert_eq!(mbc.read_ram(0xa000), 1);
        mbc.write_rom(0x4000, 0x09);
        assert_eq!(mbc.read_ram(0xa000), 1);
    }

    #[test]
    fn rtc_day_counter_wraps_and_sets_carry() {
        let mut mbc = Mbc3::new(banked_rom(), 0, true);
        mbc.write_rom(0x0000, 0x0a);
        mbc.tick_rtc(512 * 24 * 60 * 60);
        mbc.write_rom(0x6000, 0x00);
        mbc.write_rom(0x6000, 0x01);
        mbc.write_rom(0x4000, 0x0c);

        assert_eq!(mbc.read_ram(0xa000) & 0x81, 0x80);
    }
}
