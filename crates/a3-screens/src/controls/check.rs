//! A box to tick with its label beside it, as the game's dialogs show one
//! (`CMyCheckBox`, paint `FUN_00797c77`): the box is part of the dialog's
//! background, and the control draws only its tick, when it is on — the
//! picture `CheckHacken.tga`, two pixels down from its top left — and its
//! label from 13 pixels in, at the top, wrapped.

use bevy_math::{Rect, Vec2};
use image::RgbaImage;

use a3_files::rch::Control;

use crate::classic::{Paint, TEXT, TEXT_HOVER, paste};
use crate::pointer::{Geometry, Target};

/// The tick; `CheckEckig.tga` and `CheckRund.tga` go with styles no
/// dialog seen uses.
pub const TICK: &str = "CheckHacken.tga";
const TICK_DOWN: f32 = 2.0;
const LABEL_LEFT: i32 = 13;

#[derive(Clone, Default, Debug)]
pub struct CheckState {
    pub checked: bool,
    /// The label, placeholders filled in.
    pub text: String,
}

impl CheckState {
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let r = paint.control.rect;
        if self.checked
            && let Some(tick) = paint.widget(TICK)
        {
            paste(frame, tick, r.min + Vec2::new(0.0, TICK_DOWN));
        }
        let Some(font) = paint.font() else { return };
        let pointed = paint.pointed(Target::Check(paint.index));
        let color = if pointed { TEXT_HOVER } else { TEXT };
        let (x, y) = (r.min.x as i32 + LABEL_LEFT, r.min.y as i32);
        let width = (r.width() as i32 - LABEL_LEFT).max(1) as u32;
        font.draw_wrapped(frame, x, y, width, &self.text, color, false);
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
