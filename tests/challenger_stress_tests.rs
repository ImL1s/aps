//! Stress tests for CPU engine and MemoryBus edge cases.
//! Category 1: Branch delay slot interaction with load delay slots.
//! Category 2: Register write overwrite precedence (Load vs ALU/Load).
//! Category 3: Division by zero and MIN_INT / -1 signed overflow in DIV/DIVU.
//! Category 4: Address segment masking boundary values across KUSEG, KSEG0, KSEG1.

use ps_core::bus::map::mask_address;
use ps_core::bus::memory_bus::MemoryBus;
use ps_core::bus::mock_bus::MockBus;
use ps_core::bus::Bus;
use ps_core::cpu::Cpu;

// ============================================================================
// Category 1: Branch delay slot interaction with load delay slots
// ============================================================================

#[test]
fn test_challenger_load_inside_branch_delay_slot() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Data at 0x8000_0100: 0x1122_3344
    bus.load_code(0x8000_0100, &0x1122_3344u32.to_le_bytes());

    // Program:
    // 0x8000_0000: J 0x8000_0010 (Jump to target 0x8000_0010) -> 0x08000004
    // 0x8000_0004: LW $t0 (r8), 0x100($r0) (Load in branch delay slot) -> 0x8C080100
    // 0x8000_0008: NOP (Never executed due to branch) -> 0x00000000
    // 0x8000_000C: NOP (Never executed due to branch) -> 0x00000000
    // 0x8000_0010: ADDIU $t1 (r9), $t0 (r8), 0x1000 (Target inst 1: load delay slot of LW) -> 0x25091000
    // 0x8000_0014: ADDIU $t2 (r10), $t0 (r8), 0x2000 (Target inst 2: load committed) -> 0x250A2000
    let code: [(u32, u32); 6] = [
        (0x8000_0000, 0x08000004), // J 0x8000_0010
        (0x8000_0004, 0x8C080100), // LW $t0, 0x100($r0)
        (0x8000_0008, 0x00000000), // NOP
        (0x8000_000C, 0x00000000), // NOP
        (0x8000_0010, 0x25091000), // ADDIU $t1, $t0, 0x1000
        (0x8000_0014, 0x250A2000), // ADDIU $t2, $t0, 0x2000
    ];

    for (addr, inst) in code {
        bus.load_code(addr, &inst.to_le_bytes());
    }

    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;

    // Step 1: J 0x8000_0010
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8000_0004, "PC must be branch delay slot address");
    assert!(
        cpu.next_in_delay_slot,
        "next_in_delay_slot must be true after branch instruction"
    );

    // Step 2: LW $t0, 0x100($r0) inside branch delay slot
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8000_0010, "PC must jump to target 0x8000_0010");
    assert!(
        !cpu.next_in_delay_slot,
        "next_in_delay_slot reset after delay slot execution"
    );
    assert_eq!(cpu.gpr[8], 0, "$t0 not updated yet (pending load)");

    // Step 3: ADDIU $t1, $t0, 0x1000 at target 0x8000_0010
    // This instruction is in the load delay slot of LW from step 2!
    // It should read the OLD value of $t0 (0).
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8000_0014, "PC advances to 0x8000_0014");
    assert_eq!(
        cpu.gpr[9], 0x1000,
        "Instruction in load delay slot at branch target must read old $t0 value (0)"
    );

    // Step 4: ADDIU $t2, $t0, 0x2000 at target 0x8000_0014
    // At the start of step 4, $t0 is committed (0x1122_3344).
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 0x1122_3344, "$t0 committed value");
    assert_eq!(
        cpu.gpr[10], 0x1122_5344,
        "Instruction 2 cycles after load at branch target must see loaded $t0"
    );
}

