use serde::{Deserialize, Serialize};

use crate::model::HardwareModel;

const NORMAL_BIT_CYCLES: u32 = 512;
const FAST_BIT_CYCLES: u32 = 16;
const TRANSFER_START: u8 = 0x80;
const FAST_CLOCK: u8 = 0x02;
const INTERNAL_CLOCK: u8 = 0x01;

#[derive(Serialize, Deserialize)]
pub struct Serial {
    model: HardwareModel,
    data: u8,
    control: u8,
    cycle_remainder: u32,
    bits_remaining: u8,
}

impl Serial {
    pub fn new(model: HardwareModel) -> Self {
        Self {
            model,
            data: 0,
            control: 0,
            cycle_remainder: 0,
            bits_remaining: 0,
        }
    }

    pub fn read(&self, address: u16) -> u8 {
        match address {
            0xff01 => self.data,
            0xff02 => {
                let unused_bits = match self.model {
                    HardwareModel::Dmg => 0x7e,
                    HardwareModel::Cgb => 0x7c,
                };
                unused_bits | self.control
            }
            _ => 0xff,
        }
    }

    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0xff01 => self.data = value,
            0xff02 => {
                let mask = match self.model {
                    HardwareModel::Dmg => TRANSFER_START | INTERNAL_CLOCK,
                    HardwareModel::Cgb => TRANSFER_START | FAST_CLOCK | INTERNAL_CLOCK,
                };
                self.control = value & mask;
                if self.control & TRANSFER_START != 0 {
                    self.cycle_remainder = 0;
                    self.bits_remaining = 8;
                } else {
                    self.bits_remaining = 0;
                }
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, cycles: u32) -> bool {
        if self.control & (TRANSFER_START | INTERNAL_CLOCK) != TRANSFER_START | INTERNAL_CLOCK
            || self.bits_remaining == 0
        {
            return false;
        }

        self.cycle_remainder += cycles;
        let bit_cycles = if self.model == HardwareModel::Cgb && self.control & FAST_CLOCK != 0 {
            FAST_BIT_CYCLES
        } else {
            NORMAL_BIT_CYCLES
        };

        while self.cycle_remainder >= bit_cycles && self.bits_remaining > 0 {
            self.cycle_remainder -= bit_cycles;
            self.data = (self.data << 1) | 0x01;
            self.bits_remaining -= 1;
        }

        if self.bits_remaining == 0 {
            self.control &= !TRANSFER_START;
            self.cycle_remainder = 0;
            true
        } else {
            false
        }
    }
}
