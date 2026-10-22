//! The classic look: a dialog composed into one 640×480 frame, the way the
//! game blitted it, in MS Sans Serif drawn pixel for pixel. Drawing is a
//! function of the dialog, its view and the pictures loaded for it.

use std::collections::HashMap;
use std::sync::Arc;

use bevy_math::Vec2;
use image::{Rgba, RgbaImage, imageops};
use tracing::warn;

use a3_files::bitmap_font::BitmapFont;
use a3_files::rch::{Align, Control, Dialog, PLAIN_FONT};

use crate::controls::bitmap::Picture;
use crate::controls::{bar, check, combo, list, slider, spin};
use crate::fonts::Fonts;
use crate::game::{Game, transparent_black};
use crate::pointer::Target;
use crate::view::View;

/// The grey of Windows dialogs, behind a dialog without a picture.
pub const PLAIN: Rgba<u8> = Rgba([224, 222, 226, 255]);
pub(crate) const TEXT: Rgba<u8> = Rgba([0, 0, 0, 255]);
pub(crate) const TEXT_HOVER: Rgba<u8> = Rgba([176, 48, 0, 255]);
/// A popup's edge, in pixels: black, as text.
/// A dragged picture stays above this line, the bar's top less one.
const DRAG_BOTTOM: f32 = 447.0;
const POPUP_EDGE: i32 = 2;
/// The handle of sliders and scrollbars, and the arrows of list scrollbars,
/// which the game loads by name.
pub(crate) const SCROLL_UP: &str = "Pfeile/PfeilO1.bmp";
pub(crate) const SCROLL_DOWN: &str = "Pfeile/PfeilU1.bmp";
/// Screens that are a picture to act in rather than a form: the wait screen
/// and the offices.
pub const SCENES: [u16; 3] = [381, 483, 504];

/// Whether a dialog is a scene, whose objects change their picture only
/// while they are held down.
pub fn is_scene(id: u16) -> bool {
    SCENES.contains(&id)
}

/// A dialog's own picture, or the plain grey of Windows dialogs if it has
/// none that can be read.
pub fn art(game: &Game, dialog: &Dialog) -> RgbaImage {
    game.graphic(dialog.background).unwrap_or_else(|err| {
        warn!("background {}: {err}", dialog.background);
        RgbaImage::from_pixel(640, 480, PLAIN)
    })
}

/// A dialog's picture as the classic look shows it: a popup's cut to its
/// window, which may be smaller, with a black edge over its rim — the
/// window's rim where the picture is smaller (249's is two rows short).
pub fn picture(game: &Game, dialog: &Dialog) -> RgbaImage {
    let mut picture = art(game, dialog);
    if let Some(frame) = dialog.frame {
        let (w, h) = (frame.width() as u32, frame.height() as u32);
        if picture.width() != w || picture.height() != h {
            let part = imageops::crop_imm(&picture, 0, 0, w, h).to_image();
            picture = RgbaImage::from_pixel(w, h, TEXT);
            imageops::replace(&mut picture, &part, 0, 0);
        }
        let (w, h) = (picture.width() as i32, picture.height() as i32);
        fill(&mut picture, 0, 0, w, POPUP_EDGE, TEXT);
        fill(&mut picture, 0, h - POPUP_EDGE, w, POPUP_EDGE, TEXT);
        fill(&mut picture, 0, 0, POPUP_EDGE, h, TEXT);
        fill(&mut picture, w - POPUP_EDGE, 0, POPUP_EDGE, h, TEXT);
    }
    picture
}

/// A dialog in the classic look: its background and everything it draws
/// with, loaded as it opens.
pub struct Classic {
    background: RgbaImage,
    fonts: Arc<Fonts>,
    /// Button, option, animation and icon images by graphic id.
    images: HashMap<u16, RgbaImage>,
    /// Images the game loads by name: the slider handle, scrollbar arrows.
    widgets: HashMap<&'static str, RgbaImage>,
    /// What bitmaps show, loaded as they are first shown.
    bitmaps: HashMap<Picture, RgbaImage>,
}

