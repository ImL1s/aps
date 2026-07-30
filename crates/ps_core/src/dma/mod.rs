//! PS1 7-Channel DMA Controller

pub mod channel;

use crate::gpu::Gpu;
use crate::intc::{InterruptController, IRQ_DMA};
use crate::ram::Ram;
use channel::DmaChannel;

pub const DMA_MDEC_IN: usize = 0;
pub const DMA_MDEC_OUT: usize = 1;
pub const DMA_GPU: usize = 2;
pub const DMA_CDROM: usize = 3;
pub const DMA_SPU: usize = 4;
pub const DMA_PIO: usize = 5;
pub const DMA_OTC: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaController {
    pub channels: [DmaChannel; 7],
    pub dpcr: u32,
    pub dicr: u32,
}

impl Default for DmaController {
    fn default() -> Self {
        Self {
            channels: [
                DmaChannel::new(),
                DmaChannel::new(),
                DmaChannel::new(),
                DmaChannel::new(),
                DmaChannel::new(),
                DmaChannel::new(),
                DmaChannel::new(),
            ],
            dpcr: 0x0765_4321,
            dicr: 0,
        }
    }
}

impl DmaController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_channel_enabled(&self, ch: usize) -> bool {
        let shift = ch * 4 + 3;
        (self.dpcr & (1 << shift)) != 0
    }

    pub fn update_dicr(&mut self, intc: &mut InterruptController) {
        let force_irq = (self.dicr & (1 << 15)) != 0;
        let master_enable = (self.dicr & (1 << 23)) != 0;
        let irq_enable_mask = (self.dicr >> 16) & 0x7F;
        let irq_flags = (self.dicr >> 24) & 0x7F;

        let master_flag = force_irq || (master_enable && ((irq_flags & irq_enable_mask) != 0));

        if master_flag {
            self.dicr |= 1 << 31;
            intc.trigger(IRQ_DMA);
        } else {
            self.dicr &= !(1 << 31);
        }
    }

    pub fn set_channel_irq(&mut self, ch: usize, intc: &mut InterruptController) {
        if ch <= 6 {
            self.dicr |= 1 << (24 + ch);
            self.update_dicr(intc);
        }
    }

    pub fn has_active_transfer(&self) -> bool {
        for (i, ch) in self.channels.iter().enumerate() {
            if (ch.is_trigger_set() || ch.is_busy()) && self.is_channel_enabled(i) {
                return true;
            }
        }
        false
    }

    pub fn step_dma(
        &mut self,
        ram: &mut Ram,
        gpu: &mut Gpu,
        intc: &mut InterruptController,
    ) -> bool {
        let mut executed = false;

        for ch in 0..7 {
            if !self.is_channel_enabled(ch) {
                continue;
            }

            let channel = &mut self.channels[ch];
            if !channel.is_trigger_set() && !channel.is_busy() {
                continue;
            }

            channel.start_transfer();
            executed = true;

            match ch {
                // OTC Clear (Ch 6)
                6 => {
                    let mut count = channel.bcr & 0xFFFF;
                    if count == 0 {
                        count = 0x10000;
                    }

                    let mut addr = channel.madr & 0x00FF_FFFF;
                    for _ in 1..count {
                        let next_addr = addr.wrapping_sub(4) & 0x00FF_FFFF;
                        ram.write32(addr & 0x001F_FFFF, next_addr);
                        addr = next_addr;
                    }
                    ram.write32(addr & 0x001F_FFFF, 0x00FF_FFFF);
                    channel.madr = addr;
                    channel.finish_transfer();
                    self.set_channel_irq(ch, intc);
                }

                // GPU DMA (Ch 2)
                2 => {
                    if channel.sync_mode() == 2 {
                        // Linked List Mode
                        let mut header_addr = channel.madr & 0x001F_FFFF;
                        loop {
                            let header = ram.read32(header_addr);
                            let count = header >> 24;
                            let next_ptr = header & 0x00FF_FFFF;

                            for i in 1..=count {
                                let word_addr = (header_addr + i * 4) & 0x001F_FFFF;
                                let val = ram.read32(word_addr);
                                gpu.write_gp0(val);
                            }

                            if next_ptr == 0x00FF_FFFF {
                                channel.madr = next_ptr;
                                break;
                            }
                            header_addr = next_ptr & 0x001F_FFFF;
                        }
                        channel.finish_transfer();
                        self.set_channel_irq(ch, intc);
                    } else if channel.direction_from_ram() {
                        // RAM to GPU
                        let count = if channel.sync_mode() == 1 {
                            let bc = channel.bcr & 0xFFFF;
                            let ba = (channel.bcr >> 16) & 0xFFFF;
                            if bc * ba == 0 {
                                0x10000
                            } else {
                                bc * ba
                            }
                        } else {
                            let c = channel.bcr & 0xFFFF;
                            if c == 0 {
                                0x10000
                            } else {
                                c
                            }
                        };

                        let step_size = if channel.step_backward() { -4i32 } else { 4i32 };
                        let mut addr = channel.madr;
                        for _ in 0..count {
                            let val = ram.read32(addr & 0x001F_FFFF);
                            gpu.write_gp0(val);
                            addr = (addr as i32 + step_size) as u32 & 0x00FF_FFFF;
                        }
                        channel.madr = addr;
                        channel.finish_transfer();
                        self.set_channel_irq(ch, intc);
                    } else {
                        // GPU to RAM
                        let count = if channel.sync_mode() == 1 {
                            let bc = channel.bcr & 0xFFFF;
                            let ba = (channel.bcr >> 16) & 0xFFFF;
                            if bc * ba == 0 {
                                0x10000
                            } else {
                                bc * ba
                            }
                        } else {
                            let c = channel.bcr & 0xFFFF;
                            if c == 0 {
                                0x10000
                            } else {
                                c
                            }
                        };

                        let step_size = if channel.step_backward() { -4i32 } else { 4i32 };
                        let mut addr = channel.madr;
                        for _ in 0..count {
                            let val = gpu.read_gpuread();
                            ram.write32(addr & 0x001F_FFFF, val);
                            addr = (addr as i32 + step_size) as u32 & 0x00FF_FFFF;
                        }
                        channel.madr = addr;
                        channel.finish_transfer();
                        self.set_channel_irq(ch, intc);
                    }
                }
                // Generic Block transfers for other channels
                _ => {
                    let count = if channel.sync_mode() == 1 {
                        let bc = channel.bcr & 0xFFFF;
                        let ba = (channel.bcr >> 16) & 0xFFFF;
                        if bc * ba == 0 {
                            0x10000
                        } else {
                            bc * ba
                        }
                    } else {
                        let c = channel.bcr & 0xFFFF;
                        if c == 0 {
                            0x10000
                        } else {
                            c
                        }
                    };

                    let step_size = if channel.step_backward() { -4i32 } else { 4i32 };
                    let mut addr = channel.madr;
                    for _ in 0..count {
                        addr = (addr as i32 + step_size) as u32 & 0x00FF_FFFF;
                    }
                    channel.madr = addr;
                    channel.finish_transfer();
                    self.set_channel_irq(ch, intc);
                }
            }
        }

        executed
    }

    pub fn read32(&self, paddr: u32) -> u32 {
        let offset = paddr & 0xFF;
        if offset < 0xF0 {
            let ch = ((offset.saturating_sub(0x80)) >> 4) as usize;
            let reg = offset & 0xF;
            if ch < 7 {
                self.channels[ch].read32(reg)
            } else {
                0
            }
        } else {
            match offset {
                0xF0 => self.dpcr,
                0xF4 => self.dicr,
                _ => 0,
            }
        }
    }

    pub fn write32(&mut self, paddr: u32, val: u32, intc: &mut InterruptController) {
        let offset = paddr & 0xFF;
        if offset < 0xF0 {
            let ch = ((offset.saturating_sub(0x80)) >> 4) as usize;
            let reg = offset & 0xF;
            if ch < 7 {
                self.channels[ch].write32(reg, val);
            }
        } else {
            match offset {
                0xF0 => self.dpcr = val,
                0xF4 => {
                    // Low 24 bits written directly
                    let low24 = val & 0x00FF_FFFF;
                    // High bits (24..30): writing 1 clears IRQ flag
                    let clear_flags = (val >> 24) & 0x7F;
                    let cur_flags = (self.dicr >> 24) & 0x7F;
                    let new_flags = cur_flags & !clear_flags;

                    self.dicr = low24 | (new_flags << 24);
                    self.update_dicr(intc);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu::Gpu;
    use crate::intc::InterruptController;
    use crate::ram::Ram;

    #[test]
    fn test_dma_mode0_single_block() {
        let mut dma = DmaController::new();
        let mut ram = Ram::new();
        let mut gpu = Gpu::new();
        let mut intc = InterruptController::new();

        // Enable Ch2 (GPU) in DPCR
        dma.dpcr |= 1 << (2 * 4 + 3);

        // Setup Ch2 MADR = 0x1000, BCR = 4 words, CHCR = 0x01000200 (Mode 0, Trigger, RAM->GPU)
        dma.channels[2].madr = 0x1000;
        dma.channels[2].bcr = 4;
        dma.channels[2].chcr = (1 << 24) | 1; // Trigger bit + SyncMode 0 + From RAM

        let executed = dma.step_dma(&mut ram, &mut gpu, &mut intc);
        assert!(executed);
        assert_eq!(dma.channels[2].madr, 0x1010);
        assert!(!dma.channels[2].is_trigger_set());
    }

    #[test]
    fn test_dma_ch2_linked_list() {
        let mut dma = DmaController::new();
        let mut ram = Ram::new();
        let mut gpu = Gpu::new();
        let mut intc = InterruptController::new();

        // Enable Ch2 in DPCR
        dma.dpcr |= 1 << (2 * 4 + 3);

        // Write linked list into RAM:
        // Header at 0x1000: 2 words payload, next = 0x2000 => (2 << 24) | 0x2000
        // Words at 0x1004, 0x1008
        // Header at 0x2000: 1 word payload, next = 0x00FFFFFF (end)
        // Word at 0x2004
        ram.write32(0x1000, (2 << 24) | 0x2000);
        ram.write32(0x1004, 0xE1000000); // GP0 Command: Draw Mode
        ram.write32(0x1008, 0xE2000000); // GP0 Command: Texture Window
        ram.write32(0x2000, (1 << 24) | 0x00FF_FFFF);
        ram.write32(0x2004, 0xE3000000); // GP0 Command: Drawing Area Top-Left

        dma.channels[2].madr = 0x1000;
        dma.channels[2].chcr = (1 << 24) | (2 << 9); // Trigger + SyncMode 2 (Linked List)

        let executed = dma.step_dma(&mut ram, &mut gpu, &mut intc);
        assert!(executed);
        assert_eq!(dma.channels[2].madr, 0x00FF_FFFF);
    }

    #[test]
    fn test_dma_ch5_otc_clear() {
        let mut dma = DmaController::new();
        let mut ram = Ram::new();
        let mut gpu = Gpu::new();
        let mut intc = InterruptController::new();

        // Enable Ch 6 (OTC) in DPCR
        dma.dpcr |= 1 << (6 * 4 + 3);

        // Setup OTC: MADR = 0x1000, BCR = 4 words
        dma.channels[6].madr = 0x1000;
        dma.channels[6].bcr = 4;
        dma.channels[6].chcr = 1 << 24; // Trigger

        dma.step_dma(&mut ram, &mut gpu, &mut intc);

        // 0x1000 points to 0x0FFC
        assert_eq!(ram.read32(0x1000), 0x0FFC);
        // 0x0FFC points to 0x0FF8
        assert_eq!(ram.read32(0x0FFC), 0x0FF8);
        // 0x0FF8 points to 0x0FF4
        assert_eq!(ram.read32(0x0FF8), 0x0FF4);
        // 0x0FF4 points to end marker 0x00FFFFFF
        assert_eq!(ram.read32(0x0FF4), 0x00FF_FFFF);
    }
}
