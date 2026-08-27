#include "kbd.h"
#include <stdint.h>
#include <stdbool.h>

#define KBD_BUF 128

static volatile uint8_t buf[KBD_BUF];
static volatile unsigned head, tail;   /* push at head, pop at tail */
static bool shift;

static const char unshifted[128] = {
    0,  27, '1','2','3','4','5','6','7','8','9','0','-','=','\b','\t',
    'q','w','e','r','t','y','u','i','o','p','[',']','\n',  0,  'a','s',
    'd','f','g','h','j','k','l',';','\'', '`',  0, '\\','z','x','c','v',
    'b','n','m',',','.','/',  0,  '*',  0,  ' ',  0,
    /* F1..F10 etc: ignored */
};

static const char shifted[128] = {
    0,  27, '!','@','#','$','%','^','&','*','(',')','_','+','\b','\t',
    'Q','W','E','R','T','Y','U','I','O','P','{','}','\n',  0,  'A','S',
    'D','F','G','H','J','K','L',':','"', '~',  0,  '|','Z','X','C','V',
    'B','N','M','<','>','?',  0,  '*',  0,  ' ',  0,
};

void kbd_on_scancode(uint8_t sc) {
    if (sc == 0x2A || sc == 0x36) { shift = true;  return; }
    if (sc == 0xAA || sc == 0xB6) { shift = false; return; }
    if (sc & 0x80) return;                         /* key release */
    if (sc >= 128) return;

    char c = shift ? shifted[sc] : unshifted[sc];
    if (!c) return;

    unsigned next = (head + 1) % KBD_BUF;
    if (next == tail) return;                      /* full: drop */
    buf[head] = (uint8_t)c;
    head = next;
}

int kbd_getchar(void) {
    if (head == tail) return -1;
    int c = buf[tail];
    tail = (tail + 1) % KBD_BUF;
    return c;
}
