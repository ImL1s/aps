use ps_core::bus::mock_bus::MockBus;
use ps_core::bus::Bus;
use ps_core::cpu::{Cpu, ExceptionCode};

#[test]
fn test_lwl_all_offsets() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Memory contains 0x12345678 at 0x8000_1000
    // Little-endian byte layout at 0x8000_1000:
    // +0: 0x78, +1: 0x56, +2: 0x34, +3: 0x12
    bus.write32(0x8000_1000, 0x1234_5678);

    // Initial register value: 0xDEAD_BEEF (B3=0xDE, B2=0xAD, B1=0xBE, B0=0xEF)

    // Offset 0: LWL $r8, 0($r9) where $r9 = 0x8000_1000
    // Loads byte +0 (0x78) into MSB (bits 31..24). Bits 23..0 preserved (0xAD_BE_EF).
    // Expected result: 0x78AD_BEEF
    cpu.gpr[9] = 0x8000_1000;
    cpu.gpr[8] = 0xDEAD_BEEF;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0x89280000u32.to_le_bytes()); // LWL $r8, 0($r9)
    cpu.step(&mut bus);
    cpu.step(&mut bus); // NOP for load delay
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 0x78AD_BEEF, "LWL offset 0 failed");

    // Offset 1: LWL $r8, 1($r9) where $r9 = 0x8000_1000 -> addr 0x8000_1001
    // Loads bytes +1 (0x56) and +0 (0x78) into bits 31..16. Bits 15..0 preserved (0xBE_EF).
    // Expected result: 0x5678_BEEF
    cpu.gpr[9] = 0x8000_1001;
    cpu.gpr[8] = 0xDEAD_BEEF;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0x89280000u32.to_le_bytes());
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 0x5678_BEEF, "LWL offset 1 failed");

    // Offset 2: LWL $r8, 2($r9) -> addr 0x8000_1002
    // Loads bytes +2 (0x34), +1 (0x56), +0 (0x78) into bits 31..8. Bits 7..0 preserved (0xEF).
    // Expected result: 0x3456_78EF
    cpu.gpr[9] = 0x8000_1002;
    cpu.gpr[8] = 0xDEAD_BEEF;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0x89280000u32.to_le_bytes());
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 0x3456_78EF, "LWL offset 2 failed");

    // Offset 3: LWL $r8, 3($r9) -> addr 0x8000_1003
    // Loads all 4 bytes +3..+0 into bits 31..0. No bits preserved.
    // Expected result: 0x1234_5678
    cpu.gpr[9] = 0x8000_1003;
    cpu.gpr[8] = 0xDEAD_BEEF;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0x89280000u32.to_le_bytes());
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 0x1234_5678, "LWL offset 3 failed");
}

#[test]
fn test_lwr_all_offsets() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Memory contains 0x12345678 at 0x8000_1000
    // Little-endian bytes: +0: 0x78, +1: 0x56, +2: 0x34, +3: 0x12
    bus.write32(0x8000_1000, 0x1234_5678);

    // Initial register value: 0xDEAD_BEEF (B3=0xDE, B2=0xAD, B1=0xBE, B0=0xEF)

    // Offset 0: LWR $r8, 0($r9) where $r9 = 0x8000_1000
    // Loads all 4 bytes +3..+0 into bits 31..0. No bits preserved.
    // Expected result: 0x1234_5678
    cpu.gpr[9] = 0x8000_1000;
    cpu.gpr[8] = 0xDEAD_BEEF;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0x99280000u32.to_le_bytes()); // LWR $r8, 0($r9)
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 0x1234_5678, "LWR offset 0 failed");

    // Offset 1: LWR $r8, 1($r9) -> addr 0x8000_1001
    // Loads bytes +3 (0x12), +2 (0x34), +1 (0x56) into bits 23..0. MSB preserved (0xDE).
    // Expected result: 0xDE12_3456
    cpu.gpr[9] = 0x8000_1001;
    cpu.gpr[8] = 0xDEAD_BEEF;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0x99280000u32.to_le_bytes());
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 0xDE12_3456, "LWR offset 1 failed");

    // Offset 2: LWR $r8, 2($r9) -> addr 0x8000_1002
    // Loads bytes +3 (0x12), +2 (0x34) into bits 15..0. Top 16 bits preserved (0xDEAD).
    // Expected result: 0xDEAD_1234
    cpu.gpr[9] = 0x8000_1002;
    cpu.gpr[8] = 0xDEAD_BEEF;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0x99280000u32.to_le_bytes());
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 0xDEAD_1234, "LWR offset 2 failed");

    // Offset 3: LWR $r8, 3($r9) -> addr 0x8000_1003
    // Loads byte +3 (0x12) into LSB (bits 7..0). Top 24 bits preserved (0xDEADBE).
    // Expected result: 0xDEAD_BE12
    cpu.gpr[9] = 0x8000_1003;
    cpu.gpr[8] = 0xDEAD_BEEF;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0x99280000u32.to_le_bytes());
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 0xDEAD_BE12, "LWR offset 3 failed");
}

