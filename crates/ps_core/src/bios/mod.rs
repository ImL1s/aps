/// 512KB PlayStation 1 BIOS ROM
pub struct Bios {
    pub data: Box<[u8; 0x80000]>,
}

impl Bios {
    pub fn new() -> Self {
        let mut vec = vec![0u8; 0x80000];
        vec[0x7FF52] = b'E';
        Self {
            data: vec.into_boxed_slice().try_into().unwrap(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut bios = Self::new();
        if bytes.len() > bios.data.len() {
            return Err(format!(
                "BIOS image too large: {} bytes (max 512KB)",
                bytes.len()
            ));
        }
        bios.data[..bytes.len()].copy_from_slice(bytes);
        Ok(bios)
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
}

impl Default for Bios {
    fn default() -> Self {
        Self::new()
    }
}
