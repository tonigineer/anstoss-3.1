//! A box to tick with its label beside it: the Windows check box of the
//! game's dialogs, drawn as Windows 95 draws one — a sunken white square,
//! a black tick in it when it is on.

use bevy_math::Rect;
use image::{Rgba, RgbaImage};

use a3_files::rch::Control;

use crate::classic::{Paint, TEXT, TEXT_HOVER, fill};
use crate::pointer::{Geometry, Target};

/// The square, and how far from it the label starts.
const SIZE: i32 = 13;
const LABEL_GAP: i32 = 4;
const SHADOW: Rgba<u8> = Rgba([128, 128, 128, 255]);
const LIGHT: Rgba<u8> = Rgba([255, 255, 255, 255]);
const DARK: Rgba<u8> = Rgba([0, 0, 0, 255]);
const FACE: Rgba<u8> = Rgba([192, 192, 192, 255]);
/// The tick, row by row from the top, three pixels in from the square's
/// corner.
const TICK: [&str; 7] =
    ["......x", ".....xx", "x...xxx", "xx.xxx.", "xxxxx..", ".xxx...", "..x...."];

#[derive(Clone, Default, Debug)]
pub struct CheckState {
    pub checked: bool,
    /// The label, placeholders filled in.
    pub text: String,
}

impl CheckState {
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let r = paint.control.rect;
        let (x, y) = (r.min.x as i32, r.min.y as i32 + (r.height() as i32 - SIZE) / 2);
        // Sunken: shadow and black at the top and left, white and the face
        // colour at the bottom and right, white inside.
        fill(frame, x, y, SIZE, SIZE, LIGHT);
        fill(frame, x, y, SIZE - 1, 1, SHADOW);
        fill(frame, x, y, 1, SIZE - 1, SHADOW);
        fill(frame, x + 1, y + 1, SIZE - 3, 1, DARK);
        fill(frame, x + 1, y + 1, 1, SIZE - 3, DARK);
        fill(frame, x + 1, y + SIZE - 2, SIZE - 2, 1, FACE);
        fill(frame, x + SIZE - 2, y + 1, 1, SIZE - 2, FACE);
        if self.checked {
            for (dy, row) in TICK.iter().enumerate() {
                for (dx, c) in row.chars().enumerate() {
                    if c == 'x' {
                        fill(frame, x + 3 + dx as i32, y + 3 + dy as i32, 1, 1, DARK);
                    }
                }
            }
        }
        let Some(font) = paint.font() else { return };
        let pointed = paint.pointed(Target::Check(paint.index));
        let color = if pointed { TEXT_HOVER } else { TEXT };
        let top = r.min.y as i32 + (r.height() as i32 - font.height() as i32) / 2;
        font.draw(frame, x + SIZE + LABEL_GAP, top, &self.text, color, false);
    }

    pub(crate) fn targets(
        &self,
        index: usize,
        control: &Control,
        geometry: &dyn Geometry,
        out: &mut Vec<(Target, Rect)>,
    ) {
        out.push((Target::Check(index), geometry.rect(control)));
    }
}
