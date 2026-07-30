use ps_core::system::PS1System;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct HeadlessSummary {
    pub total_cycles: u64,
    pub tty_output: String,
    pub instruction_count: u64,
}

pub struct HeadlessRunner {
    pub system: PS1System,
    pub max_cycles: Option<u64>,
    pub tty_log_file: Option<File>,
    pub screenshot_path: Option<PathBuf>,
    pub last_tty_pos: usize,
}

impl HeadlessRunner {
    pub fn new(
        bios_path: &Path,
        rom_path: Option<&Path>,
        max_cycles: Option<u64>,
        tty_log: Option<&Path>,
        screenshot_path: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let mut system = PS1System::new();
        if bios_path.exists() {
            system
                .load_bios_file(bios_path)
                .map_err(|e| anyhow::anyhow!(e))?;
        }
        if let Some(rom) = rom_path {
            system
                .load_executable_file(rom)
                .map_err(|e| anyhow::anyhow!(e))?;
        }

        let tty_log_file = match tty_log {
            Some(p) => Some(File::create(p)?),
            None => None,
        };

        Ok(Self {
            system,
            max_cycles,
            tty_log_file,
            screenshot_path: screenshot_path.map(|p| p.to_path_buf()),
            last_tty_pos: 0,
        })
    }

    pub fn run(&mut self) -> anyhow::Result<HeadlessSummary> {
        let batch_size: u32 = 10_000;
        let limit = self.max_cycles.unwrap_or(u64::MAX);

        while self.system.total_cycles < limit {
            let cycles_to_run = ((limit - self.system.total_cycles) as u32).min(batch_size);
            self.system.step_batch(cycles_to_run);

            // Drain TTY characters printed by B0(0x3D) interceptor
            let current_len = self.system.bus.tty_output.len();
            if current_len > self.last_tty_pos {
                let new_bytes = &self.system.bus.tty_output[self.last_tty_pos..current_len];
                std::io::stdout().write_all(new_bytes).ok();
                std::io::stdout().flush().ok();
                if let Some(ref mut f) = self.tty_log_file {
                    f.write_all(new_bytes).ok();
                }
                self.last_tty_pos = current_len;
            }

            // Halt condition check: CPU test completion or self-loop
            if self.system.bus.tty_output.ends_with(b"Done\n")
                || self.system.bus.tty_output.ends_with(b"Done\r\n")
                || self.system.bus.tty_output.ends_with(b"Done.\n")
                || self.system.bus.tty_output.ends_with(b"Done.\r\n")
                || self
                    .system
                    .bus
                    .tty_output
                    .ends_with(b"All tests done: 00000000\n")
                || self
                    .system
                    .bus
                    .tty_output
                    .ends_with(b"All tests done: 00000000\r\n")
                || self.system.cpu.pc == self.system.cpu.next_pc
            {
                break;
            }
        }

        // Save PPM screenshot if requested
        if let Some(ref shot_path) = self.screenshot_path {
            save_ppm_screenshot(&self.system.bus.gpu, shot_path)?;
        }

        Ok(HeadlessSummary {
            total_cycles: self.system.total_cycles,
            tty_output: self.system.bus.get_tty_string(),
            instruction_count: self.system.total_cycles,
        })
    }
}

pub fn save_ppm_screenshot(gpu: &ps_core::gpu::Gpu, path: &Path) -> anyhow::Result<()> {
    let file = File::create(path)?;
    let mut writer = std::io::BufWriter::new(file);
    writeln!(writer, "P3\n1024 512\n255")?;
    for y in 0..512 {
        for x in 0..1024 {
            let pixel = gpu.vram.get_pixel(x, y);
            let r = ((pixel & 0x001F) << 3) as u8;
            let g = (((pixel >> 5) & 0x001F) << 3) as u8;
            let b = (((pixel >> 10) & 0x001F) << 3) as u8;
            writeln!(writer, "{r} {g} {b}")?;
        }
    }
    writer.flush()?;
    Ok(())
}
