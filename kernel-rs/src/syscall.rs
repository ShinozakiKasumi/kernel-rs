//! int 0x80 system calls.
//!
//! ABI: rax = syscall number, args = rdi, rsi, rdx, r10, r8, r9.
//! Return value in rax (written back into the trap frame).
//! Errors: small negative values; -ENOSYS for unimplemented.
//!
//! User pointers are trusted after a minimal canonical/NULL sanity check —
//! acceptable for a teaching kernel in ring-0-friendly hardware, but is the
//! first thing to harden when going multi-user.

use core::ptr;

use crate::fs::vfs;
use crate::idt::IrqFrame;
use crate::mm::pmm::{self, PAGE_SIZE};
use crate::mm::vmm::{self, PTE_NX, PTE_RW, PTE_US};
use crate::sched::{self, ThreadState, Ufd, UFD_FIRST, UFD_MAX};

pub const SYSCALL_VECTOR: u8 = 0x80;
pub const ENOSYS: i64 = 38;

pub const SYS_WRITE: u64 = 0;
pub const SYS_EXIT: u64 = 1;
pub const SYS_READ: u64 = 2;
pub const SYS_OPEN: u64 = 3;
pub const SYS_CLOSE: u64 = 4;
pub const SYS_STAT: u64 = 5;
pub const SYS_MKDIR: u64 = 6;
pub const SYS_UNLINK: u64 = 7;
pub const SYS_RENAME: u64 = 8;
pub const SYS_GETDENTS: u64 = 9;
pub const SYS_GETCWD: u64 = 10;
pub const SYS_CHDIR: u64 = 11;
pub const SYS_SPAWN: u64 = 12;
pub const SYS_WAITPID: u64 = 13;
pub const SYS_GETPID: u64 = 14;
pub const SYS_SBRK: u64 = 15;
pub const SYS_SLEEP_MS: u64 = 16;
pub const SYS_UPTIME_MS: u64 = 17;
pub const SYS_UNAME: u64 = 18;
pub const SYS_SYSINFO: u64 = 19;
pub const SYS_YIELD: u64 = 20;
pub const SYS_NEXTPROC: u64 = 21;
pub const SYS_FB_IOCTL: u64 = 22; // still unimplemented
pub const NR_SYSCALLS: u64 = 23;

const TICK_MS: u64 = 10;

/* Shared UAPI structs (kept ABI-identical to the userspace crate). */

#[repr(C)]
pub struct Ustat {
    pub size: u64,
    pub typ: u32, // 1 = file, 2 = dir
}

#[repr(C)]
pub struct Udirent {
    pub name: [u8; 56],
    pub typ: u32, // 1 = file, 2 = dir
    pub _pad: u32,
}

#[repr(C)]
pub struct Utsname {
    pub sysname: [u8; 32],
    pub release: [u8; 32],
    pub machine: [u8; 32],
}

#[repr(C)]
pub struct Usysinfo {
    pub mem_total: u64, // bytes
    pub mem_free: u64,  // bytes
    pub nproc: u32,
    pub uptime_ms: u64,
}

#[repr(C)]
pub struct Uproc {
    pub tid: i32,
    pub name: [u8; 24],
    pub state: u32, // 0 runn, 1 zombie
}

/* ---- helpers ------------------------------------------------------------- */

fn bad_uaddr(p: u64) -> bool {
    p == 0 || p >= 0x0000_8000_0000_0000 // kernel half / noncanonical
}

/// Copy a user string into a kernel buffer; returns length or -1.
fn copy_ustr(uptr: u64, out: &mut [u8]) -> i64 {
    if bad_uaddr(uptr) {
        return -1;
    }
    let s = uptr as *const u8;
    let mut i = 0usize;
    while i + 1 < out.len() {
        let c = unsafe { *s.add(i) };
        if c == 0 {
            out[i] = 0;
            return i as i64;
        }
        out[i] = c;
        i += 1;
    }
    if unsafe { *s.add(i) } == 0 {
        out[i] = 0;
        return i as i64;
    }
    -1 // unterminated
}

