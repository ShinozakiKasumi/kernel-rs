#![no_std]
#![no_main]
use ulib::{self, cstr, eprintln, puts};

fn main() -> i32 {
    let argc = ulib::argc();
    let mut rc = 0;
    let mut have = false;
    for i in 1..argc as usize {
        let a = ulib::arg_str(i);
        if a == "--help" { puts("usage: rm FILE..."); return 0; }
        have = true;
        let p = cstr::<256>(a);
        let mut st = ulib::Ustat::default();
        if ulib::stat(&p, &mut st) < 0 { eprintln!("rm: {}: not found", a); rc = 1; continue; }
        if st.typ == 2 { eprintln!("rm: {}: is a directory", a); rc = 1; continue; }
        if ulib::unlink(&p) < 0 { eprintln!("rm: {}: remove failed", a); rc = 1; }
    }
    if !have { eprintln!("usage: rm FILE..."); return 1; }
    rc
}
ulib::entry!(main);
