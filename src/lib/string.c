#include "string.h"
#include <stdint.h>

void *memset(void *d, int c, size_t n) {
    uint8_t *p = d;
    while (n--) *p++ = (uint8_t)c;
    return d;
}

void *memcpy(void *d, const void *s, size_t n) {
    uint8_t *pd = d;
    const uint8_t *ps = s;
    while (n--) *pd++ = *ps++;
    return d;
}

void *memmove(void *d, const void *s, size_t n) {
    uint8_t *pd = d;
    const uint8_t *ps = s;
    if (pd < ps) while (n--) *pd++ = *ps++;
    else { pd += n; ps += n; while (n--) *--pd = *--ps; }
    return d;
}

int memcmp(const void *a, const void *b, size_t n) {
    const uint8_t *pa = a, *pb = b;
    while (n--) { if (*pa != *pb) return *pa - *pb; pa++; pb++; }
    return 0;
}

size_t strlen(const char *s) {
    size_t n = 0;
    while (s[n]) n++;
    return n;
}

int strcmp(const char *a, const char *b) {
    while (*a && *a == *b) { a++; b++; }
    return (unsigned char)*a - (unsigned char)*b;
}

int strncmp(const char *a, const char *b, size_t n) {
    while (n && *a && *a == *b) { a++; b++; n--; }
    return n ? (unsigned char)*a - (unsigned char)*b : 0;
}

char *strcpy(char *d, const char *s) {
    char *r = d;
    while ((*d++ = *s++));
    return r;
}

char *strncpy(char *d, const char *s, size_t n) {
    size_t i = 0;
    for (; i < n && s[i]; i++) d[i] = s[i];
    for (; i < n; i++) d[i] = 0;
    return d;
}
