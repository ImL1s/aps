//! Tier 1: Comprehensive Unit Tests (Instruction decoding, ALU operations, register updates using MockBus)

use ps_core::bus::map::mask_address;
use ps_core::bus::mock_bus::MockBus;
use ps_core::bus::Bus;
use ps_core::cpu::{Cpu, ExceptionCode};

#[test]
fn test_tier1_cpu_reset_state() {
    let mut _bus = MockBus::new();
    let cpu = Cpu::new();
    assert_eq!(
        cpu.pc, 0xBFC0_0000,
        "Reset PC must be BIOS vector 0xBFC0_0000"
    );
    assert_eq!(cpu.gpr[0], 0, "Register $zero must always be 0");
}

#[test]
fn test_tier1_r0_invariant() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0000;
    // ADDIU $r0, $r0, 42 => 0x2400002A
    let inst_bytes = 0x2400002Au32.to_le_bytes();
    bus.load_code(0x8000_0000, &inst_bytes);

    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[0], 0, "Register $zero must remain 0 after write");
}

#[test]
fn test_tier1_addu_instruction() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.gpr[9] = 15;
    cpu.gpr[10] = 27;
    cpu.pc = 0x8000_0000;

    // ADDU $t0 (r8), $t1 (r9), $t2 (r10) => 0x012A4021
    let inst_bytes: [u8; 4] = 0x012A4021u32.to_le_bytes();
    bus.load_code(0x8000_0000, &inst_bytes);

    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 42, "ADDU must set $t0 to 42");
    assert_eq!(cpu.gpr[0], 0, "$zero must remain 0");
}

#[test]
fn test_tier1_alu_instructions() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.gpr[9] = 0b1100;
    cpu.gpr[10] = 0b1010;
    cpu.pc = 0x8000_0000;

    // AND $t0, $t1, $t2 => 0x012A4024
    // OR  $t3, $t1, $t2 => 0x012A5825
    // XOR $t4, $t1, $t2 => 0x012A6026
    // NOR $t5, $t1, $t2 => 0x012A6827
    let code: [u32; 4] = [
        0x012A4024, // AND  $r8, $r9, $r10 => 0b1000 = 8
        0x012A5825, // OR   $r11, $r9, $r10 => 0b1110 = 14
        0x012A6026, // XOR  $r12, $r9, $r10 => 0b0110 = 6
        0x012A6827, // NOR  $r13, $r9, $r10 => !(0b1110) = 0xFFFFFFF1
    ];
    for (i, op) in code.iter().enumerate() {
        bus.load_code(0x8000_0000 + (i as u32 * 4), &op.to_le_bytes());
    }

    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 8, "AND failed");
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[11], 14, "OR failed");
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[12], 6, "XOR failed");
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[13], 0xFFFFFFF1, "NOR failed");
}

#[test]
fn test_tier1_shift_instructions() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.gpr[9] = 0x8000_0000;
    cpu.pc = 0x8000_0000;

    // SRA $t0 (r8), $t1 (r9), 2 => 0x00094083 -> 0xE000_0000
    // SRL $t2 (r10), $t1 (r9), 2 => 0x00095082 -> 0x2000_0000
    // SLL $t3 (r11), $t2 (r10), 2 => 0x000A5880 -> 0x8000_0000
    let code: [u32; 3] = [
        0x00094083, // SRA $r8, $r9, 2
        0x00095082, // SRL $r10, $r9, 2
        0x000A5880, // SLL $r11, $r10, 2
    ];
    for (i, op) in code.iter().enumerate() {
        bus.load_code(0x8000_0000 + (i as u32 * 4), &op.to_le_bytes());
    }

    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 0xE000_0000, "SRA arithmetic shift failed");
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[10], 0x2000_0000, "SRL logical shift failed");
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[11], 0x8000_0000, "SLL logical shift failed");
}

