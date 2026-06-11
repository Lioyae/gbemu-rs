use std::{
    env, fs,
    io::{self, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
};

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use gbmeu::{boot::BootRoms, emulator::Emulator, model::ModelPreference, tui};

#[derive(Debug, Parser)]
#[command(
    name = "gbmeu",
    version,
    about = "终端中的 Game Boy 与 Game Boy Color 模拟器",
    disable_help_flag = true,
    disable_version_flag = true,
    help_template = "{about-with-newline}\n用法: {usage}\n\n参数:\n{positionals}\n选项:\n{options}"
)]
struct Cli {
    /// 要加载的 Game Boy 或 Game Boy Color ROM 文件
    rom: Option<PathBuf>,

    /// 硬件模式；auto 根据卡带头自动选择
    #[arg(long, value_enum, default_value_t = CliModel::Auto)]
    model: CliModel,

    /// 256 字节 DMG Boot ROM 文件
    #[arg(long)]
    dmg_boot_rom: Option<PathBuf>,

    /// 2304 字节 CGB Boot ROM 文件
    #[arg(long)]
    cgb_boot_rom: Option<PathBuf>,

    /// 显示帮助
    #[arg(short = 'h', long, action = ArgAction::Help)]
    help: Option<bool>,

    /// 显示版本
    #[arg(short = 'V', long, action = ArgAction::Version)]
    version: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum CliModel {
    #[default]
    Auto,
    Dmg,
    Cgb,
}

impl From<CliModel> for ModelPreference {
    fn from(model: CliModel) -> Self {
        match model {
            CliModel::Auto => Self::Auto,
            CliModel::Dmg => Self::Dmg,
            CliModel::Cgb => Self::Cgb,
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("程序运行失败：{error:#}");

            if should_pause_after_error(env::args_os().len(), io::stdin().is_terminal()) {
                eprint!("\n按 Enter 键退出...");
                let _ = io::stderr().flush();
                let mut input = String::new();
                let _ = io::stdin().read_line(&mut input);
            }

            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let rom_path = match cli.rom {
        Some(path) => path,
        None => match tui::select_rom()? {
            Some(path) => path,
            None => return Ok(()),
        },
    };
    let rom = fs::read(&rom_path)
        .with_context(|| format!("无法读取 ROM 文件：{}", rom_path.display()))?;
    let boot_roms = BootRoms::from_bytes(
        read_optional_file(cli.dmg_boot_rom.as_ref())?,
        read_optional_file(cli.cgb_boot_rom.as_ref())?,
    )
    .context("无法加载 Boot ROM")?;
    let emulator = Emulator::from_rom_with_boot_roms(rom, cli.model.into(), boot_roms)
        .with_context(|| format!("无法加载卡带：{}", rom_path.display()))?;
    tui::run(emulator, rom_path)
}

fn should_pause_after_error(argument_count: usize, stdin_is_terminal: bool) -> bool {
    argument_count == 1 && stdin_is_terminal
}

fn read_optional_file(path: Option<&PathBuf>) -> Result<Option<Vec<u8>>> {
    path.map(|path| {
        fs::read(path).with_context(|| format!("无法读取 Boot ROM 文件：{}", path.display()))
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pauses_for_interactive_launch_without_arguments() {
        assert!(should_pause_after_error(1, true));
        assert!(!should_pause_after_error(2, true));
        assert!(!should_pause_after_error(1, false));
    }
}
