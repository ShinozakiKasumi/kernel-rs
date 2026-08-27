#ifndef PIC_H
#define PIC_H

#include <stdint.h>

#define PIC_IRQ_BASE 0x20   /* IRQ0..7  -> vectors 32..39 */

void pic_init(void);               /* remap + set masks */
void pic_eoi(uint8_t irq);         /* send EOI for irq (0..15) */
void pic_unmask(uint8_t irq);

#endif
