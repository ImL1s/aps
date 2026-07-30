# CLAUDE.md — Project Guidelines & Developer Workflow

This document provides essential instructions, architectural principles, commands, and code style rules for developers and AI assistants working on the `aps` codebase.

---

## Project Overview

`aps` is a clean-room PlayStation 1 (PS1) emulator implemented in Rust (2021 edition). The repository is structured as a Single Package Manager (SPM) Cargo workspace containing two core crates:
1. `crates/ps_core`: Library crate implementing the hardware emulation core (CPU, Bus, RAM, BIOS, GPU, DMA, Timers, INTC, Controller).
2. `crates/aps`: Binary crate implementing CLI argument parsing, Headless execution mode, and the SDL2 GUI window frontend.

---

## Command Reference

### Build Commands
```bash
# Build debug binary (workspace)
cargo build

# Build release binary (workspace)
cargo build --release

# Check workspace for compilation errors without building artifacts
cargo check --workspace
```

### Testing Commands
```bash
# Run all workspace unit, boundary, integration, and E2E tests
cargo test --workspace

# Run specific 4-Tier test targets
cargo test --test tier1_unit_tests
cargo test --test tier2_boundary_tests
cargo test --test tier3_integration_tests
cargo test --test tier4_rom_tests

# Run challenger stress & edge-case test targets
cargo test --test challenger_stress_tests
cargo test --test challenger_unaligned_cop0_tests
cargo test --test m2_challenger_dma_tests
cargo test --test m2_challenger_gpu_tests
cargo test --test m3_challenger_system_tests

# Execute automated headless test ROM harness (Amidog CPU verification)
./scripts/run_ps1_tests.sh
```

### Code Formatting & Linting
```bash
# Check code formatting compliance across workspace
cargo fmt --check

# Format code automatically
cargo fmt

# Run Clippy lints with zero-warning enforcement (-D warnings)
cargo clippy --workspace --all-targets -- -D warnings
```

### Running the Emulator
```bash
# Run headless mode on a PS1 executable
cargo run --release -- --headless tests/roms/psxtest_cpu.exe

# Run interactive SDL2 GUI window mode
cargo run --release -- tests/roms/psxtest_cpu.exe

# Run with custom max cycle count and TTY log export
cargo run --release -- --headless --max-cycles 10000000 --tty-log tty.log tests/roms/psxtest_cpu.exe
```

---

## Key Architecture Principles

1. **Clean-Room Implementation**:
   - Built strictly from hardware documentation and specifications (e.g. MIPS R3000A spec, PSX hardware specs).
   - No external emulator codebase logic or GPL code snippets copied.

2. **Zero `unsafe` in Core Logic**:
   - `crates/ps_core` is written in **100% safe Rust**.
   - No `unsafe` blocks permitted in CPU instruction decoding, memory mapping, GPU software rasterizer, DMA controller, timers, or INTC.

3. **Single Package Manager (SPM) Workspace Layout**:
   - Hardware logic is strictly encapsulated inside `ps_core`.
   - `aps` binary consumes `ps_core` as a dependency and manages OS windowing, event polling, CLI parsing, and file I/O.

4. **Trait-Based Decoupling**:
   - The CPU engine interacts with memory via the `Bus` trait (`read8`, `read16`, `read32`, `write8`, `write16`, `write32`, `step`, `log_tty_char`).
   - Enables unit testing CPU instructions in complete isolation using `MockBus` without instantiating the full hardware bus.

5. **Physical Address Segment Masking**:
   - Virtual addresses in `KUSEG` (`0x0000_0000`), `KSEG0` (`0x8000_0000`), and `KSEG1` (`0xA000_0000`) are masked using `vaddr & 0x1FFF_FFFF` to map directly to physical memory.

---

## Code Style & Formatting Rules

1. **Strict Rust Idioms**:
   - Follow standard `rustfmt` formatting.
   - Use `#![deny(missing_docs)]` or doc comments (`///`) on public traits and structs where appropriate.
   - Maintain a zero-warnings policy (`-D warnings` in CI).

2. **Explicit Bit Manipulation**:
   - Use explicit primitive types (`u32`, `u16`, `u8`, `usize`, `i32`).
   - Use named constants for bit masks and register addresses (e.g., `RAM_MASK = 0x001F_FFFF`, `BIOS_BASE = 0x1FC0_0000`, `IRQ_VBLANK = 0`).

3. **Error Handling**:
   - In `ps_core`: Use standard `Result<T, String>` or custom enums without panicking. Avoid `unwrap()` / `expect()` in production step loops.
   - In `aps`: Use `anyhow::Result` for CLI commands and SDL2 window errors.

4. **Immutability & Minimal Scope**:
   - Keep variables immutable (`let`) by default unless mutation (`mut`) is required.
   - Limit state mutations to dedicated `step()` and `write()` methods.

5. **Testing Conventions**:
   - Co-locate module unit tests in `mod tests` blocks at the bottom of source files.
   - Place end-to-end and cross-subsystem integration tests in `tests/` matching the 4-Tier test taxonomy.
