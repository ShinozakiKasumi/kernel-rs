/* Minimal userland runtime for Shizuku (int 0x80 ABI). */
#ifndef ULIB_H
#define ULIB_H

#include <stdint.h>

#define SYS_write 0
#define SYS_exit  1

static inline int64_t sys0(int64_t n) {
    int64_t ret;
    __asm__ volatile ("int $0x80" : "=a"(ret) : "a"(n) : "memory");
    return ret;
}

static inline int64_t sys3(int64_t n, int64_t a, int64_t b, int64_t c) {
    int64_t ret;
    __asm__ volatile ("int $0x80"
                      : "=a"(ret)
                      : "a"(n), "D"(a), "S"(b), "d"(c)
                      : "memory");
    return ret;
}

static inline int64_t sys_write(int fd, const void *buf, uint64_t len) {
    return sys3(SYS_write, fd, (int64_t)(uintptr_t)buf, (int64_t)len);
}

static inline void sys_exit(int code) {
    sys3(SYS_exit, code, 0, 0);
    for (;;) { }
}

static inline uint64_t ustrlen(const char *s) {
    uint64_t n = 0;
    while (s[n]) n++;
    return n;
}

static inline void puts(const char *s) {
    sys_write(1, s, ustrlen(s));
}

#endif