#[test]
fn test_tier1_mult_div_instructions() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.gpr[9] = 20;
    cpu.gpr[10] = 3;
    cpu.pc = 0x8000_0000;

    // DIV $r9, $r10 => 0x012A001A (HI = 2, LO = 6)
    // MFLO $r8 => 0x00004012
    // MFHI $r11 => 0x00005810
    let code: [u32; 3] = [
        0x012A001A, // DIV $r9, $r10
        0x00004012, // MFLO $r8
        0x00005810, // MFHI $r11
    ];
    for (i, op) in code.iter().enumerate() {
        bus.load_code(0x8000_0000 + (i as u32 * 4), &op.to_le_bytes());
    }

    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[8], 6, "DIV quotient in LO failed");
    cpu.step(&mut bus);
    assert_eq!(cpu.gpr[11], 2, "DIV remainder in HI failed");
}

#[test]
fn test_tier1_branch_delay_slot() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0000;
    cpu.next_pc = 0x8000_0004;
    // BEQ $r0, $r0, 0x0001 followed by ADDU $r8, $r9, $r10
    let code: [u8; 8] = [
        0x01, 0x00, 0x00,
        0x10, // BEQ $r0, $r0, 0x0001 (target = 0x8000_0004 + 4 = 0x8000_0008)
        0x21, 0x40, 0x2A, 0x01, // ADDU $r8, $r9, $r10
    ];
    bus.load_code(0x8000_0000, &code);

    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8000_0004, "PC must be in delay slot");
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8000_0008, "PC must reach branch target");
}

#[test]
fn test_tier1_load_delay_slot_pipeline() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    bus.write32(0x8000_1000, 0xDEAD_BEEF);

    cpu.gpr[9] = 0x8000_1000; // $t1 = address
    cpu.gpr[8] = 0x0000_0000; // $t0 initial = 0
    cpu.pc = 0x8000_0000;

    // LW $t0 (r8), 0($t1) => 0x8D280000
    // ADDIU $t2 (r10), $t0 (r8), 5 => 0x210A0005 (Reads old $t0 value 0!)
    // NOP => 0x00000000 (At start of this cycle, $t0 becomes 0xDEAD_BEEF)
    let code: [u32; 3] = [
        0x8D280000, // LW $r8, 0($r9)
        0x210A0005, // ADDIU $r10, $r8, 5
        0x00000000, // NOP
    ];
    for (i, op) in code.iter().enumerate() {
        bus.load_code(0x8000_0000 + (i as u32 * 4), &op.to_le_bytes());
    }

    // Cycle 1: Execute LW (Schedules load for $r8)
    cpu.step(&mut bus);
    assert_eq!(
        cpu.gpr[8], 0,
        "LW load delay slot: $r8 must NOT update immediately at N"
    );

    // Cycle 2: Execute ADDIU (Reads old $r8 = 0, calculates 0 + 5 = 5 into $r10)
    cpu.step(&mut bus);
    assert_eq!(
        cpu.gpr[10], 5,
        "ADDIU in load delay slot must read old $r8 value"
    );
    assert_eq!(
        cpu.gpr[8], 0,
        "Load delay slot: $r8 still pending at N+1 execution"
    );

    // Cycle 3: Execute NOP (Load applied at start of cycle N+2)
    cpu.step(&mut bus);
    assert_eq!(
        cpu.gpr[8], 0xDEAD_BEEF,
        "Load delay slot: $r8 must update to loaded value at N+2"
    );
}

