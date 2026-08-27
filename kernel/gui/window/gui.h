#ifndef GUI_WM_H
#define GUI_WM_H

#include <stdint.h>
#include <stdbool.h>
#include "core/surface.h"

#define GUI_MAX_WINDOWS 8
#define GUI_TITLE_H    20

struct gui_window {
    char title[24];
    int  x, y, w, h;          /* body area; title bar drawn above-only visual */
    struct surface *body;     /* window contents */
    bool used, focused, dragging;
    int  drag_offx, drag_offy;
    void (*on_paint)(struct gui_window *w);   /* redraw hook */
};

void gui_init(void);
int  gui_create_window(const char *title, int x, int y, int w, int h,
                       void (*on_paint)(struct gui_window *));
struct gui_window *gui_window_by_id(int id);

/* redraw a window's content now (vm->compositor picks it up next frame) */
void gui_mark_dirty(int id);

#endif
