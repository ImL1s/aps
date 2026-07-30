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

    #[test]
    fn test_timer_target_zero_65536_wraparound() {
        let mut timers = Timers::new();
        // Mode: reset_on_target (bit 3) | irq_on_target (bit 4) | irq_repeat (bit 6)
        let mode = (1 << 3) | (1 << 4) | (1 << 6);
        timers.write16(0x1F80_1104, mode);
        timers.write16(0x1F80_1108, 0); // Target = 0

        // Step 65535 cycles -> val should be 65535, no IRQ yet
        let irqs = timers.step(65535);
        assert_eq!(irqs, 0);
        assert_eq!(timers.read16(0x1F80_1100), 65535);

        // 65536th cycle -> wraps to 0, triggers target match IRQ
        let irqs_wrap = timers.step(1);
        assert_ne!(irqs_wrap & (1 << IRQ_TIMER0), 0);
        assert_eq!(timers.read16(0x1F80_1100), 0);

        // Both reached_target (bit 11) and reached_overflow (bit 12) must be set in mode
        let mode_val = timers.read16(0x1F80_1104);
        assert_ne!(
            mode_val & (1 << 11),
            0,
            "Bit 11 (reached_target) must be set"
        );
        assert_ne!(
            mode_val & (1 << 12),
            0,
            "Bit 12 (reached_overflow) must be set"
        );
    }

    #[test]
    fn test_timer_target_0xffff_max_value_semantics() {
        let mut timers = Timers::new();

        // Part 1: reset_on_target = false, irq_on_target = true, irq_on_overflow = true, irq_repeat = true
        let mode_no_reset = (1 << 4) | (1 << 5) | (1 << 6);
        timers.write16(0x1F80_1104, mode_no_reset);
        timers.write16(0x1F80_1108, 0xFFFF);

        // Step 65535 cycles -> val reaches 0xFFFF
        let irqs1 = timers.step(65535);
        assert_ne!(
            irqs1 & (1 << IRQ_TIMER0),
            0,
            "Target match IRQ at cycle 65535"
        );
        assert_eq!(timers.read16(0x1F80_1100), 0xFFFF);

        // Step 1 more cycle -> 65536th cycle wraps to 0, triggers overflow IRQ
        let irqs2 = timers.step(1);
        assert_ne!(irqs2 & (1 << IRQ_TIMER0), 0, "Overflow IRQ at cycle 65536");
        assert_eq!(timers.read16(0x1F80_1100), 0);

        // Part 2: reset_on_target = true
        let mode_reset = (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6);
        timers.write16(0x1F80_1104, mode_reset);
        timers.write16(0x1F80_1108, 0xFFFF);

        // Step 65535 cycles -> target match triggers, val resets to 0
        timers.step(65535);
        assert_eq!(timers.read16(0x1F80_1100), 0);

        // Read mode: Bit 11 must be set, Bit 12 must NOT be set
        let m = timers.read16(0x1F80_1104);
        assert_ne!(m & (1 << 11), 0, "Bit 11 (reached_target) must be set");
        assert_eq!(
            m & (1 << 12),
            0,
            "Bit 12 (reached_overflow) must NOT be set when reset_on_target is true"
        );
    }

    #[test]
    fn test_timer_oneshot_read_write_rearm_behavior() {
        let mut timers = Timers::new();

        // Oneshot mode: irq_on_target (bit 4), reset_on_target (bit 3), irq_repeat = 0
        let mode_oneshot = (1 << 3) | (1 << 4);
        timers.write16(0x1F80_1104, mode_oneshot);
        timers.write16(0x1F80_1108, 10);

        // First match -> IRQ triggered
        let irqs1 = timers.step(10);
        assert_ne!(irqs1 & (1 << IRQ_TIMER0), 0);

        // Read mode register -> clears bit 11, but should NOT re-arm oneshot IRQ
        let mode_read = timers.read16(0x1F80_1104);
        assert_ne!(mode_read & (1 << 11), 0);

        // Second match -> IRQ must NOT trigger
        let irqs2 = timers.step(10);
        assert_eq!(
            irqs2 & (1 << IRQ_TIMER0),
            0,
            "Read mode must not re-arm oneshot IRQ"
        );

        // Write counter or target -> should NOT re-arm oneshot IRQ
        timers.write16(0x1F80_1100, 0);
        timers.write16(0x1F80_1108, 10);
        let irqs3 = timers.step(10);
        assert_eq!(
            irqs3 & (1 << IRQ_TIMER0),
            0,
            "Writing counter/target must not re-arm oneshot IRQ"
        );

        // Write mode register -> Re-arms oneshot IRQ
        timers.write16(0x1F80_1104, mode_oneshot);
        let irqs4 = timers.step(10);
        assert_ne!(
            irqs4 & (1 << IRQ_TIMER0),
            0,
            "Writing mode must re-arm oneshot IRQ"
        );
    }
}