#[test]
fn test_challenger_conditional_branch_load_delay_slot() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Data at 0x8000_0100: 0x9988_7766
    bus.load_code(0x8000_0100, &0x9988_7766u32.to_le_bytes());

    // BEQ $r0, $r0, +2 instructions (PC 0x8000_0000 + 4 + (2<<2) = 0x8000_000C) -> 0x10000002
    // Delay slot: LW $t0 (r8), 0x100($r0) -> 0x8C080100
    // Target (0x8000_000C): ORI $t1 (r9), $t0 (r8), 0x00FF -> 0x350900FF (load delay slot of LW)
    // Target+4 (0x8000_0010): ORI $t2 (r10), $t0 (r8), 0x00FF -> 0x350A00FF (load committed)

    bus.load_code(0x8000_0000, &0x10000002u32.to_le_bytes());
    bus.load_code(0x8000_0004, &0x8C080100u32.to_le_bytes());
    bus.load_code(0x8000_000C, &0x350900FFu32.to_le_bytes());
    bus.load_code(0x8000_0010, &0x350A00FFu32.to_le_bytes());

    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;

    cpu.step(&mut bus); // BEQ taken
    cpu.step(&mut bus); // LW in branch delay slot
    assert_eq!(cpu.pc, 0x8000_000C, "Branch target PC");

    cpu.step(&mut bus); // ORI $t1 in load delay slot
    assert_eq!(cpu.gpr[9], 0x00FF, "Load delay slot sees old $t0 (0)");

    cpu.step(&mut bus); // ORI $t2 after load commit
    assert_eq!(cpu.gpr[8], 0x9988_7766, "$t0 loaded");
    assert_eq!(cpu.gpr[10], 0x9988_77FF, "$t2 computed using loaded $t0");
}

// ============================================================================
// Category 2: Register write overwrite precedence
// ============================================================================

#[test]
fn test_challenger_load_then_alu_overwrite_precedence() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Data at 0x8000_0100: 0x1111_1111
    bus.load_code(0x8000_0100, &0x1111_1111u32.to_le_bytes());

    // N:   LW $t0 (r8), 0x100($r0) -> 0x8C080100
    // N+1: ADDIU $t0 (r8), $r0, 0x2222 -> 0x24082222 (ALU write to $t0 in load delay slot)
    // N+2: NOP -> 0x00000000
    // N+3: NOP -> 0x00000000
    bus.load_code(0x8000_0000, &0x8C080100u32.to_le_bytes());
    bus.load_code(0x8000_0004, &0x24082222u32.to_le_bytes());
    bus.load_code(0x8000_0008, &0x00000000u32.to_le_bytes());
    bus.load_code(0x8000_000C, &0x00000000u32.to_le_bytes());

    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;

    cpu.step(&mut bus); // LW schedules pending load of 0x1111_1111
    cpu.step(&mut bus); // ADDIU writes 0x2222 and cancels pending load write to $t0
    assert_eq!(
        cpu.gpr[8], 0x2222,
        "ALU write immediately sets $t0 to 0x2222"
    );

    cpu.step(&mut bus); // NOP
    assert_eq!(
        cpu.gpr[8], 0x2222,
        "$t0 must remain 0x2222, pending load was cancelled"
    );

    cpu.step(&mut bus); // NOP
    assert_eq!(cpu.gpr[8], 0x2222, "$t0 remains 0x2222 permanently");
}

#[test]
fn test_challenger_consecutive_loads_to_same_register() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Data at 0x8000_0100: 0xAAAA_AAAA
    // Data at 0x8000_0104: 0xBBBB_BBBB
    bus.load_code(0x8000_0100, &0xAAAA_AAAAu32.to_le_bytes());
    bus.load_code(0x8000_0104, &0xBBBB_BBBBu32.to_le_bytes());

    // N:   LW $t0 (r8), 0x100($r0) -> 0x8C080100
    // N+1: LW $t0 (r8), 0x104($r0) -> 0x8C080104 (Back-to-back load to same reg)
    // N+2: NOP -> 0x00000000
    // N+3: NOP -> 0x00000000
    bus.load_code(0x8000_0000, &0x8C080100u32.to_le_bytes());
    bus.load_code(0x8000_0004, &0x8C080104u32.to_le_bytes());
    bus.load_code(0x8000_0008, &0x00000000u32.to_le_bytes());
    bus.load_code(0x8000_000C, &0x00000000u32.to_le_bytes());

    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;

    cpu.step(&mut bus); // LW 1: pending load 0xAAAA_AAAA
    cpu.step(&mut bus); // LW 2: overrides pending/current load with 0xBBBB_BBBB
    cpu.step(&mut bus); // NOP: pending moves to current
    cpu.step(&mut bus); // NOP: commit second load
    assert_eq!(
        cpu.gpr[8], 0xBBBB_BBBB,
        "Second load value must overwrite first load"
    );
}

