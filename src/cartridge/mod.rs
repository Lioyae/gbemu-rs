pub mod header;
mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;
mod mbc6;
mod mmm01;

use mbc1::Mbc1;
use mbc2::Mbc2;
use mbc3::Mbc3;
use mbc5::Mbc5;
use mbc6::Mbc6;
use mmm01::Mmm01;

pub use header::{
    CartridgeError, CartridgeFeatures, CartridgeHeader, CartridgeType, CgbSupport, ControllerKind,
};

pub trait MemoryBankController {
    fn read_rom(&self, address: u16) -> u8;
    fn write_rom(&mut self, address: u16, value: u8);
    fn read_ram(&self, address: u16) -> u8;
    fn write_ram(&mut self, address: u16, value: u8);
}

pub struct Cartridge {
    header: CartridgeHeader,
    controller: Controller,
}

impl Cartridge {
    pub fn from_bytes(mut rom: Vec<u8>) -> Result<Self, CartridgeError> {
        let header = parse_cartridge_header(&rom)?;
        validate_configuration(&header)?;
        rom.truncate(header.rom_size());

        let controller = match header.cartridge_type() {
            CartridgeType::RomOnly | CartridgeType::RomRam | CartridgeType::RomRamBattery => {
                Controller::RomOnly(RomOnly::new(rom, header.ram_size()))
            }
            CartridgeType::Mbc1 | CartridgeType::Mbc1Ram | CartridgeType::Mbc1RamBattery => {
                Controller::Mbc1(Mbc1::new(rom, header.ram_size()))
            }
            CartridgeType::Mbc2 | CartridgeType::Mbc2Battery => {
                Controller::Mbc2(Mbc2::new(rom))
            }
            CartridgeType::Mmm01
            | CartridgeType::Mmm01Ram
            | CartridgeType::Mmm01RamBattery => {
                Controller::Mmm01(Mmm01::new(rom, header.ram_size()))
            }
            CartridgeType::Mbc3TimerBattery
            | CartridgeType::Mbc3TimerRamBattery
            | CartridgeType::Mbc3
            | CartridgeType::Mbc3Ram
            | CartridgeType::Mbc3RamBattery => Controller::Mbc3(Mbc3::new(
                rom,
                header.ram_size(),
                header.cartridge_type().features().rtc,
            )),
            CartridgeType::Mbc5
            | CartridgeType::Mbc5Ram
            | CartridgeType::Mbc5RamBattery
            | CartridgeType::Mbc5Rumble
            | CartridgeType::Mbc5RumbleRam
            | CartridgeType::Mbc5RumbleRamBattery => Controller::Mbc5(Mbc5::new(
                rom,
                header.ram_size(),
                header.cartridge_type().features().rumble,
            )),
            CartridgeType::Mbc6 => Controller::Mbc6(Mbc6::new(rom, header.ram_size())),
            cartridge_type => {
                return Err(CartridgeError::ControllerNotImplemented(cartridge_type));
            }
        };

        Ok(Self { header, controller })
    }

    pub fn header(&self) -> &CartridgeHeader {
        &self.header
    }

    pub fn read_rom(&self, address: u16) -> u8 {
        self.controller.read_rom(address)
    }

    pub fn write_rom(&mut self, address: u16, value: u8) {
        self.controller.write_rom(address, value);
    }

    pub fn read_ram(&self, address: u16) -> u8 {
        self.controller.read_ram(address)
    }

    pub fn write_ram(&mut self, address: u16, value: u8) {
        self.controller.write_ram(address, value);
    }
}

fn parse_cartridge_header(rom: &[u8]) -> Result<CartridgeHeader, CartridgeError> {
    const MENU_SIZE: usize = 32 * 1024;
    const TYPE_OFFSET: usize = 0x147;

    if let Some(menu_base) = rom.len().checked_sub(MENU_SIZE)
        && matches!(
            rom.get(menu_base + TYPE_OFFSET),
            Some(0x0b | 0x0c | 0x0d)
        )
        && let Ok(header) = CartridgeHeader::parse_at(rom, menu_base)
        && header.rom_size() == rom.len()
    {
        return Ok(header);
    }

    CartridgeHeader::parse(rom)
}

