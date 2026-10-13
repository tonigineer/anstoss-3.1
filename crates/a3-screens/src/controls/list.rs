//! List boxes: rows of tab-separated columns, one of them selected, with a
//! scrollbar while there are more rows than fit.

use bevy_math::{Rect, Vec2};
use image::{Rgba, RgbaImage, imageops};

use a3_files::rch::{Align, Control};

use super::player_box::{self, PlayerBox};
use crate::classic::{
    HANDLE, Paint, SCROLL_DOWN, SCROLL_UP, TEXT, TEXT_HOVER, columns, fill, paste,
};
use crate::pointer::{Geometry, Target};

/// Rows are one line of the dialog font.
pub const ROW_HEIGHT: f32 = 13.0;
/// Lists longer than they are tall get a scrollbar in the strip this wide
/// that the backgrounds leave free right of the list's frame: an arrow at
/// each end, a handle in between.
const SCROLLBAR_WIDTH: f32 = 20.0;
/// The scrollbar handle, the game's `ScrollerBlau.bmp`.
const SCROLL_HANDLE: Vec2 = Vec2::new(18.0, 20.0);
/// Rows the mouse wheel scrolls per notch, as Windows does.
pub const WHEEL_ROWS: i32 = 3;
const SELECTED_TEXT: Rgba<u8> = Rgba([255, 255, 255, 255]);
const SELECTED_ROW: Rgba<u8> = Rgba([0, 0, 0, 255]);

#[derive(Clone, Debug)]
pub struct ListState {
    /// The rows, their columns separated by tabs.
    pub rows: Vec<String>,
    pub selected: Option<usize>,
    /// The first row shown.
    pub scroll: usize,
    /// Where the columns start, relative to the control.
    pub tabs: Vec<f32>,
    /// How each column sits in its width — centred or to the right — where
    /// it does not start at its tab and run to the next.
    pub aligns: Vec<Option<(Align, f32)>>,
    /// The colour of each row, if it has one of its own, and the icons it
    /// shows: where from the list's left edge, and which graphic.
    pub inks: Vec<Option<[u8; 3]>>,
    pub row_icons: Vec<Vec<(f32, u16)>>,
    /// Rows marked as chosen besides the selected one, drawn as it is.
    pub marked: Vec<bool>,
    /// The box a row's player shows while he is dragged from the list.
    pub row_boxes: Vec<Option<PlayerBox>>,
}

impl Default for ListState {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            selected: None,
            scroll: 0,
            tabs: vec![0.0],
            aligns: Vec::new(),
            inks: Vec::new(),
            row_icons: Vec::new(),
            marked: Vec::new(),
            row_boxes: Vec::new(),
        }
    }
}

impl ListState {
    /// Scrolls by `rows`, as far as there are rows.
    pub fn scroll_by(&mut self, rows: i32, visible: usize) {
        let last = self.rows.len().saturating_sub(visible);
        self.scroll = (self.scroll as i32 + rows).clamp(0, last as i32) as usize;
    }

    /// Scrolls just far enough to show the selected row.
    pub fn reveal(&mut self, visible: usize) {
        if let Some(row) = self.selected {
            if row < self.scroll {
                self.scroll = row;
            } else if visible > 0 && row >= self.scroll + visible {
                self.scroll = row + 1 - visible;
            }
        }
    }

    /// Selects the row `step` rows on from the selected one, round the end,
    /// and shows it.
    pub fn step(&mut self, step: i32, visible: usize) {
        if !self.rows.is_empty() {
            let n = self.rows.len() as i32;
            let row = self.selected.map_or(0, |r| (r as i32 + step).rem_euclid(n));
            self.selected = Some(row as usize);
            self.reveal(visible);
        }
    }

    /// A click on the `row`th row shown selects it, if there is one.
    pub fn press(&mut self, row: usize) -> bool {
        if self.scroll + row >= self.rows.len() {
            return false;
        }
        self.selected = Some(self.scroll + row);
        true
    }

    /// Whether the list has more rows than it shows, and so a scrollbar.
    pub fn scrolls(&self, control: &Control) -> bool {
        self.rows.len() > visible_rows(control)
    }

    /// Where the scrollbar handle sits, in classic pixels: between the
    /// arrows, as far down as the list is scrolled.
    pub fn thumb(&self, control: &Control) -> Rect {
        let (up, down) = scroll_arrows(control);
        let room = (down.min.y - up.max.y - SCROLL_HANDLE.y).max(0.0);
        let last = self.rows.len().saturating_sub(visible_rows(control)).max(1) as f32;
        let top = up.max.y + (room * self.scroll as f32 / last).round();
        let left = up.min.x + ((SCROLLBAR_WIDTH - SCROLL_HANDLE.x) / 2.0).floor();
        Rect::new(left, top, left + SCROLL_HANDLE.x, top + SCROLL_HANDLE.y)
    }

