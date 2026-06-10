use std::{
    fs,
    io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::library::{LibraryConfig, RomEntry, ScanResult, scan_roms};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum LauncherMode {
    #[default]
    Library,
    Browser,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BrowserEntry {
    Parent(PathBuf),
    Directory(PathBuf),
    Rom(PathBuf),
}

struct Launcher {
    config: LibraryConfig,
    scan: ScanResult,
    selected: usize,
    mode: LauncherMode,
    browser_directory: PathBuf,
    browser_entries: Vec<BrowserEntry>,
    browser_selected: usize,
    status: String,
}

impl Launcher {
    fn load() -> Result<Self> {
        let mut config = LibraryConfig::load_default().context("无法加载 ROM 库配置")?;
        if config.directories().is_empty() {
            let bundled = std::env::current_dir()
                .context("无法读取当前目录")?
                .join("roms");
            if bundled.is_dir() {
                config.add_directory(bundled);
                config.save_default().context("无法保存默认 ROM 目录")?;
            }
        }
        let scan = scan_roms(config.directories());
        let browser_directory = std::env::current_dir().context("无法读取当前目录")?;
        let mut launcher = Self {
            config,
            scan,
            selected: 0,
            mode: LauncherMode::Library,
            browser_directory,
            browser_entries: Vec::new(),
            browser_selected: 0,
            status: String::new(),
        };
        launcher.refresh_browser();
        launcher.update_scan_status();
        Ok(launcher)
    }

    fn update_scan_status(&mut self) {
        self.status = format!(
            "已发现 {} 个 ROM，{} 个文件无法识别",
            self.scan.entries.len(),
            self.scan.errors.len()
        );
    }

    fn rescan(&mut self) {
        self.scan = scan_roms(self.config.directories());
        self.selected = self
            .selected
            .min(self.scan.entries.len().saturating_sub(1));
        self.update_scan_status();
    }

    fn refresh_browser(&mut self) {
        let mut directories = Vec::new();
        let mut roms = Vec::new();
        if let Some(parent) = self.browser_directory.parent() {
            directories.push(BrowserEntry::Parent(parent.to_path_buf()));
        }
        match fs::read_dir(&self.browser_directory) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        directories.push(BrowserEntry::Directory(path));
                    } else if is_rom_path(&path) {
                        roms.push(BrowserEntry::Rom(path));
                    }
                }
                directories.sort_by_key(browser_sort_key);
                roms.sort_by_key(browser_sort_key);
                directories.extend(roms);
                self.browser_entries = directories;
                self.browser_selected = self
                    .browser_selected
                    .min(self.browser_entries.len().saturating_sub(1));
                self.status = format!("浏览：{}", self.browser_directory.display());
            }
            Err(error) => {
                self.browser_entries.clear();
                self.browser_selected = 0;
                self.status = format!("无法读取目录：{error}");
            }
        }
    }

    fn move_selection(&mut self, down: bool) {
        let (selected, len) = match self.mode {
            LauncherMode::Library => (&mut self.selected, self.scan.entries.len()),
            LauncherMode::Browser => (&mut self.browser_selected, self.browser_entries.len()),
        };
        if len == 0 {
            *selected = 0;
        } else if down {
            *selected = (*selected + 1).min(len - 1);
        } else {
            *selected = selected.saturating_sub(1);
        }
    }

    fn activate(&mut self) -> Option<PathBuf> {
        match self.mode {
            LauncherMode::Library => self
                .scan
                .entries
                .get(self.selected)
                .map(|entry| entry.path.clone()),
            LauncherMode::Browser => match self.browser_entries.get(self.browser_selected).cloned()
            {
                Some(BrowserEntry::Parent(path) | BrowserEntry::Directory(path)) => {
                    self.browser_directory = path;
                    self.browser_selected = 0;
                    self.refresh_browser();
                    None
                }
                Some(BrowserEntry::Rom(path)) => Some(path),
                None => None,
            },
        }
    }

    fn add_browser_directory(&mut self) -> Result<()> {
        if self.config.add_directory(self.browser_directory.clone()) {
            self.config
                .save_default()
                .context("无法保存 ROM 库配置")?;
            self.rescan();
            self.status = format!("已添加目录：{}", self.browser_directory.display());
        } else {
            self.status = "当前目录已在 ROM 库中".to_owned();
        }
        Ok(())
    }

    fn remove_browser_directory(&mut self) -> Result<()> {
        if self.config.remove_directory(&self.browser_directory) {
            self.config
                .save_default()
                .context("无法保存 ROM 库配置")?;
            self.rescan();
            self.status = format!("已移除目录：{}", self.browser_directory.display());
        } else {
            self.status = "当前目录不在 ROM 库中".to_owned();
        }
        Ok(())
    }
}

