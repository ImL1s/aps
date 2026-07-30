//! PS1 Interrupt Controller (INTC)
//!
//! Manages 11 interrupt sources (IRQ 0 to IRQ 10) via I_STAT (0x1F80_1070)
//! and I_MASK (0x1F80_1074) registers.

pub const IRQ_VBLANK: u32 = 0;
pub const IRQ_GPU: u32 = 1;
pub const IRQ_CDROM: u32 = 2;
pub const IRQ_DMA: u32 = 3;
pub const IRQ_TIMER0: u32 = 4;
pub const IRQ_TIMER1: u32 = 5;
pub const IRQ_TIMER2: u32 = 6;
pub const IRQ_CONTROLLER: u32 = 7;
pub const IRQ_SIO: u32 = 8;
pub const IRQ_SPU: u32 = 9;
pub const IRQ_PIO: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InterruptController {
    pub istat: u32,
    pub imask: u32,
}

impl InterruptController {
    pub fn new() -> Self {
        Self { istat: 0, imask: 0 }
    }

    pub fn trigger(&mut self, irq: u32) {
        if irq <= 10 {
            self.istat |= 1 << irq;
        }
    }

    pub fn is_cpu_irq_asserted(&self) -> bool {
        (self.istat & self.imask & 0x7FF) != 0
    }

    pub fn read32(&self, paddr: u32) -> u32 {
        match paddr {
            0x1F80_1070 => self.istat,
            0x1F80_1074 => self.imask,
            _ => 0,
        }
    }

    pub fn read16(&self, paddr: u32) -> u16 {
        match paddr {
            0x1F80_1070 => self.istat as u16,
            0x1F80_1072 => (self.istat >> 16) as u16,
            0x1F80_1074 => self.imask as u16,
            0x1F80_1076 => (self.imask >> 16) as u16,
            _ => 0,
        }
    }

    pub fn read8(&self, paddr: u32) -> u8 {
        let shift = (paddr & 3) * 8;
        (self.read32(paddr & !3) >> shift) as u8
    }

    pub fn write32(&mut self, paddr: u32, val: u32) {
        match paddr {
            // Writing 1 to a bit in I_STAT clears that bit (istat &= !val)
            0x1F80_1070 => self.istat &= !val,
            0x1F80_1074 => self.imask = val & 0x7FF,
            _ => {}
        }
    }

    pub fn write16(&mut self, paddr: u32, val: u16) {
        match paddr {
            0x1F80_1070 => self.istat &= !(val as u32),
            0x1F80_1072 => self.istat &= !((val as u32) << 16),
            0x1F80_1074 => {
                let mask = (self.imask & 0xFFFF_0000) | (val as u32);
                self.imask = mask & 0x7FF;
            }
            0x1F80_1076 => {
                let mask = (self.imask & 0x0000_FFFF) | ((val as u32) << 16);
                self.imask = mask & 0x7FF;
            }
            _ => {}
        }
    }

    pub fn write8(&mut self, paddr: u32, val: u8) {
        let shift = (paddr & 3) * 8;
        let base = paddr & !3;
        if base == 0x1F80_1070 {
            self.istat &= !((val as u32) << shift);
        } else if base == 0x1F80_1074 {
            let mut cur = self.imask;
            let byte_mask = !(0xFF << shift);
            cur = (cur & byte_mask) | ((val as u32) << shift);
            self.imask = cur & 0x7FF;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intc_trigger_and_mask() {
        let mut intc = InterruptController::new();
        assert!(!intc.is_cpu_irq_asserted());

        intc.trigger(IRQ_VBLANK);
        assert_eq!(intc.read32(0x1F80_1070), 1);
        assert!(!intc.is_cpu_irq_asserted()); // mask is 0

        intc.write32(0x1F80_1074, 1);
        assert!(intc.is_cpu_irq_asserted());
    }

    #[test]
    fn test_intc_write_one_to_clear() {
        let mut intc = InterruptController::new();
        intc.trigger(IRQ_GPU);
        intc.trigger(IRQ_DMA);
        assert_eq!(intc.read32(0x1F80_1070), (1 << IRQ_GPU) | (1 << IRQ_DMA));

        // Write 1 to clear GPU bit only
        intc.write32(0x1F80_1070, 1 << IRQ_GPU);
        assert_eq!(intc.read32(0x1F80_1070), 1 << IRQ_DMA);
    }
}
