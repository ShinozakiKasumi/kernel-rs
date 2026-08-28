#![no_std]
#![no_main]
use ulib::{self, print, println, putchar, eprintln, puts};

fn cstr_of(b: &[u8]) -> &str {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    core::str::from_utf8(&b[..n]).unwrap_or("?")
}

fn main() -> i32 {
    let argc = ulib::argc();
    let (mut s, mut r, mut m) = (true, true, true);
    for i in 1..argc as usize {
        let a = ulib::arg_str(i);
        if a == "--help" { puts("usage: uname [-s] [-r] [-m] [-a]"); return 0; }
        if a == "-s" && argc == 2 { s = true; r = false; m = false; }
        else if a == "-r" && argc == 2 { r = true; s = false; m = false; }
        else if a == "-m" && argc == 2 { m = true; s = false; r = false; }
    }
    let mut u = ulib::Utsname::default();
    if ulib::uname(&mut u) < 0 { eprintln!("uname: failed"); return 1; }
    let mut sp = false;
    if s { print!("{}", cstr_of(&u.sysname)); sp = true; }
    if r { print!("{}{}", if sp { " " } else { "" }, cstr_of(&u.release)); sp = true; }
    if m { print!("{}{}", if sp { " " } else { "" }, cstr_of(&u.machine)); }
    putchar(b'\n');
    0
}
ulib::entry!(main);
