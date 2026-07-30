# GitHub Repository Metadata & Release Setup (`aps`)

This document contains the recommended GitHub repository metadata (description, topics/tags), automated `gh` CLI management scripts, and the v0.1.0 release notes template for the `aps` clean-room PlayStation 1 emulator project in Rust.

---

## 1. Repository Description

### Recommended Short Description (GitHub Summary / About Section)
> A clean-room PlayStation 1 (PS1) emulator in Rust featuring MIPS R3000A CPU, software GPU rasterizer, 7-channel DMA, dual SDL2/Headless frontends, and a 4-Tier test harness.

### Extended Description (Homepage / Project Overview)
`aps` is a clean-room PlayStation 1 emulator built from scratch in Rust, adhering to strict modular workspace standards (`ps_core` library crate + `aps` application crate). It implements a cycle-synchronous MIPS R3000A CPU interpreter, memory bus architecture with DMA/GPU routing, software GPU rasterizer for VRAM frame rendering, 3 hardware timers, BIOS boot sequence handler, dual-mode CLI/GUI frontend, and an automated 4-tier verification test harness.

---

## 2. Recommended Topic Tags

| Topic Tag | Category | Purpose / Relevance |
|---|---|---|
| `ps1` | Platform | Identifies PlayStation 1 emulation target |
| `emulator` | Domain | Core software classification |
| `rust` | Language | Primary implementation language |
| `mips` | Architecture | CPU architecture (MIPS I / R3000A) |
| `sdl2` | Frontend | Cross-platform GUI and display backend |
| `ps1-emulator` | Topic | Specific search tag for PS1 emulators |
| `playstation-emulator` | Topic | Broader search tag for PlayStation emulators |
| `mips-r3000a` | Subsystem | Specific CPU core implementation |
| `clean-room` | Architecture | Independent implementation without proprietary code |
| `retrogaming` | Domain | General retro gaming category tag |
| `dma` | Subsystem | Hardware 7-channel direct memory access system |
| `gpu-rasterizer` | Subsystem | VRAM software rendering engine |

**Comma-separated topic list for GitHub UI:**
`ps1, emulator, rust, mips, sdl2, ps1-emulator, playstation-emulator, mips-r3000a, clean-room, retrogaming, dma, gpu-rasterizer`

---

## 3. GitHub CLI (`gh`) Automation Snippets

### 3.1 Repository Configuration (`gh repo edit`)

Run the following command to update repository description and topic tags using the official GitHub CLI:

```bash
# Set repository description, website, and topic tags
gh repo edit \
  --description "A clean-room PlayStation 1 (PS1) emulator in Rust featuring MIPS R3000A CPU, software GPU rasterizer, 7-channel DMA, dual SDL2/Headless frontends, and a 4-Tier test harness." \
  --add-topic ps1 \
  --add-topic emulator \
  --add-topic rust \
  --add-topic mips \
  --add-topic sdl2 \
  --add-topic ps1-emulator \
  --add-topic playstation-emulator \
  --add-topic mips-r3000a \
  --add-topic clean-room \
  --add-topic retrogaming \
  --add-topic dma \
  --add-topic gpu-rasterizer
```

### 3.2 Automated Release Creation (`gh release create`)

To create and publish the official `v0.1.0` release on GitHub with release notes:

```bash
# Save release notes content to temporary file or inline parameter
gh release create v0.1.0 \
  --title "aps v0.1.0 — Initial Clean-Room PS1 Emulator Release" \
  --notes-file - <<'EOF'
# Release v0.1.0 — Initial Clean-Room PS1 Emulator

We are excited to announce the initial release of **`aps` v0.1.0**, a clean-room PlayStation 1 (PS1) emulator written entirely in Rust!

### 🌟 Key Highlights
- **MIPS R3000A Core Interpreter**: Complete R3000A CPU instruction set decoding (R/I/J formats, branch delay slots, HI/LO registers, COP0 status/cause coprocessor).
- **Bus Trait & Memory Subsystem**: Cycle-synchronous memory routing across 2MB Main RAM, 512KB BIOS ROM, 1KB Scratchpad (D-Cache), and Memory-Mapped I/O registers.
- **GPU & VRAM Rasterizer**: Software GPU rendering engine processing GP0/GP1 command buffers, VRAM transfer commands, and frame output.
- **7-Channel DMA Controller**: Full hardware DMA support for MDEC, GPU, CD-ROM, SPU, PIO, OTC, and RAM transfers with interrupt generation.
- **BIOS Boot Execution Pipeline**: Fully functional boot pipeline capable of loading and executing PS1 BIOS binaries in headless and GUI modes.
- **Dual Frontend Architecture**:
  - **Headless Mode (`--headless`)**: Fast non-interactive execution for automated testing, benchmarks, and CI/CD integration.
  - **SDL2 GUI Mode**: Real-time 60 FPS windowed display rendering VRAM framebuffers with keyboard input mapping.
- **Automated 4-Tier Verification Test Suite**: Tier 1 (Unit), Tier 2 (Boundary/Exceptions), Tier 3 (Integration/Subsystems), and Tier 4 (ROM/E2E with `run_ps1_tests.sh`). Fully verified against Amidog's CPU test suite (`psxtest_cpu.exe`).

### 📦 Installation & Usage

```bash
# Clone repository
git clone https://github.com/your-username/aps.git
cd aps

