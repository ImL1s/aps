use crate::bios::Bios;
use crate::bus::map::{
    mask_address, BIOS_BASE, BIOS_END, RAM_MASK, RAM_WINDOW_BASE, RAM_WINDOW_END, SCRATCHPAD_BASE,
    SCRATCHPAD_END,
};
use crate::bus::Bus;
use crate::controller::Controller;
use crate::dma::DmaController;
use crate::gpu::Gpu;
use crate::intc::{InterruptController, IRQ_GPU, IRQ_VBLANK};
use crate::ram::Ram;
use crate::scratchpad::Scratchpad;
use crate::timers::Timers;

pub struct MemoryBus {
    pub ram: Ram,
    pub bios: Bios,
    pub scratchpad: Scratchpad,
    pub intc: InterruptController,
    pub timers: Timers,
    pub dma: DmaController,
    pub gpu: Gpu,
    pub controller: Controller,
    pub tty_output: Vec<u8>,
}

impl MemoryBus {
    pub fn new(ram: Ram, bios: Bios, scratchpad: Scratchpad) -> Self {
        Self {
            ram,
            bios,
            scratchpad,
            intc: InterruptController::new(),
            timers: Timers::new(),
            dma: DmaController::new(),
            gpu: Gpu::new(),
            controller: Controller::new(),
            tty_output: Vec::new(),
        }
    }

    pub fn default_bus() -> Self {
        Self::new(Ram::new(), Bios::new(), Scratchpad::new())
    }

    pub fn log_tty_char(&mut self, ch: u8) {
        self.tty_output.push(ch);
    }

    pub fn get_tty_string(&self) -> String {
        String::from_utf8_lossy(&self.tty_output).to_string()
    }

    fn read_io8(&mut self, paddr: u32) -> u8 {
        match paddr {
            0x1F80_1040..=0x1F80_104F => self.controller.read8(paddr),
            0x1F80_1070..=0x1F80_1077 => self.intc.read8(paddr),
            0x1F80_1080..=0x1F80_10F7 => {
                let val32 = self.dma.read32(paddr & !3);
                let shift = (paddr & 3) * 8;
                (val32 >> shift) as u8
            }
            0x1F80_1100..=0x1F80_112F => self.timers.read8(paddr),
            0x1F80_1810..=0x1F80_1817 => {
                let val32 = self.read_io32(paddr & !3);
                let shift = (paddr & 3) * 8;
                (val32 >> shift) as u8
            }
            _ => 0,
        }
    }

    fn read_io16(&mut self, paddr: u32) -> u16 {
        match paddr {
            0x1F80_1040..=0x1F80_104F => self.controller.read16(paddr),
            0x1F80_1070..=0x1F80_1076 => self.intc.read16(paddr),
            0x1F80_1080..=0x1F80_10F6 => {
                let val32 = self.dma.read32(paddr & !3);
                let shift = (paddr & 2) * 8;
                (val32 >> shift) as u16
            }
            0x1F80_1100..=0x1F80_112C => self.timers.read16(paddr),
            0x1F80_1810 => self.gpu.read_gpuread() as u16,
            0x1F80_1814 => self.gpu.read_gpu_stat() as u16,
            _ => 0,
        }
    }

    fn read_io32(&mut self, paddr: u32) -> u32 {
        match paddr {
            0x1F80_1040..=0x1F80_104F => self.controller.read32(paddr),
            0x1F80_1070 | 0x1F80_1074 => self.intc.read32(paddr),
            0x1F80_1080..=0x1F80_10F4 => self.dma.read32(paddr),
            0x1F80_1100..=0x1F80_112C => self.timers.read32(paddr),
            0x1F80_1810 => self.gpu.read_gpuread(),
            0x1F80_1814 => self.gpu.read_gpu_stat(),
            _ => 0,
        }
    }

    fn write_io8(&mut self, paddr: u32, val: u8) {
        match paddr {
            0x1F80_1040..=0x1F80_104F => self.controller.write8(paddr, val),
            0x1F80_1070..=0x1F80_1077 => self.intc.write8(paddr, val),
            0x1F80_1100..=0x1F80_112F => self.timers.write8(paddr, val),
            _ => {}
        }
    }

    fn write_io16(&mut self, paddr: u32, val: u16) {
        match paddr {
            0x1F80_1040..=0x1F80_104F => self.controller.write16(paddr, val),
            0x1F80_1070..=0x1F80_1076 => self.intc.write16(paddr, val),
            0x1F80_1100..=0x1F80_112C => self.timers.write16(paddr, val),
            0x1F80_1810 => self.gpu.write_gp0(val as u32),
            0x1F80_1814 => self.gpu.write_gp1(val as u32),
            _ => {}
        }
    }

    fn write_io32(&mut self, paddr: u32, val: u32) {
        match paddr {
            0x1F80_1040..=0x1F80_104F => self.controller.write32(paddr, val),
            0x1F80_1070 | 0x1F80_1074 => self.intc.write32(paddr, val),
            0x1F80_1080..=0x1F80_10F4 => self.dma.write32(paddr, val, &mut self.intc),
            0x1F80_1100..=0x1F80_112C => self.timers.write32(paddr, val),
            0x1F80_1810 => self.gpu.write_gp0(val),
            0x1F80_1814 => self.gpu.write_gp1(val),
            _ => {}
        }
    }
}

impl Default for MemoryBus {
    fn default() -> Self {
        Self::default_bus()
    }
}

