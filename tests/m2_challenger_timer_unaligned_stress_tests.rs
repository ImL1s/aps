use ps_core::bus::mock_bus::MockBus;
use ps_core::cpu::{Cpu, ExceptionCode};
use ps_core::timers::{Timers, IRQ_TIMER0, IRQ_TIMER1, IRQ_TIMER2};

// ============================================================================
// 1. TIMER STRESS TESTS
// ============================================================================

/// Test stepping timers with large tick values (e.g. 1_000_000, 10_000_000 cycles).
/// Verifies correct wraparound, IRQ generation, accumulator stability, and no panics.
#[test]
fn test_timer_stress_large_tick_values() {
    let mut timers = Timers::new();

    // Configure Timer 0: reset_on_target = true, irq_on_target = true, irq_repeat = true
    let mode0 = (1 << 3) | (1 << 4) | (1 << 6);
    timers.write16(0x1F80_1104, mode0);
    timers.write16(0x1F80_1108, 1000); // target = 1000

    // Configure Timer 2: clock source = SysClock / 8 (clock_src = 2), irq_on_overflow = true, irq_repeat = true
    let mode2 = (2 << 8) | (1 << 5) | (1 << 6);
    timers.write16(0x1F80_1124, mode2);

    // Step 1_000_000 cycles at once
    let irqs = timers.step(1_000_000);

    // Timer 0 should have hit target multiple times and triggered IRQ
    assert_ne!(
        irqs & (1 << IRQ_TIMER0),
        0,
        "Timer 0 IRQ must fire during large step"
    );

    // Timer 2 (1_000_000 / 8 = 125,000 ticks = nearly 2 full 65536 overflows)
    assert_ne!(
        irqs & (1 << IRQ_TIMER2),
        0,
        "Timer 2 IRQ must fire during large step"
    );

    // Step another 10_000_000 cycles to ensure stability
    let irqs_huge = timers.step(10_000_000);
    assert_ne!(irqs_huge & (1 << IRQ_TIMER0), 0);
    assert_ne!(irqs_huge & (1 << IRQ_TIMER2), 0);
}

/// Test rapid target and mode changes mid-counter execution.
#[test]
fn test_timer_stress_rapid_target_mode_changes_mid_counter() {
    let mut timers = Timers::new();

    // Start with continuous mode, target = 1000
    let mode_cont = (1 << 3) | (1 << 4) | (1 << 6);
    timers.write16(0x1F80_1104, mode_cont);
    timers.write16(0x1F80_1108, 1000);

    // Step 500 cycles -> counter at 500
    timers.step(500);
    assert_eq!(timers.read16(0x1F80_1100), 500);

    // Rapidly decrease target to 300 (less than current val 500!)
    // Counter should continue counting up to 0xFFFF, wrap to 0, then hit 300
    timers.write16(0x1F80_1108, 300);

    // Step 100 cycles -> counter at 600
    let irq1 = timers.step(100);
    assert_eq!(irq1, 0, "No IRQ yet since counter is wrapping around");
    assert_eq!(timers.read16(0x1F80_1100), 600);

    // Rapidly write mode to switch to oneshot with target = 50
    let mode_oneshot = (1 << 3) | (1 << 4); // irq_repeat = 0
    timers.write16(0x1F80_1104, mode_oneshot); // Resets val to 0
    timers.write16(0x1F80_1108, 50);

    assert_eq!(timers.read16(0x1F80_1100), 0, "write_mode resets val to 0");

    // Step 50 cycles -> triggers oneshot IRQ
    let irq2 = timers.step(50);
    assert_ne!(irq2 & (1 << IRQ_TIMER0), 0, "Oneshot IRQ must fire at 50");

    // Rapidly alternate mode settings in a tight loop while stepping
    for i in 0..100 {
        let m = if i % 2 == 0 { mode_cont } else { mode_oneshot };
        timers.write16(0x1F80_1104, m);
        timers.write16(0x1F80_1108, (i as u16 + 1) * 10);
        timers.step(5);
    }
}

/// Test multi-timer simultaneous IRQ triggering.
/// Ensures all three timers (0, 1, 2) can trigger IRQs on the exact same cycle/step.
#[test]
fn test_timer_stress_multi_timer_simultaneous_irq() {
    let mut timers = Timers::new();

    let mode = (1 << 3) | (1 << 4) | (1 << 6); // reset_on_target | irq_on_target | irq_repeat

    // Timer 0 target = 100
    timers.write16(0x1F80_1104, mode);
    timers.write16(0x1F80_1108, 100);

    // Timer 1 target = 100
    timers.write16(0x1F80_1114, mode);
    timers.write16(0x1F80_1118, 100);

    // Timer 2 target = 100 (system clock, no divider)
    timers.write16(0x1F80_1124, mode);
    timers.write16(0x1F80_1128, 100);

    // Step exactly 100 cycles
    let irqs = timers.step(100);

    let expected_mask = (1 << IRQ_TIMER0) | (1 << IRQ_TIMER1) | (1 << IRQ_TIMER2);
    assert_eq!(
        irqs & expected_mask,
        expected_mask,
        "All three timers must fire IRQs simultaneously bitmask 0x70"
    );
}

