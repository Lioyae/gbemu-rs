# Rust DMG + MBC1 模拟器实施计划

> **面向自动化开发代理：** 必须使用 `superpowers:subagent-driven-development`
>（推荐）或 `superpowers:executing-plans` 按任务执行。使用复选框跟踪步骤。

**目标：** 使用 Rust 和 Ratatui 构建支持 ROM-only、MBC1、游戏模式与基础调试模式的
DMG 模拟器。

**架构：** 项目采用单 crate 模块化架构。硬件核心按 CPU、Bus、Timer、Joypad、PPU
和 Cartridge 拆分，由 Emulator 统一推进周期；TUI 只读取帧缓冲区与调试快照，不直接
修改硬件模块。

**技术栈：** Rust 2024、ratatui、crossterm、thiserror、clap、anyhow。

---

## 执行规则

- README、项目文档、必要代码注释和用户可见提示使用中文。
- Rust 标识符、文件名、命令、寄存器名和第三方库名保留英文。
- 每个任务完成后按计划创建职责单一的 Git 提交。
- 任务内部先写测试，再写实现；中间阶段不运行全量编译或测试。
- 仅在每个里程碑末执行集中审查、格式检查、Clippy、测试和 Release 构建。
- 集中检查发现的问题单独提交，不修改或压缩此前过程提交。
- 不提交游戏 ROM、DMG Boot ROM 或许可证不允许再分发的测试 ROM。

## 文件结构

```text
Cargo.toml                         依赖、二进制与库配置
README.md                          中文使用和开发文档
src/lib.rs                         核心模块导出
src/main.rs                        命令行入口与错误输出
src/app.rs                         应用事件循环与退出管理
src/cartridge/mod.rs               Cartridge 抽象与卡带分发
src/cartridge/header.rs            ROM 头解析与容量校验
src/cartridge/mbc1.rs              MBC1 控制寄存器与存储体映射
src/cpu/mod.rs                     CPU 状态、取指、执行和中断
src/cpu/registers.rs               寄存器对与标志位
src/cpu/instruction.rs             指令解码、操作数和反汇编元数据
src/bus.rs                         DMG 地址空间与设备路由
src/timer.rs                       DIV/TIMA/TMA/TAC
src/joypad.rs                      P1 寄存器与八个按键
src/ppu/mod.rs                     LCD 时序、寄存器和像素流水线
src/ppu/framebuffer.rs             160×144 四级灰度帧
src/emulator.rs                    周期协调、帧运行和单步
src/debugger.rs                    只读快照、反汇编和内存窗口
src/tui/mod.rs                     终端生命周期和界面分发
src/tui/game_view.rs               游戏画面布局
src/tui/debug_view.rs              调试界面布局
tests/common/mod.rs                测试 ROM 无界面运行工具
tests/rom_tests.rs                 可选外部测试 ROM 集成测试
```

## 里程碑 1：项目基础与卡带子系统

### 任务 1：建立可测试的库与命令行骨架

**文件：**
- 修改：`.gitignore`
- 修改：`Cargo.toml`
- 创建：`src/lib.rs`
- 修改：`src/main.rs`

- [ ] **步骤 1：配置项目依赖**

`Cargo.toml` 加入：

```toml
[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
crossterm = "0.29"
ratatui = "0.29"
thiserror = "2"
```

- [ ] **步骤 2：建立库入口和命令行参数**

`src/lib.rs` 导出 `cartridge`，`src/main.rs` 使用 `clap::Parser` 接收一个必需的
`rom: PathBuf` 参数。此任务只验证参数和文件读取，不进入终端原始模式。

- [ ] **步骤 3：提交基础工程**

```bash
git add .gitignore Cargo.toml Cargo.lock src/lib.rs src/main.rs
git commit -m "chore: 建立模拟器项目基础"
```

### 任务 2：以测试定义卡带头行为

**文件：**
- 创建：`src/cartridge/mod.rs`
- 创建：`src/cartridge/header.rs`

- [ ] **步骤 1：编写卡带头单元测试**

