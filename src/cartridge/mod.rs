pub mod header;
mod mbc1;

use mbc1::Mbc1;

pub use header::{CartridgeError, CartridgeHeader, CartridgeType};

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
        rom.truncate(header.rom_size());

        let controller = match header.cartridge_type() {
            CartridgeType::RomOnly => Controller::RomOnly(RomOnly { rom }),
            CartridgeType::Mbc1
            | CartridgeType::Mbc1Ram
            | CartridgeType::Mbc1RamBattery => {
                Controller::Mbc1(Mbc1::new(rom, header.ram_size()))
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
        let mut cartridge =
            Cartridge::from_bytes(rom_only()).expect("ROM-only 卡带应创建成功");

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
}
