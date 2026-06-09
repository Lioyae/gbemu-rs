use crate::{cpu::registers::Registers, emulator::Emulator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisassembledInstruction {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSnapshot {
    pub registers: Registers,
    pub ime: bool,
    pub halted: bool,
    pub interrupt_enable: u8,
    pub interrupt_flags: u8,
    pub lcd_mode: u8,
    pub ly: u8,
    pub disassembly: Vec<DisassembledInstruction>,
    pub memory: Vec<(u16, u8)>,
}

pub struct Debugger;

impl Debugger {
    pub fn snapshot(
        emulator: &Emulator,
        memory_start: u16,
        memory_length: usize,
        instruction_count: usize,
    ) -> DebugSnapshot {
        let registers = emulator.cpu().registers().clone();
        DebugSnapshot {
            disassembly: Self::disassemble(emulator, registers.pc, instruction_count),
            memory: Self::memory_window(emulator, memory_start, memory_length),
            registers,
            ime: emulator.cpu().ime(),
            halted: emulator.cpu().halted(),
            interrupt_enable: emulator.peek_memory(0xffff),
            interrupt_flags: emulator.peek_memory(0xff0f),
            lcd_mode: emulator.bus().lcd_mode(),
            ly: emulator.bus().ly(),
        }
    }

    pub fn disassemble(
        emulator: &Emulator,
        start: u16,
        count: usize,
    ) -> Vec<DisassembledInstruction> {
        let mut address = start;
        let mut instructions = Vec::with_capacity(count);
        for _ in 0..count {
            let opcode = emulator.peek_memory(address);
            let operand1 = emulator.peek_memory(address.wrapping_add(1));
            let operand2 = emulator.peek_memory(address.wrapping_add(2));
            let (text, length) = decode_instruction(address, opcode, operand1, operand2);
            let bytes = (0..length)
                .map(|offset| emulator.peek_memory(address.wrapping_add(offset as u16)))
                .collect();
            instructions.push(DisassembledInstruction {
                address,
                bytes,
                text,
            });
            address = address.wrapping_add(length as u16);
        }
        instructions
    }

    pub fn memory_window(emulator: &Emulator, start: u16, length: usize) -> Vec<(u16, u8)> {
        (0..length)
            .map(|offset| {
                let address = start.wrapping_add(offset as u16);
                (address, emulator.peek_memory(address))
            })
            .collect()
    }
}

fn decode_instruction(address: u16, opcode: u8, b1: u8, b2: u8) -> (String, usize) {
    const R8: [&str; 8] = ["B", "C", "D", "E", "H", "L", "(HL)", "A"];
    const R16: [&str; 4] = ["BC", "DE", "HL", "SP"];
    const STACK: [&str; 4] = ["BC", "DE", "HL", "AF"];
    const CONDITIONS: [&str; 4] = ["NZ", "Z", "NC", "C"];
    const ALU: [&str; 8] = [
        "ADD A,", "ADC A,", "SUB", "SBC A,", "AND", "XOR", "OR", "CP",
    ];

    let word = u16::from_le_bytes([b1, b2]);
    if (0x40..=0x7f).contains(&opcode) {
        if opcode == 0x76 {
            return ("HALT".to_owned(), 1);
        }
        return (
            format!(
                "LD {}, {}",
                R8[((opcode >> 3) & 7) as usize],
                R8[(opcode & 7) as usize]
            ),
            1,
        );
    }
    if (0x80..=0xbf).contains(&opcode) {
        return (
            format!(
                "{} {}",
                ALU[((opcode >> 3) & 7) as usize],
                R8[(opcode & 7) as usize]
            ),
            1,
        );
    }

    match opcode {
        0x00 => ("NOP".to_owned(), 1),
        value if value <= 0x31 && value & 0x0f == 0x01 => (
            format!("LD {}, ${word:04X}", R16[((value >> 4) & 3) as usize]),
            3,
        ),
        0x02 => ("LD (BC), A".to_owned(), 1),
        0x12 => ("LD (DE), A".to_owned(), 1),
        0x22 => ("LD (HL+), A".to_owned(), 1),
        0x32 => ("LD (HL-), A".to_owned(), 1),
        value if value <= 0x33 && value & 0x0f == 0x03 => {
            (format!("INC {}", R16[((value >> 4) & 3) as usize]), 1)
        }
        value if value <= 0x3c && value & 0x07 == 0x04 => {
            (format!("INC {}", R8[((value >> 3) & 7) as usize]), 1)
        }
        value if value <= 0x3d && value & 0x07 == 0x05 => {
            (format!("DEC {}", R8[((value >> 3) & 7) as usize]), 1)
        }
        value if value <= 0x3e && value & 0x07 == 0x06 => (
            format!("LD {}, ${b1:02X}", R8[((value >> 3) & 7) as usize]),
            2,
        ),
        0x07 => ("RLCA".to_owned(), 1),
        0x08 => (format!("LD (${word:04X}), SP"), 3),
        value if value <= 0x39 && value & 0x0f == 0x09 => {
            (format!("ADD HL, {}", R16[((value >> 4) & 3) as usize]), 1)
        }
        0x0a => ("LD A, (BC)".to_owned(), 1),
        0x1a => ("LD A, (DE)".to_owned(), 1),
        0x2a => ("LD A, (HL+)".to_owned(), 1),
        0x3a => ("LD A, (HL-)".to_owned(), 1),
        value if value <= 0x3b && value & 0x0f == 0x0b => {
            (format!("DEC {}", R16[((value >> 4) & 3) as usize]), 1)
        }
        0x0f => ("RRCA".to_owned(), 1),
        0x10 => ("STOP".to_owned(), 2),
        0x17 => ("RLA".to_owned(), 1),
        0x18 => (format!("JR ${:04X}", relative_target(address, b1)), 2),
        0x1f => ("RRA".to_owned(), 1),
        0x20 | 0x28 | 0x30 | 0x38 => (
            format!(
                "JR {}, ${:04X}",
                CONDITIONS[((opcode >> 3) & 3) as usize],
                relative_target(address, b1)
            ),
            2,
        ),
        0x27 => ("DAA".to_owned(), 1),
        0x2f => ("CPL".to_owned(), 1),
        0x37 => ("SCF".to_owned(), 1),
        0x3f => ("CCF".to_owned(), 1),
        0xc0 | 0xc8 | 0xd0 | 0xd8 => (
            format!("RET {}", CONDITIONS[((opcode >> 3) & 3) as usize]),
            1,
        ),
        0xc1 | 0xd1 | 0xe1 | 0xf1 => (format!("POP {}", STACK[((opcode >> 4) & 3) as usize]), 1),
        0xc2 | 0xca | 0xd2 | 0xda => (
            format!(
                "JP {}, ${word:04X}",
                CONDITIONS[((opcode >> 3) & 3) as usize]
            ),
            3,
        ),
        0xc3 => (format!("JP ${word:04X}"), 3),
        0xc4 | 0xcc | 0xd4 | 0xdc => (
            format!(
                "CALL {}, ${word:04X}",
                CONDITIONS[((opcode >> 3) & 3) as usize]
            ),
            3,
        ),
        0xc5 | 0xd5 | 0xe5 | 0xf5 => (format!("PUSH {}", STACK[((opcode >> 4) & 3) as usize]), 1),
        0xc6 | 0xce | 0xd6 | 0xde | 0xe6 | 0xee | 0xf6 | 0xfe => (
            format!("{} ${b1:02X}", ALU[((opcode >> 3) & 7) as usize]),
            2,
        ),
        0xc7 | 0xcf | 0xd7 | 0xdf | 0xe7 | 0xef | 0xf7 | 0xff => {
            (format!("RST ${:02X}", opcode & 0x38), 1)
        }
        0xc9 => ("RET".to_owned(), 1),
        0xcb => (decode_cb(b1), 2),
        0xcd => (format!("CALL ${word:04X}"), 3),
        0xd9 => ("RETI".to_owned(), 1),
        0xe0 => (format!("LDH ($FF{b1:02X}), A"), 2),
        0xe2 => ("LD ($FF00+C), A".to_owned(), 1),
        0xe8 => (format!("ADD SP, {:+}", b1 as i8), 2),
        0xe9 => ("JP (HL)".to_owned(), 1),
        0xea => (format!("LD (${word:04X}), A"), 3),
        0xf0 => (format!("LDH A, ($FF{b1:02X})"), 2),
        0xf2 => ("LD A, ($FF00+C)".to_owned(), 1),
        0xf3 => ("DI".to_owned(), 1),
        0xf8 => (format!("LD HL, SP{:+}", b1 as i8), 2),
        0xf9 => ("LD SP, HL".to_owned(), 1),
        0xfa => (format!("LD A, (${word:04X})"), 3),
        0xfb => ("EI".to_owned(), 1),
        _ => (format!("DB ${opcode:02X}"), 1),
    }
}

fn decode_cb(opcode: u8) -> String {
    const R8: [&str; 8] = ["B", "C", "D", "E", "H", "L", "(HL)", "A"];
    const ROTATE: [&str; 8] = ["RLC", "RRC", "RL", "RR", "SLA", "SRA", "SWAP", "SRL"];
    let group = opcode >> 6;
    let operation = (opcode >> 3) & 7;
    let target = R8[(opcode & 7) as usize];
    match group {
        0 => format!("{} {target}", ROTATE[operation as usize]),
        1 => format!("BIT {operation}, {target}"),
        2 => format!("RES {operation}, {target}"),
        3 => format!("SET {operation}, {target}"),
        _ => unreachable!("CB 指令组始终为 0..=3"),
    }
}

fn relative_target(address: u16, offset: u8) -> u16 {
    address
        .wrapping_add(2)
        .wrapping_add_signed(offset as i8 as i16)
}

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
            vec![
                (0xfffe, 0xaa),
                (0xffff, 0xbb),
                (0x0000, 0x00),
                (0x0001, 0x00)
            ]
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
