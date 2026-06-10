use super::MemoryBankController;

const ROM_BANK_SIZE: usize = 16 * 1024;
const EEPROM_SIZE: usize = 256;
const SENSOR_CENTER: u16 = 0x8000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum EepromState {
    #[default]
    Idle,
    Command,
    ReadOut,
    Write,
    WriteAll,
}

pub(super) struct Mbc7 {
    rom: Vec<u8>,
    eeprom: [u8; EEPROM_SIZE],
    rom_bank: u8,
    access: u8,
    latch_armed: bool,
    sensor_x: u16,
    sensor_y: u16,
    pins: u8,
    eeprom_state: EepromState,
    shift_register: u16,
    shift_bits: u8,
    eeprom_address: u8,
    eeprom_writable: bool,
}

impl Mbc7 {
    pub(super) fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            eeprom: [0xff; EEPROM_SIZE],
            rom_bank: 1,
            access: 0,
            latch_armed: false,
            sensor_x: SENSOR_CENTER,
            sensor_y: SENSOR_CENTER,
            pins: 0,
            eeprom_state: EepromState::Idle,
            shift_register: 0,
            shift_bits: 0,
            eeprom_address: 0,
            eeprom_writable: false,
        }
    }

    pub(super) fn eeprom(&self) -> &[u8] {
        &self.eeprom
    }

    fn registers_enabled(&self) -> bool {
        self.access == 0x03
    }

    fn read_rom_bank(&self, bank: usize, offset: usize) -> u8 {
        let bank_count = (self.rom.len() / ROM_BANK_SIZE).max(1);
        self.rom
            .get(bank % bank_count * ROM_BANK_SIZE + offset)
            .copied()
            .unwrap_or(0xff)
    }

    fn shift_input_bit(&mut self, bit: bool) {
        self.shift_register = (self.shift_register << 1) | bit as u16;
        self.shift_bits += 1;
    }

    fn decode_eeprom_command(&mut self) {
        self.eeprom_address = (self.shift_register & 0x7f) as u8;
        self.shift_bits = 0;
        match (self.shift_register >> 6) & 0x0f {
            0x0 => {
                self.eeprom_writable = false;
                self.eeprom_state = EepromState::Idle;
            }
            0x1 => {
                self.shift_register = 0;
                self.eeprom_state = EepromState::WriteAll;
            }
            0x2 => {
                if self.eeprom_writable {
                    self.eeprom.fill(0xff);
                }
                self.eeprom_state = EepromState::Idle;
            }
            0x3 => {
                self.eeprom_writable = true;
                self.eeprom_state = EepromState::Idle;
            }
            0x4..=0x7 => {
                self.shift_register = 0;
                self.eeprom_state = EepromState::Write;
            }
            0x8..=0x0b => {
                let index = self.eeprom_address as usize * 2;
                self.shift_register =
                    u16::from_be_bytes([self.eeprom[index], self.eeprom[index + 1]]);
                self.shift_bits = 16;
                self.eeprom_state = EepromState::ReadOut;
                self.pins &= !0x01;
            }
            0x0c..=0x0f => {
                if self.eeprom_writable {
                    let index = self.eeprom_address as usize * 2;
                    self.eeprom[index..index + 2].fill(0xff);
                }
                self.eeprom_state = EepromState::Idle;
            }
            _ => unreachable!(),
        }
    }

    fn finish_eeprom_data(&mut self) {
        if self.shift_bits != 16 {
            return;
        }
        if self.eeprom_writable {
            let bytes = self.shift_register.to_be_bytes();
            match self.eeprom_state {
                EepromState::Write => {
                    let index = self.eeprom_address as usize * 2;
                    self.eeprom[index..index + 2].copy_from_slice(&bytes);
                }
                EepromState::WriteAll => {
                    for word in self.eeprom.chunks_exact_mut(2) {
                        word.copy_from_slice(&bytes);
                    }
                }
                _ => {}
            }
        }
        self.eeprom_state = EepromState::Idle;
        self.shift_bits = 0;
    }

    fn write_eeprom_pins(&mut self, value: u8) {
        let old_pins = self.pins;
        self.pins = value | 0x01;
        let chip_select = value & 0x80 != 0;
        let old_chip_select = old_pins & 0x80 != 0;
        let rising_clock = old_pins & 0x40 == 0 && value & 0x40 != 0;

        if !old_chip_select && chip_select {
            self.eeprom_state = EepromState::Idle;
            self.shift_bits = 0;
            self.shift_register = 0;
        }
        if !chip_select || !rising_clock {
            return;
        }

        let input_bit = value & 0x02 != 0;
        match self.eeprom_state {
            EepromState::Idle => {
                if input_bit {
                    self.eeprom_state = EepromState::Command;
                    self.shift_bits = 0;
                    self.shift_register = 0;
                }
            }
            EepromState::Command => {
                self.shift_input_bit(input_bit);
                if self.shift_bits == 10 {
                    self.decode_eeprom_command();
                }
            }
            EepromState::ReadOut => {
                let output_bit = (self.shift_register >> 15) as u8;
                self.pins = (self.pins & !0x01) | output_bit;
                self.shift_register <<= 1;
                self.shift_bits -= 1;
                if self.shift_bits == 0 {
                    self.eeprom_state = EepromState::Idle;
                }
            }
            EepromState::Write | EepromState::WriteAll => {
                self.shift_input_bit(input_bit);
                self.finish_eeprom_data();
            }
        }
    }
}

