#include "font.h"

extern const uint8_t font8x16[256][16];

void font_draw_char(struct surface *s, int x, int y, char c,
                    uint32_t fg, uint32_t bg) {
    const uint8_t *g = font8x16[(uint8_t)c];
    for (int row = 0; row < FONT_H; row++)
        for (int col = 0; col < FONT_W; col++) {
            uint32_t color = (g[row] >> (7 - col)) & 1 ? fg : bg;
            int px = x + col, py = y + row;
            if (px >= 0 && px < s->w && py >= 0 && py < s->h)
                s->pixels[py * s->w + px] = color;
        }
}

void font_draw_string(struct surface *s, int x, int y, const char *str,
                      uint32_t fg, uint32_t bg) {
    while (*str && x + FONT_W <= s->w) {
        font_draw_char(s, x, y, *str++, fg, bg);
        x += FONT_W;
    }
}
