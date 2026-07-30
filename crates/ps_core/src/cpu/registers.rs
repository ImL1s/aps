#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterFile {
    pub gpr: [u32; 32],
    pub hi: u32,
    pub lo: u32,
    pub pc: u32,
}

impl RegisterFile {
    pub fn new() -> Self {
        Self {
            gpr: [0; 32],
            hi: 0,
            lo: 0,
            pc: 0xBFC0_0000, // BIOS Reset Vector
        }
    }

    #[inline(always)]
    pub fn read(&self, reg: usize) -> u32 {
        if reg == 0 {
            0
        } else {
            self.gpr[reg]
        }
    }

    #[inline(always)]
    pub fn write(&mut self, reg: usize, val: u32) {
        if reg != 0 {
            self.gpr[reg] = val;
        }
    }
}

impl Default for RegisterFile {
    fn default() -> Self {
        Self::new()
    }
}
