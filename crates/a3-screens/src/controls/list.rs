//! List boxes: rows of tab-separated columns, one of them selected, with a
//! scrollbar while there are more rows than fit — or always, as a list's
//! style says. The scrollbar is the game's `CMyScrBar` in the list's colour:
//! a handle as long as the rows shown are of all, over a track in some.

use bevy_math::{Rect, Vec2};
use image::{Rgba, RgbaImage, imageops};

use a3_files::rch::{Align, Control, Kind};

use super::combo;
use super::player_box::{self, PlayerBox};
use crate::classic::{Paint, SCROLL_DOWN, SCROLL_UP, TEXT, TEXT_HOVER, columns, fill, paste};
use crate::pointer::{Geometry, Target};

/// Rows are as high as the dialog file says; this high where it says
/// nothing.
const ROW_HEIGHT: f32 = 13.0;
/// The scrollbar runs down the list's right edge this wide: an arrow at
/// each end, a handle in between.
const SCROLLBAR_WIDTH: f32 = 20.0;
/// The scrollbars' colours (the table at 0x8ade78, by the list's style):
/// the long bars their handles are cut from, blue, red, red, green and
/// yellow, and the tracks some draw behind them.
pub(crate) const HANDLES: [&str; 5] = [
    "ScrGrossBlau.bmp",
    "ScrGrossRot.bmp",
    "ScrGrossRot.bmp",
    "ScrGrossGruen.bmp",
    "ScrGrossGelb.bmp",
];
pub(crate) const TRACKS: [&str; 6] =
    ["ScrBlauV.tga", "ScrRotV.tga", "ScrBlauV.tga", "ScrGruenV.tga", "ScrGelbV.tga", "ScrRotV.tga"];
/// A list's style bits: its scrollbar only while it scrolls (either of
/// 0x0c), with a track (0x08), its colour (0x70); 0x80 and 0x04 alone let
/// the rows run under it.
const ONLY_SCROLLING: u8 = 0x0c;
const WITH_TRACK: u8 = 0x08;
const COLOUR: u8 = 0x70;
const WIDE_ROWS: u8 = 0x80;
const BESIDE: u8 = 0x04;
/// The track: where it starts across the scrollbar, and how wide it is.
const TRACK_LEFT: u32 = 7;
const TRACK_WIDTH: u32 = 13;
/// The shortest the handle gets (`FUN_007ae43e`).
const MIN_HANDLE: i32 = 8;
/// Rows the mouse wheel scrolls per notch, as Windows does.
pub const WHEEL_ROWS: i32 = 3;
/// The colour a selected row is drawn half over with, by the list's
/// scheme (the schemes at 0x8ae138, 0x2e4 bytes apart; `FUN_0079c633` with
/// alpha 0x80), as high as the list's font; rows this high or more start
/// it further in. The schemes hold COLORREFs, which the blend hands on as
/// they are to a library that reads 0xRRGGBB: red and blue swap, the
/// first scheme's red shows blue, the fifth's cyan yellow.
const SELECTION: [[u8; 3]; 6] =
    [[0, 0, 255], [255, 0, 0], [200, 200, 200], [0, 255, 0], [255, 255, 0], [200, 200, 200]];
const TALL_ROW: f32 = 36.0;
const TALL_ROW_INDENT: i32 = 12;

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
    /// The icons drawn with their pure black left out: those the game gives
    /// its list with a colour key (`FUN_007a95f6` with −1), not the
    /// abilities'.
    pub keyed: Vec<u16>,
    /// Rows marked as chosen besides the selected one, drawn as it is.
    pub marked: Vec<bool>,
    /// The box a row's player shows while he is dragged from the list.
    pub row_boxes: Vec<Option<PlayerBox>>,
    /// What each row stands for, for the screen to look up: a player's
    /// place in the squad, say.
    pub keys: Vec<usize>,
    /// Whether a click tells which column it fell in (`Target::Cell`).
    pub by_column: bool,
    /// A bar each row may show under its text.
    pub bars: Vec<Option<Bar>>,
    /// The colour the selected row is drawn half over with, where the
    /// screen sets one of its own (`FUN_005a0dc0`).
    pub selection: Option<[u8; 3]>,
}

/// A row's progress bar (`FUN_007a9370`, painted by `FUN_007a7d9c`): a
/// white strip this wide from `left`, black over it as far as `value` of
/// 100 goes.
#[derive(Clone, Copy, Debug)]
pub struct Bar {
    pub left: i32,
    pub width: i32,
    pub value: i32,
    pub fill: Rgba<u8>,
}

