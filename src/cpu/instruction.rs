use thiserror::Error;

use super::{Cpu, Memory, registers::Flag};

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CpuError {
    #[error("无效的 LR35902 操作码：0x{0:02x}")]
    InvalidOpcode(u8),
}

impl Cpu {
    pub fn step<M: Memory>(&mut self, memory: &mut M) -> Result<u8, CpuError> {
        self.instruction_cycles = 0;
        let pending = self.pending_interrupts(memory);
        if self.halted {
            if pending == 0 {
                self.idle_bus(memory);
                return Ok(4);
            }
            self.halted = false;
        }

        if self.ime && pending != 0 {
            let cycles = self.service_interrupt(memory, pending);
            self.finish_cycles(memory, cycles);
            return Ok(cycles);
        }

        let opcode = self.fetch_byte(memory);
        let cycles = self.execute_base(memory, opcode)?;
        self.advance_ime_delay();
        self.finish_cycles(memory, cycles);
        Ok(cycles)
    }

    fn execute_base<M: Memory>(&mut self, memory: &mut M, opcode: u8) -> Result<u8, CpuError> {
        if (0x40..=0x7f).contains(&opcode) {
            if opcode == 0x76 {
                if !self.ime && self.pending_interrupts(memory) != 0 {
                    self.halt_bug = true;
                } else {
                    self.halted = true;
                }
                return Ok(4);
            }
            let target = (opcode >> 3) & 0x07;
            let source = opcode & 0x07;
            let value = self.read_r8(memory, source);
            self.write_r8(memory, target, value);
            return Ok(if source == 6 || target == 6 { 8 } else { 4 });
        }

        if (0x80..=0xbf).contains(&opcode) {
            let operation = (opcode >> 3) & 0x07;
            let source = opcode & 0x07;
            let value = self.read_r8(memory, source);
            self.execute_alu(operation, value);
            return Ok(if source == 6 { 8 } else { 4 });
        }

        match opcode {
            0x00 => Ok(4),
            opcode if opcode <= 0x31 && opcode & 0x0f == 0x01 => {
                let value = self.fetch_word(memory);
                self.write_r16((opcode >> 4) & 0x03, value);
                Ok(12)
            }
            0x02 => {
                self.write_bus(memory, self.registers.bc(), self.registers.a);
                Ok(8)
            }
            0x12 => {
                self.write_bus(memory, self.registers.de(), self.registers.a);
                Ok(8)
            }
            0x22 => {
                let address = self.registers.hl();
                self.write_bus(memory, address, self.registers.a);
                self.registers.set_hl(address.wrapping_add(1));
                Ok(8)
            }
            0x32 => {
                let address = self.registers.hl();
                self.write_bus(memory, address, self.registers.a);
                self.registers.set_hl(address.wrapping_sub(1));
                Ok(8)
            }
            opcode if opcode <= 0x33 && opcode & 0x0f == 0x03 => {
                let index = (opcode >> 4) & 0x03;
                let value = self.read_r16(index).wrapping_add(1);
                self.write_r16(index, value);
                Ok(8)
            }
            opcode if opcode <= 0x3c && opcode & 0x07 == 0x04 => {
                let index = (opcode >> 3) & 0x07;
                let value = self.read_r8(memory, index);
                let result = self.increment(value);
                self.write_r8(memory, index, result);
                Ok(if index == 6 { 12 } else { 4 })
            }
            opcode if opcode <= 0x3d && opcode & 0x07 == 0x05 => {
                let index = (opcode >> 3) & 0x07;
                let value = self.read_r8(memory, index);
                let result = self.decrement(value);
                self.write_r8(memory, index, result);
                Ok(if index == 6 { 12 } else { 4 })
            }
            opcode if opcode <= 0x3e && opcode & 0x07 == 0x06 => {
                let index = (opcode >> 3) & 0x07;
                let value = self.fetch_byte(memory);
                self.write_r8(memory, index, value);
                Ok(if index == 6 { 12 } else { 8 })
            }
            0x07 => {
                let carry = self.registers.a & 0x80 != 0;
                self.registers.a = self.registers.a.rotate_left(1);
                self.set_rotate_flags(carry);
                Ok(4)
            }
            0x08 => {
                let address = self.fetch_word(memory);
                self.write_bus(memory, address, self.registers.sp as u8);
                self.write_bus(
                    memory,
                    address.wrapping_add(1),
                    (self.registers.sp >> 8) as u8,
                );
                Ok(20)
            }
            opcode if opcode <= 0x39 && opcode & 0x0f == 0x09 => {
                let value = self.read_r16((opcode >> 4) & 0x03);
                self.add_hl(value);
                Ok(8)
            }
            0x0a => {
                self.registers.a = self.read_bus(memory, self.registers.bc());
                Ok(8)
            }
            0x1a => {
                self.registers.a = self.read_bus(memory, self.registers.de());
                Ok(8)
            }
            0x2a => {
                let address = self.registers.hl();
                self.registers.a = self.read_bus(memory, address);
                self.registers.set_hl(address.wrapping_add(1));
                Ok(8)
            }
            0x3a => {
                let address = self.registers.hl();
                self.registers.a = self.read_bus(memory, address);
                self.registers.set_hl(address.wrapping_sub(1));
                Ok(8)
            }
            opcode if opcode <= 0x3b && opcode & 0x0f == 0x0b => {
                let index = (opcode >> 4) & 0x03;
                let value = self.read_r16(index).wrapping_sub(1);
                self.write_r16(index, value);
                Ok(8)
            }
            0x0f => {
                let carry = self.registers.a & 0x01 != 0;
                self.registers.a = self.registers.a.rotate_right(1);
                self.set_rotate_flags(carry);
                Ok(4)
            }
            0x10 => {
                self.registers.pc = self.registers.pc.wrapping_add(1);
                self.halted = true;
                Ok(4)
            }
            0x17 => {
                let old_carry = u8::from(self.registers.flag(Flag::Carry));
                let new_carry = self.registers.a & 0x80 != 0;
                self.registers.a = (self.registers.a << 1) | old_carry;
                self.set_rotate_flags(new_carry);
                Ok(4)
            }
            0x18 => {
                self.jump_relative(memory);
                Ok(12)
            }
            0x1f => {
                let old_carry = u8::from(self.registers.flag(Flag::Carry));
                let new_carry = self.registers.a & 0x01 != 0;
                self.registers.a = (self.registers.a >> 1) | (old_carry << 7);
                self.set_rotate_flags(new_carry);
                Ok(4)
            }
            0x20 | 0x28 | 0x30 | 0x38 => {
                let offset = self.fetch_byte(memory) as i8;
                if self.condition((opcode >> 3) & 0x03) {
                    self.registers.pc = self.registers.pc.wrapping_add_signed(offset as i16);
                    Ok(12)
                } else {
                    Ok(8)
                }
            }
            0x27 => {
                self.decimal_adjust();
                Ok(4)
            }
            0x2f => {
                self.registers.a = !self.registers.a;
                self.registers.set_flag(Flag::Subtract, true);
                self.registers.set_flag(Flag::HalfCarry, true);
                Ok(4)
            }
            0x37 => {
                self.registers.set_flag(Flag::Subtract, false);
                self.registers.set_flag(Flag::HalfCarry, false);
                self.registers.set_flag(Flag::Carry, true);
                Ok(4)
            }
            0x3f => {
                let carry = !self.registers.flag(Flag::Carry);
                self.registers.set_flag(Flag::Subtract, false);
                self.registers.set_flag(Flag::HalfCarry, false);
                self.registers.set_flag(Flag::Carry, carry);
                Ok(4)
            }
            0xc0 | 0xc8 | 0xd0 | 0xd8 => {
                if self.condition((opcode >> 3) & 0x03) {
                    self.idle_bus(memory);
                    self.registers.pc = self.pop(memory);
                    Ok(20)
                } else {
                    Ok(8)
                }
            }
            0xc1 | 0xd1 | 0xe1 | 0xf1 => {
                let value = self.pop(memory);
                self.write_stack_pair((opcode >> 4) & 0x03, value);
                Ok(12)
            }
            0xc2 | 0xca | 0xd2 | 0xda => {
                let address = self.fetch_word(memory);
                if self.condition((opcode >> 3) & 0x03) {
                    self.registers.pc = address;
                    Ok(16)
                } else {
                    Ok(12)
                }
            }
            0xc3 => {
                self.registers.pc = self.fetch_word(memory);
                Ok(16)
            }
            0xc4 | 0xcc | 0xd4 | 0xdc => {
                let address = self.fetch_word(memory);
                if self.condition((opcode >> 3) & 0x03) {
                    self.idle_bus(memory);
                    self.push(memory, self.registers.pc);
                    self.registers.pc = address;
                    Ok(24)
                } else {
                    Ok(12)
                }
            }
            0xc5 | 0xd5 | 0xe5 | 0xf5 => {
                let value = self.read_stack_pair((opcode >> 4) & 0x03);
                self.idle_bus(memory);
                self.push(memory, value);
                Ok(16)
            }
            0xc6 | 0xce | 0xd6 | 0xde | 0xe6 | 0xee | 0xf6 | 0xfe => {
                let value = self.fetch_byte(memory);
                self.execute_alu((opcode >> 3) & 0x07, value);
                Ok(8)
            }
            0xc7 | 0xcf | 0xd7 | 0xdf | 0xe7 | 0xef | 0xf7 | 0xff => {
                self.idle_bus(memory);
                self.push(memory, self.registers.pc);
                self.registers.pc = (opcode & 0x38) as u16;
                Ok(16)
            }
            0xc9 => {
                self.registers.pc = self.pop(memory);
                Ok(16)
            }
            0xcb => {
                let extended_opcode = self.fetch_byte(memory);
                Ok(self.execute_cb(memory, extended_opcode))
            }
            0xcd => {
                let address = self.fetch_word(memory);
                self.idle_bus(memory);
                self.push(memory, self.registers.pc);
                self.registers.pc = address;
                Ok(24)
            }
            0xd9 => {
                self.registers.pc = self.pop(memory);
                self.ime = true;
                self.ime_enable_delay = 0;
                Ok(16)
            }
            0xe0 => {
                let address = 0xff00 | self.fetch_byte(memory) as u16;
                self.write_bus(memory, address, self.registers.a);
                Ok(12)
            }
            0xe2 => {
                self.write_bus(memory, 0xff00 | self.registers.c as u16, self.registers.a);
                Ok(8)
            }
            0xe8 => {
                let offset = self.fetch_byte(memory) as i8;
                self.registers.sp = self.add_signed_to_sp(offset);
                Ok(16)
            }
            0xe9 => {
                self.registers.pc = self.registers.hl();
                Ok(4)
            }
            0xea => {
                let address = self.fetch_word(memory);
                self.write_bus(memory, address, self.registers.a);
                Ok(16)
            }
            0xf0 => {
                let address = 0xff00 | self.fetch_byte(memory) as u16;
                self.registers.a = self.read_bus(memory, address);
                Ok(12)
            }
            0xf2 => {
                self.registers.a = self.read_bus(memory, 0xff00 | self.registers.c as u16);
                Ok(8)
            }
            0xf3 => {
                self.ime = false;
                self.ime_enable_delay = 0;
                Ok(4)
            }
            0xf8 => {
                let offset = self.fetch_byte(memory) as i8;
                let result = self.add_signed_to_sp(offset);
                self.registers.set_hl(result);
                Ok(12)
            }
            0xf9 => {
                self.registers.sp = self.registers.hl();
                Ok(8)
            }
            0xfa => {
                let address = self.fetch_word(memory);
                self.registers.a = self.read_bus(memory, address);
                Ok(16)
            }
            0xfb => {
                self.ime_enable_delay = 2;
                Ok(4)
            }
            _ => Err(CpuError::InvalidOpcode(opcode)),
        }
    }

