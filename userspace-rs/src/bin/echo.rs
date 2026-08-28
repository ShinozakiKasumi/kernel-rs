#![no_std]
#![no_main]
use ulib::{self, putchar, puts};

fn main() -> i32 {
    let argc = ulib::argc();
    let mut newline = true;
    let mut i = 1usize;
    while i < argc as usize {
        if ulib::arg_str(i) == "--help" { puts("usage: echo [-n] [TEXT...]"); return 0; }
        i += 1;
    }
    i = 1;
    if i < argc as usize && ulib::arg_str(i) == "-n" { newline = false; i += 1; }
    while i < argc as usize {
        let a = ulib::arg_str(i);
        ulib::write(1, a.as_bytes());
        if i + 1 < argc as usize { putchar(b' '); }
        i += 1;
    }
    if newline { putchar(b'\n'); }
    0
}
ulib::entry!(main);
