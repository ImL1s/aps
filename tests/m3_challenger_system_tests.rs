//! Empirical Stress Test Suite for PS1System, EXE Loading, Batch Stepping, VRAM ARGB32 Conversion, and Key Mapping
//! Target: Milestone M3 Verification

use ps_core::bus::Bus;
use ps_core::controller::{map_key_to_button, PadButton};
use ps_core::system::PS1System;
use sdl2::keyboard::Keycode;

// Helper to construct a valid 0x800-byte PS-X EXE header
fn create_psx_exe_header(
    pc: u32,
    gp: u32,
    load_addr: u32,
    text_size: u32,
    sp: u32,
    text_payload: &[u8],
) -> Vec<u8> {
    let mut header = vec![0u8; 0x800];
    // Magic header "PS-X EXE"
    header[0..8].copy_from_slice(b"PS-X EXE");

    header[0x10..0x14].copy_from_slice(&pc.to_le_bytes());
    header[0x14..0x18].copy_from_slice(&gp.to_le_bytes());
    header[0x18..0x1C].copy_from_slice(&load_addr.to_le_bytes());
    header[0x1C..0x20].copy_from_slice(&text_size.to_le_bytes());
    header[0x30..0x34].copy_from_slice(&sp.to_le_bytes());

    header.extend_from_slice(text_payload);
    header
}

#[test]
fn test_psx_exe_header_custom_addresses_and_registers() {
    let text_payload: Vec<u8> = (0..0x800).map(|i| (i & 0xFF) as u8).collect();
    let header_data = create_psx_exe_header(
        0x8005_4000, // pc
        0x800E_1234, // gp
        0x8005_4000, // load_addr
        0x0000_0800, // text_size
        0x801F_E000, // sp
        &text_payload,
    );

    let mut sys = PS1System::new();
    sys.load_executable_bytes(&header_data)
        .expect("PS-X EXE loading should succeed");

    assert_eq!(
        sys.cpu.pc, 0x8005_4000,
        "CPU $pc must match header initial_pc"
    );
    assert_eq!(
        sys.cpu.next_pc, 0x8005_4004,
        "CPU $next_pc must be initial_pc + 4"
    );
    assert_eq!(
        sys.cpu.gpr[28], 0x800E_1234,
        "CPU $gp (gpr[28]) must match header initial_gp"
    );
    assert_eq!(
        sys.cpu.gpr[29], 0x801F_E000,
        "CPU $sp (gpr[29]) must match header initial_sp"
    );

    // Physical RAM address for KSEG0 0x8005_4000 is 0x0005_4000
    let phys_addr = 0x0005_4000;
    assert_eq!(
        &sys.bus.ram.data[phys_addr..phys_addr + 0x800],
        &text_payload[..],
        "RAM data at target physical address must match EXE text payload"
    );
}

#[test]
fn test_psx_exe_header_kseg1_uncached_load_address() {
    let text_payload = vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    let header_data = create_psx_exe_header(
        0xA001_8000, // KSEG1 virtual address
        0x800E_0000,
        0xA001_8000,
        text_payload.len() as u32,
        0x801F_F000,
        &text_payload,
    );

    let mut sys = PS1System::new();
    sys.load_executable_bytes(&header_data).unwrap();

    assert_eq!(sys.cpu.pc, 0xA001_8000);
    assert_eq!(sys.cpu.next_pc, 0xA001_8004);

    // KSEG1 0xA001_8000 masks to physical 0x0001_8000
    let phys_addr = 0x0001_8000;
    assert_eq!(
        &sys.bus.ram.data[phys_addr..phys_addr + text_payload.len()],
        &text_payload[..]
    );
}

