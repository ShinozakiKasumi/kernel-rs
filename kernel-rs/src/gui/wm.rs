//! Window manager + software compositor thread.
//!
//! Each window owns an offscreen FRAME surface covering its full footprint
//! (1px border + title bar + body). Content is painted into the frame cache
//! once and reused every frame; the compositor only blits cached surfaces.
//!
//! Dragging never re-runs on_paint: the mouse handler moves the (x,y) origin
//! and damages the old and new footprints; the compositor repaints those
//! rectangles by blitting the cached frame.
//!
//! Model: fixed window table, z = array index (higher = on top). The
//! compositor is double-buffered with dirty-region tracking.

use super::dirty::{DirtyRegion, DIRTY_MAX_RECTS};
use super::font::{self, FONT_H, FONT_W};
use super::mouse;
use super::surface::{self, Rect, Surface};
use crate::fb;
use crate::vterm;

pub const GUI_MAX_WINDOWS: usize = 8;
pub const GUI_TITLE_H: i32 = 20;

const C_WALLPAPER: u32 = 0xFF1A2B4A;
const C_TITLE_ACT: u32 = 0xFF3A6EA5;
const C_TITLE_INA: u32 = 0xFF555B66;
const C_TITLE_TXT: u32 = 0xFFFFFFFF;
const C_BORDER: u32 = 0xFF0B0F17;
const C_CURSOR_FG: u32 = 0xFFFFFFFF;
const C_CURSOR_BG: u32 = 0xFF000000;

// frame cache offsets: border 1px, title bar GUI_TITLE_H, body below
const FRAME_BX: i32 = 1;
const FRAME_BY: i32 = GUI_TITLE_H + 1;

pub struct GuiWindow {
    pub title: [u8; 24],
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32, // body area; title bar drawn above-only visual
    pub body: Option<&'static mut Surface>, // offscreen content cache (w x h)
    pub frame: Option<&'static mut Surface>, // full footprint cache
    pub used: bool,
    pub focused: bool,
    pub content_dirty: bool,
    pub chrome_dirty: bool,
    pub animate: bool,
    pub vt: i32,             // bound vterm id, or -1
    pub on_paint: Option<fn(&mut GuiWindow)>,
}

impl GuiWindow {
    const fn empty() -> Self {
        GuiWindow {
            title: [0; 24],
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            body: None,
            frame: None,
            used: false,
            focused: false,
            content_dirty: false,
            chrome_dirty: false,
            animate: false,
            vt: -1,
            on_paint: None,
        }
    }

    pub fn title_str(&self) -> &str {
        let n = self
            .title
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.title.len());
        core::str::from_utf8(&self.title[..n]).unwrap_or("?")
    }
}

static mut WINS: [GuiWindow; GUI_MAX_WINDOWS] = [const { GuiWindow::empty() }; GUI_MAX_WINDOWS];
static mut FOCUSED_IDX: i32 = -1;
static mut BACKBUFFER: Surface = Surface {
    w: 0,
    h: 0,
    pixels: core::ptr::null_mut(),
};

static mut DIRTY: DirtyRegion = DirtyRegion::new();
static mut CURSOR_PREV_X: i32 = -1;
static mut CURSOR_PREV_Y: i32 = 0;
static mut STAT_FLIPPED_PX: u64 = 0;
static mut STAT_FRAMES: u64 = 0;
static mut STAT_PAINT_CALLS: u64 = 0;
static mut STAT_LOG_TICK: u64 = 0;

// --- virtual terminals (one per terminal window) ---
static mut VT_WIN: [i32; vterm::VT_MAX] = [-1; vterm::VT_MAX];
static mut WANT_NEW_TERM: bool = false; // set by the F1 handler (IRQ ctx)

fn on_vt_dirty(vt: i32) {
    unsafe {
        if vt >= 0 && (vt as usize) < vterm::VT_MAX && VT_WIN[vt as usize] >= 0 {
            mark_dirty(VT_WIN[vt as usize]);
        }
    }
}

