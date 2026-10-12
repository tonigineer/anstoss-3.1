//! The game's `AniStatic`: a picture that can play a series of frames, like
//! the waving flags and the club president's face.

use bevy_math::{Rect, Vec2};
use image::{Rgba, RgbaImage};

use a3_files::rch::Control;

use crate::classic::Paint;
use crate::pointer::{Geometry, Target};

#[derive(Clone, Default, Debug)]
pub struct AnimState {
    /// The frames as graphic ids, and the one shown. It stays on that frame
    /// while it is not playing.
    pub frames: Vec<u16>,
    pub frame: usize,
    pub playing: bool,
    /// Animations only answer the pointer when the flow says so.
    pub clickable: bool,
}

impl AnimState {
    /// Whether it moves on by itself.
    pub fn moving(&self) -> bool {
        self.playing && self.frames.len() > 1
    }

    /// On to the next frame, round the end, if it is playing.
    pub fn advance(&mut self) {
        if self.playing && !self.frames.is_empty() {
            self.frame = (self.frame + 1) % self.frames.len();
        }
    }

    /// The frame it shows, the one after it, and how far it has blended
    /// into that one, given how far the frame time has run.
    pub fn showing(&self, phase: f32) -> (usize, usize, f32) {
        let count = self.frames.len().max(1);
        let blend = if self.playing && count > 1 { phase } else { 0.0 };
        (self.frame % count, (self.frame + 1) % count, blend)
    }

    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let (shown, next, blend) = self.showing(paint.phase);
        let picture = |i: usize| self.frames.get(i).and_then(|&id| paint.image(id));
        if let (Some(shown), Some(next)) = (picture(shown), picture(next)) {
            blend_frames(frame, shown, next, paint.control.rect.min, blend);
        }
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

/// A frame part-way into the next — which the game did not do: every pixel
/// mixes what either frame would show over the background.
fn blend_frames(frame: &mut RgbaImage, shown: &RgbaImage, next: &RgbaImage, at: Vec2, t: f32) {
    let (left, top) = (at.x as i64, at.y as i64);
    for y in 0..shown.height().max(next.height()) {
        for x in 0..shown.width().max(next.width()) {
            let (fx, fy) = (left + i64::from(x), top + i64::from(y));
            let (Ok(fx), Ok(fy)) = (u32::try_from(fx), u32::try_from(fy)) else { continue };
            let Some(&under) = frame.get_pixel_checked(fx, fy) else { continue };
            let over = |picture: &RgbaImage| {
                picture.get_pixel_checked(x, y).filter(|p| p[3] > 0).copied().unwrap_or(under)
            };
            let (a, b) = (over(shown), over(next));
            let mix = |i: usize| {
                (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * t).round() as u8
            };
            frame.put_pixel(fx, fy, Rgba([mix(0), mix(1), mix(2), 255]));
        }
    }
}
