# gbmeu-rs

`gbmeu-rs` 是一个使用 Rust 编写、运行于终端中的 Nintendo Game Boy 与
Game Boy Color 模拟器。界面基于 Ratatui 和 Crossterm，提供 ROM 启动器、
游戏画面和基础调试器。

项目仍处于开发阶段。目前已经覆盖主要 CPU、内存、PPU、卡带控制器、电池存档和
即时存档流程，并支持用户提供的 Boot ROM；音频仍未实现。

## 当前功能

- LR35902 基础指令集与全部 CB 扩展指令。
- CPU 标志位、中断、EI 延迟、HALT、HALT bug、STOP 和 CGB 双速切换。
- DMG/CGB 内存映射、CGB WRAM/VRAM 分 bank、Echo RAM、HRAM、IF/IE。
- OAM DMA、CGB GDMA 与 HBlank DMA。
- DIV、TIMA、TMA、TAC 及定时器中断。
- DMG/CGB 串口内部时钟、CGB 快速模式、串口中断和 CGB 红外端口寄存器。
- 八个 Game Boy 按键与 Joypad 中断。
- DMG 灰度渲染和 CGB 15 位彩色背景、窗口、精灵与调色板。
- 自动根据卡带头选择 DMG 或 CGB 模式。
- TUI ROM 库、递归目录扫描和文件浏览器。
- TUI 游戏模式以及寄存器、反汇编、内存查看调试模式。
- 暂停、继续和单指令步进。
- 手动电池存档、RTC/Flash 持久化和未保存退出确认。
- 带 ROM 哈希与硬件模式校验的 10 槽即时存档。
- 可选的 256 字节 DMG 与 2304 字节 CGB Boot ROM 启动。
- 无界面串口测试 ROM 运行工具。

## 卡带支持

能够识别 Game Boy 官方卡带头中定义的全部 28 种卡带类型，并为以下控制器提供实现：

- ROM、ROM+RAM。
- MBC1、MBC2、MBC3、MBC5。
- MMM01、MBC6、MBC7。
- Pocket Camera、Bandai TAMA5。
- HuC1、HuC3。

MBC3、TAMA5 和 HuC3 包含 RTC 持久化；MBC6 包含 Flash 持久化；MBC7 包含
EEPROM 协议。摄像头、红外、倾斜传感器和震动等依赖真实外设的能力目前使用稳定的
兼容占位行为，不会访问主机硬件。

## 环境要求

- Rust 1.85 或更高版本。
- 支持真彩色和 Unicode 上半块字符的终端。
- ROM 启动器终端至少为 `100×24`。
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

## 启动 ROM

不带参数运行时进入 TUI ROM 启动器：

```powershell
cargo run --release
```

也可以从命令行直接指定 `.gb` 或 `.gbc` 文件，并跳过启动器：

```powershell
cargo run --release -- path/to/game.gbc
target/release/gbmeu.exe path/to/game.gb
```

双模式 ROM 默认使用 CGB。可以手动选择硬件模式：

```powershell
target/release/gbmeu.exe --model dmg path/to/game.gb
target/release/gbmeu.exe --model cgb path/to/game.gbc
```

使用自己合法取得的 Boot ROM 时，通过对应参数指定文件：

```powershell
target/release/gbmeu.exe --dmg-boot-rom path/to/dmg_boot.bin path/to/game.gb
target/release/gbmeu.exe --cgb-boot-rom path/to/cgb_boot.bin path/to/game.gbc
```

Boot ROM 必须与最终选择的硬件模式匹配。未提供 Boot ROM 时，模拟器继续使用对应硬件的
开机后寄存器状态，从卡带入口 `0x0100` 开始执行。

Windows 下可以直接双击 `gbmeu.exe` 进入启动器。首次启动时，如果程序当前目录存在
`roms/`，会自动将其加入 ROM 库。启动器配置保存在操作系统的用户配置目录中。

程序不会附带任何游戏 ROM 或 Boot ROM。请仅使用自己合法持有并有权使用的 ROM。

## 启动器控制

| 按键 | 功能 |
| --- | --- |
| `↑` / `↓` | 移动选择 |
| `Enter` | 启动 ROM，或在文件浏览器中打开目录 |
| `F2` / `Tab` | 在 ROM 库与文件浏览器之间切换 |
| `F5` / `R` | 刷新当前视图；在 ROM 库中重新扫描 ROM |
| `Backspace` | 文件浏览器中返回上级目录 |
| `A` | 将文件浏览器当前目录加入 ROM 库 |
| `D` | 从 ROM 库移除文件浏览器当前目录 |
| `Esc` | 从文件浏览器返回 ROM 库 |
| `Q` / `Esc` | 在 ROM 库中退出 |

