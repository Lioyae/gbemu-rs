use std::{
    fs, io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LibraryConfigError {
    #[error("无法确定配置目录")]
    ConfigDirectoryUnavailable,
    #[error("ROM 库配置 I/O 失败：{0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryConfig {
    directories: Vec<PathBuf>,
}

impl LibraryConfig {
    pub fn load_default() -> Result<Self, LibraryConfigError> {
        Self::load(&Self::default_path()?)
    }

    pub fn save_default(&self) -> Result<(), LibraryConfigError> {
        self.save(&Self::default_path()?)
    }

    pub fn default_path() -> Result<PathBuf, LibraryConfigError> {
        let project = ProjectDirs::from("com", "Lioyae", "gbmeu")
            .ok_or(LibraryConfigError::ConfigDirectoryUnavailable)?;
        Ok(project.config_dir().join("rom-dirs.txt"))
    }

    pub fn load(path: &Path) -> Result<Self, LibraryConfigError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error.into()),
        };
        let mut config = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            config.add_directory(PathBuf::from(line));
        }
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<(), LibraryConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut text = String::from("# gbmeu ROM 库目录，每行一个路径\n");
        for directory in &self.directories {
            text.push_str(&directory.to_string_lossy());
            text.push('\n');
        }
        fs::write(path, text)?;
        Ok(())
    }

    pub fn directories(&self) -> &[PathBuf] {
        &self.directories
    }

    pub fn add_directory(&mut self, directory: PathBuf) -> bool {
        let normalized = normalize(&directory);
        if self.directories.contains(&normalized) {
            return false;
        }
        self.directories.push(normalized);
        true
    }

    pub fn remove_directory(&mut self, directory: &Path) -> bool {
        let normalized = normalize(directory);
        let original_len = self.directories.len();
        self.directories.retain(|existing| *existing != normalized);
        self.directories.len() != original_len
    }
}

fn normalize(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "gbmeu-config-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("系统时间应有效")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("临时目录应创建成功");
        path
    }

    #[test]
    fn saves_loads_and_deduplicates_directories() {
        let root = temporary_directory();
        let first = root.join("first");
        let second = root.join("second");
        fs::create_dir_all(&first).expect("目录应创建成功");
        fs::create_dir_all(&second).expect("目录应创建成功");
        let path = root.join("config/rom-dirs.txt");
        let mut config = LibraryConfig::default();
        assert!(config.add_directory(first.clone()));
        assert!(!config.add_directory(first));
        assert!(config.add_directory(second));
        config.save(&path).expect("配置应保存成功");

        let loaded = LibraryConfig::load(&path).expect("配置应加载成功");
        assert_eq!(loaded.directories().len(), 2);

        fs::remove_dir_all(root).expect("临时目录应清理成功");
    }
}