测试使用内存中的 `0x8000` 字节 ROM，覆盖：

```rust
assert_eq!(header.title(), "TEST");
assert_eq!(header.cartridge_type(), CartridgeType::RomOnly);
assert_eq!(header.rom_size(), 32 * 1024);
assert_eq!(header.ram_size(), 0);
```

同时覆盖 ROM 过小、未知卡带类型、不支持的 ROM 容量编码和不支持的 RAM 容量编码。

- [ ] **步骤 2：提交失败测试**

```bash
git add src/cartridge/mod.rs src/cartridge/header.rs
git commit -m "test: 定义卡带头解析行为"
```

### 任务 3：实现卡带头解析

**文件：**
- 修改：`src/cartridge/header.rs`
- 修改：`src/cartridge/mod.rs`

- [ ] **步骤 1：实现公开类型**

```rust
pub enum CartridgeType {
    RomOnly,
    Mbc1,
    Mbc1Ram,
    Mbc1RamBattery,
}

pub struct CartridgeHeader {
    title: String,
    cartridge_type: CartridgeType,
    rom_size: usize,
    ram_size: usize,
}
```

实现 `CartridgeHeader::parse(&[u8]) -> Result<Self, CartridgeError>`，解析标题
`0x0134..=0x0143`、类型 `0x0147`、ROM 容量 `0x0148` 和 RAM 容量 `0x0149`。

- [ ] **步骤 2：实现中文错误**

错误类型明确区分 ROM 太小、不支持的卡带类型、不支持的 ROM 容量编码、
不支持的 RAM 容量编码和实际 ROM 长度小于声明长度。

- [ ] **步骤 3：提交实现**

```bash
git add src/cartridge/mod.rs src/cartridge/header.rs
git commit -m "feat: 实现卡带头解析"
```

### 任务 4：以测试定义 ROM-only 与 MBC1

**文件：**
- 修改：`src/cartridge/mod.rs`
- 创建：`src/cartridge/mbc1.rs`

- [ ] **步骤 1：编写 ROM-only 测试**

覆盖固定 ROM 读取、越界地址返回 `0xff`、ROM 写入不改变内容，以及无 RAM 时外部
RAM 区域返回 `0xff`。

- [ ] **步骤 2：编写 MBC1 测试**

使用每个存储体填充不同字节值的合成 ROM，覆盖：

- 初始 `0x0000..=0x3fff` 为存储体 0。
- 初始 `0x4000..=0x7fff` 为存储体 1。
- 写入 `0x2000..=0x3fff` 切换低五位。
- 低五位为 0 时映射为 1。
- 写入 `0x4000..=0x5fff` 设置高两位。
- 写入 `0x6000..=0x7fff` 切换 ROM/RAM 模式。
- `0x0000..=0x1fff` 仅在低四位等于 `0x0a` 时启用 RAM。
- RAM 模式下选择并隔离不同 RAM 存储体。

- [ ] **步骤 3：提交失败测试**

```bash
git add src/cartridge/mod.rs src/cartridge/mbc1.rs
git commit -m "test: 定义 ROM-only 与 MBC1 行为"
```

### 任务 5：实现 Cartridge 与 MBC1

**文件：**
- 修改：`src/cartridge/mod.rs`
- 修改：`src/cartridge/mbc1.rs`
- 修改：`src/main.rs`

- [ ] **步骤 1：实现卡带接口**

```rust
pub trait MemoryBankController {
    fn read_rom(&self, address: u16) -> u8;
    fn write_rom(&mut self, address: u16, value: u8);
    fn read_ram(&self, address: u16) -> u8;
    fn write_ram(&mut self, address: u16, value: u8);
}
```

`Cartridge::from_bytes(Vec<u8>)` 根据卡带头创建 ROM-only 或 MBC1 实例，并暴露
`header()`、`read_rom()`、`write_rom()`、`read_ram()` 和 `write_ram()`。

- [ ] **步骤 2：实现 MBC1 映射**

