//! 4-level paging, 4KiB pages. `PageTable` is the physical address of a PML4.

use crate::io;
use crate::mm::pmm::{self, PAGE_SIZE};

pub const PTE_P: u64 = 1 << 0; // present
pub const PTE_RW: u64 = 1 << 1; // writable
pub const PTE_US: u64 = 1 << 2; // user accessible
pub const PTE_WT: u64 = 1 << 3;
pub const PTE_CD: u64 = 1 << 4;
pub const PTE_PS: u64 = 1 << 7; // large page
pub const PTE_NX: u64 = 1 << 63; // no-execute

pub const PTE_ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

/// Physical address of a page table (PML4 for address spaces).
pub type PageTable = u64;

const PT_ENTRIES: usize = 512;

/// Kernel address space (valid after [`vmm_init`](init)).
pub static mut KERNEL_SPACE: PageTable = 0;

pub fn current() -> PageTable {
    io::read_cr3() & PTE_ADDR_MASK
}

pub fn switch(space: PageTable) {
    io::write_cr3(space);
}

fn table_mut(pa: u64) -> &'static mut [u64; PT_ENTRIES] {
    unsafe { &mut *(pmm::pa_to_va(pa) as *mut [u64; PT_ENTRIES]) }
}

fn zero_table(pa: u64) {
    unsafe {
        core::ptr::write_bytes(pmm::pa_to_va(pa), 0, PAGE_SIZE as usize);
    }
}

fn walk(space: PageTable, va: u64, create: bool, level: i32) -> Option<*mut u64> {
    let mut tbl = table_mut(space) as *mut [u64; PT_ENTRIES] as *mut u64;
    let mut lvl = 4;
    while lvl > level {
        let idx = ((va >> (12 + (lvl - 1) * 9)) & 0x1FF) as usize;
        let e = unsafe { *tbl.add(idx) };
        let mut e = e;
        if e & PTE_P == 0 {
            if !create {
                return None;
            }
            let np = pmm::alloc_page();
            if np == 0 {
                return None;
            }
            zero_table(np);
            unsafe {
                *tbl.add(idx) = np | PTE_P | PTE_RW | PTE_US;
                e = *tbl.add(idx);
            }
        } else if e & PTE_PS != 0 && lvl > 1 {
            return None; // walk through huge page unsupported
        }
        tbl = table_mut(e & PTE_ADDR_MASK) as *mut [u64; PT_ENTRIES] as *mut u64;
        lvl -= 1;
    }
    Some(tbl)
}

pub fn map(space: PageTable, va: u64, pa: u64, flags: u64) -> bool {
    let Some(pt) = walk(space, va, true, 1) else {
        return false;
    };
    let idx = ((va >> 12) & 0x1FF) as usize;
    unsafe {
        *pt.add(idx) = (pa & PTE_ADDR_MASK) | flags | PTE_P;
    }
    true
}

pub fn unmap(space: PageTable, va: u64) -> bool {
    let Some(pt) = walk(space, va, false, 1) else {
        return false;
    };
    let idx = ((va >> 12) & 0x1FF) as usize;
    unsafe {
        if *pt.add(idx) & PTE_P == 0 {
            return false;
        }
        *pt.add(idx) = 0;
    }
    crate::io::invlpg(va);
    true
}

/// Physical address of `va` including the page offset, 0 if unmapped.
pub fn translate(space: PageTable, va: u64) -> u64 {
    let Some(pt) = walk(space, va, false, 1) else {
        return 0;
    };
    let idx = ((va >> 12) & 0x1FF) as usize;
    let e = unsafe { *pt.add(idx) };
    if e & PTE_P == 0 {
        return 0;
    }
    (e & PTE_ADDR_MASK) + (va & (PAGE_SIZE - 1))
}

pub fn init() {
    // Fresh PML4; clone the high half (kernel text/data, HHDM, fb) from the
    // Limine-provided CR3, leave the low half empty for user maps later.
    let nl4 = pmm::alloc_page();
    zero_table(nl4);

    let old = table_mut(io::read_cr3() & PTE_ADDR_MASK);
    let new = table_mut(nl4);
    for i in 256..PT_ENTRIES {
        new[i] = old[i];
    }

    unsafe {
        KERNEL_SPACE = nl4;
    }
    io::write_cr3(nl4);
    crate::klog_info!(
        "vmm: kernel address space @cr3={:#x} (identity map dropped)",
        nl4
    );
}

/// Allocate a fresh address space that shares the kernel high half.
pub fn new_space() -> PageTable {
    let nl4 = pmm::alloc_page();
    if nl4 == 0 {
        return 0;
    }
    zero_table(nl4);
    let np = table_mut(nl4);
    let kp = table_mut(unsafe { KERNEL_SPACE });
    for i in 256..PT_ENTRIES {
        np[i] = kp[i]; // share kernel mappings
    }
    nl4
}

/// Free only the user-side tables + mapped user pages (entries 0..255).
pub fn destroy_space(space: PageTable) {
    let l4 = table_mut(space);
    for i4 in 0..256 {
        if l4[i4] & PTE_P == 0 {
            continue;
        }
        let l3 = table_mut(l4[i4] & PTE_ADDR_MASK);
        for i3 in 0..512 {
            if l3[i3] & PTE_P == 0 || l3[i3] & PTE_PS != 0 {
                continue;
            }
            let l2 = table_mut(l3[i3] & PTE_ADDR_MASK);
            for i2 in 0..512 {
                if l2[i2] & PTE_P == 0 || l2[i2] & PTE_PS != 0 {
                    continue;
                }
                let l1 = table_mut(l2[i2] & PTE_ADDR_MASK);
                for i1 in 0..512 {
                    if l1[i1] & PTE_P != 0 {
                        pmm::free_page(l1[i1] & PTE_ADDR_MASK);
                    }
                }
                pmm::free_page(l2[i2] & PTE_ADDR_MASK);
            }
            pmm::free_page(l3[i3] & PTE_ADDR_MASK);
        }
        pmm::free_page(l4[i4] & PTE_ADDR_MASK);
    }
    pmm::free_page(space);
}

pub fn selftest() {
    let pa = pmm::alloc_page();
    let va: u64 = 0x4000_0000; // lower half, kernel space
    let space = unsafe { KERNEL_SPACE };

    if !map(space, va, pa, PTE_RW) {
        crate::klog_err!("vmm test FAIL: map");
    }
    if translate(space, va) != pa {
        crate::klog_err!("vmm test FAIL: translate");
    }

    unsafe {
        core::ptr::write_volatile(va as *mut u64, 0xDEAD_BEEF_1234_5678);
        if core::ptr::read_volatile(pmm::pa_to_va(pa) as *const u64) != 0xDEAD_BEEF_1234_5678 {
            crate::klog_err!("vmm test FAIL: data mismatch");
        }
    }

    if !unmap(space, va) {
        crate::klog_err!("vmm test FAIL: unmap");
    }
    if translate(space, va) != 0 {
        crate::klog_err!("vmm test FAIL: translate after unmap");
    }

    pmm::free_page(pa);
    crate::klog_info!("vmm: selftest ok (va={:#x} -> pa={:#x})", va, pa);
}
