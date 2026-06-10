# Full GB/GBC Emulator Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将现有 DMG+MBC1 TUI 模拟器扩展为支持完整 CGB、全部官方卡带编码、APU、手动电池存档、10 槽即时存档和 ROM 库的模拟器。

**Architecture:** 先把 CPU/总线改成机器周期级调度，再在稳定时钟模型上增加 CGB、卡带和 APU。持久化与启动器通过显式接口连接核心，避免文件系统、音频设备和 TUI 侵入硬件模块。

**Tech Stack:** Rust 2024、Ratatui、Crossterm、cpal、serde、bincode、sha2、directories、walkdir。

---

## 文件结构

- `src/cpu/`：增加微操作执行状态，保留寄存器与 ALU 逻辑。
- `src/bus.rs`：改为单机器周期推进，并路由 CGB 寄存器。
- `src/model.rs`：DMG/CGB 模式与速度状态。
- `src/ppu/`：增加 CGB VRAM、调色板、HDMA 和像素 FIFO。
- `src/apu/`：四声道硬件与采样器。
- `src/audio/`：cpal 输出适配器。
- `src/cartridge/`：按控制器拆分 MBC2、MBC3、MBC5、MBC6、MBC7、MMM01、Camera、TAMA5、HuC。
- `src/save/`：电池和即时存档格式。
- `src/library/`：配置、扫描和文件浏览状态。
- `src/tui/launcher.rs`：ROM 库优先启动页。
- `src/tui/dialog.rs`：保存确认和错误对话框。

### Task 1: 固化硬件模式与卡带元数据

**Files:**
- Create: `src/model.rs`
- Modify: `src/lib.rs`
- Modify: `src/cartridge/header.rs`
- Test: `src/cartridge/header.rs`

- [ ] **Step 1: 编写失败测试**

测试 `0x143` 的 DMG、双模式、CGB-only 解析，并用表驱动断言 28 个卡带编码都能映射为结构化 `CartridgeType`。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test cartridge::header --lib`
Expected: 新增 CGB 标志和卡带类型测试失败。

- [ ] **Step 3: 实现最小元数据模型**

增加 `HardwareModel`、`CgbSupport`、`CartridgeFeatures`，让卡带类型暴露控制器、RAM、
电池、RTC、Rumble、Sensor 和 Camera 特性。

- [ ] **Step 4: 运行聚焦测试**

Run: `cargo test cartridge::header --lib`
Expected: PASS。

- [ ] **Step 5: 提交**

```powershell
git add src/model.rs src/lib.rs src/cartridge/header.rs
git commit -m "feat: 扩展 GB 与 GBC 卡带元数据"
```

### Task 2: 建立机器周期总线接口

**Files:**
- Modify: `src/cpu/mod.rs`
- Modify: `src/cpu/instruction.rs`
- Modify: `src/bus.rs`
- Modify: `src/emulator.rs`
- Test: `src/cpu/mod.rs`
- Test: `src/bus.rs`

- [ ] **Step 1: 编写失败测试**

增加记录总线访问序列的测试内存，断言 `LD (a16),SP`、`CALL`、中断服务和条件跳转的
读写发生在正确机器周期，并断言每个机器周期推进 4 个 T-cycle。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test cpu::tests::machine_cycle --lib`
Expected: 当前整指令接口无法提供访问序列。

- [ ] **Step 3: 实现机器周期接口**

为 `Memory` 增加周期回调，将取指、读写和内部等待拆为显式 M-cycle；`Emulator` 不再在
完整指令后一次性调用 `Bus::tick`。

- [ ] **Step 4: 运行 CPU 和总线测试**

Run: `cargo test cpu:: bus:: --lib`
Expected: PASS。

- [ ] **Step 5: 提交**

```powershell
git add src/cpu src/bus.rs src/emulator.rs
git commit -m "refactor: 使用机器周期驱动 CPU 与总线"
```

### Task 3: 通过内存时序回归

