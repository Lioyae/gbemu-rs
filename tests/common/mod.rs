use std::{fs, path::Path};

use gbmeu::emulator::{Emulator, EmulatorError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomTestStatus {
    Passed,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomTestResult {
    pub status: RomTestStatus,
    pub serial_output: String,
    pub cycles: u64,
    pub cpu_state: String,
    pub result_memory: Vec<u8>,
}

pub fn run_rom_bytes(rom: Vec<u8>, cycle_limit: u64) -> Result<RomTestResult, EmulatorError> {
    let mut emulator = Emulator::from_rom(rom)?;
    let mut output = String::new();
    let mut cycles = 0u64;

    while cycles < cycle_limit {
        cycles += u64::from(emulator.step_instruction()?);
        if emulator.read_memory(0xff02) == 0x81 {
            output.push(char::from(emulator.read_memory(0xff01)));
            emulator.write_memory(0xff02, 0x00);
        }

        if output.contains("Passed") {
            return Ok(result(&emulator, RomTestStatus::Passed, output, cycles));
        }
        if output.contains("Failed") || output.contains("FAILED") {
            return Ok(result(&emulator, RomTestStatus::Failed, output, cycles));
        }
    }

    Ok(result(&emulator, RomTestStatus::TimedOut, output, cycles))
}

pub fn run_rom_file(
    path: impl AsRef<Path>,
    cycle_limit: u64,
) -> Result<RomTestResult, Box<dyn std::error::Error>> {
    let path = path.as_ref();
    let rom = fs::read(path).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!("无法读取测试 ROM {}：{error}", path.display()),
        )
    })?;
    Ok(run_rom_bytes(rom, cycle_limit)?)
}

fn result(
    emulator: &Emulator,
    status: RomTestStatus,
    serial_output: String,
    cycles: u64,
) -> RomTestResult {
    let registers = emulator.cpu().registers();
    RomTestResult {
        status,
        serial_output,
        cycles,
        cpu_state: format!(
            "PC={:04X} SP={:04X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} IME={} HALT={}",
            registers.pc,
            registers.sp,
            registers.af(),
            registers.bc(),
            registers.de(),
            registers.hl(),
            emulator.cpu().ime(),
            emulator.cpu().halted(),
        ),
        result_memory: (0xa000..0xa010)
            .map(|address| emulator.peek_memory(address))
            .collect(),
    }
}
