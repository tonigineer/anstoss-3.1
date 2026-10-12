//! Sliders, the game's horizontal scrollbars: an arrow at each end, painted
//! into the background, and a handle that runs between them.

use bevy_math::Rect;
use image::RgbaImage;

use a3_files::rch::Control;

use super::spin::Value;
use crate::classic::{HANDLE, Paint, paste};
use crate::pointer::{Geometry, Target};

/// The arrows at either end are this wide; the handle runs between.
pub const SLIDER_ARROW: f32 = 20.0;
/// The handle, the game's `ScrollerBlau.bmp`, is this wide.
const SLIDER_HANDLE: f32 = 18.0;

/// Where the handle sits, in classic pixels.
pub fn handle(control: &Control, value: &Value) -> Rect {
    let r = control.rect;
    let (min, max) = value.range;
    let track = r.width() - 2.0 * SLIDER_ARROW - SLIDER_HANDLE;
    let share = if max > min { (value.value - min) as f32 / (max - min) as f32 } else { 0.0 };
    let left = r.min.x + SLIDER_ARROW + (track * share).round();
    Rect::new(left, r.min.y, left + SLIDER_HANDLE, r.max.y)
}

/// The value whose handle position is nearest to a handle at `left`.
pub fn value_at(control: &Control, value: &Value, left: f32) -> i32 {
    let (min, max) = value.range;
    let track = control.rect.width() - 2.0 * SLIDER_ARROW - SLIDER_HANDLE;
    let share = (left - control.rect.min.x - SLIDER_ARROW) / track.max(1.0);
    (min + (share * (max - min) as f32).round() as i32).clamp(min, max.max(min))
}

/// Where a press on the track, `along` its length from 0 to 1, puts the
/// left edge of the handle: centred under the pointer.
pub fn handle_under(control: &Control, along: f32) -> f32 {
    let r = control.rect;
    let x = r.min.x + SLIDER_ARROW + along * (r.width() - 2.0 * SLIDER_ARROW);
    x - SLIDER_HANDLE / 2.0
}

pub(crate) fn draw(value: &Value, frame: &mut RgbaImage, paint: &Paint) {
    if let Some(picture) = paint.widget(HANDLE) {
        paste(frame, picture, handle(paint.control, value).min);
    }
}

/// The arrows step it; the track between them pages it.
pub(crate) fn targets(
    index: usize,
    control: &Control,
    geometry: &dyn Geometry,
    out: &mut Vec<(Target, Rect)>,
) {
    let r = control.rect;
    let (y0, y1) = (r.min.y, r.max.y);
    let left = Rect::new(r.min.x, y0, r.min.x + SLIDER_ARROW, y1);
    let track = Rect::new(r.min.x + SLIDER_ARROW, y0, r.max.x - SLIDER_ARROW, y1);
    let right = Rect::new(r.max.x - SLIDER_ARROW, y0, r.max.x, y1);
    out.push((Target::SliderStep { control: index, up: false }, geometry.map(left)));
    out.push((Target::SliderTrack(index), geometry.map(track)));
    out.push((Target::SliderStep { control: index, up: true }, geometry.map(right)));
}
