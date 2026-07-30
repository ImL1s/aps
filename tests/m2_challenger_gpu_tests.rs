//! M2 Challenger Stress Tests for PS1 GPU Subsystem
//!
//! Tests cover:
//! 1. GP0 command state machine resilience against arbitrary payload words & fuzzed streams.
//! 2. CPU-to-VRAM loads (0xA0): odd/even widths, zero dimensions, VRAM coordinate wrapping.
//! 3. Rectangular drawing primitives (0x60..0x7F & 0x02 Fill Rect): size codes, negative coords.
//! 4. Software rasterizer scissor box clipping & drawing offsets (0xE3, 0xE4, 0xE5).
//! 5. Polygon rasterization: flat/Gouraud triangles & quads, degenerate zero-area cases.

use ps_core::gpu::gp0::Gp0State;
use ps_core::gpu::vram::rgb888_to_bgr555;
use ps_core::gpu::Gpu;

/// Simple deterministic LCG PRNG for reproducible fuzzed payload streams.
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 32) as u32
    }
}

// ============================================================================
// Category 1: GP0 Command State Machine Resilience & Arbitrary Payloads
// ============================================================================

#[test]
fn test_gp0_arbitrary_random_word_stream_fuzzing() {
    let mut gpu = Gpu::new();
    let mut prng = Lcg::new(0xDEADBEEF_12345678);

    // Stream 25,000 pseudo-random 32-bit words into write_gp0
    for _ in 0..25_000 {
        let word = prng.next_u32();
        gpu.write_gp0(word);
    }

    // State machine must remain queryable and in a valid Rust state (no panics/crashes)
    let stat = gpu.read_gpu_stat();
    assert_ne!(stat, 0, "GPUSTAT must remain non-zero after fuzzing");

    // Clear GPU via GP1 reset (0x00) to verify recovery back to Command state
    gpu.write_gp1(0x0000_0000);
    assert_eq!(
        gpu.gp0.state,
        Gp0State::Command,
        "GP1(0x00) reset failed to restore GP0 to Command state"
    );
    assert!(
        gpu.gp0.cmd_buf.is_empty(),
        "GP1(0x00) reset failed to clear GP0 cmd_buf"
    );
}

#[test]
fn test_gp0_unknown_and_reserved_opcodes() {
    let mut gpu = Gpu::new();

    // Unknown/reserved opcodes: 0x03, 0x1F, 0x40, 0x5F, 0x80, 0x9F, 0xB0, 0xD0, 0xE7, 0xFF
    let unknown_opcodes = [
        0x03, 0x1F, 0x40, 0x5F, 0x80, 0x9F, 0xB0, 0xD0, 0xE7, 0xEF, 0xF0, 0xFF,
    ];

    for &op in &unknown_opcodes {
        let cmd = (op as u32) << 24 | 0x00123456;
        gpu.write_gp0(cmd);

        // Unknown opcodes should be consumed in 1 word and keep GPU in Command state
        assert_eq!(
            gpu.gp0.state,
            Gp0State::Command,
            "Opcode {op:#04X} failed to reset to Command state"
        );
        assert!(
            gpu.gp0.cmd_buf.is_empty(),
            "Opcode {op:#04X} left uncleared cmd_buf"
        );
    }
}

