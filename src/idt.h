#ifndef IDT_H
#define IDT_H

#include <stdint.h>

/* Full CPU state at interrupt entry, pushed by isr_common (isr.S).
 * Stack order at hand-off: r15 lowest ... rax, vector, err, rip, cs,
 * rflags, rsp, ss (user rsp/ss present only from user mode). */
struct irq_frame {
    uint64_t r15, r14, r13, r12, r11, r10, r9, r8;
    uint64_t rbp, rdi, rsi, rdx, rcx, rbx, rax;
    uint64_t vector, err;
    uint64_t rip, cs, rflags, rsp, ss;
};

/* Called from isr.S common stub. */
void isr_handler(struct irq_frame *f);

void idt_init(void);   /* build IDT, lidt */
void idt_enable(void); /* sti   */

#endif
