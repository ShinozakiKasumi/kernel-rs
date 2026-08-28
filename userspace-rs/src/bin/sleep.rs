#![no_std]
#![no_main]
use ulib::{self, eprintln, puts};

fn main() -> i32 {
    if ulib::argc() > 1 && ulib::arg_str(1) == "--help" { puts("usage: sleep SECONDS"); return 0; }
    if ulib::argc() < 2 { eprintln!("usage: sleep SECONDS"); return 1; }
    let s = ulib::atoi(ulib::arg_str(1));
    ulib::sleep_ms(if s < 0 { 0 } else { s as u64 } * 1000);
    0
}
ulib::entry!(main);
