pub mod instruction;
pub mod registers;

use registers::Registers;

pub trait Memory {
    fn read8(&self, address: u16) -> u8;
    fn write8(&mut self, address: u16, value: u8);

    fn read16(&self, address: u16) -> u16 {
        let low = self.read8(address) as u16;
        let high = self.read8(address.wrapping_add(1)) as u16;
        low | high << 8
    }

    fn write16(&mut self, address: u16, value: u16) {
        self.write8(address, value as u8);
        self.write8(address.wrapping_add(1), (value >> 8) as u8);
    }
}

#[derive(Debug, Clone)]
pub struct Cpu {
    registers: Registers,
    ime: bool,
    ime_enable_delay: u8,
    halted: bool,
    halt_bug: bool,
}

impl Cpu {
    pub fn post_boot() -> Self {
        Self {
            registers: Registers::post_boot(),
            ime: false,
            ime_enable_delay: 0,
            halted: false,
            halt_bug: false,
        }
    }

    pub fn registers(&self) -> &Registers {
        &self.registers
    }

    pub fn registers_mut(&mut self) -> &mut Registers {
        &mut self.registers
    }

    pub fn ime(&self) -> bool {
        self.ime
    }

    pub fn halted(&self) -> bool {
        self.halted
    }

    fn fetch_byte(&mut self, memory: &impl Memory) -> u8 {
        let value = memory.read8(self.registers.pc);
        if self.halt_bug {
            self.halt_bug = false;
        } else {
            self.registers.pc = self.registers.pc.wrapping_add(1);
        }
        value
    }