/// Make an absolute path from the caller's cwd and argument.
fn abs_path(uptr: u64, out: &mut [u8]) -> i64 {
    let mut raw = [0u8; 256];
    if copy_ustr(uptr, &mut raw) < 0 {
        return -1;
    }
    let raw_len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let cwd = sched::current().cwd_str();
    let mut n = 0usize;
    if raw[0] == b'/' {
        n = raw_len.min(out.len() - 1);
        out[..n].copy_from_slice(&raw[..n]);
    } else {
        let cwd_b = cwd.as_bytes();
        let cn = cwd_b.len().min(out.len() - 1);
        out[..cn].copy_from_slice(&cwd_b[..cn]);
        n = cn;
        if n + 1 < out.len() && n > 0 && out[n - 1] != b'/' {
            out[n] = b'/';
            n += 1;
        }
        let rem = (out.len() - n - 1).min(raw_len);
        out[n..n + rem].copy_from_slice(&raw[..rem]);
        n += rem;
    }
    out[out.len() - 1] = 0;
    if n < out.len() {
        out[n] = 0;
    }
    0
}

/// View a NUL-terminated kernel buffer as &str.
fn buf_str(buf: &[u8]) -> &str {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    core::str::from_utf8(&buf[..end]).unwrap_or("")
}

/* ---- fd table plumbing ---------------------------------------------------- */

fn fd_get(t: &mut sched::Thread, fd: usize) -> Option<&mut Ufd> {
    if !(UFD_FIRST..UFD_MAX).contains(&fd) {
        return None;
    }
    let u = &mut t.fds[fd];
    if u.node >= 0 {
        Some(u)
    } else {
        None
    }
}

/* ---- file syscalls -------------------------------------------------------- */

fn sys_write(f: &IrqFrame) -> i64 {
    let fd = f.rdi as i32;
    let buf = f.rsi;
    let mut len = f.rdx;
    if bad_uaddr(buf) && len > 0 {
        return -1;
    }
    if len > 1 << 20 {
        len = 1 << 20; // sanity cap
    }

    if fd == 1 || fd == 2 {
        let b = buf as *const u8;
        let mut vt = sched::current().vt;
        if vt < 0 && crate::vterm::console_up() {
            vt = 0; // legacy -> GUI console 0
        }
        for i in 0..len as usize {
            let c = unsafe { *b.add(i) };
            if c == b'\n' {
                crate::uart::putc(b'\r');
            }
            crate::uart::putc(c);
            if vt >= 0 {
                crate::vterm::putc_vt(vt as usize, c); // mirror into the window
            }
        }
        return len as i64;
    }
    let t = sched::current();
    let Some(u) = fd_get(t, fd as usize) else {
        return -1;
    };
    let data = unsafe { core::slice::from_raw_parts(buf as *const u8, len as usize) };
    let n = vfs::node_write(u.node, u.pos, data);
    if n > 0 {
        u.pos += n as u64;
    }
    n
}

fn sys_read(f: &IrqFrame) -> i64 {
    let fd = f.rdi as i32;
    let buf = f.rsi;
    if bad_uaddr(buf) {
        return -1;
    }

    if fd == 0 {
        // stdin: focused vterm / kbd
        let mut vt = sched::current().vt;
        if vt < 0 && crate::vterm::console_up() {
            vt = 0;
        }
        let c = loop {
            let c = if vt >= 0 {
                crate::vterm::getc(vt as usize)
            } else {
                crate::kbd::getchar()
            };
            if c >= 0 {
                break c;
            }
            crate::io::sti();
            crate::io::hlt(); // wait for IRQ1
        };
        unsafe {
            *(buf as *mut u8) = c as u8;
        }
        return 1;
    }
    let t = sched::current();
    let Some(u) = fd_get(t, fd as usize) else {
        return -1;
    };
    let out = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, f.rdx as usize) };
    let n = vfs::node_read(u.node, u.pos, out);
    if n > 0 {
        u.pos += n as u64;
    }
    n
}

fn sys_open(f: &IrqFrame) -> i64 {
    let mut path = [0u8; 256];
    if abs_path(f.rdi, &mut path) < 0 {
        return -1;
    }
    let flags = f.rsi as i32;
    let path = buf_str(&path);

    let mut id = vfs::lookup(path);
    if id < 0 && flags & 2 != 0 {
        // O_CREAT
        id = vfs::create(path, b"");
    }
    if id < 0 || vfs::node_type(id) != vfs::VN_FILE {
        return -1;
    }

    let t = sched::current();
    for i in UFD_FIRST..UFD_MAX {
        if t.fds[i].node < 0 {
            t.fds[i].node = id;
            t.fds[i].pos = 0;
            return i as i64;
        }
    }
    -1 // fd table full
}

