#![no_std]
#![no_main]
use ulib::{self, cstr, eprintln, puts};

fn main() -> i32 {
    let argc = ulib::argc();
    let mut rc = 0;
    let mut have = false;
    for i in 1..argc as usize {
        let a = ulib::arg_str(i);
        if a == "--help" { puts("usage: touch FILE..."); return 0; }
        have = true;
        let p = cstr::<256>(a);
        let mut st = ulib::Ustat::default();
        if ulib::stat(&p, &mut st) == 0 { continue; }
        let fd = ulib::open(&p, ulib::O_WRONLY | ulib::O_CREAT);
        if fd < 0 { eprintln!("touch: {}: cannot create", a); rc = 1; continue; }
        ulib::close(fd);
    }
    if !have { eprintln!("usage: touch FILE..."); return 1; }
    rc
}
ulib::entry!(main);
