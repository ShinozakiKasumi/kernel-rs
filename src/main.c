/* M1: serial logging via kprintf, exercised by a small self-test. */

#include "uart.h"
#include "kprintf.h"
#include "fb.h"
#include "gdt.h"
#include "idt.h"
#include "mm/pmm.h"
#include "mm/vmm.h"
#include "sched.h"
#include "proc.h"
#include "vfs.h"
#include "shell.h"
#include "core/mouse.h"
#include "window/gui.h"

extern int sched_enabled;   /* idt.c */
extern const uint8_t _binary_hello_elf_start[], _binary_hello_elf_end[];

/* --- Limine base revision (special: 3 u64s, no common-magic prefix) ------- */

__attribute__((used, section(".limine_requests")))
static volatile uint64_t limine_base_revision[3] =
    { 0xf9562b2d5c95a6c8ULL, 0x6a7b384944536bdcULL, 4 };

__attribute__((used, section(".limine_requests_start")))
static volatile uint64_t limine_requests_start_marker[4] =
    { 0xc7b1dd30df4c8b88ULL, 0x0a82e883a194f07bULL, 0, 0 };

__attribute__((used, section(".limine_requests_end")))
static volatile uint64_t limine_requests_end_marker[2] =
    { 0xadc0e0531bb10d03ULL, 0x9572709f3174c460ULL };

/* --- Entry ---------------------------------------------------------------- */

void kernel_main(void) {
    uart_init();
    KLOG_INFO("boot: limine ok, serial up");

    gdt_init();     /* M3: our own GDT (kernel cs/ds + reserved user slots) */
    idt_init();     /* exceptions + IRQ stubs, PIC remapped */

    pmm_init();     /* M4: physical page allocator from Limine memmap */
    pmm_selftest();
    vmm_init();     /* M5: own kernel CR3, HHDM kept, identity dropped */
    vmm_selftest();

    sched_init();   /* M6: round-robin threads, boot context = tid 0 */
    sched_enabled = 1;
    idt_enable();   /* sti */

    sched_selftest();   /* two ticker threads must interleave */

    vfs_init();
    vfs_create("/hello.elf", _binary_hello_elf_start,
               (uint64_t)_binary_hello_elf_end -
               (uint64_t)_binary_hello_elf_start);
    KLOG_INFO("vfs: seeded /hello.elf (%lu bytes)",
              (uint64_t)_binary_hello_elf_end -
              (uint64_t)_binary_hello_elf_start);

    kthread_create("shell", shell_main, NULL);

    /* M7-M9: run embedded user ELF through write/exit syscalls */
    proc_spawn_elf("hello", _binary_hello_elf_start,
                   (uint64_t)_binary_hello_elf_end -
                   (uint64_t)_binary_hello_elf_start);

    /* M3 self-test: breakpoint vector 3 must round-trip through isr.S */
    __asm__ volatile ("int $3");
    KLOG_INFO("int3 test: survived");

    if (fb_init() == 0) {
        mouse_init();
        gui_init();           /* windows + compositor + terminal mirror */
    } else {
        KLOG_WARN("fb: unavailable, GUI disabled");
    }

    KLOG_INFO("M3 done; timer/keyboard interrupts live — type to test kbd");

    for (;;)
        __asm__ volatile ("hlt");
}
