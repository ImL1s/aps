use super::Bus;

/// Lightweight MockBus for isolated CPU instruction and boundary unit testing
pub struct MockBus {
    pub ram: Box<[u8; 0x200000]>, // 2MB Main RAM
    pub bios: Box<[u8; 0x80000]>, // 512KB BIOS
    pub cycles: u64,
    pub write_log: Vec<(u32, u32, u8)>, // (address, value, byte_width)
}

impl MockBus {
    pub fn new() -> Self {
        let ram_vec = vec![0u8; 0x200000];
        let ram_box: Box<[u8; 0x200000]> = ram_vec.into_boxed_slice().try_into().unwrap();
        let bios_vec = vec![0u8; 0x80000];
        let bios_box: Box<[u8; 0x80000]> = bios_vec.into_boxed_slice().try_into().unwrap();
        Self {
            ram: ram_box,
            bios: bios_box,
            cycles: 0,
            write_log: Vec::new(),
        }
    }

    /// Helper to write MIPS machine code instructions into RAM
    pub fn load_code(&mut self, addr: u32, code: &[u8]) {
        let phys = (addr & 0x001F_FFFF) as usize;
        if phys + code.len() <= self.ram.len() {
            self.ram[phys..phys + code.len()].copy_from_slice(code);
        }
    }
}

impl Default for MockBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus for MockBus {
    fn read32(&mut self, addr: u32) -> u32 {
        let phys = addr & 0x1FFF_FFFF;
        if phys < 0x0080_0000 {
            let offset = (phys & 0x001F_FFFF) as usize;
            u32::from_le_bytes(self.ram[offset..offset + 4].try_into().unwrap())
        } else if (0x1FC0_0000..0x1FC8_0000).contains(&phys) {
            let offset = (phys - 0x1FC0_0000) as usize;
            u32::from_le_bytes(self.bios[offset..offset + 4].try_into().unwrap())
        } else {
            0
        }
    }

    fn write32(&mut self, addr: u32, val: u32) {
        let phys = addr & 0x1FFF_FFFF;
        self.write_log.push((addr, val, 4));
        if phys < 0x0080_0000 {
            let offset = (phys & 0x001F_FFFF) as usize;
            let bytes = val.to_le_bytes();
            self.ram[offset..offset + 4].copy_from_slice(&bytes);
        }
    }

    fn read16(&mut self, addr: u32) -> u16 {
        let phys = addr & 0x1FFF_FFFF;
        if phys < 0x0080_0000 {
            let offset = (phys & 0x001F_FFFF) as usize;
            u16::from_le_bytes(self.ram[offset..offset + 2].try_into().unwrap())
        } else {
            0
        }
    }

    fn write16(&mut self, addr: u32, val: u16) {
        let phys = addr & 0x1FFF_FFFF;
        self.write_log.push((addr, val as u32, 2));
        if phys < 0x0080_0000 {
            let offset = (phys & 0x001F_FFFF) as usize;
            self.ram[offset..offset + 2].copy_from_slice(&val.to_le_bytes());
        }
    }

    fn read8(&mut self, addr: u32) -> u8 {
        let phys = addr & 0x1FFF_FFFF;
        if phys < 0x0080_0000 {
            let offset = (phys & 0x001F_FFFF) as usize;
            self.ram[offset]
        } else {
            0
        }
    }

    fn write8(&mut self, addr: u32, val: u8) {
        let phys = addr & 0x1FFF_FFFF;
        self.write_log.push((addr, val as u32, 1));
        if phys < 0x0080_0000 {
            let offset = (phys & 0x001F_FFFF) as usize;
            self.ram[offset] = val;
        }
    }

    fn step(&mut self, cycles: u32) {
        self.cycles += cycles as u64;
    }
}
