//! Round-robin kernel thread scheduler.
//!
//! Context switch model: every thread owns a 16KiB kernel stack; its
//! preemption point is an [`IrqFrame`] saved inside the thread struct. The
//! timer handler (running on the *preempted* thread's stack) copies the live
//! frame out, picks the next runnable thread, copies its frame in, and iretq
//! returns into it.

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use crate::idt::IrqFrame;
use crate::mm::pmm::{self, PAGE_SIZE};
use crate::mm::vmm::{self, PageTable};

pub const SCHED_MAX_THREADS: usize = 32;
pub const THREAD_KSTACK_SIZE: usize = (4 * PAGE_SIZE) as usize; // 16 KiB

pub type ThreadFn = extern "C" fn(*mut core::ffi::c_void) -> ();

const TIMESLICE_TICKS: u64 = 5; // 50ms at 100Hz

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ThreadState {
    Unused = 0,
    Runnable = 1,
    Zombie = 2,
}

pub const UFD_MAX: usize = 8; // per-process fds 0..7 (0..2 fixed)
pub const UFD_FIRST: usize = 3;

#[derive(Clone, Copy)]
pub struct Ufd {
    pub node: i32, // tmpfs node id, -1 = free slot
    pub pos: u64,
}

impl Ufd {
    pub const fn free() -> Self {
        Ufd { node: -1, pos: 0 }
    }
}

#[derive(Clone, Copy)]
pub struct Thread {
    pub tid: i32,
    pub name: [u8; 16],
    pub state: ThreadState,
    pub frame: IrqFrame,       // saved trap frame (switch point)
    pub space: PageTable,      // address space (0 => kernel)
    pub kstack_top: u64,       // initial top of kernel stack
    pub kstack_pa: u64,        // for the TSS/cleanup
    pub is_user: bool,
    // userspace runtime state
    pub fds: [Ufd; UFD_MAX],
    pub cwd: [u8; 96],
    pub brk: u64, // current program break (user heap)
    pub exit_status: i32,
    pub vt: i32, // vterm index; -1 = legacy serial console
}

impl Thread {
    fn zeroed() -> Self {
        Thread {
            tid: 0,
            name: [0; 16],
            state: ThreadState::Unused,
            frame: unsafe { MaybeUninit::zeroed().assume_init() },
            space: 0,
            kstack_top: 0,
            kstack_pa: 0,
            is_user: false,
            fds: [Ufd::free(); UFD_MAX],
            cwd: [0; 96],
            brk: 0,
            exit_status: 0,
            vt: -1,
        }
    }

    pub fn set_name(&mut self, name: &str) {
        let n = name.len().min(self.name.len() - 1);
        self.name[..n].copy_from_slice(&name.as_bytes()[..n]);
        self.name[n..].fill(0);
    }

    pub fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..end]).unwrap_or("<bad>")
    }

    pub fn set_cwd(&mut self, cwd: &str) {
        let n = cwd.len().min(self.cwd.len() - 1);
        self.cwd[..n].copy_from_slice(&cwd.as_bytes()[..n]);
        self.cwd[n..].fill(0);
    }

    pub fn cwd_str(&self) -> &str {
        let end = self.cwd.iter().position(|&b| b == 0).unwrap_or(self.cwd.len());
        core::str::from_utf8(&self.cwd[..end]).unwrap_or("/")
    }
}

struct ThreadsCell(UnsafeCell<[Thread; SCHED_MAX_THREADS]>);
unsafe impl Sync for ThreadsCell {}

static THREADS: ThreadsCell = ThreadsCell(UnsafeCell::new([const {
    // const zeroing without assisted MaybeUninit ergonomics
    Thread {
        tid: 0,
        name: [0; 16],
        state: ThreadState::Unused,
        frame: IrqFrame {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rbp: 0,
            rdi: 0,
            rsi: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,
            vector: 0,
            err: 0,
            rip: 0,
            cs: 0,
            rflags: 0,
            rsp: 0,
            ss: 0,
        },
        space: 0,
        kstack_top: 0,
        kstack_pa: 0,
        is_user: false,
        fds: [Ufd { node: -1, pos: 0 }; UFD_MAX],
        cwd: [0; 96],
        brk: 0,
        exit_status: 0,
        vt: -1,
    }
}; SCHED_MAX_THREADS]));

static mut NR_THREADS: usize = 0;
static mut CURRENT_IDX: usize = 0;
static mut LAST_SLICE: u64 = 0;

fn threads() -> &'static mut [Thread; SCHED_MAX_THREADS] {
    unsafe { &mut *THREADS.0.get() }
}

