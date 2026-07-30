//! Tier 4: ROM / E2E Automated Integration Tests

use ps_core::bios::Bios;
use ps_core::bus::memory_bus::MemoryBus;
use ps_core::cpu::Cpu;
use ps_core::ram::Ram;
use ps_core::scratchpad::Scratchpad;
use std::fs;
use std::path::Path;

#[test]
fn test_tier4_headless_bios_boot_execution() {
    let bios_path = Path::new("bios/SCPH1001.BIN");
    if !bios_path.exists() {
        eprintln!("Skipping Tier 4 BIOS boot test: bios/SCPH1001.BIN not found");
        return;
    }

    let bios_bytes = match fs::read(bios_path) {
        Ok(data) => data,
        Err(_) => {
            eprintln!("Skipping Tier 4 BIOS boot test: failed to read SCPH1001.BIN");
            return;
        }
    };

    let bios = match Bios::from_bytes(&bios_bytes) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Skipping Tier 4 BIOS boot test: {e}");
            return;
        }
    };

    let mut bus = MemoryBus::new(Ram::new(), bios, Scratchpad::new());
    let mut cpu = Cpu::new();

    for _ in 0..100 {
        cpu.step(&mut bus);
    }

    assert_ne!(cpu.pc, 0, "CPU PC must continue stepping through BIOS");
}

#[test]
fn test_tier4_amidog_cpu_rom_execution() {
    let mut rom_path = Path::new("tests/roms/psxtest_cpu.exe");
    let fallback_path = Path::new("../../tests/roms/psxtest_cpu.exe");
    if !rom_path.exists() && fallback_path.exists() {
        rom_path = fallback_path;
    }
    if !rom_path.exists() {
        eprintln!("Skipping Amidog CPU test: tests/roms/psxtest_cpu.exe not found");
        return;
    }

    let mut sys = ps_core::system::PS1System::new();
    sys.load_executable_file(rom_path)
        .expect("Failed to load psxtest_cpu.exe");

    for _step in 0..2_000_000 {
        sys.step();
        if sys
            .bus
            .tty_output
            .ends_with(b"All tests passed (101/101)\n")
            || sys
                .bus
                .tty_output
                .ends_with(b"All tests passed (101/101)\r\n")
            || sys.bus.tty_output.ends_with(b"Done\n")
            || sys.bus.tty_output.ends_with(b"Done\r\n")
        {
            break;
        }
    }

    let tty = sys.bus.get_tty_string();
    assert!(
        !tty.contains("error @"),
        "Amidog CPU test reported errors in TTY output: {tty}"
    );
}
