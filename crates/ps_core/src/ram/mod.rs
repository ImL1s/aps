/// 2MB PlayStation 1 Main RAM
pub struct Ram {
    pub data: Box<[u8; 0x200000]>,
}

impl Ram {
    pub fn new() -> Self {
        let vec = vec![0u8; 0x200000];
        Self {
            data: vec.into_boxed_slice().try_into().unwrap(),
        }
    }

    #[inline(always)]
    pub fn read8(&self, offset: u32) -> u8 {
        self.data[offset as usize]
    }

    #[inline(always)]
    pub fn read16(&self, offset: u32) -> u16 {
        let o = offset as usize;
        u16::from_le_bytes([self.data[o], self.data[o + 1]])
    }

    #[inline(always)]
    pub fn read32(&self, offset: u32) -> u32 {
        let o = offset as usize;
        u32::from_le_bytes([
            self.data[o],
            self.data[o + 1],
            self.data[o + 2],
            self.data[o + 3],
        ])
    }

    #[inline(always)]
    pub fn write8(&mut self, offset: u32, val: u8) {
        self.data[offset as usize] = val;
    }

    #[inline(always)]
    pub fn write16(&mut self, offset: u32, val: u16) {
        let o = offset as usize;
        self.data[o..o + 2].copy_from_slice(&val.to_le_bytes());
    }

    #[inline(always)]
    pub fn write32(&mut self, offset: u32, val: u32) {
        let o = offset as usize;
        self.data[o..o + 4].copy_from_slice(&val.to_le_bytes());
    }
}

impl Default for Ram {
    fn default() -> Self {
        Self::new()
    }
}
