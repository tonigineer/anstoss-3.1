//! A strip the game paints into itself, its `CMyPaint`: bars of colour over
//! the dialog's background (`FUN_006a9cb1`) and lines (`FUN_006a9998`),
//! drawn anew each time the screen fills it.

use image::{Rgba, RgbaImage};

use crate::classic::{Paint, fill};

#[derive(Clone, Default, Debug)]
pub struct PaintState {
    /// From the control's top left: x, y, width and height, and the colour.
    pub bars: Vec<([i32; 4], Rgba<u8>)>,
    /// From the control's top left: from x and y to x and y, and the
    /// colour; drawn over the bars.
    pub lines: Vec<([i32; 4], Rgba<u8>)>,
}

impl PaintState {
    /// A bar in a Windows colour, `0x00bbggrr`; one without width or
    /// height paints nothing.
    pub fn bar(&mut self, x: i32, y: i32, w: i32, h: i32, colorref: u32) {
        if w > 0 && h > 0 {
            let [r, g, b, _] = colorref.to_le_bytes();
            self.bars.push(([x, y, w, h], Rgba([r, g, b, 255])));
        }
    }

    /// A line in a Windows colour, `0x00bbggrr`, from the pen's place to
    /// another (`FUN_006a9998`).
    pub fn line(&mut self, from: (i32, i32), to: (i32, i32), colorref: u32) {
        let [r, g, b, _] = colorref.to_le_bytes();
        self.lines.push(([from.0, from.1, to.0, to.1], Rgba([r, g, b, 255])));
    }

    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        // It paints into a surface of its own, and so not past its edges.
        let r = paint.control.rect;
        let (left, top) = (r.min.x as i32, r.min.y as i32);
        let (right, bottom) = (r.max.x as i32, r.max.y as i32);
        for &([x, y, w, h], color) in &self.bars {
            let (x0, y0) = ((left + x).max(left), (top + y).max(top));
            let (x1, y1) = ((left + x + w).min(right), (top + y + h).min(bottom));
            fill(frame, x0, y0, x1 - x0, y1 - y0, color);
        }
        for &([x0, y0, x1, y1], color) in &self.lines {
            for (x, y) in surface_line((x0, y0), (x1, y1)) {
                let (x, y) = (left + x, top + y);
                if (left..right).contains(&x) && (top..bottom).contains(&y) {
                    frame.put_pixel(x as u32, y as u32, color);
                }
            }
        }
    }
}

/// The pixels of a line as the game draws it into a strip's own surface
/// (`FUN_007c8617` with the ends as floats), unlike its lines on the
/// screen: both ends included, a pixel for every step along the longer
/// axis, the other coordinate the nearest pixel, a tie the lower one —
/// seen on the training statistics' curves (406), where the screen's line
/// (`classic::line`) left their ends out.
fn surface_line(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let steps = dx.abs().max(dy.abs());
    if steps == 0 {
        return vec![from];
    }
    // The nearest whole number to `start + i * d / steps`, ties down.
    let along = |start: i32, d: i32, i: i32| {
        let n = 2 * (start * steps + i * d) + steps - 1;
        n.div_euclid(2 * steps)
    };
    (0..=steps).map(|i| (along(from.0, dx, i), along(from.1, dy, i))).collect()
}
