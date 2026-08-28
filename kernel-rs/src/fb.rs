//! Framebuffer over the Limine-provided linear buffer.

use crate::limine;

static mut FB_ADDR: *mut u8 = core::ptr::null_mut();
static mut FB_PITCH: u64 = 0;
static mut FB_W: u32 = 0;
static mut FB_H: u32 = 0;
static mut FB_BPP: u8 = 0;
static mut R_SHIFT: u8 = 0;
static mut G_SHIFT: u8 = 0;
static mut B_SHIFT: u8 = 0;
static mut FB_READY: bool = false;

pub type Color = u32;

pub fn init() -> i32 {
    let Some(r) = limine::framebuffer_response() else {
        crate::klog_err!("no framebuffer from bootloader");
        return -1;
    };
    if r.framebuffer_count < 1 {
        crate::klog_err!("no framebuffer from bootloader");
        return -1;
    }
    let f = unsafe { &*r.framebuffers.read() };
    unsafe {
        FB_ADDR = f.address;
        FB_PITCH = f.pitch;
        FB_W = f.width as u32;
        FB_H = f.height as u32;
        FB_BPP = f.bpp as u8;
        R_SHIFT = f.red_mask_shift;
        G_SHIFT = f.green_mask_shift;
        B_SHIFT = f.blue_mask_shift;
        FB_READY = true;
        crate::klog_info!(
            "fb: {}x{} {}bpp pitch={} addr={:#x}",
            FB_W,
            FB_H,
            FB_BPP,
            FB_PITCH,
            f.address as u64
        );
        crate::klog_info!(
            "fb: rgb shifts r={} g={} b={}",
            R_SHIFT,
            G_SHIFT,
            B_SHIFT
        );
    }
    0
}

pub fn available() -> bool {
    unsafe { FB_READY }
}

pub fn width() -> u32 {
    unsafe { FB_W }
}

pub fn height() -> u32 {
    unsafe { FB_H }
}

pub fn pixels() -> *mut u8 {
    if unsafe { FB_READY } {
        unsafe { FB_ADDR }
    } else {
        core::ptr::null_mut()
    }
}

pub fn pitch_bytes() -> u32 {
    unsafe { FB_PITCH as u32 }
}

pub fn rgb(r: u8, g: u8, b: u8) -> Color {
    unsafe { ((r as u32) << R_SHIFT) | ((g as u32) << G_SHIFT) | ((b as u32) << B_SHIFT) }
}

pub fn put_pixel(x: u32, y: u32, c: Color) {
    unsafe {
        if !FB_READY || x >= FB_W || y >= FB_H {
            return;
        }
        let row = FB_ADDR.add(y as usize * FB_PITCH as usize);
        if FB_BPP == 32 {
            *(row as *mut u32).add(x as usize) = c;
        } else if FB_BPP == 24 {
            let p = row.add(x as usize * 3);
            *p = (c & 0xFF) as u8;
            *p.add(1) = ((c >> 8) & 0xFF) as u8;
            *p.add(2) = ((c >> 16) & 0xFF) as u8;
        }
        // other depths: unsupported
    }
}

pub fn fill_rect(x: u32, y: u32, mut w: u32, mut h: u32, c: Color) {
    unsafe {
        if !FB_READY {
            return;
        }
        if x >= FB_W || y >= FB_H {
            return;
        }
        if x + w > FB_W {
            w = FB_W - x;
        }
        if y + h > FB_H {
            h = FB_H - y;
        }

        for j in 0..h {
            let row = FB_ADDR.add((y + j) as usize * FB_PITCH as usize);
            if FB_BPP == 32 {
                let p = (row as *mut u32).add(x as usize);
                for i in 0..w as usize {
                    *p.add(i) = c;
                }
            } else {
                for i in 0..w {
                    put_pixel(x + i, y + j, c);
                }
            }
        }
    }
}

pub fn clear(c: Color) {
    fill_rect(0, 0, width(), height(), c);
}

pub fn test_pattern() {
    if !available() {
        return;
    }

    // background
    clear(rgb(16, 16, 24));

    // 8 classic colour bars, top half
    const BARS: [[u8; 3]; 8] = [
        [255, 255, 255],
        [255, 255, 0],
        [0, 255, 255],
        [0, 255, 0],
        [255, 0, 255],
        [255, 0, 0],
        [0, 0, 255],
        [0, 0, 0],
    ];
    let (fb_w, fb_h) = (width(), height());
    let bar_w = fb_w / 8;
    let bar_h = fb_h / 2;
    for (i, b) in BARS.iter().enumerate() {
        fill_rect(i as u32 * bar_w, 0, bar_w, bar_h, rgb(b[0], b[1], b[2]));
    }

    // horizontal gradient, bottom half (green = f(x), blue = f(y))
    let mut y = bar_h;
    while y < fb_h {
        let mut x = 0;
        while x < fb_w {
            fill_rect(
                x,
                y,
                64,
                2,
                rgb(
                    0,
                    (x * 255 / fb_w) as u8,
                    ((y - bar_h) * 255 / (fb_h - bar_h)) as u8,
                ),
            );
            x += 64;
        }
        y += 2;
    }

    // white crosshairs to verify coordinate math at extremes
    fill_rect(fb_w / 2 - 1, 0, 2, fb_h, rgb(255, 255, 255));
    fill_rect(0, fb_h / 2 - 1, fb_w, 2, rgb(255, 255, 255));
}
