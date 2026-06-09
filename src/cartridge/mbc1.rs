#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::MemoryBankController;

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
    fn maps_upper_bits_to_fixed_region_in_ram_mode() {
        let mut mbc = Mbc1::new(banked_rom(128), 0);
        mbc.write_rom(0x2000, 0x03);
        mbc.write_rom(0x4000, 0x02);
        mbc.write_rom(0x6000, 0x01);

        assert_eq!(mbc.read_rom(0x0000), 64);
        assert_eq!(mbc.read_rom(0x4000), 3);
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
