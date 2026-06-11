#[path = "common/test_rom.rs"]
mod test_rom;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use gbmeu::{
    emulator::Emulator,
    save::{StatePaths, StateSave},
};
use test_rom::TestRom;

fn test_rom(title: &[u8]) -> Vec<u8> {
    TestRom::new(std::str::from_utf8(title).expect("测试 ROM 标题应为 UTF-8"))
        .write(0x0100, &[0x3e, 0x42, 0xea, 0x00, 0xc0, 0x18, 0xf9])
        .build()
}

fn temporary_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("系统时间应有效")
        .as_nanos();
    std::env::temp_dir().join(format!("gbmeu-{name}-{}-{nonce}", std::process::id()))
}

fn cleanup(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).expect("临时目录应清理成功");
    }
}

#[test]
fn state_slot_path_uses_rom_name_and_slot_number() {
    let paths = StatePaths::for_rom(Path::new("games/example.gbc"), 3).expect("有效槽位应生成路径");

    assert_eq!(paths.state, PathBuf::from("games/example.state3"));
    assert!(StatePaths::for_rom(Path::new("game.gb"), 10).is_err());
}

#[test]
fn state_round_trip_restores_complete_runtime_state() {
    let directory = temporary_directory("state-round-trip");
    fs::create_dir_all(&directory).expect("临时目录应创建成功");
    let rom_path = directory.join("game.gb");
    let mut emulator = Emulator::from_rom(test_rom(b"STATE")).expect("测试 ROM 应加载成功");
    emulator.write_memory(0xc123, 0x5a);
    emulator.step_instruction().expect("测试指令应执行成功");
    let saved_pc = emulator.cpu().registers().pc;

    StateSave::save(&emulator, &rom_path, 4).expect("即时存档应保存成功");
    emulator.write_memory(0xc123, 0x11);
    emulator.step_instruction().expect("测试指令应执行成功");

    StateSave::load(&mut emulator, &rom_path, 4).expect("即时存档应加载成功");

    assert_eq!(emulator.read_memory(0xc123), 0x5a);
    assert_eq!(emulator.cpu().registers().pc, saved_pc);
    cleanup(&directory);
}

#[test]
fn state_from_another_rom_is_rejected_without_modifying_emulator() {
    let directory = temporary_directory("state-rom-mismatch");
    fs::create_dir_all(&directory).expect("临时目录应创建成功");
    let rom_path = directory.join("game.gb");
    let source = Emulator::from_rom(test_rom(b"FIRST")).expect("源 ROM 应加载成功");
    StateSave::save(&source, &rom_path, 0).expect("即时存档应保存成功");

    let mut target = Emulator::from_rom(test_rom(b"SECOND")).expect("目标 ROM 应加载成功");
    target.write_memory(0xc000, 0x77);
    let result = StateSave::load(&mut target, &rom_path, 0);

    assert!(result.is_err());
    assert_eq!(target.read_memory(0xc000), 0x77);
    cleanup(&directory);
}

#[test]
fn corrupt_state_is_rejected_without_modifying_emulator() {
    let directory = temporary_directory("state-corrupt");
    fs::create_dir_all(&directory).expect("临时目录应创建成功");
    let rom_path = directory.join("game.gb");
    let paths = StatePaths::for_rom(&rom_path, 9).expect("有效槽位应生成路径");
    fs::write(&paths.state, b"not a state").expect("损坏状态文件应写入成功");
    let mut emulator = Emulator::from_rom(test_rom(b"STATE")).expect("测试 ROM 应加载成功");
    emulator.write_memory(0xc000, 0x66);

    let result = StateSave::load(&mut emulator, &rom_path, 9);

    assert!(result.is_err());
    assert_eq!(emulator.read_memory(0xc000), 0x66);
    cleanup(&directory);
}

#[test]
fn modified_payload_is_rejected_even_when_it_can_still_be_decoded() {
    const HEADER_SIZE: usize = 80;

    let directory = temporary_directory("state-checksum");
    fs::create_dir_all(&directory).expect("临时目录应创建成功");
    let rom_path = directory.join("game.gb");
    let source = Emulator::from_rom(test_rom(b"STATE")).expect("源 ROM 应加载成功");
    StateSave::save(&source, &rom_path, 2).expect("即时存档应保存成功");
    let paths = StatePaths::for_rom(&rom_path, 2).expect("有效槽位应生成路径");
    let mut bytes = fs::read(&paths.state).expect("即时存档应读取成功");
    bytes[HEADER_SIZE] ^= 0x01;
    fs::write(&paths.state, bytes).expect("修改后的即时存档应写回成功");

    let mut target = Emulator::from_rom(test_rom(b"STATE")).expect("目标 ROM 应加载成功");
    target.write_memory(0xc000, 0x55);
    let result = StateSave::load(&mut target, &rom_path, 2);

    assert!(result.is_err());
    assert_eq!(target.read_memory(0xc000), 0x55);
    cleanup(&directory);
}
