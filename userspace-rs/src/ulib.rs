//! Shizuku userland runtime — minimal libc over int 0x80.
//! Equivalent of the old userspace/ulib (crt0.S + ulib.c), in Rust.

#![no_std]

use core::arch::global_asm;

// _start: System V user entry. Kernel places [argc][argv0..argvN][NULL]
// at the initial user rsp (see proc.rs).
global_asm!(
    r#"
.text
.globl _start
_start:
    mov rdi, [rsp]
    lea rsi, [rsp + 8]
    and rsp, -16
    call umain_shim
.Lhalt:
    hlt
    jmp .Lhalt
"#
);

// ---- syscall numbers (keep in sync with kernel include/syscall.h) --------

pub const SYS_WRITE: i64 = 0;
pub const SYS_EXIT: i64 = 1;
pub const SYS_READ: i64 = 2;
pub const SYS_OPEN: i64 = 3;
pub const SYS_CLOSE: i64 = 4;
pub const SYS_STAT: i64 = 5;
pub const SYS_MKDIR: i64 = 6;
pub const SYS_UNLINK: i64 = 7;
pub const SYS_RENAME: i64 = 8;
pub const SYS_GETDENTS: i64 = 9;
pub const SYS_GETCWD: i64 = 10;
pub const SYS_CHDIR: i64 = 11;
pub const SYS_SPAWN: i64 = 12;
pub const SYS_WAITPID: i64 = 13;
pub const SYS_GETPID: i64 = 14;
pub const SYS_SBRK: i64 = 15;
pub const SYS_SLEEP_MS: i64 = 16;
pub const SYS_UPTIME_MS: i64 = 17;
pub const SYS_UNAME: i64 = 18;
pub const SYS_SYSINFO: i64 = 19;
pub const SYS_YIELD: i64 = 20;
pub const SYS_NEXTPROC: i64 = 21;

pub const O_RDONLY: i32 = 0;
pub const O_WRONLY: i32 = 1;
pub const O_RDWR: i32 = 3;
pub const O_CREAT: i32 = 2; // or-in for create-if-missing

// ---- uapi structs (match kernel/src/syscall.rs repr(C)) -------------------

#[repr(C)]
#[derive(Default)]
pub struct Ustat {
    pub size: u64,
    pub typ: u32, // 1 = file, 2 = dir
}

#[repr(C)]
pub struct Udirent {
    pub name: [u8; 56],
    pub typ: u32,
    pub _pad: u32,
}

impl Default for Udirent {
    fn default() -> Self {
        Udirent {
            name: [0; 56],
            typ: 0,
            _pad: 0,
        }
    }
}

#[repr(C)]
pub struct Utsname {
    pub sysname: [u8; 32],
    pub release: [u8; 32],
    pub machine: [u8; 32],
}

impl Default for Utsname {
    fn default() -> Self {
        Utsname {
            sysname: [0; 32],
            release: [0; 32],
            machine: [0; 32],
        }
    }
}

#[repr(C)]
#[derive(Default)]
pub struct Usysinfo {
    pub mem_total: u64,
    pub mem_free: u64,
    pub nproc: u32,
    pub uptime_ms: u64,
}

#[repr(C)]
pub struct Uproc {
    pub tid: i32,
    pub name: [u8; 24],
    pub state: u32,
}

impl Default for Uproc {
    fn default() -> Self {
        Uproc {
            tid: 0,
            name: [0; 24],
            state: 0,
        }
    }
}

// ---- raw syscall plumbing --------------------------------------------------

pub unsafe fn sys0(n: i64) -> i64 {
    let ret: i64;
    core::arch::asm!("int 0x80", inlateout("rax") n => ret, options(nostack, preserves_flags));
    ret
}

pub unsafe fn sys1(n: i64, a: i64) -> i64 {
    let ret: i64;
    core::arch::asm!(
        "int 0x80",
        inlateout("rax") n => ret,
        in("rdi") a,
        options(nostack, preserves_flags)
    );
    ret
}