fn validate_configuration(header: &CartridgeHeader) -> Result<(), CartridgeError> {
    let invalid = |message: &str| Err(CartridgeError::InvalidConfiguration(message.to_owned()));

    match header.cartridge_type() {
        CartridgeType::RomOnly | CartridgeType::RomRam | CartridgeType::RomRamBattery => {
            if header.rom_size() != 32 * 1024 {
                return invalid("无 MBC 卡带必须使用 32 KiB ROM");
            }
            let has_ram = header.cartridge_type().features().ram;
            if has_ram != (header.ram_size() != 0) {
                return invalid("无 MBC 卡带的类型与外部 RAM 容量不匹配");
            }
            if header.cartridge_type() == CartridgeType::RomOnly && header.ram_size() != 0 {
                return invalid("ROM-only 卡带必须使用 32 KiB ROM 且不能声明外部 RAM");
            }
        }
        CartridgeType::Mbc1 => {
            if header.ram_size() != 0 {
                return invalid("MBC1 类型 0x01 不能声明外部 RAM");
            }
        }
        CartridgeType::Mbc1Ram | CartridgeType::Mbc1RamBattery => {
            if header.ram_size() == 0 {
                return invalid("MBC1+RAM 卡带必须声明外部 RAM");
            }
            if header.ram_size() > 32 * 1024 {
                return invalid("MBC1 最多支持 32 KiB 外部 RAM");
            }
        }
        CartridgeType::Mmm01 | CartridgeType::Mmm01Ram | CartridgeType::Mmm01RamBattery => {
            let has_ram = header.cartridge_type().features().ram;
            if has_ram != (header.ram_size() != 0) {
                return invalid("MMM01 卡带的类型与外部 RAM 容量不匹配");
            }
            if header.rom_size() > 8 * 1024 * 1024 {
                return invalid("MMM01 最多支持 8 MiB ROM");
            }
            if header.ram_size() > 128 * 1024 {
                return invalid("MMM01 最多支持 128 KiB 外部 RAM");
            }
        }
        _ => return Ok(()),
    }

    if matches!(
        header.cartridge_type(),
        CartridgeType::Mbc1 | CartridgeType::Mbc1Ram | CartridgeType::Mbc1RamBattery
    ) {
        if header.rom_size() > 2 * 1024 * 1024 {
            return invalid("MBC1 最多支持 2 MiB ROM");
        }
        if header.ram_size() == 32 * 1024 && header.rom_size() > 512 * 1024 {
            return invalid("32 KiB RAM 的 MBC1 卡带最多支持 512 KiB ROM");
        }
    }

    Ok(())
}

enum Controller {
    RomOnly(RomOnly),
    Mbc1(Mbc1),
    Mbc2(Mbc2),
    Mmm01(Mmm01),
    Mbc3(Mbc3),
    Mbc5(Mbc5),
    Mbc6(Mbc6),
}

