#include "pic.h"
#include "kprintf.h"

#define PIC1_CMD  0x20
#define PIC1_DATA 0x21
#define PIC2_CMD  0xA0
#define PIC2_DATA 0xA1

static inline void outb(uint16_t port, uint8_t val) {
    __asm__ volatile ("outb %0, %1" : : "a"(val), "dN"(port));
}

static inline void io_wait(void) {
    __asm__ volatile ("outb %%al, $0x80" : : "a"(0));
}

/* Remap to 0x20/0x28 (above the 32 exception vectors) and mask everything
 * except the IRQ lines we actively use later via pic_unmask(). */
void pic_init(void) {
    uint8_t m1 = 0xFF, m2 = 0xFF;   /* start fully masked */

    outb(PIC1_CMD,  0x11); io_wait();   /* ICW1: init, ICW4 needed */
    outb(PIC2_CMD,  0x11); io_wait();
    outb(PIC1_DATA, PIC_IRQ_BASE);     io_wait();   /* ICW2: vector offset */
    outb(PIC2_DATA, PIC_IRQ_BASE + 8); io_wait();
    outb(PIC1_DATA, 0x04); io_wait();   /* ICW3: slave on IRQ2 */
    outb(PIC2_DATA, 0x02); io_wait();
    outb(PIC1_DATA, 0x01); io_wait();   /* ICW4: 8086 mode */
    outb(PIC2_DATA, 0x01); io_wait();

    outb(PIC1_DATA, m1);                /* masks */
    outb(PIC2_DATA, m2);

    KLOG_INFO("pic: remapped to vectors %u-%u, all lines masked",
              PIC_IRQ_BASE, PIC_IRQ_BASE + 15);
}

void pic_eoi(uint8_t irq) {
    if (irq >= 8)
        outb(PIC2_CMD, 0x20);
    outb(PIC1_CMD, 0x20);
}

void pic_unmask(uint8_t irq) {
    uint16_t port = irq < 8 ? PIC1_DATA : PIC2_DATA;
    uint8_t  bit  = irq < 8 ? irq : irq - 8;
    uint8_t  cur;
    __asm__ volatile ("inb %1, %0" : "=a"(cur) : "dN"(port));
    cur &= (uint8_t)~(1u << bit);
    outb(port, cur);
}