#[test]
fn test_psx_exe_header_zero_gp_sp_registers_unchanged() {
    let mut sys = PS1System::new();
    // Pre-populate $gp and $sp
    sys.cpu.gpr[28] = 0x1234_5678;
    sys.cpu.gpr[29] = 0x9ABC_DEF0;

    let header_data = create_psx_exe_header(
        0x8001_0000,
        0x0000_0000, // gp = 0 (should not overwrite gpr[28])
        0x8001_0000,
        0x100,
        0x0000_0000, // sp = 0 (should not overwrite gpr[29])
        &[0u8; 0x100],
    );

    sys.load_executable_bytes(&header_data).unwrap();

    assert_eq!(
        sys.cpu.gpr[28], 0x1234_5678,
        "CPU $gp must remain unchanged when header initial_gp is 0"
    );
    assert_eq!(
        sys.cpu.gpr[29], 0x9ABC_DEF0,
        "CPU $sp must remain unchanged when header initial_sp is 0"
    );
}

#[test]
fn test_psx_exe_text_size_exceeds_payload_clamping() {
    let text_payload = vec![0xAB; 0x200]; // 512 bytes payload
    let header_data = create_psx_exe_header(
        0x8001_0000,
        0x800E_0000,
        0x8001_0000,
        0x0001_0000, // Claimed text_size = 64KB
        0x801F_F000,
        &text_payload,
    );

    let mut sys = PS1System::new();
    // Must not panic due to slice index out of bounds
    sys.load_executable_bytes(&header_data).unwrap();

    let phys_addr = 0x0001_0000;
    assert_eq!(
        &sys.bus.ram.data[phys_addr..phys_addr + 0x200],
        &text_payload[..],
        "Available payload bytes must be copied into RAM without crashing"
    );
}

#[test]
fn test_psx_exe_header_truncated_file_size() {
    // Header starting with "PS-X EXE" but total size < 0x800
    let mut header_data = vec![0u8; 0x400];
    header_data[0..8].copy_from_slice(b"PS-X EXE");

    let mut sys = PS1System::new();
    // Truncated header should trigger raw binary loading fallback
    sys.load_executable_bytes(&header_data).unwrap();

    assert_eq!(
        sys.cpu.pc, 0x8001_0000,
        "Truncated header must fall back to raw binary entry point 0x8001_0000"
    );
    assert_eq!(sys.cpu.next_pc, 0x8001_0004);
}

#[test]
fn test_raw_binary_fallback_loading_at_0x8001_0000() {
    let raw_code = vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];

    let mut sys = PS1System::new();
    sys.load_executable_bytes(&raw_code).unwrap();

    assert_eq!(
        sys.cpu.pc, 0x8001_0000,
        "Raw binary fallback $pc must be 0x8001_0000"
    );
    assert_eq!(
        sys.cpu.next_pc, 0x8001_0004,
        "Raw binary fallback $next_pc must be 0x8001_0004"
    );

    let phys_addr = 0x0001_0000;
    assert_eq!(
        &sys.bus.ram.data[phys_addr..phys_addr + raw_code.len()],
        &raw_code[..],
        "Raw binary code must be loaded at physical address 0x0001_0000"
    );
}

#[test]
fn test_raw_binary_oversized_payload_clamping() {
    // 3MB payload (exceeds 2MB RAM)
    let huge_raw_code = vec![0xFF; 3 * 1024 * 1024];

    let mut sys = PS1System::new();
    sys.load_executable_bytes(&huge_raw_code).unwrap();

    let phys_addr = 0x0001_0000;
    let expected_capacity = sys.bus.ram.data.len() - phys_addr;
    assert_eq!(
        sys.bus.ram.data[phys_addr..phys_addr + 100],
        vec![0xFF; 100][..],
        "Oversized raw binary must fill RAM safely up to capacity limit"
    );
    assert_eq!(
        sys.bus.ram.data.len() - phys_addr,
        expected_capacity,
        "Copy length must be clamped to remaining RAM capacity"
    );
}

#[test]
fn test_raw_binary_empty_buffer() {
    let mut sys = PS1System::new();
    sys.load_executable_bytes(&[]).unwrap();

    assert_eq!(sys.cpu.pc, 0x8001_0000);
    assert_eq!(sys.cpu.next_pc, 0x8001_0004);
}

