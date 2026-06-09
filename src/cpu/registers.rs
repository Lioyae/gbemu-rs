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
