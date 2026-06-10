pub mod instruction;
pub mod registers;

use registers::Registers;

pub trait Memory {
    fn read8(&self, address: u16) -> u8;
    fn write8(&mut self, address: u16, value: u8);
    fn tick(&mut self, _cycles: u8) {}

    fn cpu_read8(&mut self, address: u16) -> u8 {
        self.tick(4);
        self.read8(address)
    }

    fn cpu_write8(&mut self, address: u16, value: u8) {
        self.tick(4);
        self.write8(address, value);
    }

    fn cpu_idle(&mut self) {
        self.tick(4);
    }

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
    instruction_cycles: u8,
}

impl Cpu {
    pub fn post_boot() -> Self {
        Self {
            registers: Registers::post_boot(),
            ime: false,
            ime_enable_delay: 0,
            halted: false,
            halt_bug: false,
            instruction_cycles: 0,
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

    fn fetch_byte(&mut self, memory: &mut impl Memory) -> u8 {
        let value = self.read_bus(memory, self.registers.pc);
        if self.halt_bug {
            self.halt_bug = false;
        } else {
            self.registers.pc = self.registers.pc.wrapping_add(1);
        }
        value
    }

    fn fetch_word(&mut self, memory: &mut impl Memory) -> u16 {
        let low = self.fetch_byte(memory) as u16;
        let high = self.fetch_byte(memory) as u16;
        low | high << 8
    }

    fn read_bus(&mut self, memory: &mut impl Memory, address: u16) -> u8 {
        let value = memory.cpu_read8(address);
        self.instruction_cycles += 4;
        value
    }

    fn write_bus(&mut self, memory: &mut impl Memory, address: u16, value: u8) {
        memory.cpu_write8(address, value);
        self.instruction_cycles += 4;
    }

    fn idle_bus(&mut self, memory: &mut impl Memory) {
        memory.cpu_idle();
        self.instruction_cycles += 4;
    }

    fn finish_cycles(&mut self, memory: &mut impl Memory, expected: u8) {
        while self.instruction_cycles < expected {
            self.idle_bus(memory);
        }
        debug_assert_eq!(self.instruction_cycles, expected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum BusEvent {
        Tick(u8),
        Read(u16),
        Write(u16, u8),
    }

    struct TestMemory {
        bytes: [u8; 0x10000],
        events: Vec<BusEvent>,
    }

    impl Default for TestMemory {
        fn default() -> Self {
            Self {
                bytes: [0; 0x10000],
                events: Vec::new(),
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

        fn tick(&mut self, cycles: u8) {
            self.events.push(BusEvent::Tick(cycles));
        }

        fn cpu_read8(&mut self, address: u16) -> u8 {
            self.tick(4);
            self.events.push(BusEvent::Read(address));
            self.read8(address)
        }

        fn cpu_write8(&mut self, address: u16, value: u8) {
            self.tick(4);
            self.events.push(BusEvent::Write(address, value));
            self.write8(address, value);
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

        let value = cpu.fetch_byte(&mut memory);

        assert_eq!(value, 0x42);
        assert_eq!(cpu.registers().pc, 0x0101);
    }

    #[test]
    fn fetches_little_endian_word() {
        let mut memory = TestMemory::default();
        memory.bytes[0x0100] = 0x34;
        memory.bytes[0x0101] = 0x12;
        let mut cpu = Cpu::post_boot();

        let value = cpu.fetch_word(&mut memory);

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

    #[test]
    fn machine_cycle_ld_a16_sp_orders_bus_writes() {
        let (mut cpu, mut memory) = cpu_with_program(&[0x08, 0x00, 0xc0]);
        cpu.registers_mut().sp = 0xabcd;

        assert_eq!(cpu.step(&mut memory).expect("LD (a16),SP 应执行成功"), 20);

        assert_eq!(
            memory.events,
            [
                BusEvent::Tick(4),
                BusEvent::Read(0x0100),
                BusEvent::Tick(4),
                BusEvent::Read(0x0101),
                BusEvent::Tick(4),
                BusEvent::Read(0x0102),
                BusEvent::Tick(4),
                BusEvent::Write(0xc000, 0xcd),
                BusEvent::Tick(4),
                BusEvent::Write(0xc001, 0xab),
            ]
        );
    }

    #[test]
    fn machine_cycle_call_waits_before_stack_writes() {
        let (mut cpu, mut memory) = cpu_with_program(&[0xcd, 0x00, 0x02]);

        assert_eq!(cpu.step(&mut memory).expect("CALL 应执行成功"), 24);

        assert_eq!(
            memory.events,
            [
                BusEvent::Tick(4),
                BusEvent::Read(0x0100),
                BusEvent::Tick(4),
                BusEvent::Read(0x0101),
                BusEvent::Tick(4),
                BusEvent::Read(0x0102),
                BusEvent::Tick(4),
                BusEvent::Tick(4),
                BusEvent::Write(0xfffd, 0x01),
                BusEvent::Tick(4),
                BusEvent::Write(0xfffc, 0x03),
            ]
        );
    }

    #[test]
    fn machine_cycle_interrupt_service_uses_five_cycles() {
        let (mut cpu, mut memory) = cpu_with_program(&[0x00]);
        cpu.ime = true;
        memory.write8(0xffff, 0x01);
        memory.write8(0xff0f, 0x01);

        assert_eq!(cpu.step(&mut memory).expect("中断应处理成功"), 20);

        assert_eq!(
            memory.events,
            [
                BusEvent::Tick(4),
                BusEvent::Tick(4),
                BusEvent::Tick(4),
                BusEvent::Write(0xfffd, 0x01),
                BusEvent::Tick(4),
                BusEvent::Write(0xfffc, 0x00),
                BusEvent::Tick(4),
            ]
        );
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
        cpu.registers_mut().set_flag(registers::Flag::Carry, true);

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
        let (mut cpu, mut memory) = cpu_with_program(&[0x21, 0xff, 0x0f, 0x01, 0x01, 0x00, 0x09]);

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
        cpu.registers_mut().set_flag(registers::Flag::Zero, false);

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

    #[test]
    fn executes_cb_rotate_and_reports_memory_cycles() {
        let (mut cpu, mut memory) = cpu_with_program(&[0xcb, 0x00, 0xcb, 0x06]);
        cpu.registers_mut().b = 0x80;
        cpu.registers_mut().set_hl(0xc000);
        memory.write8(0xc000, 0x01);

        assert_eq!(cpu.step(&mut memory).expect("RLC B 应执行成功"), 8);
        assert_eq!(cpu.registers().b, 0x01);
        assert!(cpu.registers().flag(registers::Flag::Carry));
        assert!(!cpu.registers().flag(registers::Flag::Zero));

        assert_eq!(cpu.step(&mut memory).expect("RLC (HL) 应执行成功"), 16);
        assert_eq!(memory.read8(0xc000), 0x02);
        assert!(!cpu.registers().flag(registers::Flag::Carry));
    }

    #[test]
    fn bit_preserves_carry_and_res_set_change_target() {
        let (mut cpu, mut memory) = cpu_with_program(&[0xcb, 0x78, 0xcb, 0xb8, 0xcb, 0xf8]);
        cpu.registers_mut().b = 0x80;
        cpu.registers_mut().set_flag(registers::Flag::Carry, true);

        cpu.step(&mut memory).expect("BIT 7,B 应执行成功");
        assert!(!cpu.registers().flag(registers::Flag::Zero));
        assert!(cpu.registers().flag(registers::Flag::HalfCarry));
        assert!(cpu.registers().flag(registers::Flag::Carry));

        cpu.step(&mut memory).expect("RES 7,B 应执行成功");
        assert_eq!(cpu.registers().b, 0x00);
        cpu.step(&mut memory).expect("SET 7,B 应执行成功");
        assert_eq!(cpu.registers().b, 0x80);
    }

    #[test]
    fn decodes_every_cb_opcode() {
        for opcode in 0u8..=u8::MAX {
            let (mut cpu, mut memory) = cpu_with_program(&[0xcb, opcode]);
            let result = cpu.step(&mut memory);
            assert!(
                result.is_ok(),
                "CB 操作码 0x{opcode:02x} 未实现：{result:?}"
            );
        }
    }

    #[test]
    fn services_highest_priority_interrupt_and_pushes_pc() {
        let (mut cpu, mut memory) = cpu_with_program(&[0x00]);
        cpu.ime = true;
        memory.write8(0xffff, 0x1f);
        memory.write8(0xff0f, 0x15);

        assert_eq!(cpu.step(&mut memory).expect("中断应处理成功"), 20);

        assert_eq!(cpu.registers().pc, 0x0040);
        assert_eq!(cpu.registers().sp, 0xfffc);
        assert_eq!(memory.read16(0xfffc), 0x0100);
        assert_eq!(memory.read8(0xff0f), 0x14);
        assert!(!cpu.ime());
    }

    #[test]
    fn ei_enables_interrupts_after_following_instruction() {
        let (mut cpu, mut memory) = cpu_with_program(&[0xfb, 0x00, 0x00]);
        memory.write8(0xffff, 0x01);
        memory.write8(0xff0f, 0x01);

        assert_eq!(cpu.step(&mut memory).expect("EI 应执行成功"), 4);
        assert!(!cpu.ime());
        assert_eq!(cpu.registers().pc, 0x0101);

        assert_eq!(cpu.step(&mut memory).expect("NOP 应执行成功"), 4);
        assert!(cpu.ime());
        assert_eq!(cpu.registers().pc, 0x0102);

        assert_eq!(cpu.step(&mut memory).expect("中断应处理成功"), 20);
        assert_eq!(cpu.registers().pc, 0x0040);
    }

    #[test]
    fn halt_waits_without_pending_interrupt() {
        let (mut cpu, mut memory) = cpu_with_program(&[0x76, 0x00]);

        assert_eq!(cpu.step(&mut memory).expect("HALT 应执行成功"), 4);
        assert!(cpu.halted());
        assert_eq!(cpu.registers().pc, 0x0101);

        assert_eq!(cpu.step(&mut memory).expect("HALT 等待应成功"), 4);
        assert_eq!(cpu.registers().pc, 0x0101);
    }

    #[test]
    fn pending_interrupt_wakes_halt_and_is_serviced_when_ime_is_set() {
        let (mut cpu, mut memory) = cpu_with_program(&[0x76]);
        cpu.step(&mut memory).expect("HALT 应执行成功");
        cpu.ime = true;
        memory.write8(0xffff, 0x04);
        memory.write8(0xff0f, 0x04);

        assert_eq!(cpu.step(&mut memory).expect("中断应处理成功"), 20);

        assert!(!cpu.halted());
        assert_eq!(cpu.registers().pc, 0x0050);
    }

    #[test]
    fn halt_bug_suppresses_next_opcode_increment() {
        let (mut cpu, mut memory) = cpu_with_program(&[0x76, 0x3e, 0x12]);
        memory.write8(0xffff, 0x01);
        memory.write8(0xff0f, 0x01);

        cpu.step(&mut memory).expect("HALT 应执行成功");
        assert!(!cpu.halted());

        cpu.step(&mut memory).expect("HALT bug 后的 LD 应执行成功");

        assert_eq!(cpu.registers().a, 0x3e);
        assert_eq!(cpu.registers().pc, 0x0102);
    }
}
