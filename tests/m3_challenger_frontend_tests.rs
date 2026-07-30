//! Empirical Stress Tests for Milestone M3: Dual Frontends & Input Mapping
//!
//! Verifies Headless execution mode, cycle limit precision, TTY log capturing,
//! PPM screenshot formatting (P3 header, 1024x512 dimensions, RGB color bounds),
//! controller active-low bitwise operations across all 16 PadButton variants,
//! and IO register 0x1F80_1040 byte/word/dword active button reads.

use aps::headless::{save_ppm_screenshot, HeadlessRunner};
use ps_core::bus::Bus;
use ps_core::controller::{Controller, PadButton};
use ps_core::system::PS1System;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

fn temp_file_path(prefix: &str, ext: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let file_name = format!(
        "{}_{}_{}.{}",
        prefix,
        std::process::id(),
        rand_suffix(),
        ext
    );
    dir.push(file_name);
    dir
}

fn rand_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(12345)
}

// ============================================================================
// 1. Controller Active-Low Bit Manipulation Across All 16 PadButton Variants
// ============================================================================

#[test]
fn test_controller_all_16_pad_buttons_individual_bit_masks() {
    let buttons = [
        (PadButton::Select, 0, 1u16 << 0),
        (PadButton::L3, 1, 1u16 << 1),
        (PadButton::R3, 2, 1u16 << 2),
        (PadButton::Start, 3, 1u16 << 3),
        (PadButton::Up, 4, 1u16 << 4),
        (PadButton::Right, 5, 1u16 << 5),
        (PadButton::Down, 6, 1u16 << 6),
        (PadButton::Left, 7, 1u16 << 7),
        (PadButton::L2, 8, 1u16 << 8),
        (PadButton::R2, 9, 1u16 << 9),
        (PadButton::L1, 10, 1u16 << 10),
        (PadButton::R1, 11, 1u16 << 11),
        (PadButton::Triangle, 12, 1u16 << 12),
        (PadButton::Circle, 13, 1u16 << 13),
        (PadButton::Cross, 14, 1u16 << 14),
        (PadButton::Square, 15, 1u16 << 15),
    ];

    assert_eq!(buttons.len(), 16, "Must cover all 16 PadButton variants");

    for (button, bit_idx, expected_mask) in buttons {
        // 1. Verify bit_mask() calculation
        let mask = button.bit_mask();
        assert_eq!(
            mask, expected_mask,
            "PadButton {button:?} bit_mask must be 1 << {bit_idx}"
        );

        // 2. Initial state must be 0xFFFF (active low, released)
        let mut ctrl = Controller::new();
        assert_eq!(ctrl.button_state, 0xFFFF);

        // 3. Press button -> bit cleared (active low 0)
        ctrl.set_button(button, true);
        assert_eq!(
            ctrl.button_state & mask,
            0,
            "Bit {bit_idx} for {button:?} must be 0 when pressed"
        );
        assert_eq!(
            ctrl.button_state, !mask,
            "Only bit {bit_idx} for {button:?} should be cleared when pressed"
        );

        // 4. Release button -> bit set back to 1
        ctrl.set_button(button, false);
        assert_eq!(
            ctrl.button_state, 0xFFFF,
            "Button state must return to 0xFFFF after release of {button:?}"
        );
    }
}

#[test]
fn test_controller_multi_button_simultaneous_presses() {
    let mut ctrl = Controller::new();

    // Press D-Pad Up (bit 4) + Right (bit 5) + Action Cross (bit 14) + Triangle (bit 12) + Start (bit 3)
    let combo = [
        PadButton::Up,
        PadButton::Right,
        PadButton::Cross,
        PadButton::Triangle,
        PadButton::Start,
    ];

    let mut combined_mask: u16 = 0;
    for &btn in &combo {
        ctrl.set_button(btn, true);
        combined_mask |= btn.bit_mask();
    }

    let expected_state = !combined_mask;
    assert_eq!(
        ctrl.button_state, expected_state,
        "Multi-button press state 0x{:04X} does not match expected 0x{:04X}",
        ctrl.button_state, expected_state
    );

    // Press ALL 16 buttons simultaneously
    let all_buttons = [
        PadButton::Select,
        PadButton::L3,
        PadButton::R3,
        PadButton::Start,
        PadButton::Up,
        PadButton::Right,
        PadButton::Down,
        PadButton::Left,
        PadButton::L2,
        PadButton::R2,
        PadButton::L1,
        PadButton::R1,
        PadButton::Triangle,
        PadButton::Circle,
        PadButton::Cross,
        PadButton::Square,
    ];

    for &btn in &all_buttons {
        ctrl.set_button(btn, true);
    }
    assert_eq!(
        ctrl.button_state, 0x0000,
        "All 16 buttons pressed simultaneously must yield 0x0000"
    );

    // Release half of the buttons (Select, Start, Up, Down, L1, R1, Triangle, Cross)
    let release_half = [
        PadButton::Select,
        PadButton::Start,
        PadButton::Up,
        PadButton::Down,
        PadButton::L1,
        PadButton::R1,
        PadButton::Triangle,
        PadButton::Cross,
    ];

    for &btn in &release_half {
        ctrl.set_button(btn, false);
    }

    let mut released_mask: u16 = 0;
    for &btn in &release_half {
        released_mask |= btn.bit_mask();
    }
    assert_eq!(
        ctrl.button_state, released_mask,
        "Partial release state 0x{:04X} does not match expected 0x{:04X}",
        ctrl.button_state, released_mask
    );
}