impl MemoryBankController for Controller {
    fn read_rom(&self, address: u16) -> u8 {
        match self {
            Self::RomOnly(controller) => controller.read_rom(address),
            Self::Mbc1(controller) => controller.read_rom(address),
            Self::Mbc2(controller) => controller.read_rom(address),
            Self::Mmm01(controller) => controller.read_rom(address),
            Self::Mbc3(controller) => controller.read_rom(address),
            Self::Mbc5(controller) => controller.read_rom(address),
            Self::Mbc6(controller) => controller.read_rom(address),
        }
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match self {
            Self::RomOnly(controller) => controller.write_rom(address, value),
            Self::Mbc1(controller) => controller.write_rom(address, value),
            Self::Mbc2(controller) => controller.write_rom(address, value),
            Self::Mmm01(controller) => controller.write_rom(address, value),
            Self::Mbc3(controller) => controller.write_rom(address, value),
            Self::Mbc5(controller) => controller.write_rom(address, value),
            Self::Mbc6(controller) => controller.write_rom(address, value),
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        match self {
            Self::RomOnly(controller) => controller.read_ram(address),
            Self::Mbc1(controller) => controller.read_ram(address),
            Self::Mbc2(controller) => controller.read_ram(address),
            Self::Mmm01(controller) => controller.read_ram(address),
            Self::Mbc3(controller) => controller.read_ram(address),
            Self::Mbc5(controller) => controller.read_ram(address),
            Self::Mbc6(controller) => controller.read_ram(address),
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        match self {
            Self::RomOnly(controller) => controller.write_ram(address, value),
            Self::Mbc1(controller) => controller.write_ram(address, value),
            Self::Mbc2(controller) => controller.write_ram(address, value),
            Self::Mmm01(controller) => controller.write_ram(address, value),
            Self::Mbc3(controller) => controller.write_ram(address, value),
            Self::Mbc5(controller) => controller.write_ram(address, value),
            Self::Mbc6(controller) => controller.write_ram(address, value),
        }
    }
}

struct RomOnly {
    rom: Vec<u8>,
    ram: Vec<u8>,
}

impl RomOnly {
    fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size],
        }
    }
}

impl MemoryBankController for RomOnly {
    fn read_rom(&self, address: u16) -> u8 {
        if address > 0x7fff {
            return 0xff;
        }
        self.rom.get(address as usize).copied().unwrap_or(0xff)
    }

    fn write_rom(&mut self, _address: u16, _value: u8) {}