#[test]
fn test_swl_all_offsets() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Source register $r8 = 0x12345678 (V3=0x12, V2=0x34, V1=0x56, V0=0x78)
    // Memory initialized to 0xAAAA_AAAA at 0x8000_1000 (bytes: +0:0xAA, +1:0xAA, +2:0xAA, +3:0xAA)

    // Offset 0: SWL $r8, 0($r9) where $r9 = 0x8000_1000
    // Stores V3 (0x12) into memory at +0. Memory becomes [0x12, 0xAA, 0xAA, 0xAA] -> 0xAAAA_AA12
    bus.write32(0x8000_1000, 0xAAAA_AAAA);
    cpu.gpr[9] = 0x8000_1000;
    cpu.gpr[8] = 0x1234_5678;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0xA9280000u32.to_le_bytes()); // SWL $r8, 0($r9)
    cpu.step(&mut bus);
    assert_eq!(bus.read32(0x8000_1000), 0xAAAA_AA12, "SWL offset 0 failed");

    // Offset 1: SWL $r8, 1($r9) -> addr 0x8000_1001
    // Stores V3 (0x12) into +1, V2 (0x34) into +0. Memory becomes [0x34, 0x12, 0xAA, 0xAA] -> 0xAAAA_1234
    bus.write32(0x8000_1000, 0xAAAA_AAAA);
    cpu.gpr[9] = 0x8000_1001;
    cpu.gpr[8] = 0x1234_5678;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0xA9280000u32.to_le_bytes());
    cpu.step(&mut bus);
    assert_eq!(bus.read32(0x8000_1000), 0xAAAA_1234, "SWL offset 1 failed");

    // Offset 2: SWL $r8, 2($r9) -> addr 0x8000_1002
    // Stores V3 (0x12) into +2, V2 (0x34) into +1, V1 (0x56) into +0. Memory: [0x56, 0x34, 0x12, 0xAA] -> 0xAA12_3456
    bus.write32(0x8000_1000, 0xAAAA_AAAA);
    cpu.gpr[9] = 0x8000_1002;
    cpu.gpr[8] = 0x1234_5678;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0xA9280000u32.to_le_bytes());
    cpu.step(&mut bus);
    assert_eq!(bus.read32(0x8000_1000), 0xAA12_3456, "SWL offset 2 failed");

    // Offset 3: SWL $r8, 3($r9) -> addr 0x8000_1003
    // Stores all 4 bytes V3..V0 into +3..+0. Memory becomes 0x1234_5678
    bus.write32(0x8000_1000, 0xAAAA_AAAA);
    cpu.gpr[9] = 0x8000_1003;
    cpu.gpr[8] = 0x1234_5678;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0xA9280000u32.to_le_bytes());
    cpu.step(&mut bus);
    assert_eq!(bus.read32(0x8000_1000), 0x1234_5678, "SWL offset 3 failed");
}