fn on_fn_key(f: i32) {
    if f == 1 {
        unsafe {
            WANT_NEW_TERM = true;
        }
    }
}

/// klog/serial mirror into the boot console (vterm 0).
pub fn term_put(s: &str) {
    for &b in s.as_bytes() {
        vterm::putc_vt(0, b);
    }
}

/// Open a new terminal window bound to a fresh vterm running /bin/sh.
/// Called from the compositor loop (never IRQ context).
fn new_terminal() {
    let vt = vterm::create();
    if vt < 0 {
        crate::klog_warn!("gui: no free vterm slots");
        return;
    }
    let vt = vt as usize;
    let title = ["term0", "term1", "term2", "term3"][vt];

    let wid = create_window(
        title,
        100 + 40 * vt as i32,
        120 + 40 * vt as i32,
        vterm::VT_COLS as i32 * FONT_W + 8,
        vterm::VT_ROWS as i32 * FONT_H + 8,
        Some(paint_terminal),
    );
    if wid < 0 {
        crate::klog_warn!("gui: no free window slots");
        return;
    }
    unsafe {
        WINS[wid as usize].vt = vt as i32;
        VT_WIN[vt] = wid;
    }

    let argv = ["/bin/sh"];
    let tid = crate::proc::spawn_path("/bin/sh", &argv);
    if tid >= 0 {
        if let Some(t) = crate::sched::thread_at(tid as usize) {
            t.vt = vt as i32;
            t.set_cwd("/");
        }
    }
    unsafe {
        crate::kbd::ROUTE_VT = vt as i32; // new window takes the keyboard
    }
}

// --- frame cache ---

/// Full screen footprint of a window: body + title bar + 1px border.
fn window_rect_screen(w: &GuiWindow) -> Rect {
    Rect {
        x: w.x - 1,
        y: w.y - GUI_TITLE_H - 1,
        w: w.w + 2,
        h: w.h + GUI_TITLE_H + 2,
    }
}

/// Render cached chrome (border + title bar) into w.frame.
fn frame_render_chrome(w: &mut GuiWindow, fw: i32, fh: i32) {
    let bar = if w.focused { C_TITLE_ACT } else { C_TITLE_INA };
    let f = w.frame.as_deref_mut().unwrap();

    // border
    surface::fill_rect(
        f,
        Rect {
            x: 0,
            y: 0,
            w: fw,
            h: fh,
        },
        C_BORDER,
    );
    // title bar
    surface::fill_rect(
        f,
        Rect {
            x: FRAME_BX,
            y: FRAME_BX,
            w: w.w,
            h: GUI_TITLE_H,
        },
        bar,
    );
    font::draw_string(f, FRAME_BX + 6, FRAME_BX + 2, &w.title, C_TITLE_TXT, bar);
    // close button glyph
    surface::fill_rect(
        f,
        Rect {
            x: FRAME_BX + w.w - 22,
            y: FRAME_BX + 4,
            w: 12,
            h: 12,
        },
        if w.focused { 0xFFC0505D } else { 0xFF777777 },
    );
}

/// Rebuild the frame cache. Body content comes from on_paint (or a plain
/// fill when no hook is installed).
fn frame_render(w: &mut GuiWindow) {
    let fw = w.w + 2;
    let fh = w.h + GUI_TITLE_H + 2;
    w.content_dirty = false;
    w.chrome_dirty = false;
    frame_render_chrome(w, fw, fh);
    if let Some(hook) = w.on_paint {
        hook(w);
        unsafe {
            STAT_PAINT_CALLS += 1;
        }
    }
    let body = w.body.as_deref_mut().unwrap();
    let frame = w.frame.as_deref_mut().unwrap();
    surface::blit(frame, FRAME_BX, FRAME_BY, body);
}

