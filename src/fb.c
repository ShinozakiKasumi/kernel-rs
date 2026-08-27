#include "fb.h"
#include "kprintf.h"

/* --- Limine framebuffer request ------------------------------------------- */
/* Minimal inline copy of the request/response structs (limine.h, v8/9 API). */

struct limine_framebuffer {
    void *address;
    uint64_t width;
    uint64_t height;
    uint64_t pitch;          /* bytes per scanline */
    uint16_t bpp;
    uint8_t  memory_model;
    uint8_t  red_mask_size,  red_mask_shift;
    uint8_t  green_mask_size, green_mask_shift;
    uint8_t  blue_mask_size,  blue_mask_shift;
    uint8_t  reserved[7];
};

struct limine_framebuffer_response {
    uint64_t revision;
    uint64_t framebuffer_count;
    struct limine_framebuffer **framebuffers;
};

struct limine_framebuffer_request {
    uint64_t id[4];
    uint64_t revision;
    struct limine_framebuffer_response *response;
};

__attribute__((used, section(".limine_requests")))
static volatile struct limine_framebuffer_request fb_request = {
    .id = { 0xc7b1dd30df4c8b88ULL, 0x0a82e883a194f07bULL,
            0x9d5827dcd881dd75ULL, 0xa3148604f6fab11bULL },
};

/* --- State ---------------------------------------------------------------- */

static volatile struct limine_framebuffer *fb;
static uint64_t fb_pitch;
static uint32_t fb_w, fb_h;
static uint8_t  fb_bpp;
static uint8_t  r_shift, g_shift, b_shift;
static int      fb_ready;

/* --- API ------------------------------------------------------------------ */

int fb_init(void) {
    if (!fb_request.response || fb_request.response->framebuffer_count < 1) {
        KLOG_ERR("no framebuffer from bootloader");
        return -1;
    }
    struct limine_framebuffer *f = fb_request.response->framebuffers[0];
    fb       = (volatile struct limine_framebuffer *)f;
    fb_pitch = f->pitch;
    fb_w     = (uint32_t)f->width;
    fb_h     = (uint32_t)f->height;
    fb_bpp   = (uint8_t)f->bpp;
    r_shift  = f->red_mask_shift;
    g_shift  = f->green_mask_shift;
    b_shift  = f->blue_mask_shift;
    fb_ready = 1;

    KLOG_INFO("fb: %ux%u %ubpp pitch=%lu addr=%p",
              fb_w, fb_h, fb_bpp, fb_pitch, f->address);
    KLOG_INFO("fb: rgb shifts r=%u g=%u b=%u", r_shift, g_shift, b_shift);
    return 0;
}

int fb_available(void) { return fb_ready; }
uint32_t fb_width(void)  { return fb_w; }
uint32_t fb_height(void) { return fb_h; }
uint8_t *fb_pixels(void) { return fb_ready ? (uint8_t *)fb->address : 0; }
uint32_t fb_pitch_bytes(void) { return (uint32_t)fb_pitch; }

fb_color_t fb_rgb(uint8_t r, uint8_t g, uint8_t b) {
    return ((fb_color_t)r << r_shift)
         | ((fb_color_t)g << g_shift)
         | ((fb_color_t)b << b_shift);
}

void fb_put_pixel(uint32_t x, uint32_t y, fb_color_t c) {
    if (!fb_ready || x >= fb_w || y >= fb_h)
        return;
    uint8_t *row = (uint8_t *)fb->address + (uint64_t)y * fb_pitch;
    if (fb_bpp == 32) {
        ((uint32_t *)row)[x] = (uint32_t)c;
    } else if (fb_bpp == 24) {
        uint8_t *p = row + (uint64_t)x * 3;
        p[0] = (uint8_t)(c & 0xFF);
        p[1] = (uint8_t)((c >> 8) & 0xFF);
        p[2] = (uint8_t)((c >> 16) & 0xFF);
    }
    /* other depths: unsupported in M2 */
}

void fb_fill_rect(uint32_t x, uint32_t y, uint32_t w, uint32_t h, fb_color_t c) {
    if (!fb_ready) return;
    if (x >= fb_w || y >= fb_h) return;
    if (x + w > fb_w) w = fb_w - x;
    if (y + h > fb_h) h = fb_h - y;

    for (uint32_t j = 0; j < h; j++) {
        uint8_t *row = (uint8_t *)fb->address + (uint64_t)(y + j) * fb_pitch;
        if (fb_bpp == 32) {
            uint32_t *p = (uint32_t *)row + x;
            for (uint32_t i = 0; i < w; i++)
                p[i] = (uint32_t)c;
        } else {
            for (uint32_t i = 0; i < w; i++)
                fb_put_pixel(x + i, y + j, c);
        }
    }
}

void fb_clear(fb_color_t c) {
    fb_fill_rect(0, 0, fb_w, fb_h, c);
}

void fb_test_pattern(void) {
    if (!fb_ready) return;

    /* background */
    fb_clear(fb_rgb(16, 16, 24));

    /* 8 classic colour bars, top half */
    static const uint8_t bars[8][3] = {
        {255, 255, 255}, {255, 255, 0}, {0, 255, 255}, {0, 255, 0},
        {255, 0, 255},   {255, 0, 0},   {0, 0, 255},   {0, 0, 0},
    };
    uint32_t bar_w = fb_w / 8;
    uint32_t bar_h = fb_h / 2;
    for (uint32_t i = 0; i < 8; i++)
        fb_fill_rect(i * bar_w, 0, bar_w, bar_h,
                     fb_rgb(bars[i][0], bars[i][1], bars[i][2]));

    /* horizontal gradient, bottom half (green = f(x), blue = f(y)) */
    for (uint32_t y = bar_h; y < fb_h; y += 2)
        for (uint32_t x = 0; x < fb_w; x += 64)
            fb_fill_rect(x, y, 64, 2,
                         fb_rgb(0,
                                (uint8_t)(x * 255 / fb_w),
                                (uint8_t)((y - bar_h) * 255 / (fb_h - bar_h))));

    /* white crosshairs to verify coordinate math at extremes */
    fb_fill_rect(fb_w / 2 - 1, 0, 2, fb_h, fb_rgb(255, 255, 255));
    fb_fill_rect(0, fb_h / 2 - 1, fb_w, 2, fb_rgb(255, 255, 255));
}
