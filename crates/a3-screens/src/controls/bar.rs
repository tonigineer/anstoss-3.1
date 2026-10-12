//! The menu bar the game puts along the bottom of its main screens, its
//! `CLeiste`: a strip of symbols, which draws itself, with a tooltip for the
//! symbol the pointer rests on.

use bevy_math::Rect;
use image::{Rgba, RgbaImage, imageops};

use a3_files::bitmap_font::BitmapFont;
use a3_files::rch::{Control, Kind};

use crate::classic::{Paint, TEXT, fill};
use crate::pointer::{Geometry, Target};

/// The bar's pictures: as it stands, with a symbol pressed, lit — every
/// other symbol from a strip of its own — and greyed out.
pub const NORMAL: u16 = 1130;
pub const PRESSED: u16 = 1131;
pub const LIT: [u16; 2] = [1132, 2510];
pub const OFF: u16 = 1133;
pub const IMAGES: [u16; 5] = [NORMAL, PRESSED, LIT[0], LIT[1], OFF];
/// Tooltips: their pale yellow, the room around their text, and how far
/// above what they explain.
const TIP_BACK: Rgba<u8> = Rgba([255, 255, 225, 255]);
const TIP_PADDING: i32 = 2;
const TIP_GAP: i32 = 2;

#[derive(Clone, Default, Debug)]
pub struct BarState {
    /// The symbol of the open screen, which shows pressed.
    pub chosen: usize,
    /// Symbols out of reach: greyed out, they do not answer.
    pub off: Vec<bool>,
    /// What the symbols say when the pointer rests on them.
    pub tips: Vec<String>,
}

impl BarState {
    pub fn item_on(&self, item: usize) -> bool {
        !self.off.get(item).copied().unwrap_or(false)
    }

    /// Its symbols as they stand, runs of those out of reach greyed, the
    /// open screen's pressed, and the one pointed at lit.
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let control = paint.control;
        let Kind::Bar { items } = &control.kind else { return };
        let (x, y) = (control.rect.min.x as i64, control.rect.min.y as i64);
        let part = |frame: &mut RgbaImage, image: u16, left: f32, right: f32| {
            let Some(strip) = paint.image(image) else { return };
            let (left, right) = (left.max(0.0) as u32, (right as u32).min(strip.width()));
            if right > left {
                let piece = imageops::crop_imm(strip, left, 0, right - left, strip.height());
                imageops::overlay(frame, &piece.to_image(), x + i64::from(left), y);
            }
        };
        if let Some(bar) = paint.image(NORMAL) {
            imageops::overlay(frame, bar, x, y);
        }
        let mut item = 0;
        while item < items.len() {
            if self.item_on(item) {
                item += 1;
                continue;
            }
            let first = item;
            while item < items.len() && !self.item_on(item) {
                item += 1;
            }
            part(frame, OFF, items[first].rect.min.x, items[item - 1].rect.max.x);
        }
        if let Some(chosen) = items.get(self.chosen) {
            part(frame, PRESSED, chosen.rect.min.x, chosen.rect.max.x);
        }
        if let Some(Target::Option { control: c, item }) = paint.view.hovered
            && c == paint.index
            && item != self.chosen
            && let Some(lit) = items.get(item)
        {
            part(frame, LIT[item % 2], lit.rect.min.x - 1.0, lit.rect.max.x + 1.0);
        }
    }

    pub(crate) fn targets(
        &self,
        index: usize,
        control: &Control,
        geometry: &dyn Geometry,
        out: &mut Vec<(Target, Rect)>,
    ) {
        let Kind::Bar { items } = &control.kind else { return };
        for item in (0..items.len()).filter(|&i| self.item_on(i)) {
            out.push((Target::Option { control: index, item }, geometry.bar(control, item)));
        }
    }
}

/// A symbol of the bar, in classic pixels.
pub fn symbol(control: &Control, item: usize) -> Rect {
    let Kind::Bar { items } = &control.kind else { return control.rect };
    let r = items.get(item).map_or(Rect::default(), |i| i.rect);
    Rect::from_corners(control.rect.min + r.min, control.rect.min + r.max)
}

/// A tooltip above a symbol, as Windows drew them: black on pale yellow in a
/// thin black frame.
pub(crate) fn draw_tip(frame: &mut RgbaImage, font: &BitmapFont, symbol: Rect, text: &str) {
    let width = font.width(text, false) as i32 + 2 * TIP_PADDING + 2;
    let height = font.height() as i32 + 2;
    let left = (symbol.min.x as i32).min(frame.width() as i32 - width).max(0);
    let top = symbol.min.y as i32 - height - TIP_GAP;
    fill(frame, left, top, width, height, TEXT);
    fill(frame, left + 1, top + 1, width - 2, height - 2, TIP_BACK);
    font.draw(frame, left + 1 + TIP_PADDING, top + 1, text, TEXT, false);
}