// --- windows ---

/// Screen-space damage only: does NOT invalidate cached content.
fn damage_footprint(w: &GuiWindow) {
    unsafe {
        DIRTY.add(window_rect_screen(w));
    }
}

pub fn create_window(
    title: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    on_paint: Option<fn(&mut GuiWindow)>,
) -> i32 {
    unsafe {
        for i in 0..GUI_MAX_WINDOWS {
            if WINS[i].used {
                continue;
            }
            let body = Surface::create(w, h);
            let frame = Surface::create(w + 2, h + GUI_TITLE_H + 2);
            let (body, frame) = match (body, frame) {
                (Some(b), Some(f)) => (b, f),
                _ => {
                    crate::klog_warn!("gui: window surfaces alloc failed");
                    return -1; // leaks the other surface on partial failure
                }
            };
            let win = &mut WINS[i];
            win.body = Some(body);
            win.frame = Some(frame);
            let tn = title.len().min(23);
            win.title[..tn].copy_from_slice(&title.as_bytes()[..tn]);
            win.title[tn] = 0;
            win.x = x;
            win.y = y;
            win.w = w;
            win.h = h;
            win.used = true;
            win.vt = -1;
            win.on_paint = on_paint;
            if FOCUSED_IDX >= 0 {
                let f = FOCUSED_IDX as usize;
                WINS[f].focused = false;
                WINS[f].chrome_dirty = true;
                let fp = window_rect_screen(&WINS[f]);
                DIRTY.add(fp);
            }
            FOCUSED_IDX = i as i32;
            win.focused = true;
            frame_render(win);
            crate::klog_info!("gui: window '{}' at {},{} {}x{}", title, x, y, w, h);
            let fp = window_rect_screen(&WINS[i]);
            DIRTY.add(fp);
            return i as i32;
        }
    }
    -1
}

pub fn window_by_id(id: i32) -> Option<&'static mut GuiWindow> {
    unsafe {
        if id >= 0 && (id as usize) < GUI_MAX_WINDOWS && WINS[id as usize].used {
            Some(&mut WINS[id as usize])
        } else {
            None
        }
    }
}

/// Content changed: invalidate the frame cache and damage the footprint.
/// Call after changing widget state. Pure moves/focus changes must NOT use
/// this — they damage the footprint only, so the cache survives.
pub fn mark_dirty(id: i32) {
    if let Some(w) = window_by_id(id) {
        w.content_dirty = true;
        damage_footprint(w);
    }
}

// --- input handling ---------------------------------------------------------

fn hit_test(mx: i32, my: i32) -> i32 {
    unsafe {
        for i in (0..GUI_MAX_WINDOWS).rev() {
            if !WINS[i].used {
                continue;
            }
            if mx >= WINS[i].x
                && mx < WINS[i].x + WINS[i].w
                && my >= WINS[i].y - GUI_TITLE_H
                && my < WINS[i].y + WINS[i].h
            {
                return i as i32;
            }
        }
    }
    -1
}