pub unsafe fn sys2(n: i64, a: i64, b: i64) -> i64 {
    let ret: i64;
    core::arch::asm!(
        "int 0x80",
        inlateout("rax") n => ret,
        in("rdi") a,
        in("rsi") b,
        options(nostack, preserves_flags)
    );
    ret
}

pub unsafe fn sys3(n: i64, a: i64, b: i64, c: i64) -> i64 {
    let ret: i64;
    core::arch::asm!(
        "int 0x80",
        inlateout("rax") n => ret,
        in("rdi") a,
        in("rsi") b,
        in("rdx") c,
        options(nostack, preserves_flags)
    );
    ret
}

// ---- kernel interface wrappers ---------------------------------------------

/// Copy a &str into a NUL-terminated stack buffer for syscalls.
pub fn cstr<const N: usize>(s: &str) -> [u8; N] {
    let mut buf = [0u8; N];
    let n = s.len().min(N - 1);
    buf[..n].copy_from_slice(&s.as_bytes()[..n]);
    buf
}

pub fn open(path: &[u8], flags: i32) -> i32 {
    unsafe { sys2(SYS_OPEN, path.as_ptr() as i64, flags as i64) as i32 }
}

pub fn read(fd: i32, buf: &mut [u8]) -> i64 {
    unsafe { sys3(SYS_READ, fd as i64, buf.as_mut_ptr() as i64, buf.len() as i64) }
}

pub fn write(fd: i32, buf: &[u8]) -> i64 {
    unsafe { sys3(SYS_WRITE, fd as i64, buf.as_ptr() as i64, buf.len() as i64) }
}

pub fn close(fd: i32) -> i32 {
    unsafe { sys1(SYS_CLOSE, fd as i64) as i32 }
}

pub fn stat(path: &[u8], st: &mut Ustat) -> i32 {
    unsafe { sys2(SYS_STAT, path.as_ptr() as i64, st as *mut Ustat as i64) as i32 }
}

pub fn mkdir(path: &[u8]) -> i32 {
    unsafe { sys1(SYS_MKDIR, path.as_ptr() as i64) as i32 }
}

pub fn unlink(path: &[u8]) -> i32 {
    unsafe { sys1(SYS_UNLINK, path.as_ptr() as i64) as i32 }
}

pub fn rename(old: &[u8], new: &[u8]) -> i32 {
    unsafe { sys2(SYS_RENAME, old.as_ptr() as i64, new.as_ptr() as i64) as i32 }
}

pub fn getdents(path: &[u8], idx: u32, d: &mut Udirent) -> i32 {
    unsafe { sys3(SYS_GETDENTS, path.as_ptr() as i64, idx as i64, d as *mut Udirent as i64) as i32 }
}

pub fn getcwd(buf: &mut [u8]) -> i64 {
    unsafe { sys2(SYS_GETCWD, buf.as_mut_ptr() as i64, buf.len() as i64) }
}

pub fn chdir(path: &[u8]) -> i32 {
    unsafe { sys1(SYS_CHDIR, path.as_ptr() as i64) as i32 }
}

pub fn spawn(path: &[u8], argv: &[*const u8]) -> i32 {
    unsafe { sys2(SYS_SPAWN, path.as_ptr() as i64, argv.as_ptr() as i64) as i32 }
}

pub fn waitpid(tid: i32, status: &mut i32) -> i32 {
    unsafe { sys2(SYS_WAITPID, tid as i64, status as *mut i32 as i64) as i32 }
}

pub fn getpid() -> i32 {
    unsafe { sys0(SYS_GETPID) as i32 }
}

pub fn sbrk(incr: i64) -> *mut u8 {
    unsafe { sys1(SYS_SBRK, incr) as *mut u8 }
}

pub fn sleep_ms(ms: u64) {
    unsafe {
        sys1(SYS_SLEEP_MS, ms as i64);
    }
}

pub fn uptime_ms() -> u64 {
    unsafe { sys0(SYS_UPTIME_MS) as u64 }
}

pub fn uname(u: &mut Utsname) -> i32 {
    unsafe { sys1(SYS_UNAME, u as *mut Utsname as i64) as i32 }
}

pub fn sysinfo(si: &mut Usysinfo) -> i32 {
    unsafe { sys1(SYS_SYSINFO, si as *mut Usysinfo as i64) as i32 }
}

