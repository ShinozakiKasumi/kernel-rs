#ifndef PROC_H
#define PROC_H

#include <stdint.h>
#include <stdbool.h>
#include "mm/vmm.h"

/* Load an ET_EXEC x86-64 ELF image from memory into `space`.
 * Returns the entry VA, or 0 on failure. */
uint64_t elf_load(page_table_t space, const void *data, uint64_t size);

/* Create a user process: new address space, embedded module bytes as ELF,
 * 16KiB user stack, ready-to-run thread. Returns tid or -1. */
int  proc_spawn_elf(const char *name, const void *elf_data, uint64_t size);

#endif
