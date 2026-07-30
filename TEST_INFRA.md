# Test Infrastructure & Verification Framework (`TEST_INFRA.md`)

## 1. Overview & Testing Philosophy

`aps` is a clean-room PlayStation 1 (MIPS R3000A) emulator built in Rust. To guarantee strict hardware accuracy, zero-regression refactoring, and automated CI compliance, `aps` adopts a **4-Tier Verification Testing Framework**.

The testing framework covers everything from isolated unit instruction decoding to full cycle-synchronous subsystem interaction and automated test ROM execution.

---

## 2. Feature Inventory & Test Mapping

| Feature # | Feature Name | Test Tier | Target File / Harness | Verification Scope |
|---|---|---|---|---|
| 1 | Cargo Workspace Setup | Tier 1 | `tests/tier1_unit_tests.rs` | Workspace crate linking (`ps_core` & `aps`) |
| 2 | MIPS R3000A CPU Core | Tier 1 | `tests/tier1_unit_tests.rs` | ALU, Branch, Jump, Load/Store, HI/LO, Delay Slots |
| 3 | COP0 Coprocessor | Tier 1 & 2 | `tests/tier1_unit_tests.rs` | Status, Cause, EPC, BadVAddr, Exception Vectors |
| 4 | Memory Bus & Bus Trait | Tier 1 | `tests/tier1_unit_tests.rs` | `MockBus` & Memory Map byte/half/word routing |
| 5 | Address Segment Masking | Tier 2 | `tests/tier2_boundary_tests.rs` | KUSEG/KSEG0/KSEG1 physical masking (`0x1FFFFFFF`) |
| 6 | Tier 1 Unit Tests | Tier 1 | `tests/tier1_unit_tests.rs` | Individual MIPS opcode unit assertions |
| 7 | Interrupt Controller (INTC)| Tier 3 | `tests/tier3_integration_tests.rs` | I_STAT / I_MASK interrupt line assertions |
| 8 | Hardware Timers | Tier 3 | `tests/tier3_integration_tests.rs` | Timer 0/1/2 mode, target, counter logic |
| 9 | 7-Channel DMA Controller | Tier 3 | `tests/tier3_integration_tests.rs` | OTC (Ch 6) linked-list & RAM block transfers |
| 10 | GPU & Software Rasterizer | Tier 3 | `tests/tier3_integration_tests.rs` | GP0 commands, GP1 stat, 16-bit VRAM rendering |
| 11 | BIOS Boot Sequence | Tier 3 & 4 | `tests/tier3_integration_tests.rs` | BIOS vector `0xBFC0_0000` execution & RAM clear |
| 12 | Tier 2 Boundary Tests | Tier 2 | `tests/tier2_boundary_tests.rs` | Unaligned memory access exceptions & RAM mirroring |
| 13 | Dual Frontend CLI Parsing | Tier 3 | `tests/tier3_integration_tests.rs` | `clap` args (`--headless`, `--max-cycles`, etc.) |
| 14 | Headless Execution Mode | Tier 3 & 4 | `tests/tier4_rom_tests.rs` | Headless runner TTY stdout capture (`B0(0x3D)`) |
| 15 | SDL2 GUI Mode Frontend | Tier 3 | Manual / Integration | 60 FPS frame timing & ARGB texture update |
| 16 | Keyboard & Input Mapping | Tier 3 | `tests/tier3_integration_tests.rs` | Controller active-low bitfield at `0x1F80_1040` |
| 17 | Tier 3 Integration Tests | Tier 3 | `tests/tier3_integration_tests.rs` | CPU-DMA-GPU-Timer interrupt chaining |
| 18 | Automated Test ROM Harness | Tier 4 | `scripts/run_ps1_tests.sh` | Shell runner script for automated execution |
| 19 | Amidog CPU Test Suite | Tier 4 | `scripts/run_ps1_tests.sh` | Passing all 101 assertions in `psxtest_cpu.exe` |
| 20 | E2E BIOS Boot & ROM Test | Tier 4 | `tests/tier4_rom_tests.rs` | End-to-end headless execution without crashing |

---

## 3. MockBus Infrastructure (`crates/ps_core/src/bus/mock_bus.rs`)

`MockBus` isolates CPU unit tests from hardware subsystems (GPU/DMA/Timers).
- **RAM**: 2MB (`Box<[u8; 0x200000]>`)
- **BIOS**: 512KB (`Box<[u8; 0x80000]>`)
- **Cycle Accounting**: `cycles: u64`
- **Write Tracking**: `write_log: Vec<(u32, u32, u8)>`
- **Helper**: `load_code(addr: u32, code: &[u8])`

---

## 4. Test Runner Commands

### 4.1 Standard Rust Test Suite
```bash
# Run all Tier 1, Tier 2, Tier 3, and Tier 4 Rust tests (27 total tests across Tier 1-4 suites, 100% passing)
LIBRARY_PATH=/opt/homebrew/lib cargo test

# Run specific tier test files
LIBRARY_PATH=/opt/homebrew/lib cargo test --test tier1_unit_tests       # 13 tests
LIBRARY_PATH=/opt/homebrew/lib cargo test --test tier2_boundary_tests   # 8 tests
LIBRARY_PATH=/opt/homebrew/lib cargo test --test tier3_integration_tests# 5 tests
LIBRARY_PATH=/opt/homebrew/lib cargo test --test tier4_rom_tests        # 1 test
```

### 4.2 Automated Test ROM Harness
```bash
# Execute full E2E Amidog CPU Test ROM suite
./scripts/run_ps1_tests.sh

# Download test ROM assets only without running emulator
./scripts/run_ps1_tests.sh --download-only

# Force re-download of test ROM assets
./scripts/run_ps1_tests.sh --force-download --download-only

# Run test ROM suite with custom timeout (e.g. 30 seconds)
./scripts/run_ps1_tests.sh --timeout 30

# Bypass build step when binary is pre-built
./scripts/run_ps1_tests.sh --no-build
```

---

## 5. Test ROM Execution Harness Architecture

The Tier 4 test harness automates test ROM verification (`psxtest_cpu.exe`):
1. **ROM Fetching**: Script checks `tests/roms/psxtest_cpu.exe` cache; if missing, downloads `psxtest_cpu.zip` from Amidog's canonical repository (`https://psx.amidog.se/lib/exe/fetch.php?media=psx:download:psxtest_cpu.zip`) and extracts `psxtest_cpu.exe`.
2. **Headless Execution**: `aps --headless tests/roms/psxtest_cpu.exe` boots the executable.
3. **Serial TTY Interception**: `ps_core` captures BIOS `B0(0x3D)` `putchar` syscalls and UART `0x1F80_1050` writes, routing ASCII output to stdout.
4. **Assertion Verification**: Parses output log for `All tests done: 00000000`, `All tests passed`, `101/101`, or 101 `PASS` occurrences while filtering out false positives on `FAILED: 0`.
5. **Exit Code Semantics**:
   - `0`: Pass (All tests passed without errors)
   - `1`: Fail (Assertion or CPU error detected)
   - `2`: Build Error (`cargo build` failure or missing binary)
   - `3`: Missing ROM (Download failed and no local ROM cache)
   - `124`: Timeout (Execution exceeded configured time limit)

6. **macOS/BSD Compatibility**:
   - Temporary log creation strictly uses BSD-compliant template strings ending in `XXXXXX` (`LOG_FILE=$(mktemp "${TMPDIR:-/tmp}/psx_amidog_test_XXXXXX")`).
