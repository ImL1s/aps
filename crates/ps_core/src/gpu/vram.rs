//! 1MB VRAM representation (1024x512 u16 pixels, BGR555 format)

pub const VRAM_WIDTH: u32 = 1024;
pub const VRAM_HEIGHT: u32 = 512;
pub const VRAM_TOTAL_PIXELS: usize = (VRAM_WIDTH * VRAM_HEIGHT) as usize;

#[derive(Clone)]
pub struct VRam {
    pub buffer: Box<[u16; VRAM_TOTAL_PIXELS]>,
}

impl Default for VRam {
    fn default() -> Self {
        let vec = vec![0u16; VRAM_TOTAL_PIXELS];
        let boxed_slice = vec.into_boxed_slice();
        let boxed_array: Box<[u16; VRAM_TOTAL_PIXELS]> = boxed_slice
            .try_into()
            .expect("Failed to allocate VRAM buffer");
        Self {
            buffer: boxed_array,
        }
    }
}

impl VRam {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> u16 {
        let px = x % VRAM_WIDTH;
        let py = y % VRAM_HEIGHT;
        self.buffer[(py * VRAM_WIDTH + px) as usize]
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: u16) {
        let px = x % VRAM_WIDTH;
        let py = y % VRAM_HEIGHT;
        self.buffer[(py * VRAM_WIDTH + px) as usize] = color;
    }
}

pub fn rgb888_to_bgr555(r: u8, g: u8, b: u8) -> u16 {
    let r5 = (r as u16 >> 3) & 0x1F;
    let g5 = (g as u16 >> 3) & 0x1F;
    let b5 = (b as u16 >> 3) & 0x1F;
    r5 | (g5 << 5) | (b5 << 10)
}

pub fn bgr555_to_rgb888(color: u16) -> (u8, u8, u8) {
    let r = ((color & 0x1F) << 3) as u8;
    let g = (((color >> 5) & 0x1F) << 3) as u8;
    let b = (((color >> 10) & 0x1F) << 3) as u8;
    (r, g, b)
}
