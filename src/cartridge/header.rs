#[cfg(test)]
mod tests {
    use super::*;

    const HEADER_END: usize = 0x150;

    fn rom_with_header(
        title: &str,
        cartridge_type: u8,
        rom_size_code: u8,
        ram_size_code: u8,
    ) -> Vec<u8> {
        let declared_size = match rom_size_code {
            0x00 => 32 * 1024,
            0x01 => 64 * 1024,
            0x02 => 128 * 1024,
            0x03 => 256 * 1024,
            0x04 => 512 * 1024,
            0x05 => 1024 * 1024,
            0x06 => 2 * 1024 * 1024,
            _ => HEADER_END,
        };
        let mut rom = vec![0; declared_size.max(HEADER_END)];
        let title_bytes = title.as_bytes();
        let title_length = title_bytes.len().min(16);
        rom[0x134..0x134 + title_length].copy_from_slice(&title_bytes[..title_length]);
        rom[0x147] = cartridge_type;
        rom[0x148] = rom_size_code;
        rom[0x149] = ram_size_code;
        rom
    }

    #[test]
    fn parses_rom_only_header() {
        let rom = rom_with_header("TEST", 0x00, 0x00, 0x00);

        let header = CartridgeHeader::parse(&rom).expect("有效卡带头应解析成功");

        assert_eq!(header.title(), "TEST");
        assert_eq!(header.cartridge_type(), CartridgeType::RomOnly);
        assert_eq!(header.rom_size(), 32 * 1024);
        assert_eq!(header.ram_size(), 0);
    }

    #[test]
    fn parses_all_supported_mbc1_types() {
        let cases = [
            (0x01, CartridgeType::Mbc1),
            (0x02, CartridgeType::Mbc1Ram),
            (0x03, CartridgeType::Mbc1RamBattery),
        ];

        for (code, expected) in cases {
            let rom = rom_with_header("MBC1", code, 0x01, 0x03);
            let header = CartridgeHeader::parse(&rom).expect("MBC1 卡带头应解析成功");
            assert_eq!(header.cartridge_type(), expected);
            assert_eq!(header.rom_size(), 64 * 1024);
            assert_eq!(header.ram_size(), 32 * 1024);
        }
    }

    #[test]
    fn stops_title_at_first_nul_byte() {
        let mut rom = rom_with_header("TITLE", 0x00, 0x00, 0x00);
        rom[0x139] = 0;
        rom[0x13a..0x13d].copy_from_slice(b"BAD");

        let header = CartridgeHeader::parse(&rom).expect("有效卡带头应解析成功");

        assert_eq!(header.title(), "TITLE");
    }

    #[test]
    fn rejects_rom_smaller_than_header() {
        let error = CartridgeHeader::parse(&vec![0; HEADER_END - 1])
            .expect_err("过小 ROM 必须被拒绝");

        assert!(matches!(
            error,
            CartridgeError::RomTooSmall {
                actual: HEADER_END - 1,
                minimum: HEADER_END
            }
        ));
    }

    #[test]
    fn rejects_unknown_cartridge_type() {
        let rom = rom_with_header("BADTYPE", 0x19, 0x00, 0x00);

        let error = CartridgeHeader::parse(&rom).expect_err("未知卡带类型必须被拒绝");

        assert!(matches!(
            error,
            CartridgeError::UnsupportedCartridgeType(0x19)
        ));
    }

    #[test]
    fn rejects_unknown_rom_size_code() {
        let rom = rom_with_header("BADROM", 0x00, 0x7f, 0x00);

        let error = CartridgeHeader::parse(&rom).expect_err("未知 ROM 容量编码必须被拒绝");

        assert!(matches!(error, CartridgeError::UnsupportedRomSize(0x7f)));
    }

    #[test]
    fn rejects_unknown_ram_size_code() {
        let rom = rom_with_header("BADRAM", 0x00, 0x00, 0x7f);

        let error = CartridgeHeader::parse(&rom).expect_err("未知 RAM 容量编码必须被拒绝");

        assert!(matches!(error, CartridgeError::UnsupportedRamSize(0x7f)));
    }

    #[test]
    fn rejects_rom_shorter_than_declared_size() {
        let mut rom = rom_with_header("TRUNCATED", 0x00, 0x02, 0x00);
        rom.truncate(32 * 1024);

        let error = CartridgeHeader::parse(&rom).expect_err("截断 ROM 必须被拒绝");

        assert!(matches!(
            error,
            CartridgeError::RomLengthMismatch {
                actual: 32 * 1024,
                declared: 128 * 1024
            }
        ));
    }
}