#[test]
fn test_system_step_batch_advancement() {
    let mut sys = PS1System::new();
    assert_eq!(sys.total_cycles, 0);

    // Step by 1 60FPS frame (564,480 cycles)
    sys.step_batch(564_480);
    assert_eq!(sys.total_cycles, 564_480);

    // Step another 10,000 cycles
    sys.step_batch(10_000);
    assert_eq!(sys.total_cycles, 574_480);
}

#[test]
fn test_render_vram_to_argb32_all_zeros() {
    let sys = PS1System::new();
    let mut frame_buffer = vec![0u32; 1024 * 512];

    sys.bus.gpu.render_vram_to_argb32(&mut frame_buffer);

    for (i, &pixel) in frame_buffer.iter().enumerate() {
        assert_eq!(
            pixel, 0xFF00_0000,
            "VRAM pixel 0x0000 must render as opaque black 0xFF00_0000 at index {i}"
        );
    }
}

#[test]
fn test_render_vram_to_argb32_all_0ffff() {
    let mut sys = PS1System::new();
    // Fill VRAM with 0xFFFF
    sys.bus.gpu.vram.buffer.fill(0xFFFF);

    let mut frame_buffer = vec![0u32; 1024 * 512];
    sys.bus.gpu.render_vram_to_argb32(&mut frame_buffer);

    // 0xFFFF: R=31 (0xF8), G=31 (0xF8), B=31 (0xF8) -> 0xFFF8_F8F8
    for (i, &pixel) in frame_buffer.iter().enumerate() {
        assert_eq!(
            pixel, 0xFFF8_F8F8,
            "VRAM pixel 0xFFFF must render as 0xFFF8_F8F8 at index {i}"
        );
    }
}

#[test]
fn test_render_vram_to_argb32_bgr555_color_bit_shifts() {
    let mut sys = PS1System::new();

    // Set specific VRAM pixels
    sys.bus.gpu.vram.buffer[0] = 0x001F; // Pure Red
    sys.bus.gpu.vram.buffer[1] = 0x03E0; // Pure Green
    sys.bus.gpu.vram.buffer[2] = 0x7C00; // Pure Blue
    sys.bus.gpu.vram.buffer[3] = 0x8000; // Mask bit set (0 color bits)
    sys.bus.gpu.vram.buffer[4] = 0x3DEF; // R=15, G=15, B=15 -> 15<<3=120 (0x78)

    let mut frame_buffer = vec![0u32; 16];
    sys.bus.gpu.render_vram_to_argb32(&mut frame_buffer);

    assert_eq!(
        frame_buffer[0], 0xFFF8_0000,
        "Pure Red 0x001F (5-bit 31 << 3 = 0xF8) must convert to ARGB32 0xFFF8_0000"
    );
    assert_eq!(
        frame_buffer[1], 0xFF00_F800,
        "Pure Green 0x03E0 (5-bit 31 << 3 = 0xF8) must convert to ARGB32 0xFF00_F800"
    );
    assert_eq!(
        frame_buffer[2], 0xFF00_00F8,
        "Pure Blue 0x7C00 (5-bit 31 << 3 = 0xF8) must convert to ARGB32 0xFF00_00F8"
    );
    assert_eq!(
        frame_buffer[3], 0xFF00_0000,
        "Mask bit 0x8000 must render as black 0xFF00_0000"
    );
    assert_eq!(
        frame_buffer[4], 0xFF78_7878,
        "Custom BGR555 0x3DEF must convert to ARGB32 0xFF78_7878"
    );
}

#[test]
fn test_render_vram_to_argb32_buffer_truncation() {
    let mut sys = PS1System::new();
    sys.bus.gpu.vram.buffer[0] = 0x001F;
    sys.bus.gpu.vram.buffer[99] = 0x03E0;

    let mut small_frame_buffer = vec![0u32; 100];
    sys.bus.gpu.render_vram_to_argb32(&mut small_frame_buffer);

    assert_eq!(small_frame_buffer.len(), 100);
    assert_eq!(small_frame_buffer[0], 0xFFF8_0000);
    assert_eq!(small_frame_buffer[99], 0xFF00_F800);
}

