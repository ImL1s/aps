//! Adversarial Stress Tests for DMA Linked Lists, OTC Backwards Pointers, DICR IRQ3, and Timer IRQs (M2)

use ps_core::bus::memory_bus::MemoryBus;
use ps_core::bus::Bus;
use ps_core::dma::DmaController;
use ps_core::gpu::Gpu;
use ps_core::intc::{InterruptController, IRQ_DMA, IRQ_TIMER0, IRQ_TIMER1, IRQ_TIMER2};
use ps_core::ram::Ram;
use ps_core::timers::Timers;

// ============================================================================
// 1. DMA Mode 2 Linked List Tests
// ============================================================================

#[test]
fn test_challenger_dma_mode2_linked_list_end_of_table_marker() {
    let mut dma = DmaController::new();
    let mut ram = Ram::new();
    let mut gpu = Gpu::new();
    let mut intc = InterruptController::new();

    // Enable Channel 2 (GPU) in DPCR
    dma.dpcr |= 1 << (2 * 4 + 3);

    // Write end marker at 0x1000: 0 words payload, next = 0x00FFFFFF
    ram.write32(0x1000, 0x00FF_FFFF);

    dma.channels[2].madr = 0x1000;
    dma.channels[2].chcr = (1 << 24) | (2 << 9); // Trigger + SyncMode 2 (Linked List)

    let initial_draw_mode = gpu.gp0.draw_mode;
    let executed = dma.step_dma(&mut ram, &mut gpu, &mut intc);

    assert!(executed, "DMA step must execute");
    assert_eq!(
        dma.channels[2].madr, 0x00FF_FFFF,
        "MADR must update to end marker 0x00FFFFFF"
    );
    assert!(
        !dma.channels[2].is_trigger_set(),
        "Trigger bit must be cleared after list completion"
    );
    assert_eq!(
        gpu.gp0.draw_mode, initial_draw_mode,
        "No GP0 state change for empty packet"
    );
}

#[test]
fn test_challenger_dma_mode2_linked_list_multi_node_payload_stream() {
    let mut dma = DmaController::new();
    let mut ram = Ram::new();
    let mut gpu = Gpu::new();
    let mut intc = InterruptController::new();

    dma.dpcr |= 1 << (2 * 4 + 3); // Enable Ch 2

    // Node 1 @ 0x1000: 2 payload words, next = 0x2000
    ram.write32(0x1000, (2 << 24) | 0x2000);
    ram.write32(0x1004, 0xE100_0011); // GP0 Draw Mode
    ram.write32(0x1008, 0xE200_0022); // GP0 Texture Window

    // Node 2 @ 0x2000: 0 payload words, next = 0x3000
    ram.write32(0x2000, 0x3000);

    // Node 3 @ 0x3000: 3 payload words, next = 0x4000
    ram.write32(0x3000, (3 << 24) | 0x4000);
    ram.write32(0x3004, 0xE300_0033); // GP0 Drawing Area Top-Left (x1=0x33, y1=0)
    ram.write32(0x3008, 0xE400_0044); // GP0 Drawing Area Bottom-Right (x2=0x44, y2=0)
    ram.write32(0x300C, 0xE500_0055); // GP0 Drawing Offset (sx=0x55, sy=0)

    // Node 4 @ 0x4000: 1 payload word, next = 0x00FFFFFF (End marker)
    ram.write32(0x4000, (1 << 24) | 0x00FF_FFFF);
    ram.write32(0x4004, 0xE100_0066); // GP0 Draw Mode

    dma.channels[2].madr = 0x1000;
    dma.channels[2].chcr = (1 << 24) | (2 << 9); // Trigger + SyncMode 2

    let executed = dma.step_dma(&mut ram, &mut gpu, &mut intc);
    assert!(executed);

    assert_eq!(dma.channels[2].madr, 0x00FF_FFFF);
    assert_eq!(gpu.gp0.clip.x1, 0x33);
    assert_eq!(gpu.gp0.clip.x2, 0x44);
    assert_eq!(gpu.gp0.draw_offset_x, 0x55);
    assert_eq!(gpu.gp0.draw_mode, 0x0000_0066);
}

