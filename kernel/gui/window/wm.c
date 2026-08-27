/* Window manager + software compositor thread.
 *
 * Model: fixed window table, z = array index (higher = on top). The
 * compositor paints wallpaper -> windows (z order) -> cursor into a
 * backbuffer, then copies it to the framebuffer. All painting is full-screen
 * per frame (teaching kernel: correctness over dirty-region tracking).
 */
#include "gui.h"
#include "core/mouse.h"
#include "render/font.h"
#include "kprintf.h"
#include "fb.h"
#include "sched.h"
#include "mm/pmm.h"
#include "lib/string.h"

#define C_WALLPAPER  0xFF1A2B4A
#define C_TITLE_ACT  0xFF3A6EA5
#define C_TITLE_INA  0xFF555B66
#define C_TITLE_TXT  0xFFFFFFFF
#define C_BORDER     0xFF0B0F17
#define C_CURSOR_FG  0xFFFFFFFF
#define C_CURSOR_BG  0xFF000000

static struct gui_window wins[GUI_MAX_WINDOWS];
static int focused_idx = -1;
static struct surface backbuffer;

/* --- terminal log mirror (filled by kprintf hook) -------------------------- */

#define TERM_LINES 14
#define TERM_COLS  56
static char term_buf[TERM_LINES][TERM_COLS];
static int  term_head;                 /* most recent line index */
static bool term_ready;

void gui_term_put(const char *s) {
    if (!s) return;
    static int col;
    while (*s) {
        char c = *s++;
        if (c == '\r') continue;
        if (c == '\n') {
            term_head = (term_head + 1) % TERM_LINES;
            memset(term_buf[term_head], ' ', TERM_COLS);
            col = 0;
            continue;
        }
        if (col < TERM_COLS - 1)
            term_buf[term_head][col++] = c;
    }
    if (term_ready) gui_mark_dirty(0);
}

/* --- windows ---------------------------------------------------------------- */

int gui_create_window(const char *title, int x, int y, int w, int h,
                      void (*on_paint)(struct gui_window *)) {
    for (int i = 0; i < GUI_MAX_WINDOWS; i++) {
        if (wins[i].used) continue;
        struct gui_window *win = &wins[i];
        win->body = surf_create(w, h);
        if (!win->body) return -1;
        strncpy(win->title, title, sizeof(win->title) - 1);
        win->x = x; win->y = y; win->w = w; win->h = h;
        win->used = true;
        win->on_paint = on_paint;
        focused_idx = i;
        win->focused = true;
        KLOG_INFO("gui: window '%s' at %d,%d %dx%d", title, x, y, w, h);
        return i;
    }
    return -1;
}

struct gui_window *gui_window_by_id(int id) {
    return (id >= 0 && id < GUI_MAX_WINDOWS && wins[id].used)
           ? &wins[id] : NULL;
}

void gui_mark_dirty(int id) { (void)id; /* full repaint each frame */ }

/* --- input handling --------------------------------------------------------- */

static int hit_test(int mx, int my) {
    for (int i = GUI_MAX_WINDOWS - 1; i >= 0; i--) {
        if (!wins[i].used) continue;
        if (mx >= wins[i].x && mx < wins[i].x + wins[i].w &&
            my >= wins[i].y - GUI_TITLE_H && my < wins[i].y + wins[i].h)
            return i;
    }
    return -1;
}

