pub mod map;
pub mod memory_bus;
pub mod mock_bus;

/// Core Bus trait interfacing CPU and memory/IO subsystems
pub trait Bus {
    fn read8(&mut self, addr: u32) -> u8;
    fn read16(&mut self, addr: u32) -> u16;
    fn read32(&mut self, addr: u32) -> u32;
    fn write8(&mut self, addr: u32, val: u8);
    fn write16(&mut self, addr: u32, val: u16);
    fn write32(&mut self, addr: u32, val: u32);

    /// Advance subsystems by a given number of CPU clock cycles
    fn step(&mut self, _cycles: u32) {}

    /// Log a TTY output character
    fn log_tty_char(&mut self, _ch: u8) {}
}