#[test]
fn test_swr_all_offsets() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Source register $r8 = 0x12345678 (V3=0x12, V2=0x34, V1=0x56, V0=0x78)
    // Memory initialized to 0xAAAA_AAAA at 0x8000_1000

    // Offset 0: SWR $r8, 0($r9) where $r9 = 0x8000_1000
    // Stores all 4 bytes V3..V0 into +3..+0. Memory becomes 0x1234_5678
    bus.write32(0x8000_1000, 0xAAAA_AAAA);
    cpu.gpr[9] = 0x8000_1000;
    cpu.gpr[8] = 0x1234_5678;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0xB9280000u32.to_le_bytes()); // SWR $r8, 0($r9)
    cpu.step(&mut bus);
    assert_eq!(bus.read32(0x8000_1000), 0x1234_5678, "SWR offset 0 failed");

    // Offset 1: SWR $r8, 1($r9) -> addr 0x8000_1001
    // Stores V0 (0x78) into +1, V1 (0x56) into +2, V2 (0x34) into +3. +0 remains 0xAA.
    // Memory becomes [0xAA, 0x78, 0x56, 0x34] -> 0x3456_78AA
    bus.write32(0x8000_1000, 0xAAAA_AAAA);
    cpu.gpr[9] = 0x8000_1001;
    cpu.gpr[8] = 0x1234_5678;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0xB9280000u32.to_le_bytes());
    cpu.step(&mut bus);
    assert_eq!(bus.read32(0x8000_1000), 0x3456_78AA, "SWR offset 1 failed");

    // Offset 2: SWR $r8, 2($r9) -> addr 0x8000_1002
    // Stores V0 (0x78) into +2, V1 (0x56) into +3. +0, +1 remain 0xAA.
    // Memory becomes [0xAA, 0xAA, 0x78, 0x56] -> 0x5678_AAAA
    bus.write32(0x8000_1000, 0xAAAA_AAAA);
    cpu.gpr[9] = 0x8000_1002;
    cpu.gpr[8] = 0x1234_5678;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0xB9280000u32.to_le_bytes());
    cpu.step(&mut bus);
    assert_eq!(bus.read32(0x8000_1000), 0x5678_AAAA, "SWR offset 2 failed");

    // Offset 3: SWR $r8, 3($r9) -> addr 0x8000_1003
    // Stores V0 (0x78) into +3. +0, +1, +2 remain 0xAA.
    // Memory becomes [0xAA, 0xAA, 0xAA, 0x78] -> 0x78AA_AAAA
    bus.write32(0x8000_1000, 0xAAAA_AAAA);
    cpu.gpr[9] = 0x8000_1003;
    cpu.gpr[8] = 0x1234_5678;
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0xB9280000u32.to_le_bytes());
    cpu.step(&mut bus);
    assert_eq!(bus.read32(0x8000_1000), 0x78AA_AAAA, "SWR offset 3 failed");
}

#[test]
fn test_interleaved_lwl_lwr_same_register_with_delay() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Memory contains 0xA1B2C3D4 at 0x8000_1000 and 0xE5F61728 at 0x8000_1004
    bus.write32(0x8000_1000, 0xA1B2_C3D4);
    bus.write32(0x8000_1004, 0xE5F6_1728);

    // Read 4 unaligned bytes starting at 0x8000_1002 (bytes 0xC3, 0xA1, 0x28, 0x17)
    // We start with arbitrary dirty register content in $r8
    cpu.gpr[8] = 0xCAFE_BABE;
    cpu.gpr[9] = 0x8000_1002;
    cpu.pc = 0x8000_0000;

    // LWL $r8, 3($r9) => addr 0x8000_1005 (word 0x8000_1004, offset 1)
    // LWR $r8, 0($r9) => addr 0x8000_1002 (word 0x8000_1000, offset 2)
    let code: [u32; 6] = [
        0x89280003, // LWL $r8, 3($r9)
        0x00000000, // NOP (delay slot)
        0x00000000, // NOP (commit LWL)
        0x99280000, // LWR $r8, 0($r9)
        0x00000000, // NOP (delay slot)
        0x00000000, // NOP (commit LWR)
    ];

    for (i, op) in code.iter().enumerate() {
        let addr = 0x8000_0000 + (i as u32 * 4);
        bus.load_code(addr, &op.to_le_bytes());
    }

    for _ in 0..6 {
        cpu.step(&mut bus);
    }

    // Bytes starting at 0x8000_1002:
    // 0x8000_1002 (+2 of word 0x8000_1000 0xA1B2_C3D4): 0xB2
    // 0x8000_1003 (+3 of word 0x8000_1000 0xA1B2_C3D4): 0xA1
    // 0x8000_1004 (+0 of word 0x8000_1004 0xE5F6_1728): 0x28
    // 0x8000_1005 (+1 of word 0x8000_1004 0xE5F6_1728): 0x17
    // Full 32-bit word assembled in little-endian: 0x1728_A1B2
    assert_eq!(
        cpu.gpr[8], 0x1728_A1B2,
        "Interleaved LWL + LWR across 4-byte boundary failed"
    );
}