**Files:**
- Modify: `tests/common/mod.rs`
- Modify: `tests/rom_tests.rs`
- Modify: `src/timer.rs`
- Modify: `src/bus.rs`

- [ ] **Step 1: 保留 `mem_timing` 失败证据**

Run: `cargo test --test rom_tests provided_mem_timing -- --ignored --nocapture --exact`
Expected: 输出三个 `01` 并失败。

- [ ] **Step 2: 修正周期边界**

根据 ROM 暴露的差异调整 Timer、DMA 和总线读写所在的 M-cycle，不改变指令语义。

- [ ] **Step 3: 运行外部 CPU 与时序 ROM**

Run:

```powershell
cargo test --test rom_tests provided_cpu_instrs -- --ignored --nocapture --exact
cargo test --test rom_tests provided_mem_timing -- --ignored --nocapture --exact
```

Expected: 两者 PASS。

- [ ] **Step 4: 提交**

```powershell
git add src tests
git commit -m "fix: 对齐 CPU 内存访问时序"
```

### Task 4: 实现 CGB 内存与速度模式

**Files:**
- Modify: `src/model.rs`
- Modify: `src/bus.rs`
- Modify: `src/emulator.rs`
- Modify: `src/timer.rs`
- Test: `src/bus.rs`

- [ ] **Step 1: 编写失败测试**

覆盖 KEY1、STOP 双速切换、SVBK、VBK、CGB-only 自动模式和双模式手动覆盖。

- [ ] **Step 2: 实现 CGB 总线状态**

增加 32 KiB WRAM、双 VRAM bank、KEY1、SVBK、VBK，并让 CPU 双速不改变 PPU/APU
基准时钟。

- [ ] **Step 3: 验证**

Run: `cargo test bus:: emulator:: timer:: --lib`
Expected: PASS。

- [ ] **Step 4: 提交**

```powershell
git add src/model.rs src/bus.rs src/emulator.rs src/timer.rs
git commit -m "feat: 实现 CGB 内存与双速模式"
```

### Task 5: 实现 CGB PPU 与 HDMA

**Files:**
- Modify: `src/ppu/mod.rs`
- Modify: `src/ppu/framebuffer.rs`
- Modify: `src/bus.rs`
- Test: `src/ppu/mod.rs`

- [ ] **Step 1: 编写失败测试**

覆盖 BG/OBJ 调色板索引和自动递增、tile 属性、VRAM bank、CGB 优先级、GDMA 和 HBlank HDMA。

- [ ] **Step 2: 实现彩色渲染**

帧缓冲改为 RGB555 转 RGB888；DMG 保留四级灰度映射；CGB 根据属性读取 bank、翻转、
调色板和优先级。

- [ ] **Step 3: 实现 DMA**

实现 FF51-FF55 的 GDMA/HDMA 状态机，并在 LCD 开关和 HBlank 边界处理启动、暂停和取消。

- [ ] **Step 4: 验证并提交**

Run: `cargo test ppu:: bus:: --lib`

```powershell
git add src/ppu src/bus.rs
git commit -m "feat: 实现 CGB 彩色 PPU 与 HDMA"
```

### Task 6: 实现全部卡带头与主流控制器

**Files:**
- Modify: `src/cartridge/mod.rs`
- Create: `src/cartridge/mbc2.rs`
- Create: `src/cartridge/mbc3.rs`
- Create: `src/cartridge/mbc5.rs`
- Create: `src/cartridge/mmm01.rs`
- Test: corresponding modules

- [ ] **Step 1: 编写 MBC2、MBC3、MBC5、MMM01 失败测试**

覆盖 bank 选择、RAM 启用、MBC2 4-bit RAM、MBC3 RTC latch、MBC5 第九 ROM bank 位和
Rumble 位隔离。

- [ ] **Step 2: 实现控制器与统一持久化接口**

`MemoryBankController` 增加 `persistent_data`、`load_persistent_data`、`dirty`、
`clear_dirty` 和 `tick_rtc`。

- [ ] **Step 3: 验证并提交**

Run: `cargo test cartridge:: --lib`

