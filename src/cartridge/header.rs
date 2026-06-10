use std::fmt;

use thiserror::Error;

const HEADER_LENGTH: usize = 0x150;
const TITLE_START: usize = 0x134;
const TITLE_END: usize = 0x144;
const CGB_TITLE_END: usize = 0x143;
const CGB_FLAG_ADDRESS: usize = 0x143;
const CARTRIDGE_TYPE_ADDRESS: usize = 0x147;
const ROM_SIZE_ADDRESS: usize = 0x148;
const RAM_SIZE_ADDRESS: usize = 0x149;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeType {
    RomOnly,
    Mbc1,
    Mbc1Ram,
    Mbc1RamBattery,
    Mbc2,
    Mbc2Battery,
    RomRam,
    RomRamBattery,
    Mmm01,
    Mmm01Ram,
    Mmm01RamBattery,
    Mbc3TimerBattery,
    Mbc3TimerRamBattery,
    Mbc3,
    Mbc3Ram,
    Mbc3RamBattery,
    Mbc5,
    Mbc5Ram,
    Mbc5RamBattery,
    Mbc5Rumble,
    Mbc5RumbleRam,
    Mbc5RumbleRamBattery,
    Mbc6,
    Mbc7SensorRumbleRamBattery,
    PocketCamera,
    BandaiTama5,
    Huc3,
    Huc1RamBattery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerKind {
    Rom,
    Mbc1,
    Mbc2,
    Mmm01,
    Mbc3,
    Mbc5,
    Mbc6,
    Mbc7,
    PocketCamera,
    BandaiTama5,
    Huc3,
    Huc1,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CartridgeFeatures {
    pub ram: bool,
    pub battery: bool,
    pub rtc: bool,
    pub rumble: bool,
    pub sensor: bool,
    pub camera: bool,
    pub infrared: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CgbSupport {
    DmgOnly,
    Compatible,
    Required,
}

impl CartridgeType {
    pub fn controller(self) -> ControllerKind {
        match self {
            Self::RomOnly | Self::RomRam | Self::RomRamBattery => ControllerKind::Rom,
            Self::Mbc1 | Self::Mbc1Ram | Self::Mbc1RamBattery => ControllerKind::Mbc1,
            Self::Mbc2 | Self::Mbc2Battery => ControllerKind::Mbc2,
            Self::Mmm01 | Self::Mmm01Ram | Self::Mmm01RamBattery => ControllerKind::Mmm01,
            Self::Mbc3TimerBattery
            | Self::Mbc3TimerRamBattery
            | Self::Mbc3
            | Self::Mbc3Ram
            | Self::Mbc3RamBattery => ControllerKind::Mbc3,
            Self::Mbc5
            | Self::Mbc5Ram
            | Self::Mbc5RamBattery
            | Self::Mbc5Rumble
            | Self::Mbc5RumbleRam
            | Self::Mbc5RumbleRamBattery => ControllerKind::Mbc5,
            Self::Mbc6 => ControllerKind::Mbc6,
            Self::Mbc7SensorRumbleRamBattery => ControllerKind::Mbc7,
            Self::PocketCamera => ControllerKind::PocketCamera,
            Self::BandaiTama5 => ControllerKind::BandaiTama5,
            Self::Huc3 => ControllerKind::Huc3,
            Self::Huc1RamBattery => ControllerKind::Huc1,
        }
    }

    pub fn features(self) -> CartridgeFeatures {
        use CartridgeType::*;

        match self {
            RomOnly | Mbc1 | Mmm01 | Mbc3 | Mbc5 => CartridgeFeatures::default(),
            Mbc1Ram | RomRam | Mmm01Ram | Mbc3Ram | Mbc5Ram => CartridgeFeatures {
                ram: true,
                ..CartridgeFeatures::default()
            },
            Mbc1RamBattery | RomRamBattery | Mmm01RamBattery | Mbc3RamBattery | Mbc5RamBattery => {
                CartridgeFeatures {
                    ram: true,
                    battery: true,
                    ..CartridgeFeatures::default()
                }
            }
            Mbc2 => CartridgeFeatures {
                ram: true,
                ..CartridgeFeatures::default()
            },
            Mbc2Battery => CartridgeFeatures {
                ram: true,
                battery: true,
                ..CartridgeFeatures::default()
            },
            Mbc3TimerBattery => CartridgeFeatures {
                battery: true,
                rtc: true,
                ..CartridgeFeatures::default()
            },
            Mbc3TimerRamBattery => CartridgeFeatures {
                ram: true,
                battery: true,
                rtc: true,
                ..CartridgeFeatures::default()
            },
            Mbc5Rumble => CartridgeFeatures {
                rumble: true,
                ..CartridgeFeatures::default()
            },
            Mbc5RumbleRam => CartridgeFeatures {
                ram: true,
                rumble: true,
                ..CartridgeFeatures::default()
            },
            Mbc5RumbleRamBattery => CartridgeFeatures {
                ram: true,
                battery: true,
                rumble: true,
                ..CartridgeFeatures::default()
            },
            Mbc6 => CartridgeFeatures {
                ram: true,
                battery: true,
                ..CartridgeFeatures::default()
            },
            Mbc7SensorRumbleRamBattery => CartridgeFeatures {
                ram: true,
                battery: true,
                rumble: true,
                sensor: true,
                ..CartridgeFeatures::default()
            },
            PocketCamera => CartridgeFeatures {
                ram: true,
                battery: true,
                camera: true,
                ..CartridgeFeatures::default()
            },
            BandaiTama5 => CartridgeFeatures {
                ram: true,
                battery: true,
                rtc: true,
                ..CartridgeFeatures::default()
            },
            Huc3 => CartridgeFeatures {
                ram: true,
                battery: true,
                rtc: true,
                infrared: true,
                ..CartridgeFeatures::default()
            },
            Huc1RamBattery => CartridgeFeatures {
                ram: true,
                battery: true,
                infrared: true,
                ..CartridgeFeatures::default()
            },
        }
    }
}

impl fmt::Display for CartridgeType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::RomOnly => "ROM-only",
            Self::Mbc1 => "MBC1",
            Self::Mbc1Ram => "MBC1+RAM",
            Self::Mbc1RamBattery => "MBC1+RAM+BATTERY",
            Self::Mbc2 => "MBC2",
            Self::Mbc2Battery => "MBC2+BATTERY",
            Self::RomRam => "ROM+RAM",
            Self::RomRamBattery => "ROM+RAM+BATTERY",
            Self::Mmm01 => "MMM01",
            Self::Mmm01Ram => "MMM01+RAM",
            Self::Mmm01RamBattery => "MMM01+RAM+BATTERY",
            Self::Mbc3TimerBattery => "MBC3+TIMER+BATTERY",
            Self::Mbc3TimerRamBattery => "MBC3+TIMER+RAM+BATTERY",
            Self::Mbc3 => "MBC3",
            Self::Mbc3Ram => "MBC3+RAM",
            Self::Mbc3RamBattery => "MBC3+RAM+BATTERY",
            Self::Mbc5 => "MBC5",
            Self::Mbc5Ram => "MBC5+RAM",
            Self::Mbc5RamBattery => "MBC5+RAM+BATTERY",
            Self::Mbc5Rumble => "MBC5+RUMBLE",
            Self::Mbc5RumbleRam => "MBC5+RUMBLE+RAM",
            Self::Mbc5RumbleRamBattery => "MBC5+RUMBLE+RAM+BATTERY",
            Self::Mbc6 => "MBC6",
            Self::Mbc7SensorRumbleRamBattery => "MBC7+SENSOR+RUMBLE+RAM+BATTERY",
            Self::PocketCamera => "POCKET CAMERA",
            Self::BandaiTama5 => "BANDAI TAMA5",
            Self::Huc3 => "HuC3",
            Self::Huc1RamBattery => "HuC1+RAM+BATTERY",
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
    cgb_support: CgbSupport,
}

impl CartridgeHeader {
    pub fn parse(rom: &[u8]) -> Result<Self, CartridgeError> {
        Self::parse_at(rom, 0)
    }

    pub(super) fn parse_at(rom: &[u8], base: usize) -> Result<Self, CartridgeError> {
        let available = rom.len().saturating_sub(base);
        if available < HEADER_LENGTH {
            return Err(CartridgeError::RomTooSmall {
                actual: available,
                minimum: HEADER_LENGTH,
            });
        }

        let cartridge_type = parse_cartridge_type(rom[base + CARTRIDGE_TYPE_ADDRESS])?;
        let rom_size = parse_rom_size(rom[base + ROM_SIZE_ADDRESS])?;
        let ram_size = parse_ram_size(rom[base + RAM_SIZE_ADDRESS])?;
        let cgb_support = parse_cgb_support(rom[base + CGB_FLAG_ADDRESS]);

        if rom.len() < rom_size {
            return Err(CartridgeError::RomLengthMismatch {
                actual: rom.len(),
                declared: rom_size,
            });
        }

        let title_end = if cgb_support == CgbSupport::DmgOnly {
            TITLE_END
        } else {
            CGB_TITLE_END
        };
        let title_bytes = &rom[base + TITLE_START..base + title_end];
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
            cgb_support,
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

    pub fn cgb_support(&self) -> CgbSupport {
        self.cgb_support
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
    #[error("卡带控制器尚未实现：{0}")]
    ControllerNotImplemented(CartridgeType),
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
        0x05 => Ok(CartridgeType::Mbc2),
        0x06 => Ok(CartridgeType::Mbc2Battery),
        0x08 => Ok(CartridgeType::RomRam),
        0x09 => Ok(CartridgeType::RomRamBattery),
        0x0b => Ok(CartridgeType::Mmm01),
        0x0c => Ok(CartridgeType::Mmm01Ram),
        0x0d => Ok(CartridgeType::Mmm01RamBattery),
        0x0f => Ok(CartridgeType::Mbc3TimerBattery),
        0x10 => Ok(CartridgeType::Mbc3TimerRamBattery),
        0x11 => Ok(CartridgeType::Mbc3),
        0x12 => Ok(CartridgeType::Mbc3Ram),
        0x13 => Ok(CartridgeType::Mbc3RamBattery),
        0x19 => Ok(CartridgeType::Mbc5),
        0x1a => Ok(CartridgeType::Mbc5Ram),
        0x1b => Ok(CartridgeType::Mbc5RamBattery),
        0x1c => Ok(CartridgeType::Mbc5Rumble),
        0x1d => Ok(CartridgeType::Mbc5RumbleRam),
        0x1e => Ok(CartridgeType::Mbc5RumbleRamBattery),
        0x20 => Ok(CartridgeType::Mbc6),
        0x22 => Ok(CartridgeType::Mbc7SensorRumbleRamBattery),
        0xfc => Ok(CartridgeType::PocketCamera),
        0xfd => Ok(CartridgeType::BandaiTama5),
        0xfe => Ok(CartridgeType::Huc3),
        0xff => Ok(CartridgeType::Huc1RamBattery),
        _ => Err(CartridgeError::UnsupportedCartridgeType(code)),
    }
}

fn parse_cgb_support(flag: u8) -> CgbSupport {
    match flag & 0xc0 {
        0x80 => CgbSupport::Compatible,
        0xc0 => CgbSupport::Required,
        _ => CgbSupport::DmgOnly,
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
    fn parses_dmg_and_cgb_support_flags() {
        let cases = [
            (0x00, CgbSupport::DmgOnly),
            (0x80, CgbSupport::Compatible),
            (0xc0, CgbSupport::Required),
        ];

        for (flag, expected) in cases {
            let mut rom = rom_with_header("COLOR", 0x00, 0x00, 0x00);
            rom[0x143] = flag;

            let header = CartridgeHeader::parse(&rom).expect("CGB 标志应解析成功");

            assert_eq!(header.cgb_support(), expected);
        }
    }

    #[test]
    fn parses_all_official_cartridge_type_codes() {
        let cases = [
            (0x00, CartridgeType::RomOnly, ControllerKind::Rom),
            (0x01, CartridgeType::Mbc1, ControllerKind::Mbc1),
            (0x02, CartridgeType::Mbc1Ram, ControllerKind::Mbc1),
            (0x03, CartridgeType::Mbc1RamBattery, ControllerKind::Mbc1),
            (0x05, CartridgeType::Mbc2, ControllerKind::Mbc2),
            (0x06, CartridgeType::Mbc2Battery, ControllerKind::Mbc2),
            (0x08, CartridgeType::RomRam, ControllerKind::Rom),
            (0x09, CartridgeType::RomRamBattery, ControllerKind::Rom),
            (0x0b, CartridgeType::Mmm01, ControllerKind::Mmm01),
            (0x0c, CartridgeType::Mmm01Ram, ControllerKind::Mmm01),
            (0x0d, CartridgeType::Mmm01RamBattery, ControllerKind::Mmm01),
            (0x0f, CartridgeType::Mbc3TimerBattery, ControllerKind::Mbc3),
            (
                0x10,
                CartridgeType::Mbc3TimerRamBattery,
                ControllerKind::Mbc3,
            ),
            (0x11, CartridgeType::Mbc3, ControllerKind::Mbc3),
            (0x12, CartridgeType::Mbc3Ram, ControllerKind::Mbc3),
            (0x13, CartridgeType::Mbc3RamBattery, ControllerKind::Mbc3),
            (0x19, CartridgeType::Mbc5, ControllerKind::Mbc5),
            (0x1a, CartridgeType::Mbc5Ram, ControllerKind::Mbc5),
            (0x1b, CartridgeType::Mbc5RamBattery, ControllerKind::Mbc5),
            (0x1c, CartridgeType::Mbc5Rumble, ControllerKind::Mbc5),
            (0x1d, CartridgeType::Mbc5RumbleRam, ControllerKind::Mbc5),
            (
                0x1e,
                CartridgeType::Mbc5RumbleRamBattery,
                ControllerKind::Mbc5,
            ),
            (0x20, CartridgeType::Mbc6, ControllerKind::Mbc6),
            (
                0x22,
                CartridgeType::Mbc7SensorRumbleRamBattery,
                ControllerKind::Mbc7,
            ),
            (
                0xfc,
                CartridgeType::PocketCamera,
                ControllerKind::PocketCamera,
            ),
            (
                0xfd,
                CartridgeType::BandaiTama5,
                ControllerKind::BandaiTama5,
            ),
            (0xfe, CartridgeType::Huc3, ControllerKind::Huc3),
            (0xff, CartridgeType::Huc1RamBattery, ControllerKind::Huc1),
        ];

        assert_eq!(cases.len(), 28);
        for (code, expected_type, expected_controller) in cases {
            let rom = rom_with_header("OFFICIAL", code, 0x00, 0x00);
            let header = CartridgeHeader::parse(&rom).expect("官方卡带编码应能识别");

            assert_eq!(header.cartridge_type(), expected_type);
            assert_eq!(header.cartridge_type().controller(), expected_controller);
        }
    }

    #[test]
    fn exposes_cartridge_features() {
        let features = CartridgeType::Mbc3TimerRamBattery.features();
        assert!(features.ram);
        assert!(features.battery);
        assert!(features.rtc);
        assert!(!features.rumble);
        assert!(!features.sensor);
        assert!(!features.camera);

        let features = CartridgeType::Mbc7SensorRumbleRamBattery.features();
        assert!(features.ram);
        assert!(features.battery);
        assert!(features.rumble);
        assert!(features.sensor);
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
        let rom = rom_with_header("BADTYPE", 0x04, 0x00, 0x00);

        let error = CartridgeHeader::parse(&rom).expect_err("未知卡带类型必须被拒绝");

        assert!(matches!(
            error,
            CartridgeError::UnsupportedCartridgeType(0x04)
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