所有 ROM/RAM 索引在访问前校验；未启用 RAM、无 RAM 或超出声明范围时读取 `0xff`，
写入被忽略。实际存储体数量用于规范化存储体编号。

- [ ] **步骤 3：让入口加载卡带**

命令行读取 ROM 后调用 `Cartridge::from_bytes`，成功时打印中文卡带摘要，
失败时通过 `anyhow::Context` 输出路径上下文。

- [ ] **步骤 4：提交实现**

```bash
git add src/cartridge src/main.rs
git commit -m "feat: 实现 ROM-only 与 MBC1 卡带"
```

### 任务 6：里程碑 1 集中审查与验证

**文件：**
- 检查：里程碑 1 的全部变更
- 修复范围：`.gitignore`
- 修复范围：`Cargo.toml`
- 修复范围：`src/lib.rs`
- 修复范围：`src/main.rs`
- 修复范围：`src/cartridge/mod.rs`
- 修复范围：`src/cartridge/header.rs`
- 修复范围：`src/cartridge/mbc1.rs`

- [ ] **步骤 1：审查本里程碑提交差异**

```bash
git diff 9883e30..HEAD
```

重点检查卡带头边界、MBC1 禁止存储体重映射、容量校验、错误信息和未检查索引。

- [ ] **步骤 2：执行唯一一次全量验证**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

预期：四条命令均以状态码 0 结束，卡带单元测试全部通过。

- [ ] **步骤 3：修复并提交审查问题**

```bash
git add -u
git commit -m "fix: 修正卡带里程碑审查问题"
```

没有发现问题时不创建空提交。

## 里程碑 2：CPU 核心与中断

### 任务 7：实现寄存器与取指基础

**文件：**
- 创建：`src/cpu/mod.rs`
- 创建：`src/cpu/registers.rs`
- 创建：`src/cpu/instruction.rs`
- 修改：`src/lib.rs`

- [ ] 先提交寄存器对、标志位掩码、Boot ROM 后初始状态和取指行为测试。
- [ ] 实现 `Registers`、`Cpu`、`ImeState`、操作数读取和栈辅助函数。
- [ ] 提交：`feat: 实现 CPU 寄存器与取指基础`。

### 任务 8：实现非 CB 指令集

**文件：**
- 修改：`src/cpu/mod.rs`
- 修改：`src/cpu/instruction.rs`

- [ ] 按加载、8 位算术、16 位算术、跳转、调用、返回、栈和控制指令分组编写测试提交。
- [ ] 使用 `match opcode` 实现 `0x00..=0xff` 非 CB 指令，并准确返回分支与非分支周期。
- [ ] 非法 DMG 操作码返回类型化执行错误，不触发未定义行为。
- [ ] 提交：`feat: 实现 LR35902 基础指令集`。

### 任务 9：实现 CB 指令、中断与 HALT

**文件：**
- 修改：`src/cpu/mod.rs`
- 修改：`src/cpu/instruction.rs`

- [ ] 测试八类旋转/移位、BIT、RES、SET、DAA、中断优先级、EI 延迟和 HALT bug。
- [ ] 实现完整 `0xcb00..=0xcbff` 指令、中断入口与 HALT 状态。
- [ ] 提交：`feat: 完成 CPU 扩展指令与中断`。

### 任务 10：里程碑 2 集中审查与验证

- [ ] 审查所有操作码覆盖、标志位公式、条件分支周期和中断边界。
- [ ] 运行格式、Clippy、CPU 重点测试、全量测试和 Release 构建。
- [ ] 审查修复提交：`fix: 修正 CPU 里程碑审查问题`。

## 里程碑 3：Bus、Timer、Joypad 与 DMA

### 任务 11：实现 Bus 地址空间

**文件：**
- 创建：`src/bus.rs`
- 修改：`src/lib.rs`

- [ ] 测试所有 DMG 地址区间、Echo RAM、禁止区、IF/IE 和 APU 占位寄存器。
- [ ] 实现 `Bus::read8`、`write8`、`read16`、`write16` 和中断请求接口。
- [ ] 提交：`feat: 实现 DMG 内存总线`。

