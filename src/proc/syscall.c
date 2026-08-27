/* int 0x80 system calls.
 *
 * ABI: rax = syscall number, args = rdi, rsi, rdx, r10, r8, r9.
 * Return value in rax (written back into the trap frame).
 */
#include "syscall.h"
#include "kprintf.h"
#include "uart.h"
#include "sched.h"

typedef int64_t (*sys_fn)(struct irq_frame *f);

static int64_t sys_write(struct irq_frame *f) {
    int fd = (int)f->rdi;
    const char *buf = (const char *)f->rsi;
    uint64_t len = f->rdx;
    if (fd != 1 && fd != 2) return -1;
    if (len > 1 << 20) len = 1 << 20;      /* sanity cap */
    for (uint64_t i = 0; i < len; i++) {
        if (buf[i] == '\n') uart_putc('\r');
        uart_putc(buf[i]);
    }
    return (int64_t)len;
}

static int64_t sys_exit(struct irq_frame *f) {
    int code = (int)f->rdi;
    KLOG_INFO("proc: '%s' exited with code %d", sched_current()->name, code);
    sched_thread_exit();
}

static sys_fn syscall_table[] = {
    [SYS_write] = sys_write,
    [SYS_exit]  = sys_exit,
};
#define NR_SYSCALLS (sizeof syscall_table / sizeof syscall_table[0])

void syscall_dispatch(struct irq_frame *f) {
    uint64_t n = f->rax;
    if (n >= NR_SYSCALLS || !syscall_table[n]) {
        KLOG_WARN("syscall: unknown #%lu from '%s'", n, sched_current()->name);
        f->rax = (uint64_t)-1;
        return;
    }
    f->rax = (uint64_t)syscall_table[n](f);
}