#[test]
fn test_gp0_partial_command_transfer_and_gp1_reset() {
    let mut gpu = Gpu::new();

    // 1. Send 2 words of a 4-word Flat Triangle (0x20)
    gpu.write_gp0(0x2000_00FF);
    gpu.write_gp0(0x000A_000A);
    assert_eq!(gpu.gp0.cmd_buf.len(), 2);
    assert_eq!(gpu.gp0.state, Gp0State::Command);

    // Issue GP1 Reset (0x00) - must clear GP0 command buffer
    gpu.write_gp1(0x0000_0000);
    assert_eq!(
        gpu.gp0.cmd_buf.len(),
        0,
        "GP1 Reset (0x00) must clear GP0 cmd_buf"
    );
    assert_eq!(gpu.gp0.state, Gp0State::Command);

    // 2. Send 2 words of a 3-word CPU-to-VRAM transfer (0xA0)
    gpu.write_gp0(0xA000_0000);
    gpu.write_gp0(0x0010_0010); // Dst (16, 16)
    assert_eq!(gpu.gp0.cmd_buf.len(), 2);

    // Issue GP1 Reset Command Buffer (0x01) - must clear GP0 command buffer & state
    gpu.write_gp1(0x0100_0000);
    assert_eq!(
        gpu.gp0.cmd_buf.len(),
        0,
        "GP1 Reset FIFO (0x01) must clear GP0 cmd_buf"
    );
    assert_eq!(
        gpu.gp0.state,
        Gp0State::Command,
        "GP1 Reset FIFO (0x01) must restore GP0 Command state"
    );

    // 3. Enter active CpuToVram state (all 3 header words sent) and reset via GP1(0x00) mid-transfer
    gpu.write_gp0(0xA000_0000);
    gpu.write_gp0(0x0010_0010); // Dst (16, 16)
    gpu.write_gp0(0x0005_0005); // W=5, H=5 -> 13 words expected
    match gpu.gp0.state {
        Gp0State::CpuToVram {
            words_remaining, ..
        } => assert_eq!(words_remaining, 13),
        _ => panic!("Expected CpuToVram state"),
    }
    // Write 1 pixel data word
    gpu.write_gp0(0x1111_1111);
    // Interrupt mid-transfer with GP1 Reset (0x00)
    gpu.write_gp1(0x0000_0000);
    assert_eq!(
        gpu.gp0.state,
        Gp0State::Command,
        "GP1 Reset (0x00) must reset active CpuToVram state back to Command state"
    );

    // Verify subsequent command (Fill Rect 0x02) executes cleanly without corruption
    let fill_color = ps_core::gpu::vram::rgb888_to_bgr555(100, 200, 50);
    gpu.write_gp0(0x0232_C864);
    gpu.write_gp0((100 << 16) | 100);
    gpu.write_gp0((10 << 16) | 10);
    assert_eq!(gpu.vram.get_pixel(100, 100), fill_color);

    // 4. Enter active CpuToVram state and reset via GP1(0x01) mid-transfer
    gpu.write_gp0(0xA000_0000);
    gpu.write_gp0(0x0020_0020); // Dst (32, 32)
    gpu.write_gp0(0x0004_0004); // W=4, H=4 -> 8 words expected
    match gpu.gp0.state {
        Gp0State::CpuToVram {
            words_remaining, ..
        } => assert_eq!(words_remaining, 8),
        _ => panic!("Expected CpuToVram state"),
    }
    // Write 1 pixel data word
    gpu.write_gp0(0x2222_2222);
    // Interrupt mid-transfer with GP1 Reset FIFO (0x01)
    gpu.write_gp1(0x0100_0000);
    assert_eq!(
        gpu.gp0.state,
        Gp0State::Command,
        "GP1 Reset FIFO (0x01) must reset active CpuToVram state back to Command state"
    );

    // Verify subsequent command (Fill Rect 0x02) executes cleanly without corruption
    let fill_color2 = ps_core::gpu::vram::rgb888_to_bgr555(50, 100, 200);
    gpu.write_gp0(0x02C8_6432);
    gpu.write_gp0((200 << 16) | 200);
    gpu.write_gp0((10 << 16) | 10);
    assert_eq!(gpu.vram.get_pixel(200, 200), fill_color2);
}

// ============================================================================
// Category 2: CPU-to-VRAM Load Stress & Alignment / Size Boundaries (0xA0)
// ============================================================================

#[test]
fn test_cpu_to_vram_odd_and_even_widths() {
    let mut gpu = Gpu::new();

    // --- Case A: Odd Width = 3, Height = 2 (6 total pixels = 3 words) ---
    // Command: 0xA0
    gpu.write_gp0(0xA000_0000);
    // Dst: X=10, Y=20
    gpu.write_gp0((20 << 16) | 10);
    // Width=3, Height=2
    gpu.write_gp0((2 << 16) | 3);

    // Assert transitioned to CpuToVram state expecting 3 words
    match gpu.gp0.state {
        Gp0State::CpuToVram {
            words_remaining, ..
        } => assert_eq!(words_remaining, 3),
        _ => panic!("Expected CpuToVram state"),
    }

    // Word 1: Low = 0x0001, High = 0x0002 -> (10, 20)=0x0001, (11, 20)=0x0002
    gpu.write_gp0(0x0002_0001);
    // Word 2: Low = 0x0003, High = 0x0004 -> (12, 20)=0x0003, (10, 21)=0x0004
    gpu.write_gp0(0x0004_0003);
    // Word 3: Low = 0x0005, High = 0x0006 -> (11, 21)=0x0005, (12, 21)=0x0006
    gpu.write_gp0(0x0006_0005);

    // Verify GPU returned to Command state
    assert_eq!(gpu.gp0.state, Gp0State::Command);

    // Verify row 0 (Y=20)
    assert_eq!(gpu.vram.get_pixel(10, 20), 0x0001);
    assert_eq!(gpu.vram.get_pixel(11, 20), 0x0002);
    assert_eq!(gpu.vram.get_pixel(12, 20), 0x0003);

    // Verify row 1 (Y=21)
    assert_eq!(gpu.vram.get_pixel(10, 21), 0x0004);
    assert_eq!(gpu.vram.get_pixel(11, 21), 0x0005);
    assert_eq!(gpu.vram.get_pixel(12, 21), 0x0006);

    // --- Case B: Single pixel transfer (Width=1, Height=1 -> 1 word) ---
    gpu.write_gp0(0xA000_0000);
    gpu.write_gp0((50 << 16) | 50);
    gpu.write_gp0((1 << 16) | 1);
    gpu.write_gp0(0xDEAD_BEEF & 0xFFFF); // Low 16 bits = 0xBEEF
    assert_eq!(gpu.gp0.state, Gp0State::Command);
    assert_eq!(gpu.vram.get_pixel(50, 50), 0xBEEF);
}

