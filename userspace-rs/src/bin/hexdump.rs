#![no_std]
#![no_main]
use ulib::{self, cstr, print, println, putchar, eprintln, puts};

const HEX: &[u8; 16] = b"0123456789abcdef";

fn main() -> i32 {
    let argc = ulib::argc();
    let mut path: Option<&str> = None;
    for i in 1..argc as usize {
        let a = ulib::arg_str(i);
        if a == "--help" { puts("usage: hexdump FILE"); return 0; }
        path = Some(a);
    }
    let path = match path { Some(p) => p, None => { eprintln!("usage: hexdump FILE"); return 1; } };
    let p = cstr::<256>(path);
    let fd = ulib::open(&p, ulib::O_RDONLY);
    if fd < 0 { eprintln!("hexdump: {}: cannot open", path); return 1; }
    let mut row = [0u8; 16];
    let mut off: u64 = 0;
    loop {
        let n = ulib::read(fd, &mut row);
        if n < 0 { eprintln!("hexdump: read error"); return 1; }
        if n == 0 { break; }
        let n = n as usize;
        print!("{:08x}  ", off as u32);
        for i in 0..16 {
            if i < n {
                print!("{}{} ", HEX[(row[i] >> 4) as usize] as char, HEX[(row[i] & 15) as usize] as char);
            } else {
                print!("   ");
            }
            if i == 7 { putchar(b' '); }
        }
        print!(" |");
        for i in 0..n {
            let c = row[i];
            putchar(if c >= 32 && c < 127 { c } else { b'.' });
        }
        println!("|");
        off += n as u64;
    }
    ulib::close(fd);
    0
}
ulib::entry!(main);