    pub(crate) fn graphics(&self) -> (Vec<u16>, bool) {
        let icons = self.row_icons.iter().flatten().map(|&(_, id)| id);
        let boxes = self.row_boxes.iter().flatten().flat_map(|b| player_box::graphics(Some(b)).0);
        (icons.chain(boxes).collect(), false)
    }

    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let control = paint.control;
        let r = control.rect;
        let (x, y, right) = (r.min.x as i32, r.min.y as i32, r.max.x as i32);
        let scrolls = self.scrolls(control);
        if scrolls {
            let (up, down) = scroll_arrows(control);
            for (name, arrow) in [(SCROLL_UP, up), (SCROLL_DOWN, down)] {
                if let Some(picture) = paint.widget(name) {
                    // Centred in the arrow's square.
                    let gap = (arrow.size()
                        - Vec2::new(picture.width() as f32, picture.height() as f32))
                        / 2.0;
                    paste(frame, picture, (arrow.min + gap).floor());
                }
            }
            if let Some(handle) = paint.widget(HANDLE) {
                paste(frame, handle, self.thumb(control).min);
            }
        }
        let Some(font) = paint.font() else { return };
        let right = if scrolls { right - SCROLLBAR_WIDTH as i32 } else { right };
        // The rows are drawn on the list's own area, which cuts off the
        // last one where the list ends.
        let (width, height) = (r.width() as u32, r.height() as u32);
        let mut area = imageops::crop_imm(frame, x as u32, y as u32, width, height).to_image();
        let (left, right) = (0, right - x);
        let shown = self.rows.iter().skip(self.scroll).take(shown_rows(control));
        for (row, line) in shown.enumerate() {
            let frame = &mut area;
            let (x, top) = (left, 1 + (row as f32 * ROW_HEIGHT) as i32);
            let selected = self.selected == Some(self.scroll + row)
                || self.marked.get(self.scroll + row).copied().unwrap_or(false);
            let ink = self.inks.get(self.scroll + row).copied().flatten();
            let color = if selected {
                fill(frame, x, top, right - x, ROW_HEIGHT as i32, SELECTED_ROW);
                SELECTED_TEXT
            } else if paint.pointed(Target::Row { control: paint.index, row }) {
                TEXT_HOVER
            } else {
                ink.map_or(TEXT, |[r, g, b]| Rgba([r, g, b, 255]))
            };
            let cells = line.split('\t');
            // The text sits at the list's left edge, a pixel above the row.
            let at = (x, top - 1);
            columns(font, frame, at, cells, &self.tabs, &self.aligns, color, right);
            for &(left, id) in self.row_icons.get(self.scroll + row).into_iter().flatten() {
                if let Some(icon) = paint.image(id) {
                    paste(frame, icon, Vec2::new(x as f32 + left, top as f32));
                }
            }
        }
        imageops::replace(frame, &area, x.into(), y.into());
    }
}

/// The rows a list has room for.
pub fn visible_rows(control: &Control) -> usize {
    ((control.rect.height() - 2.0) / ROW_HEIGHT).floor().max(0.0) as usize
}

/// The rows a list shows: every one that starts inside it, as the game
/// draws them all and the list's window cuts them off (`CMyListBox`).
pub fn shown_rows(control: &Control) -> usize {
    ((control.rect.height() - 1.0) / ROW_HEIGHT).ceil().max(0.0) as usize
}

/// Where a list's scrollbar puts its up and down arrows, in classic pixels.
pub fn scroll_arrows(control: &Control) -> (Rect, Rect) {
    let r = control.rect;
    let left = r.max.x - SCROLLBAR_WIDTH;
    let up = Rect::new(left, r.min.y, r.max.x, r.min.y + SCROLLBAR_WIDTH);
    let down = Rect::new(left, r.max.y - SCROLLBAR_WIDTH, r.max.x, r.max.y);
    (up, down)
}

/// Every row it has room for, and the arrows of its scrollbar on top.
pub(crate) fn targets(
    index: usize,
    control: &Control,
    geometry: &dyn Geometry,
    out: &mut Vec<(Target, Rect)>,
) {
    let r = control.rect;
    for row in 0..shown_rows(control) {
        let y = r.min.y + 1.0 + row as f32 * ROW_HEIGHT;
        let rect = Rect::new(r.min.x, y, r.max.x, (y + ROW_HEIGHT).min(r.max.y));
        out.push((Target::Row { control: index, row }, geometry.map(rect)));
    }
    let (up, down) = geometry.scroll(control);
    out.push((Target::Scroll { control: index, up: true }, up));
    out.push((Target::Scroll { control: index, up: false }, down));
}
