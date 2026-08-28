#![no_std]
#![no_main]
use ulib::{self, cstr, println, eprintln, puts};

fn cstr_of(b: &[u8]) -> &str {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    core::str::from_utf8(&b[..n]).unwrap_or("?")
}

fn list(path: &str) -> i32 {
    let p = cstr::<256>(path);
    let mut st = ulib::Ustat::default();
    if ulib::stat(&p, &mut st) < 0 {
        eprintln!("ls: {}: not found", path);
        return 1;
    }
    if st.typ != 2 {
        println!("{}", path);
        return 0;
    }
    let mut d = ulib::Udirent::default();
    let mut i = 0u32;
    while ulib::getdents(&p, i, &mut d) != 0 {
        let n = cstr_of(&d.name);
        println!("{}{}", n, if d.typ == 2 { "/" } else { "" });
        i += 1;
    }
    0
}

fn main() -> i32 {
    let argc = ulib::argc();
    let mut rc = 0;
    let mut files = 0;
    for i in 1..argc as usize {
        let a = ulib::arg_str(i);
        if a == "--help" {
            puts("usage: ls [DIR...]   (default: current directory)");
            return 0;
        }
        if list(a) != 0 { rc = 1; }
        files += 1;
    }
    if files == 0 { rc = list("."); }
    rc
}
ulib::entry!(main);
