/* ELF loader: maps PT_LOAD segments into a fresh address space.
 * Copies via the kernel HHDM so the target space never needs to be active. */
#include "proc.h"
#include "elf.h"
#include "mm/pmm.h"
#include "mm/vmm.h"
#include "kprintf.h"
#include "lib/string.h"

static uint64_t elf_flags_to_pte(uint32_t pf) {
    uint64_t f = PTE_US;                  /* user accessible */
    if (pf & PF_W) f |= PTE_RW;
    if (!(pf & PF_X)) f |= PTE_NX;
    return f;
}

static bool map_segment_page(page_table_t space, uint64_t va, uint64_t pte_flags) {
    uint64_t pa = pmm_alloc_page();
    if (!pa) return false;
    memset(PA_TO_VA(pa), 0, PAGE_SIZE);
    return vmm_map(space, va, pa, pte_flags);
}

uint64_t elf_load(page_table_t space, const void *data, uint64_t size) {
    const Elf64_Ehdr *eh = data;
    if (size < sizeof *eh) { KLOG_ERR("elf: too small"); return 0; }
    if (eh->e_ident[0] != 0x7F || eh->e_ident[1] != 'E' ||
        eh->e_ident[2] != 'L'  || eh->e_ident[3] != 'F') {
        KLOG_ERR("elf: bad magic"); return 0;
    }
    if (eh->e_ident[4] != 2 || eh->e_ident[5] != 1) {
        KLOG_ERR("elf: need ELF64 little-endian"); return 0;
    }
    if (eh->e_type != 2 || eh->e_machine != 62) {
        KLOG_ERR("elf: need ET_EXEC x86-64"); return 0;
    }

    for (int i = 0; i < eh->e_phnum; i++) {
        const Elf64_Phdr *ph = (const Elf64_Phdr *)
            ((const uint8_t *)data + eh->e_phoff + i * eh->e_phentsize);
        if (ph->p_type != PT_LOAD || ph->p_memsz == 0) continue;

        uint64_t flags = elf_flags_to_pte(ph->p_flags);
        uint64_t start = ph->p_vaddr & ~(PAGE_SIZE - 1);
        uint64_t end   = (ph->p_vaddr + ph->p_memsz + PAGE_SIZE - 1)
                         & ~(PAGE_SIZE - 1);

        for (uint64_t va = start; va < end; va += PAGE_SIZE) {
            if (!map_segment_page(space, va, flags)) {
                KLOG_ERR("elf: out of memory"); return 0;
            }

            /* copy the overlap of [va, va+PAGE) with the file segment */
            uint64_t seg_end = ph->p_vaddr + ph->p_filesz;
            uint64_t c_lo = va > ph->p_vaddr ? va : ph->p_vaddr;
            uint64_t c_hi = va + PAGE_SIZE < seg_end ? va + PAGE_SIZE : seg_end;
            if (c_hi > c_lo) {
                uint64_t pa = vmm_translate(space, va);
                memcpy(PA_TO_VA(pa) + (c_lo - va),
                       (const uint8_t *)data + ph->p_offset + (c_lo - ph->p_vaddr),
                       c_hi - c_lo);
            }
            /* bss remainder is already zero */
        }
        KLOG_INFO("elf: segment %#lx..%#lx flags=%x",
                  start, end, ph->p_flags);
    }
    return eh->e_entry;
}
