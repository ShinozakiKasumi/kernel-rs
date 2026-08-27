#ifndef MM_VMM_H
#define MM_VMM_H

#include <stdint.h>
#include <stdbool.h>

/* 4-level paging, 4KiB pages. */

#define PTE_P    (1ULL << 0)   /* present            */
#define PTE_RW   (1ULL << 1)   /* writable           */
#define PTE_US   (1ULL << 2)   /* user accessible    */
#define PTE_WT   (1ULL << 3)
#define PTE_CD   (1ULL << 4)
#define PTE_PS   (1ULL << 7)   /* large page         */
#define PTE_NX   (1ULL << 63)  /* no-execute         */

#define PTE_ADDR_MASK 0x000FFFFFFFFFF000ULL

typedef uint64_t *page_table_t;   /* physical address of a table */

/* Kernel address space (global after vmm_init). */
extern page_table_t kernel_space;

/* Build our own kernel address space (clones Limine's high-half mappings,
 * drops the identity map) and switch CR3 to it. */
void vmm_init(void);

/* Allocate a fresh address space that shares the kernel high half. */
page_table_t vmm_new_space(void);
void         vmm_destroy_space(page_table_t space);
void         vmm_switch(page_table_t space);
page_table_t vmm_current(void);

bool     vmm_map(page_table_t space, uint64_t va, uint64_t pa, uint64_t flags);
bool     vmm_unmap(page_table_t space, uint64_t va);
uint64_t vmm_translate(page_table_t space, uint64_t va);   /* 0 if unmapped */

static inline void vmm_flush_tlb(uint64_t va) {
    __asm__ volatile ("invlpg (%0)" : : "r"(va) : "memory");
}

void vmm_selftest(void);

#endif
