#ifndef SYSCALL_H
#define SYSCALL_H

#include "idt.h"

#define SYSCALL_VECTOR 0x80

#define SYS_write 0
#define SYS_exit  1

void syscall_init(void);   /* install the DPL-3 gate for int 0x80 */

#endif