#[test]
fn test_cpu_to_vram_zero_width_and_zero_height() {
    let mut gpu = Gpu::new();

    // Width = 0, Height = 10 -> total pixels = 0
    gpu.write_gp0(0xA000_0000);
    gpu.write_gp0((10 << 16) | 10);
    gpu.write_gp0(10 << 16);

    // Should NOT enter CpuToVram state, must remain in Command state
    assert_eq!(gpu.gp0.state, Gp0State::Command);

    // Subsequent write should be treated as next command (e.g. NOP), not payload word
    gpu.write_gp0(0x0000_0000);
    assert_eq!(gpu.gp0.state, Gp0State::Command);
}

#[test]
fn test_cpu_to_vram_wrapping_and_vram_boundary_overflow() {
    let mut gpu = Gpu::new();

    // Dst: X=1023 (right edge), Y=511 (bottom edge). Width=2, Height=2.
    gpu.write_gp0(0xA000_0000);
    gpu.write_gp0((511 << 16) | 1023);
    gpu.write_gp0((2 << 16) | 2);

    // Word 1: Low=0x1111 (at 1023, 511), High=0x2222 (wraps X to 0, 511)
    gpu.write_gp0(0x2222_1111);
    // Word 2: Low=0x3333 (at 1023, wraps Y to 0), High=0x4444 (wraps X to 0, Y to 0)
    gpu.write_gp0(0x4444_3333);

    assert_eq!(gpu.gp0.state, Gp0State::Command);
    assert_eq!(gpu.vram.get_pixel(1023, 511), 0x1111);
    assert_eq!(gpu.vram.get_pixel(0, 511), 0x2222);
    assert_eq!(gpu.vram.get_pixel(1023, 0), 0x3333);
    assert_eq!(gpu.vram.get_pixel(0, 0), 0x4444);
}

#[test]
fn test_cpu_to_vram_large_transfer() {
    let mut gpu = Gpu::new();

    // 100x100 VRAM block transfer (10,000 pixels = 5,000 words)
    let width = 100u32;
    let height = 100u32;
    gpu.write_gp0(0xA000_0000);
    gpu.write_gp0(0);
    gpu.write_gp0((height << 16) | width);

    for i in 0..5000 {
        let val = (i as u32) & 0xFFFF;
        let word = (val << 16) | val;
        gpu.write_gp0(word);
    }

    assert_eq!(gpu.gp0.state, Gp0State::Command);
    // Verify top-left and bottom-right of block
    assert_eq!(gpu.vram.get_pixel(0, 0), 0);
    assert_eq!(gpu.vram.get_pixel(99, 99), 4999);
}

// ============================================================================
// Category 3: Rectangular Drawing Primitives (0x60..0x7F & 0x02 Fill Rect)
// ============================================================================

