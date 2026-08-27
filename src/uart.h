/* UART 16550 driver: COM1 38400 8N1, polling mode.
 * Grown out of M0 inline code; interrupt-driven RX arrives with M3.
 */
#ifndef UART_H
#define UART_H

#include <stdint.h>

void uart_init(void);
void uart_putc(char c);
void uart_write(const char *s);

#endif
