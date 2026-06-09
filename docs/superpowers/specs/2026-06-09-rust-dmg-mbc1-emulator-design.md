# Rust DMG + MBC1 Emulator Design

## Goal

Build a terminal-based Nintendo Game Boy DMG emulator in Rust. The first usable
release supports ROM-only and MBC1 cartridges, runs without a DMG Boot ROM,
offers game and basic debugger views through a TUI, and intentionally omits
audio output.

## Scope

The first release includes:

- LR35902 CPU instructions, flags, interrupts, HALT behavior, and cycle timing.
- DMG memory map and memory-mapped I/O.
- ROM-only and MBC1 cartridge controllers.
- DMG timer and interrupt behavior.
- DMG PPU background, window, sprites, palettes, LCD modes, and frame timing.
- Joypad input and Joypad interrupts.
- A `160 x 144` four-shade framebuffer.
- Game and debugger TUI modes.
- Pause, resume, instruction stepping, CPU state, current disassembly, and
  memory inspection.
- Automated unit and integration tests, public test ROM execution, and at
  least one manually verified MBC1 game.
- A user-facing README with setup, controls, compatibility, limitations, and
  validation instructions.

The first release excludes:

- Game Boy Color features.
- Sound generation and host audio output.
- Save states, rewind, cheats, link cable, and network features.
- Breakpoints, watchpoints, execution history, and call tracing.
- Bundled copyrighted ROMs or Boot ROM images.

APU register addresses remain mapped with minimal compatibility behavior so
games can access them without crashing, but they do not generate sound.

## Architecture

Use one Rust crate with focused modules. Emulator hardware remains independent
from terminal rendering, allowing deterministic tests without a terminal.

```text
src/
  main.rs
  app.rs
  cartridge/
    mod.rs
    header.rs
    mbc1.rs
  cpu/
    mod.rs
    instruction.rs
    registers.rs
  bus.rs
  timer.rs
  joypad.rs
  ppu/
    mod.rs
    framebuffer.rs
  emulator.rs
  debugger.rs
  tui/
    mod.rs
    game_view.rs
    debug_view.rs
tests/
  common/
    mod.rs
  rom_tests.rs
```

Responsibilities:

- `cartridge`: load ROM bytes, validate the header, expose cartridge metadata,
  and route reads and writes through ROM-only or MBC1 banking behavior.
- `cpu`: own LR35902 registers, decode and execute one instruction at a time,
  update flags, and report consumed machine cycles.
- `bus`: implement the DMG address map and connect CPU accesses to cartridge,
  PPU, timer, Joypad, work RAM, high RAM, interrupt registers, and APU stubs.
- `timer`: implement DIV, TIMA, TMA, TAC, overflow, and timer interrupt requests.
- `ppu`: advance LCD modes from elapsed cycles, expose VRAM and OAM, enforce
  mapped registers, and render complete DMG frames.
- `joypad`: translate host key state into the P1 register and request Joypad
  interrupts on valid button transitions.
- `emulator`: initialize post-Boot-ROM state, coordinate CPU and hardware
  clocks, expose frame execution, pause/resume, and single-instruction stepping.
- `debugger`: produce immutable snapshots and disassembly without owning or
  mutating emulation state.
- `tui`: own terminal setup, input polling, layout, rendering, mode switching,
  and terminal restoration.
- `app`: coordinate ROM loading, emulator lifetime, TUI events, and shutdown.

## Execution Model

The CPU executes one instruction and returns its cycle count. The emulator
passes those cycles to the timer and PPU, then services interrupts according to
DMG priority. This cycle-driven boundary keeps hardware timing independent of
host rendering speed.

Game mode repeatedly runs enough emulated cycles to produce a frame, renders
the latest framebuffer, and uses host timing to target approximately 59.7
frames per second. Debug mode may pause execution, step exactly one CPU
instruction, and refresh a read-only snapshot after each action.

The TUI converts each pair of vertical DMG pixels into one terminal cell where
possible. Foreground and background grayscale colors preserve both pixels,
reducing the required display height while retaining the full framebuffer.
When terminal color support is insufficient, a four-character grayscale
fallback is used.

## Cartridge And Startup Behavior

The application accepts a ROM path from the command line. Header cartridge type
values select either ROM-only or MBC1 behavior. Unsupported cartridge types
produce a clear error before the terminal enters raw mode.

MBC1 implements:

- RAM enable.
- Lower five ROM bank bits, including the forbidden-bank remap.
- Upper bank bits.
- ROM banking mode.
- RAM banking mode.
- Optional external RAM based on header size.

The emulator does not load a Boot ROM. It initializes CPU registers and mapped
hardware registers to documented post-DMG-Boot-ROM values, starts execution at
`0x0100`, and leaves `0x0000..=0x00ff` mapped to cartridge ROM.