ROM 库会递归扫描所有已配置目录中的 `.gb` 和 `.gbc` 文件。列表会标记已有存档的
ROM，详情区域显示 GB/GBC 模式、卡带类型、ROM/RAM 容量，以及 `.sav`、`.rtc` 和
十个即时存档槽位的状态。单个损坏 ROM 不会阻止其他文件显示。

## 游戏与调试控制

| 主机按键 | Game Boy / 程序功能 |
| --- | --- |
| 方向键 | 十字键 |
| `Z` | B |
| `X` | A |
| `Enter` | Start |
| `Backspace` | Select |
| `Ctrl+S` | 将卡带持久数据写入磁盘 |
| `Ctrl+L` | 从磁盘重新加载卡带持久数据 |
| `0` 至 `9` | 选择即时存档槽位 |
| `F5` | 保存当前槽位的即时状态 |
| `F9` | 加载当前槽位的即时状态 |
| `Tab` | 切换游戏/调试模式 |
| `Space` | 暂停或继续 |
| `N` | 暂停时执行一条指令 |
| `PageUp` | 调试内存窗口向前移动 `0x100` 字节 |
| `PageDown` | 调试内存窗口向后移动 `0x100` 字节 |
| `Q` / `Esc` | 请求退出 |

存在未保存的卡带数据时，退出会要求选择保存、放弃或取消。电池 RAM、EEPROM 和 Flash
写入 ROM 同目录、同文件名的 `.sav` 文件；RTC 数据写入 `.rtc` 文件。保存过程使用
同目录临时文件和备份替换，加载时会先完整校验再修改模拟器状态。

即时存档写入 ROM 同目录的 `.state0` 至 `.state9`。文件包含格式版本、ROM SHA-256、
硬件模式和状态数据校验值；版本、ROM、模式或数据校验不匹配时不会修改当前模拟器。

## 调试模式

调试模式显示：

- AF、BC、DE、HL、SP 和 PC。
- Z、N、H、C 标志位。
- IME、HALT、IE 和 IF。
- 当前硬件模式、CPU 速度、LCD 模式与 LY。
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

外部测试 ROM 不会提交到仓库。当前集成测试从 `roms/` 读取文件。放置对应文件后可运行：

```powershell
cargo test --test rom_tests provided_cpu_instrs -- --ignored --nocapture --exact
cargo test --test rom_tests provided_mem_timing -- --ignored --nocapture --exact
cargo test --test rom_tests provided_dmg_acid2_smoke -- --ignored --nocapture --exact
cargo test --test rom_tests provided_mbc1_game_smoke -- --ignored --nocapture --exact
cargo test --test rom_tests provided_cgb_game_smoke -- --ignored --nocapture --exact
```

当前提供的 Blargg `cpu_instrs` 11 个独立测试和 `mem_timing` 3 个测试均已通过。
`dmg-acid2`、DMG 游戏和 `zelda1.gbc` 目前执行无界面冒烟验证，仍需人工检查最终画面、
声音缺失情况下的可玩性和细粒度图形正确性。

## 项目结构

```text
src/
  cartridge/    卡带头、控制器、RTC、EEPROM 和 Flash
  cpu/          寄存器、指令执行和中断
  ppu/          DMG/CGB LCD 时序、渲染和帧缓冲
  save/         卡带持久数据文件
  serial.rs     串口时钟、移位与中断
  tui/          ROM 启动器、游戏、调试和对话框
  app.rs        游戏输入、存档和应用状态
  bus.rs        DMG/CGB 地址空间与设备路由
  debugger.rs   调试快照和反汇编
  emulator.rs   CPU 与硬件机器周期协调
  joypad.rs     按键矩阵
  library.rs    多目录 ROM 库
  model.rs      DMG/CGB 模式选择
  timer.rs      DIV/TIMA/TMA/TAC
```

硬件核心不依赖终端界面，因此可以通过单元测试和无界面 ROM 工具独立运行。

## 已知限制

- 尚未实现 APU 和音频输出。
- 尚未实现倒带、作弊和两个模拟器之间的联机线；断线串口与红外输入固定为高电平。
- PPU 使用固定 Mode 3 周期模型，未实现逐点像素 FIFO 和全部总线争用细节。
- GDMA/HDMA 的传输与 CPU 暂停时序仍是近似模型。
- Pocket Camera、红外、倾斜传感器和震动没有接入真实主机设备。
- 调试器尚无断点、监视点和执行历史。