    fn read_ram(&self, address: u16) -> u8 {
        let Some(offset) = address.checked_sub(0xa000) else {
            return 0xff;
        };
        self.ram.get(offset as usize).copied().unwrap_or(0xff)
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        let Some(offset) = address.checked_sub(0xa000) else {
            return;
        };
        if let Some(byte) = self.ram.get_mut(offset as usize) {
            *byte = value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom_only() -> Vec<u8> {
        let mut rom = vec![0; 32 * 1024];
        rom[0x134..0x13c].copy_from_slice(b"ROM ONLY");
        rom[0x147] = 0x00;
        rom[0x148] = 0x00;
        rom[0x149] = 0x00;
        rom[0x0150] = 0x42;
        rom[0x7fff] = 0x99;
        rom
    }

    #[test]
    fn rom_only_reads_fixed_rom() {
        let cartridge = Cartridge::from_bytes(rom_only()).expect("ROM-only 卡带应创建成功");

        assert_eq!(cartridge.read_rom(0x0150), 0x42);
        assert_eq!(cartridge.read_rom(0x7fff), 0x99);
        assert_eq!(cartridge.header().title(), "ROM ONLY");
    }

    #[test]
    fn rom_only_ignores_writes_and_out_of_range_reads() {
        let mut cartridge = Cartridge::from_bytes(rom_only()).expect("ROM-only 卡带应创建成功");

        cartridge.write_rom(0x0150, 0xaa);

        assert_eq!(cartridge.read_rom(0x0150), 0x42);
        assert_eq!(cartridge.read_rom(0x8000), 0xff);
        assert_eq!(cartridge.read_ram(0xa000), 0xff);
    }

    #[test]
    fn creates_mbc1_controller_from_header() {
        let mut rom = vec![0; 64 * 1024];
        rom[0x134..0x138].copy_from_slice(b"MBC1");
        rom[0x147] = 0x01;
        rom[0x148] = 0x01;
        rom[0x149] = 0x00;
        rom[0x4000] = 0x11;
        rom[0x8000] = 0x22;

        let mut cartridge = Cartridge::from_bytes(rom).expect("MBC1 卡带应创建成功");
        assert_eq!(cartridge.read_rom(0x4000), 0x11);

        cartridge.write_rom(0x2000, 0x02);

        assert_eq!(cartridge.read_rom(0x4000), 0x22);
    }

    #[test]
    fn creates_main_official_controllers_from_header() {
        let cases = [
            (0x05, 0x00, 0x00),
            (0x06, 0x00, 0x00),
            (0x0f, 0x00, 0x00),
            (0x10, 0x02, 0x03),
            (0x11, 0x02, 0x00),
            (0x12, 0x02, 0x03),
            (0x13, 0x02, 0x03),
            (0x19, 0x02, 0x00),
            (0x1a, 0x02, 0x03),
            (0x1b, 0x02, 0x03),
            (0x1c, 0x02, 0x00),
            (0x1d, 0x02, 0x03),
            (0x1e, 0x02, 0x03),
            (0x20, 0x02, 0x03),
        ];

        for (cartridge_type, rom_size, ram_size) in cases {
            let size = (32 * 1024) << rom_size;
            let mut rom = vec![0; size];
            rom[0x147] = cartridge_type;
            rom[0x148] = rom_size;
            rom[0x149] = ram_size;

            Cartridge::from_bytes(rom).unwrap_or_else(|error| {
                panic!("卡带类型 0x{cartridge_type:02x} 应创建成功：{error}")
            });
        }
    }

    #[test]
    fn rom_ram_type_reads_and_writes_external_ram() {
        let mut rom = vec![0; 32 * 1024];
        rom[0x147] = 0x08;
        rom[0x148] = 0x00;
        rom[0x149] = 0x02;

        let mut cartridge = Cartridge::from_bytes(rom).expect("ROM+RAM 卡带应创建成功");
        cartridge.write_ram(0xa123, 0x5a);

        assert_eq!(cartridge.read_ram(0xa123), 0x5a);
    }

    #[test]
    fn loads_mmm01_header_from_last_thirty_two_kib() {
        let mut rom = vec![0; 512 * 1024];
        for bank in 0..32 {
            rom[bank * 0x4000] = bank as u8;
        }
        let menu_base = rom.len() - 32 * 1024;
        rom[menu_base + 0x134..menu_base + 0x13a].copy_from_slice(b"MMM01!");
        rom[menu_base + 0x147] = 0x0b;
        rom[menu_base + 0x148] = 0x04;
        rom[menu_base + 0x149] = 0x00;

        let cartridge = Cartridge::from_bytes(rom).expect("MMM01 菜单卡带头应创建成功");

        assert_eq!(cartridge.header().cartridge_type(), CartridgeType::Mmm01);
        assert_eq!(cartridge.header().title(), "MMM01!");
        assert_eq!(cartridge.read_rom(0x0000), 30);
        assert_eq!(cartridge.read_rom(0x4000), 31);
    }

    #[test]
    fn rejects_rom_only_with_banked_rom_size() {
        let mut rom = vec![0; 64 * 1024];
        rom[0x147] = 0x00;
        rom[0x148] = 0x01;
        rom[0x149] = 0x00;

        assert!(matches!(
            Cartridge::from_bytes(rom),
            Err(CartridgeError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn rejects_mbc1_rom_larger_than_two_mib() {
        let mut rom = vec![0; 4 * 1024 * 1024];
        rom[0x147] = 0x01;
        rom[0x148] = 0x07;
        rom[0x149] = 0x00;

        assert!(matches!(
            Cartridge::from_bytes(rom),
            Err(CartridgeError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn rejects_ram_declared_by_mbc1_without_ram_type() {
        let mut rom = vec![0; 32 * 1024];
        rom[0x147] = 0x01;
        rom[0x148] = 0x00;
        rom[0x149] = 0x03;

        assert!(matches!(
            Cartridge::from_bytes(rom),
            Err(CartridgeError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn rejects_thirty_two_kib_ram_with_large_mbc1_rom() {
        let mut rom = vec![0; 1024 * 1024];
        rom[0x147] = 0x03;
        rom[0x148] = 0x05;
        rom[0x149] = 0x03;

        assert!(matches!(
            Cartridge::from_bytes(rom),
            Err(CartridgeError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn rejects_mbc1_ram_type_without_ram_capacity() {
        let mut rom = vec![0; 32 * 1024];
        rom[0x147] = 0x02;
        rom[0x148] = 0x00;
        rom[0x149] = 0x00;

        assert!(matches!(
            Cartridge::from_bytes(rom),
            Err(CartridgeError::InvalidConfiguration(_))
        ));
    }
}
