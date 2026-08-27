/* Process layer: spawn a user process from an in-memory ELF. */
#include "proc.h"
#include "sched.h"
#include "mm/pmm.h"
#include "mm/vmm.h"
#include "kprintf.h"
#include "lib/string.h"

#define USER_STACK_TOP 0x0000700000000000ULL
#define USER_STACK_PAGES 4

int proc_spawn_elf(const char *name, const void *elf_data, uint64_t size) {
    page_table_t space = vmm_new_space();
    if (!space) { KLOG_ERR("proc: no address space"); return -1; }

    uint64_t entry = elf_load(space, elf_data, size);
    if (!entry) { vmm_destroy_space(space); return -1; }

    /* user stack right below USER_STACK_TOP */
    for (uint64_t i = 0; i < USER_STACK_PAGES; i++) {
        uint64_t va = USER_STACK_TOP - (USER_STACK_PAGES - i) * PAGE_SIZE;
        uint64_t pa = pmm_alloc_page();
        if (!pa || !vmm_map(space, va, pa, PTE_US | PTE_RW | PTE_NX)) {
            KLOG_ERR("proc: stack alloc failed");
            vmm_destroy_space(space);
            return -1;
        }
        memset(PA_TO_VA(pa), 0, PAGE_SIZE);
    }

    return thread_create_user(name, entry, USER_STACK_TOP, space);
}