#[test]
fn test_controller_bitwise_or_and_mask_operations() {
    let mut ctrl = Controller::new();

    // Verify bitwise OR mask combinations
    let mask_l1 = PadButton::L1.bit_mask();
    let mask_r1 = PadButton::R1.bit_mask();
    let mask_l2 = PadButton::L2.bit_mask();
    let mask_r2 = PadButton::R2.bit_mask();

    let shoulder_mask = mask_l1 | mask_r1 | mask_l2 | mask_r2;

    // Press all 4 shoulder buttons
    ctrl.set_button(PadButton::L1, true);
    ctrl.set_button(PadButton::R1, true);
    ctrl.set_button(PadButton::L2, true);
    ctrl.set_button(PadButton::R2, true);

    assert_eq!(ctrl.button_state & shoulder_mask, 0);
    assert_eq!(ctrl.button_state, !shoulder_mask);

    // Release L1 and R2 using bitwise mask assertions
    ctrl.set_button(PadButton::L1, false);
    ctrl.set_button(PadButton::R2, false);

    assert_eq!(ctrl.button_state & mask_l1, mask_l1, "L1 must be released");
    assert_eq!(ctrl.button_state & mask_r2, mask_r2, "R2 must be released");
    assert_eq!(ctrl.button_state & mask_r1, 0, "R1 must remain pressed");
    assert_eq!(ctrl.button_state & mask_l2, 0, "L2 must remain pressed");
}

#[test]
fn test_controller_button_toggles_rapid_cycles() {
    let mut ctrl = Controller::new();

    let toggle_buttons = [
        PadButton::Cross,
        PadButton::Square,
        PadButton::Circle,
        PadButton::Triangle,
        PadButton::Up,
        PadButton::Down,
        PadButton::Left,
        PadButton::Right,
    ];

    // Rapid toggle loop 100 iterations of press & release
    for _ in 0..100 {
        for &btn in &toggle_buttons {
            ctrl.set_button(btn, true);
            assert_eq!(
                ctrl.button_state & btn.bit_mask(),
                0,
                "Button {btn:?} should be pressed (active-low bit 0)"
            );
            ctrl.set_button(btn, false);
            assert_eq!(
                ctrl.button_state & btn.bit_mask(),
                btn.bit_mask(),
                "Button {btn:?} should be released (bit 1)"
            );
        }
    }

    assert_eq!(
        ctrl.button_state, 0xFFFF,
        "Final button state after toggles must be 0xFFFF"
    );
}

// ============================================================================
// 2. IO Register 0x1F80_1040 Reads (read8, read16, read32) & Address Aliasing
// ============================================================================

