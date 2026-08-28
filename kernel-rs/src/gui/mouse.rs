//! PS/2 mouse: IRQ12 via the slave PIC, 3-byte movement packets.

use crate::io::{inb, outb};
use crate::pic;

pub static mut X: i32 = 0;
pub static mut Y: i32 = 0;
pub static mut BUTTONS: u8 = 0;

static mut IO_TIMEOUT: bool = false;
static mut IO_TIMEOUT_AT: u32 = 0;

fn wait_write_at(line: u32) {
    let _ = line;
    let mut t = 1_000_000;
    while t > 0 && inb(0x64) & 2 != 0 {
        t -= 1;
    }
}

fn wait_read_at(line: u32) {
    let mut got = false;
    let mut t = 1_000_000;
    while t > 0 {
        if inb(0x64) & 1 != 0 {
            got = true;
            break;
        }
        t -= 1;
    }
    if !got {
        unsafe {
            IO_TIMEOUT = true;
            IO_TIMEOUT_AT = line;
        }
    }
}

macro_rules! wait_write {
    () => {
        wait_write_at(line!())
    };
}

macro_rules! wait_read {
    () => {
        wait_read_at(line!())
    };
}

fn write(byte: u8) {
    wait_write!();
    outb(0x64, 0xD4);
    wait_write!();
    outb(0x60, byte);
}

fn read() -> u8 {
    wait_read!();
    inb(0x60)
}

pub fn init() {
    crate::io::cli();
    // drain any stale output-buffer bytes while polling-busy
    for _ in 0..100_000 {
        if inb(0x64) & 1 == 0 {
            break;
        }
        let _ = inb(0x60);
    }

    // enable aux device through the 8042 controller
    wait_write!();
    outb(0x64, 0xA8);
    wait_write!();
    outb(0x64, 0x20); // read command byte
    wait_read!();
    let mut status = inb(0x60) | 2; // enable IRQ12
    status &= !0x20; // clear mouse clock disable
    wait_write!();
    outb(0x64, 0x60);
    wait_write!();
    outb(0x60, status);

    write(0xF6);
    read(); // defaults
    write(0xF4);
    read(); // enable data reporting

    unsafe {
        X = 640;
        Y = 400;
    }
    pic::unmask(2); // cascade
    pic::unmask(12); // mouse
    unsafe {
        if IO_TIMEOUT {
            crate::klog_warn!("mouse: 8042 timeout at line {}", IO_TIMEOUT_AT);
        } else {
            crate::klog_info!("mouse: PS/2 aux device enabled (IRQ12)");
        }
    }
    crate::io::sti();
}

static mut CYCLE: u8 = 0;
static mut PACKET: [u8; 3] = [0; 3];

pub fn irq_handler() {
    let b = inb(0x60);

    unsafe {
        let idx = (CYCLE % 3) as usize;
        PACKET[idx] = b;
        if idx == 0 && b & 0x08 == 0 {
            CYCLE = 0;
            return; // resync
        }
        CYCLE += 1;
        if CYCLE < 3 {
            return;
        }
        CYCLE = 0;

        let mut dx = PACKET[1] as i8 as i32;
        let mut dy = PACKET[2] as i8 as i32;
        if PACKET[0] & 0x40 != 0 {
            dx = 0; // overflow: drop
        }
        if PACKET[0] & 0x80 != 0 {
            dy = 0;
        }

        X += dx;
        Y -= dy;
        if X < 0 {
            X = 0;
        }
        if Y < 0 {
            Y = 0;
        }
        if X >= crate::fb::width() as i32 {
            X = crate::fb::width() as i32 - 1;
        }
        if Y >= crate::fb::height() as i32 {
            Y = crate::fb::height() as i32 - 1;
        }
        BUTTONS = PACKET[0] & 0x07;
    }
}
