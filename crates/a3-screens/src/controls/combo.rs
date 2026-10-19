//! Combo boxes, the game's `CMyComboBox`: a field that shows the chosen
//! text, its frame and the round arrow at its right end part of the
//! dialog's picture. A click drops the list down under it (its
//! `CMyComboOverlapped`): lavender rows of 16 pixels in a black frame, the
//! chosen one grey, a scrollbar along the right while there are more than
//! fit. The list lies over everything and takes the next click: on a row it
//! chooses, anywhere else it only closes.

use bevy_math::{Rect, Vec2};
use image::{Rgba, RgbaImage, imageops};

use a3_files::bitmap_font::BitmapFont;
use a3_files::rch::{Align, Control, Kind};

use crate::classic::{Paint, TEXT, columns, fill, paste};
use crate::controls::list::HANDLES;
use crate::pointer::{Geometry, Target};

/// The pictures the game loads for it by name: the list's arrows; the long
/// bars its handle is cut from are the lists' ([`HANDLES`]), by the colour
/// in its style's bits 4 to 6 (`FUN_0079b97a`).
pub(crate) const LIST_UP: &str = "Pfeile/PfeilO1.bmp";
pub(crate) const LIST_DOWN: &str = "Pfeile/PfeilU1.bmp";
const HANDLE_COLOUR: u8 = 0x70;
const HANDLE_SHIFT: u8 = 4;

const ROW: i32 = 16;
/// The list starts this far below the field's top; its window reaches as
/// far below the field as the combo box says.
const LIST_TOP: i32 = 20;
/// The screen the game sets, which a dropped list stays on.
const SCREEN_HEIGHT: f32 = 480.0;
/// Only the arrow at the field's right end drops the list
/// (`OnLButtonDown_007989d9`).
const ARROW: f32 = 20.0;
/// The scrollbar's strip and its arrows.
const BAR: i32 = 20;
/// Where the texts start: in the field, in a row.
const FIELD_TEXT: i32 = 2;
const ROW_TEXT: i32 = 2;
const LIST: Rgba<u8> = Rgba([220, 216, 225, 255]);
const STRIP: Rgba<u8> = Rgba([225, 216, 220, 255]);
const CHOSEN: Rgba<u8> = Rgba([176, 176, 176, 255]);
const EDGE: Rgba<u8> = Rgba([0, 0, 0, 255]);
/// What a handle keeps of its long bar's ends, and where the bar's lower
/// end starts.
const END: i32 = 3;
const LOWER_END: u32 = 0x1ba;
/// The shortest the list's handle gets (`FUN_0079bb01`).
const MIN_HANDLE: i32 = 7;

/// A row's text: in the columns the combo box has, if more than one, the
/// first `first` further in.
fn draw_text(
    font: &BitmapFont,
    frame: &mut RgbaImage,
    control: &Control,
    at: (i32, i32),
    first: i32,
    text: &str,
    right: i32,
) {
    match &control.kind {
        Kind::Combo { columns: c, .. } if c.len() > 1 => {
            let mut tabs: Vec<f32> = c.iter().map(|c| c.at).collect();
            tabs[0] += first as f32;
            let aligns: Vec<Option<(Align, f32)>> = c.iter().map(|c| c.align).collect();
            columns(font, frame, at, text.split('\t'), &tabs, &aligns, TEXT, right);
        }
        _ => font.draw(frame, at.0 + first, at.1, text, TEXT, false),
    }
}

/// A handle `length` long cut from a long upright bar: its upper end and
/// middle, then its lower end (`FUN_007ae11d` for a list's).
pub(crate) fn cut(long: &RgbaImage, length: u32) -> RgbaImage {
    let (width, end) = (long.width(), END as u32);
    let mut handle = RgbaImage::new(width, length);
    let part = |y: u32, h: u32| imageops::crop_imm(long, 0, y, width, h).to_image();
    imageops::replace(&mut handle, &part(0, length - end), 0, 0);
    imageops::replace(&mut handle, &part(LOWER_END, end), 0, i64::from(length - end));
    handle
}

#[derive(Clone, Default, Debug)]
pub struct ComboState {
    pub items: Vec<String>,
    pub selected: Option<usize>,
    /// Whether the list is down, and its first row shown.
    pub open: bool,
    pub scroll: usize,
    /// A disabled combo box does not drop its list.
    pub disabled: bool,
}