#[test]
fn test_challenger_alu_then_load_same_register() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    bus.load_code(0x8000_0100, &0x7777_7777u32.to_le_bytes());

    // N:   ADDIU $t0 (r8), $r0, 0x1234 -> 0x24081234
    // N+1: LW $t0 (r8), 0x100($r0) -> 0x8C080100
    // N+2: ORI $t1 (r9), $t0 (r8), 0x0000 -> 0x35090000 (load delay slot reads ALU value)
    // N+3: NOP -> 0x00000000
    bus.load_code(0x8000_0000, &0x24081234u32.to_le_bytes());
    bus.load_code(0x8000_0004, &0x8C080100u32.to_le_bytes());
    bus.load_code(0x8000_0008, &0x35090000u32.to_le_bytes());
    bus.load_code(0x8000_000C, &0x00000000u32.to_le_bytes());

    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;

    cpu.step(&mut bus); // ADDIU $t0 = 0x1234
    assert_eq!(cpu.gpr[8], 0x1234);

    cpu.step(&mut bus); // LW $t0 schedules 0x7777_7777
    assert_eq!(cpu.gpr[8], 0x1234);

    cpu.step(&mut bus); // ORI $t1 reads load delay slot (ALU value 0x1234)
    assert_eq!(
        cpu.gpr[9], 0x1234,
        "Load delay slot must read ALU value 0x1234"
    );

    cpu.step(&mut bus); // NOP commits load
    assert_eq!(cpu.gpr[8], 0x7777_7777, "Loaded value committed to $t0");
}

// ============================================================================
// Category 3: Division by zero and MIN_INT / -1 signed overflow
// ============================================================================

#[test]
fn test_challenger_div_by_zero_positive_numerator() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.gpr[8] = 42; // Numerator = +42
    cpu.gpr[9] = 0; // Denominator = 0

    // DIV $t0 (r8), $t1 (r9) -> 0x0109001A
    bus.load_code(0x8000_0000, &0x0109001Au32.to_le_bytes());

    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;

    cpu.step(&mut bus);

    assert_eq!(cpu.hi, 42, "DIV by zero (pos num) HI must equal numerator");
    assert_eq!(
        cpu.lo, 0xFFFF_FFFF,
        "DIV by zero (pos num) LO must equal 0xFFFF_FFFF (-1)"
    );
}

#[test]
fn test_challenger_div_by_zero_negative_numerator() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.gpr[8] = (-42i32) as u32; // Numerator = -42 (0xFFFFFFD6)
    cpu.gpr[9] = 0; // Denominator = 0

    // DIV $t0 (r8), $t1 (r9) -> 0x0109001A
    bus.load_code(0x8000_0000, &0x0109001Au32.to_le_bytes());

    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;

    cpu.step(&mut bus);

    assert_eq!(
        cpu.hi, 0xFFFFFFD6,
        "DIV by zero (neg num) HI must equal numerator (-42)"
    );
    assert_eq!(cpu.lo, 1, "DIV by zero (neg num) LO must equal 1");
}

#[test]
fn test_challenger_divu_by_zero() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.gpr[8] = 0x1234_5678; // Numerator
    cpu.gpr[9] = 0; // Denominator = 0

    // DIVU $t0 (r8), $t1 (r9) -> 0x0109001B
    bus.load_code(0x8000_0000, &0x0109001Bu32.to_le_bytes());

    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;

    cpu.step(&mut bus);

    assert_eq!(cpu.hi, 0x1234_5678, "DIVU by zero HI must equal numerator");
    assert_eq!(
        cpu.lo, 0xFFFF_FFFF,
        "DIVU by zero LO must equal 0xFFFF_FFFF"
    );
}

#[test]
fn test_challenger_div_min_int_overflow() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.gpr[8] = 0x8000_0000; // MIN_INT (-2147483648)
    cpu.gpr[9] = 0xFFFF_FFFF; // -1

    // DIV $t0 (r8), $t1 (r9) -> 0x0109001A
    bus.load_code(0x8000_0000, &0x0109001Au32.to_le_bytes());

    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;

    cpu.step(&mut bus);

    assert_eq!(cpu.hi, 0, "DIV MIN_INT / -1 HI must be 0");
    assert_eq!(
        cpu.lo, 0x8000_0000,
        "DIV MIN_INT / -1 LO must be 0x8000_0000 (MIN_INT)"
    );
}