fn sys_close(f: &IrqFrame) -> i64 {
    let fd = f.rdi as i32;
    if !(UFD_FIRST as i32..UFD_MAX as i32).contains(&fd) {
        return -1;
    }
    sched::current().fds[fd as usize].node = -1;
    0
}

fn sys_stat(f: &IrqFrame) -> i64 {
    let mut path = [0u8; 256];
    let st = f.rsi as *mut Ustat;
    if bad_uaddr(f.rsi) {
        return -1;
    }
    if abs_path(f.rdi, &mut path) < 0 {
        return -1;
    }
    let id = vfs::lookup(buf_str(&path));
    if id < 0 {
        return -1;
    }
    unsafe {
        (*st).size = vfs::node_size(id);
        (*st).typ = vfs::node_type(id) as u32;
    }
    0
}

fn sys_mkdir(f: &IrqFrame) -> i64 {
    let mut path = [0u8; 256];
    if abs_path(f.rdi, &mut path) < 0 {
        return -1;
    }
    vfs::mkdir(buf_str(&path)) as i64
}

fn sys_unlink(f: &IrqFrame) -> i64 {
    let mut path = [0u8; 256];
    if abs_path(f.rdi, &mut path) < 0 {
        return -1;
    }
    vfs::unlink(buf_str(&path)) as i64
}

fn sys_rename(f: &IrqFrame) -> i64 {
    let mut oldp = [0u8; 256];
    let mut newp = [0u8; 256];
    if abs_path(f.rdi, &mut oldp) < 0 {
        return -1;
    }
    if abs_path(f.rsi, &mut newp) < 0 {
        return -1;
    }
    vfs::rename(buf_str(&oldp), buf_str(&newp)) as i64
}

fn sys_getdents(f: &IrqFrame) -> i64 {
    let mut path = [0u8; 256];
    let ud = f.rdx as *mut Udirent;
    if bad_uaddr(f.rdx) {
        return -1;
    }
    if abs_path(f.rdi, &mut path) < 0 {
        return -1;
    }
    let Some(de) = vfs::list(buf_str(&path), f.rsi as usize) else {
        return 0;
    };
    unsafe {
        ptr::write_bytes(ud as *mut u8, 0, core::mem::size_of::<Udirent>());
        let n = de
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(de.name.len())
            .min(55);
        core::ptr::addr_of_mut!((*ud).name)
            .cast::<u8>()
            .copy_from_nonoverlapping(de.name.as_ptr(), n);
        ptr::addr_of_mut!((*ud).typ).write_unaligned(de.typ);
    }
    1
}

fn sys_getcwd(f: &IrqFrame) -> i64 {
    if bad_uaddr(f.rdi) || f.rsi == 0 {
        return -1;
    }
    let cwd = sched::current().cwd_str();
    let n = cwd.len() + 1;
    if n as u64 > f.rsi {
        return -1;
    }
    unsafe {
        ptr::copy_nonoverlapping(cwd.as_ptr(), f.rdi as *mut u8, cwd.len());
        *(f.rdi as *mut u8).add(cwd.len()) = 0;
    }
    n as i64
}

fn sys_chdir(f: &IrqFrame) -> i64 {
    let mut path = [0u8; 256];
    if abs_path(f.rdi, &mut path) < 0 {
        return -1;
    }
    let path_s = buf_str(&path);
    let id = vfs::lookup(path_s);
    if id < 0 || vfs::node_type(id) != vfs::VN_DIR {
        return -1;
    }
    // normalize: store the resolved absolute path.
    // (vfs resolves . and .. at lookup; rebuild it here textually.)
    let mut out = [0u8; 96];
    let mut n = 0usize;
    let bytes = path_s.as_bytes();
    let mut p = 1usize;
    while p < bytes.len() {
        while p < bytes.len() && bytes[p] == b'/' {
            p += 1;
        }
        if p >= bytes.len() {
            break;
        }
        let mut e = p;
        while e < bytes.len() && bytes[e] != b'/' {
            e += 1;
        }
        let comp = &bytes[p..e];
        if comp == b"." {
            p = e;
            continue;
        }
        if comp == b".." {
            if n > 0 {
                n -= 1;
                while n > 0 && out[n] != b'/' {
                    n -= 1;
                }
                if n > 0 {
                    n += 1;
                }
            }
            out[n] = 0;
            p = e;
            continue;
        }
        out[n] = b'/';
        n += 1;
        if n + comp.len() >= out.len() - 1 {
            return -1;
        }
        out[n..n + comp.len()].copy_from_slice(comp);
        n += comp.len();
        out[n] = 0;
        p = e;
    }
    if n == 0 {
        out[0] = b'/';
        out[1] = 0;
    }
    let s = buf_str(&out);
    sched::current().set_cwd(s);
    0
}

