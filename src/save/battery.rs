use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;

use crate::{
    cartridge::ControllerKind,
    emulator::{Emulator, EmulatorError},
};

const RTC_MAGIC: &[u8; 4] = b"GBRT";
const RTC_VERSION: u8 = 1;
const RTC_HEADER_SIZE: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryPaths {
    pub sav: PathBuf,
    pub rtc: PathBuf,
}

impl BatteryPaths {
    pub fn from_rom(rom_path: &Path) -> Self {
        Self {
            sav: rom_path.with_extension("sav"),
            rtc: rom_path.with_extension("rtc"),
        }
    }
}

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("存档 I/O 失败：{0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Emulator(#[from] EmulatorError),
    #[error("RTC 存档格式无效：{0}")]
    InvalidRtc(String),
    #[error("存档长度不匹配：需要 {expected} 字节，实际 {actual} 字节")]
    InvalidSaveLength { expected: usize, actual: usize },
    #[error("当前模拟器实例没有关联 ROM 文件路径")]
    MissingRomPath,
}

pub struct BatterySave;

impl BatterySave {
    pub fn save(emulator: &mut Emulator, rom_path: &Path) -> Result<(), SaveError> {
        let paths = BatteryPaths::from_rom(rom_path);
        let state = emulator.cartridge_persistent_state();
        let mut sav = state.ram;
        sav.extend_from_slice(&state.flash);
        atomic_write(&paths.sav, &sav)?;

        if !state.rtc.is_empty() {
            let rtc = encode_rtc(state.controller, &state.rtc, unix_timestamp()?);
            atomic_write(&paths.rtc, &rtc)?;
        }
        emulator.clear_cartridge_persistent_dirty();
        Ok(())
    }

    pub fn load(emulator: &mut Emulator, rom_path: &Path) -> Result<(), SaveError> {
        let paths = BatteryPaths::from_rom(rom_path);
        let template = emulator.cartridge_persistent_state();
        let sav = fs::read(&paths.sav)?;
        let expected_sav_len = template.ram.len() + template.flash.len();
        if sav.len() != expected_sav_len {
            return Err(SaveError::InvalidSaveLength {
                expected: expected_sav_len,
                actual: sav.len(),
            });
        }

        let mut state = template.clone();
        state.ram.copy_from_slice(&sav[..template.ram.len()]);
        state
            .flash
            .copy_from_slice(&sav[template.ram.len()..expected_sav_len]);

        let mut elapsed_seconds = 0;
        if !template.rtc.is_empty() {
            let rtc_file = fs::read(&paths.rtc)?;
            let decoded = decode_rtc(&rtc_file)?;
            if decoded.controller != template.controller {
                return Err(SaveError::InvalidRtc(format!(
                    "控制器不匹配：需要 {:?}，实际 {:?}",
                    template.controller, decoded.controller
                )));
            }
            if decoded.data.len() != template.rtc.len() {
                return Err(SaveError::InvalidRtc(format!(
                    "RTC 长度不匹配：需要 {} 字节，实际 {} 字节",
                    template.rtc.len(),
                    decoded.data.len()
                )));
            }
            state.rtc.copy_from_slice(&decoded.data);
            elapsed_seconds = unix_timestamp()?.saturating_sub(decoded.timestamp);
        }

        emulator.load_cartridge_persistent_state(&state)?;
        emulator.advance_cartridge_rtc(elapsed_seconds);
        emulator.clear_cartridge_persistent_dirty();
        Ok(())
    }
}

struct DecodedRtc {
    controller: ControllerKind,
    timestamp: u64,
    data: Vec<u8>,
}

fn encode_rtc(controller: ControllerKind, data: &[u8], timestamp: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(RTC_HEADER_SIZE + data.len());
    bytes.extend_from_slice(RTC_MAGIC);
    bytes.push(RTC_VERSION);
    bytes.push(controller_code(controller));
    bytes.extend_from_slice(&[0, 0]);
    bytes.extend_from_slice(&timestamp.to_le_bytes());
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(data);
    bytes
}

fn decode_rtc(bytes: &[u8]) -> Result<DecodedRtc, SaveError> {
    if bytes.len() < RTC_HEADER_SIZE || &bytes[..4] != RTC_MAGIC {
        return Err(SaveError::InvalidRtc("文件头不正确".to_owned()));
    }
    if bytes[4] != RTC_VERSION {
        return Err(SaveError::InvalidRtc(format!("不支持的版本 {}", bytes[4])));
    }
    let controller = controller_from_code(bytes[5])
        .ok_or_else(|| SaveError::InvalidRtc("控制器编码无效".to_owned()))?;
    let timestamp = u64::from_le_bytes(bytes[8..16].try_into().expect("固定长度切片"));
    let data_len = u32::from_le_bytes(bytes[16..20].try_into().expect("固定长度切片")) as usize;
    if bytes.len() != RTC_HEADER_SIZE + data_len {
        return Err(SaveError::InvalidRtc("文件长度与头部声明不一致".to_owned()));
    }
    Ok(DecodedRtc {
        controller,
        timestamp,
        data: bytes[RTC_HEADER_SIZE..].to_vec(),
    })
}