fn wm_update_mouse() {
    unsafe {
        static mut PREV_BUTTONS: u8 = 0;
        static mut DRAG_WIN: i32 = -1;
        static mut DRAG_OFFX: i32 = 0;
        static mut DRAG_OFFY: i32 = 0;

        let mx = mouse::X;
        let my = mouse::Y;
        let buttons = mouse::BUTTONS;
        let hit = hit_test(mx, my);
        let pressed = buttons & !PREV_BUTTONS;

        if pressed & 1 != 0 {
            // left press
            if hit >= 0 {
                let h = hit as usize;
                if FOCUSED_IDX >= 0 && FOCUSED_IDX != hit {
                    let f = FOCUSED_IDX as usize;
                    WINS[f].focused = false;
                    WINS[f].chrome_dirty = true; // title colour
                    let fp = window_rect_screen(&WINS[f]);
                    DIRTY.add(fp);
                }
                FOCUSED_IDX = hit;
                if !WINS[h].focused {
                    WINS[h].chrome_dirty = true;
                    let fp = window_rect_screen(&WINS[h]);
                    DIRTY.add(fp);
                }
                WINS[h].focused = true;
                if WINS[h].vt >= 0 {
                    crate::kbd::ROUTE_VT = WINS[h].vt; // typing goes to this shell
                }
                if my < WINS[h].y {
                    // title bar zone
                    DRAG_WIN = hit;
                    DRAG_OFFX = mx - WINS[h].x;
                    DRAG_OFFY = my - WINS[h].y;
                }
            }
        }
        if buttons & 1 == 0 {
            DRAG_WIN = -1;
        }

        if buttons & 1 != 0 && DRAG_WIN >= 0 {
            let w = &mut WINS[DRAG_WIN as usize];
            let old_x = w.x;
            let old_y = w.y;
            w.x = mx - DRAG_OFFX;
            w.y = my - DRAG_OFFY;
            if w.x < -w.w + 40 {
                w.x = -w.w + 40;
            }
            if w.x > fb::width() as i32 - 40 {
                w.x = fb::width() as i32 - 40;
            }
            if w.y < GUI_TITLE_H {
                w.y = GUI_TITLE_H;
            }
            if w.y > fb::height() as i32 - 20 {
                w.y = fb::height() as i32 - 20;
            }
            if w.x != old_x || w.y != old_y {
                // Move: content unchanged. Damage vacated + newly occupied
                // footprints; the compositor repaints both from the cached
                // frame surface — no on_paint, no text redraw.
                DIRTY.add(Rect {
                    x: old_x - 1,
                    y: old_y - GUI_TITLE_H - 1,
                    w: w.w + 2,
                    h: w.h + GUI_TITLE_H + 2,
                });
                let fp = window_rect_screen(w);
                DIRTY.add(fp);
            }
        }
        PREV_BUTTONS = buttons;
    }
}

// --- painting ----------------------------------------------------------------

// arrow cursor, 12x18, 1=fg 2=bg
#[rustfmt::skip]
static CURSOR_BMP: [[u8; 12]; 18] = [
    [2,0,0,0,0,0,0,0,0,0,0,0],
    [2,2,0,0,0,0,0,0,0,0,0,0],
    [2,1,2,0,0,0,0,0,0,0,0,0],
    [2,1,1,2,0,0,0,0,0,0,0,0],
    [2,1,1,1,2,0,0,0,0,0,0,0],
    [2,1,1,1,1,2,0,0,0,0,0,0],
    [2,1,1,1,1,1,2,0,0,0,0,0],
    [2,1,1,1,1,1,1,2,0,0,0,0],
    [2,1,1,1,1,1,1,1,2,0,0,0],
    [2,1,1,1,1,1,1,1,1,2,0,0],
    [2,1,1,1,1,1,1,2,2,2,2,2],
    [2,1,1,2,1,1,2,0,0,0,0,0],
    [2,1,2,0,2,1,1,2,0,0,0,0],
    [2,2,0,0,2,1,1,2,0,0,0,0],
    [2,0,0,0,0,2,1,1,2,0,0,0],
    [0,0,0,0,0,0,2,2,0,0,0,0],
    [0,0,0,0,0,0,0,2,2,0,0,0],
    [0,0,0,0,0,0,0,0,0,0,0,0],
];

fn draw_cursor() {
    unsafe {
        let bb = &mut *core::ptr::addr_of_mut!(BACKBUFFER);
        for row in 0..18i32 {
            for col in 0..12i32 {
                let v = CURSOR_BMP[row as usize][col as usize];
                if v == 0 {
                    continue;
                }
                let px = mouse::X + col;
                let py = mouse::Y + row;
                if px < 0 || py < 0 || px >= bb.w || py >= bb.h {
                    continue;
                }
                *bb.pixels.add((py * bb.w + px) as usize) =
                    if v == 2 { C_CURSOR_BG } else { C_CURSOR_FG };
            }
        }
    }
}

