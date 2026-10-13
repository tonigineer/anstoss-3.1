//! The game's `HighButton`: a group of options, one of them chosen. With an
//! image, the chosen option shows its part of that strip of buttons; without,
//! a dot in the radio bullet painted into the background marks it. Some
//! groups are buttons that act rather than stay chosen.

use bevy_math::{Rect, Vec2};
use image::{Rgba, RgbaImage, imageops};

use a3_files::rch::{Control, Item, Kind};

use crate::classic::{Paint, TEXT, TEXT_HOVER};
use crate::pointer::{Geometry, Target};

/// The radio bullets painted into the backgrounds are Windows radio buttons,
/// 12 px square, with their top-left corner 2 px left of their list and 1 px
/// below the top of their option.
pub const BULLET_SIZE: f32 = 12.0;
const BULLET_CENTRE: Vec2 = Vec2::new(4.0, 7.0);
/// Labels of bulleted options start right of the bullet.
const LABEL_INSET: f32 = 9.0;
/// The dot marking a chosen radio option.
const DOT_COLOR: Rgba<u8> = Rgba([100, 100, 100, 255]);

#[derive(Clone, Default, Debug)]
pub struct OptionsState {
    /// The chosen option.
    pub chosen: usize,
    /// The options' labels.
    pub labels: Vec<String>,
    /// A group whose options are buttons: none of them stays chosen, the one
    /// pointed at lights up.
    pub actions: bool,
    /// Options that are switched off: they show nothing and do not answer.
    pub off: Vec<bool>,
    /// How far into an option its label's room starts, where the option's
    /// picture shows something of its own on the left.
    pub label_indent: f32,
}

impl OptionsState {
    pub(crate) fn new(items: &[Item]) -> Self {
        // The dialog file breaks a label into lines with "\n".
        let labels = items.iter().map(|i| i.text.replace("\\n", "\n")).collect();
        Self { labels, ..Self::default() }
    }

    pub fn item_on(&self, item: usize) -> bool {
        !self.off.get(item).copied().unwrap_or(false)
    }

    /// A click on an option chooses it, unless it is switched off or the
    /// group's options are buttons; whether it counts.
    pub fn press(&mut self, item: usize) -> bool {
        if !self.item_on(item) {
            return false;
        }
        if !self.actions {
            self.chosen = item;
        }
        true
    }

    /// Chooses the option `step` options on, round the end.
    pub fn step(&mut self, step: i32, count: usize) {
        self.chosen = (self.chosen as i32 + step).rem_euclid(count as i32) as usize;
    }

    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let control = paint.control;
        let Kind::Options { image, items } = &control.kind else { return };
        let r = control.rect;
        let (x, y) = (r.min.x as i32, r.min.y as i32);
        for (item, it) in items.iter().enumerate() {
            // The chosen option shows lit; the game's buttons of this kind
            // do not light up when pointed at.
            let lit = !self.actions && self.chosen == item;
            if !lit || !self.item_on(item) {
                continue;
            }
            if *image == 0 {
                dot(frame, bullet_centre(control, item));
            } else if let Some(strip) = paint.image(*image) {
                // The chosen option shows its part of the button strip.
                let (ix, iy) = (it.rect.min.x as u32, it.rect.min.y as u32);
                let w = (it.rect.width() as u32).min(strip.width().saturating_sub(ix));
                let h = (it.rect.height() as u32).min(strip.height().saturating_sub(iy));
                let part = imageops::crop_imm(strip, ix, iy, w, h).to_image();
                imageops::overlay(frame, &part, (x + ix as i32).into(), (y + iy as i32).into());
            }
        }
        let Some(font) = paint.font() else { return };
        let cell = font.height() as f32;
        for (item, (it, label)) in items.iter().zip(&self.labels).enumerate() {
            let pointed = paint.pointed(Target::Option { control: paint.index, item });
            let color = if pointed { TEXT_HOVER } else { TEXT };
            if *image == 0 {
                // Windows centres a radio button's label on its bullet.
                let top = (bullet_centre(control, item).y - cell / 2.0).round() as i32;
                let left = x + (it.rect.min.x + LABEL_INSET) as i32;
                font.draw(frame, left, top, label, color, false);
            } else {
                // A label may break into lines where it says so.
                let lines: Vec<&str> = label.split('\n').collect();
                let height = cell * lines.len() as f32;
                let top = r.min.y + it.rect.min.y + (it.rect.height() - height) / 2.0;
                for (i, line) in lines.iter().enumerate() {
                    let width = font.width(line, false) as f32;
                    let room = it.rect.width() - self.label_indent;
                    let left = r.min.x + it.rect.min.x + self.label_indent + (room - width) / 2.0;
                    let top = (top + cell * i as f32).round() as i32;
                    font.draw(frame, left.round() as i32, top, line, color, false);
                }
            }
        }
    }

    /// Every option that is on; bulleted ones also answer on their bullet.
    pub(crate) fn targets(
        &self,
        index: usize,
        control: &Control,
        geometry: &dyn Geometry,
        out: &mut Vec<(Target, Rect)>,
    ) {
        let Kind::Options { image, items } = &control.kind else { return };
        let r = control.rect;
        let lead = if *image == 0 { 2.0 } else { 0.0 };
        for (item, it) in items.iter().enumerate().filter(|&(i, _)| self.item_on(i)) {
            let min = r.min + it.rect.min - Vec2::new(lead, 0.0);
            let rect = Rect::from_corners(min, r.min + it.rect.max);
            out.push((Target::Option { control: index, item }, geometry.map(rect)));
        }
    }
}

pub(crate) fn graphics(control: &Control) -> (Vec<u16>, bool) {
    match control.kind {
        Kind::Options { image, .. } if image != 0 => (vec![image], false),
        _ => (Vec::new(), false),
    }
}

/// Centre of the painted bullet of an option, in classic pixels.
pub fn bullet_centre(control: &Control, item: usize) -> Vec2 {
    let Kind::Options { items, .. } = &control.kind else { return Vec2::ZERO };
    control.rect.min + items[item].rect.min + BULLET_CENTRE
}

/// The dot of a chosen radio option, drawn as Windows drew it: four pixels
/// square with its corners cut, in the middle of the 12 px bullet.
fn dot(frame: &mut RgbaImage, centre: Vec2) {
    const DOT: [[bool; 4]; 4] = [
        [false, true, true, false],
        [true, true, true, true],
        [true, true, true, true],
        [false, true, true, false],
    ];
    let (left, top) = (centre.x as i32 - 2, centre.y as i32 - 2);
    for (dy, row) in DOT.iter().enumerate() {
        for (dx, &on) in row.iter().enumerate() {
            let (x, y) = (left + dx as i32, top + dy as i32);
            if on && let Some(pixel) = frame.get_pixel_mut_checked(x as u32, y as u32) {
                *pixel = DOT_COLOR;
            }
        }
    }
}
