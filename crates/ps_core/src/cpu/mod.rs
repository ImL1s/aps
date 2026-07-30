pub mod cop0;
pub mod decoder;

use crate::bus::Bus;
pub use cop0::{Cop0, Exception, ExceptionCode};
pub use decoder::{decode, Instruction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cpu {
    pub gpr: [u32; 32],
    pub hi: u32,
    pub lo: u32,
    pub pc: u32,
    pub next_pc: u32,

    pub cop0: Cop0,
    pub cop_regs: [[u32; 32]; 4],

    // Delay slot state tracking
    pub in_delay_slot: bool,
    pub next_in_delay_slot: bool,

    // Load delay 2-stage pipeline
    pub load_delay_current_reg: usize,
    pub load_delay_current_val: u32,
    pub load_delay_pending_reg: usize,
    pub load_delay_pending_val: u32,
    pub load_delay_applied_reg: usize,
    pub load_delay_old_val: u32,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            gpr: [0; 32],
            hi: 0,
            lo: 0,
            pc: 0xBFC0_0000, // BIOS reset vector
            next_pc: 0xBFC0_0004,
            cop0: Cop0::new(),
            cop_regs: [[0; 32]; 4],
            in_delay_slot: false,
            next_in_delay_slot: false,
            load_delay_current_reg: 0,
            load_delay_current_val: 0,
            load_delay_pending_reg: 0,
            load_delay_pending_val: 0,
            load_delay_applied_reg: 0,
            load_delay_old_val: 0,
        }
    }

    #[inline(always)]
    pub fn get_gpr_branch(&self, reg: usize) -> u32 {
        if reg != 0 && reg == self.load_delay_applied_reg {
            self.load_delay_old_val
        } else {
            self.gpr[reg]
        }
    }

    #[inline(always)]
    pub fn set_gpr(&mut self, reg: usize, val: u32) {
        if reg != 0 {
            self.gpr[reg] = val;
            if self.load_delay_current_reg == reg {
                self.load_delay_current_reg = 0;
            }
        }
    }

    #[inline(always)]
    pub fn schedule_load(&mut self, reg: usize, val: u32) {
        if reg != 0 {
            self.load_delay_pending_reg = reg;
            self.load_delay_pending_val = val;
            if self.load_delay_current_reg == reg {
                self.load_delay_current_reg = 0;
            }
        }
    }

    #[inline(always)]
    pub fn branch(&mut self, target: u32) {
        self.next_pc = target;
        self.next_in_delay_slot = true;
    }

    pub fn step<B: Bus>(&mut self, bus: &mut B) {
        // Step 1: Apply load delay slot from previous instruction
        if self.load_delay_current_reg != 0 {
            self.load_delay_applied_reg = self.load_delay_current_reg;
            self.load_delay_old_val = self.gpr[self.load_delay_current_reg];
            self.gpr[self.load_delay_current_reg] = self.load_delay_current_val;
        } else {
            self.load_delay_applied_reg = 0;
            self.load_delay_old_val = 0;
        }
        self.gpr[0] = 0; // Maintain r0 = 0 invariant

        // Advance load delay pipeline
        self.load_delay_current_reg = self.load_delay_pending_reg;
        self.load_delay_current_val = self.load_delay_pending_val;
        self.load_delay_pending_reg = 0;
        self.load_delay_pending_val = 0;

        // Step 2: Advance branch delay slot state & program counter
        let current_pc = self.pc;
        self.in_delay_slot = self.next_in_delay_slot;
        self.next_in_delay_slot = false;
        let is_current_delay_slot = self.in_delay_slot;

        // Synchronize next_pc if pc was set directly by external code (e.g. unit tests)
        if !is_current_delay_slot && !self.in_delay_slot && self.next_pc != self.pc.wrapping_add(4)
        {
            self.next_pc = self.pc.wrapping_add(4);
        }

        self.pc = self.next_pc;
        self.next_pc = self.pc.wrapping_add(4);

        // Intercept BIOS TTY vector calls (A0 / B0 / C0 vectors)
        let paddr_pc = current_pc & 0x1FFF_FFFF;
        if paddr_pc == 0x0000_00A0 || paddr_pc == 0x0000_00B0 || paddr_pc == 0x0000_00C0 {
            let fn_code = if [0x3C, 0x3D, 0x3E, 0x3F, 0x17, 0x4B].contains(&self.gpr[2]) {
                self.gpr[2]
            } else if [0x3C, 0x3D, 0x3E, 0x3F, 0x17, 0x4B].contains(&self.gpr[9]) {
                self.gpr[9]
            } else if self.gpr[2] != 0 {
                self.gpr[2]
            } else {
                self.gpr[9]
            };
            if fn_code == 0x3C || fn_code == 0x3D {
                bus.log_tty_char(self.gpr[4] as u8);
            } else if fn_code == 0x3E {
                let mut ptr = self.gpr[4];
                for _ in 0..1024 {
                    let b = bus.read8(ptr);
                    if b == 0 {
                        break;
                    }
                    bus.log_tty_char(b);
                    ptr = ptr.wrapping_add(1);
                }
            } else if fn_code == 0x3F {
                self.handle_bios_printf(bus);
            }
            // Auto-return from BIOS vector call if no BIOS ROM is installed
            if self.gpr[31] != 0 {
                self.pc = self.gpr[31];
                self.next_pc = self.gpr[31].wrapping_add(4);
                return;
            }
        }

        // Step 3: Check instruction alignment exception
        if !current_pc.is_multiple_of(4) {
            self.cop0.badvaddr = current_pc;
            let target = self.cop0.trigger_exception(
                Exception::AddressErrorLoad,
                is_current_delay_slot,
                current_pc,
            );
            self.pc = target;
            self.next_pc = target.wrapping_add(4);
            self.in_delay_slot = false;
            self.next_in_delay_slot = false;
            return;
        }

        // Step 4: Fetch instruction word
        let inst_word = bus.read32(current_pc);
        bus.step(1);

        // Step 5: Decode & Execute
        let inst = decode(inst_word);
        self.execute_inst(bus, inst, is_current_delay_slot, current_pc);
    }

    fn handle_bios_printf<B: Bus>(&mut self, bus: &mut B) {
        let fmt_ptr = self.gpr[4];
        let sp = self.gpr[29];
        let mut arg_idx = 1;

        let gpr = &self.gpr;
        let get_arg = |idx: usize, bus: &mut B| -> u32 {
            match idx {
                1 => gpr[5],
                2 => gpr[6],
                3 => gpr[7],
                _ => bus.read32(sp.wrapping_add((idx as u32) * 4)),
            }
        };

        let mut ptr = fmt_ptr;
        for _ in 0..2048 {
            let b = bus.read8(ptr);
            if b == 0 {
                break;
            }
            ptr = ptr.wrapping_add(1);
            if b != b'%' {
                bus.log_tty_char(b);
                continue;
            }

            // Parse format specifier
            let mut width: usize = 0;
            let mut zero_pad = false;
            let mut next_b = bus.read8(ptr);
            ptr = ptr.wrapping_add(1);

            if next_b == b'0' {
                zero_pad = true;
                next_b = bus.read8(ptr);
                ptr = ptr.wrapping_add(1);
            }

            while next_b.is_ascii_digit() {
                width = width * 10 + (next_b - b'0') as usize;
                next_b = bus.read8(ptr);
                ptr = ptr.wrapping_add(1);
            }

            match next_b {
                b'%' => {
                    bus.log_tty_char(b'%');
                }
                b's' => {
                    let str_ptr = get_arg(arg_idx, bus);
                    arg_idx += 1;
                    let mut s_ptr = str_ptr;
                    for _ in 0..1024 {
                        let sb = bus.read8(s_ptr);
                        if sb == 0 {
                            break;
                        }
                        bus.log_tty_char(sb);
                        s_ptr = s_ptr.wrapping_add(1);
                    }
                }
                b'd' | b'i' => {
                    let val = get_arg(arg_idx, bus) as i32;
                    arg_idx += 1;
                    let s = format!("{val}");
                    let pad = if width > s.len() { width - s.len() } else { 0 };
                    let pad_char = if zero_pad { b'0' } else { b' ' };
                    for _ in 0..pad {
                        bus.log_tty_char(pad_char);
                    }
                    for ch in s.bytes() {
                        bus.log_tty_char(ch);
                    }
                }
                b'u' => {
                    let val = get_arg(arg_idx, bus);
                    arg_idx += 1;
                    let s = format!("{val}");
                    let pad = if width > s.len() { width - s.len() } else { 0 };
                    let pad_char = if zero_pad { b'0' } else { b' ' };
                    for _ in 0..pad {
                        bus.log_tty_char(pad_char);
                    }
                    for ch in s.bytes() {
                        bus.log_tty_char(ch);
                    }
                }
                b'x' => {
                    let val = get_arg(arg_idx, bus);
                    arg_idx += 1;
                    let s = format!("{val:x}");
                    let pad = if width > s.len() { width - s.len() } else { 0 };
                    let pad_char = if zero_pad { b'0' } else { b' ' };
                    for _ in 0..pad {
                        bus.log_tty_char(pad_char);
                    }
                    for ch in s.bytes() {
                        bus.log_tty_char(ch);
                    }
                }
                b'X' => {
                    let val = get_arg(arg_idx, bus);
                    arg_idx += 1;
                    let s = format!("{val:X}");
                    let pad = if width > s.len() { width - s.len() } else { 0 };
                    let pad_char = if zero_pad { b'0' } else { b' ' };
                    for _ in 0..pad {
                        bus.log_tty_char(pad_char);
                    }
                    for ch in s.bytes() {
                        bus.log_tty_char(ch);
                    }
                }
                b'c' => {
                    let val = get_arg(arg_idx, bus) as u8;
                    arg_idx += 1;
                    bus.log_tty_char(val);
                }
                _ => {
                    bus.log_tty_char(b'%');
                    if next_b != 0 {
                        bus.log_tty_char(next_b);
                    }
                }
            }
        }
    }

    fn execute_inst<B: Bus>(
        &mut self,
        bus: &mut B,
        inst: Instruction,
        is_current_delay_slot: bool,
        current_pc: u32,
    ) {
        #[inline(always)]
        fn sign_ext(imm16: u16) -> u32 {
            (imm16 as i16) as i32 as u32
        }

        #[inline(always)]
        fn zero_ext(imm16: u16) -> u32 {
            imm16 as u32
        }

        match inst {
            // ALU R-type
            Instruction::Add { rs, rt, rd } => {
                let (res, overflow) = (self.gpr[rs] as i32).overflowing_add(self.gpr[rt] as i32);
                if overflow {
                    let target = self.cop0.trigger_exception(
                        Exception::Overflow,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    self.set_gpr(rd, res as u32);
                }
            }
            Instruction::Addu { rs, rt, rd } => {
                self.set_gpr(rd, self.gpr[rs].wrapping_add(self.gpr[rt]));
            }
            Instruction::Sub { rs, rt, rd } => {
                let (res, overflow) = (self.gpr[rs] as i32).overflowing_sub(self.gpr[rt] as i32);
                if overflow {
                    let target = self.cop0.trigger_exception(
                        Exception::Overflow,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    self.set_gpr(rd, res as u32);
                }
            }
            Instruction::Subu { rs, rt, rd } => {
                self.set_gpr(rd, self.gpr[rs].wrapping_sub(self.gpr[rt]));
            }
            Instruction::And { rs, rt, rd } => {
                self.set_gpr(rd, self.gpr[rs] & self.gpr[rt]);
            }
            Instruction::Or { rs, rt, rd } => {
                self.set_gpr(rd, self.gpr[rs] | self.gpr[rt]);
            }
            Instruction::Xor { rs, rt, rd } => {
                self.set_gpr(rd, self.gpr[rs] ^ self.gpr[rt]);
            }
            Instruction::Nor { rs, rt, rd } => {
                self.set_gpr(rd, !(self.gpr[rs] | self.gpr[rt]));
            }
            Instruction::Slt { rs, rt, rd } => {
                self.set_gpr(
                    rd,
                    if (self.gpr[rs] as i32) < (self.gpr[rt] as i32) {
                        1
                    } else {
                        0
                    },
                );
            }
            Instruction::Sltu { rs, rt, rd } => {
                self.set_gpr(rd, if self.gpr[rs] < self.gpr[rt] { 1 } else { 0 });
            }

            // Shifts
            Instruction::Sll { rt, rd, shamt } => {
                self.set_gpr(rd, self.gpr[rt] << shamt);
            }
            Instruction::Srl { rt, rd, shamt } => {
                self.set_gpr(rd, self.gpr[rt] >> shamt);
            }
            Instruction::Sra { rt, rd, shamt } => {
                self.set_gpr(rd, ((self.gpr[rt] as i32) >> shamt) as u32);
            }
            Instruction::Sllv { rs, rt, rd } => {
                self.set_gpr(rd, self.gpr[rt] << (self.gpr[rs] & 0x1F));
            }
            Instruction::Srlv { rs, rt, rd } => {
                self.set_gpr(rd, self.gpr[rt] >> (self.gpr[rs] & 0x1F));
            }
            Instruction::Srav { rs, rt, rd } => {
                self.set_gpr(rd, ((self.gpr[rt] as i32) >> (self.gpr[rs] & 0x1F)) as u32);
            }

            // Mult / Div
            Instruction::Mult { rs, rt } => {
                let res = (self.gpr[rs] as i32 as i64) * (self.gpr[rt] as i32 as i64);
                self.hi = (res >> 32) as u32;
                self.lo = res as u32;
            }
            Instruction::Multu { rs, rt } => {
                let res = (self.gpr[rs] as u64) * (self.gpr[rt] as u64);
                self.hi = (res >> 32) as u32;
                self.lo = res as u32;
            }
            Instruction::Div { rs, rt } => {
                let num = self.gpr[rs] as i32;
                let denom = self.gpr[rt] as i32;
                if denom == 0 {
                    self.hi = num as u32;
                    self.lo = if num < 0 { 1 } else { 0xFFFF_FFFF };
                } else if num == i32::MIN && denom == -1 {
                    self.hi = 0;
                    self.lo = i32::MIN as u32;
                } else {
                    self.lo = (num / denom) as u32;
                    self.hi = (num % denom) as u32;
                }
            }
            Instruction::Divu { rs, rt } => {
                let num = self.gpr[rs];
                let denom = self.gpr[rt];
                if let Some(lo) = num.checked_div(denom) {
                    self.lo = lo;
                    self.hi = num % denom;
                } else {
                    self.hi = num;
                    self.lo = 0xFFFF_FFFF;
                }
            }
            Instruction::Mfhi { rd } => {
                self.set_gpr(rd, self.hi);
            }
            Instruction::Mthi { rs } => {
                self.hi = self.gpr[rs];
            }
            Instruction::Mflo { rd } => {
                self.set_gpr(rd, self.lo);
            }
            Instruction::Mtlo { rs } => {
                self.lo = self.gpr[rs];
            }

            // ALU Immediate
            Instruction::Addi { rs, rt, imm16 } => {
                let (res, overflow) = (self.gpr[rs] as i32).overflowing_add(sign_ext(imm16) as i32);
                if overflow {
                    let target = self.cop0.trigger_exception(
                        Exception::Overflow,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    self.set_gpr(rt, res as u32);
                }
            }
            Instruction::Addiu { rs, rt, imm16 } => {
                self.set_gpr(rt, self.gpr[rs].wrapping_add(sign_ext(imm16)));
            }
            Instruction::Slti { rs, rt, imm16 } => {
                self.set_gpr(
                    rt,
                    if (self.gpr[rs] as i32) < (sign_ext(imm16) as i32) {
                        1
                    } else {
                        0
                    },
                );
            }
            Instruction::Sltiu { rs, rt, imm16 } => {
                self.set_gpr(rt, if self.gpr[rs] < sign_ext(imm16) { 1 } else { 0 });
            }
            Instruction::Andi { rs, rt, imm16 } => {
                self.set_gpr(rt, self.gpr[rs] & zero_ext(imm16));
            }
            Instruction::Ori { rs, rt, imm16 } => {
                self.set_gpr(rt, self.gpr[rs] | zero_ext(imm16));
            }
            Instruction::Xori { rs, rt, imm16 } => {
                self.set_gpr(rt, self.gpr[rs] ^ zero_ext(imm16));
            }
            Instruction::Lui { rt, imm16 } => {
                self.set_gpr(rt, zero_ext(imm16) << 16);
            }

            // Jumps & Branches
            Instruction::J { target26 } => {
                let target = (current_pc & 0xF000_0000) | (target26 << 2);
                self.branch(target);
            }
            Instruction::Jal { target26 } => {
                let target = (current_pc & 0xF000_0000) | (target26 << 2);
                self.set_gpr(31, current_pc.wrapping_add(8));
                self.branch(target);
            }
            Instruction::Jr { rs } => {
                let target = self.gpr[rs];
                self.branch(target);
            }
            Instruction::Jalr { rs, rd } => {
                let target = self.gpr[rs];
                self.set_gpr(rd, current_pc.wrapping_add(8));
                self.branch(target);
            }
            Instruction::Beq { rs, rt, imm16 } => {
                if self.gpr[rs] == self.gpr[rt] {
                    let target = current_pc
                        .wrapping_add(4)
                        .wrapping_add(sign_ext(imm16) << 2);
                    self.branch(target);
                }
            }
            Instruction::Bne { rs, rt, imm16 } => {
                if self.gpr[rs] != self.gpr[rt] {
                    let target = current_pc
                        .wrapping_add(4)
                        .wrapping_add(sign_ext(imm16) << 2);
                    self.branch(target);
                }
            }
            Instruction::Blez { rs, imm16 } => {
                if (self.gpr[rs] as i32) <= 0 {
                    let target = current_pc
                        .wrapping_add(4)
                        .wrapping_add(sign_ext(imm16) << 2);
                    self.branch(target);
                }
            }
            Instruction::Bgtz { rs, imm16 } => {
                if (self.gpr[rs] as i32) > 0 {
                    let target = current_pc
                        .wrapping_add(4)
                        .wrapping_add(sign_ext(imm16) << 2);
                    self.branch(target);
                }
            }
            Instruction::Bltz { rs, imm16 } => {
                if (self.gpr[rs] as i32) < 0 {
                    let target = current_pc
                        .wrapping_add(4)
                        .wrapping_add(sign_ext(imm16) << 2);
                    self.branch(target);
                }
            }
            Instruction::Bgez { rs, imm16 } => {
                if (self.gpr[rs] as i32) >= 0 {
                    let target = current_pc
                        .wrapping_add(4)
                        .wrapping_add(sign_ext(imm16) << 2);
                    self.branch(target);
                }
            }
            Instruction::Bltzal { rs, imm16 } => {
                let cond = (self.gpr[rs] as i32) < 0;
                self.set_gpr(31, current_pc.wrapping_add(8));
                if cond {
                    let target = current_pc
                        .wrapping_add(4)
                        .wrapping_add(sign_ext(imm16) << 2);
                    self.branch(target);
                }
            }
            Instruction::Bgezal { rs, imm16 } => {
                let cond = (self.gpr[rs] as i32) >= 0;
                self.set_gpr(31, current_pc.wrapping_add(8));
                if cond {
                    let target = current_pc
                        .wrapping_add(4)
                        .wrapping_add(sign_ext(imm16) << 2);
                    self.branch(target);
                }
            }

            // Memory Loads & Stores
            Instruction::Lb { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                let val = bus.read8(addr) as i8 as i32 as u32;
                self.schedule_load(rt, val);
            }
            Instruction::Lbu { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                let val = bus.read8(addr) as u32;
                self.schedule_load(rt, val);
            }
            Instruction::Lh { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                if !addr.is_multiple_of(2) {
                    self.cop0.badvaddr = addr;
                    let target = self.cop0.trigger_exception(
                        Exception::AddressErrorLoad,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    let val = bus.read16(addr) as i16 as i32 as u32;
                    self.schedule_load(rt, val);
                }
            }
            Instruction::Lhu { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                if !addr.is_multiple_of(2) {
                    self.cop0.badvaddr = addr;
                    let target = self.cop0.trigger_exception(
                        Exception::AddressErrorLoad,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    let val = bus.read16(addr) as u32;
                    self.schedule_load(rt, val);
                }
            }
            Instruction::Lw { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                if !addr.is_multiple_of(4) {
                    self.cop0.badvaddr = addr;
                    let target = self.cop0.trigger_exception(
                        Exception::AddressErrorLoad,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    let val = bus.read32(addr);
                    self.schedule_load(rt, val);
                }
            }
            Instruction::Lwl { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                let aligned_addr = addr & !3;
                let mem_word = bus.read32(aligned_addr);
                let shift = (addr & 3) * 8;
                let mask = 0x00FFFFFFu32 >> shift;
                let cur_val = if self.load_delay_current_reg == rt && rt != 0 {
                    self.load_delay_current_val
                } else {
                    self.gpr[rt]
                };
                let val = (cur_val & mask) | (mem_word << (24 - shift));
                self.schedule_load(rt, val);
            }
            Instruction::Lwr { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                let aligned_addr = addr & !3;
                let mem_word = bus.read32(aligned_addr);
                let shift = (addr & 3) * 8;
                let mask = if shift == 0 {
                    0
                } else {
                    0xFFFFFFFFu32 << (32 - shift)
                };
                let cur_val = if self.load_delay_current_reg == rt && rt != 0 {
                    self.load_delay_current_val
                } else {
                    self.gpr[rt]
                };
                let val = (cur_val & mask) | (mem_word >> shift);
                self.schedule_load(rt, val);
            }
            Instruction::Sb { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                bus.write8(addr, self.gpr[rt] as u8);
            }
            Instruction::Sh { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                if !addr.is_multiple_of(2) {
                    self.cop0.badvaddr = addr;
                    let target = self.cop0.trigger_exception(
                        Exception::AddressErrorStore,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    bus.write16(addr, self.gpr[rt] as u16);
                }
            }
            Instruction::Sw { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                if !addr.is_multiple_of(4) {
                    self.cop0.badvaddr = addr;
                    let target = self.cop0.trigger_exception(
                        Exception::AddressErrorStore,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    bus.write32(addr, self.gpr[rt]);
                }
            }
            Instruction::Swl { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                let aligned_addr = addr & !3;
                let mem_word = bus.read32(aligned_addr);
                let shift = (addr & 3) * 8;
                let mask = if shift == 24 {
                    0
                } else {
                    0xFFFFFF00u32 << shift
                };
                let reg_val = self.gpr[rt];
                let val = (mem_word & mask) | (reg_val >> (24 - shift));
                bus.write32(aligned_addr, val);
            }
            Instruction::Swr { rs, rt, imm16 } => {
                let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                let aligned_addr = addr & !3;
                let mem_word = bus.read32(aligned_addr);
                let shift = (addr & 3) * 8;
                let mask = 0x00FFFFFFu32 >> (24 - shift);
                let reg_val = self.gpr[rt];
                let val = (mem_word & mask) | (reg_val << shift);
                bus.write32(aligned_addr, val);
            }

            // COP0 & System
            Instruction::Mfc0 { rt, rd } => {
                let val = self.cop0.read_reg(rd);
                self.schedule_load(rt, val);
            }
            Instruction::Mtc0 { rt, rd } => {
                let val = self.gpr[rt];
                self.cop0.write_reg(rd, val);
            }
            Instruction::Rfe => {
                self.cop0.rfe();
            }
            Instruction::Syscall => {
                let target = self.cop0.trigger_exception(
                    Exception::Syscall,
                    is_current_delay_slot,
                    current_pc,
                );
                self.pc = target;
                self.next_pc = target.wrapping_add(4);
                self.in_delay_slot = false;
                self.next_in_delay_slot = false;
            }
            Instruction::Break => {
                let target = self.cop0.trigger_exception(
                    Exception::Break,
                    is_current_delay_slot,
                    current_pc,
                );
                self.pc = target;
                self.next_pc = target.wrapping_add(4);
                self.in_delay_slot = false;
                self.next_in_delay_slot = false;
            }
            Instruction::CopUnusable { cop_num } => {
                if !self.cop0.is_cop_usable(cop_num) {
                    let target = self.cop0.trigger_cop_unusable_exception(
                        cop_num,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                }
            }
            Instruction::Lwc {
                cop_num,
                rs,
                rt,
                imm16,
            } => {
                if !self.cop0.is_cop_usable(cop_num) {
                    let target = self.cop0.trigger_cop_unusable_exception(
                        cop_num,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                    let val = bus.read32(addr);
                    self.cop_regs[cop_num as usize][rt] = val;
                }
            }
            Instruction::Ldc {
                cop_num,
                rs,
                rt,
                imm16,
            } => {
                if !self.cop0.is_cop_usable(cop_num) {
                    let target = self.cop0.trigger_cop_unusable_exception(
                        cop_num,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                    let val_lo = bus.read32(addr);
                    let val_hi = bus.read32(addr.wrapping_add(4));
                    self.cop_regs[cop_num as usize][rt] = val_lo;
                    self.cop_regs[cop_num as usize][(rt + 1) & 31] = val_hi;
                }
            }
            Instruction::Swc {
                cop_num,
                rs,
                rt,
                imm16,
            } => {
                if !self.cop0.is_cop_usable(cop_num) {
                    let target = self.cop0.trigger_cop_unusable_exception(
                        cop_num,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                    let val = self.cop_regs[cop_num as usize][rt];
                    bus.write32(addr, val);
                }
            }
            Instruction::Sdc {
                cop_num,
                rs,
                rt,
                imm16,
            } => {
                if !self.cop0.is_cop_usable(cop_num) {
                    let target = self.cop0.trigger_cop_unusable_exception(
                        cop_num,
                        is_current_delay_slot,
                        current_pc,
                    );
                    self.pc = target;
                    self.next_pc = target.wrapping_add(4);
                    self.in_delay_slot = false;
                    self.next_in_delay_slot = false;
                } else {
                    let addr = self.gpr[rs].wrapping_add(sign_ext(imm16));
                    let val_lo = self.cop_regs[cop_num as usize][rt];
                    let val_hi = self.cop_regs[cop_num as usize][(rt + 1) & 31];
                    bus.write32(addr, val_lo);
                    bus.write32(addr.wrapping_add(4), val_hi);
                }
            }
            Instruction::Reserved => {
                let target = self.cop0.trigger_exception(
                    Exception::ReservedInstruction,
                    is_current_delay_slot,
                    current_pc,
                );
                self.pc = target;
                self.next_pc = target.wrapping_add(4);
                self.in_delay_slot = false;
                self.next_in_delay_slot = false;
            }
        }
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::memory_bus::MemoryBus;

    #[test]
    fn test_bios_a0_printf_interception() {
        let mut bus = MemoryBus::default_bus();
        let mut cpu = Cpu::new();

        // Put format string "Hello %s %d\0" at RAM address 0x1000
        let fmt_str = b"Hello %s %d\0";
        for (i, &b) in fmt_str.iter().enumerate() {
            bus.ram.write8(0x1000 + i as u32, b);
        }

        // Put string "World\0" at RAM address 0x2000
        let arg_str = b"World\0";
        for (i, &b) in arg_str.iter().enumerate() {
            bus.ram.write8(0x2000 + i as u32, b);
        }

        cpu.pc = 0x8000_00A0;
        cpu.gpr[9] = 0x3F; // A0 printf
        cpu.gpr[4] = 0x8000_1000; // $a0 = fmt
        cpu.gpr[5] = 0x8000_2000; // $a1 = "World"
        cpu.gpr[6] = 42; // $a2 = 42
        cpu.gpr[31] = 0x8001_0000; // $ra = caller

        cpu.step(&mut bus);

        assert_eq!(cpu.pc, 0x8001_0000);
        assert_eq!(bus.get_tty_string(), "Hello World 42");
    }
}