#[test]
fn test_rectangular_primitives_all_size_codes() {
    let mut gpu = Gpu::new();
    let red_bgr = rgb888_to_bgr555(255, 0, 0);

    // --- Size Code 0: Variable Size (0x60) ---
    // Word 0: 0x600000FF (Red)
    gpu.write_gp0(0x6000_00FF);
    // Word 1: (X=10, Y=10)
    gpu.write_gp0((10 << 16) | 10);
    // Word 2: (W=5, H=4)
    gpu.write_gp0((4 << 16) | 5);

    assert_eq!(gpu.vram.get_pixel(10, 10), red_bgr);
    assert_eq!(gpu.vram.get_pixel(14, 13), red_bgr);
    assert_eq!(gpu.vram.get_pixel(15, 10), 0); // Out of bounds of rect

    // --- Size Code 1: 1x1 Rect (0x68) ---
    let green_bgr = rgb888_to_bgr555(0, 255, 0);
    gpu.write_gp0(0x6800_FF00);
    gpu.write_gp0((20 << 16) | 20); // (20, 20)
    assert_eq!(gpu.vram.get_pixel(20, 20), green_bgr);
    assert_eq!(gpu.vram.get_pixel(21, 20), 0);

    // --- Size Code 2: 8x8 Rect (0x70) ---
    let blue_bgr = rgb888_to_bgr555(0, 0, 255);
    gpu.write_gp0(0x70FF_0000);
    gpu.write_gp0((30 << 16) | 30);
    assert_eq!(gpu.vram.get_pixel(30, 30), blue_bgr);
    assert_eq!(gpu.vram.get_pixel(37, 37), blue_bgr);
    assert_eq!(gpu.vram.get_pixel(38, 30), 0);

    // --- Size Code 3: 16x16 Rect (0x78) ---
    let white_bgr = rgb888_to_bgr555(255, 255, 255);
    gpu.write_gp0(0x78FF_FFFF);
    gpu.write_gp0((40 << 16) | 40);
    assert_eq!(gpu.vram.get_pixel(40, 40), white_bgr);
    assert_eq!(gpu.vram.get_pixel(55, 55), white_bgr);
    assert_eq!(gpu.vram.get_pixel(56, 40), 0);
}

#[test]
fn test_rectangular_primitives_negative_and_extreme_coordinates() {
    let mut gpu = Gpu::new();
    let col = rgb888_to_bgr555(128, 128, 128);

    // Rect starting at X = -5, Y = -5, W = 10, H = 10 (Variable size 0x60)
    // Sign-extended i16 -5 is 0xFFFB
    gpu.write_gp0(0x6080_8080);
    gpu.write_gp0((0xFFFB << 16) | 0xFFFB);
    gpu.write_gp0((10 << 16) | 10);

    // Pixels (0..5, 0..5) should be drawn inside VRAM
    assert_eq!(gpu.vram.get_pixel(0, 0), col);
    assert_eq!(gpu.vram.get_pixel(4, 4), col);

    // Extreme negative coordinates (e.g. X = -1000, Y = -1000, W = 10, H = 10)
    gpu.write_gp0(0x6080_8080);
    gpu.write_gp0(((0xFC18u32) << 16) | 0xFC18u32);
    gpu.write_gp0((10 << 16) | 10);

    // Out-of-screen rect should not panic or write into VRAM
    assert_eq!(gpu.gp0.state, Gp0State::Command);
}

#[test]
fn test_gp0_0x02_fill_rectangle_vram() {
    let mut gpu = Gpu::new();
    let fill_color = rgb888_to_bgr555(100, 150, 200);

    // Fill Rect command 0x02:
    // Word 0: 0x02C89664 (R=100, G=150, B=200)
    // Word 1: X=100, Y=150
    // Word 2: W=50, H=30
    gpu.write_gp0(0x02C8_9664);
    gpu.write_gp0((150 << 16) | 100);
    gpu.write_gp0((30 << 16) | 50);

    assert_eq!(gpu.vram.get_pixel(100, 150), fill_color);
    assert_eq!(gpu.vram.get_pixel(149, 179), fill_color);
    assert_eq!(gpu.vram.get_pixel(150, 150), 0);
}

// ============================================================================
// Category 4: Software Rasterizer Scissor Box Clipping & Drawing Offsets
// ============================================================================

