//! Statics: a line of text, or a block that wraps where it has room for more
//! lines, which the flow may fill in and make clickable.

use bevy_math::Rect;
use image::{Rgba, RgbaImage};

use a3_files::bitmap_font::BitmapFont;
use a3_files::rch::{Align, Control};

use crate::classic::{Paint, TEXT_HOVER, centred};
use crate::pointer::{Geometry, Target};

#[derive(Clone, Default, Debug)]
pub struct StaticState {
    /// What it shows, placeholders filled in.
    pub text: String,
    /// Statics only answer the pointer when the flow says so.
    pub clickable: bool,
    /// One that answers to be dragged does not light up.
    pub quiet: bool,
}

impl StaticState {
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let Some(font) = paint.font_of(paint.control.font) else { return };
        let pointed = paint.pointed(Target::Control(paint.index)) && !self.quiet;
        let [r, g, b] = paint.control.ink;
        let color = if pointed { TEXT_HOVER } else { Rgba([r, g, b, 255]) };
        draw_static(font, frame, paint.control, &self.text, color);
    }

    pub(crate) fn targets(
        &self,
        index: usize,
        control: &Control,
        geometry: &dyn Geometry,
        out: &mut Vec<(Target, Rect)>,
    ) {
        if self.clickable {
            out.push((Target::Control(index), geometry.rect(control)));
        }
    }
}

/// Aligned within its rectangle, wrapped if it has room for more than one
/// line, each line aligned on its own (`DrawText` with `DT_WORDBREAK`, as
/// `CMyStatic` paints, `FUN_007ae930`).
fn draw_static(
    font: &BitmapFont,
    frame: &mut RgbaImage,
    control: &Control,
    text: &str,
    color: Rgba<u8>,
) {
    let r = control.rect;
    let (x, y) = (r.min.x as i32, r.min.y as i32);
    let lines = if r.height() >= 2.0 * font.height() as f32 {
        font.wrap(text, r.width() as u32, false)
    } else {
        vec![text.to_owned()]
    };
    // A static is a window of its own: nothing it draws shows below it.
    let (x0, x1) = (r.min.x.max(0.0) as u32, (r.max.x.max(0.0) as u32).min(frame.width()));
    let top = (r.max.y.max(0.0) as u32).min(frame.height());
    let bottom = top.saturating_add(2 * font.height()).min(frame.height());
    let below: Vec<(u32, u32, Rgba<u8>)> = (top..bottom)
        .flat_map(|py| (x0..x1).map(move |px| (px, py)))
        .map(|(px, py)| (px, py, *frame.get_pixel(px, py)))
        .collect();
    for (i, line) in lines.iter().enumerate() {
        let width = font.width(line, false) as i32;
        let left = match control.align {
            Align::Left => x,
            Align::Centre => centred(x, r.width() as i32, width),
            Align::Right => r.max.x as i32 - width,
        };
        font.draw(frame, left, y + i as i32 * font.height() as i32, line, color, false);
    }
    for (px, py, pixel) in below {
        frame.put_pixel(px, py, pixel);
    }
}