const BAR_HEIGHT: i32 = 11;
const BAR_BACK: Rgba<u8> = Rgba([255, 255, 255, 255]);
/// A progress bar's colour.
pub const BAR_FILL: Rgba<u8> = Rgba([0, 0, 0, 255]);

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
            keyed: Vec::new(),
            marked: Vec::new(),
            row_boxes: Vec::new(),
            keys: Vec::new(),
            by_column: false,
            bars: Vec::new(),
            selection: None,
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

    /// Whether the list has more rows than it shows.
    pub fn scrolls(&self, control: &Control) -> bool {
        self.rows.len() > visible_rows(control)
    }

    /// Whether the classic look shows its scrollbar: while it scrolls, or
    /// always, as its style says (`FUN_007a9863`).
    pub fn scrollbar(&self, control: &Control) -> bool {
        self.scrolls(control) || style(control) & ONLY_SCROLLING == 0
    }

    /// Where the scrollbar handle sits, in classic pixels: between the
    /// arrows, as long as the rows shown are of all (the whole track when
    /// they are all), as far down as the list is scrolled (`FUN_007ae43e`,
    /// `FUN_007ad818`).
    pub fn thumb(&self, control: &Control) -> Rect {
        let (up, down) = scroll_arrows(control);
        let track = (down.min.y - up.max.y) as i32;
        let (rows, shown) = (self.rows.len() as i32, visible_rows(control) as i32);
        let length = if shown < rows { (track * shown / rows).max(MIN_HANDLE) } else { track };
        let last = rows - shown;
        let down = if last > 0 { self.scroll as i32 * (track - length) / last } else { 0 };
        let top = up.max.y + down as f32;
        Rect::new(up.min.x, top, up.max.x, top + length as f32)
    }

    /// Whether the pointer is on a column of the row.
    fn pointed_cell(&self, paint: &Paint, row: usize) -> bool {
        self.by_column
            && (0..self.tabs.len())
                .any(|column| paint.pointed(Target::Cell { control: paint.index, row, column }))
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
        if self.scrollbar(control) {
            let (up, down) = scroll_arrows(control);
            if style(control) & WITH_TRACK != 0
                && let Some(track) = paint.widget(TRACKS[scheme(control).min(TRACKS.len() - 1)])
            {
                let height = ((down.min.y - up.max.y) as u32).min(track.height());
                let width = TRACK_WIDTH.min(track.width());
                let part = imageops::crop_imm(track, 0, 0, width, height).to_image();
                let at = Vec2::new(up.min.x + TRACK_LEFT as f32, up.max.y);
                imageops::overlay(frame, &part, at.x as i64, at.y as i64);
            }
            for (name, arrow) in [(SCROLL_UP, up), (SCROLL_DOWN, down)] {
                if let Some(picture) = paint.widget(name) {
                    // Centred in the arrow's square.
                    let gap = (arrow.size()
                        - Vec2::new(picture.width() as f32, picture.height() as f32))
                        / 2.0;
                    paste(frame, picture, (arrow.min + gap).floor());
                }
            }
            if let Some(long) = paint.widget(HANDLES[scheme(control).min(HANDLES.len() - 1)]) {
                let thumb = self.thumb(control);
                paste(frame, &combo::cut(long, thumb.height() as u32), thumb.min);
            }
        }
        let Some(font) = paint.font() else { return };
        // The rows stop short of the scrollbar's strip, scrolling or not,
        // unless the style lets them run under it (`FUN_007a7d9c`).
        let style = style(control);
        let beside = style & WIDE_ROWS == 0 && (style & BESIDE == 0 || style & WITH_TRACK != 0);
        let right = if beside { right - SCROLLBAR_WIDTH as i32 } else { right };
        // The rows are drawn on the list's own area, which cuts off the
        // last one where the list ends.
        let (width, height) = (r.width() as u32, r.height() as u32);
        let mut area = imageops::crop_imm(frame, x as u32, y as u32, width, height).to_image();
        let (left, right) = (0, right - x);
        let shown = self.rows.iter().skip(self.scroll).take(visible_rows(control));
        for (row, line) in shown.enumerate() {
            let frame = &mut area;
            let (x, top) = (left, 1 + (row as f32 * row_height(control)) as i32);
            let selected = self.selected == Some(self.scroll + row)
                || self.marked.get(self.scroll + row).copied().unwrap_or(false);
            let ink = self.inks.get(self.scroll + row).copied().flatten();
            // The bar first: a selection lies over it.
            if let Some(bar) = self.bars.get(self.scroll + row).copied().flatten() {
                fill(frame, x + bar.left, top, bar.width, BAR_HEIGHT, BAR_BACK);
                let filled = (bar.width + 1) * bar.value / 100;
                fill(frame, x + bar.left, top, filled, BAR_HEIGHT, bar.fill);
            }
            let color = if selected {
                let [r, g, b] =
                    self.selection.unwrap_or(SELECTION[scheme(control).min(SELECTION.len() - 1)]);
                let indent = if row_height(control) >= TALL_ROW { TALL_ROW_INDENT } else { 0 };
                let (x0, y0) = (x + indent, top - 1);
                for py in y0.max(0)..(y0 + font.height() as i32).min(frame.height() as i32) {
                    for px in x0.max(0)..right.min(frame.width() as i32) {
                        let p = frame.get_pixel_mut(px as u32, py as u32);
                        let mix = |a: u8, b: u8| ((u16::from(a) + u16::from(b)) / 2) as u8;
                        *p = Rgba([mix(p[0], r), mix(p[1], g), mix(p[2], b), 255]);
                    }
                }
                ink.map_or(TEXT, |[r, g, b]| Rgba([r, g, b, 255]))
            } else if paint.pointed(Target::Row { control: paint.index, row })
                || self.pointed_cell(paint, row)
            {
                TEXT_HOVER
            } else {
                ink.map_or(TEXT, |[r, g, b]| Rgba([r, g, b, 255]))
            };
            let cells = line.split('\t');
            // The text sits at the list's left edge, a pixel above the row.
            let at = (x, top - 1);
            columns(font, frame, at, cells, &self.tabs, &self.aligns, color, right);
            // So do the icons, the keyed ones without their pure black.
            for &(left, id) in self.row_icons.get(self.scroll + row).into_iter().flatten() {
                if let Some(icon) = paint.image(id) {
                    let at = Vec2::new(x as f32 + left, at.1 as f32);
                    if self.keyed.contains(&id) {
                        paste(frame, &crate::game::transparent_black(icon.clone()), at);
                    } else {
                        paste(frame, icon, at);
                    }
                }
            }
        }
        imageops::replace(frame, &area, x.into(), y.into());
    }
}

