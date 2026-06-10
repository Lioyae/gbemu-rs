use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::HardwareModel;

const DMG_BOOT_ROM_SIZE: usize = 0x100;
const CGB_BOOT_ROM_SIZE: usize = 0x900;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootRom {
    model: HardwareModel,
    bytes: Vec<u8>,
}

impl BootRom {
    fn from_bytes(model: HardwareModel, bytes: Vec<u8>) -> Result<Self, BootError> {
        let expected = match model {
            HardwareModel::Dmg => DMG_BOOT_ROM_SIZE,
            HardwareModel::Cgb => CGB_BOOT_ROM_SIZE,
        };
        if bytes.len() != expected {
            return Err(BootError::InvalidLength {
                model,
                expected,
                actual: bytes.len(),
            });
        }
        Ok(Self { model, bytes })
    }

    pub(crate) fn read(&self, address: u16) -> Option<u8> {
        let index = match (self.model, address) {
            (_, 0x0000..=0x00ff) => address as usize,
            (HardwareModel::Cgb, 0x0200..=0x08ff) => address as usize,
            _ => return None,
        };
        self.bytes.get(index).copied()
    }
}

#[derive(Debug, Default)]
pub struct BootRoms {
    dmg: Option<BootRom>,
    cgb: Option<BootRom>,
}

impl BootRoms {
    pub fn from_bytes(dmg: Option<Vec<u8>>, cgb: Option<Vec<u8>>) -> Result<Self, BootError> {
        Ok(Self {
            dmg: dmg
                .map(|bytes| BootRom::from_bytes(HardwareModel::Dmg, bytes))
                .transpose()?,
            cgb: cgb
                .map(|bytes| BootRom::from_bytes(HardwareModel::Cgb, bytes))
                .transpose()?,
        })
    }

    pub(crate) fn into_model(self, model: HardwareModel) -> Result<Option<BootRom>, BootError> {
        match model {
            HardwareModel::Dmg => match (self.dmg, self.cgb) {
                (boot @ Some(_), _) | (boot @ None, None) => Ok(boot),
                (None, Some(_)) => Err(BootError::ModelMismatch {
                    selected: HardwareModel::Dmg,
                    provided: HardwareModel::Cgb,
                }),
            },
            HardwareModel::Cgb => match (self.cgb, self.dmg) {
                (boot @ Some(_), _) | (boot @ None, None) => Ok(boot),
                (None, Some(_)) => Err(BootError::ModelMismatch {
                    selected: HardwareModel::Cgb,
                    provided: HardwareModel::Dmg,
                }),
            },
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BootError {
    #[error("{model:?} Boot ROM 长度无效：需要 {expected} 字节，实际 {actual} 字节")]
    InvalidLength {
        model: HardwareModel,
        expected: usize,
        actual: usize,
    },
    #[error("Boot ROM 模式不匹配：当前选择 {selected:?}，但只提供了 {provided:?}")]
    ModelMismatch {
        selected: HardwareModel,
        provided: HardwareModel,
    },
}
