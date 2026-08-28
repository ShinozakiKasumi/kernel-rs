#![no_std]
#![no_main]
use ulib::{self, cstr, eprintln, puts};

fn main() -> i32 {
    let argc = ulib::argc();
    let mut rc = 0;
    let mut have = false;
    for i in 1..argc as usize {
        let a = ulib::arg_str(i);
        if a == "--help" { puts("usage: mkdir DIR..."); return 0; }
        have = true;
        let p = cstr::<256>(a);
        if ulib::mkdir(&p) < 0 { eprintln!("mkdir: {}: failed (exists or bad path)", a); rc = 1; }
    }
    if !have { eprintln!("usage: mkdir DIR..."); return 1; }
    rc
}
ulib::entry!(main);