### 任务 12：实现 Timer 与 Joypad

**文件：**
- 创建：`src/timer.rs`
- 创建：`src/joypad.rs`
- 修改：`src/bus.rs`

- [ ] 测试四种 TAC 频率、DIV 重置、TIMA 溢出重载、P1 选择位和按键下降沿中断。
- [ ] 以内部 16 位分频计数器的下降沿实现 Timer。
- [ ] 使用 `JoypadButton` 枚举维护八个按键并生成 P1 值。
- [ ] 提交：`feat: 实现定时器与手柄输入`。

### 任务 13：实现 OAM DMA

**文件：**
- 修改：`src/bus.rs`

- [ ] 测试 `FF46` 启动、160 字节复制、DMA 期间 CPU 总线限制和完成时序。
- [ ] 按周期推进 DMA，允许 HRAM 访问并限制其他 CPU 访问。
- [ ] 提交：`feat: 实现 OAM DMA`。

### 任务 14：里程碑 3 集中审查与验证

- [ ] 审查地址边界、定时器边沿、DMA 限制和中断位操作。
- [ ] 运行格式、Clippy、设备重点测试、全量测试和 Release 构建。
- [ ] 审查修复提交：`fix: 修正总线设备里程碑审查问题`。

## 里程碑 4：PPU 与帧缓冲区

### 任务 15：实现 PPU 寄存器和 LCD 时序

**文件：**
- 创建：`src/ppu/mod.rs`
- 创建：`src/ppu/framebuffer.rs`
- 修改：`src/bus.rs`
- 修改：`src/lib.rs`

- [ ] 测试 VRAM/OAM 访问限制、LCD 开关、Mode 2/3/0/1 周期、LY/LYC 和 STAT 中断。
- [ ] 实现 456 点每扫描线、154 扫描线和 VBlank 帧边界。
- [ ] 提交：`feat: 实现 PPU 寄存器与 LCD 时序`。

### 任务 16：实现背景、窗口与精灵渲染

**文件：**
- 修改：`src/ppu/mod.rs`
- 修改：`src/ppu/framebuffer.rs`

- [ ] 测试两种图块寻址、SCX/SCY、WX/WY、BGP、OBP、翻转、透明色和优先级。
- [ ] 每条可见扫描线生成 160 个 `Shade`，按背景、窗口、最多十个精灵合成。
- [ ] 提交：`feat: 实现 DMG 图形渲染`。

### 任务 17：里程碑 4 集中审查与验证

- [ ] 审查 LCD 周期、STAT 边沿、精灵选择和像素边界。
- [ ] 运行格式、Clippy、PPU 重点测试、全量测试和 Release 构建。
- [ ] 审查修复提交：`fix: 修正 PPU 里程碑审查问题`。

## 里程碑 5：模拟器协调与 ROM 测试

### 任务 18：实现 Emulator

**文件：**
- 创建：`src/emulator.rs`
- 修改：`src/lib.rs`

- [ ] 测试 post-Boot 状态、单步周期传播、帧完成、暂停和中断协调。
- [ ] 实现 `step_instruction`、`run_until_frame`、`set_button` 和帧读取接口。
- [ ] 提交：`feat: 实现模拟器周期协调层`。

### 任务 19：实现无界面 ROM 测试工具

**文件：**
- 创建：`tests/common/mod.rs`
- 创建：`tests/rom_tests.rs`

- [ ] 实现按周期上限运行、检测串口测试输出、识别通过/失败文本的测试工具。
- [ ] 外部 ROM 不存在时使用明确的 ignored test，不伪造通过结果。
- [ ] 提交：`test: 添加无界面 ROM 测试工具`。

### 任务 20：里程碑 5 集中审查与验证

- [ ] 审查周期传播、帧退出条件、死循环上限和测试结果判断。
- [ ] 运行格式、Clippy、协调层重点测试、全量测试和 Release 构建。
- [ ] 审查修复提交：`fix: 修正模拟器协调里程碑问题`。

