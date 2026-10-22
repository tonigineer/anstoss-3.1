//! The game's `HighButton`: a group of options, one of them chosen. With an
//! image, the chosen option shows its part of that strip of buttons; without,
//! a dot in the radio bullet painted into the background marks it. Some
//! groups are buttons that act rather than stay chosen.

use bevy_math::{Rect, Vec2};
use image::{Rgba, RgbaImage, imageops};

use a3_files::rch::{Control, Item, Kind, TOGGLE_OPTIONS};

use crate::classic::{Paint, TEXT_HOVER, centred};
use crate::pointer::{Geometry, Target};

/// The radio bullets painted into the backgrounds are Windows radio buttons,
/// 12 px square, with their top-left corner 2 px left of their list and 1 px
/// below the top of their option.
pub const BULLET_SIZE: f32 = 12.0;
const BULLET_CENTRE: Vec2 = Vec2::new(4.0, 7.0);
/// Labels of bulleted options start right of the bullet.
const LABEL_INSET: f32 = 10.0;
/// The dot marking a chosen radio option.
const DOT_COLOR: Rgba<u8> = Rgba([0, 0, 0, 255]);
/// A picture group whose labels are centred in their options.
const CENTRED_LABELS: u16 = 0x2;

/// What [`OptionsState::chosen`] is with no option chosen.
pub const NONE: usize = usize::MAX;

#[derive(Clone, Default, Debug)]
pub struct OptionsState {
    /// The chosen option, or [`NONE`].
    pub chosen: usize,
    /// The options' labels.
    pub labels: Vec<String>,
    /// A group whose options are buttons: none of them stays chosen, the one
    /// pointed at lights up.
    pub actions: bool,
    /// Options that are switched off: they show nothing and do not answer.
    pub off: Vec<bool>,
    /// Options disabled as the game disables a button group's
    /// (`FUN_007a72c2`): their label in the group's disabled ink, no answer.
    pub dimmed: Vec<bool>,
    /// A picture of its own under the options (`FUN_007a743a`).
    pub background: Option<u16>,
    /// A click on the chosen option clears it.
    pub toggle: bool,
}

impl OptionsState {
    pub(crate) fn new(items: &[Item], style: u16) -> Self {
        // The dialog file breaks a label into lines with "\n".
        let labels = items.iter().map(|i| i.text.replace("\\n", "\n")).collect();
        Self { labels, toggle: style & TOGGLE_OPTIONS != 0, ..Self::default() }
    }

    /// The chosen option, if one is.
    pub fn selected(&self) -> Option<usize> {
        (self.chosen != NONE).then_some(self.chosen)
    }

    pub fn item_on(&self, item: usize) -> bool {
        !self.off.get(item).copied().unwrap_or(false) && !self.dimmed(item)
    }

    fn dimmed(&self, item: usize) -> bool {
        self.dimmed.get(item).copied().unwrap_or(false)
    }

