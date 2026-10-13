//! A menu that opens where a right click falls, as the game's `CPopupMenue`
//! (dialog 415, 120 pixels wide) draws one: light grey paper in a black
//! edge with a shadow to the right, an entry every 16 pixels. A blank entry
//! only makes room.

use bevy_math::{Rect, Vec2};
use image::{Rgba, RgbaImage};

use a3_files::rch::Control;

use crate::classic::{Paint, TEXT, TEXT_HOVER, fill};
use crate::pointer::{Geometry, Target};

const WIDTH: f32 = 120.0;
const ENTRY: f32 = 16.0;
const TEXT_LEFT: i32 = 2;
const PAPER: Rgba<u8> = Rgba([225, 216, 220, 255]);
const EDGE: Rgba<u8> = Rgba([0, 0, 0, 255]);
const SHADOW: Rgba<u8> = Rgba([125, 125, 125, 255]);
const SHADOW_WIDTH: i32 = 2;

#[derive(Clone, Default, Debug)]
pub struct MenuState {
    /// Where it opened, its top left; `None` while it is closed.
    pub at: Option<Vec2>,
    pub entries: Vec<String>,
}

/// Where entry `i` is of a menu opened at `at`.
pub(crate) fn entry(at: Vec2, i: usize) -> Rect {
    let top = at.y + ENTRY * i as f32;
    Rect::new(at.x, top, at.x + WIDTH, top + ENTRY)
}

impl MenuState {
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let Some(at) = self.at else { return };
        let height = (ENTRY * self.entries.len() as f32) as i32;
        let (x, y) = (at.x as i32, at.y as i32);
        fill(frame, x + WIDTH as i32, y + SHADOW_WIDTH, SHADOW_WIDTH, height, SHADOW);
        fill(frame, x, y, WIDTH as i32, height, EDGE);
        fill(frame, x + 1, y + 1, WIDTH as i32 - 2, height - 2, PAPER);
        let Some(font) = paint.font() else { return };
        for (i, text) in self.entries.iter().enumerate() {
            let r = entry(at, i);
            let pointed = paint.pointed(Target::Option { control: paint.index, item: i });
            let color = if pointed { TEXT_HOVER } else { TEXT };
            let top = r.min.y as i32 + (ENTRY as i32 - font.height() as i32) / 2;
            font.draw(frame, x + TEXT_LEFT, top, text, color, false);
        }
    }

    pub(crate) fn targets(
        &self,
        index: usize,
        _: &Control,
        geometry: &dyn Geometry,
        out: &mut Vec<(Target, Rect)>,
    ) {
        let Some(at) = self.at else { return };
        for (i, text) in self.entries.iter().enumerate() {
            if !text.trim().is_empty() {
                out.push((Target::Option { control: index, item: i }, geometry.map(entry(at, i))));
            }
        }
    }
}
