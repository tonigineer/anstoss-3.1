//! Spinners: the tiny up and down arrows next to a number. Most draw small
//! arrows of their own over the dialog's picture, the up arrow at the top,
//! the down arrow at the bottom (`CMyScrBar`'s style 0x40, `FUN_007ad236`),
//! brown from the fourth colour on. Their value, which sliders share.

use bevy_math::{Rect, Vec2};
use image::RgbaImage;

use a3_files::rch::{Control, Kind};

use crate::classic::{Paint, paste};
use crate::pointer::{Geometry, Target};

/// The small arrows, up and down, by colour.
pub(crate) const ARROWS: [&str; 4] =
    ["Pfeile/ScPfO1.bmp", "Pfeile/ScPfU1.bmp", "Pfeile/ScPfBraunO1.bmp", "Pfeile/ScPfBraunU1.bmp"];
const SMALL_ARROWS: u16 = 0x40;
const FIRST_BROWN: u16 = 3;

pub(crate) fn draw(frame: &mut RgbaImage, paint: &Paint) {
    let Kind::Spin { color, style } = paint.control.kind else { return };
    if style & SMALL_ARROWS == 0 {
        return;
    }
    let r = paint.control.rect;
    let brown = usize::from(color >= FIRST_BROWN) * 2;
    if let Some(up) = paint.widget(ARROWS[brown]) {
        paste(frame, up, r.min);
    }
    if let Some(down) = paint.widget(ARROWS[brown + 1]) {
        paste(frame, down, Vec2::new(r.min.x, r.max.y - down.height() as f32));
    }
}

/// A spinner's or slider's value, within its range.
#[derive(Clone, Copy, Default, Debug)]
pub struct Value {
    pub value: i32,
    pub range: (i32, i32),
    /// A slider's handle as long as the screen sets it (`FUN_007ae43e`),
    /// where it is not the usual twenty pixels.
    pub handle: Option<i32>,
}

impl Value {
    pub fn step(&mut self, up: bool) {
        self.set(self.value + if up { 1 } else { -1 });
    }

    pub fn set(&mut self, value: i32) {
        let (min, max) = self.range;
        self.value = value.clamp(min, max.max(min));
    }
}

/// The upper part steps up, the lower part down.
pub(crate) fn targets(
    index: usize,
    control: &Control,
    geometry: &dyn Geometry,
    out: &mut Vec<(Target, Rect)>,
) {
    let (up, down) = geometry.spin(index, control);
    out.push((Target::Spin { control: index, up: true }, up));
    out.push((Target::Spin { control: index, up: false }, down));
}