impl Classic {
    /// Loads what the dialog's controls draw with as the view stands, on
    /// `background`: the dialog's picture, or for a popup the screen under
    /// it with its picture on top.
    pub fn new(game: &Game, dialog: &Dialog, view: &View, background: RgbaImage) -> Self {
        let fonts = game.fonts();
        let images = HashMap::new();
        let mut widgets = HashMap::new();
        let sliders = slider::HANDLES.into_iter().chain(slider::ARROWS).chain(slider::TRACKS);
        let lists = list::HANDLES.into_iter().chain(list::TRACKS);
        let named = [SCROLL_UP, SCROLL_DOWN, check::TICK, combo::FIELD_HELD]
            .into_iter()
            .chain(lists)
            .chain(sliders)
            .chain(spin::ARROWS);
        for name in named {
            match game.picture(name) {
                // A TGA brings its own alpha, shadows in black.
                Ok(picture) if name.ends_with(".tga") => _ = widgets.insert(name, picture),
                Ok(picture) => _ = widgets.insert(name, transparent_black(picture)),
                Err(err) => warn!("{name}: {err}"),
            }
        }
        let mut classic = Self { background, fonts, images, widgets, bitmaps: HashMap::new() };
        classic.update_with(game, dialog, view);
        classic
    }

    /// Loads what the controls and the bitmaps show that has not been
    /// loaded yet.
    pub fn update_with(&mut self, game: &Game, dialog: &Dialog, view: &View) {
        let dragged = view.drag_pictures.iter().map(|&(_, id)| (vec![id], false));
        let states = dialog.controls.iter().zip(&view.controls).map(|(c, s)| s.state.graphics(c));
        for (ids, sprite) in states.chain(dragged) {
            for id in ids {
                if self.images.contains_key(&id) {
                    continue;
                }
                let picture = if sprite { game.sprite(id) } else { game.graphic(id) };
                match picture {
                    Ok(picture) => _ = self.images.insert(id, picture),
                    Err(err) => warn!("graphic {id}: {err}"),
                }
            }
        }
        self.update(game, view);
    }

    /// Loads what the bitmaps show that has not been loaded yet.
    pub fn update(&mut self, game: &Game, view: &View) {
        let pictures: Vec<&Picture> =
            view.controls.iter().filter_map(|c| c.state.as_bitmap()?.picture.as_ref()).collect();
        // What no bitmap shows any more goes (the grounds draw a map with
        // every move).
        self.bitmaps.retain(|p, _| pictures.contains(&p));
        for picture in pictures {
            if !self.bitmaps.contains_key(picture) {
                let loaded = picture.load(game).unwrap_or_else(|err| {
                    warn!("{picture:?}: {err}");
                    RgbaImage::new(1, 1)
                });
                self.bitmaps.insert(picture.clone(), loaded);
            }
        }
    }

    /// The frame as the view stands: with the caret of the focused field
    /// shown or not, and playing animations `phase` of the way to their next
    /// frame.
    pub fn draw(&self, dialog: &Dialog, view: &View, caret: bool, phase: f32) -> RgbaImage {
        let mut frame = self.background.clone();
        let lines = |frame: &mut RgbaImage| {
            for &([x0, y0, x1, y1], [r, g, b]) in &view.lines {
                line(frame, (x0, y0), (x1, y1), Rgba([r, g, b, 255]));
            }
        };
        for &([x, y, w, h], alpha) in &view.shades {
            darken(&mut frame, x, y, w, h, alpha);
        }
        if !view.lines_on_top {
            lines(&mut frame);
        }
        let scene = is_scene(view.id);
        for (index, (control, shown)) in dialog.controls.iter().zip(&view.controls).enumerate() {
            if shown.hidden {
                continue;
            }
            let paint = Paint { classic: self, view, index, control, caret, phase, scene };
            shown.state.draw(&mut frame, &paint);
        }
        if view.lines_on_top {
            lines(&mut frame);
        }
        // The player held by the pointer goes with it.
        if let Some((size, dragged)) = view.dragged_box(dialog) {
            let control = &dialog.controls[size];
            let paint = Paint { classic: self, view, index: size, control, caret, phase, scene };
            dragged.draw(&mut frame, &paint);
        }
        // So does a control dragged as a picture, its text on it, cut to
        // the control's size, kept above the bar (`FUN_006693b7`).
        if let Some((picture, c, at)) = view.dragged_picture(dialog)
            && let Some(image) = self.images.get(&picture)
        {
            let control = &dialog.controls[c];
            let (w, h) = (image.width() as f32, image.height() as f32);
            let at = Vec2::new(at.x.clamp(0.0, 640.0 - w), at.y.clamp(0.0, DRAG_BOTTOM - h));
            let size = control.rect.size();
            let cut = imageops::crop_imm(image, 0, 0, size.x as u32, size.y as u32).to_image();
            paste(&mut frame, &cut, at.floor());
            if let (Some(font), Some(text)) =
                (self.fonts.get(PLAIN_FONT), view.controls[c].state.text())
            {
                let left = centred(at.x as i32, size.x as i32, font.width(text, false) as i32);
                font.draw(&mut frame, left, at.y as i32, text, TEXT, false);
            }
        }
        // What does nothing yet is crossed out.
        if !view.inert.is_empty() {
            for (target, at) in crate::pointer::targets(dialog, view, &crate::pointer::Pixels) {
                if view.inert.contains(&target) {
                    cross(&mut frame, at);
                }
            }
        }
        // A dropped list lies over everything.
        for (index, (control, shown)) in dialog.controls.iter().zip(&view.controls).enumerate() {
            if let (false, Some(combo)) = (shown.hidden, shown.state.as_combo()) {
                let paint = Paint { classic: self, view, index, control, caret, phase, scene };
                combo.draw_list(&mut frame, &paint);
            }
        }
        if let (Some((control, item)), Some(font)) = (view.tip, self.fonts.get(PLAIN_FONT)) {
            let text = view.controls.get(control).and_then(|c| c.state.as_bar()?.tips.get(item));
            if let (Some(text), Some(c)) = (text, dialog.controls.get(control)) {
                bar::draw_tip(&mut frame, font, bar::symbol(c, item), text);
            }
        }
        frame
    }
}