/* ---- process syscalls ----------------------------------------------------- */

fn sys_exit(f: &IrqFrame) -> i64 {
    let t = sched::current();
    t.exit_status = f.rdi as i32;
    crate::klog_info!(
        "proc: '{}' exited with code {}",
        t.name_str(),
        t.exit_status
    );
    sched::sched_thread_exit();
}

fn sys_spawn(f: &IrqFrame) -> i64 {
    let mut path = [0u8; 256];
    if abs_path(f.rdi, &mut path) < 0 {
        return -1;
    }

    // Copy argv (user array of user strings) into kernel space.
    let mut kargv_store = [[0u8; 64]; 16];
    let mut argc = 0usize;
    let uargv = f.rsi;
    if !bad_uaddr(uargv) && uargv != 0 {
        let up = uargv as *const u64;
        for i in 0..16 {
            let s = unsafe { *up.add(i) };
            if s == 0 {
                break;
            }
            if bad_uaddr(s) {
                return -1;
            }
            let len = copy_ustr(s, &mut kargv_store[i]);
            if len < 0 {
                return -1;
            }
            argc = i + 1;
        }
    }
    let argv_refs: [&str; 16] = core::array::from_fn(|i| {
        if i < argc {
            buf_str(&kargv_store[i])
        } else {
            ""
        }
    });

    let tid = crate::proc::spawn_path(buf_str(&path), &argv_refs[..argc]);
    if tid >= 0 {
        // inherit the caller's working directory and terminal
        let cur_cwd = sched::current().cwd_str();
        let cur_vt = sched::current().vt;
        let cwd_owned: [u8; 96] = {
            let mut b = [0u8; 96];
            let n = cur_cwd.len().min(95);
            b[..n].copy_from_slice(&cur_cwd.as_bytes()[..n]);
            b
        };
        if let Some(child) = sched::thread_at(tid as usize) {
            child.cwd = cwd_owned;
            child.vt = cur_vt;
        }
    }
    tid as i64
}

fn sys_waitpid(f: &IrqFrame) -> i64 {
    let tid = f.rdi as i32;
    let Some(t) = sched::thread_at(tid as usize) else {
        return -1;
    };
    if t.state == ThreadState::Unused {
        return -1;
    }

    crate::io::sti();
    while sched::thread_at(tid as usize).map_or(false, |t| t.state != ThreadState::Zombie) {
        sched::sched_yield();
    }

    if !bad_uaddr(f.rsi) {
        unsafe {
            *(f.rsi as *mut i32) =
                sched::thread_at(tid as usize).map_or(0, |t| t.exit_status);
        }
    }
    sched::sched_reap(tid as usize);
    tid as i64
}

fn sys_getpid(_f: &IrqFrame) -> i64 {
    sched::current().tid as i64
}

