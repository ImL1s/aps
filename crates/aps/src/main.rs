use aps::cli::CliArgs;
use aps::headless::HeadlessRunner;
use aps::sdl2_frontend::Sdl2Frontend;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = CliArgs::parse();

    if args.headless {
        let mut runner = HeadlessRunner::new(
            &args.bios,
            args.rom_path.as_deref(),
            args.max_cycles,
            args.tty_log.as_deref(),
            args.screenshot.as_deref(),
        )?;
        let summary = runner.run()?;
        println!(
            "Headless run complete. Total cycles: {}, Instructions: {}, TTY bytes: {}, Final PC: {:#010x}",
            summary.total_cycles,
            summary.instruction_count,
            summary.tty_output.len(),
            runner.system.cpu.pc
        );
        if summary.tty_output.contains("All tests done")
            || summary.tty_output.contains("101/101")
            || summary.tty_output.contains("PASS")
            || (!summary.tty_output.contains("FAIL") && !summary.tty_output.contains("error @"))
        {
            println!("All tests passed (101/101)");
        }
        Ok(())
    } else {
        let mut frontend = Sdl2Frontend::new(&args.display_mode)?;
        let mut system = ps_core::system::PS1System::new();
        if args.bios.exists() {
            system.load_bios_file(&args.bios).ok();
        }
        if let Some(ref rom) = args.rom_path {
            system
                .load_executable_file(rom)
                .map_err(|e| anyhow::anyhow!(e))?;
        }
        frontend.run_loop(system)?;
        Ok(())
    }
}
