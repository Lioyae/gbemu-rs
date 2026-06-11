#![allow(dead_code)]

const ROM_SIZE: usize = 32 * 1024;
const TITLE_START: usize = 0x134;
const TITLE_LENGTH: usize = 16;

pub struct TestRom {
    bytes: Vec<u8>,
}

impl TestRom {
    pub fn new(title: &str) -> Self {
        let mut bytes = vec![0; ROM_SIZE];
        let title = title.as_bytes();
        let length = title.len().min(TITLE_LENGTH);
        bytes[TITLE_START..TITLE_START + length].copy_from_slice(&title[..length]);
        Self { bytes }
    }

    pub fn cgb_flag(mut self, flag: u8) -> Self {
        self.bytes[0x143] = flag;
        self
    }

    pub fn cartridge(mut self, cartridge_type: u8, ram_size_code: u8) -> Self {
        self.bytes[0x147] = cartridge_type;
        self.bytes[0x148] = 0x00;
        self.bytes[0x149] = ram_size_code;
        self
    }

    pub fn write(mut self, address: usize, data: &[u8]) -> Self {
        let end = address
            .checked_add(data.len())
            .expect("测试 ROM 写入地址不应溢出");
        assert!(end <= self.bytes.len(), "测试 ROM 写入超出 32 KiB 范围");
        self.bytes[address..end].copy_from_slice(data);
        self
    }

    pub fn build(self) -> Vec<u8> {
        self.bytes
    }
}
