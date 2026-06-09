mod common;

use common::{RomTestStatus, run_rom_bytes, run_rom_file};

fn serial_test_rom(message: &str) -> Vec<u8> {
    let mut rom = vec![0; 32 * 1024];
    let mut cursor = 0x0100;
    for byte in message.bytes() {
        let instructions = [0x3e, byte, 0xe0, 0x01, 0x3e, 0x81, 0xe0, 0x02];
        rom[cursor..cursor + instructions.len()].copy_from_slice(&instructions);
        cursor += instructions.len();
    }
    rom[cursor..cursor + 2].copy_from_slice(&[0x18, 0xfe]);
    rom[0x134..0x138].copy_from_slice(b"TEST");
    rom[0x147] = 0x00;
    rom[0x148] = 0x00;
    rom[0x149] = 0x00;
    rom
}

#[test]
fn headless_runner_detects_serial_pass() {
    let result = run_rom_bytes(serial_test_rom("Passed"), 10_000).expect("测试 ROM 应运行成功");

    assert_eq!(result.status, RomTestStatus::Passed);
    assert_eq!(result.serial_output, "Passed");
    assert!(result.cycles < 10_000);
}

#[test]
fn headless_runner_detects_serial_failure() {
    let result = run_rom_bytes(serial_test_rom("Failed"), 10_000).expect("测试 ROM 应运行成功");

    assert_eq!(result.status, RomTestStatus::Failed);
    assert_eq!(result.serial_output, "Failed");
}

#[test]
fn headless_runner_stops_at_cycle_limit() {
    let result = run_rom_bytes(serial_test_rom("Wait"), 1_000).expect("测试 ROM 应运行成功");

    assert_eq!(result.status, RomTestStatus::TimedOut);
    assert!(result.cycles >= 1_000);
}

#[test]
#[ignore = "需要用户将 Blargg cpu_instrs.gb 放入 test-roms/blargg/"]
fn blargg_cpu_instrs() {
    let result =
        run_rom_file("test-roms/blargg/cpu_instrs.gb", 200_000_000).expect("测试 ROM 应运行成功");

    assert_eq!(
        result.status,
        RomTestStatus::Passed,
        "串口输出：{}",
        result.serial_output
    );
}

#[test]
#[ignore = "需要用户将 Blargg instr_timing.gb 放入 test-roms/blargg/"]
fn blargg_instruction_timing() {
    let result =
        run_rom_file("test-roms/blargg/instr_timing.gb", 100_000_000).expect("测试 ROM 应运行成功");

    assert_eq!(
        result.status,
        RomTestStatus::Passed,
        "串口输出：{}",
        result.serial_output
    );
}
