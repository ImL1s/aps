use clap::Parser;
use std::path::PathBuf;

/// Clean-Room PlayStation 1 (PS1) Emulator CLI
#[derive(Parser, Debug, Clone)]
#[command(
    name = "aps",
    version = "0.1.0",
    author = "APS Emulator Team",
    about = "Clean-room PlayStation 1 (MIPS R3000A) Emulator in Rust"
)]
pub struct CliArgs {
    /// Path to PS1 ROM or Executable (.exe, .psx, .bin)
    #[arg(value_name = "ROM_PATH")]
    pub rom_path: Option<PathBuf>,

    /// Path to PS1 BIOS ROM image (512KB, default: bios/SCPH1001.BIN)
    #[arg(long, default_value = "bios/SCPH1001.BIN")]
    pub bios: PathBuf,

    /// Execute in headless mode (no SDL2 window or GUI)
    #[arg(long, default_value_t = false)]
    pub headless: bool,

    /// Maximum CPU cycles to execute before auto-terminating
    #[arg(long)]
    pub max_cycles: Option<u64>,

    /// Save framebuffer screenshot image on exit (.ppm)
    #[arg(long)]
    pub screenshot: Option<PathBuf>,

    /// Save serial / BIOS TTY log output to file
    #[arg(long)]
    pub tty_log: Option<PathBuf>,

    /// Display mode for SDL2 frontend ("windowed", "vram_debug")
    #[arg(long, default_value = "windowed")]
    pub display_mode: String,
}
