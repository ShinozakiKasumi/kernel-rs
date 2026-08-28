//! COM1 serial port (matches src/uart.c).

use crate::io::{inb, outb};

pub const COM1: u16 = 0x3f8;

/// Optional sink (GUI terminal window) receiving every byte logged.
/// Stored as a raw function pointer; set by the GUI.
pub static mut GUI_UART_SINK: Option<fn(&str)> = None;

pub fn init() {
    outb(COM1 + 1, 0x00); // Disable all interrupts
    outb(COM1 + 3, 0x80); // Enable DLAB (set baud rate divisor)
    outb(COM1 + 0, 0x03); // Divisor 3 -> 38400 baud
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03); // 8 bits, no parity, one stop bit (8N1)
    outb(COM1 + 2, 0xC7); // Enable FIFO, clear both, 14-byte threshold
    outb(COM1 + 4, 0x0B); // IRQs enabled, RTS/DSR set
}

fn tx_empty() -> bool {
    inb(COM1 + 5) & 0x20 != 0
}

pub fn putc(c: u8) {
    while !tx_empty() {}
    outb(COM1, c);
}

/// Poll one byte from the serial RX FIFO; None when empty.
/// Lets the serial console act as an input device too (\r -> \n).
pub fn getc() -> Option<u8> {
    if inb(COM1 + 5) & 0x01 == 0 {
        return None;
    }
    let mut c = inb(COM1);
    if c == b'\r' {
        c = b'\n'; // terminal sends CR on Enter
    }
    if c == 127 {
        c = 8; // DEL -> backspace
    }
    Some(c)
}

pub fn write_bytes(s: &[u8]) {
    for &b in s {
        if b == b'\n' {
            putc(b'\r');
        }
        putc(b);
    }
}