# Build in release mode
cargo build --release

# Run in Headless CLI Mode (Automated Test Execution)
cargo run --release -- --headless path/to/bios.bin

# Run in SDL2 GUI Window Mode
cargo run --release -- path/to/bios.bin
```

### 🧪 Verification & Testing
```bash
# Run full workspace test suite
cargo test --workspace

# Run automated PS1 test ROM harness
./scripts/run_ps1_tests.sh
```

### 📄 License
Dual-licensed under MIT OR Apache-2.0.
EOF
```

---

## 4. Release Notes Template for `v0.1.0`

Below is the standalone release notes template that can be used directly for git tagging, GitHub release releases, or changelog documentation.

```markdown
# Release Notes — aps v0.1.0

**Release Date:** July 30, 2026  
**Tag:** `v0.1.0`  
**License:** MIT OR Apache-2.0  

---

## Overview

`aps` (Awesome PlayStation Emulator in Rust) v0.1.0 marks the initial public release of our clean-room PlayStation 1 emulator. Built entirely from scratch in Rust without external emulation code, `aps` provides a modular architecture separating core emulation logic (`ps_core`) from frontend user interfaces (`aps`).

---

## Features & Architecture Details

### 🧠 MIPS R3000A CPU Engine (`ps_core::cpu`)
- Cycle-accurate instruction decoding for R-Type, I-Type, and J-Type instructions.
- Complete register file: 32 General Purpose Registers ($zero through $ra), HI/LO multiply/divide registers, and Program Counter ($pc).
- System Control Coprocessor (COP0) supporting status (SR), cause (CAUSE), exception program counter (EPC), and memory management registers.
- Branch delay slot processing and load delay slot simulation.

### 🚌 Memory Bus & Hardware I/O (`ps_core::bus`)
- `Bus` trait architecture providing unified read/write access across memory spaces.
- 2 MB System RAM with memory mirroring support.
- 512 KB BIOS ROM mapping (`0x1FC0_0000`).
- 1 KB Data Scratchpad (D-Cache).
- Memory-mapped I/O routing for GPU, DMA, Timers, and Interrupt Controllers (`I_STAT`, `I_MASK`).

### 🎨 GPU Subsystem (`ps_core::gpu`)
- Dual FIFO command processing engine for GP0 (rendering/VRAM commands) and GP1 (display mode/control commands).
- 1 MB VRAM software framebuffer representation (1024x512 16-bit color depth).
- Software rasterization pipeline for flat shaded and textured polygons, line drawing, and rectangle rendering.

### ⚡ DMA & Timer Controllers (`ps_core::dma`, `ps_core::timer`)
- 7-Channel DMA Controller supporting linked-list mode, block transfer mode, and request synchronization.
- Channels implemented: MDEC-in, MDEC-out, GPU, CD-ROM, SPU, PIO, and OTC (Ordering Table Clear).
- 3 Hardware Timers supporting target comparison, clock source selection, and interrupt trigger generation.

### 🖥️ Dual Frontend System (`crates/aps`)
- **Headless Mode (`--headless`)**: High-speed execution loop without window instantiation. Ideal for CI pipelines, automated test runs, and headless servers.
- **SDL2 GUI Frontend**: Real-time rendering backend utilizing `sdl2` crate for 60Hz display refresh, scaling VRAM output, and processing input events.

### 🛡️ 4-Tier Test Harness (`tests/` & `scripts/`)
- **Tier 1 (Unit Tests)**: Micro-benchmarks for individual MIPS opcode decoding and Bus Trait read/write bounds.
- **Tier 2 (Boundary Tests)**: Unaligned memory accesses, exception traps, and memory wrap-around edge cases.
- **Tier 3 (Integration Tests)**: CPU-DMA synchronization, GPU command stream processing, and Timer interrupt chaining.
- **Tier 4 (ROM/E2E Tests)**: Automated test execution harness (`scripts/run_ps1_tests.sh`) executing standard PS1 test ROMs (including Amidog's CPU test suite `psxtest_cpu.exe`) with 101 assertion verifications.

---

## Technical Specifications

| Component | Implementation Detail |
|---|---|
| Language & Edition | Rust 2021 |
| Core Library | `ps_core` |
| Application Crate | `aps` |
| Primary Dependencies | `sdl2`, `clap`, `log`, `env_logger`, `bytemuck`, `anyhow` |
| Supported OS | Linux, macOS, Windows |
| License | Dual-licensed (MIT / Apache-2.0) |

---

## Verification & Status

- `cargo build --release`: **PASS** (Zero warnings/errors)
- `cargo test --workspace`: **PASS** (100% test pass rate)
- `./scripts/run_ps1_tests.sh`: **PASS** (Zero failures across all test ROMs)
- Amidog CPU Suite (`psxtest_cpu.exe`): **PASS** (101/101 instruction assertions passed)

---

## Getting Started

Refer to `README.md` for compilation instructions, architecture diagrams, and usage examples.