#[test]
fn test_interleaved_lwl_lwr_back_to_back_same_register() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    bus.write32(0x8000_1000, 0x1234_5678);

    // Initial dirty register value
    cpu.gpr[8] = 0xDEAD_BEEF;
    cpu.gpr[9] = 0x8000_1001; // offset 1
    cpu.pc = 0x8000_0000;

    // LWL $r8, 0($r9) immediately followed by LWR $r8, 0($r9) WITHOUT NOP
    let code: [u32; 4] = [
        0x89280000, // LWL $r8, 0($r9) at offset 1 -> LWL loads 0x5678 into top 16 bits
        0x99280000, // LWR $r8, 0($r9) at offset 1 -> LWR loads 0x123456 into bottom 24 bits
        0x00000000, // NOP
        0x00000000, // NOP
    ];

    for (i, op) in code.iter().enumerate() {
        let addr = 0x8000_0000 + (i as u32 * 4);
        bus.load_code(addr, &op.to_le_bytes());
    }

    for _ in 0..4 {
        cpu.step(&mut bus);
    }

    // Combined LWL + LWR at offset 1 should assemble full 32-bit word 0x5612_3456
    // Top 8 bits from LWL: 0x56
    // Lower 24 bits from LWR: 0x123456
    // Total: 0x5612_3456
    assert_eq!(
        cpu.gpr[8], 0x5612_3456,
        "Back-to-back LWL + LWR without NOP into same register failed"
    );
}

#[test]
fn test_delay_slot_syscall_exception() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0000;
    cpu.gpr[1] = 5;
    cpu.gpr[2] = 5;

    // 0x8000_0000: BEQ $r1, $r2, 16 (0x10220010) -> target 0x8000_0044
    // 0x8000_0004: SYSCALL (0x0000000C) (in branch delay slot!)
    bus.load_code(0x8000_0000, &0x10220010u32.to_le_bytes()); // BEQ $r1, $r2, 16
    bus.load_code(0x8000_0004, &0x0000000Cu32.to_le_bytes()); // SYSCALL

    cpu.step(&mut bus); // Execute BEQ (schedules next_in_delay_slot = true)
    cpu.step(&mut bus); // Execute SYSCALL inside delay slot

    // Verify COP0 exception state:
    // Cause.BD (bit 31) must be 1
    assert_eq!(
        cpu.cop0.cause & 0x8000_0000,
        0x8000_0000,
        "Cause.BD must be 1 when exception is triggered inside branch delay slot"
    );
    // Cause.ExcCode (bits 6..2) must be 8 (Syscall)
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::Syscall as u32,
        "Cause.ExcCode must be Syscall (8)"
    );
    // EPC must point to the BRANCH instruction (0x8000_0000), NOT the delay slot (0x8000_0004)
    assert_eq!(
        cpu.cop0.epc, 0x8000_0000,
        "EPC must equal PC - 4 (pointing to branch instruction) when exception occurs in delay slot"
    );
    // PC should vector to boot exception vector (BEV=1 by default) 0xBFC0_0180
    assert_eq!(cpu.pc, 0xBFC0_0180, "PC must vector to exception vector");
}