/// What a control draws with and in which state: the dialog's pictures, the
/// view, and the control itself.
pub(crate) struct Paint<'a> {
    classic: &'a Classic,
    pub view: &'a View,
    pub index: usize,
    pub control: &'a Control,
    pub caret: bool,
    pub phase: f32,
    pub scene: bool,
}

impl Paint<'_> {
    /// The dialog font.
    pub fn font(&self) -> Option<&BitmapFont> {
        self.classic.fonts.get(PLAIN_FONT)
    }

    /// Font `number` of the dialog file.
    pub fn font_of(&self, number: u8) -> Option<&BitmapFont> {
        self.classic.fonts.get(number)
    }

    pub fn image(&self, id: u16) -> Option<&RgbaImage> {
        self.classic.images.get(&id)
    }

    pub fn widget(&self, name: &str) -> Option<&RgbaImage> {
        self.classic.widgets.get(name)
    }

    pub fn bitmap(&self, picture: &Picture) -> Option<&RgbaImage> {
        self.classic.bitmaps.get(picture)
    }

    /// Whether the pointer is on `target`.
    pub fn pointed(&self, target: Target) -> bool {
        self.view.hovered == Some(target)
    }
}

/// A red cross from corner to corner of a rectangle, three pixels thick.
fn cross(frame: &mut RgbaImage, at: bevy_math::Rect) {
    const INK: Rgba<u8> = Rgba([220, 20, 20, 255]);
    const HALF_WIDTH: f32 = 1.5;
    let (w, h) = (at.width(), at.height());
    if w < 1.0 || h < 1.0 {
        return;
    }
    let length = (w * w + h * h).sqrt();
    let (x0, y0) = (at.min.x.max(0.0) as u32, at.min.y.max(0.0) as u32);
    let (x1, y1) = ((at.max.x as u32).min(frame.width()), (at.max.y as u32).min(frame.height()));
    for y in y0..y1 {
        for x in x0..x1 {
            let (dx, dy) = (x as f32 + 0.5 - at.min.x, y as f32 + 0.5 - at.min.y);
            // Distances to the two diagonals.
            let down = (dx * h - dy * w).abs() / length;
            let up = (dx * h + dy * w - w * h).abs() / length;
            if down.min(up) <= HALF_WIDTH {
                frame.put_pixel(x, y, INK);
            }
        }
    }
}

/// Where a text `text` pixels wide starts centred in `width` from `left`,
/// as the game centres it: on the middle of the room, half the text to the
/// left of it, each half rounded down (`FUN_0079d255`, `sgl_DrawText`).
pub(crate) fn centred(left: i32, width: i32, text: i32) -> i32 {
    left + width / 2 - (text >> 1)
}

pub(crate) fn paste(frame: &mut RgbaImage, picture: &RgbaImage, at: Vec2) {
    imageops::overlay(frame, picture, at.x as i64, at.y as i64);
}

