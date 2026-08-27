#ifndef KBD_H
#define KBD_H

#include <stdint.h>

/* Feed an XT scancode (set 1) from the IRQ1 handler. */
void kbd_on_scancode(uint8_t scancode);

/* Poll next ASCII char: returns byte, or -1 when the buffer is empty. */
int  kbd_getchar(void);

#endif
