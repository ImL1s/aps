//! PS1 Hardware Timers Manager (Timers 0, 1, 2)

pub mod timer;

use timer::Timer;

pub const IRQ_TIMER0: u32 = 4;
pub const IRQ_TIMER1: u32 = 5;
pub const IRQ_TIMER2: u32 = 6;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Timers {
    pub timer0: Timer,
    pub timer1: Timer,
    pub timer2: Timer,
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timer0: Timer::new(),
            timer1: Timer::new(),
            timer2: Timer::new(),
        }
    }

    /// Advance timers by `cycles`. Returns bitmask of INTC IRQ lines triggered.
    pub fn step(&mut self, cycles: u32) -> u32 {
        let mut irqs = 0u32;
        if self.timer0.step(cycles, 0) {
            irqs |= 1 << IRQ_TIMER0;
        }
        if self.timer1.step(cycles, 1) {
            irqs |= 1 << IRQ_TIMER1;
        }
        if self.timer2.step(cycles, 2) {
            irqs |= 1 << IRQ_TIMER2;
        }
        irqs
    }

    pub fn read32(&mut self, paddr: u32) -> u32 {
        self.read16(paddr) as u32
    }

    pub fn read16(&mut self, paddr: u32) -> u16 {
        let timer_idx = (paddr >> 4) & 0x3;
        let reg_offset = paddr & 0xF;
        let timer = match timer_idx {
            0 => &mut self.timer0,
            1 => &mut self.timer1,
            2 => &mut self.timer2,
            _ => return 0,
        };

        match reg_offset {
            0x0 => timer.val,
            0x4 => timer.read_mode(),
            0x8 => timer.target,
            _ => 0,
        }
    }

    pub fn read8(&mut self, paddr: u32) -> u8 {
        let shift = (paddr & 1) * 8;
        (self.read16(paddr & !1) >> shift) as u8
    }

    pub fn write32(&mut self, paddr: u32, val: u32) {
        self.write16(paddr, val as u16);
    }

    pub fn write16(&mut self, paddr: u32, val: u16) {
        let timer_idx = (paddr >> 4) & 0x3;
        let reg_offset = paddr & 0xF;
        let timer = match timer_idx {
            0 => &mut self.timer0,
            1 => &mut self.timer1,
            2 => &mut self.timer2,
            _ => return,
        };

        match reg_offset {
            0x0 => timer.val = val,
            0x4 => timer.write_mode(val),
            0x8 => timer.target = val,
            _ => {}
        }
    }

    pub fn write8(&mut self, paddr: u32, val: u8) {
        let base = paddr & !1;
        let shift = (paddr & 1) * 8;
        let cur = self.read16(base);
        let mask = !(0xFF << shift);
        let new_val = (cur & mask) | ((val as u16) << shift);
        self.write16(base, new_val);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer0_target_match_irq() {
        let mut timers = Timers::new();
        // Mode: reset_on_target (bit 3) | irq_on_target (bit 4) | irq_repeat (bit 6)
        let mode = (1 << 3) | (1 << 4) | (1 << 6);
        timers.write16(0x1F80_1104, mode);
        timers.write16(0x1F80_1108, 10);

        let irqs = timers.step(10);
        assert_ne!(irqs & (1 << IRQ_TIMER0), 0);
        assert_eq!(timers.read16(0x1F80_1100), 0); // Reset on target
    }

    #[test]
    fn test_timer2_sysclock_divider() {
        let mut timers = Timers::new();
        // Mode: clock_source = 2 (SysClock / 8)
        let mode = 2 << 8;
        timers.write16(0x1F80_1124, mode);

        // Step 7 cycles -> not enough for 1 tick
        let irqs = timers.step(7);
        assert_eq!(irqs, 0);
        assert_eq!(timers.read16(0x1F80_1120), 0);

        // Step 1 more cycle -> 8 total = 1 tick
        timers.step(1);
        assert_eq!(timers.read16(0x1F80_1120), 1);
    }
}