/// A line from one point towards another, as the game's Direct3D renderer
/// has them rasterised (`sgl_DrawLine`; seen under Wine): every column — or
/// row, for a steep line — whose middle the line spans, and in it the pixel
/// the line passes through, with the line moved half a pixel down and, for
/// a flat line, 63/128 of a pixel across, for a steep one half a pixel.
/// So a flat line keeps its left end and a steep one its top end; where a
/// steep line runs between two pixels it takes the right one.
pub(crate) fn line(frame: &mut RgbaImage, from: (i32, i32), to: (i32, i32), color: Rgba<u8>) {
    // In 256ths of a pixel.
    const UNIT: i64 = 256;
    const FLAT_ACROSS: i64 = 126;
    const STEEP_ACROSS: i64 = 128;
    const DOWN: i64 = 128;
    let flat = (to.0 - from.0).abs() >= (to.1 - from.1).abs();
    let across = if flat { FLAT_ACROSS } else { STEEP_ACROSS };
    let (x0, y0) = (i64::from(from.0) * UNIT + across, i64::from(from.1) * UNIT + DOWN);
    let (x1, y1) = (i64::from(to.0) * UNIT + across, i64::from(to.1) * UNIT + DOWN);
    let (dx, dy) = (x1 - x0, y1 - y0);
    let mut put = |x: i64, y: i64| {
        if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y))
            && x < frame.width()
            && y < frame.height()
        {
            frame.put_pixel(x, y, color);
        }
    };
    // Along the longer way a pixel where its middle lies within the line;
    // across, the one the line passes through there.
    let (major, minor, d_major, d_minor) =
        if flat { ((x0, x1), y0, dx, dy) } else { ((y0, y1), x0, dy, dx) };
    if d_major == 0 {
        return;
    }
    let (lo, hi) = (major.0.min(major.1), major.0.max(major.1));
    for step in lo.div_euclid(UNIT) - 1..=hi.div_euclid(UNIT) + 1 {
        let middle = step * UNIT + UNIT / 2;
        if !(lo <= middle && middle < hi) {
            continue;
        }
        let (mut num, mut den) = (minor * d_major + (middle - major.0) * d_minor, UNIT * d_major);
        if den < 0 {
            (num, den) = (-num, -den);
        }
        let across = num.div_euclid(den);
        if flat { put(step, across) } else { put(across, step) }
    }
}

/// A rectangle darkened, as black laid over it with an alpha.
fn darken(frame: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, alpha: u8) {
    let keep = 255 - u32::from(alpha);
    let (x0, y0) = (x.max(0) as u32, y.max(0) as u32);
    let x1 = (x + w).clamp(0, frame.width() as i32) as u32;
    let y1 = (y + h).clamp(0, frame.height() as i32) as u32;
    for py in y0..y1 {
        for px in x0..x1 {
            let p = frame.get_pixel_mut(px, py);
            for c in &mut p.0[..3] {
                *c = (u32::from(*c) * keep / 255) as u8;
            }
        }
    }
}

pub(crate) fn fill(frame: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, color: Rgba<u8>) {
    for py in y.max(0)..(y + h).min(frame.height() as i32) {
        for px in x.max(0)..(x + w).min(frame.width() as i32) {
            frame.put_pixel(px as u32, py as u32, color);
        }
    }
}

/// Tab-separated cells at the given column starts, clipped to the next
/// column — or to the left, centred or to the right in a width of their
/// own.
#[allow(clippy::too_many_arguments)]
pub(crate) fn columns<'a>(
    font: &BitmapFont,
    frame: &mut RgbaImage,
    (x, y): (i32, i32),
    cells: impl Iterator<Item = &'a str>,
    tabs: &[f32],
    aligns: &[Option<(Align, f32)>],
    color: Rgba<u8>,
    right: i32,
) {
    for (i, cell) in cells.enumerate() {
        let Some(&tab) = tabs.get(i) else { break };
        let left = x + tab as i32;
        let spare = |width: f32| width as i32 - font.width(cell, false) as i32;
        // A column placed without a width runs to the next one's start
        // (`FUN_007a756f`).
        let to_next = tabs.get(i + 1).map_or((right - left) as f32, |&next| next - tab);
        let align =
            aligns.get(i).copied().flatten().map(|(a, w)| (a, if w == 0.0 { to_next } else { w }));
        match align {
            Some((Align::Centre, width)) => {
                let at = centred(left, width as i32, font.width(cell, false) as i32);
                font.draw(frame, at, y, cell, color, false)
            }
            Some((Align::Right, width)) => {
                font.draw(frame, left + spare(width), y, cell, color, false)
            }
            Some((Align::Left, width)) => {
                let clip = (left + width as i32).min(right);
                font.draw_clipped(frame, left, y, cell, color, false, clip);
            }
            _ => {
                let clip = tabs.get(i + 1).map_or(right, |&next| x + next as i32 - 4);
                font.draw_clipped(frame, left, y, cell, color, false, clip.min(right));
            }
        }
    }
}