#[test]
fn test_challenger_dma_mode2_linked_list_address_masking_and_upper_bits() {
    let mut dma = DmaController::new();
    let mut ram = Ram::new();
    let mut gpu = Gpu::new();
    let mut intc = InterruptController::new();

    dma.dpcr |= 1 << (2 * 4 + 3);

    // Upper bits set in header: 0xFF_001000
    // Node 1: header has upper bits 0xAA in payload count byte area except top count (1),
    // next_ptr = 0xFF_002000
    ram.write32(0x1000, (1 << 24) | 0xFF_002000);
    ram.write32(0x1004, 0xE100_0077);

    // Node 2 @ 0x2000: next_ptr = 0xFE_FFFFFF
    ram.write32(0x2000, 0xFEFF_FFFF);

    // Setup MADR with upper bits set (e.g. KSEG0 0x8000_1000)
    dma.channels[2].madr = 0x8000_1000;
    dma.channels[2].chcr = (1 << 24) | (2 << 9);

    let executed = dma.step_dma(&mut ram, &mut gpu, &mut intc);
    assert!(executed);

    assert_eq!(
        dma.channels[2].madr, 0x00FF_FFFF,
        "MADR must be masked to 0x00FFFFFF"
    );
    assert_eq!(gpu.gp0.draw_mode, 0x0000_0077);
}

// ============================================================================
// 2. OTC Backwards Pointer Chain Tests
// ============================================================================

#[test]
fn test_challenger_otc_backwards_pointer_single_element() {
    let mut dma = DmaController::new();
    let mut ram = Ram::new();
    let mut gpu = Gpu::new();
    let mut intc = InterruptController::new();

    dma.dpcr |= 1 << (6 * 4 + 3); // Enable Ch6 (OTC)

    dma.channels[6].madr = 0x1000;
    dma.channels[6].bcr = 1; // 1 word
    dma.channels[6].chcr = 1 << 24; // Trigger

    let executed = dma.step_dma(&mut ram, &mut gpu, &mut intc);
    assert!(executed);

    // Single element at 0x1000 should contain end marker 0x00FFFFFF
    assert_eq!(ram.read32(0x1000), 0x00FF_FFFF);
    assert_eq!(dma.channels[6].madr, 0x1000);
}

#[test]
fn test_challenger_otc_backwards_pointer_multi_element() {
    let mut dma = DmaController::new();
    let mut ram = Ram::new();
    let mut gpu = Gpu::new();
    let mut intc = InterruptController::new();

    dma.dpcr |= 1 << (6 * 4 + 3);

    let start_addr = 0x10000u32;
    let count = 16u32;

    dma.channels[6].madr = start_addr;
    dma.channels[6].bcr = count;
    dma.channels[6].chcr = 1 << 24;

    dma.step_dma(&mut ram, &mut gpu, &mut intc);

    // Verify chain
    let mut curr_addr = start_addr;
    for i in 1..count {
        let expected_next = curr_addr - 4;
        assert_eq!(
            ram.read32(curr_addr),
            expected_next,
            "OTC link at element {i} (0x{curr_addr:X}) failed"
        );
        curr_addr = expected_next;
    }
    // Last element points to end marker 0x00FFFFFF
    assert_eq!(
        ram.read32(curr_addr),
        0x00FF_FFFF,
        "Last OTC link must be 0x00FFFFFF"
    );
    assert_eq!(dma.channels[6].madr, curr_addr);
}

#[test]
fn test_challenger_otc_backwards_pointer_large_bcr_zero() {
    let mut dma = DmaController::new();
    let mut ram = Ram::new();
    let mut gpu = Gpu::new();
    let mut intc = InterruptController::new();

    dma.dpcr |= 1 << (6 * 4 + 3);

    // BCR = 0 is interpreted as 0x10000 (65536 words)
    let start_addr = 0x001F_FFFCu32; // Top of 2MB RAM
    dma.channels[6].madr = start_addr;
    dma.channels[6].bcr = 0;
    dma.channels[6].chcr = 1 << 24;

    dma.step_dma(&mut ram, &mut gpu, &mut intc);

    // First node at 0x001F_FFFC points to 0x001F_FFF8
    assert_eq!(ram.read32(start_addr), start_addr - 4);

    // Final node address: 0x001F_FFFC - (65535 * 4) = 0x001B_FFFC
    let final_addr = start_addr - (65535 * 4);
    assert_eq!(ram.read32(final_addr), 0x00FF_FFFF);
    assert_eq!(dma.channels[6].madr, final_addr);
}

// ============================================================================
// 3. DMA DICR IRQ3 Assertions
// ============================================================================