extern "C" {
    fn kthread_trampoline(); // asm.rs
}

pub fn init() {
    let threads = threads();
    threads[0] = Thread::zeroed();
    threads[0].tid = 0;
    threads[0].set_name("kernel");
    threads[0].state = ThreadState::Runnable; // boot context
    threads[0].space = unsafe { vmm::KERNEL_SPACE };
    threads[0].set_cwd("/");
    unsafe {
        NR_THREADS = 1;
        CURRENT_IDX = 0;
    }
    crate::klog_info!("sched: {} slots, boot context = tid 0", SCHED_MAX_THREADS);
}

/// Create a kernel thread running fn(arg). Returns tid or -1.
pub fn kthread_create(name: &str, f: ThreadFn, arg: *mut core::ffi::c_void) -> i32 {
    let threads = threads();
    let mut slot = -1i32;
    for (i, t) in threads.iter().enumerate() {
        if t.state == ThreadState::Unused {
            slot = i as i32;
            break;
        }
    }
    if slot < 0 {
        return -1;
    }

    let pa = pmm::alloc_pages(THREAD_KSTACK_SIZE / PAGE_SIZE as usize, PAGE_SIZE as usize);
    if pa == 0 {
        return -1;
    }

    let t = &mut threads[slot as usize];
    *t = Thread::zeroed();
    t.tid = slot;
    t.state = ThreadState::Runnable;
    t.space = unsafe { vmm::KERNEL_SPACE };
    t.kstack_top = pmm::pa_to_va(pa) as u64 + THREAD_KSTACK_SIZE as u64;
    t.kstack_pa = pa;
    t.set_name(name);

    // Fabricate the first trap frame at the top of the new stack. After the
    // first iretq, kthread_trampoline runs fn(arg) with rbx=fn, rdi=arg.
    let fr = &mut t.frame;
    fr.rip = kthread_trampoline as u64;
    fr.cs = 0x08;
    fr.ss = 0x10;
    fr.rflags = 0x202; // IF | reserved bit
    fr.rsp = t.kstack_top;
    fr.rbx = f as u64;
    fr.rdi = arg as u64;

    unsafe {
        if slot as usize >= NR_THREADS {
            NR_THREADS = slot as usize + 1;
        }
    }
    crate::klog_info!("sched: created '{}' (tid {})", name, slot);
    slot
}

fn alloc_thread_slot(name: &str) -> i32 {
    let threads = threads();
    for i in 0..SCHED_MAX_THREADS {
        if threads[i].state == ThreadState::Unused {
            let t = &mut threads[i];
            *t = Thread::zeroed();
            for fd in t.fds.iter_mut() {
                fd.node = -1;
            }
            t.vt = -1;
            t.set_cwd("/");
            t.tid = i as i32;
            t.state = ThreadState::Runnable;
            t.set_name(name);
            unsafe {
                if i >= NR_THREADS {
                    NR_THREADS = i + 1;
                }
            }
            return i as i32;
        }
    }
    -1
}

/// Create a thread that starts in ring 3 at `entry` with user stack
/// `user_rsp`, address space `space`; the kernel stack (kstack) serves
/// interrupts/syscalls while the thread is in userland. Returns tid or -1.
pub fn thread_create_user(name: &str, entry: u64, user_rsp: u64, space: PageTable) -> i32 {
    let slot = alloc_thread_slot(name);
    if slot < 0 {
        return -1;
    }

    let pa = pmm::alloc_pages(THREAD_KSTACK_SIZE / PAGE_SIZE as usize, PAGE_SIZE as usize);
    if pa == 0 {
        return -1;
    }

    let t = &mut threads()[slot as usize];
    t.space = space;
    t.is_user = true;
    t.kstack_top = pmm::pa_to_va(pa) as u64 + THREAD_KSTACK_SIZE as u64;
    t.kstack_pa = pa;

    let fr = &mut t.frame;
    fr.rip = entry;
    fr.cs = 0x20 | 3; // user code selector, RPL3
    fr.ss = 0x18 | 3; // user data selector, RPL3
    fr.rsp = user_rsp;
    fr.rflags = 0x202;

    crate::klog_info!("sched: user '{}' (tid {}) entry={:#x}", name, slot, entry);
    slot
}

fn pick_next() -> usize {
    let cur = unsafe { CURRENT_IDX };
    for i in 1..=SCHED_MAX_THREADS {
        let idx = (cur + i) % SCHED_MAX_THREADS;
        if threads()[idx].state == ThreadState::Runnable {
            return idx;
        }
    }
    cur
}

