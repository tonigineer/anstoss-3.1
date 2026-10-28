//! A table the screen fills row by row, the game's `CScrBalkWnd` (the
//! statistics' pages): rows of tab-separated cells in columns of their own,
//! each row in its colour with a coloured box at its start if it has one,
//! over the dialog's background. Its finish (`FUN_006de7b0`) spreads up to
//! 24 rows over the height, and its paint (`FUN_006dee6c`, the cells by
//! `FUN_006dfd47`) puts each cell left, centred or right in its column.
//! With its `cross` set it is the season results' cross-table instead, the
//! game's `CTabellenWnd`: twenty pixels down, ten columns at a time with
//! their numbers over them and a bar to step across (`FUN_0072cec3`,
//! `FUN_0072cc7e`).

use a3_files::bitmap_font::BitmapFont;
use bevy_math::{Rect, Vec2};
use image::{Rgba, RgbaImage, imageops};

use super::{combo, slider};
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
/// The cross-table's own cell (`FUN_0079c74a` with COLORREF 200), and
/// how far down its bar's track stands.
const ACROSS_BLUE: Rgba<u8> = Rgba([0, 0, 200, 255]);
const ACROSS_TRACK: u32 = 7;
/// The cross-table's bar: 138 pixels wide (`FUN_0072cc7e`'s 0x8a), and
/// the cells between are 31 apart and 30 wide (`FUN_0072cec3`).
const ACROSS_WIDE: i32 = 138;
const ACROSS_CELL: i32 = 30;

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
    /// The cross-table's window: the number of the first column shown
    /// (`+0xc0`), only for the season results page; then the rows stand
    /// twenty pixels down at a pitch of as many as fit, every column has
    /// its number over the bar and its cell against itself a blue box.
    pub cross: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct TableRow {
    pub text: String,
    pub ink: [u8; 3],
    /// The colour of the box at its start, if it has one.
    pub mark: Option<[u8; 3]>,
    /// The row's own columns, where the page set them for this one row
    /// (`FUN_006df44c` and `FUN_006df73a`); else the table's.
    pub columns: Option<Vec<(u16, u16)>>,
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

    /// Moves the cross-table's window one column over, never past the
    /// last ten that fit (`OnHScroll_0072d54c` with the bar's range 0 to
    /// the count less ten).
    pub fn across_by(&mut self, delta: i32) {
        if let Some(across) = self.cross {
            let last = self.rows.len().saturating_sub(10) as i32;
            self.cross = Some((i32::try_from(across).unwrap_or(0) + delta).clamp(0, last) as usize);
        }
    }

    /// The arrows of the cross-table's bar, at the head of its window —
    /// the `CMyScrBar` 138 pixels wide and twenty high that
    /// `FUN_0072cc7e` gives it.
    pub(crate) fn across_arrows(rect: Rect) -> (Rect, Rect) {
        let (x, y) = (rect.min.x, rect.min.y);
        let right = x + ACROSS_WIDE as f32;
        (
            Rect::new(x, y, x + SCROLLBAR_WIDTH, y + SCROLLBAR_WIDTH),
            Rect::new(right - SCROLLBAR_WIDTH, y, right, y + SCROLLBAR_WIDTH),
        )
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
        if self.cross.is_some() {
            // `FUN_0072c918`: under the bar's twenty pixels, as many
            // apart as the rest gives to each row, no more than 21.
            return (0x14, ((height - 0x14) / rows.max(1)).min(0x15));
        }
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
        Self::cells_of(&self.columns, self.width)
    }

    /// The same for one row's own columns.
    fn cells_of(columns: &[(u16, u16)], width: i32) -> Vec<(i32, u16, i32)> {
        let lefts: Vec<i32> = columns.iter().map(|&(x, _)| i32::from(x & PLACE)).collect();
        columns
            .iter()
            .enumerate()
            .map(|(i, &(x, w))| {
                let left = lefts[i];
                let align = x & (CENTRE | RIGHT);
                let right = if w >= 1 {
                    left + i32::from(w)
                } else {
                    // `FUN_006df779` gives a width-less column the next
                    // one's left as its right edge; a right-aligned cell
                    // ends 2 before it, as on the Champions League group
                    // tables (`FUN_00447c15`'s columns at 0x879508).
                    match lefts.get(i + 1) {
                        Some(&n) if align == RIGHT => n - 2,
                        Some(&n) => n,
                        None => width,
                    }
                };
                (left, align, right)
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
        if let Some(across) = self.cross {
            self.across_bar(&mut area, across, paint);
            self.numbers(&mut area, across, font);
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
            let own;
            let cells = match &row.columns {
                Some(columns) => {
                    own = Self::cells_of(columns, self.width);
                    &own
                }
                None => &cells,
            };
            let place = self.scroll + i;
            for (k, (text, &(left, align, right))) in row.text.split('\t').zip(cells).enumerate()
            {
                if left >= self.width {
                    break;
                }
                let right = right.min(self.width);
                // The cross-table starts its labels where the column
                // stands; the other tables give the first one two over.
                let in_ = if self.cross.is_some() || k > 0 { 0 } else { FIRST_IN };
                let x = left + in_;
                let w = font.width(text, false) as i32 + 1;
                let dx = match align {
                    RIGHT => right - x - w,
                    CENTRE => (right - x - w) / 2,
                    _ => in_,
                };
                font.draw(&mut area, x + dx, y, text, ink, false);
                // A club's own cell, its score over a blue box
                // (`FUN_0079c74a` with 200, drawn after the text).
                if let Some(across) = self.cross
                    && k > 0
                    && across + k - 1 == place {
                    fill(&mut area, left, y, ACROSS_CELL, pitch - 1, ACROSS_BLUE);
                }
            }
        }
        imageops::replace(frame, &area, i64::from(x0), i64::from(y0));
    }

    /// The numbers over the cross-table's columns, at the bar's height:
    /// ten of them from the window on, their places from one, three
    /// pixels further in while they stand below ten, each in the colour
    /// of the row it numbers (`FUN_0072cec3`).
    fn numbers(&self, area: &mut RgbaImage, across: usize, font: &BitmapFont) {
        for j in 0..10 {
            let Some(row) = self.rows.get(across + j) else { break };
            let x = 147 + 31 * j as i32 + if across + j < 9 { 3 } else { 0 };
            let [r, g, b] = row.ink;
            font.draw(area, x, 2, &(across + j + 1).to_string(), Rgba([r, g, b, 255]), false);
        }
    }

    /// The bar at the head of the cross-table's window (`FUN_0072cc7e`):
    /// its semi-transparent track — the yellow line and the groove
    /// darkening the background — between the arrows at the ends of 138
    /// pixels, and its handle as long as the ten columns shown are of
    /// all, run along as the window is stepped (`FUN_007ae43e`,
    /// `FUN_007ad818`).
    fn across_bar(&self, area: &mut RgbaImage, across: usize, paint: &Paint) {
        let track = ACROSS_WIDE - 2 * 20;
        if let Some(t) = paint.widget(slider::TRACKS[4]) {
            let part = imageops::crop_imm(t, 0, 0, track.min(t.width() as i32) as u32, t.height());
            imageops::overlay(area, &part.to_image(), 20, ACROSS_TRACK as i64);
        }
        for (name, x) in slider::ARROWS.into_iter().zip([0, ACROSS_WIDE - 20]) {
            if let Some(arrow) = paint.widget(name) {
                paste(area, arrow, Vec2::new(x as f32, 0.0));
            }
        }
        if let Some(long) = paint.widget(slider::HANDLES[4]) {
            let count = self.rows.len() as i32;
            let length = (track * 10 / count.max(1)).max(MIN_HANDLE);
            let last = (count - 10).max(0);
            let at = if last > 0 { across as i32 * (track - length) / last } else { 0 };
            paste(area, &slider::cut(long, length as u32), Vec2::new((20 + at) as f32, 0.0));
        }
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
