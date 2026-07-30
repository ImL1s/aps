
use ps_core::system::PS1System;
use std::path::Path;

fn main() {
    let mut sys = PS1System::new();
    sys.load_executable_file(Path::new("tests/roms/psxtest_cpu.exe")).unwrap();
    println!("Start PC: {:#010X}", sys.cpu.pc);
    for i in 0..100 {
        println!("Step {:2}: PC={:#010X}, r2={:#010X}, r4={:#010X}, r9={:#010X}, r31={:#010X}", 
            i, sys.cpu.pc, sys.cpu.gpr[2], sys.cpu.gpr[4], sys.cpu.gpr[9], sys.cpu.gpr[31]);
        sys.step();
    }
}