/// A list's style, as the dialog file gives it.
fn style(control: &Control) -> u8 {
    match control.kind {
        Kind::ListBox { style, .. } => style,
        _ => 0,
    }
}

/// Where a list's rows put their icons, slot by slot, from its left edge,
/// as the dialog file sets them.
pub fn icon_slots(control: &Control) -> Vec<f32> {
    match &control.kind {
        Kind::ListBox { icons, .. } => icons.clone(),
        _ => Vec::new(),
    }
}

/// The colour of its scrollbar.
fn scheme(control: &Control) -> usize {
    usize::from((style(control) & COLOUR) >> 4)
}

/// A row's column: from its start to the next one's.
fn cell(row: Rect, list: Rect, tabs: &[f32], column: usize) -> Rect {
    let at = tabs.get(column).copied().unwrap_or(0.0);
    let end = tabs.get(column + 1).map_or(row.max.x, |&next| list.min.x + next);
    Rect::new(list.min.x + at, row.min.y, end, row.max.y)
}

/// Where a row's column is, in classic pixels, by the columns the dialog
/// file gives the list.
pub fn cell_rect(control: &Control, row: usize, column: usize) -> Option<Rect> {
    let Kind::ListBox { columns, .. } = &control.kind else { return None };
    let tabs: Vec<f32> = columns.iter().map(|c| c.at).collect();
    let r = control.rect;
    let y = r.min.y + 1.0 + row as f32 * row_height(control);
    let line = Rect::new(r.min.x, y, r.max.x, (y + row_height(control)).min(r.max.y));
    Some(cell(line, r, &tabs, column))
}

/// How high a list's rows are.
pub fn row_height(control: &Control) -> f32 {
    match control.kind {
        Kind::ListBox { row_height, .. } if row_height > 0 => f32::from(row_height),
        _ => ROW_HEIGHT,
    }
}

/// The rows a list shows, and scrolls by: as many as its height holds, the
/// last of them perhaps cut off a pixel where the list ends (`CMyListBox`,
/// `FUN_007a9d22`).
pub fn visible_rows(control: &Control) -> usize {
    (control.rect.height() / row_height(control)).floor().max(0.0) as usize
}

/// Where a list's scrollbar puts its up and down arrows, in classic pixels.
pub fn scroll_arrows(control: &Control) -> (Rect, Rect) {
    let r = control.rect;
    let left = r.max.x - SCROLLBAR_WIDTH;
    let up = Rect::new(left, r.min.y, r.max.x, r.min.y + SCROLLBAR_WIDTH);
    let down = Rect::new(left, r.max.y - SCROLLBAR_WIDTH, r.max.x, r.max.y);
    (up, down)
}

/// Every row it has room for — or every column of each, in a list that
/// answers by column: the stretch from one column's start to the next —
/// and the arrows of its scrollbar on top.
pub(crate) fn targets(
    state: &ListState,
    index: usize,
    control: &Control,
    geometry: &dyn Geometry,
    out: &mut Vec<(Target, Rect)>,
) {
    let r = control.rect;
    for row in 0..visible_rows(control) {
        let y = r.min.y + 1.0 + row as f32 * row_height(control);
        let rect = Rect::new(r.min.x, y, r.max.x, (y + row_height(control)).min(r.max.y));
        if !state.by_column {
            out.push((Target::Row { control: index, row }, geometry.map(rect)));
            continue;
        }
        for column in 0..state.tabs.len() {
            let cell = cell(rect, r, &state.tabs, column);
            out.push((Target::Cell { control: index, row, column }, geometry.map(cell)));
        }
    }
    let (up, down) = geometry.scroll(control);
    out.push((Target::Scroll { control: index, up: true }, up));
    out.push((Target::Scroll { control: index, up: false }, down));
}
