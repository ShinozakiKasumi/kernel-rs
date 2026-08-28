//! ELF loader: maps PT_LOAD segments into a fresh address space.
//! Copies via the kernel HHDM so the target space never needs to be active.

use crate::mm::pmm::{self, PAGE_SIZE};
use crate::mm::vmm::{self, PageTable, PTE_NX, PTE_RW, PTE_US};
use crate::proc::elf_defs::*;

fn elf_flags_to_pte(pf: u32) -> u64 {
    let mut f = PTE_US; // user accessible
    if pf & PF_W != 0 {
        f |= PTE_RW;
    }
    if pf & PF_X == 0 {
        f |= PTE_NX;
    }
    f
}

fn map_segment_page(space: PageTable, va: u64, pte_flags: u64) -> bool {
    let pa = pmm::alloc_page();
    if pa == 0 {
        return false;
    }
    unsafe {
        core::ptr::write_bytes(pmm::pa_to_va(pa), 0, PAGE_SIZE as usize);
    }
    vmm::map(space, va, pa, pte_flags)
}

/// Returns the entry address, or 0 on failure.
pub fn elf_load(space: PageTable, data: &[u8]) -> u64 {
    if data.len() < core::mem::size_of::<Elf64Ehdr>() {
        crate::klog_err!("elf: too small");
        return 0;
    }
    let eh = unsafe { &*(data.as_ptr() as *const Elf64Ehdr) };
    if eh.e_ident[0] != 0x7F || eh.e_ident[1] != b'E' || eh.e_ident[2] != b'L'
        || eh.e_ident[3] != b'F'
    {
        crate::klog_err!("elf: bad magic");
        return 0;
    }
    if eh.e_ident[4] != 2 || eh.e_ident[5] != 1 {
        crate::klog_err!("elf: need ELF64 little-endian");
        return 0;
    }
    if eh.e_type != 2 || eh.e_machine != 62 {
        crate::klog_err!("elf: need ET_EXEC x86-64");
        return 0;
    }

    for i in 0..eh.e_phnum as usize {
        let ph_off = eh.e_phoff as usize + i * eh.e_phentsize as usize;
        let ph = unsafe { &*(data.as_ptr().add(ph_off) as *const Elf64Phdr) };
        if ph.p_type != PT_LOAD || ph.p_memsz == 0 {
            continue;
        }

        let flags = elf_flags_to_pte(ph.p_flags);
        let start = ph.p_vaddr & !(PAGE_SIZE - 1);
        let end = (ph.p_vaddr + ph.p_memsz + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        let mut va = start;
        while va < end {
            if !map_segment_page(space, va, flags) {
                crate::klog_err!("elf: out of memory");
                return 0;
            }

            // copy the overlap of [va, va+PAGE) with the file segment
            let seg_end = ph.p_vaddr + ph.p_filesz;
            let c_lo = va.max(ph.p_vaddr);
            let c_hi = (va + PAGE_SIZE).min(seg_end);
            if c_hi > c_lo {
                let pa = vmm::translate(space, va);
                unsafe {
                    let dst = pmm::pa_to_va(pa).add((c_lo - va) as usize);
                    let src = data
                        .as_ptr()
                        .add((ph.p_offset + (c_lo - ph.p_vaddr)) as usize);
                    core::ptr::copy_nonoverlapping(src, dst, (c_hi - c_lo) as usize);
                }
            }
            // bss remainder is already zero
            va += PAGE_SIZE;
        }
        crate::klog_info!(
            "elf: segment {:#x}..{:#x} flags={:#x}",
            start,
            end,
            ph.p_flags
        );
    }
    eh.e_entry
}