#[test]
fn test_io_register_0x1f80_1040_read8_read16_read32_active_combinations() {
    let mut system = PS1System::new();

    // Default status: 0xFFFF for 16-bit, 0xFF for lower/upper 8-bit, 0x0000_FFFF for 32-bit
    assert_eq!(system.bus.read8(0x1F80_1040), 0xFF);
    assert_eq!(system.bus.read8(0x1F80_1041), 0xFF);
    assert_eq!(system.bus.read16(0x1F80_1040), 0xFFFF);
    assert_eq!(system.bus.read32(0x1F80_1040), 0x0000_FFFF);

    // Test KSEG0 (0x9F80_1040) and KSEG1 (0xBF80_1040) aliased reads
    assert_eq!(system.bus.read16(0x9F80_1040), 0xFFFF);
    assert_eq!(system.bus.read32(0xBF80_1040), 0x0000_FFFF);

    // Press Up (bit 4, lower byte) and Triangle (bit 12, upper byte) and Square (bit 15, upper byte)
    system.bus.controller.set_button(PadButton::Up, true);
    system.bus.controller.set_button(PadButton::Triangle, true);
    system.bus.controller.set_button(PadButton::Square, true);

    // Lower byte (0x1F80_1040): 0xFF & !(1 << 4) = 0xEF
    // Upper byte (0x1F80_1041): 0xFF & !((1 << 12 | 1 << 15) >> 8) = 0xFF & !(0x10 | 0x80) = 0xFF & !0x90 = 0x6F
    // 16-bit word (0x1F80_1040): 0x6FEF
    // 32-bit dword (0x1F80_1040): 0x0000_6FEF

    let read_b0 = system.bus.read8(0x1F80_1040);
    let read_b1 = system.bus.read8(0x1F80_1041);
    let read_w = system.bus.read16(0x1F80_1040);
    let read_dw = system.bus.read32(0x1F80_1040);

    assert_eq!(read_b0, 0xEF, "read8(0x1F80_1040) must be 0xEF");
    assert_eq!(read_b1, 0x6F, "read8(0x1F80_1041) must be 0x6F");
    assert_eq!(read_w, 0x6FEF, "read16(0x1F80_1040) must be 0x6FEF");
    assert_eq!(
        read_dw, 0x0000_6FEF,
        "read32(0x1F80_1040) must be 0x0000_6FEF"
    );

    // KSEG0/KSEG1 virtual address bus reads
    assert_eq!(system.bus.read16(0x9F80_1040), 0x6FEF);
    assert_eq!(system.bus.read32(0xBF80_1040), 0x0000_6FEF);

    // Release Up button
    system.bus.controller.set_button(PadButton::Up, false);
    assert_eq!(system.bus.read8(0x1F80_1040), 0xFF);
    assert_eq!(system.bus.read16(0x1F80_1040), 0x6FFF);
    assert_eq!(system.bus.read32(0x1F80_1040), 0x0000_6FFF);
}

#[test]
fn test_io_register_0x1f80_1044_joy_stat_reads() {
    let mut system = PS1System::new();

    // JOY_STAT (0x1F80_1044) returns 0x05 (RX FIFO Not Empty / TX Ready)
    assert_eq!(system.bus.read8(0x1F80_1044), 0x05);
    assert_eq!(system.bus.read16(0x1F80_1044), 0x0005);
    assert_eq!(system.bus.read32(0x1F80_1044), 0x0000_0005);

    // KSEG0 and KSEG1 aliased reads
    assert_eq!(system.bus.read16(0x9F80_1044), 0x0005);
    assert_eq!(system.bus.read32(0xBF80_1044), 0x0000_0005);
}

// ============================================================================
// 3. HeadlessRunner Execution Mode, Cycle Limits, TTY Logging, PPM Screenshots
// ============================================================================

#[test]
fn test_headless_runner_cycle_limit_precision() {
    let dummy_bios = PathBuf::from("bios/SCPH1001.BIN"); // may or may not exist

    // Case 1: max_cycles = Some(15_000)
    let mut runner = HeadlessRunner::new(&dummy_bios, None, Some(15_000), None, None)
        .expect("HeadlessRunner creation should succeed");

    let summary = runner.run().expect("Headless run should succeed");
    assert!(
        summary.total_cycles >= 15_000 && summary.total_cycles <= 15_000 + 10_000,
        "Total cycles ({}) must stop precisely within batch step boundary of 15,000",
        summary.total_cycles
    );

    // Case 2: max_cycles = Some(50_000)
    let mut runner2 = HeadlessRunner::new(&dummy_bios, None, Some(50_000), None, None)
        .expect("HeadlessRunner creation should succeed");

    let summary2 = runner2.run().expect("Headless run should succeed");
    assert!(
        summary2.total_cycles >= 50_000 && summary2.total_cycles <= 50_000 + 10_000,
        "Total cycles ({}) must stop precisely within batch step boundary of 50,000",
        summary2.total_cycles
    );

    // Case 3: max_cycles = Some(0)
    let mut runner0 = HeadlessRunner::new(&dummy_bios, None, Some(0), None, None)
        .expect("HeadlessRunner creation should succeed");

    let summary0 = runner0.run().expect("Headless run should succeed");
    assert_eq!(
        summary0.total_cycles, 0,
        "max_cycles = 0 must execute 0 cycles"
    );
}

#[test]
fn test_headless_runner_tty_output_buffering_and_file_logging() {
    let log_path = temp_file_path("test_tty_log", "log");
    let dummy_bios = PathBuf::from("bios/SCPH1001.BIN");

    let mut runner = HeadlessRunner::new(&dummy_bios, None, Some(20_000), Some(&log_path), None)
        .expect("HeadlessRunner creation with tty_log should succeed");

    // Inject TTY chars directly into memory bus tty_output
    let test_msg = "APS Headless TTY Capture Test 123!\nLine 2 stdout verification.\n";
    for ch in test_msg.bytes() {
        runner.system.bus.log_tty_char(ch);
    }

    let summary = runner.run().expect("Headless run should succeed");

    // 1. Verify in-memory TTY summary
    assert_eq!(
        summary.tty_output, test_msg,
        "In-memory TTY output must match injected characters"
    );

    // 2. Verify disk file content matches byte-for-byte
    assert!(log_path.exists(), "TTY log file must exist on disk");
    let file_content = fs::read_to_string(&log_path).expect("Failed to read TTY log file");
    assert_eq!(
        file_content, test_msg,
        "File TTY log content must match summary TTY output"
    );

    // Cleanup
    let _ = fs::remove_file(&log_path);
}