pub(super) fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("gbmeu-save");
    let temporary = path.with_file_name(format!(".{file_name}.tmp"));
    let backup = path.with_file_name(format!(".{file_name}.bak"));

    let mut file = File::create(&temporary)?;
    file.write_all(data)?;
    file.sync_all()?;
    drop(file);

    if path.exists() {
        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        fs::rename(path, &backup)?;
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::rename(&backup, path);
            return Err(error);
        }
        fs::remove_file(backup)?;
    } else {
        fs::rename(temporary, path)?;
    }
    Ok(())
}

fn unix_timestamp() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(io::Error::other)
}

fn controller_code(controller: ControllerKind) -> u8 {
    match controller {
        ControllerKind::Rom => 0,
        ControllerKind::Mbc1 => 1,
        ControllerKind::Mbc2 => 2,
        ControllerKind::Mmm01 => 3,
        ControllerKind::Mbc3 => 4,
        ControllerKind::Mbc5 => 5,
        ControllerKind::Mbc6 => 6,
        ControllerKind::Mbc7 => 7,
        ControllerKind::PocketCamera => 8,
        ControllerKind::BandaiTama5 => 9,
        ControllerKind::Huc3 => 10,
        ControllerKind::Huc1 => 11,
    }
}

fn controller_from_code(code: u8) -> Option<ControllerKind> {
    Some(match code {
        0 => ControllerKind::Rom,
        1 => ControllerKind::Mbc1,
        2 => ControllerKind::Mbc2,
        3 => ControllerKind::Mmm01,
        4 => ControllerKind::Mbc3,
        5 => ControllerKind::Mbc5,
        6 => ControllerKind::Mbc6,
        7 => ControllerKind::Mbc7,
        8 => ControllerKind::PocketCamera,
        9 => ControllerKind::BandaiTama5,
        10 => ControllerKind::Huc3,
        11 => ControllerKind::Huc1,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory(test_name: &str) -> PathBuf {
        let unique = format!(
            "gbmeu-{test_name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("系统时间应有效")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("临时目录应创建成功");
        path
    }

    fn battery_rom() -> Vec<u8> {
        let mut rom = vec![0; 32 * 1024];
        rom[0x147] = 0x03;
        rom[0x148] = 0x00;
        rom[0x149] = 0x02;
        rom
    }

    #[test]
    fn derives_sav_and_rtc_paths_from_rom_path() {
        let paths = BatteryPaths::from_rom(Path::new("D:/roms/game.gbc"));

        assert_eq!(paths.sav, PathBuf::from("D:/roms/game.sav"));
        assert_eq!(paths.rtc, PathBuf::from("D:/roms/game.rtc"));
    }

    #[test]
    fn saves_and_loads_ram_without_partial_updates() {
        let directory = temporary_directory("battery-round-trip");
        let rom_path = directory.join("game.gb");
        let mut source = Emulator::from_rom(battery_rom()).expect("电池 ROM 应创建成功");
        source.write_memory(0x0000, 0x0a);
        source.write_memory(0xa123, 0x5a);
        BatterySave::save(&mut source, &rom_path).expect("存档应写入成功");
        assert!(!source.cartridge_persistent_dirty());

        let mut target = Emulator::from_rom(battery_rom()).expect("电池 ROM 应创建成功");
        BatterySave::load(&mut target, &rom_path).expect("存档应加载成功");
        target.write_memory(0x0000, 0x0a);
        assert_eq!(target.read_memory(0xa123), 0x5a);
        assert!(!target.cartridge_persistent_dirty());

        fs::remove_dir_all(directory).expect("临时目录应清理成功");
    }

    #[test]
    fn rejects_wrong_save_length_without_changing_ram() {
        let directory = temporary_directory("battery-invalid");
        let rom_path = directory.join("game.gb");
        let paths = BatteryPaths::from_rom(&rom_path);
        fs::write(&paths.sav, [0xaa]).expect("损坏存档应写入成功");

        let mut emulator = Emulator::from_rom(battery_rom()).expect("电池 ROM 应创建成功");
        emulator.write_memory(0x0000, 0x0a);
        emulator.write_memory(0xa123, 0x5a);
        assert!(matches!(
            BatterySave::load(&mut emulator, &rom_path),
            Err(SaveError::InvalidSaveLength { .. })
        ));
        assert_eq!(emulator.read_memory(0xa123), 0x5a);

        fs::remove_dir_all(directory).expect("临时目录应清理成功");
    }

    #[test]
    fn rtc_format_round_trips_and_rejects_damage() {
        let bytes = encode_rtc(ControllerKind::Mbc3, &[1, 2, 3], 42);
        let decoded = decode_rtc(&bytes).expect("RTC 格式应解析成功");
        assert_eq!(decoded.controller, ControllerKind::Mbc3);
        assert_eq!(decoded.timestamp, 42);
        assert_eq!(decoded.data, [1, 2, 3]);

        let mut damaged = bytes;
        damaged[0] = 0;
        assert!(matches!(
            decode_rtc(&damaged),
            Err(SaveError::InvalidRtc(_))
        ));
    }
}