    fn read_r8<M: Memory>(&mut self, memory: &mut M, index: u8) -> u8 {
        match index {
            0 => self.registers.b,
            1 => self.registers.c,
            2 => self.registers.d,
            3 => self.registers.e,
            4 => self.registers.h,
            5 => self.registers.l,
            6 => self.read_bus(memory, self.registers.hl()),
            7 => self.registers.a,
            _ => unreachable!("8 位寄存器索引始终为 0..=7"),
        }
    }

    fn write_r8<M: Memory>(&mut self, memory: &mut M, index: u8, value: u8) {
        match index {
            0 => self.registers.b = value,
            1 => self.registers.c = value,
            2 => self.registers.d = value,
            3 => self.registers.e = value,
            4 => self.registers.h = value,
            5 => self.registers.l = value,
            6 => self.write_bus(memory, self.registers.hl(), value),
            7 => self.registers.a = value,
            _ => unreachable!("8 位寄存器索引始终为 0..=7"),
        }
    }

    fn read_r16(&self, index: u8) -> u16 {
        match index {
            0 => self.registers.bc(),
            1 => self.registers.de(),
            2 => self.registers.hl(),
            3 => self.registers.sp,
            _ => unreachable!("16 位寄存器索引始终为 0..=3"),
        }
    }

