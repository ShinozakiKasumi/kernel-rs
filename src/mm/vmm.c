#include "mm/vmm.h"
#include "mm/pmm.h"
#include "kprintf.h"
#include "lib/string.h"

#define PT_ENTRIES 512

page_table_t kernel_space;

static inline uint64_t read_cr3(void) {
    uint64_t v;
    __asm__ volatile ("mov %%cr3, %0" : "=r"(v));
    return v & PTE_ADDR_MASK;
}

static inline void write_cr3(uint64_t pa) {
    __asm__ volatile ("mov %0, %%cr3" : : "r"(pa) : "memory");
}

static inline void zero_table(uint64_t pa) {
    memset(PA_TO_VA(pa), 0, PAGE_SIZE);
}

page_table_t vmm_current(void) {
    return (page_table_t)read_cr3();
}

void vmm_switch(page_table_t space) {
    write_cr3((uint64_t)space);
}

static uint64_t *walk(page_table_t space, uint64_t va, bool create, int level) {
    uint64_t *tbl = PA_TO_VA((uint64_t)space);
    for (int lvl = 4; lvl > level; lvl--) {
        unsigned idx = (va >> (12 + (lvl - 1) * 9)) & 0x1FF;
        uint64_t e = tbl[idx];
        if (!(e & PTE_P)) {
            if (!create) return NULL;
            uint64_t np = pmm_alloc_page();
            if (!np) return NULL;
            zero_table(np);
            tbl[idx] = np | PTE_P | PTE_RW | PTE_US;
            e = tbl[idx];
        } else if ((e & PTE_PS) && lvl > 1) {
            return NULL;  /* walk through huge page unsupported */
        }
        tbl = PA_TO_VA(e & PTE_ADDR_MASK);
    }
    return tbl;
}

bool vmm_map(page_table_t space, uint64_t va, uint64_t pa, uint64_t flags) {
    uint64_t *pt = walk(space, va, true, 1);
    if (!pt) return false;
    unsigned idx = (va >> 12) & 0x1FF;
    pt[idx] = (pa & PTE_ADDR_MASK) | flags | PTE_P;
    return true;
}

bool vmm_unmap(page_table_t space, uint64_t va) {
    uint64_t *pt = walk(space, va, false, 1);
    if (!pt) return false;
    unsigned idx = (va >> 12) & 0x1FF;
    if (!(pt[idx] & PTE_P)) return false;
    pt[idx] = 0;
    vmm_flush_tlb(va);
    return true;
}

uint64_t vmm_translate(page_table_t space, uint64_t va) {
    uint64_t *pt = walk(space, va, false, 1);
    if (!pt) return 0;
    uint64_t e = pt[(va >> 12) & 0x1FF];
    if (!(e & PTE_P)) return 0;
    return (e & PTE_ADDR_MASK) + (va & (PAGE_SIZE - 1));
}

void vmm_init(void) {
    /* Fresh PML4; clone the high half (kernel text/data, HHDM, fb) from the
     * Limine-provided CR3, leave the low half empty for user maps later. */
    uint64_t nl4 = pmm_alloc_page();
    zero_table(nl4);

    uint64_t *old_pml4 = PA_TO_VA(read_cr3());
    uint64_t *new_pml4 = PA_TO_VA(nl4);
    for (int i = 256; i < PT_ENTRIES; i++)
        new_pml4[i] = old_pml4[i];

    kernel_space = (page_table_t)nl4;
    write_cr3(nl4);
    KLOG_INFO("vmm: kernel address space @cr3=%#lx (identity map dropped)", nl4);
}

page_table_t vmm_new_space(void) {
    uint64_t nl4 = pmm_alloc_page();
    if (!nl4) return NULL;
    zero_table(nl4);
    uint64_t *np = PA_TO_VA(nl4);
    uint64_t *kp = PA_TO_VA((uint64_t)kernel_space);
    for (int i = 256; i < PT_ENTRIES; i++)
        np[i] = kp[i];   /* share kernel mappings */
    return (page_table_t)nl4;
}

/* Free only the user-side tables + mapped user pages (entries 0..255). */
void vmm_destroy_space(page_table_t space) {
    uint64_t *l4 = PA_TO_VA((uint64_t)space);
    for (int i4 = 0; i4 < 256; i4++) {
        if (!(l4[i4] & PTE_P)) continue;
        uint64_t *l3 = PA_TO_VA(l4[i4] & PTE_ADDR_MASK);
        for (int i3 = 0; i3 < 512; i3++) {
            if (!(l3[i3] & PTE_P) || (l3[i3] & PTE_PS)) continue;
            uint64_t *l2 = PA_TO_VA(l3[i3] & PTE_ADDR_MASK);
            for (int i2 = 0; i2 < 512; i2++) {
                if (!(l2[i2] & PTE_P) || (l2[i2] & PTE_PS)) continue;
                uint64_t *l1 = PA_TO_VA(l2[i2] & PTE_ADDR_MASK);
                for (int i1 = 0; i1 < 512; i1++)
                    if (l1[i1] & PTE_P)
                        pmm_free_page(l1[i1] & PTE_ADDR_MASK);
                pmm_free_page(l2[i2] & PTE_ADDR_MASK);
            }
            pmm_free_page(l3[i3] & PTE_ADDR_MASK);
        }
        pmm_free_page(l4[i4] & PTE_ADDR_MASK);
    }
    pmm_free_page((uint64_t)space);
}

void vmm_selftest(void) {
    uint64_t pa = pmm_alloc_page();
    uint64_t va = 0x40000000ULL;   /* lower half, kernel space */

    if (!vmm_map(kernel_space, va, pa, PTE_RW))
        KLOG_ERR("vmm test FAIL: map");
    if (vmm_translate(kernel_space, va) != pa)
        KLOG_ERR("vmm test FAIL: translate");

    *(volatile uint64_t *)va = 0xDEADBEEF12345678ULL;
    if (*(volatile uint64_t *)PA_TO_VA(pa) != 0xDEADBEEF12345678ULL)
        KLOG_ERR("vmm test FAIL: data mismatch");

    if (!vmm_unmap(kernel_space, va))
        KLOG_ERR("vmm test FAIL: unmap");
    if (vmm_translate(kernel_space, va) != 0)
        KLOG_ERR("vmm test FAIL: translate after unmap");

    pmm_free_page(pa);
    KLOG_INFO("vmm: selftest ok (va=%#lx -> pa=%#lx)", va, pa);
}
