/* GDT: flat 64-bit kernel code/data segments.
 * Limine already loads its own GDT; we install ours so segment layout is
 * stable for user segments (M7) and TSS (M6).
 *
 * Layout:
 *   0x00 null
 *   0x08 kernel code (64-bit, DPL 0)
 *   0x10 kernel data          (DPL 0)
 *   0x18 user data            (DPL 3)   -- reserved, used in M7
 *   0x20 user code (64-bit)   (DPL 3)   -- reserved, used in M7
 *   0x28 TSS (16 bytes)        -- ring3 -> ring0 interrupt stack
 */
#include "gdt.h"
#include "kprintf.h"
#include "lib/string.h"
#include <stdint.h>

static uint64_t gdt[7];

static const uint16_t KERNEL_CS = 0x08;
static const uint16_t KERNEL_DS = 0x10;

struct tss {
    uint32_t reserved0;
    uint64_t rsp0, rsp1, rsp2;
    uint64_t reserved1;
    uint64_t ist1, ist2, ist3, ist4, ist5, ist6, ist7;
    uint64_t reserved2;
    uint16_t reserved3;
    uint16_t iomap_base;
} __attribute__((packed));

static struct tss tss;
static uint64_t boot_irq_stack[2048];   /* 16KiB fallback for rsp0 */

struct gdtr {
    uint16_t limit;
    uint64_t base;
} __attribute__((packed));

void gdt_set_kernel_stack(uint64_t rsp0) {
    tss.rsp0 = rsp0;
}

static uint64_t make_tss_desc_low(uint64_t base, uint32_t limit) {
    return ((uint64_t)(limit & 0xFFFF))
         | ((base & 0xFFFFFF) << 16)
         | (0x89ULL << 40)                       /* present, type=64-bit TSS */
         | (((uint64_t)(limit >> 16) & 0xF) << 48)
         | (((base >> 24) & 0xFF) << 56);
}

void gdt_init(void) {
    gdt[0] = 0x0000000000000000ULL; /* null          */
    gdt[1] = 0x00AF9A000000FFFFULL; /* kernel code   */
    gdt[2] = 0x00AF92000000FFFFULL; /* kernel data   */
    gdt[3] = 0x00AFF2000000FFFFULL; /* user data     */
    gdt[4] = 0x00AFFA000000FFFFULL; /* user code     */

    tss.rsp0       = (uint64_t)&boot_irq_stack + sizeof boot_irq_stack;
    tss.iomap_base = sizeof tss;
    uint64_t base  = (uint64_t)&tss;
    uint32_t limit = sizeof(tss) - 1;
    gdt[5] = make_tss_desc_low(base, limit);
    gdt[6] = base >> 32;

    struct gdtr gdtr = {
        .limit = sizeof(gdt) - 1,
        .base  = (uint64_t)&gdt,
    };

    __asm__ volatile (
        "lgdt %0\n\t"
        /* reload data segments */
        "mov %1, %%ds\n\t"
        "mov %1, %%es\n\t"
        "mov %1, %%fs\n\t"
        "mov %1, %%gs\n\t"
        "mov %1, %%ss\n\t"
        /* reload cs via far return */
        "pushq %2\n\t"
        "leaq 1f(%%rip), %%rax\n\t"
        "pushq %%rax\n\t"
        "lretq\n\t"
        "1:\n\t"
        :
        : "m"(gdtr), "r"(KERNEL_DS), "r"((uint64_t)KERNEL_CS)
        : "rax", "memory");

    KLOG_INFO("gdt: installed (cs=%#lx ds=%#lx)",
              (unsigned long)KERNEL_CS, (unsigned long)KERNEL_DS);

    __asm__ volatile ("ltr %w0" : : "r"((uint16_t)0x28));
    KLOG_INFO("gdt: tss loaded, rsp0=%p", (void *)tss.rsp0);
}
