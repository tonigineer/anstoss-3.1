//! Faders, the game's upright sliders (`CMySchieber`): a small picture, the
//! handle, that stands higher or lower in the control by its value. A click
//! above the handle takes one off, below it adds one (`OnLButtonDown_007ab7e0`,
//! page up and down).

use bevy_math::{Rect, Vec2};
use image::RgbaImage;

use a3_files::rch::{Control, Kind};

use super::spin::Value;
use crate::classic::{Paint, paste};
use crate::pointer::{Geometry, Target};

/// How high the handle is: the game takes it from its picture, and the
/// only faders' (2518, `scroller/schieberGruen.bmp`) is this high.
const HANDLE_HEIGHT: f32 = 6.0;

/// The handle's picture.
fn handle(control: &Control) -> Option<u16> {
    match control.kind {
        Kind::Fader { handle } => Some(handle),
        _ => None,
    }
}

/// Where the handle's top is, from the control's: as far down as the
/// value is in its range (`FUN_007abbb9`), in whole pixels rounded down.
fn handle_top(control: &Control, value: &Value, height: f32) -> f32 {
    let (min, max) = value.range;
    let track = (control.rect.height() - height) as i64;
    let down = if max > min {
        (i64::from(value.value - min) * track).div_euclid(i64::from(max - min))
    } else {
        0
    };
    down as f32
}

pub(crate) fn graphics(control: &Control) -> (Vec<u16>, bool) {
    (handle(control).into_iter().collect(), false)
}

pub(crate) fn draw(value: &Value, frame: &mut RgbaImage, paint: &Paint) {
    let control = paint.control;
    let Some(picture) = handle(control).and_then(|id| paint.image(id)) else { return };
    let top = handle_top(control, value, picture.height() as f32);
    paste(frame, picture, control.rect.min + Vec2::new(0.0, top));
}

/// Above the handle and below it.
pub(crate) fn targets(
    value: &Value,
    index: usize,
    control: &Control,
    geometry: &dyn Geometry,
    out: &mut Vec<(Target, Rect)>,
) {
    let (r, height) = (control.rect, HANDLE_HEIGHT);
    let top = r.min.y + handle_top(control, value, height);
    let above = Rect::new(r.min.x, r.min.y, r.max.x, top);
    let below = Rect::new(r.min.x, top + height, r.max.x, r.max.y);
    for (rect, up) in [(above, false), (below, true)] {
        if rect.height() > 0.0 {
            let shown = Control { rect, ..control.clone() };
            out.push((Target::SliderStep { control: index, up }, geometry.rect(&shown)));
        }
    }
}