#[test]
fn test_delay_slot_break_exception() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0010;

    // 0x8000_0010: J 0x8000_0080 -> 0x08000020
    // 0x8000_0014: BREAK -> 0x0000000D (in branch delay slot)
    bus.load_code(0x8000_0010, &0x08000020u32.to_le_bytes()); // J 0x8000_0080
    bus.load_code(0x8000_0014, &0x0000000Du32.to_le_bytes()); // BREAK

    cpu.step(&mut bus); // Execute J
    cpu.step(&mut bus); // Execute BREAK in delay slot

    assert_eq!(
        cpu.cop0.cause & 0x8000_0000,
        0x8000_0000,
        "Cause.BD must be 1"
    );
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::Break as u32,
        "Cause.ExcCode must be Break (9)"
    );
    assert_eq!(
        cpu.cop0.epc, 0x8000_0010,
        "EPC must point to jump instruction at 0x8000_0010"
    );
}

#[test]
fn test_delay_slot_overflow_exception() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0020;
    cpu.gpr[1] = 0x7FFF_FFFF;
    cpu.gpr[2] = 1;
    cpu.gpr[31] = 0x8000_0090;

    // 0x8000_0020: JR $r31 -> 0x03E00008
    // 0x8000_0024: ADD $r3, $r1, $r2 -> 0x00221820 (overflows 0x7FFFFFFF + 1 in delay slot!)
    bus.load_code(0x8000_0020, &0x03E00008u32.to_le_bytes()); // JR $r31
    bus.load_code(0x8000_0024, &0x00221820u32.to_le_bytes()); // ADD $r3, $r1, $r2

    cpu.step(&mut bus); // Execute JR
    cpu.step(&mut bus); // Execute ADD (overflow)

    assert_eq!(
        cpu.cop0.cause & 0x8000_0000,
        0x8000_0000,
        "Cause.BD must be 1"
    );
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::Overflow as u32,
        "Cause.ExcCode must be Overflow (12)"
    );
    assert_eq!(
        cpu.cop0.epc, 0x8000_0020,
        "EPC must point to JR instruction at 0x8000_0020"
    );
}

#[test]
fn test_delay_slot_unaligned_load_exception() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0030;
    cpu.gpr[9] = 0x8000_1001; // Unaligned address

    // 0x8000_0030: J 0x8000_0090 -> 0x08000024
    // 0x8000_0034: LW $r8, 0($r9) -> 0x8D280000 (unaligned load in delay slot!)
    bus.load_code(0x8000_0030, &0x08000024u32.to_le_bytes()); // J 0x8000_0090
    bus.load_code(0x8000_0034, &0x8D280000u32.to_le_bytes()); // LW $r8, 0($r9)

    cpu.step(&mut bus); // Execute J
    cpu.step(&mut bus); // Execute LW (unaligned load exception)

    assert_eq!(
        cpu.cop0.cause & 0x8000_0000,
        0x8000_0000,
        "Cause.BD must be 1"
    );
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::AddressErrorLoad as u32,
        "Cause.ExcCode must be AddressErrorLoad (4)"
    );
    assert_eq!(
        cpu.cop0.badvaddr, 0x8000_1001,
        "BadVAddr must contain unaligned load address"
    );
    assert_eq!(
        cpu.cop0.epc, 0x8000_0030,
        "EPC must point to J instruction at 0x8000_0030"
    );
}

#[test]
fn test_delay_slot_unaligned_store_exception() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0040;
    cpu.gpr[1] = 1;
    cpu.gpr[2] = 2;
    cpu.gpr[9] = 0x8000_1002; // Unaligned store address for SW

    // 0x8000_0040: BNE $r1, $r2, 16 -> 0x14220010
    // 0x8000_0044: SW $r8, 0($r9) -> 0xAD280000 (unaligned store in delay slot!)
    bus.load_code(0x8000_0040, &0x14220010u32.to_le_bytes()); // BNE $r1, $r2, 16
    bus.load_code(0x8000_0044, &0xAD280000u32.to_le_bytes()); // SW $r8, 0($r9)

    cpu.step(&mut bus); // Execute BNE
    cpu.step(&mut bus); // Execute SW (unaligned store exception)

    assert_eq!(
        cpu.cop0.cause & 0x8000_0000,
        0x8000_0000,
        "Cause.BD must be 1"
    );
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::AddressErrorStore as u32,
        "Cause.ExcCode must be AddressErrorStore (5)"
    );
    assert_eq!(
        cpu.cop0.badvaddr, 0x8000_1002,
        "BadVAddr must contain unaligned store address"
    );
    assert_eq!(
        cpu.cop0.epc, 0x8000_0040,
        "EPC must point to BNE instruction at 0x8000_0040"
    );
}

