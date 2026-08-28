#![no_std]
#![no_main]
use ulib::{self, puts};

fn main() -> i32 {
    if ulib::argc() > 1 && ulib::arg_str(1) == "--help" { puts("usage: clear"); return 0; }
    ulib::write(1, b"\x1b[2J\x1b[H");
    0
}
ulib::entry!(main);
