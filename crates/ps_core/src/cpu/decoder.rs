#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    // ALU R-Type
    Add {
        rs: usize,
        rt: usize,
        rd: usize,
    },
    Addu {
        rs: usize,
        rt: usize,
        rd: usize,
    },
    Sub {
        rs: usize,
        rt: usize,
        rd: usize,
    },
    Subu {
        rs: usize,
        rt: usize,
        rd: usize,
    },
    And {
        rs: usize,
        rt: usize,
        rd: usize,
    },
    Or {
        rs: usize,
        rt: usize,
        rd: usize,
    },
    Xor {
        rs: usize,
        rt: usize,
        rd: usize,
    },
    Nor {
        rs: usize,
        rt: usize,
        rd: usize,
    },
    Slt {
        rs: usize,
        rt: usize,
        rd: usize,
    },
    Sltu {
        rs: usize,
        rt: usize,
        rd: usize,
    },

    // Shifts
    Sll {
        rt: usize,
        rd: usize,
        shamt: u32,
    },
    Srl {
        rt: usize,
        rd: usize,
        shamt: u32,
    },
    Sra {
        rt: usize,
        rd: usize,
        shamt: u32,
    },
    Sllv {
        rs: usize,
        rt: usize,
        rd: usize,
    },
    Srlv {
        rs: usize,
        rt: usize,
        rd: usize,
    },
    Srav {
        rs: usize,
        rt: usize,
        rd: usize,
    },

    // Multiply / Divide
    Mult {
        rs: usize,
        rt: usize,
    },
    Multu {
        rs: usize,
        rt: usize,
    },
    Div {
        rs: usize,
        rt: usize,
    },
    Divu {
        rs: usize,
        rt: usize,
    },
    Mfhi {
        rd: usize,
    },
    Mthi {
        rs: usize,
    },
    Mflo {
        rd: usize,
    },
    Mtlo {
        rs: usize,
    },

    // ALU Immediate
    Addi {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Addiu {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Slti {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Sltiu {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Andi {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Ori {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Xori {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Lui {
        rt: usize,
        imm16: u16,
    },

    // Jumps & Branches
    J {
        target26: u32,
    },
    Jal {
        target26: u32,
    },
    Jr {
        rs: usize,
    },
    Jalr {
        rs: usize,
        rd: usize,
    },
    Beq {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Bne {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Blez {
        rs: usize,
        imm16: u16,
    },
    Bgtz {
        rs: usize,
        imm16: u16,
    },
    Bltz {
        rs: usize,
        imm16: u16,
    },
    Bgez {
        rs: usize,
        imm16: u16,
    },
    Bltzal {
        rs: usize,
        imm16: u16,
    },
    Bgezal {
        rs: usize,
        imm16: u16,
    },

    // Memory Load / Store
    Lb {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Lbu {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Lh {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Lhu {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Lw {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Lwl {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Lwr {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Sb {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Sh {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Sw {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Swl {
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Swr {
        rs: usize,
        rt: usize,
        imm16: u16,
    },

    // COP0 & System
    Mfc0 {
        rt: usize,
        rd: usize,
    },
    Mtc0 {
        rt: usize,
        rd: usize,
    },
    Rfe,
    Syscall,
    Break,

    CopUnusable {
        cop_num: u32,
    },
    Lwc {
        cop_num: u32,
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Ldc {
        cop_num: u32,
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Swc {
        cop_num: u32,
        rs: usize,
        rt: usize,
        imm16: u16,
    },
    Sdc {
        cop_num: u32,
        rs: usize,
        rt: usize,
        imm16: u16,
    },

    Reserved,
}

pub fn decode(word: u32) -> Instruction {
    let opcode = (word >> 26) & 0x3F;
    let rs = ((word >> 21) & 0x1F) as usize;
    let rt = ((word >> 16) & 0x1F) as usize;
    let rd = ((word >> 11) & 0x1F) as usize;
    let shamt = (word >> 6) & 0x1F;
    let funct = word & 0x3F;
    let imm16 = (word & 0xFFFF) as u16;
    let target26 = word & 0x03FF_FFFF;

    match opcode {
        0x00 => match funct {
            0x00 => Instruction::Sll { rt, rd, shamt },
            0x02 => Instruction::Srl { rt, rd, shamt },
            0x03 => Instruction::Sra { rt, rd, shamt },
            0x04 => Instruction::Sllv { rs, rt, rd },
            0x06 => Instruction::Srlv { rs, rt, rd },
            0x07 => Instruction::Srav { rs, rt, rd },
            0x08 => Instruction::Jr { rs },
            0x09 => Instruction::Jalr { rs, rd },
            0x0C => Instruction::Syscall,
            0x0D => Instruction::Break,
            0x10 => Instruction::Mfhi { rd },
            0x11 => Instruction::Mthi { rs },
            0x12 => Instruction::Mflo { rd },
            0x13 => Instruction::Mtlo { rs },
            0x18 => Instruction::Mult { rs, rt },
            0x19 => Instruction::Multu { rs, rt },
            0x1A => Instruction::Div { rs, rt },
            0x1B => Instruction::Divu { rs, rt },
            0x20 => Instruction::Add { rs, rt, rd },
            0x21 => Instruction::Addu { rs, rt, rd },
            0x22 => Instruction::Sub { rs, rt, rd },
            0x23 => Instruction::Subu { rs, rt, rd },
            0x24 => Instruction::And { rs, rt, rd },
            0x25 => Instruction::Or { rs, rt, rd },
            0x26 => Instruction::Xor { rs, rt, rd },
            0x27 => Instruction::Nor { rs, rt, rd },
            0x2A => Instruction::Slt { rs, rt, rd },
            0x2B => Instruction::Sltu { rs, rt, rd },
            _ => Instruction::Reserved,
        },
        0x01 => match rt {
            0x00 => Instruction::Bltz { rs, imm16 },
            0x01 => Instruction::Bgez { rs, imm16 },
            0x10 => Instruction::Bltzal { rs, imm16 },
            0x11 => Instruction::Bgezal { rs, imm16 },
            _ => Instruction::Reserved,
        },
        0x02 => Instruction::J { target26 },
        0x03 => Instruction::Jal { target26 },
        0x04 => Instruction::Beq { rs, rt, imm16 },
        0x05 => Instruction::Bne { rs, rt, imm16 },
        0x06 => Instruction::Blez { rs, imm16 },
        0x07 => Instruction::Bgtz { rs, imm16 },
        0x08 => Instruction::Addi { rs, rt, imm16 },
        0x09 => Instruction::Addiu { rs, rt, imm16 },
        0x0A => Instruction::Slti { rs, rt, imm16 },
        0x0B => Instruction::Sltiu { rs, rt, imm16 },
        0x0C => Instruction::Andi { rs, rt, imm16 },
        0x0D => Instruction::Ori { rs, rt, imm16 },
        0x0E => Instruction::Xori { rs, rt, imm16 },
        0x0F => Instruction::Lui { rt, imm16 },
        0x10 => match rs {
            0x00 => Instruction::Mfc0 { rt, rd },
            0x04 => Instruction::Mtc0 { rt, rd },
            0x10 => match funct {
                0x10 => Instruction::Rfe,
                _ => Instruction::Reserved,
            },
            _ => Instruction::Reserved,
        },
        0x11 | 0x15 | 0x19 | 0x1D => Instruction::CopUnusable { cop_num: 1 },
        0x12 | 0x16 | 0x1A | 0x1E => Instruction::CopUnusable { cop_num: 2 },
        0x13 | 0x17 | 0x1B | 0x1F => Instruction::CopUnusable { cop_num: 3 },
        0x14 | 0x18 | 0x1C => Instruction::CopUnusable { cop_num: 0 },
        0x20 => Instruction::Lb { rs, rt, imm16 },
        0x21 => Instruction::Lh { rs, rt, imm16 },
        0x22 => Instruction::Lwl { rs, rt, imm16 },
        0x23 => Instruction::Lw { rs, rt, imm16 },
        0x24 => Instruction::Lbu { rs, rt, imm16 },
        0x25 => Instruction::Lhu { rs, rt, imm16 },
        0x26 => Instruction::Lwr { rs, rt, imm16 },
        0x28 => Instruction::Sb { rs, rt, imm16 },
        0x29 => Instruction::Sh { rs, rt, imm16 },
        0x2A => Instruction::Swl { rs, rt, imm16 },
        0x2B => Instruction::Sw { rs, rt, imm16 },
        0x2E => Instruction::Swr { rs, rt, imm16 },
        0x30 => Instruction::Lwc {
            cop_num: 0,
            rs,
            rt,
            imm16,
        },
        0x31 => Instruction::Lwc {
            cop_num: 1,
            rs,
            rt,
            imm16,
        },
        0x32 => Instruction::Lwc {
            cop_num: 2,
            rs,
            rt,
            imm16,
        },
        0x33 => Instruction::Lwc {
            cop_num: 3,
            rs,
            rt,
            imm16,
        },
        0x38 => Instruction::Swc {
            cop_num: 0,
            rs,
            rt,
            imm16,
        },
        0x39 => Instruction::Swc {
            cop_num: 1,
            rs,
            rt,
            imm16,
        },
        0x3A => Instruction::Swc {
            cop_num: 2,
            rs,
            rt,
            imm16,
        },
        0x3B => Instruction::Swc {
            cop_num: 3,
            rs,
            rt,
            imm16,
        },
        _ => Instruction::Reserved,
    }
}
