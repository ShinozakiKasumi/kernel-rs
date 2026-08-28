#![no_std]
#![no_main]
use ulib::{self, cstr, eprintln, puts};

fn main() -> i32 {
    let argc = ulib::argc();
    let mut src: Option<&str> = None;
    let mut dst: Option<&str> = None;
    for i in 1..argc as usize {
        let a = ulib::arg_str(i);
        if a == "--help" { puts("usage: cp SRC DST"); return 0; }
        if src.is_none() { src = Some(a); }
        else if dst.is_none() { dst = Some(a); }
        else { eprintln!("cp: too many arguments"); return 1; }
    }
    let (src, dst) = match (src, dst) {
        (Some(s), Some(d)) => (s, d),
        _ => { eprintln!("usage: cp SRC DST"); return 1; }
    };
    let sp = cstr::<256>(src);
    let dp = cstr::<256>(dst);
    let mut st = ulib::Ustat::default();
    if ulib::stat(&sp, &mut st) < 0 || st.typ != 1 {
        eprintln!("cp: {}: not a file", src); return 1;
    }
    let rin = ulib::open(&sp, ulib::O_RDONLY);
    if rin < 0 { eprintln!("cp: {}: cannot open", src); return 1; }
    if ulib::stat(&dp, &mut st) == 0 && st.typ != 1 {
        eprintln!("cp: {}: is a directory", dst); return 1;
    }
    let rout = ulib::open(&dp, ulib::O_WRONLY | ulib::O_CREAT);
    if rout < 0 { eprintln!("cp: {}: cannot create", dst); return 1; }
    let mut buf = [0u8; 512];
    let mut rc = 0;
    loop {
        let n = ulib::read(rin, &mut buf);
        if n <= 0 { break; }
        if ulib::write(rout, &buf[..n as usize]) != n {
            eprintln!("cp: {}: write error", dst); rc = 1; break;
        }
    }
    ulib::close(rin);
    ulib::close(rout);
    rc
}
ulib::entry!(main);