#[test]
fn test_tier1_unaligned_loads_stores() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // Memory contains 0x12345678 at 0x8000_1000
    bus.write32(0x8000_1000, 0x1234_5678);

    // Test 1: LWL at offset 1
    cpu.gpr[9] = 0x8000_1001; // Unaligned address (offset 1)
    cpu.gpr[8] = 0x1122_3344; // Initial value with lower 16 bits 0x3344
    cpu.pc = 0x8000_0000;

    // LWL $t0 (r8), 0($t1) => 0x89280000
    let inst_bytes = 0x89280000u32.to_le_bytes();
    bus.load_code(0x8000_0000, &inst_bytes);

    cpu.step(&mut bus); // Execute LWL (schedules pending load)
    cpu.step(&mut bus); // Execute NOP
    cpu.step(&mut bus); // Execute NOP (commits load)

    // Offset 1 -> B1=0x56, B0=0x78 loaded into top 16 bits (0x5678), lower 16 bits preserved (0x3344)
    assert_eq!(
        cpu.gpr[8], 0x5678_3344,
        "LWL exact 32-bit register update failed"
    );

    // Test 2: LWR at offset 1
    cpu.gpr[9] = 0x8000_1001;
    cpu.gpr[8] = 0x1122_3344; // Initial value with top 8 bits 0x11
    cpu.pc = 0x8000_0000;

    // LWR $t0 (r8), 0($t1) => 0x99280000
    let lwr_bytes = 0x99280000u32.to_le_bytes();
    bus.load_code(0x8000_0000, &lwr_bytes);

    cpu.step(&mut bus);
    cpu.step(&mut bus);
    cpu.step(&mut bus);

    // Offset 1 -> B3=0x12, B2=0x34, B1=0x56 loaded into lower 24 bits (0x123456), top 8 bits preserved (0x11)
    assert_eq!(
        cpu.gpr[8], 0x1112_3456,
        "LWR exact 32-bit register update failed"
    );

    // Test 3: Combined LWL + LWR loading full unaligned 32-bit word across 0x8000_1001..0x8000_1004
    bus.write32(0x8000_1004, 0x9ABC_DEF0); // bytes [0xF0, 0xE0, 0xBC, 0x9A] at 0x8000_1004
    cpu.gpr[9] = 0x8000_1001;
    cpu.gpr[8] = 0x0000_0000;
    cpu.pc = 0x8000_0000;

    // LWL $r8, 3($r9) => address 0x8000_1004 (offset 0): 0x89280003
    // LWR $r8, 0($r9) => address 0x8000_1001 (offset 1): 0x99280000
    let code: [u32; 6] = [
        0x89280003, // LWL $r8, 3($r9)
        0x00000000, // NOP (load delay slot for LWL)
        0x00000000, // NOP (LWL commits to gpr[8])
        0x99280000, // LWR $r8, 0($r9)
        0x00000000, // NOP (load delay slot for LWR)
        0x00000000, // NOP (LWR commits to gpr[8])
    ];
    for (i, op) in code.iter().enumerate() {
        bus.load_code(0x8000_0000 + (i as u32 * 4), &op.to_le_bytes());
    }

    for _ in 0..6 {
        cpu.step(&mut bus);
    }

    // Word at 0x8000_1001 consists of bytes 0x22, 0x33, 0x44, 0xF0 -> Little Endian 0xF012_3456
    assert_eq!(
        cpu.gpr[8], 0xF012_3456,
        "Combined LWL+LWR unaligned load failed"
    );
}

