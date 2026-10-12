//! The start menu's artwork cleaned of the interface painted into it: the
//! shared picture the remastered look of most dialogs is painted from (see
//! [`super::backdrop`]).

use bevy::prelude::*;
use image::{Rgba, RgbaImage};

/// The patches of the start menu's background that hold its painted
/// interface, as classic rectangles: the orange rule, the version bars down
/// to their shadow, the heading's stem and bar, and the OK ball with its
/// stripes; the bullets come on top. Rows are filled across, the stem and
/// the ball's corner down; the order matters where patches meet.
const ROWS: [(u32, u32, u32, u32); 3] =
    [(0, 640, 19, 26), (196, 640, 26, 61), (336, 640, 130, 154)];
const STEM: (u32, u32, u32, u32) = (349, 366, 125, 240);
const CORNER: (u32, u32, u32, u32) = (583, 640, 443, 480);
/// Half the size of a painted bullet.
const BULLET: u32 = 8;

/// The start menu's background without the interface painted into it. Every
/// patch is filled by blending the untouched pixels on either side.
pub(super) fn clean_artwork(background: &RgbaImage, bullets: &[Vec2]) -> RgbaImage {
    let mut art = background.clone();
    let [rule, bars, heading] = ROWS;
    fill_rows(&mut art, rule.0..rule.1, rule.2, rule.3);
    fill_rows(&mut art, bars.0..bars.1, bars.2, bars.3);
    fill_columns(&mut art, STEM.0, STEM.1, STEM.2..STEM.3);
    fill_rows(&mut art, heading.0..heading.1, heading.2, heading.3);
    for centre in bullets {
        let (cx, cy) = (centre.x as u32, centre.y as u32);
        fill_rows(&mut art, cx - BULLET..cx + BULLET + 1, cy - BULLET, cy + BULLET - 1);
    }
    fill_columns(&mut art, CORNER.0, CORNER.1, CORNER.2..CORNER.3);
    art
}

/// Where the cleaned artwork was patched: those blends smear, so the wide
/// picture repaints them (classic rectangles).
pub(super) fn patches(bullets: &[Vec2]) -> Vec<Rect> {
    let rect = |(x0, x1, y0, y1): (u32, u32, u32, u32)| {
        Rect::new(x0 as f32, y0 as f32, x1 as f32, y1 as f32)
    };
    let half = BULLET as f32;
    ROWS.iter()
        .copied()
        .chain([STEM, CORNER])
        .map(rect)
        .chain(bullets.iter().map(|&c| Rect::from_center_half_size(c, Vec2::splat(half))))
        .collect()
}

/// Replaces the rows strictly between `top` and `bottom` in `columns` with a
/// blend from row `top` to row `bottom` (or repeats `top` past the edge).
fn fill_rows(img: &mut RgbaImage, columns: std::ops::Range<u32>, top: u32, bottom: u32) {
    let height = img.height();
    for x in columns {
        let a = *img.get_pixel(x, top);
        let b = if bottom < height { *img.get_pixel(x, bottom) } else { a };
        for y in top + 1..bottom.min(height) {
            let t = (y - top) as f32 / (bottom - top) as f32;
            img.put_pixel(x, y, lerp(a, b, t));
        }
    }
}

/// Replaces the columns strictly between `left` and `right` in `rows` with a
/// blend from column `left` to column `right` (or repeats `left` past the edge).
fn fill_columns(img: &mut RgbaImage, left: u32, right: u32, rows: std::ops::Range<u32>) {
    let width = img.width();
    for y in rows {
        let a = *img.get_pixel(left, y);
        let b = if right < width { *img.get_pixel(right, y) } else { a };
        for x in left + 1..right.min(width) {
            let t = (x - left) as f32 / (right - left) as f32;
            img.put_pixel(x, y, lerp(a, b, t));
        }
    }
}

fn lerp(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
    Rgba(std::array::from_fn(|i| {
        (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * t).round() as u8
    }))
}