    fn write_r16(&mut self, index: u8, value: u16) {
        match index {
            0 => self.registers.set_bc(value),
            1 => self.registers.set_de(value),
            2 => self.registers.set_hl(value),
            3 => self.registers.sp = value,
            _ => unreachable!("16 位寄存器索引始终为 0..=3"),
        }
    }

    fn read_stack_pair(&self, index: u8) -> u16 {
        match index {
            0 => self.registers.bc(),
            1 => self.registers.de(),
            2 => self.registers.hl(),
            3 => self.registers.af(),
            _ => unreachable!("栈寄存器索引始终为 0..=3"),
        }
    }

    fn write_stack_pair(&mut self, index: u8, value: u16) {
        match index {
            0 => self.registers.set_bc(value),
            1 => self.registers.set_de(value),
            2 => self.registers.set_hl(value),
            3 => self.registers.set_af(value),
            _ => unreachable!("栈寄存器索引始终为 0..=3"),
        }
    }

    fn execute_alu(&mut self, operation: u8, value: u8) {
        match operation {
            0 => self.add_a(value, false),
            1 => self.add_a(value, self.registers.flag(Flag::Carry)),
            2 => self.subtract_a(value, false, true),
            3 => self.subtract_a(value, self.registers.flag(Flag::Carry), true),
            4 => {
                self.registers.a &= value;
                self.set_logic_flags(true);
            }
            5 => {
                self.registers.a ^= value;
                self.set_logic_flags(false);
            }
            6 => {
                self.registers.a |= value;
                self.set_logic_flags(false);
            }
            7 => self.subtract_a(value, false, false),
            _ => unreachable!("ALU 操作索引始终为 0..=7"),
        }
    }