```powershell
git add src/cartridge
git commit -m "feat: 实现主流官方卡带控制器"
```

### Task 7: 实现特殊卡带控制器协议

**Files:**
- Create: `src/cartridge/mbc6.rs`
- Create: `src/cartridge/mbc7.rs`
- Create: `src/cartridge/camera.rs`
- Create: `src/cartridge/tama5.rs`
- Create: `src/cartridge/huc.rs`
- Modify: `src/cartridge/mod.rs`

- [ ] **Step 1: 编写协议测试**

覆盖 MBC6 分半 ROM/RAM bank、MBC7 EEPROM 串行命令、传感器 latch、Camera 寄存器、
TAMA5 nibble 协议、HuC bank 和红外寄存器。

- [ ] **Step 2: 实现稳定占位外设**

传感器默认中心值 `0x8000`，相机生成固定灰阶帧，红外默认无信号，Rumble 只记录状态并
通过可选回调通知前端。

- [ ] **Step 3: 验证并提交**

Run: `cargo test cartridge:: --lib`

```powershell
git add src/cartridge
git commit -m "feat: 实现特殊官方卡带协议"
```

### Task 8: 实现 APU 核心

**Files:**
- Create: `src/apu/mod.rs`
- Create: `src/apu/square.rs`
- Create: `src/apu/wave.rs`
- Create: `src/apu/noise.rs`
- Modify: `src/bus.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: 编写声道失败测试**

覆盖触发、DAC 开关、长度、包络、扫频、Wave RAM、LFSR、NR50/NR51/NR52 和 DIV-APU。

- [ ] **Step 2: 实现四声道和混音**

APU 按硬件周期推进，将立体声样本写入有界环形缓冲；缓冲满时丢弃最旧样本。

- [ ] **Step 3: 验证并提交**

Run: `cargo test apu:: bus:: --lib`

```powershell
git add src/apu src/bus.rs src/lib.rs
git commit -m "feat: 实现 DMG 与 CGB APU"
```

### Task 9: 接入 cpal 音频输出

**Files:**
- Modify: `Cargo.toml`
- Create: `src/audio/mod.rs`
- Modify: `src/tui/mod.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: 增加无设备与静音状态测试**

音频前端必须允许设备初始化失败，并能调整静音和主音量而不修改 APU 寄存器。

- [ ] **Step 2: 实现 cpal 适配器**

使用默认输出设备和配置，按设备采样格式转换核心样本；回调只读共享缓冲，不持有模拟器锁。

- [ ] **Step 3: 验证并提交**

Run: `cargo test audio:: app:: --lib`

```powershell
git add Cargo.toml Cargo.lock src/audio src/tui/mod.rs src/app.rs
git commit -m "feat: 添加跨平台音频输出"
```

### Task 10: 实现手动电池存档

**Files:**
- Create: `src/save/mod.rs`
- Create: `src/save/battery.rs`
- Modify: `src/emulator.rs`
- Modify: `src/app.rs`
- Create: `src/tui/dialog.rs`

- [ ] **Step 1: 编写失败测试**

覆盖 `.sav/.rtc` 路径、原子写入、完全手动读取、脏标记和退出确认三种选择。

- [ ] **Step 2: 实现保存服务**

保存服务从卡带导出 SRAM/EEPROM/RTC；加载前验证长度和格式，失败时保持当前数据。

- [ ] **Step 3: 绑定快捷键**

`Ctrl+L` 加载，`Ctrl+S` 保存；退出时由 App 状态机显示确认框。

- [ ] **Step 4: 验证并提交**

Run: `cargo test save:: app:: --lib`

```powershell
git add src/save src/emulator.rs src/app.rs src/tui/dialog.rs
git commit -m "feat: 添加手动卡带存档"
```

### Task 11: 实现 10 槽即时存档