## TUI And Controls

Game mode displays:

- The scaled Game Boy framebuffer.
- ROM title and cartridge type.
- Running or paused state.
- Measured FPS.
- A compact control reminder.

Debugger mode displays:

- CPU registers and flags.
- Interrupt state and current LCD mode.
- Current instruction plus nearby disassembly.
- A scrollable memory window.
- Run, pause, and single-step status.

Default controls:

| Host key | Action |
| --- | --- |
| Arrow keys | D-pad |
| `Z` | B |
| `X` | A |
| `Enter` | Start |
| `Backspace` | Select |
| `Tab` | Switch game/debugger mode |
| `Space` | Pause/resume |
| `N` | Step one instruction while paused |
| `PageUp` / `PageDown` | Move debugger memory window |
| `Q` or `Esc` | Quit |

Control mapping is documented but not configurable in the first release.

## Error Handling

Library-facing modules return typed errors for ROM loading and cartridge header
validation. The binary adds context for file paths and user actions before
printing errors.

Expected user errors include:

- Missing or unreadable ROM files.
- ROM files too small to contain a valid header.
- Unsupported cartridge controller or declared ROM/RAM size.
- Terminal too small for either view.
- Terminal initialization or event polling failure.

The terminal restoration guard is created immediately after entering raw mode
and restores the screen and cursor on normal exit or propagated errors. Runtime
hardware invariants use debug assertions where practical; malformed games must
not cause unchecked indexing or memory safety failures.

## Testing Strategy

Unit tests cover deterministic hardware behavior:

- CPU instruction results, flags, program counter changes, stack behavior, and
  cycle counts.
- Bus address routing, echo RAM, prohibited ranges, DMA, and interrupt flags.
- Timer frequencies, DIV reset, overflow, reload, and interrupt requests.
- MBC1 ROM/RAM bank selection and mode changes.
- Joypad selection bits, button transitions, and interrupts.
- PPU tile decoding, palettes, sprite priority, LCD mode timing, and frame
  boundaries.
- Debugger formatting and non-mutating memory inspection.

Integration tests run redistributable or user-supplied public test ROMs through
a headless emulator harness with cycle and timeout limits. The documented
validation target is:

- CPU instruction and timing test ROMs pass.
- PPU behavior test ROMs selected for implemented DMG behavior pass.
- At least one ROM-only game reaches playable output.
- At least one MBC1 game switches banks and reaches playable output.

No third-party test ROM binaries are committed unless their licenses explicitly
permit redistribution. The README documents where users can place external test
assets locally.

## Development And Review Policy

Implementation is divided into large milestones. Work inside each milestone is
saved through focused Git commits, but expensive review and verification occur
once at the milestone boundary.

Each milestone ends with:

1. Review all changes made since the previous milestone tag or review commit.
2. Run `cargo fmt --check`.
3. Run `cargo clippy --all-targets --all-features -- -D warnings`.
4. Run the milestone's focused tests.
5. Run `cargo test --all-targets`.
6. Run `cargo build --release`.
7. Record fixes in one or more focused commits.

Intermediate commits do not require a full compile and test cycle. A narrow
command may still be run when needed to diagnose a compiler error or verify
high-risk behavior, but repeated full-project checks are avoided.

Planned milestones:

1. Project foundation and cartridge subsystem.
2. CPU core and interrupts.
3. Bus, timer, Joypad, and DMA.
4. PPU and framebuffer.
5. Emulator orchestration and headless ROM tests.
6. TUI game/debugger modes and input.
7. Compatibility validation, README, and release hardening.

## Documentation

The final `README.md` contains:

- Project status and supported hardware.
- Build and run commands.
- ROM ownership notice.
- Complete controls.
- Game and debugger mode descriptions.
- Supported and unsupported cartridge features.
- Test commands and optional external test ROM layout.
- Known limitations, including absent audio.
- High-level architecture and contribution guidance.

## Acceptance Criteria

The first release is complete when:

- A release build accepts a ROM path and restores the terminal correctly on
  normal exit and errors.
- ROM-only and MBC1 cartridges are parsed and banked correctly.
- CPU, timer, interrupt, Joypad, DMA, and PPU milestone tests pass.
- Game mode shows stable four-shade DMG frames and accepts all eight controls.
- Debug mode switches at runtime and supports pause, resume, one-instruction
  stepping, register display, disassembly, and memory inspection.
- Selected public CPU and PPU test ROMs pass in the headless harness.
- At least one ROM-only and one MBC1 game are manually verified as playable.
- Audio is explicitly reported as unsupported.
- The README accurately describes build, usage, controls, validation, and
  limitations.