#[test]
fn test_ppm_screenshot_formatting_and_color_bounds() {
    let shot_path = temp_file_path("test_screenshot", "ppm");
    let dummy_bios = PathBuf::from("bios/SCPH1001.BIN");

    let mut runner = HeadlessRunner::new(&dummy_bios, None, Some(1000), None, Some(&shot_path))
        .expect("HeadlessRunner creation with screenshot should succeed");

    // Set up test VRAM pixel patterns in BGR555:
    // Pixel 0 (0,0): Red 0x001F (r=31, g=0, b=0 -> 248 0 0)
    // Pixel 1 (1,0): Green 0x03E0 (r=0, g=31, b=0 -> 0 248 0)
    // Pixel 2 (2,0): Blue 0x7C00 (r=0, g=0, b=31 -> 0 0 248)
    // Pixel 3 (3,0): White 0x7FFF (r=31, g=31, b=31 -> 248 248 248)
    // Pixel 4 (4,0): Black 0x0000 (0 0 0)
    runner.system.bus.gpu.vram.buffer[0] = 0x001F;
    runner.system.bus.gpu.vram.buffer[1] = 0x03E0;
    runner.system.bus.gpu.vram.buffer[2] = 0x7C00;
    runner.system.bus.gpu.vram.buffer[3] = 0x7FFF;
    runner.system.bus.gpu.vram.buffer[4] = 0x0000;

    // Run runner which writes screenshot on exit
    runner.run().expect("Headless run should succeed");

    assert!(shot_path.exists(), "PPM screenshot file must be created");

    let file = File::open(&shot_path).expect("Failed to open PPM screenshot file");
    let mut reader = BufReader::new(file);

    let mut line1 = String::new();
    reader.read_line(&mut line1).expect("Line 1 read failed");
    assert_eq!(line1.trim(), "P3", "PPM line 1 header must be 'P3'");

    let mut line2 = String::new();
    reader.read_line(&mut line2).expect("Line 2 read failed");
    assert_eq!(
        line2.trim(),
        "1024 512",
        "PPM line 2 dimensions must be '1024 512'"
    );

    let mut line3 = String::new();
    reader.read_line(&mut line3).expect("Line 3 read failed");
    assert_eq!(
        line3.trim(),
        "255",
        "PPM line 3 max color value must be '255'"
    );

    // Verify first 5 pixel color lines
    let mut pixel_lines = Vec::new();
    for _ in 0..5 {
        let mut p_line = String::new();
        reader.read_line(&mut p_line).expect("Pixel read failed");
        pixel_lines.push(p_line.trim().to_string());
    }

    assert_eq!(pixel_lines[0], "248 0 0", "Pixel (0,0) Red mapping error");
    assert_eq!(pixel_lines[1], "0 248 0", "Pixel (1,0) Green mapping error");
    assert_eq!(pixel_lines[2], "0 0 248", "Pixel (2,0) Blue mapping error");
    assert_eq!(
        pixel_lines[3], "248 248 248",
        "Pixel (3,0) White mapping error"
    );
    assert_eq!(pixel_lines[4], "0 0 0", "Pixel (4,0) Black mapping error");

    // Scan all remaining lines to ensure total line count == 1024 * 512 and all values in 0..=255
    let mut count = 5;
    for line_res in reader.lines() {
        let line = line_res.expect("Line read error");
        count += 1;
        let parts: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(parts.len(), 3, "PPM pixel line must have 3 RGB components");

        let r: u16 = parts[0].parse().expect("Invalid R value");
        let g: u16 = parts[1].parse().expect("Invalid G value");
        let b: u16 = parts[2].parse().expect("Invalid B value");

        assert!(r <= 255, "R value {r} out of bounds");
        assert!(g <= 255, "G value {g} out of bounds");
        assert!(b <= 255, "B value {b} out of bounds");
    }

    assert_eq!(
        count,
        1024 * 512,
        "Total PPM pixel lines count must equal exactly 1024 * 512 = 524,288"
    );

    // Also test direct standalone call to save_ppm_screenshot
    let shot_path2 = temp_file_path("test_screenshot2", "ppm");
    save_ppm_screenshot(&runner.system.bus.gpu, &shot_path2)
        .expect("Standalone save_ppm_screenshot should succeed");
    assert!(shot_path2.exists(), "Standalone PPM file must exist");

    // Cleanup
    let _ = fs::remove_file(&shot_path);
    let _ = fs::remove_file(&shot_path2);
}
