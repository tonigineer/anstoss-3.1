//! Faders, the game's sliders (`CMySchieber`): a small picture, the
//! handle, that stands higher or lower in the control by its value — or,
//! in the lying ones, further left or right. A click before the handle
//! takes one off, after it adds one (`OnLButtonDown_007ab7e0`, page up
//! and down).

use bevy_math::{Rect, Vec2};
use image::RgbaImage;

use a3_files::rch::{Control, Kind};

use super::spin::Value;
use crate::classic::{Paint, paste};
use crate::pointer::{Geometry, Target};

/// How high the handle is: the game takes it from its picture, and the
/// only faders' (2518, `scroller/schieberGruen.bmp`) is this high.
const HANDLE_HEIGHT: f32 = 6.0;

/// The handle's picture, and whether the fader stands or lies.
fn handle(control: &Control) -> Option<(u16, bool)> {
    match control.kind {
        Kind::Fader { handle, upright } => Some((handle, upright)),
        _ => None,
    }
}

/// How far along its track the handle sits: as far as the value is in
/// its range (`FUN_007abbb9`), in whole pixels rounded down.
fn handle_on(value: &Value, size: f32, track: f32) -> f32 {
    let (min, max) = value.range;
    let track = (track - size) as i64;
    let on = if max > min {
        (i64::from(value.value - min) * track).div_euclid(i64::from(max - min))
    } else {
        0
    };
    on as f32
}

pub(crate) fn graphics(control: &Control) -> (Vec<u16>, bool) {
    (handle(control).map(|(id, _)| id).into_iter().collect(), false)
}

pub(crate) fn draw(value: &Value, frame: &mut RgbaImage, paint: &Paint) {
    let control = paint.control;
    let Some((id, upright)) = handle(control) else { return };
    let Some(picture) = paint.image(id) else { return };
    let (r, size) = (control.rect, if upright { picture.height() } else { picture.width() });
    let on = handle_on(value, size as f32, if upright { r.height() } else { r.width() });
    let at = if upright {
        r.min + Vec2::new(0.0, on)
    } else {
        r.min + Vec2::new(on, (r.height() - picture.height() as f32) / 2.0)
    };
    paste(frame, picture, at);
}

/// Before the handle and after it.
pub(crate) fn targets(
    value: &Value,
    index: usize,
    control: &Control,
    geometry: &dyn Geometry,
    out: &mut Vec<(Target, Rect)>,
) {
    let (r, size) = (control.rect, HANDLE_HEIGHT);
    let upright = matches!(control.kind, Kind::Fader { upright: true, .. });
    let (before, after) = if upright {
        let on = r.min.y + handle_on(value, size, r.height());
        (
            Rect::new(r.min.x, r.min.y, r.max.x, on),
            Rect::new(r.min.x, on + size, r.max.x, r.max.y),
        )
    } else {
        let on = r.min.x + handle_on(value, size, r.width());
        (
            Rect::new(r.min.x, r.min.y, on, r.max.y),
            Rect::new(on + size, r.min.y, r.max.x, r.max.y),
        )
    };
    for (rect, up) in [(before, false), (after, true)] {
        if (if upright { rect.height() } else { rect.width() }) > 0.0 {
            let shown = Control { rect, ..control.clone() };
            out.push((Target::SliderStep { control: index, up }, geometry.rect(&shown)));
        }
    }
}
