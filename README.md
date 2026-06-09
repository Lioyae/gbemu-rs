# gbmeu

`gbmeu` 是一个使用 Rust 编写、运行于终端中的 Nintendo Game Boy DMG 模拟器。
界面基于 Ratatui 和 Crossterm，提供游戏画面与基础调试器两种模式。

项目当前处于开发阶段。核心模块已有自动化测试，但尚未使用外部公开测试 ROM 和实际游戏
完成兼容性验收。

## 当前功能

- LR35902 基础指令集与全部 CB 扩展指令。
- CPU 标志位、中断、EI 延迟、HALT 和 HALT bug。
- DMG 内存映射、Echo RAM、HRAM、IF/IE 和 OAM DMA。
- DIV、TIMA、TMA、TAC 及定时器中断。
- 八个 Game Boy 按键与 Joypad 中断。
- ROM-only 卡带。
- MBC1 ROM/RAM 存储体切换。
- DMG PPU LCD 模式、VBlank、STAT、LY/LYC。
- 背景、窗口、8×8/8×16 精灵和四级灰度帧缓冲。
- TUI 游戏模式。
- CPU 寄存器、标志位、反汇编和内存查看调试模式。
- 暂停、继续和单指令步进。
- 无界面串口测试 ROM 运行工具。

## 尚未支持

- 音频输出和完整 APU 行为。
- Game Boy Color。
- MBC2、MBC3、MBC5 等其他卡带控制器。
- MBC1 电池 RAM 持久化。
- DMG Boot ROM 启动过程。
- 存档状态、倒带、作弊和联机线。
- 调试断点、监视点和执行历史。

PPU 当前使用固定的 Mode 3 周期模型，未模拟像素 FIFO 导致的细粒度时序变化。

## 环境要求

- Rust 1.85 或更高版本。
- 支持真彩色和 Unicode 上半块字符的终端。
- 游戏模式终端至少为 `162×76`。
- 调试模式终端至少为 `120×36`。

Windows Terminal、现代 PowerShell 终端或同等能力的终端较适合运行。

## 构建

```powershell
cargo build --release
```

生成的程序位于：

```text
target/release/gbmeu.exe
```

## 运行

```powershell
cargo run --release -- path/to/game.gb
```

也可以直接运行：

```powershell
target/release/gbmeu.exe path/to/game.gb
```

程序不会附带任何游戏 ROM 或 Boot ROM。请仅使用自己合法持有并有权使用的 ROM。

## 控制方式

| 主机按键 | Game Boy / 程序功能 |
| --- | --- |
| 方向键 | 十字键 |
| `Z` | B |
| `X` | A |
| `Enter` | Start |
| `Backspace` | Select |
| `Tab` | 切换游戏/调试模式 |
| `Space` | 暂停或继续 |
| `N` | 暂停时执行一条指令 |
| `PageUp` | 调试内存窗口向前移动 `0x100` 字节 |
| `PageDown` | 调试内存窗口向后移动 `0x100` 字节 |
| `Q` 或 `Esc` | 退出 |

## 调试模式

调试模式显示：

- AF、BC、DE、HL、SP 和 PC。
- Z、N、H、C 标志位。
- IME、HALT、IE 和 IF。
- 当前 LCD 模式与 LY。
- 从当前 PC 开始的反汇编。
- 可翻页的 128 字节内存窗口。

按下 `Space` 暂停后，可使用 `N` 逐条执行指令。

## 自动化测试

运行格式、静态检查和全部默认测试：

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

外部测试 ROM 不会提交到仓库。可按以下结构放置：

```text
test-roms/
  blargg/
    cpu_instrs.gb
    instr_timing.gb
```

放置后手动运行被忽略的测试：

```powershell
cargo test --test rom_tests -- --ignored --nocapture
```

未提供文件时，这些测试保持 `ignored`，不会被计为通过。

## 项目结构

```text
src/
  cartridge/    ROM 头、ROM-only 和 MBC1
  cpu/          寄存器、指令执行和中断
  ppu/          LCD 时序、渲染和帧缓冲
  tui/          游戏与调试界面
  app.rs        输入和应用状态
  bus.rs        DMG 地址空间与设备路由
  debugger.rs   调试快照和反汇编
  emulator.rs   CPU 与硬件周期协调
  joypad.rs     按键矩阵
  timer.rs      DIV/TIMA/TMA/TAC
```

硬件核心不依赖终端界面，因此可以通过单元测试和无界面 ROM 工具独立运行。

## 已知限制

- 尚未通过 Blargg、Mooneye 等完整外部测试套件。
- 尚未记录 ROM-only 和 MBC1 实际游戏的人工可玩验证结果。
- PPU 时序为扫描线级近似，依赖精确像素 FIFO 行为的程序可能显示异常。
- STOP、DMA 总线争用、定时器边界和部分中断时序仍可能需要测试 ROM 校正。
- 不产生声音。
- MBC1 电池存档不会写入磁盘。

## 开发策略

开发按大版本里程碑推进。每个功能过程使用独立 Git 提交；格式检查、Clippy、全量测试、
Release 构建和集中代码审查仅在里程碑结束时执行，以减少重复编译成本。

设计与实施计划位于 `docs/superpowers/`。项目文档、必要代码注释和用户可见提示使用中文；
Rust 标识符和硬件寄存器名称保留英文。