#[test]
fn test_tier1_signed_overflow_exceptions() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    // 1. ADD Signed Overflow Exception
    cpu.pc = 0x8000_0000;
    cpu.cop0.status = 0; // BEV = 0 -> Exception Vector 0x8000_0080
    cpu.gpr[9] = 0x7FFF_FFFF; // i32::MAX
    cpu.gpr[10] = 1;
    // ADD $r8, $r9, $r10 => 0x012A4020
    bus.load_code(0x8000_0000, &0x012A4020u32.to_le_bytes());
    cpu.step(&mut bus);

    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::Overflow as u32,
        "ADD overflow must set Cause ExcCode to Overflow (12)"
    );
    assert_eq!(
        cpu.cop0.epc, 0x8000_0000,
        "ADD overflow EPC must record instruction PC"
    );
    assert_eq!(
        cpu.pc, 0x8000_0080,
        "ADD overflow PC must jump to vector 0x8000_0080"
    );

    // 2. SUB Signed Overflow Exception
    let mut cpu = Cpu::new();
    cpu.pc = 0x8000_0000;
    cpu.cop0.status = 0;
    cpu.gpr[9] = 0x8000_0000; // i32::MIN
    cpu.gpr[10] = 1;
    // SUB $r8, $r9, $r10 => 0x012A4022
    bus.load_code(0x8000_0000, &0x012A4022u32.to_le_bytes());
    cpu.step(&mut bus);

    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::Overflow as u32,
        "SUB overflow must set Cause ExcCode to Overflow (12)"
    );
    assert_eq!(
        cpu.cop0.epc, 0x8000_0000,
        "SUB overflow EPC must record instruction PC"
    );
    assert_eq!(
        cpu.pc, 0x8000_0080,
        "SUB overflow PC must jump to vector 0x8000_0080"
    );

    // 3. ADDI Signed Overflow Exception (Normal Vector BEV=0)
    let mut cpu = Cpu::new();
    cpu.pc = 0x8000_0000;
    cpu.cop0.status = 0;
    cpu.gpr[9] = 0x7FFF_FFFF; // i32::MAX
                              // ADDI $r8, $r9, 1 => 0x21280001
    bus.load_code(0x8000_0000, &0x21280001u32.to_le_bytes());
    cpu.step(&mut bus);

    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::Overflow as u32,
        "ADDI overflow must set Cause ExcCode to Overflow (12)"
    );
    assert_eq!(
        cpu.cop0.epc, 0x8000_0000,
        "ADDI overflow EPC must record instruction PC"
    );
    assert_eq!(
        cpu.pc, 0x8000_0080,
        "ADDI overflow PC must jump to normal vector 0x8000_0080"
    );

    // 4. ADDI Signed Overflow Exception (Boot Vector BEV=1)
    let mut cpu = Cpu::new();
    cpu.pc = 0x8000_0000;
    cpu.cop0.status = 0x0040_0000; // BEV = 1
    cpu.gpr[9] = 0x7FFF_FFFF;
    bus.load_code(0x8000_0000, &0x21280001u32.to_le_bytes());
    cpu.step(&mut bus);

    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::Overflow as u32,
        "ADDI overflow BEV=1 must set Cause ExcCode to Overflow (12)"
    );
    assert_eq!(
        cpu.cop0.epc, 0x8000_0000,
        "ADDI overflow BEV=1 EPC must record instruction PC"
    );
    assert_eq!(
        cpu.pc, 0xBFC0_0180,
        "ADDI overflow BEV=1 PC must jump to boot vector 0xBFC0_0180"
    );
}

#[test]
fn test_tier1_cop0_and_exceptions() {
    let mut bus = MockBus::new();
    let mut cpu = Cpu::new();

    cpu.pc = 0x8000_0000;
    cpu.cop0.status = 0; // BEV = 0

    // SYSCALL => 0x0000000C
    let inst_bytes = 0x0000000Cu32.to_le_bytes();
    bus.load_code(0x8000_0000, &inst_bytes);

    cpu.step(&mut bus);

    assert_eq!(
        cpu.cop0.cause_exc_code(),
        ExceptionCode::Syscall as u32,
        "Must trigger Syscall exception"
    );
    assert_eq!(cpu.cop0.epc, 0x8000_0000, "EPC must store faulting PC");
    assert_eq!(
        cpu.pc, 0x8000_0080,
        "PC must jump to normal exception vector 0x8000_0080"
    );
}

#[test]
fn test_tier1_address_segment_masking() {
    assert_eq!(mask_address(0x0000_1000), 0x0000_1000, "KUSEG translation");
    assert_eq!(mask_address(0x8000_1000), 0x0000_1000, "KSEG0 translation");
    assert_eq!(mask_address(0xA000_1000), 0x0000_1000, "KSEG1 translation");
}

#[test]
fn test_tier1_mock_bus_rw() {
    let mut bus = MockBus::new();
    bus.write32(0x8000_1000, 0xDEAD_BEEF);
    assert_eq!(bus.read32(0x8000_1000), 0xDEAD_BEEF);
    assert_eq!(bus.read16(0x8000_1000), 0xBEEF);
    assert_eq!(bus.read8(0x8000_1000), 0xEF);
}