    fn fetch_word(&mut self, memory: &impl Memory) -> u16 {
        let low = self.fetch_byte(memory) as u16;
        let high = self.fetch_byte(memory) as u16;
        low | high << 8
    }
}

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

    fn cpu_with_program(program: &[u8]) -> (Cpu, TestMemory) {
        let cpu = Cpu::post_boot();
        let mut memory = TestMemory::default();
        memory.bytes[0x0100..0x0100 + program.len()].copy_from_slice(program);
        (cpu, memory)
    }

    #[test]
    fn executes_immediate_load_and_add_with_flags() {
        let (mut cpu, mut memory) = cpu_with_program(&[0x3e, 0x0f, 0xc6, 0x01]);

        assert_eq!(cpu.step(&mut memory).expect("LD 应执行成功"), 8);
        assert_eq!(cpu.step(&mut memory).expect("ADD 应执行成功"), 8);

        assert_eq!(cpu.registers().a, 0x10);
        assert!(!cpu.registers().flag(registers::Flag::Zero));
        assert!(!cpu.registers().flag(registers::Flag::Subtract));
        assert!(cpu.registers().flag(registers::Flag::HalfCarry));
        assert!(!cpu.registers().flag(registers::Flag::Carry));
    }

    #[test]
    fn increment_and_decrement_preserve_carry() {
        let (mut cpu, mut memory) = cpu_with_program(&[0x04, 0x05]);
        cpu.registers_mut().b = 0x0f;
        cpu.registers_mut()
            .set_flag(registers::Flag::Carry, true);

        cpu.step(&mut memory).expect("INC 应执行成功");
        assert_eq!(cpu.registers().b, 0x10);
        assert!(cpu.registers().flag(registers::Flag::HalfCarry));
        assert!(cpu.registers().flag(registers::Flag::Carry));

        cpu.step(&mut memory).expect("DEC 应执行成功");
        assert_eq!(cpu.registers().b, 0x0f);
        assert!(cpu.registers().flag(registers::Flag::Subtract));
        assert!(cpu.registers().flag(registers::Flag::HalfCarry));
        assert!(cpu.registers().flag(registers::Flag::Carry));
    }

    #[test]
    fn executes_sixteen_bit_load_and_add() {
        let (mut cpu, mut memory) =
            cpu_with_program(&[0x21, 0xff, 0x0f, 0x01, 0x01, 0x00, 0x09]);

        assert_eq!(cpu.step(&mut memory).expect("LD HL 应执行成功"), 12);
        assert_eq!(cpu.step(&mut memory).expect("LD BC 应执行成功"), 12);
        assert_eq!(cpu.step(&mut memory).expect("ADD HL 应执行成功"), 8);

        assert_eq!(cpu.registers().hl(), 0x1000);
        assert!(cpu.registers().flag(registers::Flag::HalfCarry));
        assert!(!cpu.registers().flag(registers::Flag::Carry));
    }

    #[test]
    fn executes_indirect_load_and_updates_hl() {
        let (mut cpu, mut memory) = cpu_with_program(&[0x22, 0x2a]);
        cpu.registers_mut().a = 0x5a;
        cpu.registers_mut().set_hl(0xc000);

        assert_eq!(cpu.step(&mut memory).expect("LD (HL+),A 应执行成功"), 8);
        assert_eq!(memory.read8(0xc000), 0x5a);
        assert_eq!(cpu.registers().hl(), 0xc001);

        memory.write8(0xc001, 0xa5);
        assert_eq!(cpu.step(&mut memory).expect("LD A,(HL+) 应执行成功"), 8);
        assert_eq!(cpu.registers().a, 0xa5);
        assert_eq!(cpu.registers().hl(), 0xc002);
    }

    #[test]
    fn conditional_jump_reports_taken_and_not_taken_cycles() {
        let (mut cpu, mut memory) = cpu_with_program(&[0x20, 0x02, 0x00, 0x00]);
        cpu.registers_mut()
            .set_flag(registers::Flag::Zero, false);

        assert_eq!(cpu.step(&mut memory).expect("JR NZ 应执行成功"), 12);
        assert_eq!(cpu.registers().pc, 0x0104);

        cpu.registers_mut().pc = 0x0100;
        cpu.registers_mut().set_flag(registers::Flag::Zero, true);
        assert_eq!(cpu.step(&mut memory).expect("JR NZ 应执行成功"), 8);
        assert_eq!(cpu.registers().pc, 0x0102);
    }

    #[test]
    fn call_and_return_use_little_endian_stack() {
        let (mut cpu, mut memory) = cpu_with_program(&[0xcd, 0x00, 0x02]);
        memory.bytes[0x0200] = 0xc9;

        assert_eq!(cpu.step(&mut memory).expect("CALL 应执行成功"), 24);
        assert_eq!(cpu.registers().pc, 0x0200);
        assert_eq!(cpu.registers().sp, 0xfffc);
        assert_eq!(memory.read16(0xfffc), 0x0103);

        assert_eq!(cpu.step(&mut memory).expect("RET 应执行成功"), 16);
        assert_eq!(cpu.registers().pc, 0x0103);
        assert_eq!(cpu.registers().sp, 0xfffe);
    }

    #[test]
    fn decimal_adjust_produces_bcd_result() {
        let (mut cpu, mut memory) = cpu_with_program(&[0x3e, 0x15, 0xc6, 0x27, 0x27]);

        cpu.step(&mut memory).expect("LD 应执行成功");
        cpu.step(&mut memory).expect("ADD 应执行成功");
        cpu.step(&mut memory).expect("DAA 应执行成功");

        assert_eq!(cpu.registers().a, 0x42);
        assert!(!cpu.registers().flag(registers::Flag::Carry));
    }

    #[test]
    fn add_sp_signed_value_sets_low_byte_flags() {
        let (mut cpu, mut memory) = cpu_with_program(&[0xe8, 0x01]);
        cpu.registers_mut().sp = 0x00ff;

        assert_eq!(cpu.step(&mut memory).expect("ADD SP 应执行成功"), 16);

        assert_eq!(cpu.registers().sp, 0x0100);
        assert!(cpu.registers().flag(registers::Flag::HalfCarry));
        assert!(cpu.registers().flag(registers::Flag::Carry));
    }

    #[test]
    fn rejects_illegal_opcode() {
        let (mut cpu, mut memory) = cpu_with_program(&[0xd3]);

        assert!(matches!(
            cpu.step(&mut memory),
            Err(instruction::CpuError::InvalidOpcode(0xd3))
        ));
    }

    #[test]
    fn decodes_every_legal_base_opcode() {
        const ILLEGAL: [u8; 11] = [
            0xd3, 0xdb, 0xdd, 0xe3, 0xe4, 0xeb, 0xec, 0xed, 0xf4, 0xfc, 0xfd,
        ];

        for opcode in 0u8..=u8::MAX {
            if opcode == 0xcb || ILLEGAL.contains(&opcode) {
                continue;
            }
            let (mut cpu, mut memory) = cpu_with_program(&[opcode, 0, 0]);
            let result = cpu.step(&mut memory);
            assert!(result.is_ok(), "操作码 0x{opcode:02x} 未实现：{result:?}");
        }
    }
}
