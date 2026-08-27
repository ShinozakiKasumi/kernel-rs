/* kprintf: freestanding formatted output onto the serial console.
 *
 * Supported: %% %c %s %d %i %u %x %X %p ; %l variants (ld lu lx).
 * No float, no width/precision (teaching kernel: keep it small).
 */
#ifndef KPRINTF_H
#define KPRINTF_H

#include <stdint.h>

#define KLOG_INFO(fmt, ...)  kprintf("[kernel] " fmt "\n", ##__VA_ARGS__)
#define KLOG_WARN(fmt, ...)  kprintf("[warn]   " fmt "\n", ##__VA_ARGS__)
#define KLOG_ERR(fmt, ...)   kprintf("[error]  " fmt "\n", ##__VA_ARGS__)

void kprintf(const char *fmt, ...);

#endif
