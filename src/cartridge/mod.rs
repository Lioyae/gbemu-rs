pub mod header;
mod mbc1;

use mbc1::Mbc1;

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
        let header = CartridgeHeader::parse(&rom)?;
        validate_configuration(&header)?;
        rom.truncate(header.rom_size());

        let controller = match header.cartridge_type() {
            CartridgeType::RomOnly => Controller::RomOnly(RomOnly { rom }),
            CartridgeType::Mbc1 | CartridgeType::Mbc1Ram | CartridgeType::Mbc1RamBattery => {
                Controller::Mbc1(Mbc1::new(rom, header.ram_size()))
            }
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

fn validate_configuration(header: &CartridgeHeader) -> Result<(), CartridgeError> {
    let invalid = |message: &str| Err(CartridgeError::InvalidConfiguration(message.to_owned()));

    match header.cartridge_type() {
        CartridgeType::RomOnly => {
            if header.rom_size() != 32 * 1024 || header.ram_size() != 0 {
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
}

impl MemoryBankController for Controller {
    fn read_rom(&self, address: u16) -> u8 {
        match self {
            Self::RomOnly(controller) => controller.read_rom(address),
            Self::Mbc1(controller) => controller.read_rom(address),
        }
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match self {
            Self::RomOnly(controller) => controller.write_rom(address, value),
            Self::Mbc1(controller) => controller.write_rom(address, value),
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        match self {
            Self::RomOnly(controller) => controller.read_ram(address),
            Self::Mbc1(controller) => controller.read_ram(address),
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        match self {
            Self::RomOnly(controller) => controller.write_ram(address, value),
            Self::Mbc1(controller) => controller.write_ram(address, value),
        }
    }
}

struct RomOnly {
    rom: Vec<u8>,
}

impl MemoryBankController for RomOnly {
    fn read_rom(&self, address: u16) -> u8 {
        if address > 0x7fff {
            return 0xff;
        }
        self.rom.get(address as usize).copied().unwrap_or(0xff)
    }

    fn write_rom(&mut self, _address: u16, _value: u8) {}

    fn read_ram(&self, _address: u16) -> u8 {
        0xff
    }

    fn write_ram(&mut self, _address: u16, _value: u8) {}
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