impl Bus for MemoryBus {
    fn read8(&mut self, vaddr: u32) -> u8 {
        let paddr = mask_address(vaddr);
        match paddr {
            RAM_WINDOW_BASE..=RAM_WINDOW_END => self.ram.read8(paddr & RAM_MASK),
            SCRATCHPAD_BASE..=SCRATCHPAD_END => self.scratchpad.read8(paddr - SCRATCHPAD_BASE),
            BIOS_BASE..=BIOS_END => self.bios.read8(paddr - BIOS_BASE),
            0x1F80_1000..=0x1F80_2000 => self.read_io8(paddr),
            _ => 0,
        }
    }

    fn read16(&mut self, vaddr: u32) -> u16 {
        let paddr = mask_address(vaddr);
        match paddr {
            RAM_WINDOW_BASE..=RAM_WINDOW_END => self.ram.read16(paddr & RAM_MASK),
            SCRATCHPAD_BASE..=SCRATCHPAD_END => self.scratchpad.read16(paddr - SCRATCHPAD_BASE),
            BIOS_BASE..=BIOS_END => self.bios.read16(paddr - BIOS_BASE),
            0x1F80_1000..=0x1F80_2000 => self.read_io16(paddr),
            _ => 0,
        }
    }

    fn read32(&mut self, vaddr: u32) -> u32 {
        let paddr = mask_address(vaddr);
        match paddr {
            RAM_WINDOW_BASE..=RAM_WINDOW_END => self.ram.read32(paddr & RAM_MASK),
            SCRATCHPAD_BASE..=SCRATCHPAD_END => self.scratchpad.read32(paddr - SCRATCHPAD_BASE),
            BIOS_BASE..=BIOS_END => self.bios.read32(paddr - BIOS_BASE),
            0x1F80_1000..=0x1F80_2000 => self.read_io32(paddr),
            _ => 0,
        }
    }

    fn write8(&mut self, vaddr: u32, val: u8) {
        let paddr = mask_address(vaddr);
        match paddr {
            RAM_WINDOW_BASE..=RAM_WINDOW_END => self.ram.write8(paddr & RAM_MASK, val),
            SCRATCHPAD_BASE..=SCRATCHPAD_END => {
                self.scratchpad.write8(paddr - SCRATCHPAD_BASE, val)
            }
            BIOS_BASE..=BIOS_END => {} // ROM read only
            0x1F80_1000..=0x1F80_2000 => self.write_io8(paddr, val),
            _ => {}
        }
    }

    fn write16(&mut self, vaddr: u32, val: u16) {
        let paddr = mask_address(vaddr);
        match paddr {
            RAM_WINDOW_BASE..=RAM_WINDOW_END => self.ram.write16(paddr & RAM_MASK, val),
            SCRATCHPAD_BASE..=SCRATCHPAD_END => {
                self.scratchpad.write16(paddr - SCRATCHPAD_BASE, val)
            }
            BIOS_BASE..=BIOS_END => {} // ROM read only
            0x1F80_1000..=0x1F80_2000 => self.write_io16(paddr, val),
            _ => {}
        }
    }

    fn write32(&mut self, vaddr: u32, val: u32) {
        let paddr = mask_address(vaddr);
        match paddr {
            RAM_WINDOW_BASE..=RAM_WINDOW_END => self.ram.write32(paddr & RAM_MASK, val),
            SCRATCHPAD_BASE..=SCRATCHPAD_END => {
                self.scratchpad.write32(paddr - SCRATCHPAD_BASE, val)
            }
            BIOS_BASE..=BIOS_END => {} // ROM read only
            0x1F80_1000..=0x1F80_2000 => self.write_io32(paddr, val),
            _ => {}
        }
    }

    fn step(&mut self, cycles: u32) {
        // 1. Advance timers by cycles, collect IRQs
        let timer_irqs = self.timers.step(cycles);
        if timer_irqs != 0 {
            self.intc.istat |= timer_irqs;
        }

        // 2. Step GPU by cycles, collect VBLANK/GPU IRQs
        let (gpu_irq, vblank_irq) = self.gpu.step(cycles);
        if gpu_irq {
            self.intc.trigger(IRQ_GPU);
        }
        if vblank_irq {
            self.intc.trigger(IRQ_VBLANK);
        }

        // 3. Process DMA transfers if active
        if self.dma.has_active_transfer() {
            self.dma
                .step_dma(&mut self.ram, &mut self.gpu, &mut self.intc);
        }
    }

    fn log_tty_char(&mut self, ch: u8) {
        self.tty_output.push(ch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::Cpu;

    #[test]
    fn test_bios_tty_interceptor() {
        let mut bus = MemoryBus::default();
        let mut cpu = Cpu::new();

        // Set PC to BIOS B0 vector
        cpu.pc = 0xA000_00B0;
        cpu.next_pc = 0xA000_00B4;
        cpu.gpr[9] = 0x3D; // $t1 = 0x3D (putchar)
        cpu.gpr[4] = b'H' as u32; // $a0 = 'H'

        cpu.step(&mut bus);
        assert_eq!(bus.get_tty_string(), "H");

        cpu.pc = 0x8000_00B0;
        cpu.next_pc = 0x8000_00B4;
        cpu.gpr[2] = 0x3D; // $v0 = 0x3D (putchar)
        cpu.gpr[4] = b'i' as u32; // $a0 = 'i'

        cpu.step(&mut bus);
        assert_eq!(bus.get_tty_string(), "Hi");
    }
}
