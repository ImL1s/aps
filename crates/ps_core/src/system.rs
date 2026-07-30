//! PS1 System Hardware Core Wrapper

use crate::bios::Bios;
use crate::bus::memory_bus::MemoryBus;
use crate::bus::Bus;
use crate::cpu::Cpu;
use std::fs;
use std::path::Path;

pub struct PS1System {
    pub cpu: Cpu,
    pub bus: MemoryBus,
    pub total_cycles: u64,
}

impl PS1System {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            bus: MemoryBus::default(),
            total_cycles: 0,
        }
    }

    pub fn load_bios_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let bios = Bios::from_bytes(bytes)?;
        self.bus.bios = bios;
        Ok(())
    }

    pub fn load_bios_file(&mut self, path: &Path) -> Result<(), String> {
        let bytes = fs::read(path).map_err(|e| format!("Failed to read BIOS file: {e}"))?;
        self.load_bios_bytes(&bytes)
    }

    pub fn load_executable_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() >= 0x800 && &bytes[0..8] == b"PS-X EXE" {
            let initial_pc = u32::from_le_bytes(bytes[0x10..0x14].try_into().unwrap());
            let initial_gp = u32::from_le_bytes(bytes[0x14..0x18].try_into().unwrap());
            let load_addr = u32::from_le_bytes(bytes[0x18..0x1C].try_into().unwrap());
            let text_size = u32::from_le_bytes(bytes[0x1C..0x20].try_into().unwrap());
            let initial_sp = u32::from_le_bytes(bytes[0x30..0x34].try_into().unwrap());

            let text_data = &bytes[0x800..];
            let copy_len = (text_size as usize).min(text_data.len());
            let phys_addr = (load_addr & 0x1FFF_FFFF) as usize;
            if phys_addr + copy_len <= self.bus.ram.data.len() {
                self.bus.ram.data[phys_addr..phys_addr + copy_len]
                    .copy_from_slice(&text_data[..copy_len]);
            }

            self.cpu.pc = initial_pc;
            self.cpu.next_pc = initial_pc.wrapping_add(4);
            // Clear BEV bit (bit 22 of COP0 Status) so normal exception vector 0x8000_0080 is used
            self.cpu.cop0.status &= !(1 << 22);
            if initial_gp != 0 {
                self.cpu.gpr[28] = initial_gp;
            }
            if initial_sp != 0 {
                self.cpu.gpr[29] = initial_sp;
            }
            // Clear BEV bit in Status register so exceptions route to RAM vector 0x8000_0080
            self.cpu.cop0.status &= !(1 << 22);

            // Initialize Scratchpad 0x1F800000 with argument count/marker (0x20) expected by PSX executables
            self.bus.scratchpad.write32(0, 0x20);
        } else {
            // Raw binary fallback at 0x8001_0000
            let phys_addr = 0x0001_0000;
            let max_capacity = self.bus.ram.data.len().saturating_sub(phys_addr);
            let copy_len = bytes.len().min(max_capacity);
            self.bus.ram.data[phys_addr..phys_addr + copy_len].copy_from_slice(&bytes[..copy_len]);
            self.cpu.pc = 0x8001_0000;
            self.cpu.next_pc = 0x8001_0004;
        }
        Ok(())
    }

    pub fn load_executable_file(&mut self, path: &Path) -> Result<(), String> {
        let bytes = fs::read(path).map_err(|e| format!("Failed to read executable file: {e}"))?;
        self.load_executable_bytes(&bytes)
    }

    pub fn step(&mut self) {
        self.cpu.step(&mut self.bus);
        self.bus.step(1);
        self.total_cycles += 1;
    }

    pub fn step_batch(&mut self, batch_cycles: u32) {
        for _ in 0..batch_cycles {
            self.step();
        }
    }
}

impl Default for PS1System {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_creation_and_step() {
        let mut sys = PS1System::new();
        assert_eq!(sys.total_cycles, 0);
        sys.step();
        assert_eq!(sys.total_cycles, 1);
        sys.step_batch(99);
        assert_eq!(sys.total_cycles, 100);
    }

    #[test]
    fn test_system_load_raw_binary() {
        let mut sys = PS1System::new();
        let raw_code = vec![0x00, 0x00, 0x00, 0x00]; // NOP instruction
        sys.load_executable_bytes(&raw_code).unwrap();
        assert_eq!(sys.cpu.pc, 0x8001_0000);
        assert_eq!(sys.cpu.next_pc, 0x8001_0004);
    }
}