static void wm_update_mouse(void) {
    static uint8_t prev_buttons;

    int hit = hit_test(mouse_x, mouse_y);
    uint8_t pressed = mouse_buttons & ~prev_buttons;

    if (pressed & 1) {               /* left press */
        if (hit >= 0) {
            if (focused_idx >= 0) wins[focused_idx].focused = false;
            focused_idx = hit;
            wins[hit].focused = true;
            /* raise to top: shift array? keep simple: title bar drag */
            struct gui_window *w = &wins[hit];
            if (mouse_y < w->y) {    /* title bar zone */
                w->dragging = true;
                w->drag_offx = mouse_x - w->x;
                w->drag_offy = mouse_y - w->y;
            }
        }
    }
    if (!(mouse_buttons & 1))
        for (int i = 0; i < GUI_MAX_WINDOWS; i++)
            wins[i].dragging = false;

    if (mouse_buttons & 1) {
        for (int i = 0; i < GUI_MAX_WINDOWS; i++) {
            struct gui_window *w = &wins[i];
            if (w->used && w->dragging) {
                w->x = mouse_x - w->drag_offx;
                w->y = mouse_y - w->drag_offy;
                if (w->x < -w->w + 40) w->x = -w->w + 40;
                if (w->x > (int)fb_width() - 40) w->x = fb_width() - 40;
                if (w->y < GUI_TITLE_H) w->y = GUI_TITLE_H;
                if (w->y > (int)fb_height() - 20) w->y = fb_height() - 20;
            }
        }
    }
    prev_buttons = mouse_buttons;
}

/* --- painting ---------------------------------------------------------------- */

static void draw_title_bar(struct gui_window *w) {
    /* draw title bar + border directly onto the backbuffer */
    uint32_t bar = w->focused ? C_TITLE_ACT : C_TITLE_INA;
    struct rect r = { w->x, w->y - GUI_TITLE_H, w->w, GUI_TITLE_H };
    surf_fill_rect(&backbuffer, r, bar);
    font_draw_string(&backbuffer, w->x + 6, w->y - GUI_TITLE_H + 2,
                     w->title, C_TITLE_TXT, bar);
    /* close button glyph */
    struct rect bx = { w->x + w->w - 22, w->y - GUI_TITLE_H + 4, 12, 12 };
    surf_fill_rect(&backbuffer, bx, w->focused ? 0xFFC0505D : 0xFF777777);
    /* border */
    struct rect top    = { w->x - 1, w->y - GUI_TITLE_H - 1, w->w + 2, 1 };
    struct rect bottom = { w->x - 1, w->y + w->h, w->w + 2, 1 };
    struct rect left   = { w->x - 1, w->y - GUI_TITLE_H - 1, 1, w->h + GUI_TITLE_H + 2 };
    struct rect right  = { w->x + w->w, w->y - GUI_TITLE_H - 1, 1, w->h + GUI_TITLE_H + 2 };
    surf_fill_rect(&backbuffer, top, C_BORDER);
    surf_fill_rect(&backbuffer, bottom, C_BORDER);
    surf_fill_rect(&backbuffer, left, C_BORDER);
    surf_fill_rect(&backbuffer, right, C_BORDER);
}

/* arrow cursor, 12x18, 1=fg 2=bg */
static const uint8_t cursor_bmp[18][12] = {
    {2,0,0,0,0,0,0,0,0,0,0,0},
    {2,2,0,0,0,0,0,0,0,0,0,0},
    {2,1,2,0,0,0,0,0,0,0,0,0},
    {2,1,1,2,0,0,0,0,0,0,0,0},
    {2,1,1,1,2,0,0,0,0,0,0,0},
    {2,1,1,1,1,2,0,0,0,0,0,0},
    {2,1,1,1,1,1,2,0,0,0,0,0},
    {2,1,1,1,1,1,1,2,0,0,0,0},
    {2,1,1,1,1,1,1,1,2,0,0,0},
    {2,1,1,1,1,1,1,1,1,2,0,0},
    {2,1,1,1,1,1,1,1,1,1,2,0},
    {2,1,1,1,1,1,1,2,2,2,2,2},
    {2,1,1,2,1,1,2,0,0,0,0,0},
    {2,1,2,0,2,1,1,2,0,0,0,0},
    {2,2,0,0,2,1,1,2,0,0,0,0},
    {2,0,0,0,0,2,1,1,2,0,0,0},
    {0,0,0,0,0,2,1,1,2,0,0,0},
    {0,0,0,0,0,0,2,2,0,0,0,0},
};

