#![no_std]
#![no_main]
use ulib::{self, cstr, eprintln, puts};

fn main() -> i32 {
    let argc = ulib::argc();
    let mut src: Option<&str> = None;
    let mut dst: Option<&str> = None;
    for i in 1..argc as usize {
        let a = ulib::arg_str(i);
        if a == "--help" { puts("usage: mv OLD NEW"); return 0; }
        if src.is_none() { src = Some(a); }
        else if dst.is_none() { dst = Some(a); }
        else { eprintln!("mv: too many arguments"); return 1; }
    }
    let (src, dst) = match (src, dst) {
        (Some(s), Some(d)) => (s, d),
        _ => { eprintln!("usage: mv OLD NEW"); return 1; }
    };
    let sp = cstr::<256>(src);
    let dp = cstr::<256>(dst);
    if ulib::rename(&sp, &dp) < 0 {
        eprintln!("mv: cannot rename {} -> {}", src, dst);
        return 1;
    }
    0
}
ulib::entry!(main);
