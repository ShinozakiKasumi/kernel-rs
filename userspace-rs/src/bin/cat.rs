#![no_std]
#![no_main]
use ulib::{self, cstr, eprintln, puts};

fn main() -> i32 {
    let argc = ulib::argc();
    let mut rc = 0;
    let mut have = false;
    for i in 1..argc as usize {
        let a = ulib::arg_str(i);
        if a == "--help" { puts("usage: cat FILE..."); return 0; }
        have = true;
        let p = cstr::<256>(a);
        let fd = ulib::open(&p, ulib::O_RDONLY);
        if fd < 0 { eprintln!("cat: {}: cannot open", a); rc = 1; continue; }
        let mut buf = [0u8; 4096];
        loop {
            let n = ulib::read(fd, &mut buf);
            if n <= 0 {
                if n < 0 { eprintln!("cat: {}: read error", a); rc = 1; }
                break;
            }
            ulib::write(1, &buf[..n as usize]);
        }
        ulib::close(fd);
    }
    if !have { eprintln!("usage: cat FILE..."); return 1; }
    rc
}
ulib::entry!(main);
