//! A table the screen fills row by row, the game's `CScrBalkWnd` (the
//! statistics' pages): rows of tab-separated cells in columns of their own,
//! each row in its colour with a coloured box at its start if it has one,
//! over the dialog's background. Its finish (`FUN_006de7b0`) spreads up to
//! 24 rows over the height, and its paint (`FUN_006dee6c`, the cells by
//! `FUN_006dfd47`) puts each cell left, centred or right in its column.

use bevy_math::{Rect, Vec2};
use image::{Rgba, RgbaImage, imageops};

use super::combo;
use super::list::{HANDLES, MIN_HANDLE, SCROLLBAR_WIDTH, TRACKS, TRACK_LEFT, TRACK_WIDTH};
use crate::classic::{Paint, SCROLL_DOWN, SCROLL_UP, fill, paste};

/// A row's height, and the most rows shown.
const ROW: i32 = 14;
const SHOWN: usize = 25;
/// The most a row's gap grows.
const GAP: i32 = 4;
/// The box a row starts with.
const MARK: (i32, i32) = (20, 14);
/// A column's place: its left edge, and 0x4000 centred, 0x8000 to the
/// right in its width.
const PLACE: u16 = 0x3fff;
const CENTRE: u16 = 0x4000;
const RIGHT: u16 = 0x8000;
/// The first cell stands this far in, and a left one this far more.
const FIRST_IN: i32 = 2;

#[derive(Clone, Debug, Default)]
pub struct TableState {
    /// The columns as `FUN_006df48b` takes them: where each starts with
    /// its alignment, and its width, 0 to run to the next.
    pub columns: Vec<(u16, u16)>,
    pub rows: Vec<TableRow>,
    /// How wide a row is (`+0xc0`).
    pub width: i32,
    /// Up to this many rows stand centred up and down (`+0x10ec`); −1
    /// never.
    pub centred: i32,
    /// Which row stands at the top once there are more than fit — the
    /// bar's position (`+0x10e8`).
    pub scroll: usize,
}

#[derive(Clone, Debug, Default)]
pub struct TableRow {
    pub text: String,
    pub ink: [u8; 3],
    /// The colour of the box at its start, if it has one.
    pub mark: Option<[u8; 3]>,
}

impl TableState {
    /// Whether there are more rows than fit, so the bar stands.
    pub fn scrolls(&self) -> bool {
        self.rows.len() > SHOWN
    }

    /// Scrolls one row up or down, never past the last that fits
    /// (`OnVScroll_006dfcde`'s line step).
    pub fn scroll_by(&mut self, delta: i32) {
        let last = self.rows.len().saturating_sub(SHOWN) as i32;
        self.scroll = (self.scroll as i32 + delta).clamp(0, last) as usize;
    }

    /// The bar's window: it ends short of the table's own edges, 21 pixels
    /// from the right and 16 from the bottom. Its arrows sit at its right
    /// and bottom, as a list's do.
    pub(crate) fn bar_arrows(rect: Rect) -> (Rect, Rect) {
        let (right, bottom) = (rect.max.x - 21.0, rect.max.y - 16.0);
        (
            Rect::new(right - SCROLLBAR_WIDTH, rect.min.y, right, rect.min.y + SCROLLBAR_WIDTH),
            Rect::new(
                right - SCROLLBAR_WIDTH,
                bottom - SCROLLBAR_WIDTH,
                right,
                bottom,
            ),
        )
    }

    /// Where the rows stand: the first one's top, and how far apart.
    fn layout(&self, height: i32) -> (i32, i32) {
        let rows = self.rows.len() as i32;
        let shown = rows.min(SHOWN as i32);
        let gap = if (2..SHOWN as i32).contains(&rows) {
            ((height - rows * ROW) / (rows - 1)).min(GAP)
        } else {
            0
        };
        let first = if self.centred > 0 && rows <= self.centred {
            (height - (shown - 1) * (ROW + gap) - ROW) / 2
        } else {
            0
        };
        (first, ROW + gap)
    }

