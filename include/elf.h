/* Minimal 64-bit ELF definitions (ELF-64 spec, x86-64 subset). */
#ifndef ELF_H
#define ELF_H

#include <stdint.h>

typedef struct {
    uint8_t  e_ident[16];   /* 0x7f 'E' 'L' 'F', class 2=64, data 1=LE ... */
    uint16_t e_type;        /* 2 = ET_EXEC */
    uint16_t e_machine;     /* 62 = AMD64 */
    uint32_t e_version;
    uint64_t e_entry;
    uint64_t e_phoff;
    uint64_t e_shoff;
    uint32_t e_flags;
    uint16_t e_ehsize;
    uint16_t e_phentsize;
    uint16_t e_phnum;
    uint16_t e_shentsize;
    uint16_t e_shnum;
    uint16_t e_shstrndx;
} Elf64_Ehdr;

typedef struct {
    uint32_t p_type;        /* 1 = PT_LOAD */
    uint32_t p_flags;       /* 1=X 2=W 4=R */
    uint64_t p_offset;
    uint64_t p_vaddr;
    uint64_t p_paddr;
    uint64_t p_filesz;
    uint64_t p_memsz;
    uint64_t p_align;
} Elf64_Phdr;

#define PT_LOAD 1
#define PF_X 1
#define PF_W 2
#define PF_R 4

#endif
