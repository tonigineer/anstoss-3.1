//! Sliders, the game's horizontal scrollbars (`CMyScrBar`): an arrow at
//! each end and a handle that runs between them, twenty pixels of a long
//! bar in the slider's colour — its first and last three pixels, and the
//! middle between (`FUN_007ae2c4`).

use bevy_math::Rect;
use image::{RgbaImage, imageops};

use a3_files::rch::{Control, Kind};

use super::spin::Value;
use crate::classic::{Paint, paste};
use crate::pointer::{Geometry, Target};

/// The arrows at either end are this wide; the handle runs between.
pub const SLIDER_ARROW: f32 = 20.0;
const SLIDER_HANDLE: f32 = 20.0;
/// The long bars the handles are cut from, by style: blue, red, red, green,
/// yellow.
pub(crate) const HANDLES: [&str; 5] = [
    "ScrGrossBlauHori.bmp",
    "ScrGrossRotHori.bmp",
    "ScrGrossRotHori.bmp",
    "ScrGrossGruenHori.bmp",
    "ScrGrossGelbHori.bmp",
];
/// The arrows at its ends.
pub(crate) const ARROWS: [&str; 2] = ["Pfeile/PfeilL1.bmp", "Pfeile/PfeilR1.bmp"];
/// The ends a handle keeps of its bar, and where the bar's right end starts.
const END: u32 = 3;
const RIGHT_END: u32 = 0x1ba;

/// Where the handle sits, in classic pixels: as far along the track as the
/// value is in its range, in whole pixels rounded down, as the game puts it.
pub fn handle(control: &Control, value: &Value) -> Rect {
    let r = control.rect;
    let (min, max) = value.range;
    let track = (r.width() - 2.0 * SLIDER_ARROW - SLIDER_HANDLE) as i64;
    let along = if max > min {
        (i64::from(value.value - min) * track).div_euclid(i64::from(max - min))
    } else {
        0
    };
    let left = r.min.x + SLIDER_ARROW + along as f32;
    Rect::new(left, r.min.y, left + SLIDER_HANDLE, r.max.y)
}

/// The value of a handle at `left`, rounded down as the game reckons it.
pub fn value_at(control: &Control, value: &Value, left: f32) -> i32 {
    let (min, max) = value.range;
    let track = ((control.rect.width() - 2.0 * SLIDER_ARROW - SLIDER_HANDLE) as i64).max(1);
    let along = (left - control.rect.min.x - SLIDER_ARROW) as i64;
    let value = i64::from(min) + (i64::from(max - min) * along).div_euclid(track);
    (value as i32).clamp(min, max.max(min))
}

/// Where a press on the track, `along` its length from 0 to 1, puts the
/// left edge of the handle: centred under the pointer.
pub fn handle_under(control: &Control, along: f32) -> f32 {
    let r = control.rect;
    let x = r.min.x + SLIDER_ARROW + along * (r.width() - 2.0 * SLIDER_ARROW);
    x - SLIDER_HANDLE / 2.0
}

pub(crate) fn draw(value: &Value, frame: &mut RgbaImage, paint: &Paint) {
    let Kind::Slider { style } = paint.control.kind else { return };
    let r = paint.control.rect;
    let ends = [r.min.x, r.max.x - SLIDER_ARROW];
    for (name, x) in ARROWS.into_iter().zip(ends) {
        if let Some(arrow) = paint.widget(name) {
            paste(frame, arrow, bevy_math::Vec2::new(x, r.min.y));
        }
    }
    let name = HANDLES[usize::from(style).min(HANDLES.len() - 1)];
    if let Some(bar) = paint.widget(name) {
        paste(frame, &cut(bar), handle(paint.control, value).min);
    }
}

/// A handle cut from its long bar.
fn cut(bar: &RgbaImage) -> RgbaImage {
    let (width, height) = (SLIDER_HANDLE as u32, bar.height());
    let mut handle = RgbaImage::new(width, height);
    let part = |x: u32, w: u32| imageops::crop_imm(bar, x, 0, w, height).to_image();
    imageops::replace(&mut handle, &part(0, width - END), 0, 0);
    imageops::replace(&mut handle, &part(RIGHT_END, END), i64::from(width - END), 0);
    handle
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
