#include "surface.h"
#include "mm/pmm.h"
#include "lib/string.h"

struct surface *surf_create(int32_t w, int32_t h) {
    if (w <= 0 || h <= 0) return NULL;
    size_t bmap_bytes = sizeof(struct surface);
    struct surface *s = PA_TO_VA(pmm_alloc_pages(1, PAGE_SIZE));
    if (!s) return NULL;
    (void)bmap_bytes;
    size_t bytes = (size_t)w * h * 4;
    size_t pages = (bytes + PAGE_SIZE - 1) / PAGE_SIZE;
    uint32_t *px = PA_TO_VA(pmm_alloc_pages(pages, PAGE_SIZE));
    if (!px) { pmm_free_page(VA_TO_PA(s)); return NULL; }
    s->w = w; s->h = h; s->pixels = px;
    memset(px, 0, pages * PAGE_SIZE);
    return s;
}

void surf_destroy(struct surface *s) {
    if (!s) return;
    size_t bytes = (size_t)s->w * s->h * 4;
    size_t pages = (bytes + PAGE_SIZE - 1) / PAGE_SIZE;
    for (size_t i = 0; i < pages; i++)
        pmm_free_page(VA_TO_PA(s->pixels) + i * PAGE_SIZE);
    pmm_free_page(VA_TO_PA(s));
}

bool rect_clip_to_surface(const struct surface *s, struct rect *r) {
    if (r->x < 0) { r->w += r->x; r->x = 0; }
    if (r->y < 0) { r->h += r->y; r->y = 0; }
    if (r->x + r->w > s->w) r->w = s->w - r->x;
    if (r->y + r->h > s->h) r->h = s->h - r->y;
    return r->w > 0 && r->h > 0;
}

void surf_fill_rect(struct surface *s, struct rect r, uint32_t color) {
    if (!rect_clip_to_surface(s, &r)) return;
    for (int32_t y = r.y; y < r.y + r.h; y++)
        for (int32_t x = r.x; x < r.x + r.w; x++)
            s->pixels[y * s->w + x] = color;
}

void surf_blit(struct surface *dst, int dx, int dy, const struct surface *src) {
    struct rect r = { dx, dy, src->w, src->h };
    if (!rect_clip_to_surface(dst, &r)) return;
    int sx = r.x - dx, sy = r.y - dy;
    for (int32_t y = 0; y < r.h; y++)
        memcpy(&dst->pixels[(r.y + y) * dst->w + r.x],
               &src->pixels[(sy + y) * src->w + sx],
               (size_t)r.w * 4);
}