pub fn select_rom() -> Result<Option<PathBuf>> {
    let _guard = LauncherTerminalGuard::enter().context("无法初始化启动器终端")?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).context("无法创建启动器绘制后端")?;
    terminal.clear().context("无法清空启动器终端")?;
    let mut launcher = Launcher::load()?;

    loop {
        let size = terminal.size().context("无法读取终端尺寸")?;
        if size.width < 100 || size.height < 24 {
            anyhow::bail!(
                "终端尺寸不足：当前 {}×{}，启动器至少需要 100×24",
                size.width,
                size.height
            );
        }
        terminal
            .draw(|frame| render(frame, &launcher))
            .context("无法绘制 ROM 启动器")?;

        if let Event::Key(key) = event::read().context("无法读取启动器按键")? {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            match key.code {
                KeyCode::Char('q' | 'Q') => return Ok(None),
                KeyCode::Esc if launcher.mode == LauncherMode::Browser => {
                    launcher.mode = LauncherMode::Library;
                    launcher.update_scan_status();
                }
                KeyCode::Esc => return Ok(None),
                KeyCode::Up => launcher.move_selection(false),
                KeyCode::Down => launcher.move_selection(true),
                KeyCode::Enter => {
                    if let Some(path) = launcher.activate() {
                        return Ok(Some(path));
                    }
                }
                KeyCode::F(2) | KeyCode::Tab => {
                    launcher.mode = match launcher.mode {
                        LauncherMode::Library => LauncherMode::Browser,
                        LauncherMode::Browser => LauncherMode::Library,
                    };
                }
                KeyCode::Char('r' | 'R') => launcher.rescan(),
                KeyCode::Char('a' | 'A') if launcher.mode == LauncherMode::Browser => {
                    launcher.add_browser_directory()?;
                }
                KeyCode::Char('d' | 'D') if launcher.mode == LauncherMode::Browser => {
                    launcher.remove_browser_directory()?;
                }
                KeyCode::Backspace if launcher.mode == LauncherMode::Browser => {
                    if let Some(parent) = launcher.browser_directory.parent() {
                        launcher.browser_directory = parent.to_path_buf();
                        launcher.browser_selected = 0;
                        launcher.refresh_browser();
                    }
                }
                _ => {}
            }
        }
    }
}

fn render(frame: &mut Frame, launcher: &Launcher) {
    let area = frame.area();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new("gbmeu ROM 启动器")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL)),
        vertical[0],
    );

    match launcher.mode {
        LauncherMode::Library => render_library(frame, launcher, vertical[1]),
        LauncherMode::Browser => render_browser(frame, launcher, vertical[1]),
    }

    let help = match launcher.mode {
        LauncherMode::Library => {
            "↑↓ 选择  Enter 启动  F2/Tab 文件浏览器  R 重扫  Q/Esc 退出"
        }
        LauncherMode::Browser => {
            "↑↓ 选择  Enter 打开/启动  Backspace 上级  A 添加目录  D 移除目录  Esc 返回"
        }
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(launcher.status.clone()),
            Line::from(help),
        ]),
        vertical[2],
    );
}