/// Test Oneshot vs Continuous mode interactions under edge conditions.
#[test]
fn test_timer_stress_oneshot_vs_continuous_edge_interactions() {
    let mut timers = Timers::new();

    // Oneshot mode setup
    let mode_oneshot = (1 << 3) | (1 << 4); // reset_on_target, irq_on_target, irq_repeat = 0
    timers.write16(0x1F80_1104, mode_oneshot);
    timers.write16(0x1F80_1108, 20);

    // 1st run -> IRQ fires at 20
    let irq1 = timers.step(20);
    assert_ne!(irq1 & (1 << IRQ_TIMER0), 0);

    // Step 200 cycles in oneshot mode without mode write -> IRQ should NOT fire
    let irq2 = timers.step(200);
    assert_eq!(
        irq2 & (1 << IRQ_TIMER0),
        0,
        "Oneshot mode must not fire twice without re-arm"
    );

    // Switch to Continuous mode mid-run via mode write
    let mode_cont = (1 << 3) | (1 << 4) | (1 << 6); // irq_repeat = 1
    timers.write16(0x1F80_1104, mode_cont);
    timers.write16(0x1F80_1108, 20);

    // 1st continuous match
    let irq3 = timers.step(20);
    assert_ne!(irq3 & (1 << IRQ_TIMER0), 0);

    // 2nd continuous match
    let irq4 = timers.step(20);
    assert_ne!(
        irq4 & (1 << IRQ_TIMER0),
        0,
        "Continuous mode must fire repeatedly"
    );

    // Switch back to Oneshot mode while counter is at 10
    timers.step(10);
    assert_eq!(timers.read16(0x1F80_1100), 10);

    timers.write16(0x1F80_1104, mode_oneshot); // mode write resets counter to 0 and re-arms
    timers.write16(0x1F80_1108, 20);

    // Step 20 cycles -> oneshot IRQ fires
    let irq5 = timers.step(20);
    assert_ne!(irq5 & (1 << IRQ_TIMER0), 0);

    // Step another 20 cycles -> no IRQ
    let irq6 = timers.step(20);
    assert_eq!(irq6 & (1 << IRQ_TIMER0), 0);
}

/// Test Timer 2 fractional divider accumulation across large steps.
#[test]
fn test_timer_stress_clock_divider_accum_large_ticks() {
    let mut timers = Timers::new();

    // Timer 2 with SysClock / 8 divider
    let mode = (2 << 8) | (1 << 3) | (1 << 4) | (1 << 6);
    timers.write16(0x1F80_1124, mode);
    timers.write16(0x1F80_1128, 100);

    // Step 7 cycles -> accum = 7, ticks = 0
    assert_eq!(timers.step(7), 0);
    assert_eq!(timers.timer2.accum, 7);
    assert_eq!(timers.read16(0x1F80_1120), 0);

    // Step 1 cycle -> total 8 cycles = 1 tick, accum = 0
    assert_eq!(timers.step(1), 0);
    assert_eq!(timers.timer2.accum, 0);
    assert_eq!(timers.read16(0x1F80_1120), 1);

    // Step 791 cycles (791 + 0 = 791 -> 98 ticks + remainder 7 accum)
    // Total ticks so far = 1 + 98 = 99 ticks
    timers.step(791);
    assert_eq!(timers.timer2.accum, 7);
    assert_eq!(timers.read16(0x1F80_1120), 99);

    // Step 1 cycle -> 100th tick -> IRQ fires and counter resets
    let irqs = timers.step(1);
    assert_ne!(irqs & (1 << IRQ_TIMER2), 0);
    assert_eq!(timers.read16(0x1F80_1120), 0);
    assert_eq!(timers.timer2.accum, 0);
}

// ============================================================================
// 2. UNALIGNED ACCESS EXCEPTION STRESS TESTS
// ============================================================================

