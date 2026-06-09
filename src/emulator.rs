#[cfg(test)]
mod tests {
    use crate::{
        cpu::{Memory, registers::Flag},
        joypad::JoypadButton,
    };

    use super::*;

    fn test_rom(program: &[u8]) -> Vec<u8> {
        let mut rom = vec![0; 32 * 1024];
        rom[0x100..0x100 + program.len()].copy_from_slice(program);
        rom[0x134..0x138].copy_from_slice(b"TEST");
        rom[0x147] = 0x00;
        rom[0x148] = 0x00;
        rom[0x149] = 0x00;
        rom
    }

    #[test]
    fn starts_with_post_boot_cpu_and_hardware_state() {
        let emulator = Emulator::from_rom(test_rom(&[0x00])).expect("测试 ROM 应加载成功");

        assert_eq!(emulator.cpu().registers().pc, 0x0100);
        assert_eq!(emulator.cpu().registers().af(), 0x01b0);
        assert_eq!(emulator.read_memory(0xff40), 0x91);
        assert_eq!(emulator.read_memory(0xff47), 0xfc);
        assert!(!emulator.paused());
    }

    #[test]
    fn single_step_advances_cpu_and_hardware_cycles() {
        let mut emulator =
            Emulator::from_rom(test_rom(&[0x00; 64])).expect("测试 ROM 应加载成功");

        for _ in 0..64 {
            assert_eq!(emulator.step_instruction().expect("NOP 应执行成功"), 4);
        }

        assert_eq!(emulator.cpu().registers().pc, 0x0140);
        assert_eq!(emulator.read_memory(0xff04), 1);
    }

    #[test]
    fn run_until_frame_stops_at_frame_boundary() {
        let mut emulator =
            Emulator::from_rom(test_rom(&[0x18, 0xfe])).expect("测试 ROM 应加载成功");

        let result = emulator
            .run_until_frame(80_000)
            .expect("帧运行应成功");

        assert!(result.frame_ready);
        assert!(result.cycles >= 65_664);
        assert!(result.cycles <= 65_676);
        assert!(result.instructions > 0);
    }

    #[test]
    fn frame_run_respects_cycle_budget_when_lcd_is_disabled() {
        let mut emulator =
            Emulator::from_rom(test_rom(&[0x18, 0xfe])).expect("测试 ROM 应加载成功");
        emulator.write_memory(0xff40, 0x00);

        let result = emulator
            .run_until_frame(1_000)
            .expect("受限帧运行应成功");

        assert!(!result.frame_ready);
        assert!(result.cycles >= 1_000);
        assert!(result.cycles <= 1_012);
    }

    #[test]
    fn paused_emulator_does_not_run_frame_but_allows_manual_step() {
        let mut emulator =
            Emulator::from_rom(test_rom(&[0x00])).expect("测试 ROM 应加载成功");
        emulator.set_paused(true);

        let result = emulator
            .run_until_frame(100)
            .expect("暂停运行应成功");
        assert_eq!(result, FrameRunResult::default());
        assert_eq!(emulator.cpu().registers().pc, 0x0100);

        emulator.step_instruction().expect("手动单步应成功");
        assert_eq!(emulator.cpu().registers().pc, 0x0101);
    }

    #[test]
    fn forwards_button_state_to_joypad_and_interrupts() {
        let mut emulator =
            Emulator::from_rom(test_rom(&[0x00])).expect("测试 ROM 应加载成功");
        emulator.write_memory(0xff00, 0x10);
        emulator.write_memory(0xff0f, 0x00);

        emulator.set_button(JoypadButton::A, true);

        assert_eq!(emulator.read_memory(0xff00) & 0x0f, 0x0e);
        assert_ne!(emulator.read_memory(0xff0f) & 0x10, 0);
    }

    #[test]
    fn exposes_framebuffer_without_mutating_cpu_state() {
        let emulator = Emulator::from_rom(test_rom(&[0x00])).expect("测试 ROM 应加载成功");
        let pc = emulator.cpu().registers().pc;

        let framebuffer = emulator.framebuffer();

        assert_eq!(framebuffer.width(), 160);
        assert_eq!(emulator.cpu().registers().pc, pc);
        assert!(emulator.cpu().registers().flag(Flag::Carry));
    }

    #[test]
    fn memory_helpers_route_through_bus() {
        let mut emulator =
            Emulator::from_rom(test_rom(&[0x00])).expect("测试 ROM 应加载成功");

        emulator.write_memory(0xc000, 0x5a);

        assert_eq!(Memory::read8(emulator.bus(), 0xc000), 0x5a);
        assert_eq!(emulator.read_memory(0xe000), 0x5a);
    }
}
