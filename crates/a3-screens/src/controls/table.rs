//! A table the screen fills row by row, the game's `CScrBalkWnd` (the
//! statistics' pages): rows of tab-separated cells in columns of their own,
//! each row in its colour with a coloured box at its start if it has one,
//! over the dialog's background. Its finish (`FUN_006de7b0`) spreads up to
//! 24 rows over the height, and its paint (`FUN_006dee6c`, the cells by
//! `FUN_006dfd47`) puts each cell left, centred or right in its column.

use image::{Rgba, RgbaImage, imageops};

use crate::classic::{Paint, fill};

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
}

#[derive(Clone, Debug, Default)]
pub struct TableRow {
    pub text: String,
    pub ink: [u8; 3],
    /// The colour of the box at its start, if it has one.
    pub mark: Option<[u8; 3]>,
}

impl TableState {
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
        let (first, pitch) = self.layout(height as i32);
        let cells = self.cells();
        for (i, row) in self.rows.iter().take(SHOWN).enumerate() {
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
}
