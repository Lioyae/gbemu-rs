pub mod header;
mod mbc1;

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
