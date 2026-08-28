//! vterm: per-window virtual terminals (text grid + input queue).
//!
//! Each vterm owns a character grid (what the window paints) and an input
//! queue (what the keyboard IRQ pushes into when that window is focused).
//! No locking: producer = IRQ context / syscalls on the owning thread,
//! consumer = same thread + compositor; teaching-kernel acceptable.

use core::cell::UnsafeCell;

pub const VT_MAX: usize = 4;
pub const VT_ROWS: usize = 14;
pub const VT_COLS: usize = 56;
pub const VT_INQ: usize = 128;

#[derive(Clone, Copy)]
struct Vterm {
    used: bool,
    grid: [[u8; VT_COLS + 1]; VT_ROWS],
    row: usize,
    col: usize, // next write position
    inq: [u8; VT_INQ],
    inq_head: usize,
    inq_tail: usize,
}

impl Vterm {
    const fn empty() -> Self {
        Vterm {
            used: false,
            grid: [[b' '; VT_COLS + 1]; VT_ROWS],
            row: 0,
            col: 0,
            inq: [0; VT_INQ],
            inq_head: 0,
            inq_tail: 0,
        }
    }
}

struct TermsCell(UnsafeCell<[Vterm; VT_MAX]>);
unsafe impl Sync for TermsCell {}

static TERMS: TermsCell = TermsCell(UnsafeCell::new([const { Vterm::empty() }; VT_MAX]));
static mut CONSOLE_UP: bool = false;
static mut DIRTY_HOOK: Option<fn(i32)> = None; // set by the WM to repaint windows

fn terms() -> &'static mut [Vterm; VT_MAX] {
    unsafe { &mut *TERMS.0.get() }
}

/// Allocate a vterm; id 0 is the boot console. -1 when full.
pub fn create() -> i32 {
    for i in 0..VT_MAX {
        if terms()[i].used {
            continue;
        }
        terms()[i] = Vterm::empty();
        terms()[i].used = true;
        if i == 0 {
            unsafe {
                CONSOLE_UP = true;
            }
        }
        return i as i32;
    }
    -1
}

pub fn set_dirty_hook(hook: fn(i32)) {
    unsafe {
        DIRTY_HOOK = Some(hook);
    }
}

fn scroll(v: &mut Vterm) {
    for r in 0..VT_ROWS - 1 {
        v.grid[r] = v.grid[r + 1];
    }
    v.grid[VT_ROWS - 1] = [b' '; VT_COLS + 1];
    v.row = VT_ROWS - 1;
}

/// Output side: write a character (handles \n, \r, scroll).
pub fn putc_vt(id: usize, c: u8) {
    if id >= VT_MAX || !terms()[id].used {
        return;
    }
    let v = &mut terms()[id];
    if c == b'\r' {
        return;
    }
    if c == b'\n' {
        v.row += 1;
        v.col = 0;
        if v.row >= VT_ROWS {
            scroll(v);
        }
    } else {
        v.grid[v.row][v.col] = c;
        v.col += 1;
        if v.col >= VT_COLS {
            v.col = 0;
            v.row += 1;
            if v.row >= VT_ROWS {
                scroll(v);
            }
        }
    }
    unsafe {
        if let Some(hook) = DIRTY_HOOK {
            hook(id as i32);
        }
    }
}

/// Input side: typed characters queue here.
pub fn push(id: i32, c: u8) {
    if id < 0 || id as usize >= VT_MAX || !terms()[id as usize].used {
        return;
    }
    let v = &mut terms()[id as usize];
    let next = (v.inq_head + 1) % VT_INQ;
    if next == v.inq_tail {
        return; // full: drop
    }
    v.inq[v.inq_head] = c;
    v.inq_head = next;
}

pub fn getc(id: usize) -> i32 {
    if id >= VT_MAX || !terms()[id].used {
        return -1;
    }
    let v = &mut terms()[id];
    if v.inq_head == v.inq_tail {
        return -1;
    }
    let c = v.inq[v.inq_tail];
    v.inq_tail = (v.inq_tail + 1) % VT_INQ;
    c as i32
}

/// Rendering: fetch a screen row (VT_COLS chars).
pub fn row(id: usize, row: usize) -> Option<&'static [u8]> {
    if id >= VT_MAX || row >= VT_ROWS {
        return None;
    }
    Some(&terms()[id].grid[row][..VT_COLS])
}

pub fn console_up() -> bool {
    unsafe { CONSOLE_UP }
}
