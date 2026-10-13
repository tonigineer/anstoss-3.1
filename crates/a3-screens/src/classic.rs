//! The classic look: a dialog composed into one 640×480 frame, the way the
//! game blitted it, in MS Sans Serif drawn pixel for pixel. Drawing is a
//! function of the dialog, its view and the pictures loaded for it.

use std::collections::HashMap;
use std::path::Path;

use bevy_math::Vec2;
use image::{Rgba, RgbaImage, imageops};
use tracing::warn;

use a3_files::bitmap_font::BitmapFont;
use a3_files::rch::{Align, Control, Dialog};

use crate::controls::bitmap::Picture;
use crate::controls::{bar, combo, slider};
use crate::game::{Game, transparent_black};
use crate::pointer::Target;
use crate::view::View;

/// Wine's MS Sans Serif for 96 dpi and code page 1252.
const MS_SANS_SERIF: &str = "/usr/share/wine/fonts/sserife.fon";
/// 8 pt at 96 dpi, the size of Windows dialogs.
const CELL_HEIGHT: u32 = 13;
/// The grey of Windows dialogs, behind a dialog without a picture.
pub const PLAIN: Rgba<u8> = Rgba([224, 222, 226, 255]);
pub(crate) const TEXT: Rgba<u8> = Rgba([0, 0, 0, 255]);
pub(crate) const TEXT_HOVER: Rgba<u8> = Rgba([176, 48, 0, 255]);
/// A popup's edge, in pixels: black, as text.
const POPUP_EDGE: i32 = 2;
/// The handle of sliders and scrollbars, and the arrows of list scrollbars,
/// which the game loads by name.
pub(crate) const HANDLE: &str = "ScrollerBlau.bmp";
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
pub fn picture(game: &Game, dialog: &Dialog) -> RgbaImage {
    let mut picture = game.graphic(dialog.background).unwrap_or_else(|err| {
        warn!("background {}: {err}", dialog.background);
        RgbaImage::from_pixel(640, 480, PLAIN)
    });
    // A popup has a black edge over the rim of its picture.
    if dialog.frame.is_some() {
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
    font: Option<BitmapFont>,
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
        let font = BitmapFont::load(Path::new(MS_SANS_SERIF), CELL_HEIGHT)
            .inspect_err(|err| warn!("{MS_SANS_SERIF}: {err}; the classic look has no text"))
            .ok();
        let mut images = HashMap::new();
        for (control, shown) in dialog.controls.iter().zip(&view.controls) {
            let (ids, sprite) = shown.state.graphics(control);
            for id in ids {
                let picture = if sprite { game.sprite(id) } else { game.graphic(id) };
                match picture {
                    Ok(picture) => _ = images.insert(id, picture),
                    Err(err) => warn!("graphic {id}: {err}"),
                }
            }
        }
        let mut widgets = HashMap::new();
        let combo = [combo::LIST_HANDLE];
        let sliders = slider::HANDLES.into_iter().chain(slider::ARROWS);
        for name in [HANDLE, SCROLL_UP, SCROLL_DOWN].into_iter().chain(combo).chain(sliders) {
            match game.picture(name) {
                Ok(picture) => _ = widgets.insert(name, transparent_black(picture)),
                Err(err) => warn!("{name}: {err}"),
            }
        }
        let mut classic = Self { background, font, images, widgets, bitmaps: HashMap::new() };
        classic.update(game, view);
        classic
    }

    /// Loads what the bitmaps show that has not been loaded yet.
    pub fn update(&mut self, game: &Game, view: &View) {
        let pictures = view.controls.iter().filter_map(|c| c.state.as_bitmap()?.picture.as_ref());
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
        let scene = is_scene(view.id);
        for (index, (control, shown)) in dialog.controls.iter().zip(&view.controls).enumerate() {
            if shown.hidden {
                continue;
            }
            let paint = Paint { classic: self, view, index, control, caret, phase, scene };
            shown.state.draw(&mut frame, &paint);
        }
        // The player held by the pointer goes with it.
        if let Some((size, dragged)) = view.dragged_box(dialog) {
            let control = &dialog.controls[size];
            let paint = Paint { classic: self, view, index: size, control, caret, phase, scene };
            dragged.draw(&mut frame, &paint);
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
        if let (Some((control, item)), Some(font)) = (view.tip, &self.font) {
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
    pub fn font(&self) -> Option<&BitmapFont> {
        self.classic.font.as_ref()
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

pub(crate) fn paste(frame: &mut RgbaImage, picture: &RgbaImage, at: Vec2) {
    imageops::overlay(frame, picture, at.x as i64, at.y as i64);
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
        match aligns.get(i).copied().flatten() {
            Some((Align::Centre, width)) => {
                font.draw(frame, left + spare(width) / 2, y, cell, color, false)
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