#[test]
fn test_scissor_box_bounds_clamping_and_inverted_clip() {
    let mut gpu = Gpu::new();
    let color = rgb888_to_bgr555(255, 255, 0);

    // Set Top-Left Clip (0xE3): X1=50, Y1=50
    gpu.write_gp0(0xE300_0000 | (50 << 10) | 50);
    // Set Bottom-Right Clip (0xE4): X2=100, Y2=100
    gpu.write_gp0(0xE400_0000 | (100 << 10) | 100);

    assert_eq!(gpu.gp0.clip.x1, 50);
    assert_eq!(gpu.gp0.clip.y1, 50);
    assert_eq!(gpu.gp0.clip.x2, 100);
    assert_eq!(gpu.gp0.clip.y2, 100);

    // Draw large rect (0x60) from (0, 0) to (200, 200)
    gpu.write_gp0(0x6000_FFFF);
    gpu.write_gp0(0x0000_0000);
    gpu.write_gp0((200 << 16) | 200);

    // Pixels outside clip box must remain 0
    assert_eq!(gpu.vram.get_pixel(10, 10), 0);
    assert_eq!(gpu.vram.get_pixel(49, 50), 0);
    assert_eq!(gpu.vram.get_pixel(101, 100), 0);

    // Pixels inside clip box (50..=100, 50..=100) must be rendered
    assert_eq!(gpu.vram.get_pixel(50, 50), color);
    assert_eq!(gpu.vram.get_pixel(100, 100), color);

    // --- Inverted Clip Box: X1=200, X2=100 ---
    gpu.write_gp0(0xE300_0000 | (50 << 10) | 200); // X1=200, Y1=50
    gpu.write_gp0(0xE400_0000 | (100 << 10) | 100); // X2=100, Y2=100

    // Try drawing 16x16 rect at X=300, Y=300 (where VRAM pixel is currently 0)
    gpu.write_gp0(0x7800_FFFF);
    gpu.write_gp0((300 << 16) | 300);

    // Nothing should be rendered because clip region is invalid (X1 > X2)
    assert_eq!(gpu.vram.get_pixel(300, 300), 0);
}

#[test]
fn test_drawing_offset_signed_wrapping_and_clipping() {
    let mut gpu = Gpu::new();

    // Set drawing offset (0xE5) with negative X=-20 (0x7EC), positive Y=30
    // 11-bit sign extension: -20 in 11-bit is 0x7EC
    let off_x = ((-20i32) & 0x7FF) as u32;
    let off_y = ((30i32) & 0x7FF) as u32;
    gpu.write_gp0(0xE500_0000 | (off_y << 11) | off_x);

    assert_eq!(gpu.gp0.draw_offset_x, -20);
    assert_eq!(gpu.gp0.draw_offset_y, 30);

    // Draw 16x16 Rect (0x78) at X=30, Y=20 -> Effective pos = (10, 50)
    let color = rgb888_to_bgr555(0, 255, 255);
    gpu.write_gp0(0x78FF_FF00);
    gpu.write_gp0((20 << 16) | 30);

    assert_eq!(gpu.vram.get_pixel(10, 50), color);
    assert_eq!(gpu.vram.get_pixel(25, 65), color);
}

// ============================================================================
// Category 5: Polygon Rasterizer Stress (Flat/Gouraud Triangles/Quads)
// ============================================================================

#[test]
fn test_polygon_rasterizer_degenerate_triangles() {
    let mut gpu = Gpu::new();

    // Degenerate triangle: all 3 vertices at the exact same point (10, 10)
    gpu.write_gp0(0x2000_00FF);
    gpu.write_gp0((10 << 16) | 10);
    gpu.write_gp0((10 << 16) | 10);
    gpu.write_gp0((10 << 16) | 10);

    assert_eq!(gpu.gp0.state, Gp0State::Command);

    // Horizontal line triangle (area = 0): (10, 10), (30, 10), (50, 10)
    gpu.write_gp0(0x2000_00FF);
    gpu.write_gp0((10 << 16) | 10);
    gpu.write_gp0((10 << 16) | 30);
    gpu.write_gp0((10 << 16) | 50);

    assert_eq!(gpu.gp0.state, Gp0State::Command);
}

#[test]
fn test_polygon_gouraud_shading_and_quads() {
    let mut gpu = Gpu::new();

    // Gouraud Quad (0x38 - 8 words):
    // V0: (10, 10) Red
    // V1: (40, 10) Green
    // V2: (10, 40) Blue
    // V3: (40, 40) White
    gpu.write_gp0(0x3800_00FF); // Word 0: Opcode 0x38, C0 = Red
    gpu.write_gp0((10 << 16) | 10); // Word 1: V0
    gpu.write_gp0(0x0000_FF00); // Word 2: C1 = Green
    gpu.write_gp0((10 << 16) | 40); // Word 3: V1
    gpu.write_gp0(0x00FF_0000); // Word 4: C2 = Blue
    gpu.write_gp0((40 << 16) | 10); // Word 5: V2
    gpu.write_gp0(0x00FF_FFFF); // Word 6: C3 = White
    gpu.write_gp0((40 << 16) | 40); // Word 7: V3

    assert_eq!(gpu.gp0.state, Gp0State::Command);

    // Quad interior pixels must be non-zero (rendered by Gouraud rasterizer)
    assert_ne!(gpu.vram.get_pixel(15, 15), 0);
    assert_ne!(gpu.vram.get_pixel(35, 35), 0);
}
