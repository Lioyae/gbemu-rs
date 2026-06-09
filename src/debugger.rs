#[cfg(test)]
mod tests {
    use crate::emulator::Emulator;

    use super::*;

    fn test_emulator(program: &[u8]) -> Emulator {
        let mut rom = vec![0; 32 * 1024];
        rom[0x100..0x100 + program.len()].copy_from_slice(program);
        rom[0x134..0x138].copy_from_slice(b"TEST");
        rom[0x147] = 0x00;
        rom[0x148] = 0x00;
        rom[0x149] = 0x00;
        Emulator::from_rom(rom).expect("测试 ROM 应加载成功")
    }

    #[test]
    fn snapshot_does_not_mutate_emulator_state() {
        let mut emulator = test_emulator(&[0x3e, 0x42, 0x00]);
        emulator.write_memory(0xc000, 0x5a);
        let pc = emulator.cpu().registers().pc;

        let snapshot = Debugger::snapshot(&emulator, 0xc000, 4, 3);

        assert_eq!(emulator.cpu().registers().pc, pc);
        assert_eq!(snapshot.registers.pc, 0x0100);
        assert_eq!(snapshot.memory[0], (0xc000, 0x5a));
        assert_eq!(snapshot.disassembly[0].text, "LD A, $42");
    }

    #[test]
    fn disassembles_immediate_jump_and_cb_instruction_lengths() {
        let emulator = test_emulator(&[0xc3, 0x34, 0x12, 0xcb, 0x7c, 0x00]);

        let instructions = Debugger::disassemble(&emulator, 0x0100, 3);

        assert_eq!(instructions[0].bytes, vec![0xc3, 0x34, 0x12]);
        assert_eq!(instructions[0].text, "JP $1234");
        assert_eq!(instructions[1].bytes, vec![0xcb, 0x7c]);
        assert_eq!(instructions[1].text, "BIT 7, H");
        assert_eq!(instructions[2].bytes, vec![0x00]);
        assert_eq!(instructions[2].text, "NOP");
    }

    #[test]
    fn memory_window_wraps_at_end_of_address_space() {
        let mut emulator = test_emulator(&[0x00]);
        emulator.write_memory(0xfffe, 0xaa);
        emulator.write_memory(0xffff, 0xbb);

        let memory = Debugger::memory_window(&emulator, 0xfffe, 4);

        assert_eq!(
            memory,
            vec![(0xfffe, 0xaa), (0xffff, 0xbb), (0x0000, 0x00), (0x0001, 0x00)]
        );
    }

    #[test]
    fn snapshot_contains_interrupt_and_lcd_state() {
        let mut emulator = test_emulator(&[0x00]);
        emulator.write_memory(0xffff, 0x1f);
        emulator.write_memory(0xff0f, 0x04);

        let snapshot = Debugger::snapshot(&emulator, 0, 1, 1);

        assert_eq!(snapshot.interrupt_enable, 0x1f);
        assert_eq!(snapshot.interrupt_flags & 0x1f, 0x04);
        assert_eq!(snapshot.lcd_mode, 2);
        assert_eq!(snapshot.ly, 0);
        assert!(!snapshot.ime);
        assert!(!snapshot.halted);
    }

    #[test]
    fn formats_relative_target_from_instruction_end() {
        let emulator = test_emulator(&[0x20, 0xfe]);

        let instruction = &Debugger::disassemble(&emulator, 0x0100, 1)[0];

        assert_eq!(instruction.text, "JR NZ, $0100");
    }
}
