//! Tier 2: Boundary Tests (Exceptions, memory alignment, segment masking, hardware subsystems)

use ps_core::bios::Bios;
use ps_core::bus::memory_bus::MemoryBus;
use ps_core::bus::mock_bus::MockBus;
use ps_core::bus::Bus;
use ps_core::cpu::{Cpu, ExceptionCode};
use ps_core::intc::IRQ_TIMER0;

#[test]
fn test_tier2_unaligned_load_exception() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0000;
    cpu.gpr[8] = 0x8000_0000; // $t0 = 0x8000_0000
    cpu.cop0.status = 0; // BEV = 0 for normal vector 0x8000_0080
                         // Unaligned load word LW $t0, 1($t0) => 0x8D080001
    let inst_bytes = 0x8D080001u32.to_le_bytes();
    bus.load_code(0x8000_0000, &inst_bytes);

    cpu.step(&mut bus);

    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::AddressErrorLoad as u32,
        "Must trigger AdEL exception"
    );
    assert_eq!(
        cpu.cop0.badvaddr, 0x8000_0001,
        "BadVAddr must contain unaligned address 0x8000_0001"
    );
    assert_eq!(
        cpu.pc, 0x8000_0080,
        "PC must vector to exception handler 0x8000_0080"
    );
}

#[test]
fn test_tier2_memory_segment_aliasing() {
    let mut bus = MockBus::new();

    bus.write32(0x0000_1000, 0xDEAD_BEEF);

    let val_kseg0 = bus.read32(0x8000_1000);
    let val_kseg1 = bus.read32(0xA000_1000);

    assert_eq!(val_kseg0, 0xDEAD_BEEF, "KSEG0 must alias physical RAM");
    assert_eq!(val_kseg1, 0xDEAD_BEEF, "KSEG1 must alias physical RAM");
}

#[test]
fn test_tier2_ram_2mb_mirroring() {
    let mut bus = MockBus::new();

    bus.write32(0x0000_0004, 0xCAFE_BABE);
    let val_mirror = bus.read32(0x0020_0004);

    assert_eq!(
        val_mirror, 0xCAFE_BABE,
        "2MB RAM mirroring must return identical data"
    );
}

#[test]
fn test_tier2_intc_status_mask_assertion() {
    let mut bus = MemoryBus::default();

    // Mask for IRQ 0 (VBLANK) and IRQ 4 (Timer 0)
    bus.write32(0x1F80_1074, (1 << 0) | (1 << 4));
    assert_eq!(bus.read32(0x1F80_1074), (1 << 0) | (1 << 4));

    // Manually trigger IRQ 0
    bus.intc.trigger(0);
    assert_eq!(bus.read32(0x1F80_1070), 1 << 0);
    assert!(bus.intc.is_cpu_irq_asserted());

    // Write 1 to clear IRQ 0 in I_STAT
    bus.write32(0x1F80_1070, 1 << 0);
    assert_eq!(bus.read32(0x1F80_1070), 0);
    assert!(!bus.intc.is_cpu_irq_asserted());
}

#[test]
fn test_tier2_timer_target_overflow_boundary() {
    let mut bus = MemoryBus::default();

    // Configure Timer 0 target = 5, mode = reset_on_target (bit 3) | irq_on_target (bit 4) | irq_repeat (bit 6)
    let mode = (1 << 3) | (1 << 4) | (1 << 6);
    bus.write16(0x1F80_1104, mode);
    bus.write16(0x1F80_1108, 5);

    // Step 5 cycles
    bus.step(5);

    // INTC I_STAT bit 4 (IRQ_TIMER0) should be set
    let istat = bus.read32(0x1F80_1070);
    assert_ne!(
        istat & (1 << IRQ_TIMER0),
        0,
        "Timer 0 target match must assert IRQ_TIMER0 on INTC"
    );
}

#[test]
fn test_tier2_dma_linked_list_traversal() {
    let mut bus = MemoryBus::default();

    // Enable DMA Ch2 (GPU) in DPCR
    bus.write32(0x1F80_10F0, 0x0765_4321 | (1 << 11));

    // Write linked list header at 0x1000: 1 word payload, next = 0x00FFFFFF (end)
    bus.write32(0x1000, (1 << 24) | 0x00FF_FFFF);
    // Payload GP0 command: GP0(0xE1) Draw Mode
    bus.write32(0x1004, 0xE100_0055);

    // Set Ch2 MADR = 0x1000, CHCR = Trigger | SyncMode 2
    bus.write32(0x1F80_10A0, 0x1000);
    bus.write32(0x1F80_10A8, (1 << 24) | (2 << 9));

    bus.step(1);

    // Check Ch2 MADR is end pointer
    assert_eq!(bus.read32(0x1F80_10A0), 0x00FF_FFFF);
    assert_eq!(bus.gpu.gp0.draw_mode, 0x0000_0055);
}

#[test]
fn test_tier2_gpu_gp0_gp1_gpustat_boundary() {
    let mut bus = MemoryBus::default();

    // Default GPUSTAT should have bits 26, 27, 28 set (0x1400_0000) and Display Enable (0x0080_0000)
    let stat = bus.read32(0x1F80_1814);
    assert_eq!(stat & 0x1480_0000, 0x1480_0000);

    // GP1 Display Enable = OFF (cmd 0x0300_0001)
    bus.write32(0x1F80_1814, 0x0300_0001);
    let stat_off = bus.read32(0x1F80_1814);
    assert_ne!(stat_off & (1 << 23), 0);

    // GP0 CPU-to-VRAM (0xA0) transfer to (x=0, y=0, w=2, h=1)
    bus.write32(0x1F80_1810, 0xA000_0000);
    bus.write32(0x1F80_1810, 0x0000_0000); // Dst X=0, Y=0
    bus.write32(0x1F80_1810, 0x0001_0002); // W=2, H=1
    bus.write32(0x1F80_1810, 0x03E0_001F); // Pixels: 0x001F (Red), 0x03E0 (Green)

    assert_eq!(bus.gpu.vram.get_pixel(0, 0), 0x001F);
    assert_eq!(bus.gpu.vram.get_pixel(1, 0), 0x03E0);
}

#[test]
fn test_tier2_bios_reset_vector_fetch() {
    let bios_bytes = vec![0x12, 0x34, 0x56, 0x78];
    let bios = Bios::from_bytes(&bios_bytes).unwrap();
    let bus = MemoryBus::new(
        ps_core::ram::Ram::new(),
        bios,
        ps_core::scratchpad::Scratchpad::new(),
    );

    // Reset vector at KSEG1 0xBFC0_0000 maps to physical BIOS 0x1FC0_0000
    let mut mut_bus = bus;
    let word = mut_bus.read32(0xBFC0_0000);
    assert_eq!(word, 0x7856_3412);
}
