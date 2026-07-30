//! Single hardware timer implementation for PS1 (Timer 0, 1, 2).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timer {
    pub val: u16,
    pub mode: u16,
    pub target: u16,
    pub accum: u32,
    pub irq_fired_once: bool,
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            val: 0,
            mode: 0x0400, // Bit 10 default 1 (interrupt line inactive)
            target: 0,
            accum: 0,
            irq_fired_once: false,
        }
    }
}

impl Timer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_mode(&mut self, new_mode: u16) {
        self.mode = new_mode | (1 << 10); // Reset IRQ bit 10 to 1 (inactive)
        self.val = 0;
        self.irq_fired_once = false;
    }

    pub fn read_mode(&mut self) -> u16 {
        let m = self.mode;
        // Clear reached_target (bit 11) and reached_overflow (bit 12) on read
        self.mode &= !((1 << 11) | (1 << 12));
        m
    }

    pub fn step(&mut self, cycles: u32, timer_idx: usize) -> bool {
        let clock_src = (self.mode >> 8) & 0x3;

        let ticks = if timer_idx == 2 && (clock_src == 2 || clock_src == 3) {
            self.accum += cycles;
            let t = self.accum / 8;
            self.accum %= 8;
            t
        } else {
            cycles
        };

        if ticks == 0 {
            return false;
        }

        let mut irq_triggered = false;
        let reset_on_target = (self.mode & (1 << 3)) != 0;
        let irq_on_target = (self.mode & (1 << 4)) != 0;
        let irq_on_overflow = (self.mode & (1 << 5)) != 0;
        let irq_repeat = (self.mode & (1 << 6)) != 0;
        let irq_toggle = (self.mode & (1 << 7)) != 0;

        for _ in 0..ticks {
            let prev = self.val;
            self.val = self.val.wrapping_add(1);

            // Check target match
            if self.val == self.target {
                self.mode |= 1 << 11; // reached_target
                if reset_on_target {
                    self.val = 0;
                }
                if irq_on_target && (irq_repeat || !self.irq_fired_once) {
                    irq_triggered = true;
                    self.irq_fired_once = true;
                    if irq_toggle {
                        self.mode ^= 1 << 10;
                    } else {
                        self.mode &= !(1 << 10);
                    }
                }
            }

            // Check overflow
            if prev == 0xFFFF && self.val == 0 {
                self.mode |= 1 << 12; // reached_overflow
                if irq_on_overflow && (irq_repeat || !self.irq_fired_once) {
                    irq_triggered = true;
                    self.irq_fired_once = true;
                    if irq_toggle {
                        self.mode ^= 1 << 10;
                    } else {
                        self.mode &= !(1 << 10);
                    }
                }
            }
        }

        irq_triggered
    }
}
