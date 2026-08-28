//! Pixel surfaces backed by physically contiguous pages (row-major xRGB8888).

use crate::mm::pmm::{self, PAGE_SIZE};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub struct Surface {
    pub w: i32,
    pub h: i32,
    pub pixels: *mut u32,
}

impl Surface {
    /// Allocate the header + pixel buffer from the PMM. None on OOM.
    pub fn create(w: i32, h: i32) -> Option<&'static mut Surface> {
        if w <= 0 || h <= 0 {
            return None;
        }
        let hdr_pa = pmm::alloc_pages(1, PAGE_SIZE as usize);
        if hdr_pa == 0 {
            return None;
        }
        let s = unsafe { &mut *(pmm::pa_to_va(hdr_pa) as *mut Surface) };
        let bytes = (w as usize) * (h as usize) * 4;
        let pages = bytes.div_ceil(PAGE_SIZE as usize);
        let px_pa = pmm::alloc_pages(pages, PAGE_SIZE as usize);
        if px_pa == 0 {
            pmm::free_page(hdr_pa);
            return None;
        }
        unsafe {
            core::ptr::write_bytes(pmm::pa_to_va(px_pa), 0, pages * PAGE_SIZE as usize);
        }
        s.w = w;
        s.h = h;
        s.pixels = pmm::pa_to_va(px_pa) as *mut u32;
        Some(s)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        let bytes = (self.w as usize) * (self.h as usize) * 4;
        let pages = bytes.div_ceil(PAGE_SIZE as usize);
        let px_pa = pmm::va_to_pa(self.pixels as u64);
        for i in 0..pages as u64 {
            pmm::free_page(px_pa + i * PAGE_SIZE);
        }
        pmm::free_page(pmm::va_to_pa(self as *mut Surface as u64));
    }
}

pub fn clip_to_surface(s: &Surface, r: &mut Rect) -> bool {
    if r.x < 0 {
        r.w += r.x;
        r.x = 0;
    }
    if r.y < 0 {
        r.h += r.y;
        r.y = 0;
    }
    if r.x + r.w > s.w {
        r.w = s.w - r.x;
    }
    if r.y + r.h > s.h {
        r.h = s.h - r.y;
    }
    r.w > 0 && r.h > 0
}

pub fn fill_rect(s: &mut Surface, mut r: Rect, color: u32) {
    if !clip_to_surface(s, &mut r) {
        return;
    }
    for y in r.y..r.y + r.h {
        for x in r.x..r.x + r.w {
            unsafe {
                *s.pixels.add((y * s.w + x) as usize) = color;
            }
        }
    }
}

pub fn blit(dst: &mut Surface, dx: i32, dy: i32, src: &Surface) {
    let mut r = Rect {
        x: dx,
        y: dy,
        w: src.w,
        h: src.h,
    };
    if !clip_to_surface(dst, &mut r) {
        return;
    }
    let sx = r.x - dx;
    let sy = r.y - dy;
    for y in 0..r.h {
        unsafe {
            let d = dst.pixels.add(((r.y + y) * dst.w + r.x) as usize);
            let s = src.pixels.add(((sy + y) * src.w + sx) as usize);
            core::ptr::copy_nonoverlapping(s, d, r.w as usize);
        }
    }
}
