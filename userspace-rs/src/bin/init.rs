#![no_std]
#![no_main]

use ulib::{self, cstr, println, puts};

fn cstr_of(b: &[u8]) -> &str {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    core::str::from_utf8(&b[..n]).unwrap_or("?")
}

fn main() -> i32 {
    let mut u = ulib::Utsname::default();
    if ulib::uname(&mut u) == 0 {
        println!(
            "[init] {} {} ({}) — userspace up",
            cstr_of(&u.sysname),
            cstr_of(&u.release),
            cstr_of(&u.machine)
        );
    }

    let sh_path = cstr::<64>("/bin/sh");
    let argv = [sh_path.as_ptr(), core::ptr::null()];
    loop {
        let pid = ulib::spawn(&sh_path, &argv);
        if pid < 0 {
            puts("[init] cannot spawn /bin/sh");
            loop {
                ulib::u_yield();
            }
        }
        let mut status: i32 = 0;
        ulib::waitpid(pid, &mut status);
        println!("[init] /bin/sh exited ({}), restarting", status);
        ulib::sleep_ms(200);
    }
}

ulib::entry!(main);
