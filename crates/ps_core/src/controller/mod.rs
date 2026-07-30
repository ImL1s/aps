//! PlayStation 1 Digital Controller Subsystem (Active-Low Input Registers)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadButton {
    Select,
    L3,
    R3,
    Start,
    Up,
    Right,
    Down,
    Left,
    L2,
    R2,
    L1,
    R1,
    Triangle,
    Circle,
    Cross,
    Square,
}

impl PadButton {
    pub fn bit_mask(self) -> u16 {
        match self {
            PadButton::Select => 1 << 0,
            PadButton::L3 => 1 << 1,
            PadButton::R3 => 1 << 2,
            PadButton::Start => 1 << 3,
            PadButton::Up => 1 << 4,
            PadButton::Right => 1 << 5,
            PadButton::Down => 1 << 6,
            PadButton::Left => 1 << 7,
            PadButton::L2 => 1 << 8,
            PadButton::R2 => 1 << 9,
            PadButton::L1 => 1 << 10,
            PadButton::R1 => 1 << 11,
            PadButton::Triangle => 1 << 12,
            PadButton::Circle => 1 << 13,
            PadButton::Cross => 1 << 14,
            PadButton::Square => 1 << 15,
        }
    }
}

pub fn map_key_to_button(key: sdl2::keyboard::Keycode) -> Option<PadButton> {
    use sdl2::keyboard::Keycode;
    match key {
        Keycode::RShift | Keycode::Backspace => Some(PadButton::Select),
        Keycode::Return | Keycode::Space => Some(PadButton::Start),
        Keycode::Up => Some(PadButton::Up),
        Keycode::Right => Some(PadButton::Right),
        Keycode::Down => Some(PadButton::Down),
        Keycode::Left => Some(PadButton::Left),
        Keycode::E | Keycode::Num3 => Some(PadButton::L2),
        Keycode::R | Keycode::Num4 => Some(PadButton::R2),
        Keycode::Q | Keycode::Num1 => Some(PadButton::L1),
        Keycode::W | Keycode::Num2 => Some(PadButton::R1),
        Keycode::S | Keycode::I => Some(PadButton::Triangle),
        Keycode::X | Keycode::K => Some(PadButton::Circle),
        Keycode::Z | Keycode::J => Some(PadButton::Cross),
        Keycode::A | Keycode::U => Some(PadButton::Square),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct Controller {
    pub button_state: u16,
}

impl Controller {
    pub fn new() -> Self {
        Self {
            button_state: 0xFFFF,
        }
    }

    pub fn set_button(&mut self, button: PadButton, pressed: bool) {
        let mask = button.bit_mask();
        if pressed {
            self.button_state &= !mask; // Active low (0 = pressed)
        } else {
            self.button_state |= mask; // 1 = released
        }
    }

    pub fn read8(&self, paddr: u32) -> u8 {
        match paddr {
            0x1F80_1040 => (self.button_state & 0xFF) as u8,
            0x1F80_1041 => ((self.button_state >> 8) & 0xFF) as u8,
            0x1F80_1044 => 0x05, // JOY_STAT: RX FIFO Not Empty / TX Ready
            _ => 0,
        }
    }

    pub fn read16(&self, paddr: u32) -> u16 {
        match paddr {
            0x1F80_1040 => self.button_state,
            0x1F80_1044 => 0x0005,
            _ => 0,
        }
    }

    pub fn read32(&self, paddr: u32) -> u32 {
        self.read16(paddr) as u32
    }

    pub fn write8(&mut self, _paddr: u32, _val: u8) {}
    pub fn write16(&mut self, _paddr: u32, _val: u16) {}
    pub fn write32(&mut self, _paddr: u32, _val: u32) {}
}

impl Default for Controller {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_initial_state_active_low() {
        let ctrl = Controller::new();
        assert_eq!(ctrl.button_state, 0xFFFF);
        assert_eq!(ctrl.read16(0x1F80_1040), 0xFFFF);
        assert_eq!(ctrl.read8(0x1F80_1040), 0xFF);
        assert_eq!(ctrl.read8(0x1F80_1041), 0xFF);
    }

    #[test]
    fn test_controller_button_press_and_release() {
        let mut ctrl = Controller::new();
        ctrl.set_button(PadButton::Cross, true);
        assert_eq!(ctrl.read16(0x1F80_1040) & (1 << 14), 0);

        ctrl.set_button(PadButton::Start, true);
        assert_eq!(ctrl.read16(0x1F80_1040) & (1 << 3), 0);

        ctrl.set_button(PadButton::Cross, false);
        assert_eq!(ctrl.read16(0x1F80_1040) & (1 << 14), 1 << 14);
        assert_eq!(ctrl.read16(0x1F80_1040) & (1 << 3), 0);
    }
}
