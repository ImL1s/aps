//! Tier 3: Integration Tests (CPU-DMA-GPU-Timer-INTC pipeline interaction)

use ps_core::bios::Bios;
use ps_core::bus::memory_bus::MemoryBus;
use ps_core::bus::Bus;
use ps_core::cpu::Cpu;
use ps_core::ram::Ram;
use ps_core::scratchpad::Scratchpad;

#[test]
fn test_tier3_cpu_memory_bus_integration() {
    let mut bus = MemoryBus::new(Ram::new(), Bios::new(), Scratchpad::new());
    let mut cpu = Cpu::new();

    // ADDIU $t0 (r8), $r0, 0x1234 -> 0x24081234
    // SW $t0 (r8), 0x100($r0) -> 0xAC080100
    // LW $t1 (r9), 0x100($r0) -> 0x8C090100
    let code: [u32; 3] = [
        0x24081234, // ADDIU $r8, $r0, 0x1234
        0xAC080100, // SW $r8, 0x100($r0)
        0x8C090100, // LW $r9, 0x100($r0)
    ];

    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;
    for (i, op) in code.iter().enumerate() {
        bus.write32(0x8000_0000 + (i as u32 * 4), *op);
    }

    // Step 1: ADDIU $r8
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 0x1234, "ADDIU must load 0x1234 into $r8");

    // Step 2: SW $r8 to 0x8000_0100
    cpu.step(&mut bus);
    assert_eq!(
        bus.read32(0x8000_0100),
        0x1234,
        "SW must write 0x1234 to MemoryBus RAM"
    );

    // Step 3: LW $r9 from 0x8000_0100 (Schedules load)
    cpu.step(&mut bus); // LW executes (schedules pending load)
    cpu.step(&mut bus); // Pipeline advances pending to current
    cpu.step(&mut bus); // Pipeline commits current load to gpr[9]
    assert_eq!(
        cpu.gpr[9], 0x1234,
        "LW via MemoryBus must retrieve written value into $r9"
    );
}

#[test]
fn test_tier3_scratchpad_cpu_access() {
    let mut bus = MemoryBus::new(Ram::new(), Bios::new(), Scratchpad::new());
    let mut cpu = Cpu::new();

    // Scratchpad is mapped to 0x1F80_0000..0x1F80_0400
    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;
    cpu.gpr[8] = 0xDEAD_BEEF;
    cpu.gpr[10] = 0x1F80_0010; // Scratchpad address

    // SW $r8, 0($r10) => 0xAD480000
    bus.write32(0x8000_0000, 0xAD480000);

    cpu.step(&mut bus);
    assert_eq!(
        bus.read32(0x1F80_0010),
        0xDEAD_BEEF,
        "MemoryBus must route SW to Scratchpad"
    );
}

#[test]
fn test_tier3_cpu_dma_gpu_pipeline_chain() {
    let mut bus = MemoryBus::default();
    let mut cpu = Cpu::new();

    // Enable DMA Ch 2 (GPU) in DPCR
    bus.write32(0x1F80_10F0, 0x0765_4321 | (1 << 11));

    // Write GPU linked list in RAM at 0x8000_2000
    // Packet: 3 words payload, next = 0x00FFFFFF
    // GP0 Command: Fill Rectangle (0x02) Color (0, 255, 0) Green
    // X=10, Y=10, W=20, H=20
    bus.write32(0x8000_2000, (3 << 24) | 0x00FF_FFFF);
    bus.write32(0x8000_2004, 0x0200_FF00); // Green color fill
    bus.write32(0x8000_2008, (10 << 16) | 10); // X=10, Y=10
    bus.write32(0x8000_200C, (20 << 16) | 20); // W=20, H=20

    // Set DMA Ch2 MADR = 0x0000_2000 (RAM address)
    bus.write32(0x1F80_10A0, 0x0000_2000);
    // Set DMA Ch2 CHCR = Trigger (1 << 24) | Mode 2 (2 << 9)
    bus.write32(0x1F80_10A8, (1 << 24) | (2 << 9));

    // Execute CPU step & Bus step
    cpu.step(&mut bus);
    bus.step(100);

    // Verify GPU VRAM has been drawn by DMA pipeline chain
    assert_ne!(
        bus.gpu.vram.get_pixel(15, 15),
        0,
        "CPU-DMA-GPU pipeline chain must execute fill rect into VRAM"
    );
}

#[test]
fn test_tier3_controller_active_low_io_mapping() {
    use ps_core::controller::PadButton;
    use ps_core::system::PS1System;

    let mut system = PS1System::new();

    // Default status: 0xFFFF (active low, no buttons pressed)
    assert_eq!(system.bus.read16(0x1F80_1040), 0xFFFF);

    // Press Cross button (bit 14)
    system.bus.controller.set_button(PadButton::Cross, true);
    let state = system.bus.read16(0x1F80_1040);
    assert_eq!(
        state & (1 << 14),
        0,
        "Bit 14 (Cross) must be 0 when pressed"
    );

    // Release Cross button
    system.bus.controller.set_button(PadButton::Cross, false);
    assert_eq!(system.bus.read16(0x1F80_1040), 0xFFFF);
}

#[test]
fn test_tier3_timer0_intc_interrupt_assertion_chain() {
    use ps_core::system::PS1System;

    let mut system = PS1System::new();

    // Set Timer 0 target = 100, mode = reset on target + enable interrupt
    system.bus.write32(0x1F80_1104, 0x0018); // Mode: Reset on target (bit 3) + IRQ on target (bit 4)
    system.bus.write32(0x1F80_1108, 100); // Target: 100 cycles
    system.bus.write32(0x1F80_1074, 1 << 4); // INTC I_MASK: enable Timer 0 IRQ (bit 4)

    // Step 105 cycles
    system.step_batch(105);

    // Verify I_STAT bit 4 set and IRQ asserted
    let istat = system.bus.read32(0x1F80_1070);
    assert_ne!(
        istat & (1 << 4),
        0,
        "Timer 0 interrupt bit must be set in I_STAT"
    );
    assert!(system.bus.intc.is_cpu_irq_asserted());
}
