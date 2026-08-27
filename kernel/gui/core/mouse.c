/* PS/2 mouse: IRQ12 via the slave PIC, 3-byte movement packets. */
#include "mouse.h"
#include "kprintf.h"
#include <stdbool.h>
#include "pic.h"

int32_t mouse_x, mouse_y;
uint8_t mouse_buttons;

static inline uint8_t inb(uint16_t p) {
    uint8_t v; __asm__ volatile ("inb %1, %0" : "=a"(v) : "dN"(p)); return v;
}
static inline void outb(uint16_t p, uint8_t v) {
    __asm__ volatile ("outb %0, %1" : : "a"(v), "dN"(p));
}

static bool io_timeout;
static int  io_timeout_at;

static void mouse_wait_write_at(int line) {
    (void)line;
    for (int t = 1000000; t-- > 0 && (inb(0x64) & 2); )
        ;
}
static void mouse_wait_read_at(int line) {
    bool got = false;
    for (int t = 1000000; t-- > 0; )
        if (inb(0x64) & 1) { got = true; break; }
    if (!got) { io_timeout = true; io_timeout_at = line; }
}
#define mouse_wait_write() mouse_wait_write_at(__LINE__)
#define mouse_wait_read()  mouse_wait_read_at(__LINE__)

static void mouse_write(uint8_t byte) {
    mouse_wait_write(); outb(0x64, 0xD4);
    mouse_wait_write(); outb(0x60, byte);
}

static uint8_t mouse_read(void) {
    mouse_wait_read();
    return inb(0x60);
}

void mouse_init(void) {
    /* drain any stale output-buffer bytes while polling-busy */
    __asm__ volatile ("cli");
    for (int t = 100000; t-- > 0 && (inb(0x64) & 1); )
        (void)inb(0x60);

    /* enable aux device through the 8042 controller */
    mouse_wait_write(); outb(0x64, 0xA8);
    mouse_wait_write(); outb(0x64, 0x20);          /* read command byte */
    mouse_wait_read();
    uint8_t status = inb(0x60) | 2;              /* enable IRQ12 */
    status &= ~0x20;                             /* clear mouse clock disable */
    mouse_wait_write(); outb(0x64, 0x60);
    mouse_wait_write(); outb(0x60, status);

    mouse_write(0xF6); mouse_read();             /* defaults */
    mouse_write(0xF4); mouse_read();             /* enable data reporting */

    mouse_x = 640; mouse_y = 400;
    pic_unmask(2);                               /* cascade */
    pic_unmask(12);                              /* mouse */
    if (io_timeout)
        KLOG_WARN("mouse: 8042 timeout at line %d", io_timeout_at);
    else
        KLOG_INFO("mouse: PS/2 aux device enabled (IRQ12)");
    __asm__ volatile ("sti");
}

static uint8_t cycle, packet[3];

void mouse_irq_handler(void) {
    uint8_t b = inb(0x60);

    uint8_t idx = cycle % 3;
    packet[idx] = b;
    if (idx == 0 && !(b & 0x08)) { cycle = 0; return; }  /* resync */
    if (++cycle < 3) return;
    cycle = 0;

    int dx = (int8_t)packet[1];
    int dy = (int8_t)packet[2];
    if (packet[0] & 0x40) dx = 0;                /* overflow: drop */
    if (packet[0] & 0x80) dy = 0;

    mouse_x += dx;
    mouse_y -= dy;
    if (mouse_x < 0) mouse_x = 0;
    if (mouse_y < 0) mouse_y = 0;
    extern uint32_t fb_width(void), fb_height(void);
    if (mouse_x >= (int)fb_width())  mouse_x = fb_width() - 1;
    if (mouse_y >= (int)fb_height()) mouse_y = fb_height() - 1;
    mouse_buttons = packet[0] & 0x07;
}