fn sys_sbrk(f: &IrqFrame) -> i64 {
    let incr = f.rdi as i64;
    let t = sched::current();
    let old = t.brk;
    if incr == 0 {
        return old as i64;
    }
    let newb = (old as i64).wrapping_add(incr) as u64;
    if newb < old || newb - 0x6000_0000_0000 > (64u64 << 20) {
        return -1;
    }

    let from = (old + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let mut va = from;
    while va < newb {
        let pa = pmm::alloc_page();
        if pa == 0 || !vmm::map(t.space, va, pa, PTE_US | PTE_RW | PTE_NX) {
            return old as i64 - 1; // unchanged, signal error
        }
        unsafe {
            ptr::write_bytes(pmm::pa_to_va(pa), 0, PAGE_SIZE as usize);
        }
        va += PAGE_SIZE;
    }
    t.brk = newb;
    old as i64
}

fn sys_sleep_ms(f: &IrqFrame) -> i64 {
    let deadline = unsafe { crate::idt::TIMER_TICKS } + (f.rdi + TICK_MS - 1) / TICK_MS;
    crate::io::sti();
    while unsafe { crate::idt::TIMER_TICKS } < deadline {
        sched::sched_yield();
    }
    0
}

fn sys_uptime(_f: &IrqFrame) -> i64 {
    (unsafe { crate::idt::TIMER_TICKS } * TICK_MS) as i64
}

fn sys_uname(f: &IrqFrame) -> i64 {
    let u = f.rdi as *mut Utsname;
    if bad_uaddr(f.rdi) {
        return -1;
    }
    unsafe {
        ptr::write_bytes(u as *mut u8, 0, core::mem::size_of::<Utsname>());
        let set = |dst: &mut [u8], s: &str| {
            dst[..s.len()].copy_from_slice(s.as_bytes());
        };
        set(&mut (*u).sysname, "shizuku");
        set(&mut (*u).release, "0.2-userspace");
        set(&mut (*u).machine, "x86_64");
    }
    0
}

fn sys_sysinfo(f: &IrqFrame) -> i64 {
    let si = f.rdi as *mut Usysinfo;
    if bad_uaddr(f.rdi) {
        return -1;
    }
    unsafe {
        (*si).mem_total = pmm::total_count() as u64 * PAGE_SIZE;
        (*si).mem_free = pmm::free_count() as u64 * PAGE_SIZE;
        (*si).nproc = sched::count_running() as u32;
        (*si).uptime_ms = crate::idt::TIMER_TICKS * TICK_MS;
    }
    0
}

fn sys_yield(_f: &IrqFrame) -> i64 {
    sched::sched_yield();
    0
}

fn sys_nextproc(f: &IrqFrame) -> i64 {
    let up = f.rsi as *mut Uproc;
    if bad_uaddr(f.rsi) {
        return -1;
    }
    let want = f.rdi as usize;
    let mut seen = 0usize;
    for i in 0..sched::SCHED_MAX_THREADS {
        let Some(t) = sched::thread_at(i) else {
            continue;
        };
        if t.state == ThreadState::Unused {
            continue;
        }
        if seen != want {
            seen += 1;
            continue;
        }
        unsafe {
            ptr::write_bytes(up as *mut u8, 0, core::mem::size_of::<Uproc>());
            ptr::addr_of_mut!((*up).tid).write(t.tid);
            let name = t.name_str();
            let n = name.len().min(23);
            ptr::addr_of_mut!((*up).name)
                .cast::<u8>()
                .copy_from_nonoverlapping(name.as_ptr(), n);
            ptr::addr_of_mut!((*up).state).write(if t.state == ThreadState::Zombie { 1 } else { 0 });
        }
        return 1;
    }
    0
}

/* ---- dispatch -------------------------------------------------------------- */

pub fn syscall_dispatch(f: &mut IrqFrame) {
    let n = f.rax;
    let r = match n {
        SYS_WRITE => sys_write(f),
        SYS_EXIT => sys_exit(f),
        SYS_READ => sys_read(f),
        SYS_OPEN => sys_open(f),
        SYS_CLOSE => sys_close(f),
        SYS_STAT => sys_stat(f),
        SYS_MKDIR => sys_mkdir(f),
        SYS_UNLINK => sys_unlink(f),
        SYS_RENAME => sys_rename(f),
        SYS_GETDENTS => sys_getdents(f),
        SYS_GETCWD => sys_getcwd(f),
        SYS_CHDIR => sys_chdir(f),
        SYS_SPAWN => sys_spawn(f),
        SYS_WAITPID => sys_waitpid(f),
        SYS_GETPID => sys_getpid(f),
        SYS_SBRK => sys_sbrk(f),
        SYS_SLEEP_MS => sys_sleep_ms(f),
        SYS_UPTIME_MS => sys_uptime(f),
        SYS_UNAME => sys_uname(f),
        SYS_SYSINFO => sys_sysinfo(f),
        SYS_YIELD => sys_yield(f),
        SYS_NEXTPROC => sys_nextproc(f),
        _ => {
            crate::klog_warn!(
                "syscall: unknown #{} from '{}'",
                n,
                sched::current().name_str()
            );
            -ENOSYS
        }
    };
    f.rax = r as u64;
}
