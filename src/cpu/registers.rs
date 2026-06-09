#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Flag {
    Zero = 0x80,
    Subtract = 0x40,
    HalfCarry = 0x20,
    Carry = 0x10,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl Registers {
    pub fn post_boot() -> Self {
        Self {
            a: 0x01,
            f: 0xb0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xd8,
            h: 0x01,
            l: 0x4d,
            sp: 0xfffe,
            pc: 0x0100,
        }
    }

    pub fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }

    pub fn set_af(&mut self, value: u16) {
        let [high, low] = value.to_be_bytes();
        self.a = high;
        self.set_f(low);
    }

    pub fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }

    pub fn set_bc(&mut self, value: u16) {
        [self.b, self.c] = value.to_be_bytes();
    }

    pub fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }

    pub fn set_de(&mut self, value: u16) {
        [self.d, self.e] = value.to_be_bytes();
    }

    pub fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }

    pub fn set_hl(&mut self, value: u16) {
        [self.h, self.l] = value.to_be_bytes();
    }

    pub fn set_f(&mut self, value: u8) {
        self.f = value & 0xf0;
    }

    pub fn flag(&self, flag: Flag) -> bool {
        self.f & flag as u8 != 0
    }

    pub fn set_flag(&mut self, flag: Flag, value: bool) {
        if value {
            self.f |= flag as u8;
        } else {
            self.f &= !(flag as u8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_register_pairs() {
        let mut registers = Registers::default();

        registers.set_af(0x12ff);
        registers.set_bc(0x3456);
        registers.set_de(0x789a);
        registers.set_hl(0xbcde);

        assert_eq!(registers.af(), 0x12f0);
        assert_eq!(registers.bc(), 0x3456);
        assert_eq!(registers.de(), 0x789a);
        assert_eq!(registers.hl(), 0xbcde);
    }

    #[test]
    fn keeps_lower_flag_nibble_clear() {
        let mut registers = Registers::default();

        registers.set_f(0xff);

        assert_eq!(registers.f, 0xf0);
    }

    #[test]
    fn sets_and_reads_individual_flags() {
        let mut registers = Registers::default();

        registers.set_flag(Flag::Zero, true);
        registers.set_flag(Flag::HalfCarry, true);

        assert!(registers.flag(Flag::Zero));
        assert!(!registers.flag(Flag::Subtract));
        assert!(registers.flag(Flag::HalfCarry));
        assert!(!registers.flag(Flag::Carry));

        registers.set_flag(Flag::Zero, false);
        assert!(!registers.flag(Flag::Zero));
    }
}
