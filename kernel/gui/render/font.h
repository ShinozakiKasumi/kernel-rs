#ifndef GUI_FONT_H
#define GUI_FONT_H

#include <stdint.h>
#include "core/surface.h"

#define FONT_W 8
#define FONT_H 16

void font_draw_char(struct surface *s, int x, int y, char c,
                    uint32_t fg, uint32_t bg);
void font_draw_string(struct surface *s, int x, int y, const char *str,
                      uint32_t fg, uint32_t bg);

#endif