impl ComboState {
    /// How tall the dropped list is.
    fn height(control: &Control) -> i32 {
        let drop = match control.kind {
            Kind::Combo { drop, .. } => i32::from(drop),
            _ => 0,
        };
        drop + control.rect.height() as i32 - LIST_TOP
    }

    /// The dropped list, in classic pixels: below the field, or above it
    /// where the screen ends too soon, as Windows drops it.
    fn list(control: &Control) -> Rect {
        let r = control.rect;
        let height = Self::height(control) as f32;
        if Self::above(control) {
            return Rect::new(r.min.x, r.min.y - height, r.max.x, r.min.y);
        }
        let top = r.min.y + LIST_TOP as f32;
        Rect::new(r.min.x, top, r.max.x, top + height)
    }

    /// Whether the list drops above the field.
    fn above(control: &Control) -> bool {
        control.rect.min.y + (LIST_TOP + Self::height(control)) as f32 > SCREEN_HEIGHT
    }

    /// How many rows the list shows.
    fn rows(control: &Control) -> usize {
        (Self::height(control) / ROW).max(1) as usize
    }

    fn scrolls(&self, control: &Control) -> bool {
        self.items.len() > Self::rows(control)
    }

    /// Opens or closes the list, opened on the chosen row.
    pub fn toggle(&mut self, control: &Control) {
        self.open = !self.open && !self.disabled;
        if self.open {
            let last = self.items.len().saturating_sub(Self::rows(control));
            self.scroll = self.selected.unwrap_or(0).min(last);
        }
    }

    /// Chooses the `row`th row shown, and closes the list; whether there was
    /// one.
    pub fn press(&mut self, row: usize) -> bool {
        self.open = false;
        let item = self.scroll + row;
        if item < self.items.len() {
            self.selected = Some(item);
            true
        } else {
            false
        }
    }

    pub fn scroll_by(&mut self, rows: i32, control: &Control) {
        let last = self.items.len().saturating_sub(Self::rows(control));
        self.scroll = (self.scroll as i32 + rows).clamp(0, last as i32) as usize;
    }