// --- clipped painting helpers -----------------------------------------------

fn rect_intersects(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

/// Blit src onto dst at (dx,dy), writing only pixels inside `clip`.
/// (`clip` is in dst coordinates; dirty rects are already screen-clipped.)
fn blit_clipped(dst: &mut Surface, dx: i32, dy: i32, src: &Surface, clip: Rect) {
    let x0 = dx.max(clip.x);
    let y0 = dy.max(clip.y);
    let x1 = (dx + src.w).min(clip.x + clip.w);
    let y1 = (dy + src.h).min(clip.y + clip.h);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for y in y0..y1 {
        unsafe {
            core::ptr::copy_nonoverlapping(
                src.pixels.add(((y - dy) * src.w + (x0 - dx)) as usize),
                dst.pixels.add((y * dst.w + x0) as usize),
                (x1 - x0) as usize,
            );
        }
    }
}

// --- compositor thread -------------------------------------------------------

extern "C" fn compositor_thread(_arg: *mut core::ffi::c_void) {
    let mut last = crate::idt::timer_ticks();

    loop {
        unsafe {
            if WANT_NEW_TERM {
                // F1: spawn a terminal
                WANT_NEW_TERM = false;
                new_terminal();
            }
        }
        wm_update_mouse();

        // 1. collect damage
        unsafe {
            // moving the software cursor damages both its old and new footprint
            if mouse::X != CURSOR_PREV_X || mouse::Y != CURSOR_PREV_Y {
                if CURSOR_PREV_X >= 0 {
                    DIRTY.add_xywh(CURSOR_PREV_X, CURSOR_PREV_Y, 12, 18);
                }
                DIRTY.add_xywh(mouse::X, mouse::Y, 12, 18);
                CURSOR_PREV_X = mouse::X;
                CURSOR_PREV_Y = mouse::Y;
            }
            // animated windows repaint every frame, everything else is static
            for i in 0..GUI_MAX_WINDOWS {
                if WINS[i].used && WINS[i].animate {
                    mark_dirty(i as i32);
                }
            }

            // 1b. refresh stale frame caches once per frame, before clipping
            for i in 0..GUI_MAX_WINDOWS {
                if WINS[i].used && (WINS[i].content_dirty || WINS[i].chrome_dirty) {
                    frame_render(&mut *WINS.as_mut_ptr().add(i));
                }
            }
        }

        let mut dr = [Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        }; DIRTY_MAX_RECTS];
        let ndirty = unsafe { DIRTY.flush(&mut dr) };

        // 2. repaint: only damaged rects, wallpaper -> windows -> cursor
        if ndirty > 0 {
            const HDR: Rect = Rect {
                x: 12,
                y: 10,
                w: 72 * 8,
                h: 16,
            };
            unsafe {
                let bb = &mut *core::ptr::addr_of_mut!(BACKBUFFER);
                for &r in dr.iter().take(ndirty) {
                    surface::fill_rect(bb, r, C_WALLPAPER);
                    if rect_intersects(r, HDR) {
                        let hdr_line = b"Shizuku GUI - F1: new terminal | click to focus, type into it";
                        font::draw_string(bb, 12, 10, hdr_line, 0xFF9BAEDC, C_WALLPAPER);
                    }
                    for i in 0..GUI_MAX_WINDOWS {
                        let w = &mut WINS[i];
                        if !w.used {
                            continue;
                        }
                        if !rect_intersects(r, window_rect_screen(w)) {
                            continue;
                        }
                        blit_clipped(
                            bb,
                            w.x - FRAME_BX,
                            w.y - GUI_TITLE_H - FRAME_BX,
                            w.frame.as_deref().unwrap(),
                            r,
                        );
                    }
                }
            }
            draw_cursor();

            // 3. partial flip: copy only damaged spans
            let fbp = fb::pixels();
            let pitch = fb::pitch_bytes() as usize;
            unsafe {
                let bb = &*core::ptr::addr_of!(BACKBUFFER);
                for &r in dr.iter().take(ndirty) {
                    for y in r.y..r.y + r.h {
                        core::ptr::copy_nonoverlapping(
                            bb.pixels.add((y * bb.w + r.x) as usize) as *const u8,
                            fbp.add(y as usize * pitch + r.x as usize * 4),
                            r.w as usize * 4,
                        );
                    }
                    STAT_FLIPPED_PX += (r.w * r.h) as u64;
                }
            }
            unsafe {
                STAT_FRAMES += 1;

                // coarse throughput metric, ~1 line/sec at 100 Hz ticks
                let now = crate::idt::timer_ticks();
                if now - STAT_LOG_TICK >= 100 {
                    crate::klog_info!(
                        "gui: {} px flipped, {} content paints in {} damaged frames",
                        STAT_FLIPPED_PX,
                        STAT_PAINT_CALLS,
                        STAT_FRAMES
                    );
                    STAT_FLIPPED_PX = 0;
                    STAT_FRAMES = 0;
                    STAT_PAINT_CALLS = 0;
                    STAT_LOG_TICK = now;
                }
            }
        }

        while crate::idt::timer_ticks() == last {
            unsafe {
                core::arch::asm!("sti; hlt");
            }
        }
        last = crate::idt::timer_ticks(); // ~100fps cap; idle frames paint nothing
    }
}

