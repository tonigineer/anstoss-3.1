//! A strip the game paints into itself, its `CMyPaint`: bars of colour over
//! the dialog's background (`FUN_006a9cb1`), drawn anew each time the screen
//! fills it.

use image::{Rgba, RgbaImage};

use crate::classic::{Paint, fill};

#[derive(Clone, Default, Debug)]
pub struct PaintState {
    /// From the control's top left: x, y, width and height, and the colour.
    pub bars: Vec<([i32; 4], Rgba<u8>)>,
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
    }
}
