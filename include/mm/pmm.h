#ifndef MM_PMM_H
#define MM_PMM_H

#include <stdint.h>
#include <stddef.h>

#define PAGE_SIZE 4096ULL

/* Higher-half direct map base, provided by Limine. */
extern uint64_t hhdm_offset;

#define PA_TO_VA(pa) ((void *)((pa) + hhdm_offset))
#define VA_TO_PA(va) (((uint64_t)(va)) - hhdm_offset)

void     pmm_init(void);

/* Single-page alloc/free (returns/accepts physical addresses, 4K-aligned). */
uintptr_t pmm_alloc_page(void);
void      pmm_free_page(uintptr_t pa);

/* Aligned contiguous run: `align` and `count` in bytes (both powers of two
 * for align; count any page multiple). Returns 0 on failure. */
uintptr_t pmm_alloc_pages(size_t count, size_t align);

size_t pmm_free_count(void);   /* free pages */
size_t pmm_total_count(void);  /* total managed pages */

void pmm_selftest(void);

#endif