    fn add_a(&mut self, value: u8, with_carry: bool) {
        let carry = u8::from(with_carry);
        let old = self.registers.a;
        let (partial, carry1) = old.overflowing_add(value);
        let (result, carry2) = partial.overflowing_add(carry);
        self.registers.a = result;
        self.registers.set_flag(Flag::Zero, result == 0);
        self.registers.set_flag(Flag::Subtract, false);
        self.registers.set_flag(
            Flag::HalfCarry,
            (old & 0x0f) + (value & 0x0f) + carry > 0x0f,
        );
        self.registers.set_flag(Flag::Carry, carry1 || carry2);
    }

    fn subtract_a(&mut self, value: u8, with_carry: bool, store: bool) {
        let carry = u8::from(with_carry);
        let old = self.registers.a;
        let (partial, borrow1) = old.overflowing_sub(value);
        let (result, borrow2) = partial.overflowing_sub(carry);
        if store {
            self.registers.a = result;
        }
        self.registers.set_flag(Flag::Zero, result == 0);
        self.registers.set_flag(Flag::Subtract, true);
        self.registers
            .set_flag(Flag::HalfCarry, (old & 0x0f) < (value & 0x0f) + carry);
        self.registers.set_flag(Flag::Carry, borrow1 || borrow2);
    }