/// Context switch: return `to`'s saved frame in `f` and switch address space.
fn switch_to(f: &mut IrqFrame, to: usize) {
    let cur = unsafe { CURRENT_IDX };
    let threads = threads();
    threads[cur].frame = *f; // save preempted context
    unsafe {
        CURRENT_IDX = to;
    }
    let nt = &threads[to];
    *f = nt.frame; // iretq will land here

    if nt.space != 0 && nt.space != threads[cur].space {
        vmm::switch(nt.space);
    }

    // TSS hook (M7)
    if nt.kstack_top != 0 {
        crate::gdt::set_kernel_stack(nt.kstack_top);
    }
}

/// Called from the timer interrupt with the live trap frame.
pub fn sched_tick(f: &mut IrqFrame) {
    unsafe {
        LAST_SLICE += 1;
        if LAST_SLICE < TIMESLICE_TICKS {
            return;
        }
        LAST_SLICE = 0;
    }

    let next = pick_next();
    if next == unsafe { CURRENT_IDX } {
        return;
    }
    switch_to(f, next);
}

pub fn sched_yield() {
    unsafe {
        LAST_SLICE = TIMESLICE_TICKS; // make next tick switch immediately
    }
    unsafe {
        core::arch::asm!("int 32", options(nostack)); // run through the timer path now
    }
}

/// Mark the current thread zombie and switch away. Called from
/// `kthread_trampoline` when a thread function returns.
#[no_mangle]
pub extern "C" fn sched_thread_exit() -> ! {
    let t = current();
    crate::klog_info!("sched: '{}' (tid {}) exited", t.name_str(), t.tid);
    t.state = ThreadState::Zombie;
    sched_yield(); // never returns to the zombie frame
    loop {
        crate::io::cli();
        crate::io::hlt();
    }
}

/// Free a zombie's resources and recycle its slot. Must not be running.
pub fn sched_reap(tid: usize) {    let threads = threads();
    let ks = unsafe { vmm::KERNEL_SPACE };
    let t = &mut threads[tid];
    if t.state != ThreadState::Zombie {
        return;
    }
    if t.is_user && t.space != 0 && t.space != ks {
        vmm::destroy_space(t.space);
    }
    if t.kstack_pa != 0 {
        for i in 0..THREAD_KSTACK_SIZE as u64 / PAGE_SIZE {
            pmm::free_page(t.kstack_pa + i * PAGE_SIZE);
        }
    }
    *t = Thread::zeroed();
}

pub fn current() -> &'static mut Thread {
    unsafe { &mut threads()[CURRENT_IDX] }
}

pub fn thread_at(idx: usize) -> Option<&'static mut Thread> {
    if idx >= SCHED_MAX_THREADS {
        return None;
    }
    Some(&mut threads()[idx])
}

pub fn count() -> usize {
    unsafe { NR_THREADS }
}

pub fn count_running() -> usize {
    let mut n = 0;
    for i in 0..unsafe { NR_THREADS } {
        if threads()[i].state != ThreadState::Unused {
            n += 1;
        }
    }
    n
}

/// List threads via an exit-status-independent emitter (used by `ps`).
pub fn list(emit: &mut dyn FnMut(core::fmt::Arguments)) {
    emit(format_args!("{:>3}  {:<12} {}  {}\n", "tid", "name", "state", "space"));
    for i in 0..unsafe { NR_THREADS } {
        let t = &threads()[i];
        if t.state != ThreadState::Unused {
            emit(format_args!(
                "{:>3}  {:<12} {}  {:#x}\n",
                t.tid,
                t.name_str(),
                if t.state == ThreadState::Zombie {
                    "zombie"
                } else {
                    "runnable"
                },
                t.space
            ));
        }
    }
}

/* --- selftest ------------------------------------------------------------- */

static mut TA_RUNS: i32 = 0;
static mut TB_RUNS: i32 = 0;

extern "C" fn ta(_arg: *mut core::ffi::c_void) {
    for i in 0..3 {
        unsafe {
            TA_RUNS += 1;
        }
        crate::klog_info!("[TA] tick {}", i);
        sched_yield();
    }
    crate::klog_info!("[TA] done");
}

extern "C" fn tb(_arg: *mut core::ffi::c_void) {
    for i in 0..3 {
        unsafe {
            TB_RUNS += 1;
        }
        crate::klog_info!("[TB] tick {}", i);
        sched_yield();
    }
    crate::klog_info!("[TB] done");
}

pub fn selftest() {
    crate::klog_info!("sched: spawning two test threads...");
    kthread_create("test-a", ta, core::ptr::null_mut());
    kthread_create("test-b", tb, core::ptr::null_mut());
}
