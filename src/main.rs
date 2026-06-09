use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use gbmeu::{emulator::Emulator, tui};

#[derive(Debug, Parser)]
#[command(name = "gbmeu", version, about = "终端中的 Game Boy DMG 模拟器")]
struct Cli {
    /// 要加载的 Game Boy ROM 文件
    rom: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let rom =
        fs::read(&cli.rom).with_context(|| format!("无法读取 ROM 文件：{}", cli.rom.display()))?;
    let emulator =
        Emulator::from_rom(rom).with_context(|| format!("无法加载卡带：{}", cli.rom.display()))?;
    tui::run(emulator)
}