**Files:**
- Modify: `Cargo.toml`
- Create: `src/save/state.rs`
- Modify: `src/emulator.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: 编写失败测试**

覆盖槽位路径、格式版本、ROM SHA-256、模式校验、损坏文件和往返恢复。

- [ ] **Step 2: 实现显式状态快照**

为 CPU、Bus、PPU、APU、Timer、Joypad 和 Cartridge 定义可序列化状态；加载采用先解析校验、
后整体替换，避免部分更新。

- [ ] **Step 3: 绑定快捷键**

数字键选择 0-9，`F5` 保存，`F9` 读取，并在 TUI 显示当前槽位和操作结果。

- [ ] **Step 4: 验证并提交**

Run: `cargo test save:: emulator:: app:: --lib`

```powershell
git add Cargo.toml Cargo.lock src/save src/emulator.rs src/app.rs
git commit -m "feat: 添加十槽即时存档"
```

### Task 12: 实现可选 Boot ROM

**Files:**
- Create: `src/boot.rs`
- Modify: `src/bus.rs`
- Modify: `src/emulator.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: 编写失败测试**

覆盖 DMG 256-byte、CGB 2304-byte Boot ROM 映射、FF50 禁用和缺失时开机后状态。

- [ ] **Step 2: 实现 Boot ROM 配置**

启动参数和用户配置可分别指定 DMG/CGB Boot ROM；模式不匹配时返回中文错误。

- [ ] **Step 3: 验证并提交**

Run: `cargo test boot:: bus:: emulator:: --lib`

```powershell
git add src/boot.rs src/bus.rs src/emulator.rs src/main.rs
git commit -m "feat: 支持可选 DMG 与 CGB Boot ROM"
```

### Task 13: 实现 ROM 库与配置

**Files:**
- Modify: `Cargo.toml`
- Create: `src/library/mod.rs`
- Create: `src/library/config.rs`
- Create: `src/library/scanner.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: 编写失败测试**

使用临时目录覆盖多目录递归扫描、扩展名大小写、重复路径、坏 ROM 隔离、配置往返和存档状态。

- [ ] **Step 2: 实现配置和扫描**

使用 `directories` 确定配置目录，`walkdir` 扫描，规范化路径并按标题排序。

- [ ] **Step 3: 验证并提交**

Run: `cargo test library:: --lib`

```powershell
git add Cargo.toml Cargo.lock src/library src/lib.rs
git commit -m "feat: 添加多目录 ROM 库"
```

### Task 14: 实现启动器与文件浏览器

**Files:**
- Create: `src/tui/launcher.rs`
- Create: `src/tui/file_browser.rs`
- Modify: `src/tui/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: 编写状态测试**

覆盖列表移动、目录添加/移除、刷新、F2 文件浏览、Enter 启动、错误 ROM 显示和取消。

- [ ] **Step 2: 实现 ROM 库优先布局**

左侧游戏列表，右侧卡带详情，底部状态栏；文件浏览器作为独立页面。

- [ ] **Step 3: 保留命令行直启**

`gbmeu [ROM]` 有参数直接启动；无参数进入启动器；`--model auto|dmg|cgb` 控制双模式 ROM。

- [ ] **Step 4: 验证并提交**

Run: `cargo test tui:: library:: --lib`

```powershell
git add src/tui src/main.rs
git commit -m "feat: 添加 ROM 库启动器与文件浏览器"
```

### Task 15: 集成验证与文档

**Files:**
- Modify: `README.md`
- Modify: `tests/rom_tests.rs`

- [ ] **Step 1: 更新中文文档**

记录 GBC、卡带、音频、两类存档、ROM 库、Boot ROM、快捷键和特殊外设占位边界。

- [ ] **Step 2: 执行完整验证**

Run:

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Expected: 全部成功。

- [ ] **Step 3: 执行外部 ROM 验证**

Run 已配置的 CPU、内存时序、DMG PPU、CGB PPU、Timer、DMA、APU 和 MBC 测试 ROM，
逐项记录通过、失败和未提供状态，不把缺失 ROM 计为通过。

- [ ] **Step 4: 最终提交**

```powershell
git add README.md tests
git commit -m "docs: 完善 GB 与 GBC 模拟器使用说明"
```
