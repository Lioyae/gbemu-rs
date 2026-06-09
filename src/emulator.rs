use thiserror::Error;

use crate::{
    bus::Bus,
    cartridge::{Cartridge, CartridgeError},
    cpu::{Cpu, instruction::CpuError},
    joypad::JoypadButton,
    ppu::framebuffer::Framebuffer,
};

#[derive(Debug, Error)]
pub enum EmulatorError {
    #[error(transparent)]
    Cartridge(#[from] CartridgeError),
    #[error(transparent)]
    Cpu(#[from] CpuError),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameRunResult {
    pub cycles: u32,
    pub instructions: u32,
    pub frame_ready: bool,
}

pub struct Emulator {
    cpu: Cpu,
    bus: Bus,
    paused: bool,
}

impl Emulator {
    pub fn from_rom(rom: Vec<u8>) -> Result<Self, EmulatorError> {
        let cartridge = Cartridge::from_bytes(rom)?;
        Ok(Self {
            cpu: Cpu::post_boot(),
            bus: Bus::new(cartridge),
            paused: false,
        })
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn bus(&self) -> &Bus {
        &self.bus
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn step_instruction(&mut self) -> Result<u8, EmulatorError> {
        let cycles = self.cpu.step(&mut self.bus)?;
        self.bus.tick(u32::from(cycles));
        Ok(cycles)
    }

    pub fn run_until_frame(&mut self, cycle_budget: u32) -> Result<FrameRunResult, EmulatorError> {
        if self.paused {
            return Ok(FrameRunResult::default());
        }
        if self.bus.take_frame_ready() {
            return Ok(FrameRunResult {
                frame_ready: true,
                ..FrameRunResult::default()
            });
        }

        let mut result = FrameRunResult::default();
        while result.cycles < cycle_budget {
            result.cycles += u32::from(self.step_instruction()?);
            result.instructions += 1;
            if self.bus.take_frame_ready() {
                result.frame_ready = true;
                break;
            }
        }
        Ok(result)
    }

    pub fn set_button(&mut self, button: JoypadButton, pressed: bool) {
        self.bus.set_button(button, pressed);
    }

    pub fn framebuffer(&self) -> &Framebuffer {
        self.bus.framebuffer()
    }

    pub fn read_memory(&self, address: u16) -> u8 {
        self.bus.read_byte(address)
    }

    pub fn peek_memory(&self, address: u16) -> u8 {
        self.bus.peek_byte(address)
    }

    pub fn write_memory(&mut self, address: u16, value: u8) {
        self.bus.write_byte(address, value);
    }
}

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
        let mut emulator = Emulator::from_rom(test_rom(&[0x00; 64])).expect("测试 ROM 应加载成功");

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

        let result = emulator.run_until_frame(80_000).expect("帧运行应成功");

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

        let result = emulator.run_until_frame(1_000).expect("受限帧运行应成功");

        assert!(!result.frame_ready);
        assert!(result.cycles >= 1_000);
        assert!(result.cycles <= 1_012);
    }

    #[test]
    fn paused_emulator_does_not_run_frame_but_allows_manual_step() {
        let mut emulator = Emulator::from_rom(test_rom(&[0x00])).expect("测试 ROM 应加载成功");
        emulator.set_paused(true);

        let result = emulator.run_until_frame(100).expect("暂停运行应成功");
        assert_eq!(result, FrameRunResult::default());
        assert_eq!(emulator.cpu().registers().pc, 0x0100);

        emulator.step_instruction().expect("手动单步应成功");
        assert_eq!(emulator.cpu().registers().pc, 0x0101);
    }

    #[test]
    fn forwards_button_state_to_joypad_and_interrupts() {
        let mut emulator = Emulator::from_rom(test_rom(&[0x00])).expect("测试 ROM 应加载成功");
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
        let mut emulator = Emulator::from_rom(test_rom(&[0x00])).expect("测试 ROM 应加载成功");

        emulator.write_memory(0xc000, 0x5a);

        assert_eq!(Memory::read8(emulator.bus(), 0xc000), 0x5a);
        assert_eq!(emulator.read_memory(0xe000), 0x5a);
    }
}
