use std::fmt;

use thiserror::Error;

const HEADER_LENGTH: usize = 0x150;
const TITLE_START: usize = 0x134;
const TITLE_END: usize = 0x144;
const CARTRIDGE_TYPE_ADDRESS: usize = 0x147;
const ROM_SIZE_ADDRESS: usize = 0x148;
const RAM_SIZE_ADDRESS: usize = 0x149;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeType {
    RomOnly,
    Mbc1,
    Mbc1Ram,
    Mbc1RamBattery,
}

impl fmt::Display for CartridgeType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::RomOnly => "ROM-only",
            Self::Mbc1 => "MBC1",
            Self::Mbc1Ram => "MBC1+RAM",
            Self::Mbc1RamBattery => "MBC1+RAM+BATTERY",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeHeader {
    title: String,
    cartridge_type: CartridgeType,
    rom_size: usize,
    ram_size: usize,
}

impl CartridgeHeader {
    pub fn parse(rom: &[u8]) -> Result<Self, CartridgeError> {
        if rom.len() < HEADER_LENGTH {
            return Err(CartridgeError::RomTooSmall {
                actual: rom.len(),
                minimum: HEADER_LENGTH,
            });
        }

        let cartridge_type = parse_cartridge_type(rom[CARTRIDGE_TYPE_ADDRESS])?;
        let rom_size = parse_rom_size(rom[ROM_SIZE_ADDRESS])?;
        let ram_size = parse_ram_size(rom[RAM_SIZE_ADDRESS])?;

        if rom.len() < rom_size {
            return Err(CartridgeError::RomLengthMismatch {
                actual: rom.len(),
                declared: rom_size,
            });
        }

        let title_bytes = &rom[TITLE_START..TITLE_END];
        let title_length = title_bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(title_bytes.len());
        let title = String::from_utf8_lossy(&title_bytes[..title_length])
            .trim_end()
            .to_owned();

        Ok(Self {
            title,
            cartridge_type,
            rom_size,
            ram_size,
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn cartridge_type(&self) -> CartridgeType {
        self.cartridge_type
    }

    pub fn rom_size(&self) -> usize {
        self.rom_size
    }

    pub fn ram_size(&self) -> usize {
        self.ram_size
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CartridgeError {
    #[error("ROM 文件过小：实际 {actual} 字节，至少需要 {minimum} 字节")]
    RomTooSmall { actual: usize, minimum: usize },
    #[error("不支持的卡带类型：0x{0:02x}")]
    UnsupportedCartridgeType(u8),
    #[error("不支持的 ROM 容量编码：0x{0:02x}")]
    UnsupportedRomSize(u8),
    #[error("不支持的 RAM 容量编码：0x{0:02x}")]
    UnsupportedRamSize(u8),
    #[error("ROM 文件长度与卡带头不符：实际 {actual} 字节，声明 {declared} 字节")]
    RomLengthMismatch { actual: usize, declared: usize },
    #[error("卡带头配置无效：{0}")]
    InvalidConfiguration(String),
}

fn parse_cartridge_type(code: u8) -> Result<CartridgeType, CartridgeError> {
    match code {
        0x00 => Ok(CartridgeType::RomOnly),
        0x01 => Ok(CartridgeType::Mbc1),
        0x02 => Ok(CartridgeType::Mbc1Ram),
        0x03 => Ok(CartridgeType::Mbc1RamBattery),
        _ => Err(CartridgeError::UnsupportedCartridgeType(code)),
    }
}

fn parse_rom_size(code: u8) -> Result<usize, CartridgeError> {
    match code {
        0x00..=0x08 => Ok((32 * 1024) << code),
        0x52 => Ok(72 * 16 * 1024),
        0x53 => Ok(80 * 16 * 1024),
        0x54 => Ok(96 * 16 * 1024),
        _ => Err(CartridgeError::UnsupportedRomSize(code)),
    }
}

fn parse_ram_size(code: u8) -> Result<usize, CartridgeError> {
    match code {
        0x00 => Ok(0),
        0x02 => Ok(8 * 1024),
        0x03 => Ok(32 * 1024),
        0x04 => Ok(128 * 1024),
        0x05 => Ok(64 * 1024),
        _ => Err(CartridgeError::UnsupportedRamSize(code)),
    }
}

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
        const TOO_SMALL: usize = HEADER_END - 1;
        let error = CartridgeHeader::parse(&vec![0; TOO_SMALL]).expect_err("过小 ROM 必须被拒绝");

        assert!(matches!(
            error,
            CartridgeError::RomTooSmall {
                actual: TOO_SMALL,
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
    fn rejects_unused_ram_size_code() {
        let rom = rom_with_header("UNUSEDRAM", 0x02, 0x00, 0x01);

        let error = CartridgeHeader::parse(&rom).expect_err("未使用的 RAM 容量编码必须被拒绝");

        assert!(matches!(error, CartridgeError::UnsupportedRamSize(0x01)));
    }

    #[test]
    fn rejects_rom_shorter_than_declared_size() {
        const TRUNCATED_SIZE: usize = 32 * 1024;
        const DECLARED_SIZE: usize = 128 * 1024;
        let mut rom = rom_with_header("TRUNCATED", 0x00, 0x02, 0x00);
        rom.truncate(TRUNCATED_SIZE);

        let error = CartridgeHeader::parse(&rom).expect_err("截断 ROM 必须被拒绝");

        assert!(matches!(
            error,
            CartridgeError::RomLengthMismatch {
                actual: TRUNCATED_SIZE,
                declared: DECLARED_SIZE
            }
        ));
    }
}
