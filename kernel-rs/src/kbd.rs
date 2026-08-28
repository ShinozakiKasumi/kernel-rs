//! PS/2 scancode set 1 -> ASCII, ring buffer, optional vterm routing.

use core::cell::UnsafeCell;

const KBD_BUF: usize = 128;

struct BufCell(UnsafeCell<[u8; KBD_BUF]>);
unsafe impl Sync for BufCell {}

static BUF: BufCell = BufCell(UnsafeCell::new([0; KBD_BUF]));
static mut HEAD: usize = 0; // push
static mut TAIL: usize = 0; // pop
static mut SHIFT: bool = false;

/// When >= 0, typed chars go to this vterm instead of the kernel ring.
pub static mut ROUTE_VT: i32 = -1;
/// Called with function-key number (F1 only today).
pub static mut FN_HOOK: Option<fn(i32)> = None;

/// View of a 59-entry scancode table padded out to 128 entries.
const fn pad128(t: [u8; 64]) -> [u8; 128] {
    let mut out = [0u8; 128];
    let mut i = 0;
    while i < 64 {
        out[i] = t[i];
        i += 1;
    }
    out
}

const UNSHIFTED: [u8; 128] = pad128([
    0, 27, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', 8, b'\t',
    b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'\n', 0, b'a',
    b's', b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', b'\'', b'`', 0, b'\\', b'z', b'x',
    b'c', b'v', b'b', b'n', b'm', b',', b'.', b'/', 0, b'*', 0, b' ', 0,
    // F1..F10 etc: ignored
    0, 0, 0, 0, 0,
]);

const SHIFTED: [u8; 128] = pad128([
    0, 27, b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')', b'_', b'+', 8, b'\t',
    b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', b'{', b'}', b'\n', 0, b'A',
    b'S', b'D', b'F', b'G', b'H', b'J', b'K', b'L', b':', b'"', b'~', 0, b'|', b'Z', b'X',
    b'C', b'V', b'B', b'N', b'M', b'<', b'>', b'?', 0, b'*', 0, b' ', 0, 0, 0, 0, 0, 0,
]);

pub fn kbd_on_scancode(sc: u8) {
    unsafe {
        match sc {
            0x2A | 0x36 => {
                SHIFT = true;
                return;
            }
            0xAA | 0xB6 => {
                SHIFT = false;
                return;
            }
            _ => {}
        }
        if sc & 0x80 != 0 {
            return; // key release
        }
        if sc >= 128 {
            return;
        }

        if sc == 0x3B {
            // F1: new GUI terminal
            if let Some(hook) = FN_HOOK {
                hook(1);
            }
            return;
        }

        let idx = sc as usize;
        let c = if idx < 64 {
            if SHIFT {
                SHIFTED[idx]
            } else {
                UNSHIFTED[idx]
            }
        } else {
            0
        };
        if c == 0 {
            return;
        }

        if ROUTE_VT >= 0 {
            crate::vterm::push(ROUTE_VT, c);
            return;
        }

        let next = (HEAD + 1) % KBD_BUF;
        if next == TAIL {
            return; // full: drop
        }
        (*BUF.0.get())[HEAD] = c;
        HEAD = next;
    }
}

pub fn getchar() -> i32 {
    unsafe {
        if HEAD == TAIL {
            return -1;
        }
        let c = (*BUF.0.get())[TAIL];
        TAIL = (TAIL + 1) % KBD_BUF;
        c as i32
    }
}

/// Inject a character (e.g. from the serial console) as if typed.
pub fn push_char(c: u8) {
    unsafe {
        let next = (HEAD + 1) % KBD_BUF;
        if next == TAIL {
            return;
        }
        (*BUF.0.get())[HEAD] = c;
        HEAD = next;
    }
}
