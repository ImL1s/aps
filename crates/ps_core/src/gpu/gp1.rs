//! GP1 Control command processor & GPUSTAT status register bitfield

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gp1 {
    pub display_enable: bool,
    pub dma_direction: u32,
    pub irq_requested: bool,
    pub display_vram_x: u32,
    pub display_vram_y: u32,
    pub display_horiz_x1: u32,
    pub display_horiz_x2: u32,
    pub display_vert_y1: u32,
    pub display_vert_y2: u32,
    pub display_mode: u32,
}

impl Default for Gp1 {
    fn default() -> Self {
        Self {
            display_enable: true, // 1 = Display Off (Default)
            dma_direction: 0,
            irq_requested: false,
            display_vram_x: 0,
            display_vram_y: 0,
            display_horiz_x1: 0,
            display_horiz_x2: 0,
            display_vert_y1: 0,
            display_vert_y2: 0,
            display_mode: 0,
        }
    }
}

impl Gp1 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn get_gpustat(&self) -> u32 {
        let mut stat = 0x1C00_0000u32; // Ready bits 26, 27, 28 set to 1

        if self.display_enable {
            stat |= 1 << 23;
        }
        if self.irq_requested {
            stat |= 1 << 24;
        }
        stat |= (self.dma_direction & 3) << 29;

        // Add bits from display mode if configured
        stat |= self.display_mode & 0x007F_FFFF;

        stat
    }

    pub fn process_command(&mut self, cmd_word: u32) {
        let opcode = (cmd_word >> 24) & 0xFF;
        match opcode {
            0x00 => self.reset(),
            0x01 => {
                // Reset command buffer / FIFO
            }
            0x02 => {
                // Acknowledge IRQ
                self.irq_requested = false;
            }
            0x03 => {
                // Display Enable (bit 0 = 0: ON, bit 0 = 1: OFF)
                self.display_enable = (cmd_word & 1) != 0;
            }
            0x04 => {
                // DMA Direction
                self.dma_direction = cmd_word & 3;
            }
            0x05 => {
                // Display VRAM start position
                self.display_vram_x = cmd_word & 0x3FE;
                self.display_vram_y = (cmd_word >> 10) & 0x1FF;
            }
            0x06 => {
                // Horizontal Display Range
                self.display_horiz_x1 = cmd_word & 0xFFF;
                self.display_horiz_x2 = (cmd_word >> 12) & 0xFFF;
            }
            0x07 => {
                // Vertical Display Range
                self.display_vert_y1 = cmd_word & 0x3FF;
                self.display_vert_y2 = (cmd_word >> 10) & 0x3FF;
            }
            0x08 => {
                // Display Mode
                self.display_mode = cmd_word & 0x003F_FFFF;
            }
            _ => {}
        }
    }
}