#[test]
fn test_map_key_to_button_all_supported_keycodes() {
    let expected_mappings = vec![
        (Keycode::RShift, PadButton::Select),
        (Keycode::Backspace, PadButton::Select),
        (Keycode::Return, PadButton::Start),
        (Keycode::Space, PadButton::Start),
        (Keycode::Up, PadButton::Up),
        (Keycode::Right, PadButton::Right),
        (Keycode::Down, PadButton::Down),
        (Keycode::Left, PadButton::Left),
        (Keycode::E, PadButton::L2),
        (Keycode::Num3, PadButton::L2),
        (Keycode::R, PadButton::R2),
        (Keycode::Num4, PadButton::R2),
        (Keycode::Q, PadButton::L1),
        (Keycode::Num1, PadButton::L1),
        (Keycode::W, PadButton::R1),
        (Keycode::Num2, PadButton::R1),
        (Keycode::S, PadButton::Triangle),
        (Keycode::I, PadButton::Triangle),
        (Keycode::X, PadButton::Circle),
        (Keycode::K, PadButton::Circle),
        (Keycode::Z, PadButton::Cross),
        (Keycode::J, PadButton::Cross),
        (Keycode::A, PadButton::Square),
        (Keycode::U, PadButton::Square),
    ];

    for (key, expected_btn) in expected_mappings {
        assert_eq!(
            map_key_to_button(key),
            Some(expected_btn),
            "Keycode {key:?} must map to PadButton {expected_btn:?}"
        );
    }
}

#[test]
fn test_map_key_to_button_unsupported_keycodes() {
    let unsupported_keys = vec![
        Keycode::F1,
        Keycode::F5,
        Keycode::F12,
        Keycode::Tab,
        Keycode::Escape,
        Keycode::LAlt,
        Keycode::LCtrl,
        Keycode::LShift,
        Keycode::Num0,
        Keycode::P,
        Keycode::V,
        Keycode::C,
        Keycode::D,
        Keycode::F,
        Keycode::G,
        Keycode::H,
        Keycode::L,
        Keycode::M,
        Keycode::N,
        Keycode::O,
        Keycode::T,
        Keycode::Y,
    ];

    for key in unsupported_keys {
        assert_eq!(
            map_key_to_button(key),
            None,
            "Keycode {key:?} must map to None"
        );
    }
}

#[test]
fn test_key_mapping_integration_with_controller_active_low_bus() {
    let mut sys = PS1System::new();

    // Default 0x1F80_1040 is 0xFFFF (active low, released)
    assert_eq!(sys.bus.read16(0x1F80_1040), 0xFFFF);

    // Map 'Z' key -> Cross button
    let btn = map_key_to_button(Keycode::Z).expect("Z key must map to PadButton::Cross");
    sys.bus.controller.set_button(btn, true);

    // Active low: bit 14 (Cross) cleared to 0
    let io_val = sys.bus.read16(0x1F80_1040);
    assert_eq!(
        io_val & (1 << 14),
        0,
        "Bit 14 must be 0 when Cross is pressed"
    );

    // Map 'Up' key -> Up button (bit 4)
    let up_btn = map_key_to_button(Keycode::Up).expect("Up key must map to PadButton::Up");
    sys.bus.controller.set_button(up_btn, true);

    let io_val_both = sys.bus.read16(0x1F80_1040);
    assert_eq!(io_val_both & (1 << 14), 0);
    assert_eq!(io_val_both & (1 << 4), 0);

    // Release both buttons
    sys.bus.controller.set_button(btn, false);
    sys.bus.controller.set_button(up_btn, false);

    assert_eq!(
        sys.bus.read16(0x1F80_1040),
        0xFFFF,
        "All bits must reset to 1 (0xFFFF) when released"
    );
}