    fn set_logic_flags(&mut self, half_carry: bool) {
        self.registers.set_flag(Flag::Zero, self.registers.a == 0);
        self.registers.set_flag(Flag::Subtract, false);
        self.registers.set_flag(Flag::HalfCarry, half_carry);
        self.registers.set_flag(Flag::Carry, false);
    }

    fn increment(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        self.registers.set_flag(Flag::Zero, result == 0);
        self.registers.set_flag(Flag::Subtract, false);
        self.registers
            .set_flag(Flag::HalfCarry, value & 0x0f == 0x0f);
        result
    }

    fn decrement(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        self.registers.set_flag(Flag::Zero, result == 0);
        self.registers.set_flag(Flag::Subtract, true);
        self.registers.set_flag(Flag::HalfCarry, value & 0x0f == 0);
        result
    }

    fn add_hl(&mut self, value: u16) {
        let old = self.registers.hl();
        let (result, carry) = old.overflowing_add(value);
        self.registers.set_hl(result);
        self.registers.set_flag(Flag::Subtract, false);
        self.registers
            .set_flag(Flag::HalfCarry, (old & 0x0fff) + (value & 0x0fff) > 0x0fff);
        self.registers.set_flag(Flag::Carry, carry);
    }

    fn add_signed_to_sp(&mut self, offset: i8) -> u16 {
        let old = self.registers.sp;
        let offset_bits = offset as i16 as u16;
        let result = old.wrapping_add_signed(offset as i16);
        self.registers.set_flag(Flag::Zero, false);
        self.registers.set_flag(Flag::Subtract, false);
        self.registers
            .set_flag(Flag::HalfCarry, (old ^ offset_bits ^ result) & 0x10 != 0);
        self.registers
            .set_flag(Flag::Carry, (old ^ offset_bits ^ result) & 0x100 != 0);
        result
    }

    fn decimal_adjust(&mut self) {
        let mut value = self.registers.a;
        let mut correction = 0;
        let mut carry = self.registers.flag(Flag::Carry);

        if !self.registers.flag(Flag::Subtract) {
            if self.registers.flag(Flag::HalfCarry) || value & 0x0f > 9 {
                correction |= 0x06;
            }
            if carry || value > 0x99 {
                correction |= 0x60;
                carry = true;
            }
            value = value.wrapping_add(correction);
        } else {
            if self.registers.flag(Flag::HalfCarry) {
                correction |= 0x06;
            }
            if carry {
                correction |= 0x60;
            }
            value = value.wrapping_sub(correction);
        }

        self.registers.a = value;
        self.registers.set_flag(Flag::Zero, value == 0);
        self.registers.set_flag(Flag::HalfCarry, false);
        self.registers.set_flag(Flag::Carry, carry);
    }

    fn set_rotate_flags(&mut self, carry: bool) {
        self.registers.set_flag(Flag::Zero, false);
        self.registers.set_flag(Flag::Subtract, false);
        self.registers.set_flag(Flag::HalfCarry, false);
        self.registers.set_flag(Flag::Carry, carry);
    }

    fn condition(&self, index: u8) -> bool {
        match index {
            0 => !self.registers.flag(Flag::Zero),
            1 => self.registers.flag(Flag::Zero),
            2 => !self.registers.flag(Flag::Carry),
            3 => self.registers.flag(Flag::Carry),
            _ => unreachable!("条件索引始终为 0..=3"),
        }
    }

    fn jump_relative<M: Memory>(&mut self, memory: &mut M) {
        let offset = self.fetch_byte(memory) as i8;
        self.registers.pc = self.registers.pc.wrapping_add_signed(offset as i16);
    }

