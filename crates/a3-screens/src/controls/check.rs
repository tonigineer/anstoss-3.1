//! A box to tick with its label beside it, as the game's dialogs show one:
//! the box is part of the dialog's background, and the control draws only
//! its label and, when it is on, the tick — the box two pixels left of
//! where the dialog file puts the control, both a pixel up, as the
//! original's land.

use bevy_math::Rect;
use image::{Rgba, RgbaImage};

use a3_files::rch::Control;

use crate::classic::{Paint, TEXT, TEXT_HOVER, fill};
use crate::pointer::{Geometry, Target};

/// The box the background shows, from the control's top left, and where
/// the label starts.
const SIZE: i32 = 13;
const BOX_LEFT: i32 = -2;
const BOX_UP: i32 = -1;
const LABEL_LEFT: i32 = 13;
const DARK: Rgba<u8> = Rgba([0, 0, 0, 255]);
/// The tick, row by row from the top, three pixels in from the box's
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
        let x = r.min.x as i32 + BOX_LEFT;
        let y = r.min.y as i32 + (r.height() as i32 - SIZE) / 2 + BOX_UP;
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
        let top = r.min.y as i32 + (r.height() as i32 - font.height() as i32) / 2 + BOX_UP;
        font.draw(frame, r.min.x as i32 + LABEL_LEFT, top, &self.text, color, false);
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
