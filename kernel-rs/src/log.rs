//! Kernel logging: `klog!` formats like Rust's `format_args!`, writes to the
//! serial port, and mirrors the whole line to the GUI terminal sink when set.

use core::fmt::{self, Write};

struct Sink {
    buf: [u8; 192],
    len: usize,
}

impl Sink {
    fn push_byte(&mut self, b: u8) {
        crate::uart::putc(b);
        if self.len < self.buf.len() - 1 {
            self.buf[self.len] = b;
            self.len += 1;
        }
    }
}

impl Write for Sink {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            if b == b'\n' {
                self.push_byte(b'\r');
            }
            self.push_byte(b);
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn _klog(args: fmt::Arguments) {
    let mut sink = Sink {
        buf: [0; 192],
        len: 0,
    };
    let _ = sink.write_fmt(args);
    if sink.len > 0 {
        unsafe {
            if let Some(sink_fn) = crate::uart::GUI_UART_SINK {
                if let Ok(s) = core::str::from_utf8(&sink.buf[..sink.len]) {
                    sink_fn(s);
                }
            }
        }
    }
}

#[macro_export]
macro_rules! klog {
    ($($arg:tt)*) => {
        $crate::log::_klog(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! klog_info {
    ($($arg:tt)*) => {
        $crate::klog!("[info] ");
        $crate::klog!($($arg)*);
        $crate::klog!("\n");
    };
}

#[macro_export]
macro_rules! klog_warn {
    ($($arg:tt)*) => {
        $crate::klog!("[warn] ");
        $crate::klog!($($arg)*);
        $crate::klog!("\n");
    };
}

#[macro_export]
macro_rules! klog_err {
    ($($arg:tt)*) => {
        $crate::klog!("[err ] ");
        $crate::klog!($($arg)*);
        $crate::klog!("\n");
    };
}
