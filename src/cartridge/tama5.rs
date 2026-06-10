use super::{
    CartridgeError, MemoryBankController, PersistentState, validate_persistent_length,
};

const ROM_BANK_SIZE: usize = 16 * 1024;
const REGISTER_COUNT: usize = 8;
const EEPROM_SIZE: usize = 32;
const RTC_PAGE_SIZE: usize = 16;

pub(super) struct Tama5 {
    rom: Vec<u8>,
    eeprom: [u8; EEPROM_SIZE],
    selected_register: u8,
    registers: [u8; REGISTER_COUNT],
    rtc_timer: [u8; RTC_PAGE_SIZE],
    rtc_alarm: [u8; RTC_PAGE_SIZE],
    rtc_free_zero: [u8; RTC_PAGE_SIZE],
    rtc_free_one: [u8; RTC_PAGE_SIZE],
    timer_disabled: bool,
}

impl Tama5 {
    pub(super) fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            eeprom: [0; EEPROM_SIZE],
            selected_register: 0,
            registers: [0; REGISTER_COUNT],
            rtc_timer: [0; RTC_PAGE_SIZE],
            rtc_alarm: [0; RTC_PAGE_SIZE],
            rtc_free_zero: [0; RTC_PAGE_SIZE],
            rtc_free_one: [0; RTC_PAGE_SIZE],
            timer_disabled: false,
        }
    }

    pub(super) fn tick_rtc(&mut self, elapsed_seconds: u64) {
        if self.timer_disabled {
            return;
        }
        let current_seconds = self.rtc_timer[0] as u64
            + self.rtc_timer[1] as u64 * 10
            + (self.rtc_timer[2] as u64 + self.rtc_timer[3] as u64 * 10) * 60
            + (self.rtc_timer[4] as u64 + self.rtc_timer[5] as u64 * 10) * 3600;
        let total = current_seconds + elapsed_seconds;
        let seconds = total % 60;
        let minutes = total / 60 % 60;
        let hours = total / 3600 % 24;
        self.rtc_timer[0] = (seconds % 10) as u8;
        self.rtc_timer[1] = (seconds / 10) as u8;
        self.rtc_timer[2] = (minutes % 10) as u8;
        self.rtc_timer[3] = (minutes / 10) as u8;
        self.rtc_timer[4] = (hours % 10) as u8;
        self.rtc_timer[5] = (hours / 10) as u8;
    }

    fn rom_bank(&self) -> usize {
        self.registers[0] as usize | ((self.registers[1] as usize) << 4)
    }

    fn data_address(&self) -> usize {
        (((self.registers[6] << 4) & 0x10) | self.registers[7]) as usize
    }

    fn output_byte(&self) -> u8 {
        (self.registers[5] << 4) | self.registers[4]
    }

    fn execute_address_write(&mut self) {
        let address = self.data_address();
        let output = self.output_byte();
        match self.registers[6] >> 1 {
            0x0 => self.eeprom[address] = output,
            0x1 => {}
            0x2 => match address {
                0x00 => self.timer_disabled = true,
                0x01 => {
                    self.timer_disabled = false;
                    self.rtc_timer[0] = 0;
                    self.rtc_timer[1] = 0;
                }
                0x04 => {
                    self.rtc_timer[2] = output & 0x0f;
                    self.rtc_timer[3] = output >> 4;
                }
                0x05 => {
                    self.rtc_timer[4] = output & 0x0f;
                    self.rtc_timer[5] = output >> 4;
                }
                _ => {}
            },
            0x4 => {
                let rtc_address = self.registers[4] as usize;
                if rtc_address >= RTC_PAGE_SIZE {
                    return;
                }
                let value = self.registers[5] & 0x0f;
                match self.registers[7] {
                    0 => self.rtc_timer[rtc_address] = value,
                    2 => self.rtc_alarm[rtc_address] = value,
                    4 => self.rtc_free_zero[rtc_address] = value,
                    6 => self.rtc_free_one[rtc_address] = value,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn read_selected_data(&self) -> u8 {
        let mut value = match self.registers[6] >> 1 {
            0x1 => self.eeprom[self.data_address()],
            0x2 => match self.data_address() {
                0x06 => (self.rtc_timer[3] << 4) | self.rtc_timer[2],
                0x07 => (self.rtc_timer[5] << 4) | self.rtc_timer[4],
                address => address as u8,
            },
            0x4 => {
                let address = self.registers[4] as usize;
                if address >= RTC_PAGE_SIZE {
                    0
                } else {
                    match self.registers[7] {
                        1 => self.rtc_timer[address],
                        3 => self.rtc_alarm[address],
                        5 => self.rtc_free_zero[address],
                        7 => self.rtc_free_one[address],
                        _ => 0,
                    }
                }
            }
            _ => 0,
        };
        if self.selected_register == 0x0d {
            value >>= 4;
        }
        0xf0 | (value & 0x0f)
    }
}

impl MemoryBankController for Tama5 {
    fn read_rom(&self, address: u16) -> u8 {
        let bank_count = (self.rom.len() / ROM_BANK_SIZE).max(1);
        let index = match address {
            0x0000..=0x3fff => address as usize,
            0x4000..=0x7fff => {
                self.rom_bank() % bank_count * ROM_BANK_SIZE + address as usize - 0x4000
            }
            _ => return 0xff,
        };
        self.rom.get(index).copied().unwrap_or(0xff)
    }

    fn write_rom(&mut self, _address: u16, _value: u8) {}

    fn read_ram(&self, address: u16) -> u8 {
        if !(0xa000..=0xbfff).contains(&address) {
            return 0xff;
        }
        if address & 1 != 0 {
            return 0xff;
        }
        match self.selected_register {
            0x0a => 0xf1,
            0x0c | 0x0d => self.read_selected_data(),
            _ => 0xf1,
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !(0xa000..=0xbfff).contains(&address) {
            return;
        }
        if address & 1 != 0 {
            self.selected_register = value;
            return;
        }
        let register = self.selected_register as usize;
        if register >= self.registers.len() {
            return;
        }
        self.registers[register] = value & 0x0f;
        if register == 7 {
            self.execute_address_write();
        }
    }

    fn persistent_state(&self) -> PersistentState {
        let mut rtc = Vec::with_capacity(RTC_PAGE_SIZE * 4 + 1);
        rtc.extend_from_slice(&self.rtc_timer);
        rtc.extend_from_slice(&self.rtc_alarm);
        rtc.extend_from_slice(&self.rtc_free_zero);
        rtc.extend_from_slice(&self.rtc_free_one);
        rtc.push(self.timer_disabled as u8);
        PersistentState {
            ram: self.eeprom.to_vec(),
            rtc,
            ..PersistentState::default()
        }
    }

    fn load_persistent_state(
        &mut self,
        state: &PersistentState,
    ) -> Result<(), CartridgeError> {
        const RTC_SIZE: usize = RTC_PAGE_SIZE * 4 + 1;
        validate_persistent_length(self.eeprom.len(), state.ram.len(), "TAMA5 EEPROM")?;
        validate_persistent_length(RTC_SIZE, state.rtc.len(), "TAMA5 RTC")?;

        self.eeprom.copy_from_slice(&state.ram);
        self.rtc_timer.copy_from_slice(&state.rtc[0..16]);
        self.rtc_alarm.copy_from_slice(&state.rtc[16..32]);
        self.rtc_free_zero.copy_from_slice(&state.rtc[32..48]);
        self.rtc_free_one.copy_from_slice(&state.rtc[48..64]);
        self.timer_disabled = state.rtc[64] != 0;
        Ok(())
    }

    fn tick_rtc(&mut self, elapsed_seconds: u64) {
        Tama5::tick_rtc(self, elapsed_seconds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::MemoryBankController;

    fn banked_rom() -> Vec<u8> {
        let mut rom = vec![0; 32 * 16 * 1024];
        for bank in 0..32 {
            rom[bank * 0x4000] = bank as u8;
        }
        rom
    }

    fn write_register(tama: &mut Tama5, register: u8, value: u8) {
        tama.write_ram(0xa001, register);
        tama.write_ram(0xa000, value);
    }

    #[test]
    fn combines_low_and_high_nibbles_for_rom_bank() {
        let mut tama = Tama5::new(banked_rom());
        write_register(&mut tama, 0x00, 0x02);
        write_register(&mut tama, 0x01, 0x01);

        assert_eq!(tama.read_rom(0x4000), 0x12);
    }

    #[test]
    fn writes_and_reads_internal_eeprom_through_nibble_protocol() {
        let mut tama = Tama5::new(banked_rom());
        write_register(&mut tama, 0x04, 0x0b);
        write_register(&mut tama, 0x05, 0x0a);
        write_register(&mut tama, 0x06, 0x00);
        write_register(&mut tama, 0x07, 0x03);

        write_register(&mut tama, 0x06, 0x02);
        write_register(&mut tama, 0x07, 0x03);
        tama.write_ram(0xa001, 0x0c);
        assert_eq!(tama.read_ram(0xa000), 0xfb);
        tama.write_ram(0xa001, 0x0d);
        assert_eq!(tama.read_ram(0xa000), 0xfa);
    }

    #[test]
    fn reports_active_status_on_register_ten() {
        let mut tama = Tama5::new(banked_rom());
        tama.write_ram(0xa001, 0x0a);

        assert_eq!(tama.read_ram(0xa000), 0xf1);
        assert_eq!(tama.read_ram(0xa001), 0xff);
    }

    #[test]
    fn advances_rtc_and_reads_bcd_minute() {
        let mut tama = Tama5::new(banked_rom());
        tama.tick_rtc(61);
        write_register(&mut tama, 0x06, 0x04);
        write_register(&mut tama, 0x07, 0x06);

        tama.write_ram(0xa001, 0x0c);
        assert_eq!(tama.read_ram(0xa000), 0xf1);
        tama.write_ram(0xa001, 0x0d);
        assert_eq!(tama.read_ram(0xa000), 0xf0);
    }
}
