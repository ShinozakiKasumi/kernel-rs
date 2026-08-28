#![no_std]
#![no_main]
use ulib::{self, puts};

fn main() -> i32 {
    if ulib::argc() > 1 && ulib::arg_str(1) == "--help" { puts("usage: true"); }
    0
}
ulib::entry!(main);