#[test]
fn test_rfe_3level_stack_restoration() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Initial Status bits:
    // Bit 22 = BEV (1)
    // Bits 5..0: KUo=1, IEo=1, KUp=0, IEp=1, KUc=0, IEc=1 -> 0b110101 (0x35)
    cpu.cop0.status = 0x0040_0035;

    // RFE opcode: 0x42000010
    cpu.pc = 0x8000_0000;
    bus.load_code(0x8000_0000, &0x42000010u32.to_le_bytes());

    cpu.step(&mut bus); // Execute RFE

    // RFE pops mode stack:
    // KUc, IEc <= KUp, IEp (0, 1) -> bits 1..0 become 01
    // KUp, IEp <= KUo, IEo (1, 1) -> bits 3..2 become 11
    // KUo, IEo remain 1, 1 -> bits 5..4 stay 11
    // Expected bits 5..0: 0b111101 (0x3D)
    // Total Status = 0x0040_003D
    assert_eq!(
        cpu.cop0.status, 0x0040_003D,
        "RFE 3-level stack restoration failed to pop mode bits correctly"
    );
}

#[test]
fn test_rfe_nested_exceptions() {
    let mut cpu = Cpu::new();

    // Initial mode: User mode (KUc=1), Interrupts Enabled (IEc=1)
    // Status bits 5..0 = 0b000003 (KUc=1, IEc=1)
    cpu.cop0.status = 0x0040_0003;

    // Trigger exception 1
    cpu.cop0
        .trigger_exception(ExceptionCode::Syscall, false, 0x8000_0100);
    // After exception 1:
    // KUc, IEc <= 0, 0
    // KUp, IEp <= 1, 1
    // KUo, IEo <= 0, 0
    // Bits 5..0 = 0b001100 (0x0C)
    assert_eq!(cpu.cop0.status & 0x3F, 0x0C, "Exception 1 push failed");

    // Trigger exception 2 (nested exception inside kernel exception handler)
    cpu.cop0
        .trigger_exception(ExceptionCode::Break, false, 0x8000_0200);
    // After exception 2:
    // KUc, IEc <= 0, 0
    // KUp, IEp <= 0, 0
    // KUo, IEo <= 1, 1
    // Bits 5..0 = 0b110000 (0x30)
    assert_eq!(cpu.cop0.status & 0x3F, 0x30, "Exception 2 push failed");

    // Execute first RFE (returning from exception 2 handler to exception 1 handler)
    cpu.cop0.rfe();
    // After RFE 1:
    // KUc, IEc <= KUp, IEp (0, 0)
    // KUp, IEp <= KUo, IEo (1, 1)
    // KUo, IEo remain (1, 1)
    // Bits 5..0 = 0b111100 (0x3C)
    assert_eq!(
        cpu.cop0.status & 0x3F,
        0x3C,
        "First RFE pop from nested exception failed"
    );

    // Execute second RFE (returning from exception 1 handler to original user mode)
    cpu.cop0.rfe();
    // After RFE 2:
    // KUc, IEc <= KUp, IEp (1, 1) -> User mode, IE=1 restored!
    // KUp, IEp <= KUo, IEo (1, 1)
    // KUo, IEo remain (1, 1)
    // Bits 5..0 = 0b111111 (0x3F)
    assert_eq!(
        cpu.cop0.status & 0x03,
        0x03,
        "Second RFE pop failed to restore original User mode (KUc=1, IEc=1)"
    );
}

