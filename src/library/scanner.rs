use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use walkdir::WalkDir;

use crate::cartridge::{Cartridge, CartridgeType, CgbSupport};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RomSaveStatus {
    pub sav: bool,
    pub rtc: bool,
    pub state_slots: [bool; 10],
}

impl RomSaveStatus {
    pub fn any(self) -> bool {
        self.sav || self.rtc || self.state_slots.into_iter().any(|exists| exists)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomEntry {
    pub path: PathBuf,
    pub title: String,
    pub cartridge_type: CartridgeType,
    pub cgb_support: CgbSupport,
    pub file_size: u64,
    pub rom_size: usize,
    pub ram_size: usize,
    pub save_status: RomSaveStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanError {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanResult {
    pub entries: Vec<RomEntry>,
    pub errors: Vec<ScanError>,
}

pub fn scan_roms(directories: &[PathBuf]) -> ScanResult {
    let mut result = ScanResult::default();
    let mut seen = HashSet::new();

    for directory in directories {
        for entry in WalkDir::new(directory).follow_links(false) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    result.errors.push(ScanError {
                        path: error
                            .path()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| directory.clone()),
                        message: error.to_string(),
                    });
                    continue;
                }
            };
            if !entry.file_type().is_file() || !is_rom_path(entry.path()) {
                continue;
            }
            let canonical = entry
                .path()
                .canonicalize()
                .unwrap_or_else(|_| entry.path().to_path_buf());
            if !seen.insert(canonical.clone()) {
                continue;
            }

            match inspect_rom(&canonical) {
                Ok(rom) => result.entries.push(rom),
                Err(message) => result.errors.push(ScanError {
                    path: canonical,
                    message,
                }),
            }
        }
    }

    result.entries.sort_by(|left, right| {
        left.title
            .to_lowercase()
            .cmp(&right.title.to_lowercase())
            .then_with(|| left.path.cmp(&right.path))
    });
    result
}

fn is_rom_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("gb") || extension.eq_ignore_ascii_case("gbc")
        })
}

fn inspect_rom(path: &Path) -> Result<RomEntry, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let header = Cartridge::inspect_header(&bytes).map_err(|error| error.to_string())?;
    Ok(RomEntry {
        path: path.to_path_buf(),
        title: if header.title().is_empty() {
            path.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("未命名 ROM")
                .to_owned()
        } else {
            header.title().to_owned()
        },
        cartridge_type: header.cartridge_type(),
        cgb_support: header.cgb_support(),
        file_size: bytes.len() as u64,
        rom_size: header.rom_size(),
        ram_size: header.ram_size(),
        save_status: inspect_save_status(path),
    })
}

fn inspect_save_status(path: &Path) -> RomSaveStatus {
    RomSaveStatus {
        sav: path.with_extension("sav").is_file(),
        rtc: path.with_extension("rtc").is_file(),
        state_slots: std::array::from_fn(|slot| {
            path.with_extension(format!("state{slot}")).is_file()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "gbmeu-scanner-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("系统时间应有效")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("临时目录应创建成功");
        path
    }

    fn rom_with_header(
        title: &str,
        cartridge_type: u8,
        ram_size_code: u8,
        cgb_flag: u8,
    ) -> Vec<u8> {
        let mut rom = vec![0; 32 * 1024];
        let title = title.as_bytes();
        rom[0x134..0x134 + title.len()].copy_from_slice(title);
        rom[0x143] = cgb_flag;
        rom[0x147] = cartridge_type;
        rom[0x149] = ram_size_code;
        rom
    }

    fn rom(title: &str) -> Vec<u8> {
        rom_with_header(title, 0x00, 0x00, 0x00)
    }

    #[test]
    fn recursively_scans_extensions_deduplicates_and_isolates_bad_roms() {
        let root = temporary_directory();
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("嵌套目录应创建成功");
        let alpha_path = root.join("A.GB");
        fs::write(&alpha_path, rom_with_header("ALPHA", 0x10, 0x03, 0x80)).expect("ROM 应写入成功");
        fs::write(alpha_path.with_extension("sav"), [0xaa]).expect("电池存档应写入成功");
        fs::write(alpha_path.with_extension("rtc"), [0xbb]).expect("RTC 存档应写入成功");
        fs::write(alpha_path.with_extension("state3"), [0xcc]).expect("即时存档应写入成功");
        fs::write(nested.join("b.gbc"), rom("BETA")).expect("ROM 应写入成功");
        fs::write(nested.join("bad.gb"), [0; 4]).expect("坏 ROM 应写入成功");

        let result = scan_roms(&[root.clone(), nested]);

        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].title, "ALPHA");
        assert_eq!(result.entries[0].rom_size, 32 * 1024);
        assert_eq!(result.entries[0].ram_size, 32 * 1024);
        assert!(result.entries[0].save_status.sav);
        assert!(result.entries[0].save_status.rtc);
        assert_eq!(
            result.entries[0].save_status.state_slots,
            [
                false, false, false, true, false, false, false, false, false, false
            ]
        );
        assert_eq!(result.entries[1].title, "BETA");
        assert_eq!(result.errors.len(), 1);

        fs::remove_dir_all(root).expect("临时目录应清理成功");
    }
}
