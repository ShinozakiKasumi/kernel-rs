//! 8259 PIC: remapped above the exception vectors, everything masked unless
//! explicitly unmasked.

use crate::io::{inb, io_wait, outb};

pub const PIC1_CMD: u16 = 0x20;
pub const PIC1_DATA: u16 = 0x21;
pub const PIC2_CMD: u16 = 0xA0;
pub const PIC2_DATA: u16 = 0xA1;
pub const PIC_IRQ_BASE: u8 = 0x20;

pub fn init() {
    outb(PIC1_CMD, 0x11);
    io_wait(); // ICW1: init, ICW4 needed
    outb(PIC2_CMD, 0x11);
    io_wait();
    outb(PIC1_DATA, PIC_IRQ_BASE);
    io_wait(); // ICW2: vector offset
    outb(PIC2_DATA, PIC_IRQ_BASE + 8);
    io_wait();
    outb(PIC1_DATA, 0x04);
    io_wait(); // ICW3: slave on IRQ2
    outb(PIC2_DATA, 0x02);
    io_wait();
    outb(PIC1_DATA, 0x01);
    io_wait(); // ICW4: 8086 mode
    outb(PIC2_DATA, 0x01);
    io_wait();

    outb(PIC1_DATA, 0xFF); // start fully masked
    outb(PIC2_DATA, 0xFF);

    crate::klog_info!(
        "pic: remapped to vectors {}-{}, all lines masked",
        PIC_IRQ_BASE,
        PIC_IRQ_BASE + 15
    );
}

pub fn eoi(irq: u8) {
    if irq >= 8 {
        outb(PIC2_CMD, 0x20);
    }
    outb(PIC1_CMD, 0x20);
}

pub fn unmask(irq: u8) {
    let port = if irq < 8 { PIC1_DATA } else { PIC2_DATA };
    let bit = if irq < 8 { irq } else { irq - 8 };
    let cur = inb(port) & !(1 << bit);
    outb(port, cur);
}

pub fn mask(irq: u8) {
    let port = if irq < 8 { PIC1_DATA } else { PIC2_DATA };
    let bit = if irq < 8 { irq } else { irq - 8 };
    let cur = inb(port) | (1 << bit);
    outb(port, cur);
}