pub fn u_yield() {
    unsafe {
        sys0(SYS_YIELD);
    }
}

pub fn nextproc(idx: u32, p: &mut Uproc) -> i32 {
    unsafe { sys2(SYS_NEXTPROC, idx as i64, p as *mut Uproc as i64) as i32 }
}

pub fn exit(code: i32) -> ! {
    unsafe {
        sys1(SYS_EXIT, code as i64);
    }
    loop {}
}

// ---- stdio-ish -------------------------------------------------------------

pub fn puts(s: &str) -> i32 {
    write(1, s.as_bytes());
    write(1, b"\n");
    0
}

pub fn putchar(c: u8) -> i32 {
    write(1, &[c]);
    0
}

pub struct FmtFd(pub i32);

impl core::fmt::Write for FmtFd {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write(self.0, s.as_bytes());
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write as _;
        let _ = core::fmt::write(&mut $crate::FmtFd(1), format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write as _;
        let _ = core::fmt::write(&mut $crate::FmtFd(1), format_args!($($arg)*));
        let _ = core::fmt::write(&mut $crate::FmtFd(1), format_args!("\n"));
    }};
}

#[macro_export]
macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use core::fmt::Write as _;
        let _ = core::fmt::write(&mut $crate::FmtFd(2), format_args!($($arg)*));
        let _ = core::fmt::write(&mut $crate::FmtFd(2), format_args!("\n"));
    }};
}

// ---- stdlib-ish ------------------------------------------------------------

pub fn atoi(s: &str) -> i32 {
    let s = s.as_bytes();
    let mut v: i64 = 0;
    let mut neg = false;
    let mut i = 0;
    while i < s.len() && s[i] == b' ' {
        i += 1;
    }
    if i < s.len() && s[i] == b'-' {
        neg = true;
        i += 1;
    }
    while i < s.len() && s[i] >= b'0' && s[i] <= b'9' {
        v = v * 10 + (s[i] - b'0') as i64;
        i += 1;
    }
    let v = if neg { -v } else { v };
    v as i32
}

/// Bump allocator over SYS_sbrk (free is a no-op, like the C ulib).
pub fn malloc(size: usize) -> *mut u8 {
    let size = (size.max(1) + 15) & !15;
    let p = sbrk(size as i64);
    if (p as i64) < 0 {
        core::ptr::null_mut()
    } else {
        p
    }
}

// ---- arguments -------------------------------------------------------------

/// Arguments captured by the entry shim; re-borrowed per access.
static mut ARGV_STORAGE: *const *const u8 = core::ptr::null();
static mut ARGC: i32 = 0;

/// argv snapshot as NUL-terminated byte strings.
pub fn args() -> &'static [*const u8] {
    unsafe { core::slice::from_raw_parts(ARGV_STORAGE, ARGC as usize) }
}

pub fn arg_str(i: usize) -> &'static str {
    let args = args();
    if i >= args.len() {
        return "";
    }
    let p = args[i];
    if p.is_null() {
        return "";
    }
    let mut len = 0usize;
    unsafe {
        while *p.add(len) != 0 {
            len += 1;
        }
        core::str::from_utf8(core::slice::from_raw_parts(p, len)).unwrap_or("")
    }
}

pub fn argc() -> i32 {
    unsafe { ARGC }
}

// ---- entry shim ------------------------------------------------------------

extern "Rust" {
    fn umain() -> i32;
}

/// Called from `_start` with rsp-based argc/argv. Stores them in statics
/// and invokes the program's `umain`.
#[no_mangle]
extern "C" fn umain_shim(argc: i32, argv: *const *const u8) -> ! {
    unsafe {
        ARGC = argc;
        ARGV_STORAGE = argv;
    }
    let code = unsafe { umain() };
    exit(code)
}

/// Define the program entry point. Usage: `entry!(main)` where
/// `fn main() -> i32`.
#[macro_export]
macro_rules! entry {
    ($main:path) => {
        #[no_mangle]
        fn umain() -> i32 {
            $main()
        }
    };
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    write(2, b"user program panicked\n");
    exit(127)
}
