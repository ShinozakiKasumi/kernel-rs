/* Round-robin kernel thread scheduler.
 *
 * Context switch model: every thread owns a 16KiB kernel stack; its preemption
 * point is a struct irq_frame saved inside struct thread. The timer handler
 * (running on the *preempted* thread's stack) copies the live frame out, picks
 * the next runnable thread, copies its frame in, and iretq returns into it.
 */
#include "sched.h"
#include "kprintf.h"
#include "mm/pmm.h"
#include "lib/string.h"

#define TIMESLICE_TICKS 5   /* 50ms at 100Hz */

static struct thread threads[SCHED_MAX_THREADS];
extern void kthread_trampoline(void);   /* isr.S */
static int    nr_threads;
static int    current_idx;
static int    last_slice;     /* tick counter of last switch */

void sched_init(void) {
    memset(threads, 0, sizeof threads);
    threads[0].tid   = 0;
    strcpy(threads[0].name, "kernel");
    threads[0].state = TH_RUNNABLE;      /* boot context */
    threads[0].space = kernel_space;
    nr_threads = 1;
    current_idx = 0;
    KLOG_INFO("sched: %d slots, boot context = tid 0", SCHED_MAX_THREADS);
}

int kthread_create(const char *name, thread_fn fn, void *arg) {
    int slot = -1;
    for (int i = 0; i < SCHED_MAX_THREADS; i++)
        if (threads[i].state == TH_UNUSED) { slot = i; break; }
    if (slot < 0) return -1;

    uintptr_t pa = pmm_alloc_pages(THREAD_KSTACK_SIZE / PAGE_SIZE, PAGE_SIZE);
    if (!pa) return -1;

    struct thread *t = &threads[slot];
    memset(t, 0, sizeof *t);
    t->tid  = slot;
    t->state = TH_RUNNABLE;
    t->space = kernel_space;
    t->kstack_top = (uint64_t)PA_TO_VA(pa) + THREAD_KSTACK_SIZE;
    t->kstack_pa  = pa;
    strncpy(t->name, name, sizeof(t->name) - 1);

    /* Fabricate the first trap frame at the top of the new stack. After the
     * first iretq, kthread_trampoline runs fn(arg) with rbx=fn, rdi=arg. */
    struct irq_frame *fr = &t->frame;
    fr->rip    = (uint64_t)kthread_trampoline;
    fr->cs     = 0x08;
    fr->ss     = 0x10;
    fr->rflags = 0x202;             /* IF | reserved bit */
    fr->rsp    = t->kstack_top;
    fr->rbx    = (uint64_t)fn;
    fr->rdi    = (uint64_t)arg;

    if (slot >= nr_threads) nr_threads = slot + 1;
    KLOG_INFO("sched: created '%s' (tid %d)", name, slot);
    return slot;
}

static int alloc_thread_slot(const char *name) {
    for (int i = 0; i < SCHED_MAX_THREADS; i++)
        if (threads[i].state == TH_UNUSED) {
            struct thread *t = &threads[i];
            memset(t, 0, sizeof *t);
            t->tid = i;
            t->state = TH_RUNNABLE;
            strncpy(t->name, name, sizeof(t->name) - 1);
            if (i >= nr_threads) nr_threads = i + 1;
            return i;
        }
    return -1;
}

/* Create a thread that starts in ring 3 at `entry` with user stack
 * `user_rsp`, address space `space`; the kernel stack (kstack) serves
 * interrupts/syscalls while the thread is in userland. Returns tid or -1. */
int thread_create_user(const char *name, uint64_t entry, uint64_t user_rsp,
                       page_table_t space) {
    int slot = alloc_thread_slot(name);
    if (slot < 0) return -1;

    uintptr_t pa = pmm_alloc_pages(THREAD_KSTACK_SIZE / PAGE_SIZE, PAGE_SIZE);
    if (!pa) return -1;

    struct thread *t = &threads[slot];
    t->space      = space;
    t->is_user    = true;
    t->kstack_top = (uint64_t)PA_TO_VA(pa) + THREAD_KSTACK_SIZE;
    t->kstack_pa  = pa;

    struct irq_frame *fr = &t->frame;
    fr->rip    = entry;
    fr->cs     = 0x20 | 3;          /* user code selector, RPL3 */
    fr->ss     = 0x18 | 3;          /* user data selector, RPL3 */
    fr->rsp    = user_rsp;
    fr->rflags = 0x202;

    KLOG_INFO("sched: user '%s' (tid %d) entry=%p", name, slot, (void *)entry);
    return slot;
}

static int pick_next(void) {
    for (int i = 1; i <= SCHED_MAX_THREADS; i++) {
        int idx = (current_idx + i) % SCHED_MAX_THREADS;
        if (threads[idx].state == TH_RUNNABLE)
            return idx;
    }
    return current_idx;
}

void sched_tick(struct irq_frame *f) {
    last_slice++;
    if (last_slice < TIMESLICE_TICKS) return;
    last_slice = 0;

    int next = pick_next();
    struct thread *cur = &threads[current_idx];
    if (next == current_idx) return;

    cur->frame = *f;                 /* save preempted context */
    current_idx = next;
    struct thread *nt = &threads[next];
    *f = nt->frame;                  /* iretq will land here */

    if (nt->space && nt->space != cur->space)
        vmm_switch(nt->space);

    /* TSS hook (M7) */
    extern void gdt_set_kernel_stack(uint64_t rsp0);
    if (nt->kstack_top)
        gdt_set_kernel_stack(nt->kstack_top);
}

void sched_yield(void) {
    struct irq_frame fake;           /* unused: force a full timeslice out */
    (void)fake;
    last_slice = TIMESLICE_TICKS;    /* make next tick switch immediately */
    __asm__ volatile ("int $32");    /* run through the timer path now */
}

_Noreturn void sched_thread_exit(void) {
    struct thread *t = &threads[current_idx];
    KLOG_INFO("sched: '%s' (tid %d) exited", t->name, t->tid);
    t->state = TH_ZOMBIE;
    sched_yield();                   /* never returns to the zombie frame */
    for (;;) __asm__ volatile ("cli; hlt");
}

struct thread *sched_current(void) { return &threads[current_idx]; }
int sched_count(void) { return nr_threads; }

void sched_list(void (*emit)(const char *, ...)) {
    emit("%3s  %-12s %s  %s\n", "tid", "name", "state", "space");
    for (int i = 0; i < nr_threads; i++)
        if (threads[i].state != TH_UNUSED)
            emit("%3d  %-12s %s  %p\n", threads[i].tid, threads[i].name,
                 threads[i].state == TH_ZOMBIE ? "zombie" : "runnable",
                 (void *)threads[i].space);
}

/* Scheduler self-test: two counters must interleave. */
static volatile int ta_runs, tb_runs;

static void ta(void *arg) {
    (void)arg;
    for (int i = 0; i < 3; i++) {
        ta_runs++;
        KLOG_INFO("[TA] tick %d", i);
        sched_yield();
    }
    KLOG_INFO("[TA] done");
}

static void tb(void *arg) {
    (void)arg;
    for (int i = 0; i < 3; i++) {
        tb_runs++;
        KLOG_INFO("[TB] tick %d", i);
        sched_yield();
    }
    KLOG_INFO("[TB] done");
}

void sched_selftest(void) {
    KLOG_INFO("sched: spawning two test threads...");
    kthread_create("test-a", ta, NULL);
    kthread_create("test-b", tb, NULL);
}
