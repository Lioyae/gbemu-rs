use gbmeu::{
    boot::{BootError, BootRoms},
    emulator::Emulator,
    model::{HardwareModel, ModelPreference},
};
use std::process::Command;

fn dmg_rom() -> Vec<u8> {
    let mut rom = vec![0; 32 * 1024];
    rom[0x0000] = 0x42;
    rom[0x0100] = 0x00;
    rom[0x134..0x138].copy_from_slice(b"BOOT");
    rom[0x147] = 0x00;
    rom[0x148] = 0x00;
    rom[0x149] = 0x00;
    rom
}

fn cgb_rom() -> Vec<u8> {
    let mut rom = dmg_rom();
    rom[0x0100] = 0x77;
    rom[0x0200] = 0x55;
    rom[0x143] = 0xc0;
    rom
}

#[test]
fn rejects_boot_rom_with_wrong_size() {
    let result = BootRoms::from_bytes(Some(vec![0; 255]), None);

    assert!(matches!(
        result,
        Err(BootError::InvalidLength {
            model: HardwareModel::Dmg,
            ..
        })
    ));
}

#[test]
fn dmg_boot_rom_maps_low_page_until_ff50_disables_it() {
    let mut boot = vec![0; 0x100];
    boot[0] = 0x99;
    let boots = BootRoms::from_bytes(Some(boot), None).expect("DMG Boot ROM 应有效");
    let mut emulator = Emulator::from_rom_with_boot_roms(dmg_rom(), ModelPreference::Auto, boots)
        .expect("DMG ROM 应使用 Boot ROM 启动");

    assert_eq!(emulator.cpu().registers().pc, 0x0000);
    assert_eq!(emulator.read_memory(0x0000), 0x99);

    emulator.write_memory(0xff50, 0x01);
    assert_eq!(emulator.read_memory(0x0000), 0x42);
    emulator.write_memory(0xff50, 0x00);
    assert_eq!(emulator.read_memory(0x0000), 0x42);
}

#[test]
fn cgb_boot_rom_leaves_cartridge_header_window_visible() {
    let mut boot = vec![0; 0x900];
    boot[0] = 0x11;
    boot[0x200] = 0x22;
    let boots = BootRoms::from_bytes(None, Some(boot)).expect("CGB Boot ROM 应有效");
    let emulator = Emulator::from_rom_with_boot_roms(cgb_rom(), ModelPreference::Auto, boots)
        .expect("CGB ROM 应使用 Boot ROM 启动");

    assert_eq!(emulator.model(), HardwareModel::Cgb);
    assert_eq!(emulator.read_memory(0x0000), 0x11);
    assert_eq!(emulator.read_memory(0x0100), 0x77);
    assert_eq!(emulator.read_memory(0x0200), 0x22);
}

#[test]
fn command_line_exposes_model_and_boot_rom_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_gbmeu"))
        .arg("--help")
        .output()
        .expect("命令行帮助应可运行");
    let stdout = String::from_utf8(output.stdout).expect("帮助文本应为 UTF-8");

    assert!(output.status.success());
    assert!(stdout.contains("--model"));
    assert!(stdout.contains("--dmg-boot-rom"));
    assert!(stdout.contains("--cgb-boot-rom"));
}

#[test]
fn rejects_boot_rom_that_does_not_match_selected_hardware() {
    let boots = BootRoms::from_bytes(Some(vec![0; 0x100]), None).expect("DMG Boot ROM 应有效");
    let result = Emulator::from_rom_with_boot_roms(cgb_rom(), ModelPreference::Auto, boots);

    assert!(result.is_err());
}
