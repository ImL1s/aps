# Project: aps (Clean-Room PlayStation 1 Emulator in Rust)

## Architecture
- Workspace layout: Cargo workspace with `crates/ps_core` (library crate) and `crates/aps` (CLI binary crate).
- `ps_core`: MIPS R3000A CPU core, COP0 coprocessor, Bus trait & memory mapper, 2MB RAM, 512KB BIOS ROM, 1KB Scratchpad, GPU with software rasterizer, 7-channel DMA controller, 3 hardware timers, Interrupt Controller (INTC), Controller input registers.
- `aps`: CLI argument parsing (`clap`), Headless execution runner with TTY stdout capturing, SDL2 GUI rendering frontend with 60 FPS timing.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Cargo Workspace Setup | Multi-crate workspace setup (`ps_core` lib, `aps` bin) | M1 | survey |
| 2 | MIPS R3000A CPU Core | 32 GPRs, HI/LO, PC, R/I/J instruction set, delay slots | M1 | survey |
| 3 | COP0 Control Coprocessor | Status, Cause, EPC, BadVAddr, PRId, exception vectors | M1 | survey |
| 4 | Memory Bus & Bus Trait | Bus trait, 2MB RAM, 512KB BIOS, 1KB Scratchpad, IO routing | M1 | survey |
| 5 | Address Segment Masking | KUSEG/KSEG0/KSEG1 0x1FFFFFFF physical masking & RAM mirroring | M1 | survey |
| 6 | Tier 1 Unit Tests (MockBus) | Individual instruction unit tests using MockBus | M1 | survey |
| 7 | Interrupt Controller (INTC) | I_STAT / I_MASK registers, 11 interrupt line assertions | M2 | survey |
| 8 | Hardware Timers | 3 timers (Timer 0/1/2) with target/mode/counter logic | M2 | survey |
| 9 | 7-Channel DMA Controller | Channels 0-6, Block/Linked-List transfers, DPCR/DICR registers | M2 | survey |
| 10 | GPU & Software Rasterizer | GP0 commands, GP1 control, 1MB VRAM, BGR555 software rasterizer | M2 | survey |
| 11 | BIOS Boot Sequence | Executing BIOS reset vector 0xBFC0_0000, RAM clear, syscalls | M2 | survey |
| 12 | Tier 2 Boundary Tests | Unaligned memory access exceptions, memory mirroring edge cases | M2 | survey |
| 13 | Dual Frontend CLI Parsing | `clap` args for `--headless`, `--bios`, `--max-cycles`, `<rom_path>` | M3 | survey |
| 14 | Headless Execution Mode | Non-interactive execution loop, BIOS TTY B0(0x3D) stdout capture | M3 | survey |
| 15 | SDL2 GUI Mode Frontend | SDL2 window, 60FPS sync, 16-bit VRAM to 32-bit ARGB texture | M3 | survey |
| 16 | Keyboard & Input Mapping | Controller IO 0x1F80_1040 mapping (D-Pad, buttons, L1/R1/L2/R2) | M3 | survey |
| 17 | Tier 3 Integration Tests | CPU-DMA-GPU interaction, timer-interrupt chaining | M3 | survey |
| 18 | Automated Test ROM Harness | `scripts/run_ps1_tests.sh` downloading/running test ROMs | M4 | survey |
| 19 | Amidog CPU Test Suite | Passing all 101 instruction verification assertions in psxtest_cpu.exe | M4 | survey |
| 20 | E2E BIOS Boot & ROM Test | Headless BIOS boot & test ROM execution without crashing | M4 | survey |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | M1 | R1: Core System Architecture & MIPS R3000A CPU Engine | none | DONE |
| 2 | M2 | R2: Subsystems (GPU/DMA/Timers/INTC) & BIOS Boot Pipeline | M1 | DONE |
| 3 | M3 | R3: Dual Frontends (Headless CLI & SDL2 GUI) | M1, M2 | DONE |
| 4 | M4 | R4: 4-Tier Test Suite & Automated ROM Runner (`run_ps1_tests.sh`) | M1, M2, M3 | DONE |

## Interface Contracts
### CPU ↔ Bus Trait
- `Bus::read8(&mut self, addr: u32) -> u8`
- `Bus::read16(&mut self, addr: u32) -> u16`
- `Bus::read32(&mut self, addr: u32) -> u32`
- `Bus::write8(&mut self, addr: u32, val: u8)`
- `Bus::write16(&mut self, addr: u32, val: u16)`
- `Bus::write32(&mut self, addr: u32, val: u32)`

### Bus ↔ Subsystems
- `Subsystems::step(&mut self, cycles: u32) -> Option<InterruptFlags>`
- `GPU::read_gpu_stat(&self) -> u32`
- `DMA::step_dma(&mut self, ram: &mut Ram, gpu: &mut Gpu) -> bool`

### Core ↔ Frontend
- `HeadlessRunner::run(&mut self, max_cycles: Option<u64>) -> Result<ExitDiagnostics>`
- `Sdl2Frontend::run_loop(&mut self) -> Result<()>`

## Code Layout
- `Cargo.toml` (workspace manifest)
- `crates/ps_core/` (core library)
  - `src/lib.rs`
  - `src/cpu/` (`mod.rs`, `registers.rs`, `cop0.rs`, `decoder.rs`, `instructions.rs`)
  - `src/bus/` (`mod.rs`, `mock_bus.rs`, `map.rs`)
  - `src/ram/` (`mod.rs`)
  - `src/bios/` (`mod.rs`)
  - `src/gpu/` (`mod.rs`, `gp0.rs`, `gp1.rs`, `vram.rs`, `rasterizer.rs`)
  - `src/dma/` (`mod.rs`, `channel.rs`)
  - `src/timers/` (`mod.rs`, `timer.rs`)
  - `src/intc/` (`mod.rs`)
  - `src/controller/` (`mod.rs`)
- `crates/aps/` (CLI & Frontend binary)
  - `src/main.rs`
  - `src/cli.rs`
  - `src/headless.rs`
  - `src/sdl2_frontend.rs`
- `scripts/run_ps1_tests.sh`
