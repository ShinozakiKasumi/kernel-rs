#![no_std]
#![no_main]
use ulib::{self, eprintln, puts};

fn main() -> i32 {
    let mut buf = [0u8; 256];
    if ulib::getcwd(&mut buf) < 0 { eprintln!("pwd: failed"); return 1; }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    puts(core::str::from_utf8(&buf[..n]).unwrap_or("?"));
    0
}
ulib::entry!(main);