/// Test unaligned memory loads/stores (LH, LHU, LW, SH, SW) inside branch delay slots.
/// Verifies Cause.BD = 1, EPC = Branch PC, BadVAddr = target address, and vectoring.
#[test]
fn test_unaligned_stress_loads_stores_in_branch_delay_slots() {
    let mut bus = MockBus::new();

    // Test cases: (Opcode, Unaligned Addr, Expected ExcCode)
    let test_cases = [
        (
            0x85280000u32,
            0x8000_1001u32,
            ExceptionCode::AddressErrorLoad,
        ), // LH $r8, 0($r9)
        (
            0x95280000u32,
            0x8000_1003u32,
            ExceptionCode::AddressErrorLoad,
        ), // LHU $r8, 0($r9)
        (
            0x8D280000u32,
            0x8000_1002u32,
            ExceptionCode::AddressErrorLoad,
        ), // LW $r8, 0($r9)
        (
            0xA5280000u32,
            0x8000_1001u32,
            ExceptionCode::AddressErrorStore,
        ), // SH $r8, 0($r9)
        (
            0xAD280000u32,
            0x8000_1003u32,
            ExceptionCode::AddressErrorStore,
        ), // SW $r8, 0($r9)
    ];

    for (inst_word, unaligned_addr, expected_exc) in test_cases {
        let mut cpu = Cpu::new();
        cpu.pc = 0x8000_0100;
        cpu.gpr[1] = 10;
        cpu.gpr[2] = 20;
        cpu.gpr[9] = unaligned_addr;

        // 0x8000_0100: BNE $r1, $r2, 4 (0x14220004) -> untaken branch or taken
        // 0x8000_0104: inst_word (unaligned load/store in delay slot!)
        bus.load_code(0x8000_0100, &0x14220004u32.to_le_bytes());
        bus.load_code(0x8000_0104, &inst_word.to_le_bytes());

        cpu.step(&mut bus); // Execute BNE
        cpu.step(&mut bus); // Execute delay slot instruction -> triggers unaligned exception!

        assert_eq!(
            cpu.cop0.cause & 0x8000_0000,
            0x8000_0000,
            "Cause.BD bit must be 1 for delay slot exception"
        );
        assert_eq!(
            cpu.cop0.cause_exc_code(),
            expected_exc as u32,
            "ExcCode mismatch"
        );
        assert_eq!(
            cpu.cop0.epc, 0x8000_0100,
            "EPC must point to branch instruction at 0x8000_0100"
        );
        assert_eq!(
            cpu.cop0.badvaddr, unaligned_addr,
            "BadVAddr must equal unaligned memory address"
        );
        assert_eq!(
            cpu.pc, 0xBFC0_0180,
            "PC must vector to boot exception handler when BEV=1"
        );
    }
}

/// Test branch jumping to unaligned instruction address (unaligned PC fetch exception).
/// Verifies Cause.BD = 0 (exception occurs on fetching target, NOT in delay slot),
/// BadVAddr = unaligned PC, EPC = unaligned PC.
#[test]
fn test_unaligned_stress_branch_target_unaligned_pc_fetch() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0200;
    cpu.gpr[31] = 0x8000_0301; // Unaligned jump target!

    // 0x8000_0200: JR $r31 (0x03E00008)
    // 0x8000_0204: NOP     (0x00000000) (delay slot executed cleanly)
    bus.load_code(0x8000_0200, &0x03E00008u32.to_le_bytes());
    bus.load_code(0x8000_0204, &0x00000000u32.to_le_bytes());

    cpu.step(&mut bus); // Execute JR
    cpu.step(&mut bus); // Execute delay slot NOP (delay slot completes normally)
    cpu.step(&mut bus); // Fetch instruction at unaligned PC 0x8000_0301 -> Exception!

    assert_eq!(
        cpu.cop0.cause & 0x8000_0000,
        0,
        "Cause.BD must be 0 because PC fetch exception is NOT in delay slot"
    );
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::AddressErrorLoad as u32,
        "ExcCode must be AddressErrorLoad"
    );
    assert_eq!(
        cpu.cop0.badvaddr, 0x8000_0301,
        "BadVAddr must equal unaligned target PC"
    );
    assert_eq!(
        cpu.cop0.epc, 0x8000_0301,
        "EPC must equal unaligned target PC"
    );
    assert_eq!(cpu.pc, 0xBFC0_0180);
}

/// Test exception vectoring behavior under Status.BEV = 0 vs Status.BEV = 1.
#[test]
fn test_unaligned_stress_exception_vectoring_bev_0_vs_bev_1() {
    let mut bus = MockBus::new();

    // Scenario A: BEV = 1 (bit 22 set in Status) -> Vectors to 0xBFC0_0180
    {
        let mut cpu = Cpu::new();
        cpu.pc = 0x8000_0000;
        cpu.cop0.status = 0x0040_0000; // BEV = 1
        cpu.gpr[9] = 0x8000_1001; // Unaligned LW address
        bus.load_code(0x8000_0000, &0x8D280000u32.to_le_bytes());

        cpu.step(&mut bus);
        assert_eq!(
            cpu.pc, 0xBFC0_0180,
            "When BEV=1, exception vector must be 0xBFC0_0180"
        );
    }

    // Scenario B: BEV = 0 (bit 22 clear in Status) -> Vectors to 0x8000_0080
    {
        let mut cpu = Cpu::new();
        cpu.pc = 0x8000_0000;
        cpu.cop0.status = 0x0000_0000; // BEV = 0
        cpu.gpr[9] = 0x8000_1002; // Unaligned SW address
        bus.load_code(0x8000_0000, &0xAD280000u32.to_le_bytes());

        cpu.step(&mut bus);
        assert_eq!(
            cpu.pc, 0x8000_0080,
            "When BEV=0, exception vector must be 0x8000_0080"
        );
    }
}

