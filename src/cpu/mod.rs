pub mod instruction;
pub mod registers;

#[cfg(test)]
mod tests {
    use super::*;

    struct TestMemory {
        bytes: [u8; 0x10000],
    }

    impl Default for TestMemory {
        fn default() -> Self {
            Self {
                bytes: [0; 0x10000],
            }
        }
    }

    impl Memory for TestMemory {
        fn read8(&self, address: u16) -> u8 {
            self.bytes[address as usize]
        }

        fn write8(&mut self, address: u16, value: u8) {
            self.bytes[address as usize] = value;
        }
    }

    #[test]
    fn starts_in_post_boot_state() {
        let cpu = Cpu::post_boot();

        assert_eq!(cpu.registers().af(), 0x01b0);
        assert_eq!(cpu.registers().bc(), 0x0013);
        assert_eq!(cpu.registers().de(), 0x00d8);
        assert_eq!(cpu.registers().hl(), 0x014d);
        assert_eq!(cpu.registers().sp, 0xfffe);
        assert_eq!(cpu.registers().pc, 0x0100);
        assert!(!cpu.ime());
        assert!(!cpu.halted());
    }

    #[test]
    fn fetches_byte_and_advances_program_counter() {
        let mut memory = TestMemory::default();
        memory.bytes[0x0100] = 0x42;
        let mut cpu = Cpu::post_boot();

        let value = cpu.fetch_byte(&memory);

        assert_eq!(value, 0x42);
        assert_eq!(cpu.registers().pc, 0x0101);
    }

    #[test]
    fn fetches_little_endian_word() {
        let mut memory = TestMemory::default();
        memory.bytes[0x0100] = 0x34;
        memory.bytes[0x0101] = 0x12;
        let mut cpu = Cpu::post_boot();

        let value = cpu.fetch_word(&memory);

        assert_eq!(value, 0x1234);
        assert_eq!(cpu.registers().pc, 0x0102);
    }

    #[test]
    fn memory_word_helpers_use_little_endian_order() {
        let mut memory = TestMemory::default();

        memory.write16(0xc000, 0xabcd);

        assert_eq!(memory.read8(0xc000), 0xcd);
        assert_eq!(memory.read8(0xc001), 0xab);
        assert_eq!(memory.read16(0xc000), 0xabcd);
    }
}