fn render_library(frame: &mut Frame, launcher: &Launcher, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);
    let items: Vec<ListItem<'_>> = launcher
        .scan
        .entries
        .iter()
        .map(|entry| {
            ListItem::new(Line::from(vec![
                Span::styled(&entry.title, Style::default().fg(Color::White)),
                Span::raw(format!("  [{}]", entry.cartridge_type)),
            ]))
        })
        .collect();
    let mut state = ListState::default().with_selected(
        (!launcher.scan.entries.is_empty()).then_some(launcher.selected),
    );
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" ROM 库 "))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> "),
        columns[0],
        &mut state,
    );

    let detail = launcher
        .scan
        .entries
        .get(launcher.selected)
        .map(render_rom_detail)
        .unwrap_or_else(|| "ROM 库为空。\n\n按 F2 打开文件浏览器，按 A 添加当前目录。".to_owned());
    frame.render_widget(
        Paragraph::new(detail).block(Block::default().borders(Borders::ALL).title(" 详情 ")),
        columns[1],
    );
}

fn render_browser(frame: &mut Frame, launcher: &Launcher, area: Rect) {
    let items: Vec<ListItem<'_>> = launcher
        .browser_entries
        .iter()
        .map(|entry| {
            let (prefix, path) = match entry {
                BrowserEntry::Parent(path) => ("[..] ", path),
                BrowserEntry::Directory(path) => ("[目录] ", path),
                BrowserEntry::Rom(path) => ("[ROM] ", path),
            };
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            ListItem::new(format!("{prefix}{name}"))
        })
        .collect();
    let mut state = ListState::default().with_selected(
        (!launcher.browser_entries.is_empty()).then_some(launcher.browser_selected),
    );
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" 文件浏览器：{} ", launcher.browser_directory.display())),
            )
            .highlight_style(Style::default().bg(Color::DarkGray))
            .highlight_symbol("> "),
        area,
        &mut state,
    );
}

fn render_rom_detail(entry: &RomEntry) -> String {
    format!(
        "标题：{}\n\n卡带：{}\n模式：{:?}\n大小：{} KiB\n\n路径：{}",
        entry.title,
        entry.cartridge_type,
        entry.cgb_support,
        entry.file_size / 1024,
        entry.path.display()
    )
}

fn browser_sort_key(entry: &BrowserEntry) -> String {
    let path = match entry {
        BrowserEntry::Parent(path)
        | BrowserEntry::Directory(path)
        | BrowserEntry::Rom(path) => path,
    };
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_lowercase()
}

fn is_rom_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("gb") || extension.eq_ignore_ascii_case("gbc")
        })
}

struct LauncherTerminalGuard;

impl LauncherTerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for LauncherTerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_library_selection_without_leaving_bounds() {
        let mut launcher = Launcher {
            config: LibraryConfig::default(),
            scan: ScanResult {
                entries: vec![
                    RomEntry {
                        path: PathBuf::from("a.gb"),
                        title: "A".to_owned(),
                        cartridge_type: crate::cartridge::CartridgeType::RomOnly,
                        cgb_support: crate::cartridge::CgbSupport::DmgOnly,
                        file_size: 32 * 1024,
                    },
                    RomEntry {
                        path: PathBuf::from("b.gbc"),
                        title: "B".to_owned(),
                        cartridge_type: crate::cartridge::CartridgeType::RomOnly,
                        cgb_support: crate::cartridge::CgbSupport::Compatible,
                        file_size: 32 * 1024,
                    },
                ],
                errors: Vec::new(),
            },
            selected: 0,
            mode: LauncherMode::Library,
            browser_directory: PathBuf::new(),
            browser_entries: Vec::new(),
            browser_selected: 0,
            status: String::new(),
        };

        launcher.move_selection(false);
        assert_eq!(launcher.selected, 0);
        launcher.move_selection(true);
        launcher.move_selection(true);
        assert_eq!(launcher.selected, 1);
        assert_eq!(launcher.activate(), Some(PathBuf::from("b.gbc")));
    }
}