    fn push<M: Memory>(&mut self, memory: &mut M, value: u16) {
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        self.write_bus(memory, self.registers.sp, (value >> 8) as u8);
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        self.write_bus(memory, self.registers.sp, value as u8);
    }

    fn pop<M: Memory>(&mut self, memory: &mut M) -> u16 {
        let low = self.read_bus(memory, self.registers.sp) as u16;
        self.registers.sp = self.registers.sp.wrapping_add(1);
        let high = self.read_bus(memory, self.registers.sp) as u16;
        self.registers.sp = self.registers.sp.wrapping_add(1);
        low | high << 8
    }

    fn advance_ime_delay(&mut self) {
        if self.ime_enable_delay == 0 {
            return;
        }
        self.ime_enable_delay -= 1;
        if self.ime_enable_delay == 0 {
            self.ime = true;
        }
    }

    fn execute_cb<M: Memory>(&mut self, memory: &mut M, opcode: u8) -> u8 {
        let group = opcode >> 6;
        let operation = (opcode >> 3) & 0x07;
        let target = opcode & 0x07;
        let value = self.read_r8(memory, target);

        match group {
            0 => {
                let old_carry = u8::from(self.registers.flag(Flag::Carry));
                let (result, carry) = match operation {
                    0 => (value.rotate_left(1), value & 0x80 != 0),
                    1 => (value.rotate_right(1), value & 0x01 != 0),
                    2 => ((value << 1) | old_carry, value & 0x80 != 0),
                    3 => ((value >> 1) | (old_carry << 7), value & 0x01 != 0),
                    4 => (value << 1, value & 0x80 != 0),
                    5 => ((value >> 1) | (value & 0x80), value & 0x01 != 0),
                    6 => (value.rotate_left(4), false),
                    7 => (value >> 1, value & 0x01 != 0),
                    _ => unreachable!("CB 旋转操作索引始终为 0..=7"),
                };
                self.write_r8(memory, target, result);
                self.registers.set_flag(Flag::Zero, result == 0);
                self.registers.set_flag(Flag::Subtract, false);
                self.registers.set_flag(Flag::HalfCarry, false);
                self.registers.set_flag(Flag::Carry, carry);
                if target == 6 { 16 } else { 8 }
            }
            1 => {
                self.registers
                    .set_flag(Flag::Zero, value & (1 << operation) == 0);
                self.registers.set_flag(Flag::Subtract, false);
                self.registers.set_flag(Flag::HalfCarry, true);
                if target == 6 { 12 } else { 8 }
            }
            2 => {
                self.write_r8(memory, target, value & !(1 << operation));
                if target == 6 { 16 } else { 8 }
            }
            3 => {
                self.write_r8(memory, target, value | (1 << operation));
                if target == 6 { 16 } else { 8 }
            }
            _ => unreachable!("CB 指令组始终为 0..=3"),
        }
    }

    fn pending_interrupts<M: Memory>(&self, memory: &M) -> u8 {
        memory.read8(0xffff) & memory.read8(0xff0f) & 0x1f
    }

    fn service_interrupt<M: Memory>(&mut self, memory: &mut M, pending: u8) -> u8 {
        let index = pending.trailing_zeros() as u8;
        let requested = memory.read8(0xff0f);
        memory.write8(0xff0f, requested & !(1 << index));

        self.ime = false;
        self.ime_enable_delay = 0;
        self.halted = false;
        self.idle_bus(memory);
        self.idle_bus(memory);
        self.push(memory, self.registers.pc);
        self.registers.pc = match index {
            0 => 0x0040,
            1 => 0x0048,
            2 => 0x0050,
            3 => 0x0058,
            4 => 0x0060,
            _ => unreachable!("中断索引始终为 0..=4"),
        };
        self.idle_bus(memory);
        20
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_invalid_opcode_in_chinese() {
        assert_eq!(
            CpuError::InvalidOpcode(0xd3).to_string(),
            "无效的 LR35902 操作码：0xd3"
        );
    }
}
