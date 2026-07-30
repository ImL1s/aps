//! PS1 DMA Channel implementation

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DmaChannel {
    pub madr: u32,
    pub bcr: u32,
    pub chcr: u32,
}

impl DmaChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn direction_from_ram(&self) -> bool {
        (self.chcr & 1) != 0
    }

    pub fn step_backward(&self) -> bool {
        (self.chcr & 2) != 0
    }

    pub fn sync_mode(&self) -> u32 {
        (self.chcr >> 9) & 3
    }

    pub fn is_trigger_set(&self) -> bool {
        (self.chcr & (1 << 24)) != 0
    }

    pub fn is_busy(&self) -> bool {
        (self.chcr & (1 << 28)) != 0
    }

    pub fn start_transfer(&mut self) {
        self.chcr |= 1 << 28; // Set busy bit
    }

    pub fn finish_transfer(&mut self) {
        self.chcr &= !(1 << 24); // Clear trigger bit
        self.chcr &= !(1 << 28); // Clear busy bit
    }

    pub fn read32(&self, offset: u32) -> u32 {
        match offset {
            0x0 => self.madr,
            0x4 => self.bcr,
            0x8 => self.chcr,
            _ => 0,
        }
    }

    pub fn write32(&mut self, offset: u32, val: u32) {
        match offset {
            0x0 => self.madr = val & 0x00FF_FFFF,
            0x4 => self.bcr = val,
            0x8 => self.chcr = val,
            _ => {}
        }
    }
}
