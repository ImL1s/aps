//! PS1 GPU Subsystem & Software Rasterizer Master Struct

pub mod gp0;
pub mod gp1;
pub mod rasterizer;
pub mod vram;

use gp0::Gp0;
use gp1::Gp1;
use vram::VRam;

#[derive(Default)]
pub struct Gpu {
    pub vram: VRam,
    pub gp0: Gp0,
    pub gp1: Gp1,
    pub cycles_accum: u32,
    pub scanline: u32,
}

impl Gpu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.gp0.reset();
        self.gp1.reset();
        self.cycles_accum = 0;
        self.scanline = 0;
    }

    /// Advance GPU state by CPU cycles.
    /// Returns (gpu_irq, vblank_irq).
    pub fn step(&mut self, cycles: u32) -> (bool, bool) {
        let mut vblank_irq = false;
        let gpu_irq = self.gp1.irq_requested;

        // Roughly 3413 cycles per scanline in NTSC mode (263 total lines)
        self.cycles_accum += cycles;
        if self.cycles_accum >= 3413 {
            self.cycles_accum -= 3413;
            let prev_scanline = self.scanline;
            self.scanline = (self.scanline + 1) % 263;

            // VBLANK triggers when entering scanline 240
            if prev_scanline < 240 && self.scanline >= 240 {
                vblank_irq = true;
            }
        }

        (gpu_irq, vblank_irq)
    }

    pub fn read_gpu_stat(&self) -> u32 {
        self.gp1.get_gpustat()
    }

    pub fn read_gpuread(&self) -> u32 {
        0
    }

    pub fn write_gp0(&mut self, val: u32) {
        self.gp0.write_word(val, &mut self.vram);
    }

    pub fn write_gp1(&mut self, val: u32) {
        let opcode = (val >> 24) & 0xFF;
        if opcode == 0x00 {
            self.gp0.reset();
        } else if opcode == 0x01 {
            self.gp0.cmd_buf.clear();
            self.gp0.state = gp0::Gp0State::Command;
        }
        self.gp1.process_command(val);
    }

    pub fn render_vram_to_argb32(&self, frame_buffer: &mut [u32]) {
        let width = 1024;
        let height = 512;
        let len = (width * height).min(frame_buffer.len());
        for (i, pixel_out) in frame_buffer[..len].iter_mut().enumerate() {
            let px = self.vram.buffer[i];
            let r = ((px & 0x001F) as u32) << 3;
            let g = (((px >> 5) & 0x001F) as u32) << 3;
            let b = (((px >> 10) & 0x001F) as u32) << 3;
            *pixel_out = 0xFF00_0000 | (r << 16) | (g << 8) | b;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_gp1_reset_gpustat() {
        let mut gpu = Gpu::new();
        gpu.write_gp1(0x0000_0000); // GP1 Reset
        let stat = gpu.read_gpu_stat();
        // Bits 26, 27, 28 set (0x1400_0000), Display Enable bit 23 set (0x0080_0000)
        assert_eq!(stat & 0x1480_0000, 0x1480_0000);
    }

    #[test]
    fn test_gpu_cpu_to_vram_transfer() {
        let mut gpu = Gpu::new();

        // 1. Send GP0(0xA0) - CPU to VRAM
        gpu.write_gp0(0xA000_0000);
        // 2. Dst X=10, Y=20
        gpu.write_gp0((20 << 16) | 10);
        // 3. Width=2, Height=1 (2 pixels = 1 word)
        gpu.write_gp0((1 << 16) | 2);

        // 4. Send pixel data word: low pixel 0x001F (Red), high pixel 0x03E0 (Green)
        gpu.write_gp0(0x03E0_001F);

        assert_eq!(gpu.vram.get_pixel(10, 20), 0x001F);
        assert_eq!(gpu.vram.get_pixel(11, 20), 0x03E0);
    }

    #[test]
    fn test_gpu_draw_triangle_rasterization() {
        let mut gpu = Gpu::new();

        // Send Flat Triangle GP0(0x20)
        // Word 0: Color (255, 0, 0) -> Red
        gpu.write_gp0(0x2000_00FF);
        // Word 1: V0 (x=10, y=10)
        gpu.write_gp0((10 << 16) | 10);
        // Word 2: V1 (x=30, y=10)
        gpu.write_gp0((10 << 16) | 30);
        // Word 3: V2 (x=10, y=30)
        gpu.write_gp0((30 << 16) | 10);

        // Check pixel inside triangle (15, 15) is non-zero
        assert_ne!(gpu.vram.get_pixel(15, 15), 0);
    }
}
