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
/// Where a round box's dot is centred (494, shot k08).
const ROUND_DOT: Vec2 = Vec2::new(5.0, 7.0);
pub(crate) const TICK_DOWN: f32 = 2.0;
const LABEL_LEFT: i32 = 13;

#[derive(Clone, Default, Debug)]
pub struct CheckState {
    pub checked: bool,
    /// The label, placeholders filled in.
    pub text: String,
    /// A disabled box looks the same but takes no click (`WS_DISABLED`).
    pub disabled: bool,
}

impl CheckState {
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let r = paint.control.rect;
        let round = matches!(paint.control.kind, a3_files::rch::Kind::Check { round: true });
        if self.checked && round {
            super::options::dot(frame, r.min + ROUND_DOT);
        } else if self.checked
            && let Some(tick) = paint.widget(TICK)
        {
            paste(frame, tick, r.min + Vec2::new(0.0, TICK_DOWN));
        }
        // A box no wider than its tick has no room for its label.
        let Some(font) = paint.font().filter(|_| r.width() as i32 > LABEL_LEFT) else { return };
        let pointed = paint.pointed(Target::Check(paint.index));
        let color = if pointed { TEXT_HOVER } else { TEXT };
        let (x, y) = (r.min.x as i32 + LABEL_LEFT, r.min.y as i32);
        let width = (r.width() as i32 - LABEL_LEFT) as u32;
        font.draw_wrapped(frame, x, y, width, &self.text, color, false);
    }

    pub(crate) fn targets(
        &self,
        index: usize,
        control: &Control,
        geometry: &dyn Geometry,
        out: &mut Vec<(Target, Rect)>,
    ) {
        if !self.disabled {
            out.push((Target::Check(index), geometry.rect(control)));
        }
    }
}
