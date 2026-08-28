#![no_std]
#![no_main]

//! sh: interactive user shell (Rust port of userspace/sh/sh.c).

use ulib::{self, cstr, eprintln, print, println, puts};

const LINE_MAX: usize = 200;
const ARG_MAX: usize = 12;

static mut LAST_DIR: [u8; 96] = [0; 96];

fn prompt() {
    let mut cwd = [0u8; 96];
    if ulib::getcwd(&mut cwd) < 0 {
        print!("? # ");
        return;
    }
    let s = core::str::from_utf8(&cwd).unwrap_or("?");
    let s = s.trim_end_matches('\0');
    print!("{} # ", s);
}

/// Read one edited line from stdin (fd0); returns length, or -1 on Ctrl-D.
fn readline(buf: &mut [u8]) -> i32 {
    let mut n = 0usize;
    loop {
        let mut c = 0u8;
        if ulib::read(0, core::slice::from_mut(&mut c)) != 1 {
            continue;
        }
        match c {
            b'\n' => {
                ulib::write(1, b"\n");
                break;
            }
            8 | 127 => {
                if n > 0 {
                    n -= 1;
                    ulib::write(1, b"\x08 \x08");
                }
            }
            4 if n == 0 => return -1,
            _ => {
                if n + 1 < buf.len() {
                    buf[n] = c;
                    n += 1;
                    ulib::write(1, &[c]);
                }
            }
        }
    }
    buf[n] = 0;
    n as i32
}

/// Split `line` on spaces, NUL-terminating each token in place.
/// Returns (token count, (start,end) pairs into `line`).
fn tokenize(line: &mut [u8]) -> (usize, [(usize, usize); ARG_MAX]) {
    let mut spans = [(0usize, 0usize); ARG_MAX];
    let mut n = 0;
    let mut i = 0;
    let bytes = line.len();
    while n + 1 < ARG_MAX {
        while i < bytes && (line[i] == b' ' || line[i] == b'\t') {
            i += 1;
        }
        if i >= bytes || line[i] == 0 {
            break;
        }
        let start = i;
        while i < bytes && line[i] != 0 && line[i] != b' ' && line[i] != b'\t' {
            i += 1;
        }
        let end = i;
        if i < bytes && line[i] != 0 {
            line[i] = 0;
            i += 1;
        }
        spans[n] = (start, end);
        n += 1;
    }
    (n, spans)
}

fn run_builtin(args: &[&[u8]]) -> i32 {
    let name = args[0];
    if name == b"exit" {
        let code = if args.len() > 1 {
            ulib::atoi(core::str::from_utf8(args[1]).unwrap_or("0"))
        } else {
            0
        };
        ulib::exit(code);
    }
    if name == b"cd" {
        let dst = if args.len() > 1 { args[1] } else { b"/" };
        let dst = if dst == b"-" {
            unsafe { &LAST_DIR[..] }
        } else {
            dst
        };
        let mut prev = [0u8; 96];
        ulib::getcwd(&mut prev);
        let d = cstr::<128>(core::str::from_utf8(dst).unwrap_or("/").trim_end_matches('\0'));
        if ulib::chdir(&d) < 0 {
            eprintln!("cd: {}: no such directory", core::str::from_utf8(dst).unwrap_or("?"));
            return 1;
        }
        unsafe {
            LAST_DIR = prev;
        }
        return 0;
    }
    if name == b"help" {
        puts("builtins: cd exit help   everything else runs from /bin");
        return 0;
    }
    -1
}

fn main() -> i32 {
    puts("shizuku shell — 'help' for builtins, 'ls /bin' for commands");

    let mut line = [0u8; LINE_MAX];
    loop {
        prompt();
        let n = readline(&mut line);
        if n < 0 {
            puts("bye");
            return 0;
        }
        if n == 0 {
            continue;
        }

        let base = line.as_ptr();
        let (argc, spans) = tokenize(&mut line);
        if argc == 0 {
            continue;
        }

        // token arguments as byte slices + raw NUL-terminated pointers
        let mut args: [&[u8]; ARG_MAX] = [&[]; ARG_MAX];
        let mut arg_ptrs = [core::ptr::null::<u8>(); ARG_MAX];
        for i in 0..argc {
            let (s, e) = spans[i];
            args[i] = &line[s..e];
            unsafe {
                arg_ptrs[i] = base.add(s);
            }
        }
        let args = &args[..argc];

        if run_builtin(args) >= 0 {
            continue;
        }

        // external: /bin/<name> or literal path
        let mut path = [0u8; 128];
        let name = args[0];
        let plen = if name.contains(&b'/') {
            let n = name.len().min(path.len() - 1);
            path[..n].copy_from_slice(&name[..n]);
            n
        } else {
            path[..5].copy_from_slice(b"/bin/");
            let n = name.len().min(path.len() - 6);
            path[5..5 + n].copy_from_slice(&name[..n]);
            5 + n
        };
        path[plen] = 0;

        let pid = ulib::spawn(&path[..plen + 1], &arg_ptrs[..argc + 1]);
        if pid < 0 {
            eprintln!("sh: {}: command not found", core::str::from_utf8(name).unwrap_or("?"));
            continue;
        }
        let mut status = 0i32;
        ulib::waitpid(pid, &mut status);
        if status != 0 {
            println!("[exit {}]", status);
        }
    }
}

ulib::entry!(main);
