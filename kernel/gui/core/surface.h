#ifndef GUI_SURFACE_H
#define GUI_SURFACE_H

#include <stdint.h>
#include <stdbool.h>

struct rect { int32_t x, y, w, h; };

struct surface {
    int32_t   w, h;
    uint32_t *pixels;          /* row-major, xRGB8888 */
};

struct surface *surf_create(int32_t w, int32_t h);
void  surf_destroy(struct surface *s);
void  surf_fill_rect(struct surface *s, struct rect r, uint32_t color);
void  surf_blit(struct surface *dst, int dx, int dy, const struct surface *src);
bool  rect_clip_to_surface(const struct surface *s, struct rect *r);

#endif
