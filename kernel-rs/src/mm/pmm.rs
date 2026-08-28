//! Physical page allocator: static bitmap over the Limine memory map.
//!
//! bit = 1 -> free, 0 -> used/unusable. Only Limine "usable" entries become
//! free; kernel image, modules, reclaimable and firmware regions stay used.

use core::cell::UnsafeCell;

use crate::limine;

pub const PAGE_SIZE: u64 = 4096;

const PMM_MAX_PHYS: u64 = 512 << 20; // cap: 512 MiB
const BITMAP_PAGES: usize = (PMM_MAX_PHYS / PAGE_SIZE) as usize; // 131072 pages
const BITMAP_WORDS: usize = BITMAP_PAGES / 64; // 2048 words = 16KiB

struct BitmapCell(UnsafeCell<[u64; BITMAP_WORDS]>);
unsafe impl Sync for BitmapCell {}

/// Starts zeroed: everything used.
static BITMAP: BitmapCell = BitmapCell(UnsafeCell::new([0; BITMAP_WORDS]));

static mut TOTAL_PAGES: usize = 0;
static mut FREE_PAGES: usize = 0;

/// Higher-half direct map base, provided by Limine.
pub static mut HHDM_OFFSET: u64 = 0;

#[inline(always)]
pub fn pa_to_va(pa: u64) -> *mut u8 {
    unsafe { (pa + HHDM_OFFSET) as *mut u8 }
}

#[inline(always)]
pub fn va_to_pa(va: u64) -> u64 {
    unsafe { va - HHDM_OFFSET }
}

fn bm_set(i: usize) {
    unsafe { (*BITMAP.0.get())[i >> 6] |= 1 << (i & 63) };
}

fn bm_clear(i: usize) {
    unsafe { (*BITMAP.0.get())[i >> 6] &= !(1 << (i & 63)) };
}

fn bm_test(i: usize) -> bool {
    unsafe { ((*BITMAP.0.get())[i >> 6] >> (i & 63)) & 1 != 0 }
}

pub fn init() {
    let (mm, hhdm) = match (limine::memmap_response(), limine::hhdm_response()) {
        (Some(mm), Some(h)) => (mm, h),
        _ => {
            crate::klog_err!("pmm: missing limine memmap/hhdm response");
            loop {
                crate::io::hlt();
            }
        }
    };

    unsafe {
        HHDM_OFFSET = hhdm.offset;
        crate::klog_info!("pmm: hhdm offset={:#x}", HHDM_OFFSET);
    }

    let mut top: u64 = 0;

    for i in 0..mm.entry_count as usize {
        let e = unsafe { &*mm.entries.add(i).read() };
        if e.typ != limine::LIMINE_MEMMAP_USABLE {
            continue;
        }

        let base = (e.base + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let mut end = (e.base + e.length) & !(PAGE_SIZE - 1);
        if end > PMM_MAX_PHYS {
            end = PMM_MAX_PHYS;
        }
        if base >= end {
            continue;
        }

        let mut p = base;
        while p < end {
            bm_set((p / PAGE_SIZE) as usize);
            unsafe {
                FREE_PAGES += 1;
                TOTAL_PAGES += 1;
            }
            p += PAGE_SIZE;
        }
        if end > top {
            top = end;
        }
    }
    unsafe {
        crate::klog_info!(
            "pmm: {} pages free ({} MiB), top={:#x}",
            FREE_PAGES,
            FREE_PAGES as u64 * PAGE_SIZE >> 20,
            top
        );
    }
}

pub fn alloc_page() -> u64 {
    for w in 0..BITMAP_WORDS {
        let word = unsafe { (*BITMAP.0.get())[w] };
        if word == 0 {
            continue;
        }
        let bit = word.trailing_zeros() as usize;
        let idx = (w << 6) + bit;
        bm_clear(idx);
        unsafe {
            FREE_PAGES -= 1;
        }
        return idx as u64 * PAGE_SIZE;
    }
    0
}

pub fn alloc_pages(count: usize, align: usize) -> u64 {
    let align = align.max(PAGE_SIZE as usize);
    let align_pages = align / PAGE_SIZE as usize;

    let mut i = 0usize;
    while i + count <= BITMAP_PAGES {
        if i % align_pages != 0 {
            i -= i % align_pages;
            i += 1; // C for-loop increment on `continue`
            continue;
        }
        let mut j = 0;
        while j < count {
            if !bm_test(i + j) {
                break;
            }
            j += 1;
        }
        if j == count {
            for k in 0..count {
                bm_clear(i + k);
            }
            unsafe {
                FREE_PAGES -= count;
            }
            return i as u64 * PAGE_SIZE;
        }
        i += j; // skip past the used page (loop adds 1)
        i += 1;
    }
    0
}

pub fn free_page(pa: u64) {
    let idx = (pa / PAGE_SIZE) as usize;
    if pa % PAGE_SIZE != 0 || idx >= BITMAP_PAGES || bm_test(idx) {
        crate::klog_err!("pmm: bad free of {:#x}", pa);
        return;
    }
    bm_set(idx);
    unsafe {
        FREE_PAGES += 1;
    }
}

pub fn free_count() -> usize {
    unsafe { FREE_PAGES }
}

pub fn total_count() -> usize {
    unsafe { TOTAL_PAGES }
}

pub fn selftest() {
    let before = free_count();

    let a = alloc_page();
    let b = alloc_page();
    if a == 0 || b == 0 || a == b || a % PAGE_SIZE != 0 || b % PAGE_SIZE != 0 {
        crate::klog_err!("pmm test FAIL: a={:#x} b={:#x}", a, b);
    }
    if free_count() != before - 2 {
        crate::klog_err!("pmm test FAIL: count after alloc = {}", free_count());
    }

    free_page(a);
    if free_count() != before - 1 {
        crate::klog_err!("pmm test FAIL: count after one free");
    }

    let run = alloc_pages(4, PAGE_SIZE as usize);
    if run == 0 || run % PAGE_SIZE != 0 {
        crate::klog_err!("pmm test FAIL: 4-page run");
    }
    free_page(b);
    crate::klog_info!("pmm: selftest ok (a={:#x} b={:#x} run={:#x})", a, b, run);
}