#[test]
fn test_challenger_dma_dicr_irq3_triggering_and_clearing() {
    let mut dma = DmaController::new();
    let mut ram = Ram::new();
    let mut gpu = Gpu::new();
    let mut intc = InterruptController::new();

    // Configure DICR: Master Enable (bit 23) + Channel 2 Enable (bit 18)
    dma.write32(0x1F80_10F4, (1 << 23) | (1 << 18), &mut intc);

    // Enable Ch 2 in DPCR
    dma.write32(0x1F80_10F0, 1 << (2 * 4 + 3), &mut intc);

    // Trigger Ch2 block transfer
    dma.channels[2].madr = 0x1000;
    dma.channels[2].bcr = 4;
    dma.channels[2].chcr = 1 << 24;

    dma.step_dma(&mut ram, &mut gpu, &mut intc);

    // DICR bit 26 (Ch2 IRQ flag) and bit 31 (Master IRQ flag) must be set
    let dicr = dma.read32(0x1F80_10F4);
    assert_ne!(dicr & (1 << 26), 0, "Ch2 IRQ flag (bit 26) must be set");
    assert_ne!(dicr & (1 << 31), 0, "Master IRQ flag (bit 31) must be set");

    // INTC I_STAT bit 3 (IRQ_DMA) must be set
    assert_ne!(
        intc.read32(0x1F80_1070) & (1 << IRQ_DMA),
        0,
        "IRQ_DMA must be asserted on INTC"
    );

    // Write 1 to DICR bit 26 to clear Ch2 IRQ flag
    dma.write32(0x1F80_10F4, 1 << 26, &mut intc);

    let dicr_after = dma.read32(0x1F80_10F4);
    assert_eq!(dicr_after & (1 << 26), 0, "Ch2 IRQ flag must be cleared");
    assert_eq!(
        dicr_after & (1 << 31),
        0,
        "Master IRQ flag must clear when no active flags remain"
    );
}

#[test]
fn test_challenger_dma_dicr_force_irq_bit() {
    let mut dma = DmaController::new();
    let mut intc = InterruptController::new();

    // Write DICR bit 15 (Force IRQ) without Master Enable or Channel Enables
    dma.write32(0x1F80_10F4, 1 << 15, &mut intc);

    let dicr = dma.read32(0x1F80_10F4);
    assert_ne!(dicr & (1 << 31), 0, "Force IRQ must assert bit 31");
    assert_ne!(
        intc.read32(0x1F80_1070) & (1 << IRQ_DMA),
        0,
        "Force IRQ must trigger IRQ_DMA on INTC"
    );

    // Clear Force IRQ bit
    dma.write32(0x1F80_10F4, 0, &mut intc);
    let dicr_cleared = dma.read32(0x1F80_10F4);
    assert_eq!(
        dicr_cleared & (1 << 31),
        0,
        "Bit 31 must clear when Force IRQ is removed"
    );
}

// ============================================================================
// 4. Timer 0/1/2 Target Match and Overflow IRQ Assertions
// ============================================================================

#[test]
fn test_challenger_timer0_target_match_varying_cycles() {
    let mut timers = Timers::new();
    let _intc = InterruptController::new();

    // Mode: reset_on_target (bit 3) | irq_on_target (bit 4) | irq_repeat (bit 6)
    let mode = (1 << 3) | (1 << 4) | (1 << 6);
    timers.write16(0x1F80_1104, mode);
    timers.write16(0x1F80_1108, 25);

    // Test 1: Step 1 cycle at a time for 24 cycles -> No IRQ yet
    for _ in 0..24 {
        let irqs = timers.step(1);
        assert_eq!(irqs, 0);
    }
    assert_eq!(timers.read16(0x1F80_1100), 24);

    // 25th cycle -> Trigger IRQ, val resets to 0
    let irqs = timers.step(1);
    assert_ne!(irqs & (1 << IRQ_TIMER0), 0);
    assert_eq!(timers.read16(0x1F80_1100), 0);

    // Test 2: Step 37 cycles in a single step -> should wrap (37 - 25 = 12) and trigger IRQ
    let irqs_large = timers.step(37);
    assert_ne!(irqs_large & (1 << IRQ_TIMER0), 0);
    assert_eq!(timers.read16(0x1F80_1100), 12);
}