#[test]
fn test_unaligned_pc_fetch_exception() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Set PC to unaligned address 0x8000_0001
    cpu.pc = 0x8000_0001;
    cpu.cop0.status = 0x0040_0000; // BEV = 1

    cpu.step(&mut bus);

    assert_eq!(
        cpu.cop0.badvaddr, 0x8000_0001,
        "BadVAddr must equal unaligned PC"
    );
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::AddressErrorLoad as u32,
        "ExcCode must be AddressErrorLoad"
    );
    assert_eq!(cpu.cop0.epc, 0x8000_0001, "EPC must equal unaligned PC");
    assert_eq!(cpu.cop0.cause & 0x8000_0000, 0, "Cause.BD must be 0");
    assert_eq!(cpu.pc, 0xBFC0_0180, "PC must vector to 0xBFC0_0180");
}

#[test]
fn test_unaligned_lh_lhu_sh_exceptions() {
    let mut bus = MockBus::new();

    // 1. LH unaligned
    let mut cpu = Cpu::new();
    cpu.pc = 0x8000_0000;
    cpu.gpr[9] = 0x8000_1001;
    bus.load_code(0x8000_0000, &0x85280000u32.to_le_bytes()); // LH $r8, 0($r9)
    cpu.step(&mut bus);
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::AddressErrorLoad as u32
    );
    assert_eq!(cpu.cop0.badvaddr, 0x8000_1001);

    // 2. LHU unaligned
    let mut cpu = Cpu::new();
    cpu.pc = 0x8000_0000;
    cpu.gpr[9] = 0x8000_1003;
    bus.load_code(0x8000_0000, &0x95280000u32.to_le_bytes()); // LHU $r8, 0($r9)
    cpu.step(&mut bus);
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::AddressErrorLoad as u32
    );
    assert_eq!(cpu.cop0.badvaddr, 0x8000_1003);

    // 3. SH unaligned
    let mut cpu = Cpu::new();
    cpu.pc = 0x8000_0000;
    cpu.gpr[9] = 0x8000_1001;
    bus.load_code(0x8000_0000, &0xA5280000u32.to_le_bytes()); // SH $r8, 0($r9)
    cpu.step(&mut bus);
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::AddressErrorStore as u32
    );
    assert_eq!(cpu.cop0.badvaddr, 0x8000_1001);
}

#[test]
fn test_direct_unaligned_sw_exception() {
    let mut bus = MockBus::new();

    for offset in [1u32, 2, 3] {
        let mut cpu = Cpu::new();
        cpu.pc = 0x8000_0000;
        let unaligned_addr = 0x8000_1000 + offset;
        cpu.gpr[9] = unaligned_addr;
        bus.load_code(0x8000_0000, &0xAD280000u32.to_le_bytes()); // SW $r8, 0($r9)
        cpu.step(&mut bus);

        assert_eq!(
            cpu.cop0.cause_exc_code(),
            ExceptionCode::AddressErrorStore as u32,
            "SW at offset {offset} must trigger AddressErrorStore"
        );
        assert_eq!(
            cpu.cop0.badvaddr, unaligned_addr,
            "BadVAddr must match unaligned store address at offset {offset}"
        );
        assert_eq!(cpu.cop0.epc, 0x8000_0000);
        assert_eq!(cpu.cop0.cause & 0x8000_0000, 0);
    }
}

#[test]
fn test_jump_to_unaligned_pc_fetch_exception() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0000;
    cpu.gpr[31] = 0x8000_0005; // Unaligned jump target address

    // 0x8000_0000: JR $r31 (0x03E00008)
    // 0x8000_0004: NOP      (0x00000000)
    bus.load_code(0x8000_0000, &0x03E00008u32.to_le_bytes());
    bus.load_code(0x8000_0004, &0x00000000u32.to_le_bytes());

    cpu.step(&mut bus); // Execute JR $r31
    cpu.step(&mut bus); // Execute NOP in delay slot
    cpu.step(&mut bus); // Attempt PC fetch at 0x8000_0005 -> Exception!

    assert_eq!(cpu.cop0.badvaddr, 0x8000_0005);
    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::AddressErrorLoad as u32
    );
    assert_eq!(cpu.cop0.epc, 0x8000_0005);
    assert_eq!(cpu.cop0.cause & 0x8000_0000, 0);
    assert_eq!(cpu.pc, 0xBFC0_0180);
}