    /// Each column's left edge, alignment and right edge.
    fn cells(&self) -> Vec<(i32, u16, i32)> {
        let lefts: Vec<i32> = self.columns.iter().map(|&(x, _)| i32::from(x & PLACE)).collect();
        self.columns
            .iter()
            .enumerate()
            .map(|(i, &(x, w))| {
                let left = lefts[i];
                let right = if w >= 1 {
                    left + i32::from(w)
                } else {
                    lefts.get(i + 1).copied().unwrap_or(self.width)
                };
                (left, x & (CENTRE | RIGHT), right)
            })
            .collect()
    }

    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let Some(font) = paint.font() else { return };
        let r = paint.control.rect;
        let (x0, y0) = (r.min.x as i32, r.min.y as i32);
        let (width, height) = (r.width() as u32, r.height() as u32);
        // It paints on a window of its own, and so not past its edges.
        let mut area = imageops::crop_imm(frame, x0 as u32, y0 as u32, width, height).to_image();
        if self.scrolls() {
            self.bar(&mut area, r, paint);
        }
        let (first, pitch) = self.layout(height as i32);
        let cells = self.cells();
        for (i, row) in self.rows.iter().skip(self.scroll).take(SHOWN).enumerate() {
            let y = first + i as i32 * pitch;
            if let Some([r, g, b]) = row.mark {
                fill(&mut area, 0, y, MARK.0, MARK.1, Rgba([r, g, b, 255]));
            }
            let [r, g, b] = row.ink;
            let ink = Rgba([r, g, b, 255]);
            for (k, (text, &(left, align, right))) in row.text.split('\t').zip(&cells).enumerate()
            {
                if left >= self.width {
                    break;
                }
                let right = right.min(self.width);
                let x = if k == 0 { left + FIRST_IN } else { left };
                let w = font.width(text, false) as i32 + 1;
                let dx = match align {
                    RIGHT => right - x - w,
                    CENTRE => (right - x - w) / 2,
                    _ if k == 0 => FIRST_IN,
                    _ => 0,
                };
                font.draw(&mut area, x + dx, y, text, ink, false);
            }
        }
        imageops::replace(frame, &area, i64::from(x0), i64::from(y0));
    }

    /// The bar over the rows' right edge: yellow, with a track and a
    /// handle as long as the rows shown are of all, moved down as the
    /// table is scrolled (`FUN_007ae43e`, `FUN_007ad818`).
    fn bar(&self, area: &mut RgbaImage, rect: Rect, paint: &Paint) {
        let (up, down) = Self::bar_arrows(rect);
        let off = rect.min;
        let move_ = |q: Rect| {
            Rect::new(q.min.x - off.x, q.min.y - off.y, q.max.x - off.x, q.max.y - off.y)
        };
        let (up, down) = (move_(up), move_(down));
        if let Some(track) = paint.widget(TRACKS[4]) {
            let high = (down.min.y - up.max.y).max(0.0) as u32;
            let part = imageops::crop_imm(track, 0, 0, TRACK_WIDTH.min(track.width()), high).to_image();
            imageops::overlay(area, &part, (up.min.x + TRACK_LEFT as f32) as i64, up.max.y as i64);
        }
        for (name, arrow) in [(SCROLL_UP, up), (SCROLL_DOWN, down)] {
            if let Some(picture) = paint.widget(name) {
                // Centred in the arrow's square.
                let gap =
                    (arrow.size() - Vec2::new(picture.width() as f32, picture.height() as f32))
                        / 2.0;
                paste(area, picture, (arrow.min + gap).floor());
            }
        }
        if let Some(long) = paint.widget(HANDLES[4]) {
            let track = (down.min.y - up.max.y) as i32;
            let length = (track * SHOWN as i32 / self.rows.len() as i32).max(MIN_HANDLE);
            let last = self.rows.len() as i32 - SHOWN as i32;
            let at = if last > 0 { self.scroll as i32 * (track - length) / last } else { 0 };
            paste(area, &combo::cut(long, length as u32), Vec2::new(up.min.x, up.max.y + at as f32));
        }
    }
}