    /// A click on an option chooses it, unless it is switched off or the
    /// group's options are buttons; whether it counts.
    pub fn press(&mut self, item: usize) -> bool {
        if !self.item_on(item) {
            return false;
        }
        if self.toggle && self.chosen == item {
            self.chosen = NONE;
        } else if !self.actions {
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
        let Kind::Options { image, items, ink, chosen_ink, disabled_ink, .. } = &control.kind
        else {
            return;
        };
        let (ink, chosen_ink) = (*ink, *chosen_ink);
        let r = control.rect;
        let (x, y) = (r.min.x as i32, r.min.y as i32);
        // Cut to the control, as its window cuts it.
        if let Some(picture) = self.background.and_then(|id| paint.image(id)) {
            let (w, h) =
                (picture.width().min(r.width() as u32), picture.height().min(r.height() as u32));
            let part = imageops::crop_imm(picture, 0, 0, w, h).to_image();
            imageops::overlay(frame, &part, x.into(), y.into());
        }
        for (item, it) in items.iter().enumerate() {
            // The chosen option shows lit; the game's buttons of this kind
            // do not light up when pointed at.
            let lit = !self.actions && self.chosen == item;
            if !lit || self.off.get(item).copied().unwrap_or(false) {
                continue;
            }
            if *image == 0 {
                dot(frame, bullet_centre(control, item));
            } else if let Some(strip) = paint.image(*image) {
                // The chosen option shows its part of the button strip, cut
                // to the group's window.
                let (ix, iy) = (it.rect.min.x as u32, it.rect.min.y as u32);
                let room = (r.width() as u32, r.height() as u32);
                let w = (it.rect.width() as u32)
                    .min(strip.width().saturating_sub(ix))
                    .min(room.0.saturating_sub(ix));
                let h = (it.rect.height() as u32)
                    .min(strip.height().saturating_sub(iy))
                    .min(room.1.saturating_sub(iy));
                if w == 0 || h == 0 {
                    continue;
                }
                let part = imageops::crop_imm(strip, ix, iy, w, h).to_image();
                imageops::overlay(frame, &part, (x + ix as i32).into(), (y + iy as i32).into());
            }
        }
        let Some(font) = paint.font() else { return };
        let cell = font.height() as f32;
        for (item, (it, label)) in items.iter().zip(&self.labels).enumerate() {
            // An option outside the group's window does not show.
            if !inside(control, it) {
                continue;
            }
            let pointed = paint.pointed(Target::Option { control: paint.index, item });
            let [ir, ig, ib] = if self.dimmed(item) {
                *disabled_ink
            } else if item == self.chosen {
                chosen_ink
            } else {
                ink
            };
            let color = if pointed { TEXT_HOVER } else { Rgba([ir, ig, ib, 255]) };
            if *image == 0 {
                // Windows centres a radio button's label on its bullet.
                let top = (bullet_centre(control, item).y - cell / 2.0).floor() as i32;
                let left = x + (it.rect.min.x + LABEL_INSET) as i32;
                font.draw(frame, left, top, label, color, false);
            } else {
                // A label may break into lines where it says so.
                let lines: Vec<&str> = label.split('\n').collect();
                // Centred in whole pixels, as the game centres (`FUN_0079d255`).
                let height = cell * lines.len() as f32;
                let top = r.min.y
                    + it.rect.min.y
                    + if it.text_at.y == 0.0 {
                        ((it.rect.height() - height) / 2.0).floor()
                    } else {
                        it.text_at.y
                    };
                // Centred or at its place, as the group's style says
                // (`FUN_007a670e`: bits 1–2).
                let centre = match &control.kind {
                    Kind::Options { style, .. } => style & CENTRED_LABELS != 0,
                    _ => true,
                };
                for (i, line) in lines.iter().enumerate() {
                    let width = font.width(line, false) as i32;
                    let room = (it.rect.width() - it.text_at.x) as i32;
                    let start = (r.min.x + it.rect.min.x + it.text_at.x) as i32;
                    let left = if centre { centred(start, room, width) } else { start };
                    let top = (top + cell * i as f32).round() as i32;
                    font.draw(frame, left, top, line, color, false);
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
        let Kind::Options { image, items, .. } = &control.kind else { return };
        let r = control.rect;
        let lead = if *image == 0 { 2.0 } else { 0.0 };
        let shown = |&(i, it): &(usize, &Item)| self.item_on(i) && inside(control, it);
        for (item, it) in items.iter().enumerate().filter(shown) {
            let min = r.min + it.rect.min - Vec2::new(lead, 0.0);
            let rect = Rect::from_corners(min, (r.min + it.rect.max).min(r.max));
            out.push((Target::Option { control: index, item }, geometry.map(rect)));
        }
    }
}

pub(crate) fn graphics(state: &OptionsState, control: &Control) -> (Vec<u16>, bool) {
    let mut ids: Vec<u16> = state.background.into_iter().collect();
    if let Kind::Options { image, .. } = control.kind
        && image != 0
    {
        ids.push(image);
    }
    (ids, false)
}

/// Whether an option starts inside its group's window, which cuts what
/// lies outside it.
fn inside(control: &Control, item: &Item) -> bool {
    item.rect.min.x < control.rect.width() && item.rect.min.y < control.rect.height()
}

/// Centre of the painted bullet of an option, in classic pixels.
pub fn bullet_centre(control: &Control, item: usize) -> Vec2 {
    let Kind::Options { items, .. } = &control.kind else { return Vec2::ZERO };
    control.rect.min + items[item].rect.min + BULLET_CENTRE
}

/// The dot of a chosen radio option, drawn as Windows drew it: four pixels
/// square with its corners cut, in the middle of the 12 px bullet.
pub(crate) fn dot(frame: &mut RgbaImage, centre: Vec2) {
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