static void draw_cursor(void) {
    for (int row = 0; row < 18; row++)
        for (int col = 0; col < 12; col++) {
            uint8_t v = cursor_bmp[row][col];
            if (!v) continue;
            int px = mouse_x + col, py = mouse_y + row;
            if (px < 0 || py < 0 || px >= backbuffer.w || py >= backbuffer.h) continue;
            backbuffer.pixels[py * backbuffer.w + px] =
                v == 2 ? C_CURSOR_BG : C_CURSOR_FG;
        }
}

/* --- compositor thread ------------------------------------------------------- */

static void compositor_thread(void *arg) {
    (void)arg;
    extern volatile uint64_t timer_ticks;   /* idt.c */
    uint64_t last = timer_ticks;

    for (;;) {
        wm_update_mouse();

        /* wallpaper */
        struct rect full = { 0, 0, backbuffer.w, backbuffer.h };
        surf_fill_rect(&backbuffer, full, C_WALLPAPER);
        font_draw_string(&backbuffer, 12, 10,
                         "Shizuku GUI - drag title bars, type in serial shell",
                         0xFF9BAEDC, C_WALLPAPER);

        for (int i = 0; i < GUI_MAX_WINDOWS; i++) {
            struct gui_window *w = &wins[i];
            if (!w->used) continue;
            if (w->on_paint) w->on_paint(w);
            surf_blit(&backbuffer, w->x, w->y, w->body);
            draw_title_bar(w);
        }
        draw_cursor();

        /* flip to framebuffer */
        uint8_t *fbp = fb_pixels();
        uint32_t pitch = fb_pitch_bytes();
        for (int32_t y = 0; y < backbuffer.h; y++)
            memcpy(fbp + (size_t)y * pitch,
                   &backbuffer.pixels[y * backbuffer.w],
                   (size_t)backbuffer.w * 4);

        while (timer_ticks == last) __asm__ volatile ("sti; hlt");
        last = timer_ticks;   /* ~100fps cap; one frame per tick */
    }
}

static void paint_terminal(struct gui_window *w) {
    struct rect r = { 0, 0, w->w, w->h };
    surf_fill_rect(w->body, r, 0xFF101418);
    for (int i = 0; i < TERM_LINES; i++) {
        int idx = (term_head + 1 + i) % TERM_LINES;
        font_draw_string(w->body, 4, 4 + i * FONT_H,
                         term_buf[idx], 0xFFD0FFD0, 0xFF101418);
    }
}

static void paint_demo(struct gui_window *w) {
    extern volatile uint64_t timer_ticks;
    static int phase;
    if ((timer_ticks & 7) == 0) phase++;
    struct rect r = { 0, 0, w->w, w->h };
    surf_fill_rect(w->body, r, 0xFF202028);
    for (int i = 0; i < 8; i++) {
        uint32_t c = 0xFF000000 | ((i * 32 + phase) % 256) << 16 |
                     (255 - (i * 24 + phase) % 256) << 8;
        struct rect b = { 12 + i * (w->w - 40) / 8, 20,
                          (w->w - 40) / 8 - 4, w->h - 40 };
        surf_fill_rect(w->body, b, c);
    }
    font_draw_string(w->body, 12, w->h - 20, "animated demo",
                     0xFFFFFFFF, 0xFF202028);
}

void gui_init(void) {
    backbuffer.w = fb_width();
    backbuffer.h = fb_height();
    backbuffer.pixels = PA_TO_VA(pmm_alloc_pages(
        ((size_t)backbuffer.w * backbuffer.h * 4 + PAGE_SIZE - 1) / PAGE_SIZE,
        PAGE_SIZE));

    memset(term_buf, ' ', sizeof term_buf);
    for (int i = 0; i < TERM_LINES; i++)
        term_buf[i][TERM_COLS - 1] = 0;

    gui_create_window("terminal", 60, 90, TERM_COLS * FONT_W + 8,
                      TERM_LINES * FONT_H + 8, paint_terminal);
    gui_create_window("demo", 600, 200, 340, 200, paint_demo);
    term_ready = true;

    extern void (*gui_uart_sink)(const char *);
    gui_uart_sink = gui_term_put;

    kthread_create("compositor", compositor_thread, NULL);
    KLOG_INFO("gui: %dx%d, compositor running", backbuffer.w, backbuffer.h);
}
