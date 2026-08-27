/* Interactive shell over the serial console (COM1 also carries input when
 * QEMU runs with -serial stdio? No: serial is write-only here; keyboard
 * input arrives via PS/2 IRQ1 -> kbd ring buffer). */
#include "shell.h"
#include "kbd.h"
#include "kprintf.h"
#include "uart.h"
#include "sched.h"
#include "proc.h"
#include "vfs.h"
#include "mm/pmm.h"
#include "lib/string.h"

#define LINE_MAX 128
#define MAX_ARGS 8

static void prompt(void) {
    kprintf("shizuku> ");
}

static int readline(char *line, size_t cap) {
    size_t n = 0;
    for (;;) {
        int c = kbd_getchar();
        if (c < 0) {
            __asm__ volatile ("sti; hlt");   /* idle until any IRQ */
            continue;
        }
        if (c == '\n') {
            uart_write("\n");
            line[n] = 0;
            return (int)n;
        }
        if (c == '\b') {
            if (n) { n--; uart_write("\b \b"); }
            continue;
        }
        if (n + 1 < cap) {
            line[n++] = (char)c;
            uart_putc((char)c);              /* local echo */
        }
    }
}

static int tokenize(char *line, char *argv[]) {
    int argc = 0;
    char *p = line;
    while (*p && argc < MAX_ARGS) {
        while (*p == ' ') p++;
        if (!*p) break;
        argv[argc++] = p;
        while (*p && *p != ' ') p++;
        if (*p) *p++ = 0;
    }
    return argc;
}

/* --- built-in commands ------------------------------------------------------ */

static void cmd_help(void) {
    kprintf("commands: help clear ps mem ls cat run\n");
    kprintf("  clear          clear the screen (ANSI)\n");
    kprintf("  ps             list threads/processes\n");
    kprintf("  mem            physical memory stats\n");
    kprintf("  ls             list files in /\n");
    kprintf("  cat <file>     print file contents\n");
    kprintf("  run <file>     load ELF from tmpfs and run as user process\n");
}

static void cmd_ps(void)   { sched_list(kprintf); }

static void cmd_mem(void) {
    size_t free = pmm_free_count(), total = pmm_total_count();
    kprintf("memory: %lu/%lu pages free (%lu MiB used, %lu MiB total)\n",
            free, total, (total - free) * 4 / 1024, total * 4 / 1024);
}

static void cmd_ls(void) {
    struct dirent de;
    for (unsigned i = 0; vfs_list(i, &de); i++)
        kprintf("%8lu  %s\n", de.size, de.name);
}

static void cmd_cat(const char *path) {
    int64_t size = vfs_size(path);
    if (size < 0) { kprintf("cat: %s: not found\n", path); return; }
    char *buf = (char *)PA_TO_VA(pmm_alloc_pages((size + PAGE_SIZE - 1) / PAGE_SIZE, PAGE_SIZE));
    int64_t got = vfs_read(path, 0, buf, (uint64_t)size);
    for (int64_t i = 0; i < got; i++) uart_putc(buf[i]);
    uart_putc('\n');
}

static void cmd_run(const char *path) {
    int64_t size = vfs_size(path);
    if (size < 0) { kprintf("run: %s: not found\n", path); return; }
    size_t npages = ((uint64_t)size + PAGE_SIZE - 1) / PAGE_SIZE;
    uint8_t *buf = PA_TO_VA(pmm_alloc_pages(npages, PAGE_SIZE));
    if (vfs_read(path, 0, buf, (uint64_t)size) != size) {
        kprintf("run: %s: read error\n", path); return;
    }
    int tid = proc_spawn_elf(path, buf, (uint64_t)size);
    if (tid < 0)
        kprintf("run: %s: spawn failed (bad ELF?)\n", path);
    else
        kprintf("run: started '%s' as tid %d\n", path, tid);
}

/* --- dispatch --------------------------------------------------------------- */

static void dispatch(int argc, char *argv[]) {
    const char *cmd = argv[0];
    if (!strcmp(cmd, "help")) cmd_help();
    else if (!strcmp(cmd, "clear")) { uart_write("\033[2J\033[H"); }
    else if (!strcmp(cmd, "ps"))   cmd_ps();
    else if (!strcmp(cmd, "mem"))  cmd_mem();
    else if (!strcmp(cmd, "ls"))   cmd_ls();
    else if (!strcmp(cmd, "cat") && argc == 2) cmd_cat(argv[1]);
    else if (!strcmp(cmd, "run") && argc == 2) cmd_run(argv[1]);
    else kprintf("unknown command '%s' -- try 'help'\n", cmd);
}

void shell_main(void *arg) {
    (void)arg;
    static char line[LINE_MAX];
    static char *argv[MAX_ARGS];

    kprintf("\n=== shizuku shell (M12) ===\n");
    kprintf("type 'help' for commands\n");
    for (;;) {
        prompt();
        int n = readline(line, sizeof line);
        if (n == 0) continue;
        int argc = tokenize(line, argv);
        if (argc) dispatch(argc, argv);
    }
}
