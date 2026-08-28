#![no_std]
#![no_main]
use ulib::{self, println, puts};

fn cstr_of(b: &[u8]) -> &str {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    core::str::from_utf8(&b[..n]).unwrap_or("?")
}

fn main() -> i32 {
    if ulib::argc() > 1 && ulib::arg_str(1) == "--help" {
        puts("usage: ps");
        return 0;
    }
    println!("  TID  NAME                 STATE");
    let mut p = ulib::Uproc::default();
    let mut i = 0u32;
    while ulib::nextproc(i, &mut p) != 0 {
        let name = cstr_of(&p.name);
        println!("  {:<4} {:<20} {}", p.tid, name, if p.state == 1 { "zombie" } else { "runnable" });
        i += 1;
    }
    0
}
ulib::entry!(main);
