//! Interactive shell over the serial console / PS/2 keyboard.

use crate::fs::vfs;
use crate::mm::pmm::{self, PAGE_SIZE};
use crate::{kbd, uart};

const LINE_MAX: usize = 128;
const MAX_ARGS: usize = 8;

fn prompt() {
    crate::klog!("shizuku> ");
}

fn readline(line: &mut [u8; LINE_MAX]) -> usize {
    let mut n = 0usize;
    loop {
        let c = kbd::getchar();
        if c < 0 {
            crate::io::sti();
            crate::io::hlt(); // idle until any IRQ
            continue;
        }
        let c = c as u8;
        if c == b'\n' {
            uart::write_bytes(b"\n");
            line[n] = 0;
            return n;
        }
        if c == 8 {
            if n > 0 {
                n -= 1;
                uart::write_bytes(b"\x08 \x08");
            }
            continue;
        }
        if n + 1 < line.len() - 1 {
            line[n] = c;
            n += 1;
            uart::putc(c); // local echo
        }
    }
}

/* --- built-in commands ------------------------------------------------------ */

fn cmd_help() {
    crate::klog!("commands: help clear ps mem ls cat run\n");
    crate::klog!("  clear          clear the screen (ANSI)\n");
    crate::klog!("  ps             list threads/processes\n");
    crate::klog!("  mem            physical memory stats\n");
    crate::klog!("  ls             list files in /\n");
    crate::klog!("  cat <file>     print file contents\n");
    crate::klog!("  run <file>     load ELF from tmpfs and run as user process\n");
}

fn cmd_ps() {
    crate::sched::list(&mut |args| crate::log::_klog(args));
}

fn cmd_mem() {
    let free = pmm::free_count() as u64;
    let total = pmm::total_count() as u64;
    crate::klog!(
        "memory: {}/{} pages free ({} MiB used, {} MiB total)\n",
        free,
        total,
        (total - free) * 4 / 1024,
        total * 4 / 1024
    );
}

fn cmd_ls() {
    let mut i = 0usize;
    while let Some(de) = vfs::list("/", i) {
        let end = de.name.iter().position(|&b| b == 0).unwrap_or(de.name.len());
        crate::klog!(
            "{:8}  {}\n",
            de.size,
            core::str::from_utf8(&de.name[..end]).unwrap_or("?")
        );
        i += 1;
    }
}

fn cmd_cat(path: &str) {
    let size = vfs::path_size(path);
    if size < 0 {
        crate::klog!("cat: {}: not found\n", path);
        return;
    }
    let buf_pa = pmm::alloc_pages(
        (size as usize + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize,
        PAGE_SIZE as usize,
    );
    let buf = unsafe { core::slice::from_raw_parts_mut(pmm::pa_to_va(buf_pa), size as usize) };
    let got = vfs::read_path(path, 0, buf);
    for i in 0..got.max(0) as usize {
        uart::putc(buf[i]);
    }
    uart::putc(b'\n');
}

fn cmd_run(path: &str) {
    let size = vfs::path_size(path);
    if size < 0 {
        crate::klog!("run: {}: not found\n", path);
        return;
    }
    let npages = (size as usize + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;
    let pa = pmm::alloc_pages(npages, PAGE_SIZE as usize);
    let buf = unsafe { core::slice::from_raw_parts_mut(pmm::pa_to_va(pa), size as usize) };
    if vfs::read_path(path, 0, buf) != size {
        crate::klog!("run: {}: read error\n", path);
        return;
    }
    let tid = crate::proc::spawn_elf(path, buf, &[]);
    if tid < 0 {
        crate::klog!("run: {}: spawn failed (bad ELF?)\n", path);
    } else {
        crate::klog!("run: started '{}' as tid {}\n", path, tid);
    }
}

/* --- dispatch --------------------------------------------------------------- */

fn dispatch(argv: &[&[u8]]) {
    let cmd = argv[0];
    match cmd {
        b"help" => cmd_help(),
        b"clear" => uart::write_bytes(b"\x1b[2J\x1b[H"),
        b"ps" => cmd_ps(),
        b"mem" => cmd_mem(),
        b"ls" => cmd_ls(),
        b"cat" if argv.len() == 2 => {
            cmd_cat(core::str::from_utf8(argv[1]).unwrap_or(""));
        }
        b"run" if argv.len() == 2 => {
            cmd_run(core::str::from_utf8(argv[1]).unwrap_or(""));
        }
        _ => crate::klog!(
            "unknown command '{}' -- try 'help'\n",
            core::str::from_utf8(cmd).unwrap_or("?")
        ),
    }
}

pub extern "C" fn shell_main(_arg: *mut core::ffi::c_void) {
    let mut line = [0u8; LINE_MAX];

    crate::klog!("\n=== shizuku shell ===\n");
    crate::klog!("type 'help' for commands\n");
    loop {
        prompt();
        let n = readline(&mut line);
        if n == 0 {
            continue;
        }
        // tokenize in place
        let mut argv: [&[u8]; MAX_ARGS] = [&[]; MAX_ARGS];
        let mut argc = 0usize;
        let mut p = 0usize;
        while p < n && argc < MAX_ARGS {
            while p < n && line[p] == b' ' {
                p += 1;
            }
            if p >= n {
                break;
            }
            let start = p;
            while p < n && line[p] != b' ' {
                p += 1;
            }
            argv[argc] = &line[start..p];
            argc += 1;
        }
        if argc > 0 {
            dispatch(&argv[..argc]);
        }
    }
}