// --- drag self-test ----------------------------------------------------------
// Simulates a title-bar drag through the real input path (mouse globals ->
// wm_update_mouse) and asserts the two performance invariants:
//   1. the dragged window's frame cache is never invalidated (no on_paint,
//      no text/button redraw — pure surface moves);
//   2. each move step damages only the old and new footprints.
// Runs once from gui_init(), before the compositor thread starts.
fn drag_selftest(id: i32, orig_x: i32, orig_y: i32) {
    unsafe {
        let w = &mut *WINS.as_mut_ptr().add(id as usize);
        let sx = orig_x + 20;
        let sy = orig_y - 10; // grab point on title bar
        let mut pass = true;

        mouse::X = sx;
        mouse::Y = sy;
        mouse::BUTTONS = 0;
        wm_update_mouse(); // settle prev_buttons
        mouse::BUTTONS = 1;
        wm_update_mouse(); // press: enters DRAGGING
        // the press itself may legitimately dirty chrome (focus change: title
        // bar colour); the drag loop below asserts moves never do.
        w.chrome_dirty = false;
        w.content_dirty = false;

        for step in 1..=8i32 {
            mouse::X = sx + step * 12; // fast diagonal drag
            mouse::Y = sy + step * 6;
            DIRTY.clear(); // count only this step
            wm_update_mouse();
            if w.content_dirty || w.chrome_dirty {
                crate::klog_err!("drag: selftest FAIL: cache invalidated mid-drag");
                pass = false;
            }
            if DIRTY.n < 1 || DIRTY.n > 4 {
                // old + new footprints
                crate::klog_err!("drag: selftest FAIL: {} rects for one move", DIRTY.n);
                pass = false;
            }
        }
        mouse::BUTTONS = 0;
        wm_update_mouse(); // release -> IDLE

        if w.x != orig_x + 96 || w.y != orig_y + 48 {
            // 8 steps of +12/+6
            crate::klog_err!("drag: selftest FAIL: ended at {},{}", w.x, w.y);
            pass = false;
        }
        // restore
        w.x = orig_x;
        w.y = orig_y;
        w.chrome_dirty = false;
        w.content_dirty = false;
        DIRTY.clear();
        DIRTY.add_all();
        if pass {
            crate::klog_info!("drag: selftest PASS (8 moves, 0 content repaints)");
        }
    }
}

