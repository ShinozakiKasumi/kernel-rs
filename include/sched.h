#ifndef SCHED_H
#define SCHED_H

#include <stdint.h>
#include <stdbool.h>
#include "idt.h"
#include "mm/vmm.h"

#define SCHED_MAX_THREADS 32
#define THREAD_KSTACK_SIZE (4 * PAGE_SIZE)   /* 16 KiB */

typedef void (*thread_fn)(void *arg);

enum thread_state { TH_UNUSED, TH_RUNNABLE, TH_ZOMBIE };

struct thread {
    int              tid;
    char             name[16];
    enum thread_state state;
    struct irq_frame frame;          /* saved trap frame (switch point) */
    page_table_t     space;          /* address space (NULL => kernel) */
    uint64_t         kstack_top;     /* initial top of kernel stack    */
    uint64_t         kstack_pa;      /* for the TSS/cleanup            */
    bool             is_user;
};

void  sched_init(void);

/* Create a kernel thread running fn(arg). Returns tid or -1. */
int   kthread_create(const char *name, thread_fn fn, void *arg);

/* Create a ring-3 thread (used by the ELF loader / proc layer). */
int   thread_create_user(const char *name, uint64_t entry, uint64_t user_rsp,
                         page_table_t space);

/* Called from the timer interrupt with the live trap frame. */
void  sched_tick(struct irq_frame *f);

_Noreturn void sched_thread_exit(void);
void  sched_yield(void);

struct thread *sched_current(void);
int   sched_count(void);
void  sched_list(void (*emit)(const char *fmt, ...));   /* for `ps` */
void  sched_selftest(void);

#endif