#[test]
fn test_challenger_timer1_overflow_irq_varying_cycles() {
    let mut timers = Timers::new();

    // Mode: irq_on_overflow (bit 5) | irq_repeat (bit 6)
    let mode = (1 << 5) | (1 << 6);
    timers.write16(0x1F80_1114, mode);
    timers.write16(0x1F80_1118, 0); // Target = 0 (ignored for overflow test)

    // Set val = 0xFFF0
    timers.write16(0x1F80_1110, 0xFFF0);

    // Step 15 cycles -> val = 0xFFFF, no overflow yet
    let irqs = timers.step(15);
    assert_eq!(irqs, 0);
    assert_eq!(timers.read16(0x1F80_1110), 0xFFFF);

    // Step 5 cycles -> passes 0xFFFF -> 0 -> 0x0004. Triggers IRQ_TIMER1
    let irqs_overflow = timers.step(5);
    assert_ne!(irqs_overflow & (1 << IRQ_TIMER1), 0);
    assert_eq!(timers.read16(0x1F80_1110), 4);

    // Mode reached_overflow bit (bit 12) must have been set
    let read_m = timers.read16(0x1F80_1114);
    assert_ne!(read_m & (1 << 12), 0, "Reached overflow bit must be set");
}

#[test]
fn test_challenger_timer2_sysclock_divider_and_fractional_cycles() {
    let mut timers = Timers::new();

    // Mode: clock_source = SysClock/8 (2 << 8), reset_on_target (1 << 3), irq_on_target (1 << 4), irq_repeat (1 << 6)
    let mode = (2 << 8) | (1 << 3) | (1 << 4) | (1 << 6);
    timers.write16(0x1F80_1124, mode);
    timers.write16(0x1F80_1128, 5);

    // Step 7 cycles -> fractional accumulation (accum = 7), 0 ticks
    let irqs1 = timers.step(7);
    assert_eq!(irqs1, 0);
    assert_eq!(timers.read16(0x1F80_1120), 0);

    // Step 3 cycles -> accum total = 10 -> 1 tick (accum rem = 2), val = 1
    let irqs2 = timers.step(3);
    assert_eq!(irqs2, 0);
    assert_eq!(timers.read16(0x1F80_1120), 1);

    // Step 32 cycles -> 4 ticks (val increases 1 -> 5), reaches target 5!
    let irqs3 = timers.step(32);
    assert_ne!(
        irqs3 & (1 << IRQ_TIMER2),
        0,
        "Timer 2 target match IRQ must trigger"
    );
    assert_eq!(
        timers.read16(0x1F80_1120),
        0,
        "Timer 2 value must reset on target"
    );
}

#[test]
fn test_challenger_timer_oneshot_vs_repeat_mode() {
    let mut timers = Timers::new();

    // One-shot mode: irq_on_target (1 << 4), reset_on_target (1 << 3), irq_repeat = 0
    let mode_oneshot = (1 << 3) | (1 << 4);
    timers.write16(0x1F80_1104, mode_oneshot);
    timers.write16(0x1F80_1108, 10);

    // Step 10 cycles -> First target match fires IRQ
    let irqs1 = timers.step(10);
    assert_ne!(
        irqs1 & (1 << IRQ_TIMER0),
        0,
        "One-shot IRQ must fire on 1st match"
    );

    // Step 10 cycles -> Second target match does NOT fire IRQ
    let irqs2 = timers.step(10);
    assert_eq!(
        irqs2 & (1 << IRQ_TIMER0),
        0,
        "One-shot IRQ must NOT fire on 2nd match"
    );

    // Re-arm timer by rewriting mode
    timers.write16(0x1F80_1104, mode_oneshot);

    // Step 10 cycles -> Third match (after re-arm) fires IRQ
    let irqs3 = timers.step(10);
    assert_ne!(
        irqs3 & (1 << IRQ_TIMER0),
        0,
        "One-shot IRQ must fire after re-arming"
    );
}

#[test]
fn test_challenger_memory_bus_timer_irq_propagation_to_intc() {
    let mut bus = MemoryBus::default();

    // Enable IRQ_TIMER0 (bit 4) in INTC I_MASK
    bus.write32(0x1F80_1074, 1 << IRQ_TIMER0);

    // Configure Timer 0 target = 10, reset_on_target, irq_on_target, irq_repeat
    let mode = (1 << 3) | (1 << 4) | (1 << 6);
    bus.write16(0x1F80_1104, mode);
    bus.write16(0x1F80_1108, 10);

    // Step bus by 10 cycles
    bus.step(10);

    // Verify INTC I_STAT has IRQ_TIMER0 bit set and CPU IRQ is asserted
    let istat = bus.read32(0x1F80_1070);
    assert_ne!(
        istat & (1 << IRQ_TIMER0),
        0,
        "MemoryBus step must propagate Timer IRQ to INTC I_STAT"
    );
    assert!(
        bus.intc.is_cpu_irq_asserted(),
        "CPU IRQ must be asserted on INTC when masked"
    );
}
