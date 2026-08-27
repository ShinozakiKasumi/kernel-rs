#include "kprintf.h"
#include "uart.h"
#include "lib/string.h"

#include <stdarg.h>
#include <stdint.h>

/* Every formatted call is also mirrored (as bytes) to gui_uart_sink so the
 * GUI terminal window shows the kernel log without touching uart code. */
extern void (*gui_uart_sink)(const char *s);

static char cap_buf[192];
static int  cap_len;

static void emit_char(char c) {
    uart_putc(c);
    if (cap_len < (int)sizeof(cap_buf) - 1)
        cap_buf[cap_len++] = c;
}

static void emit_str(const char *s) {
    while (*s) emit_char(*s++);
}

static int print_uint_buf(char *out, uint64_t v, unsigned base, int upper) {
    const char *digits = upper ? "0123456789ABCDEF" : "0123456789abcdef";
    char tmp[32];
    int i = 0;

    if (!v) tmp[i++] = '0';
    while (v && i < (int)sizeof(tmp)) {
        tmp[i++] = digits[v % base];
        v /= base;
    }
    for (int j = 0; j < i; j++)
        out[j] = tmp[i - 1 - j];
    out[i] = 0;
    return i;
}

static void emit_padded(const char *s, int width) {
    int len = (int)strlen(s);
    for (int i = len; i < width; i++)
        emit_char(' ');
    emit_str(s);
}

static void print_int(int64_t v, int width) {
    char buf[40];
    if (v < 0) {
        buf[0] = '-';
        print_uint_buf(buf + 1, (uint64_t)(-(v + 1)) + 1, 10, 0);
    } else {
        print_uint_buf(buf, (uint64_t)v, 10, 0);
    }
    emit_padded(buf, width);
}

void kprintf(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    cap_len = 0;

    for (; *fmt; fmt++) {
        if (*fmt != '%') {
            if (*fmt == '\n')
                emit_char('\r');
            emit_char(*fmt);
            continue;
        }

        fmt++; /* consume '%' */
        while (*fmt == '#' || *fmt == '0' || *fmt == '-') fmt++;  /* flags */
        int width = 0;
        while (*fmt >= '0' && *fmt <= '9')
            width = width * 10 + (*fmt++ - '0');
        int lng = 0;
        if (*fmt == 'l') { lng = 1; fmt++; }

        char num[40];
        switch (*fmt) {
        case '%': emit_char('%'); break;
        case 'c': emit_char((char)va_arg(ap, int)); break;
        case 's': {
            const char *s = va_arg(ap, const char *);
            if (!s) s = "(null)";
            emit_padded(s, width);
            break;
        }
        case 'd': case 'i':
            print_int(lng ? va_arg(ap, long) : va_arg(ap, int), width);
            break;
        case 'u':
            print_uint_buf(num,
                lng ? va_arg(ap, unsigned long) : va_arg(ap, unsigned), 10, 0);
            emit_padded(num, width);
            break;
        case 'x':
            print_uint_buf(num,
                lng ? va_arg(ap, unsigned long) : va_arg(ap, unsigned), 16, 0);
            emit_padded(num, width);
            break;
        case 'X':
            print_uint_buf(num,
                lng ? va_arg(ap, unsigned long) : va_arg(ap, unsigned), 16, 1);
            emit_padded(num, width);
            break;
        case 'p': {
            uintptr_t p = (uintptr_t)va_arg(ap, void *);
            emit_str("0x");
            print_uint_buf(num, p, 16, 0);
            emit_str(num);
            break;
        }
        case '\0':
            goto out; /* trailing '%' at end of format */
        default:
            emit_char('%');
            emit_char(*fmt);
            break;
        }
    }
out:
    va_end(ap);
    cap_buf[cap_len] = 0;
    if (gui_uart_sink && cap_len > 0)
        gui_uart_sink(cap_buf);
}
