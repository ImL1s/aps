#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Exception {
    Interrupt = 0x00,
    AddressErrorLoad = 0x04,
    AddressErrorStore = 0x05,
    Syscall = 0x08,
    Break = 0x09,
    ReservedInstruction = 0x0A,
    CoprocessorUnusable = 0x0B,
    Overflow = 0x0C,
}

pub type ExceptionCode = Exception;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cop0 {
    pub badvaddr: u32, // Register 8
    pub status: u32,   // Register 12 (SR)
    pub cause: u32,    // Register 13
    pub epc: u32,      // Register 14
    pub prid: u32,     // Register 15 (read-only 0x0000_0002)
}

impl Cop0 {
    pub fn new() -> Self {
        Self {
            badvaddr: 0,
            // Default Status: BEV bit (bit 22) is set to 1 on reset (0x0040_0000)
            status: 0x0040_0000,
            cause: 0,
            epc: 0,
            prid: 0x0000_0002,
        }
    }

    pub fn cause_exc_code(&self) -> u32 {
        (self.cause >> 2) & 0x1F
    }

    pub fn read_reg(&self, reg: usize) -> u32 {
        match reg {
            8 => self.badvaddr,
            12 => self.status,
            13 => self.cause,
            14 => self.epc,
            15 => 0x0000_0002,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, reg: usize, val: u32) {
        match reg {
            8 => self.badvaddr = val,
            12 => self.status = val,
            13 => self.cause = val,
            14 => self.epc = val,
            15 => {} // PRId is read-only
            _ => {}
        }
    }

    /// Trigger exception and return exception vector address
    pub fn trigger_exception(&mut self, exc: Exception, in_delay_slot: bool, pc: u32) -> u32 {
        // Shift 3-level interrupt/user mode stack in status register (bits 5..0)
        let mode = self.status & 0x3F;
        let shifted_mode = (mode << 2) & 0x3F;
        self.status = (self.status & !0x3F) | shifted_mode;

        // Update Cause register ExcCode (bits 6..2)
        self.cause = (self.cause & !0x7C) | (((exc as u32) & 0x1F) << 2);

        // Branch delay slot tracking (bit 31 of Cause) & EPC assignment
        if in_delay_slot {
            self.cause |= 1 << 31;
            self.epc = pc.wrapping_sub(4);
        } else {
            self.cause &= !(1 << 31);
            self.epc = pc;
        }

        // Determine exception vector address based on BEV bit (bit 22 of Status)
        if (self.status & (1 << 22)) != 0 {
            0xBFC0_0180 // Boot exception vector
        } else {
            0x8000_0080 // Normal exception vector
        }
    }

    pub fn is_cop_usable(&self, cop_num: u32) -> bool {
        match cop_num {
            0 => true,
            2 => (self.status & (1 << 30)) != 0,
            _ => false,
        }
    }

    pub fn trigger_cop_unusable_exception(
        &mut self,
        cop_num: u32,
        in_delay_slot: bool,
        pc: u32,
    ) -> u32 {
        let target = self.trigger_exception(Exception::CoprocessorUnusable, in_delay_slot, pc);
        self.cause = (self.cause & !(3 << 28)) | ((cop_num & 3) << 28);
        target
    }

    /// Restore From Exception (RFE) instruction handler
    pub fn rfe(&mut self) {
        // Pop 3-level interrupt/user mode stack in status register
        let mode = self.status & 0x3F;
        let shifted_mode = (mode >> 2) & 0x0F;
        self.status = (self.status & !0x0F) | shifted_mode;
    }
}

impl Default for Cop0 {
    fn default() -> Self {
        Self::new()
    }
}
