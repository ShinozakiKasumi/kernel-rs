//! 8x16 VGA font drawing onto surfaces.

use super::surface::Surface;
use crate::gui::font8x16_data::FONT8X16;

pub const FONT_W: i32 = 8;
pub const FONT_H: i32 = 16;

pub fn draw_char(s: &mut Surface, x: i32, y: i32, c: u8, fg: u32, bg: u32) {
    let g = &FONT8X16[c as usize];
    for row in 0..FONT_H {
        for col in 0..FONT_W {
            let color = if (g[row as usize] >> (7 - col)) & 1 != 0 {
                fg
            } else {
                bg
            };
            let px = x + col;
            let py = y + row;
            if px >= 0 && px < s.w && py >= 0 && py < s.h {
                unsafe {
                    *s.pixels.add((py * s.w + px) as usize) = color;
                }
            }
        }
    }
}

pub fn draw_string(s: &mut Surface, mut x: i32, y: i32, str_: &[u8], fg: u32, bg: u32) {
    for &c in str_ {
        if c == 0 || x + FONT_W > s.w {
            break;
        }
        draw_char(s, x, y, c, fg, bg);
        x += FONT_W;
    }
}