## 里程碑 6：TUI 与基础调试器

### 任务 21：实现调试快照和反汇编

**文件：**
- 创建：`src/debugger.rs`
- 修改：`src/lib.rs`

- [ ] 测试快照不改变 CPU/Bus、指令长度、立即数格式和内存窗口边界。
- [ ] 实现寄存器、标志、中断、LCD 状态、附近指令和内存字节快照。
- [ ] 提交：`feat: 实现基础调试快照`。

### 任务 22：实现终端生命周期和游戏界面

**文件：**
- 创建：`src/tui/mod.rs`
- 创建：`src/tui/game_view.rs`
- 创建：`src/app.rs`
- 修改：`src/main.rs`

- [ ] 测试按键到 Joypad 映射、终端尺寸判断和退出事件。
- [ ] 使用 Ratatui Canvas/Buffer 绘制双像素单元格、状态栏和中文按键提示。
- [ ] 使用 RAII 保护对象恢复原始模式、备用屏幕和光标。
- [ ] 提交：`feat: 实现 TUI 游戏模式`。

### 任务 23：实现调试界面与模式切换

**文件：**
- 创建：`src/tui/debug_view.rs`
- 修改：`src/tui/mod.rs`
- 修改：`src/app.rs`

- [ ] 测试 Tab、Space、N、PageUp、PageDown 和只在暂停时单步。
- [ ] 绘制寄存器、标志、中断、反汇编和内存面板。
- [ ] 提交：`feat: 实现 TUI 调试模式`。

### 任务 24：里程碑 6 集中审查与验证

- [ ] 审查终端恢复、输入释放、暂停语义、布局边界和中文提示。
- [ ] 运行格式、Clippy、TUI 逻辑测试、全量测试和 Release 构建。
- [ ] 人工启动程序确认正常退出与错误退出均恢复终端。
- [ ] 审查修复提交：`fix: 修正 TUI 里程碑审查问题`。

## 里程碑 7：兼容性、README 与发布加固

### 任务 25：运行公开测试 ROM 与实际游戏

**文件：**
- 修改：`tests/rom_tests.rs`
- 修复范围：`src/cartridge/mod.rs`
- 修复范围：`src/cartridge/mbc1.rs`
- 修复范围：`src/cpu/mod.rs`
- 修复范围：`src/cpu/instruction.rs`
- 修复范围：`src/bus.rs`
- 修复范围：`src/timer.rs`
- 修复范围：`src/joypad.rs`
- 修复范围：`src/ppu/mod.rs`
- 修复范围：`src/emulator.rs`

- [ ] 使用合法获取的 CPU 和 PPU 测试 ROM 运行 ignored tests。
- [ ] 记录每项测试结果，不将测试 ROM 加入 Git。
- [ ] 修复结果明确指向的硬件行为，每类修复单独提交。
- [ ] 人工验证至少一款 ROM-only 与一款 MBC1 游戏达到可玩状态。

### 任务 26：编写中文 README

**文件：**
- 创建：`README.md`

- [ ] 写明项目状态、支持范围、构建要求、运行命令、按键、调试模式、测试方式、
  外部 ROM 目录、版权声明、架构和已知限制。
- [ ] 明确标注当前不支持音频、CGB、存档状态与非 MBC1 控制器。
- [ ] 提交：`docs: 添加中文使用说明`。

### 任务 27：最终集中审查与验收

- [ ] 从设计规格逐项核对验收标准。
- [ ] 运行 `cargo fmt --check`。
- [ ] 运行 `cargo clippy --all-targets --all-features -- -D warnings`。
- [ ] 运行 `cargo test --all-targets`。
- [ ] 运行已配置的 ignored ROM tests。
- [ ] 运行 `cargo build --release`。
- [ ] 检查 `git status --short`，确认无意外生成物被提交。
- [ ] 最终修复提交：`fix: 完成首版发布验收`；没有修复时不创建空提交。
