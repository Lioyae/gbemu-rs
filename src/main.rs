use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use gbmeu::cartridge::Cartridge;

#[derive(Debug, Parser)]
#[command(name = "gbmeu", version, about = "终端中的 Game Boy DMG 模拟器")]
struct Cli {
    /// 要加载的 Game Boy ROM 文件
    rom: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let rom = fs::read(&cli.rom)
        .with_context(|| format!("无法读取 ROM 文件：{}", cli.rom.display()))?;
    let cartridge = Cartridge::from_bytes(rom)
        .with_context(|| format!("无法加载卡带：{}", cli.rom.display()))?;

    println!(
        "已加载 ROM：{}（标题：{}，类型：{}，ROM：{} KiB，RAM：{} KiB）",
        cli.rom.display(),
        cartridge.header().title(),
        cartridge.header().cartridge_type(),
        cartridge.header().rom_size() / 1024,
        cartridge.header().ram_size() / 1024,
    );
    Ok(())
}
