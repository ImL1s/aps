# Technical Architecture Specification — PlayStation 1 Emulator (`aps`)

This document provides a deep technical architectural specification of the `aps` clean-room PlayStation 1 (PS1) emulator, detailing the MIPS R3000A CPU interpreter, Memory Bus & mapping rules, GPU software rasterizer, 7-Channel DMA controller, Hardware Timers, Interrupt Controller (INTC), Dual Frontend system, and 4-Tier verification framework.

---

## Table of Contents
1. [System Overview & Workspaces](#1-system-overview--workspaces)
2. [MIPS R3000A CPU Core & COP0](#2-mips-r3000a-cpu-core--cop0)
3. [Memory System & Bus Trait](#3-memory-system--bus-trait)
4. [GPU Subsystem & Software Rasterizer](#4-gpu-subsystem--software-rasterizer)
5. [7-Channel DMA Controller](#5-7-channel-dma-controller)
6. [Hardware Timers & Interrupt Controller (INTC)](#6-hardware-timers--interrupt-controller-intc)
7. [Dual Frontend Architecture](#7-dual-frontend-architecture)
8. [4-Tier Verification Strategy](#8-4-tier-verification-strategy)

---

## 1. System Overview & Workspaces

The emulator is organized as a Cargo workspace with strict separation between core hardware emulation (`ps_core`) and frontend drivers (`aps`):

```
                       +-----------------------------------+
                       |             aps CLI               |
                       | (clap, Headless, SDL2 Frontend)   |
                       +-----------------------------------+
                                         |
                                         v
                       +-----------------------------------+
                       |             ps_core               |
                       | (System, CPU, Bus, Subsystems)    |
                       +-----------------------------------+
                                         |
            +--------------------+-------+-------+--------------------+
            |                    |               |                    |
            v                    v               v                    v
      +-----------+        +-----------+   +-----------+        +-----------+
      |    CPU    | <----> |    Bus    |   |    GPU    |        |    DMA    |
      +-----------+        +-----------+   +-----------+        +-----------+
                                 |
                         +-------+-------+
                         |               |
                         v               v
                   +-----------+   +-----------+
                   |  Timers   |   |   INTC    |
                   +-----------+   +-----------+
```

- **`ps_core`**: Pure Rust library crate (100% safe Rust). Contains no OS windowing or external GUI dependencies.
- **`aps`**: CLI binary crate handling command-line arguments, file loading, headless execution loops, and SDL2 rendering window loops.

---

## 2. MIPS R3000A CPU Core & COP0

### CPU Overview
The CPU core implements a 32-bit Little-Endian MIPS R3000A RISC processor operating at **33.8688 MHz**.

### Registers
- **32 General Purpose Registers (`gpr: [u32; 32]`)**:
  - `$0` (`$zero`): Hardwired to `0`. Writes are discarded, reads always yield `0`.
  - `$28` (`$gp`): Global Pointer (initialized by PSX EXE headers).
  - `$29` (`$sp`): Stack Pointer (initialized by PSX EXE headers).
  - `$31` (`$ra`): Return Address (set by `JAL`, `JALR`, `BLTZAL`, `BGEZAL`).
- **Multiply/Divide Registers**:
  - `HI`: Holds 32 most-significant bits of multiplication or division remainder (`MFHI`, `MTHI`).
  - `LO`: Holds 32 least-significant bits of multiplication or division quotient (`MFLO`, `MTLO`).
- **Program Counter**:
  - `pc`: Current program counter.
  - `next_pc`: Next program counter (for 1-cycle branch delay slot execution).

### Instruction Decoding & Formats
All instructions are 32-bit words decoded into three primary formats:
- **R-type**: `[ opcode (6) | rs (5) | rt (5) | rd (5) | shamt (5) | funct (6) ]`
- **I-type**: `[ opcode (6) | rs (5) | rt (5) | imm16 (16) ]`
- **J-type**: `[ opcode (6) | target26 (26) ]`

### Pipeline & Delay Slot Mechanics

#### 1. Branch Delay Slot
Branch instructions (`J`, `JAL`, `JR`, `JALR`, `BEQ`, `BNE`, `BLEZ`, `BGTZ`, `BLTZ`, `BGEZ`, `BLTZAL`, `BGEZAL`) set `next_pc` to the branch target and set `next_in_delay_slot = true`. The instruction immediately following the branch (the delay slot instruction at `pc + 4`) is executed before `pc` jumps to the target address.

#### 2. Two-Stage Load Delay Pipeline
Memory loads (`LB`, `LBU`, `LH`, `LHU`, `LW`, `LWL`, `LWR`, `MFC0`) do not update the destination register immediately in the current cycle. Instead, the load value is scheduled into a 2-stage pipeline (`load_delay_pending_reg`, `load_delay_current_reg`, `load_delay_applied_reg`):

```
Cycle N  : Load Instruction scheduled -> pending_reg = rt, pending_val = data
Cycle N+1: Delay slot instruction executes -> current_reg = pending_reg, current_val = pending_val
           If delay slot instruction reads rt, it reads OLD value via get_gpr_branch(rt)
Cycle N+2: Load value applied to gpr[rt]
```

### COP0 System Control Coprocessor
The System Control Coprocessor manages exceptions, interrupt masking, and memory translation status:

```rust
pub struct Cop0 {
    pub badvaddr: u32, // Register 8:  Faulting address on unaligned access
    pub status: u32,   // Register 12: Status Register (SR)
    pub cause: u32,    // Register 13: Cause Register (CR)
    pub epc: u32,      // Register 14: Exception Program Counter
    pub prid: u32,     // Register 15: Processor Revision ID (0x0000_0002)
}
```

#### Status Register (Reg 12 - `SR`)
- Bit 22 (`BEV`): Boot Exception Vector flag. When `1`, exceptions route to BIOS vector `0xBFC0_0180`. When `0`, exceptions route to RAM vector `0x8000_0080`.
- Bits 5..0 (`KUo`, `IEo`, `KUp`, `IEp`, `KUc`, `IEc`): 3-level stack storing Kernel/User mode and Interrupt Enable status.

#### Cause Register (Reg 13 - `CR`)
- Bit 31 (`BD`): Set to `1` if exception occurred inside a branch delay slot.
- Bits 6..2 (`ExcCode`): Exception code.

#### Exception Types
| Exception | Code | Description | Vector |
|-----------|------|-------------|--------|
| `Interrupt` | `0x00` | Hardware interrupt request | BEV ? `0xBFC0_0180` : `0x8000_0080` |
| `AddressErrorLoad` | `0x04` | Unaligned 16-bit or 32-bit load or PC fetch | BEV ? `0xBFC0_0180` : `0x8000_0080` |
| `AddressErrorStore` | `0x05` | Unaligned 16-bit or 32-bit store | BEV ? `0xBFC0_0180` : `0x8000_0080` |
| `Syscall` | `0x08` | `SYSCALL` instruction execution | BEV ? `0xBFC0_0180` : `0x8000_0080` |
| `Break` | `0x09` | `BREAK` instruction execution | BEV ? `0xBFC0_0180` : `0x8000_0080` |
| `ReservedInstruction` | `0x0A` | Unrecognized or illegal opcode | BEV ? `0xBFC0_0180` : `0x8000_0080` |
| `CoprocessorUnusable` | `0x0B` | Accessing disabled coprocessor | BEV ? `0xBFC0_0180` : `0x8000_0080` |
| `Overflow` | `0x0C` | Signed arithmetic overflow (`ADD`, `SUB`, `ADDI`) | BEV ? `0xBFC0_0180` : `0x8000_0080` |

#### `RFE` (Restore From Exception)
Pops the 3-level interrupt/mode stack in `Status`: shifts bits 5..2 right by 2 to bits 3..0.

### BIOS Vector Interception
When `pc` lands on BIOS C-library vector entrypoints (`0x0000_00A0`, `0x0000_00B0`, `0x0000_00C0`), the CPU checks function code `$v0` (or `$t1`):
- `0x3C` / `0x3D` (`putchar`): Emits character `$a0` to TTY output log buffer.
- `0x3E` (`puts`): Reads null-terminated string at `$a0` and emits to TTY log buffer.
- `0x3F` (`printf`): Parses format string at `$a0` with arguments `$a1..$a3` and stack pointer `$sp`, outputting formatted text to TTY log buffer.

---

## 3. Memory System & Bus Trait

### Physical Address Segment Masking
The CPU generates 32-bit virtual addresses mapped to a 512MB physical space:

```rust
#[inline(always)]
pub fn mask_address(vaddr: u32) -> u32 {
    vaddr & 0x1FFF_FFFF
}
```

- **KUSEG** (`0x0000_0000..0x7FFF_FFFF`): User space (2GB virtual -> 512MB physical).
- **KSEG0** (`0x8000_0000..0x9FFF_FFFF`): Kernel cached space (512MB virtual -> 512MB physical).
- **KSEG1** (`0xA000_0000..0xBFFF_FFFF`): Kernel uncached space (512MB virtual -> 512MB physical).

### Memory Map Layout

| Physical Address Range | Size | Description | Mirroring / Masking |
|-----------------------|------|-------------|---------------------|
| `0x0000_0000..0x001F_FFFF` | 2MB | Main RAM | Primary 2MB RAM |
| `0x0020_0000..0x007F_FFFF` | 6MB | Main RAM Mirror | Mirrored every 2MB (`addr & 0x001F_FFFF`) |
| `0x1F80_0000..0x1F80_03FF` | 1KB | Scratchpad Fast D-Cache | Mapped to CPU fast internal RAM |
| `0x1F80_1000..0x1F80_1020` | 32B | Memory Control Registers | Expansion / RAM Configuration |
| `0x1F80_1040..0x1F80_1050` | 16B | Controller & Memory Card I/O | Digital pad active-low buttons |
| `0x1F80_1070..0x1F80_1078` | 8B | INTC Registers | `I_STAT` (0x1070), `I_MASK` (0x1074) |
| `0x1F80_1080..0x1F80_10F8` | 120B | DMA Controller Registers | Channels 0-6, `DPCR`, `DICR` |
| `0x1F80_1100..0x1F80_1130` | 48B | Hardware Timers Registers | Timer 0, 1, 2 (Value, Mode, Target) |
| `0x1F80_1810..0x1F80_1814` | 8B | GPU Registers | `GP0` Data (0x1810), `GP1` Stat (0x1814) |
| `0x1FC0_0000..0x1FC7_FFFF` | 512KB | BIOS ROM | System Bootloader (SCPH1001.BIN) |

### Bus Trait Interface
```rust
pub trait Bus {
    fn read8(&mut self, addr: u32) -> u8;
    fn read16(&mut self, addr: u32) -> u16;
    fn read32(&mut self, addr: u32) -> u32;
    fn write8(&mut self, addr: u32, val: u8);
    fn write16(&mut self, addr: u32, val: u16);
    fn write32(&mut self, addr: u32, val: u32);
    fn step(&mut self, cycles: u32);
    fn log_tty_char(&mut self, ch: u8);
}
```

- **`MemoryBus`**: Concrete implementation routing reads and writes across RAM, BIOS, Scratchpad, GPU, DMA, Timers, INTC, and Controllers.
- **`MockBus`**: In-memory test bus used in Tier 1 CPU instruction unit tests.

---

## 4. GPU Subsystem & Software Rasterizer

### VRAM Architecture
VRAM is a 1MB linear buffer structured as **1024 x 512 16-bit BGR555 pixels**:
- Format: `[ Bit 15: Mask Bit | Bits 14..10: Blue | Bits 9..5: Green | Bits 4..0: Red ]`

### GP0 Command Processor
Processing 32-bit command words written to `0x1F80_1810`:

```
+-------------------------------------------------------------------------------+
| Opcode (Bits 31..24) | Command Description                                   |
+----------------------+--------------------------------------------------------+
| 0x02                 | Fill Rectangle in VRAM (X, Y, Width, Height, Color)    |
| 0x20                 | Flat Textured/Untextured Monochr. Triangle (3 Vertices)|
| 0x28                 | Flat Textured/Untextured Monochr. Quad (4 Vertices)    |
| 0x30                 | Gouraud Shaded Untextured Triangle (3 Colors, 3 Verts) |
| 0x38                 | Gouraud Shaded Untextured Quad (4 Colors, 4 Verts)     |
| 0x60, 0x68, 0x70     | Rectangular Primitives (Variable, 1x1, 8x8, 16x16)     |
| 0xA0                 | CPU-to-VRAM Image Transfer                            |
| 0xC0                 | VRAM-to-CPU Image Transfer                            |
| 0xE3                 | Set Drawing Area Top-Left (Scissor Left/Top)           |
| 0xE4                 | Set Drawing Area Bottom-Right (Scissor Right/Bottom)   |
| 0xE5                 | Set Drawing Offset (X, Y signed offsets)              |
+-------------------------------------------------------------------------------+
```

### GP1 Control Processor
Processing 32-bit control words written to `0x1F80_1814`:
- `0x00`: Soft Reset GPU.
- `0x01`: Reset Command Buffer FIFO.
- `0x02`: Acknowledge GPU IRQ (`irq_requested = false`).
- `0x03`: Display Enable (0 = Enabled, 1 = Disabled).
- `0x04`: DMA Direction (0 = Off, 1 = FIFO, 2 = CPU-to-VRAM, 3 = VRAM-to-CPU).
- `0x05`: Display VRAM Start Address (`display_vram_x`, `display_vram_y`).
- `0x06` / `0x07`: Horizontal / Vertical Display Range.
- `0x08`: Display Mode (Color depth, PAL/NTSC, Resolution).

### GPUSTAT Register Layout
`GPUSTAT` (read from `0x1F80_1814`) reports hardware status flags:
- Bits 0..4: Texture page X/Y & colors.
- Bit 19: Vertical Interlace.
- Bit 23: Display Disable.
- Bit 24: Interrupt Request Flag.
- Bit 26: Ready to receive Command.
- Bit 27: Ready to send VRAM image to CPU.
- Bit 28: Ready to receive DMA block.

### Software Rasterizer Engine

#### Edge-Function Barycentric Triangle Rasterization
Triangles are rendered using a 2D bounding-box rasterizer evaluating edge-functions:

$$\text{edge}(v_0, v_1, p) = (p.x - v_0.x)(v_1.y - v_0.y) - (p.y - v_0.y)(v_1.x - v_0.x)$$

```rust
let area = edge(v0, v1, v2);
if area == 0 { return; } // Degenerate triangle

for y in min_y..=max_y {
    for x in min_x..=max_x {
        let w0 = edge(v1, v2, p);
        let w1 = edge(v2, v0, p);
        let w2 = edge(v0, v1, p);

        if (w0 >= 0 && w1 >= 0 && w2 >= 0) || (w0 <= 0 && w1 <= 0 && w2 <= 0) {
            // Calculate Gouraud color weights:
            let r = (w0 * c0.r + w1 * c1.r + w2 * c2.r) / area;
            let g = (w0 * c0.g + w1 * c1.g + w2 * c2.g) / area;
            let b = (w0 * c0.b + w1 * c1.b + w2 * c2.b) / area;
            vram.set_pixel(scr_x, scr_y, pack_bgr555(r, g, b));
        }
    }
}
```

---

## 5. 7-Channel DMA Controller

The DMA Controller manages background data movement between RAM and peripherals without CPU intervention:

```
+-------------------------------------------------------------------------------+
| Channel # | Subsystem | Purpose & Usage                                       |
+-----------+-----------+-------------------------------------------------------+
| DMA 0     | MDEC in   | Macroblock Decoder Input Stream                       |
| DMA 1     | MDEC out  | Macroblock Decoder Output Stream                      |
| DMA 2     | GPU       | GPU Command Lists (Linked-List) & VRAM Image Transfers|
| DMA 3     | CDROM     | Sector Buffer Reading to RAM                          |
| DMA 4     | SPU       | Sound Processing Unit Sample Transfers                 |
| DMA 5     | PIO       | Parallel I/O Expansion Port Data                      |
| DMA 6     | OTC       | Ordering Table Clear (Hardware Link Chain Generation) |
+-------------------------------------------------------------------------------+
```

### Channel Registers (Base 0x1F80_1080 + Channel * 0x10)
- `MADR` (`0x0`): Memory Address (24-bit physical address).
- `BCR`  (`0x4`): Block Count & Block Size (`bc | (ba << 16)`).
- `CHCR` (`0x8`): Channel Control Register:
  - Bit 0: Direction (`0` = To RAM, `1` = From RAM).
  - Bit 1: Memory Address Step (`0` = Forward +4, `1` = Backward -4).
  - Bits 9..10: Sync Mode (`0` = Immediate, `1` = Multi-block, `2` = Linked-List).
  - Bit 24: Trigger (`1` = Start transfer).
  - Bit 28: Busy status flag.

### Linked-List Mode (Channel 2 GPU)
Traverses linked-list ordering tables stored in RAM:
1. Fetch 32-bit packet header at `MADR`:
   - `count = header >> 24` (number of GP0 payload words).
   - `next_ptr = header & 0x00FF_FFFF` (pointer to next node).
2. Write `count` words following header into `GP0`.
3. If `next_ptr == 0x00FF_FFFF`, terminate transfer; otherwise set `MADR = next_ptr` and repeat.

### Ordering Table Clear (Channel 6 OTC)
Fills an Ordering Table array backwards in RAM with pointers to preceding elements:
- `RAM[MADR] = MADR - 4`
- Final element is written with end-marker `0x00FF_FFFF`.

---

## 6. Hardware Timers & Interrupt Controller (INTC)

### Hardware Timers
Three 16-bit timers with selectable clock sources and interrupt modes:
- **Timer 0** (`0x1F80_1100`): Pixel Clock (Dot Clock) or SysClock.
- **Timer 1** (`0x1F80_1110`): HBLANK or SysClock.
- **Timer 2** (`0x1F80_1120`): SysClock or SysClock / 8 divider.

```rust
pub struct Timer {
    pub val: u16,        // Current 16-bit Counter Value
    pub mode: u16,       // Control / Mode Register
    pub target: u16,     // Target Match Value
}
```

### Interrupt Controller (INTC)
Manages 11 hardware interrupt sources via `I_STAT` (`0x1F80_1070`) and `I_MASK` (`0x1F80_1074`):

```
+-------------------------------------------------------------------------------+
| IRQ # | Line Name   | Triggering Condition                                    |
+-------+-------------+---------------------------------------------------------+
| IRQ 0 | VBLANK      | GPU scanline enters vertical blanking (line 240)        |
| IRQ 1 | GPU         | GP1 interrupt command or display boundary match         |
| IRQ 2 | CDROM       | CD-ROM sector data ready or command acknowledge         |
| IRQ 3 | DMA         | DMA Channel completion flag set in DICR                 |
| IRQ 4 | TIMER0      | Timer 0 target match or counter overflow                |
| IRQ 5 | TIMER1      | Timer 1 target match or counter overflow                |
| IRQ 6 | TIMER2      | Timer 2 target match or counter overflow                |
| IRQ 7 | CONTROLLER  | Digital Pad / Memory Card serial transfer byte complete |
| IRQ 8 | SIO         | Serial I/O port transfer complete                       |
| IRQ 9 | SPU         | Sound Processing Unit interrupt request                 |
| IRQ 10| PIO         | Parallel Expansion I/O interrupt request                |
+-------------------------------------------------------------------------------+
```

- **`I_STAT` Write-1-to-Clear**: Writing a `1` bit to `I_STAT` clears that interrupt bit (`istat &= !val`).
- **CPU IRQ Assertion**: Asserted when `(I_STAT & I_MASK & 0x7FF) != 0`.

---

## 7. Dual Frontend Architecture

```
                          +------------------------+
                          |   CLI / User Launch    |
                          +------------------------+
                                      |
                     +----------------+----------------+
                     |                                 |
                     v                                 v
         [ --headless Specified ]            [ Standard Launch ]
                     |                                 |
                     v                                 v
         +-----------------------+         +-----------------------+
         |    HeadlessRunner     |         |     Sdl2Frontend      |
         |                       |         |                       |
         | - Batch cycle step    |         | - 60 FPS Frame Sync   |
         | - TTY stdout & log    |         | - ARGB32 Texture Sync |
         | - Auto-halt match     |         | - Keyboard Input Map  |
         | - PPM screenshot dump |         | - SDL2 Window Canvas  |
         +-----------------------+         +-----------------------+
```

### 1. Headless CLI Mode (`HeadlessRunner`)
- Non-interactive cycle loop (`batch_size = 10_000`).
- Intercepts BIOS `0x3D` / `0x3F` stdout output and flushes to `stdout` and optional `--tty-log`.
- Automatically terminates when TTY emits `Done`, `All tests done`, or CPU enters a `pc == next_pc` self-loop.
- Generates 24-bit `.ppm` framebuffer images when `--screenshot` is specified.

### 2. SDL2 GUI Window Mode (`Sdl2Frontend`)
- Fixed frame timing: **564,480 cycles per frame** (33.8688 MHz / 60 FPS).
- Renders 1024x512 VRAM into an ARGB32 streaming SDL2 texture.
- Maps physical key events (`KeyDown`, `KeyUp`) to active-low PS1 controller registers (`0x1F80_1040`).

---

## 8. 4-Tier Verification Strategy

```
+-------------------------------------------------------------------------------+
| Tier   | Level       | Scope & Test Targets                                   |
+--------+-------------+--------------------------------------------------------+
| Tier 1 | Unit        | Isolated MIPS CPU instructions & memory address decoding|
|        |             | via MockBus harness (tier1_unit_tests.rs).             |
| Tier 2 | Boundary    | Exception generation (AdEL, AdES, Ov), memory mirroring|
|        |             | & KSEG segment aliasing (tier2_boundary_tests.rs).      |
| Tier 3 | Integration | CPU-DMA-GPU linked-list pipelines, Timer-INTC IRQ      |
|        |             | chaining, and Controller IO maps (tier3_integration).  |
| Tier 4 | ROM / E2E   | Full BIOS boot sequence & Amidog CPU verification ROM   |
|        |             | (psxtest_cpu.exe) asserting 101/101 test passes.       |
+-------------------------------------------------------------------------------+
```

Automated verification harness `./scripts/run_ps1_tests.sh` executes Tier 4 test ROMs in headless mode, verifying zero CPU instruction failures across the 101 Amidog assertion checks.
