//! Statics: a line of text, or a block that wraps where it has room for more
//! lines, which the flow may fill in and make clickable.

use bevy_math::Rect;
use image::{Rgba, RgbaImage};

use a3_files::bitmap_font::BitmapFont;
use a3_files::rch::{Align, Control};

use crate::classic::{Paint, TEXT_HOVER};
use crate::pointer::{Geometry, Target};

#[derive(Clone, Default, Debug)]
pub struct StaticState {
    /// What it shows, placeholders filled in.
    pub text: String,
    /// Statics only answer the pointer when the flow says so.
    pub clickable: bool,
}

impl StaticState {
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let Some(font) = paint.font() else { return };
        let pointed = paint.pointed(Target::Control(paint.index));
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
/// line.
fn draw_static(
    font: &BitmapFont,
    frame: &mut RgbaImage,
    control: &Control,
    text: &str,
    color: Rgba<u8>,
) {
    let r = control.rect;
    let (x, y) = (r.min.x as i32, r.min.y as i32);
    if r.height() >= 2.0 * font.height() as f32 {
        font.draw_wrapped(frame, x, y, r.width() as u32, text, color, control.bold);
        return;
    }
    let width = font.width(text, control.bold) as i32;
    let left = match control.align {
        Align::Left => x,
        Align::Centre => x + (r.width() as i32 - width) / 2,
        Align::Right => r.max.x as i32 - width,
    };
    font.draw(frame, left, y, text, color, control.bold);
}
