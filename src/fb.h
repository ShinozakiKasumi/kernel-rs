/* Framebuffer driver: pixel access, clear, filled rectangles,
 * and a colour test pattern, on top of the Limine framebuffer.
 *
 * Handles any 32bpp-xRGB layout; 24bpp falls back to per-pixel byte writes.
 */
#ifndef FB_H
#define FB_H

#include <stdint.h>
#include <stddef.h>

/* 32-bit packed colour, layout resolved at init from the framebuffer info. */
typedef uint32_t fb_color_t;

fb_color_t fb_rgb(uint8_t r, uint8_t g, uint8_t b);

/* Initialise from Limine. Returns 0 on success, -1 if no framebuffer. */
int  fb_init(void);

int  fb_available(void);
uint32_t fb_width(void);
uint32_t fb_height(void);

void fb_put_pixel(uint32_t x, uint32_t y, fb_color_t c);
void fb_fill_rect(uint32_t x, uint32_t y, uint32_t w, uint32_t h, fb_color_t c);
void fb_clear(fb_color_t c);

uint8_t *fb_pixels(void);
uint32_t fb_pitch_bytes(void);

/* Colour bars + gradient test pattern covering the whole screen. */
void fb_test_pattern(void);

#endif
