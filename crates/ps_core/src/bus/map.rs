/// Mask MIPS virtual address to 512MB physical address space (KUSEG, KSEG0, KSEG1).
/// Regions KUSEG (0x00000000..0x7FFFFFFF), KSEG0 (0x80000000..0x9FFFFFFF),
/// and KSEG1 (0xA0000000..0xBFFFFFFF) all map to the lower 512MB of physical memory.
#[inline(always)]
pub fn mask_address(vaddr: u32) -> u32 {
    vaddr & 0x1FFF_FFFF
}

pub const RAM_WINDOW_BASE: u32 = 0x0000_0000;
pub const RAM_WINDOW_END: u32 = 0x007F_FFFF;
pub const RAM_SIZE: u32 = 0x0020_0000; // 2MB
pub const RAM_MASK: u32 = 0x001F_FFFF; // 2MB mirroring mask

pub const SCRATCHPAD_BASE: u32 = 0x1F80_0000;
pub const SCRATCHPAD_END: u32 = 0x1F80_03FF;
pub const SCRATCHPAD_SIZE: u32 = 0x0000_0400; // 1KB

pub const BIOS_BASE: u32 = 0x1FC0_0000;
pub const BIOS_END: u32 = 0x1FC7_FFFF;
pub const BIOS_SIZE: u32 = 0x0008_0000; // 512KB

pub const IO_BASE: u32 = 0x1F80_1000;
pub const IO_END: u32 = 0x1F80_2000;