/// Test 3-level mode stack shifting across nested exceptions and RFE instructions.
#[test]
fn test_unaligned_stress_nested_exception_stack_3_level_shifting() {
    let mut cpu = Cpu::new();

    // Initial Status: User Mode (KUc=1), Interrupts Enabled (IEc=1) -> bits 5..0 = 0b000003 (0x03)
    cpu.cop0.status = 0x0040_0003;

    // 1st Exception: Syscall from User Mode
    let vec1 = cpu
        .cop0
        .trigger_exception(ExceptionCode::Syscall, false, 0x8000_1000);
    assert_eq!(vec1, 0xBFC0_0180);
    // Stack shift: KUc/IEc (0,0), KUp/IEp (1,1), KUo/IEo (0,0) -> bits 5..0 = 0b001100 (0x0C)
    assert_eq!(
        cpu.cop0.status & 0x3F,
        0x0C,
        "1st exception push: bits 5..0 should be 0x0C"
    );
    assert_eq!(cpu.cop0.epc, 0x8000_1000);

    // 2nd Exception: Unaligned load while inside 1st exception handler
    let vec2 = cpu
        .cop0
        .trigger_exception(ExceptionCode::AddressErrorLoad, false, 0x8000_2000);
    assert_eq!(vec2, 0xBFC0_0180);
    // Stack shift: KUc/IEc (0,0), KUp/IEp (0,0), KUo/IEo (1,1) -> bits 5..0 = 0b110000 (0x30)
    assert_eq!(
        cpu.cop0.status & 0x3F,
        0x30,
        "2nd exception push: bits 5..0 should be 0x30"
    );
    assert_eq!(cpu.cop0.epc, 0x8000_2000);

    // Execute 1st RFE (returns from 2nd exception handler to 1st exception handler)
    cpu.cop0.rfe();
    // After RFE: bits 3..0 get shifted right by 2: KUp/IEp (1,1) -> KUc/IEc (0,0), KUp/IEp (1,1)
    // bits 5..0 = 0b111100 (0x3C)
    assert_eq!(
        cpu.cop0.status & 0x3F,
        0x3C,
        "1st RFE pop: bits 5..0 should be 0x3C"
    );

    // Execute 2nd RFE (returns from 1st exception handler to original User Mode)
    cpu.cop0.rfe();
    // After 2nd RFE: KUc/IEc (1,1) -> Original User Mode & IE=1 restored!
    assert_eq!(
        cpu.cop0.status & 0x03,
        0x03,
        "2nd RFE pop: original User Mode (KUc=1, IEc=1) must be restored"
    );
}

/// Test JALR branch delay slot unaligned store exception.
/// Verifies $ra is updated by JALR before the exception aborts execution,
/// Cause.BD = 1, EPC = JALR PC.
#[test]
fn test_unaligned_stress_jalr_delay_slot_unaligned_store() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0400;
    cpu.gpr[4] = 0x8000_0800; // Target PC for JALR
    cpu.gpr[9] = 0x8000_1001; // Unaligned store address

    // 0x8000_0400: JALR $r4, $r31 (0x0080F809)
    // 0x8000_0404: SW $r8, 0($r9) (0xAD280000) (unaligned store in delay slot!)
    bus.load_code(0x8000_0400, &0x0080F809u32.to_le_bytes());
    bus.load_code(0x8000_0404, &0xAD280000u32.to_le_bytes());

    cpu.step(&mut bus); // Execute JALR -> sets $r31 = 0x8000_0408, next_in_delay_slot = true
    assert_eq!(
        cpu.gpr[31], 0x8000_0408,
        "JALR must set return address in $r31"
    );

    cpu.step(&mut bus); // Execute SW (unaligned store in delay slot!)

    assert_eq!(cpu.cop0.cause & 0x8000_0000, 0x8000_0000, "Cause.BD = 1");
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::AddressErrorStore as u32
    );
    assert_eq!(cpu.cop0.epc, 0x8000_0400, "EPC must point to JALR");
    assert_eq!(cpu.cop0.badvaddr, 0x8000_1001);
    assert_eq!(cpu.pc, 0xBFC0_0180);
}
