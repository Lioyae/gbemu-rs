use std::{
    fs, io,
    path::{Path, PathBuf},
};

use bincode::Options;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{emulator::Emulator, model::HardwareModel};

use super::battery::atomic_write;

const MAGIC: &[u8; 4] = b"GBST";
const VERSION: u16 = 1;
const HEADER_SIZE: usize = 80;
const MAX_STATE_SIZE: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatePaths {
    pub state: PathBuf,
}

impl StatePaths {
    pub fn for_rom(rom_path: &Path, slot: u8) -> Result<Self, StateError> {
        validate_slot(slot)?;
        Ok(Self {
            state: rom_path.with_extension(format!("state{slot}")),
        })
    }
}

pub struct StateSave;

impl StateSave {
    pub fn save(emulator: &Emulator, rom_path: &Path, slot: u8) -> Result<(), StateError> {
        let paths = StatePaths::for_rom(rom_path, slot)?;
        let payload = codec().serialize(emulator).map_err(StateError::Serialize)?;
        let payload_length =
            u64::try_from(payload.len()).map_err(|_| StateError::StateTooLarge(payload.len()))?;
        if payload_length > MAX_STATE_SIZE {
            return Err(StateError::StateTooLarge(payload.len()));
        }

        let mut bytes = Vec::with_capacity(HEADER_SIZE + payload.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.push(model_code(emulator.model()));
        bytes.push(0);
        bytes.extend_from_slice(emulator.rom_hash());
        bytes.extend_from_slice(&Sha256::digest(&payload));
        bytes.extend_from_slice(&payload_length.to_le_bytes());
        bytes.extend_from_slice(&payload);
        atomic_write(&paths.state, &bytes)?;
        Ok(())
    }

    pub fn load(emulator: &mut Emulator, rom_path: &Path, slot: u8) -> Result<(), StateError> {
        let paths = StatePaths::for_rom(rom_path, slot)?;
        let bytes = fs::read(&paths.state)?;
        let header = StateHeader::parse(&bytes)?;
        let payload = &bytes[HEADER_SIZE..];

        if &header.rom_hash != emulator.rom_hash() {
            return Err(StateError::RomMismatch);
        }
        if Sha256::digest(payload).as_slice() != header.payload_hash {
            return Err(StateError::InvalidFormat("状态数据校验失败".to_owned()));
        }
        if header.model != emulator.model() {
            return Err(StateError::ModelMismatch {
                expected: emulator.model(),
                actual: header.model,
            });
        }

        let candidate: Emulator = codec()
            .deserialize(payload)
            .map_err(StateError::Deserialize)?;
        if candidate.rom_hash() != emulator.rom_hash() {
            return Err(StateError::RomMismatch);
        }
        if candidate.model() != header.model {
            return Err(StateError::ModelMismatch {
                expected: header.model,
                actual: candidate.model(),
            });
        }

        *emulator = candidate;
        Ok(())
    }
}

struct StateHeader {
    model: HardwareModel,
    rom_hash: [u8; 32],
    payload_hash: [u8; 32],
}

impl StateHeader {
    fn parse(bytes: &[u8]) -> Result<Self, StateError> {
        if bytes.len() < HEADER_SIZE {
            return Err(StateError::InvalidFormat("文件头不完整".to_owned()));
        }
        if &bytes[..4] != MAGIC {
            return Err(StateError::InvalidFormat("文件魔数不正确".to_owned()));
        }

        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != VERSION {
            return Err(StateError::UnsupportedVersion(version));
        }
        let model = model_from_code(bytes[6])
            .ok_or_else(|| StateError::InvalidFormat("硬件模式编码无效".to_owned()))?;
        let rom_hash = bytes[8..40].try_into().expect("固定长度的 ROM 哈希切片");
        let payload_hash = bytes[40..72].try_into().expect("固定长度的状态哈希切片");
        let payload_length =
            u64::from_le_bytes(bytes[72..80].try_into().expect("固定长度的状态长度切片"));
        if payload_length > MAX_STATE_SIZE {
            return Err(StateError::StateTooLarge(
                usize::try_from(payload_length).unwrap_or(usize::MAX),
            ));
        }
        let actual_length = bytes.len() - HEADER_SIZE;
        if payload_length != actual_length as u64 {
            return Err(StateError::InvalidFormat(format!(
                "状态长度不匹配：声明 {payload_length} 字节，实际 {actual_length} 字节"
            )));
        }

        Ok(Self {
            model,
            rom_hash,
            payload_hash,
        })
    }
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(MAX_STATE_SIZE)
        .reject_trailing_bytes()
}

fn validate_slot(slot: u8) -> Result<(), StateError> {
    if slot <= 9 {
        Ok(())
    } else {
        Err(StateError::InvalidSlot(slot))
    }
}

fn model_code(model: HardwareModel) -> u8 {
    match model {
        HardwareModel::Dmg => 0,
        HardwareModel::Cgb => 1,
    }
}

fn model_from_code(code: u8) -> Option<HardwareModel> {
    match code {
        0 => Some(HardwareModel::Dmg),
        1 => Some(HardwareModel::Cgb),
        _ => None,
    }
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("即时存档槽位必须为 0 到 9，实际为 {0}")]
    InvalidSlot(u8),
    #[error("即时存档格式无效：{0}")]
    InvalidFormat(String),
    #[error("不支持的即时存档版本：{0}")]
    UnsupportedVersion(u16),
    #[error("即时存档属于另一个 ROM")]
    RomMismatch,
    #[error("即时存档硬件模式不匹配：需要 {expected:?}，实际 {actual:?}")]
    ModelMismatch {
        expected: HardwareModel,
        actual: HardwareModel,
    },
    #[error("即时存档过大：{0} 字节")]
    StateTooLarge(usize),
    #[error("无法读写即时存档：{0}")]
    Io(#[from] io::Error),
    #[error("无法编码即时存档：{0}")]
    Serialize(bincode::Error),
    #[error("无法解码即时存档：{0}")]
    Deserialize(bincode::Error),
}