    /// The field: the chosen text.
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let r = paint.control.rect;
        let (x, y) = (r.min.x as i32, r.min.y as i32);
        if let (Some(font), Some(text)) =
            (paint.font(), self.selected.and_then(|s| self.items.get(s)))
        {
            // Centred top to bottom in whole pixels.
            let top = y + (r.height() as i32 - font.height() as i32) / 2;
            let at = (x + FIELD_TEXT, top);
            draw_text(font, frame, paint.control, at, 0, text, r.max.x as i32);
        }
    }

    /// The dropped list, over everything else.
    pub(crate) fn draw_list(&self, frame: &mut RgbaImage, paint: &Paint) {
        if !self.open {
            return;
        }
        let control = paint.control;
        let l = Self::list(control);
        let (x0, y0, x1, y1) = (l.min.x as i32, l.min.y as i32, l.max.x as i32, l.max.y as i32);
        let (w, h) = (x1 - x0, y1 - y0);
        // The line above the bottom edge stays unpainted, and the edge's
        // lines end short of the bottom right corner, as the game draws
        // them (`FUN_0079a0b2`).
        fill(frame, x0, y0, w, h - 2, LIST);
        fill(frame, x0, y0, 1, h, EDGE);
        fill(frame, x1 - 1, y0, 1, h - 1, EDGE);
        fill(frame, x0, y1 - 1, w - 1, 1, EDGE);
        let scrolls = self.scrolls(control);
        let right = if scrolls { x1 - 1 - BAR } else { x1 - 1 };
        // The rows, clipped above that line, from a line below the top —
        // from the top in a list above the field.
        let first = if Self::above(control) { y0 } else { y0 + 1 };
        let mut rows =
            RgbaImage::from_pixel((right - x0 - 1).max(1) as u32, (y1 - 2 - first) as u32, LIST);
        let font = paint.font();
        for (i, text) in self.items.iter().enumerate().skip(self.scroll) {
            let top = ((i - self.scroll) as i32) * ROW;
            if top >= rows.height() as i32 {
                break;
            }
            if self.selected == Some(i) {
                let width = rows.width() as i32;
                fill(&mut rows, 0, top, width, ROW, CHOSEN);
            }
            if let Some(font) = font {
                // The columns count from the row's edge, the first a
                // pixel in.
                let right = rows.width() as i32;
                draw_text(font, &mut rows, control, (0, top), ROW_TEXT - 1, text, right);
            }
        }
        paste(frame, &rows, Vec2::new((x0 + 1) as f32, first as f32));
        if scrolls {
            let bar = right;
            fill(frame, bar, y0 + 1, BAR, h - 3 - BAR, STRIP);
            if let Some(up) = paint.widget(LIST_UP) {
                paste(frame, up, Vec2::new(bar as f32, (y0 + 1) as f32));
            }
            if let Some(down) = paint.widget(LIST_DOWN) {
                paste(frame, down, Vec2::new(bar as f32, (y1 - 1 - BAR) as f32));
            }
            if let Some(long) = paint.widget(handle_bar(control)) {
                // As long as the rows shown are of all, cut from the long
                // bar as a slider's handle is.
                let track = h - 2 * BAR;
                let rows = Self::rows(control) as i32;
                let length =
                    (track * rows / self.items.len().max(1) as i32).clamp(MIN_HANDLE, track);
                let handle = cut(long, length as u32);
                let last = self.items.len().saturating_sub(Self::rows(control)).max(1);
                let down = (track - length) * self.scroll as i32 / last as i32;
                paste(frame, &handle, Vec2::new(bar as f32, (y0 + 1 + BAR + down) as f32));
            }
        }
    }

    /// Closed, the field opens it. Open, its rows and arrows lie over the
    /// whole dialog, which closes it.
    pub(crate) fn targets(
        &self,
        index: usize,
        control: &Control,
        geometry: &dyn Geometry,
        out: &mut Vec<(Target, Rect)>,
    ) {
        if !self.open {
            let r = geometry.rect(control);
            let arrow = r.width() * ARROW / control.rect.width();
            let rect = Rect::new(r.max.x - arrow, r.min.y, r.max.x, r.max.y);
            out.push((Target::Combo(index), rect));
        }
    }

    /// The open list's targets, which go after every other control's.
    pub(crate) fn list_targets(
        &self,
        index: usize,
        control: &Control,
        geometry: &dyn Geometry,
        out: &mut Vec<(Target, Rect)>,
    ) {
        if !self.open {
            return;
        }
        let everywhere = Rect::new(-10_000.0, -10_000.0, 10_000.0, 10_000.0);
        out.push((Target::Combo(index), geometry.map(everywhere)));
        let l = Self::list(control);
        let scrolls = self.scrolls(control);
        let right = if scrolls { l.max.x - 1.0 - BAR as f32 } else { l.max.x - 1.0 };
        let shown = self.items.len().saturating_sub(self.scroll).min(Self::rows(control) + 1);
        let first = if Self::above(control) { l.min.y } else { l.min.y + 1.0 };
        for row in 0..shown {
            let top = first + (row as i32 * ROW) as f32;
            let bottom = (top + ROW as f32).min(l.max.y - 1.0);
            let r = Rect::new(l.min.x + 1.0, top, right, bottom);
            out.push((Target::Row { control: index, row }, geometry.map(r)));
        }
        if scrolls {
            let x = right;
            let up = Rect::new(x, l.min.y + 1.0, x + BAR as f32, l.min.y + 1.0 + BAR as f32);
            let down = Rect::new(x, l.max.y - 1.0 - BAR as f32, x + BAR as f32, l.max.y - 1.0);
            out.push((Target::Scroll { control: index, up: true }, geometry.map(up)));
            out.push((Target::Scroll { control: index, up: false }, geometry.map(down)));
        }
    }
}

/// The long bar a combo box's handle is cut from, by its colour.
fn handle_bar(control: &Control) -> &'static str {
    let colour = match control.kind {
        Kind::Combo { style, .. } => usize::from((style & HANDLE_COLOUR) >> HANDLE_SHIFT),
        _ => HANDLES.len() - 1,
    };
    HANDLES[colour.min(HANDLES.len() - 1)]
}