// ============================================================================
// Category 4: Address segment masking boundary values
// ============================================================================

#[test]
fn test_challenger_mask_address_boundary_values() {
    let boundaries: [(u32, u32, &str); 6] = [
        (0x0000_0000, 0x0000_0000, "KUSEG start"),
        (0x7FFF_FFFF, 0x1FFF_FFFF, "KUSEG end"),
        (0x8000_0000, 0x0000_0000, "KSEG0 start"),
        (0x9FFF_FFFF, 0x1FFF_FFFF, "KSEG0 end"),
        (0xA000_0000, 0x0000_0000, "KSEG1 start"),
        (0xBFFF_FFFF, 0x1FFF_FFFF, "KSEG1 end"),
    ];

    for (vaddr, expected_paddr, label) in boundaries {
        assert_eq!(
            mask_address(vaddr),
            expected_paddr,
            "mask_address({vaddr}) failed for {label}"
        );
    }
}

#[test]
fn test_challenger_memory_bus_segment_aliasing_and_mirroring() {
    let mut bus = MemoryBus::default_bus();

    // 1. RAM Start Boundary (0x0000_0000): Write via KUSEG, read via KSEG0 and KSEG1
    bus.write32(0x0000_0000, 0xDEAD_BEEF);
    assert_eq!(bus.read32(0x0000_0000), 0xDEAD_BEEF, "KUSEG RAM start read");
    assert_eq!(
        bus.read32(0x8000_0000),
        0xDEAD_BEEF,
        "KSEG0 RAM start alias"
    );
    assert_eq!(
        bus.read32(0xA000_0000),
        0xDEAD_BEEF,
        "KSEG1 RAM start alias"
    );

    // 2. 2MB RAM End Boundary (0x001F_FFFC): Write via KSEG0, read via KUSEG and KSEG1
    bus.write32(0x801F_FFFC, 0xCAFE_BABE);
    assert_eq!(bus.read32(0x001F_FFFC), 0xCAFE_BABE, "KUSEG RAM end alias");
    assert_eq!(bus.read32(0x801F_FFFC), 0xCAFE_BABE, "KSEG0 RAM end read");
    assert_eq!(bus.read32(0xA01F_FFFC), 0xCAFE_BABE, "KSEG1 RAM end alias");

    // 3. RAM 2MB Mirroring at 0x0020_0000: Write via KSEG1 (0xA020_0000), read via KUSEG (0x0000_0000)
    bus.write32(0xA020_0000, 0x1122_3344);
    assert_eq!(bus.read32(0x0000_0000), 0x1122_3344, "KUSEG RAM mirror 0");
    assert_eq!(bus.read32(0x8000_0000), 0x1122_3344, "KSEG0 RAM mirror 0");
    assert_eq!(bus.read32(0xA000_0000), 0x1122_3344, "KSEG1 RAM mirror 0");

    // 4. Scratchpad Boundary (0x1F80_0000..0x1F80_03FF) across KUSEG/KSEG0/KSEG1
    bus.write32(0xBF80_0000, 0x55AA_55AA);
    assert_eq!(
        bus.read32(0x1F80_0000),
        0x55AA_55AA,
        "KUSEG Scratchpad alias"
    );
    assert_eq!(
        bus.read32(0x9F80_0000),
        0x55AA_55AA,
        "KSEG0 Scratchpad alias"
    );
    assert_eq!(
        bus.read32(0xBF80_0000),
        0x55AA_55AA,
        "KSEG1 Scratchpad read"
    );

    // 5. BIOS Boundary (0x1FC0_0000..0x1FC7_FFFF) across KUSEG/KSEG0/KSEG1
    // Load data into BIOS
    bus.bios.data[0] = 0x12;
    bus.bios.data[1] = 0x34;
    bus.bios.data[2] = 0x56;
    bus.bios.data[3] = 0x78;
    assert_eq!(bus.read32(0x1FC0_0000), 0x7856_3412, "KUSEG BIOS read");
    assert_eq!(bus.read32(0x9FC0_0000), 0x7856_3412, "KSEG0 BIOS alias");
    assert_eq!(bus.read32(0xBFC0_0000), 0x7856_3412, "KSEG1 BIOS alias");
}
