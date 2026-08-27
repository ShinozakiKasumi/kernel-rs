#include "uart.h"

#define COM1 0x3f8

static inline void outb(uint16_t port, uint8_t val) {
    __asm__ volatile ("outb %0, %1" : : "a"(val), "dN"(port));
}

static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    __asm__ volatile ("inb %1, %0" : "=a"(ret) : "dN"(port));
    return ret;
}

void uart_init(void) {
    outb(COM1 + 1, 0x00);  /* Disable all interrupts */
    outb(COM1 + 3, 0x80);  /* Enable DLAB (set baud rate divisor) */
    outb(COM1 + 0, 0x03);  /* Divisor 3 -> 38400 baud */
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);  /* 8 bits, no parity, one stop bit (8N1) */
    outb(COM1 + 2, 0xC7);  /* Enable FIFO, clear both, 14-byte threshold */
    outb(COM1 + 4, 0x0B);  /* IRQs enabled, RTS/DSR set */
}

static int uart_tx_empty(void) {
    return inb(COM1 + 5) & 0x20;
}

void uart_putc(char c) {
    while (!uart_tx_empty()) { }
    outb(COM1, (uint8_t)c);
}

void (*gui_uart_sink)(const char *);   /* set by GUI (terminal window) */

void uart_write(const char *s) {
    while (*s) {
        if (*s == '\n')
            uart_putc('\r');
        uart_putc(*s++);
    }
}
