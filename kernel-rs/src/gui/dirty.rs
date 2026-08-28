//! Dirty-region tracking for the GUI compositor.
//!
//! A dirty region is a small fixed-size list of non-overlapping rectangles,
//! clipped to a bounds rectangle (usually the screen). Widgets and the WM
//! mark damaged areas with [`add`]; the compositor drains the list with
//! [`flush`], repaints only those rectangles into the backbuffer, and copies
//! only those spans to the framebuffer.

use super::surface::Rect;

pub const DIRTY_MAX_RECTS: usize = 16;

#[derive(Clone, Copy)]
pub struct DirtyRegion {
    pub r: [Rect; DIRTY_MAX_RECTS],
    pub n: i32,       // live rects
    pub bw: i32,      // clip bounds (e.g. screen size)
    pub bh: i32,
}

impl DirtyRegion {
    pub const fn new() -> Self {
        DirtyRegion {
            r: [Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            }; DIRTY_MAX_RECTS],
            n: 0,
            bw: 0,
            bh: 0,
        }
    }

    pub fn init(&mut self, bounds_w: i32, bounds_h: i32) {
        self.n = 0;
        self.bw = bounds_w;
        self.bh = bounds_h;
    }

    pub fn clear(&mut self) {
        self.n = 0;
    }

    pub fn empty(&self) -> bool {
        self.n == 0
    }

    /// Mark an area damaged. Clipped to bounds; merged into the rect list.
    pub fn add(&mut self, mut r: Rect) {
        if !self.clip(&mut r) {
            return; // fully outside the screen
        }

        // Iteratively union with every rect that overlaps/touches it.
        let mut i = 0;
        while i < self.n as usize {
            if touching(self.r[i], r) {
                r = union(self.r[i], r);
                self.n -= 1;
                self.r[i] = self.r[self.n as usize]; // pop by swap-with-last
                i = 0; // merged rect may touch others now
            } else {
                i += 1;
            }
        }

        if (self.n as usize) < DIRTY_MAX_RECTS {
            self.r[self.n as usize] = r;
            self.n += 1;
        } else {
            // Overflow: collapse to the bounding box of all damage.
            for i in 0..self.n as usize {
                r = union(self.r[i], r);
            }
            self.r[0] = r;
            self.n = 1;
        }
    }

    pub fn add_xywh(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.add(Rect { x, y, w, h });
    }

    /// Mark everything damaged (e.g. right after init).
    pub fn add_all(&mut self) {
        self.n = 1;
        self.r[0] = Rect {
            x: 0,
            y: 0,
            w: self.bw,
            h: self.bh,
        };
    }

    /// Drain: copy up to `out.len()` rects to `out`, reset the region.
    pub fn flush(&mut self, out: &mut [Rect]) -> usize {
        let n = (self.n as usize).min(out.len());
        out[..n].copy_from_slice(&self.r[..n]);
        self.n = 0;
        n
    }

    fn clip(&self, r: &mut Rect) -> bool {
        let mut x1 = r.x + r.w;
        let mut y1 = r.y + r.h;
        if r.x < 0 {
            r.x = 0;
        }
        if r.y < 0 {
            r.y = 0;
        }
        if x1 > self.bw {
            x1 = self.bw;
        }
        if y1 > self.bh {
            y1 = self.bh;
        }
        r.w = x1 - r.x;
        r.h = y1 - r.y;
        r.w > 0 && r.h > 0
    }
}

fn union(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = (a.x + a.w).max(b.x + b.w);
    let y1 = (a.y + a.h).max(b.y + b.h);
    Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

/// Intersecting OR edge-touching rectangles merge into one.
fn touching(a: Rect, b: Rect) -> bool {
    a.x <= b.x + b.w && b.x <= a.x + a.w && a.y <= b.y + b.h && b.y <= a.y + a.h
}

/// Boot-time correctness check; logs PASS/FAIL over serial.
pub fn selftest() {
    let mut d = DirtyRegion::new();
    let mut fails = 0;
    let mut out = [Rect::default(); DIRTY_MAX_RECTS];

    // 1. two overlapping rects merge into their union
    d.init(100, 100);
    d.add_xywh(0, 0, 10, 10);
    d.add_xywh(5, 5, 10, 10);
    if d.n != 1 || d.r[0].x != 0 || d.r[0].y != 0 || d.r[0].w != 15 || d.r[0].h != 15 {
        fails += 1;
    }

    // 2. disjoint rects stay separate
    d.init(100, 100);
    d.add_xywh(0, 0, 10, 10);
    d.add_xywh(50, 50, 10, 10);
    if d.n != 2 {
        fails += 1;
    }

    // 3. clipping to bounds
    d.init(100, 100);
    d.add_xywh(90, 90, 50, 50);
    if d.n != 1 || d.r[0].w != 10 || d.r[0].h != 10 {
        fails += 1;
    }

    // 4. fully offscreen add is dropped
    d.init(100, 100);
    d.add_xywh(-50, -50, 10, 10);
    d.add_xywh(200, 200, 10, 10);
    if d.n != 0 {
        fails += 1;
    }

    // 5. transitive merge: C touches A and B, all collapse to one
    d.init(100, 100);
    d.add_xywh(0, 0, 10, 10);
    d.add_xywh(20, 20, 10, 10);
    d.add_xywh(5, 5, 20, 20); // bridges both
    if d.n != 1 || d.r[0].w != 30 || d.r[0].h != 30 {
        fails += 1;
    }

    // 6. touching edges merge
    d.init(100, 100);
    d.add_xywh(0, 0, 10, 10);
    d.add_xywh(10, 0, 10, 10);
    if d.n != 1 || d.r[0].w != 20 {
        fails += 1;
    }

    // 7. overflow falls back to one bounding box
    d.init(1000, 1000);
    for i in 0..(DIRTY_MAX_RECTS + 4) as i32 {
        d.add_xywh(i * 60, i * 60, 10, 10);
    }
    if d.n != 1 || d.r[0].x != 0 || d.r[0].y != 0 || d.r[0].w != 970 || d.r[0].h != 970 {
        fails += 1;
    }

    // 8. flush drains the list
    d.init(100, 100);
    d.add_xywh(0, 0, 10, 10);
    let got = d.flush(&mut out);
    if got != 1 || d.n != 0 || !d.empty() {
        fails += 1;
    }

    if fails > 0 {
        crate::klog_err!("gui: dirty selftest FAILED ({} cases)", fails);
    } else {
        crate::klog_info!("gui: dirty selftest PASS");
    }
}