impl MemoryBankController for Mbc7 {
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
            0x0000..=0x1fff => {
                if value == 0x0a {
                    self.access |= 0x01;
                } else {
                    self.access = 0;
                }
            }
            0x2000..=0x3fff => self.rom_bank = value & 0x7f,
            0x4000..=0x5fff => {
                if value == 0x40 {
                    self.access |= 0x02;
                } else {
                    self.access &= !0x02;
                }
            }
            _ => {}
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.registers_enabled() || !(0xa000..=0xafff).contains(&address) {
            return 0xff;
        }
        match address & 0x00f0 {
            0x0020 => self.sensor_x as u8,
            0x0030 => (self.sensor_x >> 8) as u8,
            0x0040 => self.sensor_y as u8,
            0x0050 => (self.sensor_y >> 8) as u8,
            0x0060 => 0x00,
            0x0070 => 0xff,
            0x0080 => self.pins,
            _ => 0xff,
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !self.registers_enabled() || !(0xa000..=0xafff).contains(&address) {
            return;
        }
        match address & 0x00f0 {
            0x0000 => {
                self.latch_armed = value == 0x55;
                if self.latch_armed {
                    self.sensor_x = SENSOR_CENTER;
                    self.sensor_y = SENSOR_CENTER;
                }
            }
            0x0010 if value == 0xaa && self.latch_armed => {
                self.sensor_x = SENSOR_CENTER;
                self.sensor_y = SENSOR_CENTER;
                self.latch_armed = false;
            }
            0x0080 => self.write_eeprom_pins(value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::MemoryBankController;

    const EEPROM_REGISTER: u16 = 0xa080;

    fn banked_rom() -> Vec<u8> {
        let mut rom = vec![0; 128 * 16 * 1024];
        for bank in 0..128 {
            rom[bank * 0x4000] = bank as u8;
        }
        rom
    }

    fn enable_registers(mbc: &mut Mbc7) {
        mbc.write_rom(0x0000, 0x0a);
        mbc.write_rom(0x4000, 0x40);
    }

    fn clock_bit(mbc: &mut Mbc7, bit: bool) {
        let low = 0x80 | ((bit as u8) << 1);
        mbc.write_ram(EEPROM_REGISTER, low);
        mbc.write_ram(EEPROM_REGISTER, low | 0x40);
    }

    fn begin_command(mbc: &mut Mbc7, command: u16) {
        mbc.write_ram(EEPROM_REGISTER, 0x00);
        mbc.write_ram(EEPROM_REGISTER, 0x80);
        clock_bit(mbc, true);
        for shift in (0..10).rev() {
            clock_bit(mbc, command & (1 << shift) != 0);
        }
    }

    #[test]
    fn switches_rom_bank_and_requires_both_register_enables() {
        let mut mbc = Mbc7::new(banked_rom());
        mbc.write_rom(0x2000, 0x22);
        assert_eq!(mbc.read_rom(0x4000), 0x22);
        assert_eq!(mbc.read_ram(0xa020), 0xff);

        enable_registers(&mut mbc);
        assert_eq!(mbc.read_ram(0xa020), 0x00);
        assert_eq!(mbc.read_ram(0xa030), 0x80);
    }

    #[test]
    fn latches_stable_centered_accelerometer_values() {
        let mut mbc = Mbc7::new(banked_rom());
        enable_registers(&mut mbc);

        mbc.write_ram(0xa000, 0x55);
        mbc.write_ram(0xa010, 0xaa);

        assert_eq!(mbc.read_ram(0xa020), 0x00);
        assert_eq!(mbc.read_ram(0xa030), 0x80);
        assert_eq!(mbc.read_ram(0xa040), 0x00);
        assert_eq!(mbc.read_ram(0xa050), 0x80);
    }

    #[test]
    fn writes_eeprom_word_after_serial_write_enable() {
        let mut mbc = Mbc7::new(banked_rom());
        enable_registers(&mut mbc);

        begin_command(&mut mbc, 0b0011_000000);
        begin_command(&mut mbc, 0b010_0000001);
        for shift in (0..16).rev() {
            clock_bit(&mut mbc, 0xbeef & (1 << shift) != 0);
        }

        assert_eq!(&mbc.eeprom()[2..4], &[0xbe, 0xef]);
    }
}
