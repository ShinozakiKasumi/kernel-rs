#![no_std]
#![no_main]
use ulib::{self, println, eprintln, puts};

fn main() -> i32 {
    if ulib::argc() > 1 && ulib::arg_str(1) == "--help" { puts("usage: mem"); return 0; }
    let mut si = ulib::Usysinfo::default();
    if ulib::sysinfo(&mut si) < 0 { eprintln!("mem: sysinfo failed"); return 1; }
    println!("total {} KiB  free {} KiB  used {} KiB",
        si.mem_total / 1024, si.mem_free / 1024, (si.mem_total - si.mem_free) / 1024);
    println!("procs {}   uptime {} ms", si.nproc, si.uptime_ms);
    0
}
ulib::entry!(main);
