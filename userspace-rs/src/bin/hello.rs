#![no_std]
#![no_main]

use ulib::{self, println};

fn main() -> i32 {
    println!("[user] hello from ring3");
    println!(
        "[user] argc={} argv0={}",
        ulib::argc(),
        if ulib::argc() > 0 { ulib::arg_str(0) } else { "(none)" }
    );
    println!("[user] pid={} uptime={} ms", ulib::getpid(), ulib::uptime_ms());
    7
}

ulib::entry!(main);