fn paint_terminal(w: &mut GuiWindow) {
    let body = w.body.as_deref_mut().unwrap();
    surface::fill_rect(
        body,
        Rect {
            x: 0,
            y: 0,
            w: w.w,
            h: w.h,
        },
        0xFF101418,
    );
    for i in 0..vterm::VT_ROWS {
        let line: &[u8] = if w.vt >= 0 {
            vterm::row(w.vt as usize, i).unwrap_or(b"")
        } else {
            b""
        };
        font::draw_string(body, 4, 4 + i as i32 * FONT_H, line, 0xFFD0FFD0, 0xFF101418);
    }
}

fn paint_demo(w: &mut GuiWindow) {
    static mut PHASE: i32 = 0;
    unsafe {
        if crate::idt::timer_ticks() & 7 == 0 {
            PHASE += 1;
        }
        let phase = PHASE;
        let body = w.body.as_deref_mut().unwrap();
        surface::fill_rect(
            body,
            Rect {
                x: 0,
                y: 0,
                w: w.w,
                h: w.h,
            },
            0xFF202028,
        );
        for i in 0..8i32 {
            let c = 0xFF000000
                | ((((i * 32 + phase) % 256) as u32) << 16)
                | (((255 - (i * 24 + phase) % 256) as u32) << 8);
            surface::fill_rect(
                body,
                Rect {
                    x: 12 + i * (w.w - 40) / 8,
                    y: 20,
                    w: (w.w - 40) / 8 - 4,
                    h: w.h - 40,
                },
                c,
            );
        }
        font::draw_string(
            body,
            12,
            w.h - 20,
            b"animated demo",
            0xFFFFFFFF,
            0xFF202028,
        );
    }
}

// --- entry point --------------------------------------------------------------

pub fn init() {
    let w = fb::width() as i32;
    let h = fb::height() as i32;
    let npages = ((w as usize * h as usize * 4) + crate::mm::pmm::PAGE_SIZE as usize - 1)
        / crate::mm::pmm::PAGE_SIZE as usize;
    let pa = crate::mm::pmm::alloc_pages(npages, crate::mm::pmm::PAGE_SIZE as usize);
    unsafe {
        BACKBUFFER.w = w;
        BACKBUFFER.h = h;
        BACKBUFFER.pixels = crate::mm::pmm::pa_to_va(pa) as *mut u32;

        DIRTY.init(w, h);
        super::dirty::selftest();
        DIRTY.add_all(); // first frame paints everything

        for i in 0..vterm::VT_MAX {
            VT_WIN[i] = -1;
        }
    }
    vterm::set_dirty_hook(on_vt_dirty);
    unsafe {
        crate::kbd::FN_HOOK = Some(on_fn_key);
    }

    let vt0 = vterm::create(); // boot console
    let wid = create_window(
        "terminal",
        60,
        90,
        vterm::VT_COLS as i32 * FONT_W + 8,
        vterm::VT_ROWS as i32 * FONT_H + 8,
        Some(paint_terminal),
    );
    if wid >= 0 && vt0 >= 0 {
        unsafe {
            WINS[wid as usize].vt = vt0;
            VT_WIN[vt0 as usize] = wid;
            crate::kbd::ROUTE_VT = vt0; // typing goes to the boot shell
        }
    }
    let demo_id = create_window("demo", 600, 200, 340, 200, Some(paint_demo));
    if demo_id >= 0 {
        unsafe {
            WINS[demo_id as usize].animate = true;
        }
    }
    if wid >= 0 {
        drag_selftest(wid, 60, 90);
    }

    unsafe {
        crate::uart::GUI_UART_SINK = Some(term_put);
    }

    crate::sched::kthread_create("compositor", compositor_thread, core::ptr::null_mut());
    crate::klog_info!("gui: {}x{}, compositor running", w, h);
}
