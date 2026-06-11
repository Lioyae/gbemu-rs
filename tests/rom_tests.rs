mod common;

use std::fs;

use common::{RomTestStatus, run_rom_bytes, run_rom_file};
use gbmeu::emulator::Emulator;

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
fn headless_runner_detects_uppercase_serial_failure() {
    let result = run_rom_bytes(serial_test_rom("FAILED"), 10_000).expect("测试 ROM 应运行成功");

    assert_eq!(result.status, RomTestStatus::Failed);
    assert_eq!(result.serial_output, "FAILED");
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

macro_rules! provided_serial_rom_test {
    ($name:ident, $path:literal, $cycle_limit:expr) => {
        #[test]
        #[ignore = "需要用户提供 roms 目录中的外部测试 ROM"]
        fn $name() {
            let result = run_rom_file($path, $cycle_limit).expect("测试 ROM 应运行成功");
            println!(
                "{}：状态 {:?}，周期 {}，{}\n结果内存：{:02X?}\n串口输出：{}",
                $path,
                result.status,
                result.cycles,
                result.cpu_state,
                result.result_memory,
                result.serial_output
            );
            assert_eq!(
                result.status,
                RomTestStatus::Passed,
                "{} 串口输出：{}",
                $path,
                result.serial_output
            );
        }
    };
}

provided_serial_rom_test!(provided_01_special, "roms/01-special.gb", 50_000_000);
provided_serial_rom_test!(provided_02_interrupts, "roms/02-interrupts.gb", 50_000_000);
provided_serial_rom_test!(provided_03_op_sp_hl, "roms/03-op sp,hl.gb", 50_000_000);
provided_serial_rom_test!(provided_04_op_r_imm, "roms/04-op r,imm.gb", 50_000_000);
provided_serial_rom_test!(provided_05_op_rp, "roms/05-op rp.gb", 50_000_000);
provided_serial_rom_test!(provided_06_ld_r_r, "roms/06-ld r,r.gb", 50_000_000);
provided_serial_rom_test!(
    provided_07_control_flow,
    "roms/07-jr,jp,call,ret,rst.gb",
    50_000_000
);
provided_serial_rom_test!(
    provided_08_misc_instructions,
    "roms/08-misc instrs.gb",
    50_000_000
);
provided_serial_rom_test!(provided_09_op_r_r, "roms/09-op r,r.gb", 50_000_000);
provided_serial_rom_test!(provided_10_bit_ops, "roms/10-bit ops.gb", 300_000_000);
provided_serial_rom_test!(provided_11_op_a_hl, "roms/11-op a,(hl).gb", 300_000_000);
provided_serial_rom_test!(provided_cpu_instrs, "roms/cpu_instrs.gb", 300_000_000);
provided_serial_rom_test!(provided_mem_timing, "roms/mem_timing.gb", 200_000_000);

#[test]
#[ignore = "需要用户提供 roms/dmg-acid2.gb"]
fn provided_dmg_acid2_smoke() {
    run_frame_smoke("roms/dmg-acid2.gb", 120);
}

#[test]
#[ignore = "需要用户提供 roms/zg.gb"]
fn provided_mbc1_game_smoke() {
    run_frame_smoke("roms/zg.gb", 300);
}

#[test]
#[ignore = "需要用户提供 roms/zelda1.gbc"]
fn provided_cgb_game_smoke() {
    run_frame_smoke("roms/zelda1.gbc", 300);
}

#[test]
#[ignore = "需要用户提供 roms/Pocket-Yellow.gb"]
fn provided_large_mbc_game_smoke() {
    run_frame_smoke("roms/Pocket-Yellow.gb", 300);
}

#[test]
#[ignore = "需要用户提供 roms/GB-War.gb"]
fn provided_gb_war_smoke() {
    run_frame_smoke("roms/GB-War.gb", 300);
}

fn run_frame_smoke(path: &str, frame_count: usize) {
    let rom = fs::read(path).expect("应能读取外部 ROM");
    let mut emulator = Emulator::from_rom(rom).expect("外部 ROM 应能加载");
    let mut completed_frames = 0;
    let mut total_cycles = 0u64;

    for _ in 0..frame_count {
        let result = emulator
            .run_until_frame(80_000)
            .expect("外部 ROM 运行时不应出现 CPU 错误");
        total_cycles += u64::from(result.cycles);
        completed_frames += usize::from(result.frame_ready);
    }

    println!("{path}：完成 {completed_frames}/{frame_count} 帧，共 {total_cycles} 周期");
    assert!(completed_frames > 0, "{path} 未产生任何画面帧");
}
