//! The classic look: a dialog composed into one 640×480 frame, the way the
//! game blitted it, in MS Sans Serif drawn pixel for pixel.

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;
use image::imageops;
use image::{Rgba, RgbaImage};

use super::{
    Caret, Frames, LABEL_INSET, Picture, PlayerBox, ROW_HEIGHT, SCROLLBAR_WIDTH, Target, View,
    bullet_centre, hub, scroll_arrows, scroll_thumb, scrolls, slider_handle, visible_rows,
};
use crate::bitmap_font::BitmapFont;
use crate::game::{Game, transparent_black};
use crate::look::{SharpMaterial, show_frame, smooth_image, spawn_classic_layer};
use crate::rch::{Align, Control, Dialog, Kind};

/// Wine's MS Sans Serif for 96 dpi and code page 1252.
const MS_SANS_SERIF: &str = "/usr/share/wine/fonts/sserife.fon";
/// 8 pt at 96 dpi, the size of Windows dialogs.
const CELL_HEIGHT: u32 = 13;
const TEXT: Rgba<u8> = Rgba([0, 0, 0, 255]);
const TEXT_HOVER: Rgba<u8> = Rgba([176, 48, 0, 255]);
const SELECTED_TEXT: Rgba<u8> = Rgba([255, 255, 255, 255]);
const SELECTED_ROW: Rgba<u8> = Rgba([0, 0, 0, 255]);
/// The dot marking a chosen radio option.
const DOT_COLOR: Rgba<u8> = Rgba([100, 100, 100, 255]);
/// Blending animations is drawn in this many steps per frame — 40 a second —
/// which is smooth enough while composing the screen stays cheap.
const BLEND_STEPS: f32 = 8.0;
/// A player's box: an empty one centres the position's name on this height;
/// a filled one shows the position, and the strength ending at the right,
/// on a line below the name, and the abilities, one every few pixels, from
/// the left.
const EMPTY_BOX_HEIGHT: i32 = 26;
const BOX_DETAIL_TOP: i32 = 12;
const BOX_POSITION: i32 = 59;
const BOX_STRENGTH_RIGHT: i32 = 98;
const BOX_ICON_LEFT: i32 = 2;
const BOX_ICON_STEP: i32 = 14;
const BOX_ICON_TOP: i32 = 11;
const BOX_INK: Rgba<u8> = Rgba([255, 255, 255, 255]);
/// Tooltips: their pale yellow, the room around their text, and how far
/// above what they explain.
const TIP_BACK: Rgba<u8> = Rgba([255, 255, 225, 255]);
const TIP_PADDING: i32 = 2;
const TIP_GAP: i32 = 2;
/// The handle of sliders and scrollbars, and the arrows of list scrollbars.
const HANDLE: &str = "ScrollerBlau.bmp";
const SCROLL_UP: &str = "Pfeile/PfeilO1.bmp";
const SCROLL_DOWN: &str = "Pfeile/PfeilU1.bmp";

#[derive(Resource)]
pub(super) struct ClassicDialog {
    id: u16,
    background: RgbaImage,
    font: Option<BitmapFont>,
    /// Button, option and animation images by graphic id.
    images: HashMap<u16, RgbaImage>,
    /// Images the game loads by name: the slider handle, scrollbar arrows.
    widgets: HashMap<&'static str, RgbaImage>,
    /// What bitmaps show, loaded as they are first shown.
    bitmaps: HashMap<Picture, RgbaImage>,
    screen: Handle<Image>,
    material: Handle<SharpMaterial>,
    /// The blending step last drawn.
    step: u32,
}

impl ClassicDialog {
    pub(super) fn id(&self) -> u16 {
        self.id
    }

    /// The screen as it was last drawn.
    pub(super) fn last_frame(&self, images: &Assets<Image>) -> Option<RgbaImage> {
        let image = images.get(&self.screen)?;
        let size = image.size();
        RgbaImage::from_raw(size.x, size.y, image.data.clone()?)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn(
    commands: &mut Commands,
    stage: Entity,
    dialog: &Dialog,
    view: &View,
    background: RgbaImage,
    game: &Game,
    images: &mut Assets<Image>,
    materials: &mut Assets<SharpMaterial>,
) {
    let font = BitmapFont::load(Path::new(MS_SANS_SERIF), CELL_HEIGHT)
        .inspect_err(|err| warn!("{MS_SANS_SERIF}: {err}; the classic look has no text"))
        .ok();
    let mut pictures = HashMap::new();
    for (control, state) in dialog.controls.iter().zip(&view.controls) {
        let (ids, sprite) = match control.kind {
            Kind::Button { normal, hover } => (vec![normal, hover], true),
            Kind::Options { image, .. } if image != 0 => (vec![image], false),
            Kind::Anim => (state.frames.clone(), true),
            Kind::Bar { .. } => (hub::BAR_IMAGES.to_vec(), false),
            Kind::Player => {
                let icons = state.player.iter().flat_map(|p| p.icons.iter().copied());
                ([hub::BOX_FILLED, hub::BOX_EMPTY].into_iter().chain(icons).collect(), false)
            }
            Kind::ListBox => (state.row_icons.iter().flatten().map(|&(_, id)| id).collect(), false),
            _ => continue,
        };
        for id in ids {
            let picture = if sprite { game.sprite(id) } else { game.graphic(id) };
            match picture {
                Ok(picture) => _ = pictures.insert(id, picture),
                Err(err) => warn!("graphic {id}: {err}"),
            }
        }
    }
    let mut widgets = HashMap::new();
    for (name, sprite) in [(HANDLE, true), (SCROLL_UP, true), (SCROLL_DOWN, true)] {
        match game.picture(name) {
            Ok(picture) => {
                let picture = if sprite { transparent_black(picture) } else { picture };
                widgets.insert(name, picture);
            }
            Err(err) => warn!("{name}: {err}"),
        }
    }
    let mut classic = ClassicDialog {
        id: view.id,
        background,
        font,
        images: pictures,
        widgets,
        bitmaps: HashMap::new(),
        screen: Handle::default(),
        material: Handle::default(),
        step: 0,
    };
    load_bitmaps(&mut classic, view, game);
    let frame = compose(&classic, dialog, view, false, 0.0);
    classic.screen = images.add(smooth_image(frame));
    classic.material = spawn_classic_layer(commands, stage, materials, classic.screen.clone());
    commands.insert_resource(classic);
}

pub(super) fn redraw(
    classic: Option<ResMut<ClassicDialog>>,
    view: Res<View>,
    caret: Res<Caret>,
    frames: Res<Frames>,
    game: Res<Game>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SharpMaterial>>,
) {
    let Some(mut classic) = classic.filter(|c| c.id == view.id) else { return };
    if view.is_changed() {
        load_bitmaps(&mut classic, &view, &game);
    }
    let step = (frames.phase() * BLEND_STEPS) as u32;
    let blended = frames.is_changed() && step != classic.step;
    if !(view.is_changed() || caret.is_changed() || blended) {
        return;
    }
    classic.step = step;
    let Some(dialog) = game.dialogs.get(&view.id) else { return };
    let frame = compose(&classic, dialog, &view, caret.on, step as f32 / BLEND_STEPS);
    show_frame(&mut images, &mut materials, &classic.screen, &classic.material, frame);
}

/// Loads what the bitmaps show that has not been loaded yet.
fn load_bitmaps(classic: &mut ClassicDialog, view: &View, game: &Game) {
    for picture in view.controls.iter().filter_map(|c| c.picture.as_ref()) {
        if !classic.bitmaps.contains_key(picture) {
            let loaded = picture.load(game).unwrap_or_else(|err| {
                warn!("{picture:?}: {err}");
                RgbaImage::new(1, 1)
            });
            classic.bitmaps.insert(picture.clone(), loaded);
        }
    }
}

fn compose(
    classic: &ClassicDialog,
    dialog: &Dialog,
    view: &View,
    caret: bool,
    phase: f32,
) -> RgbaImage {
    let mut frame = classic.background.clone();
    let scene = super::backdrop::is_scene(view.id);
    for (index, (control, state)) in dialog.controls.iter().zip(&view.controls).enumerate() {
        if state.hidden {
            continue;
        }
        let r = control.rect;
        let (x, y, right) = (r.min.x as i32, r.min.y as i32, r.max.x as i32);
        let paste = |frame: &mut RgbaImage, picture: &RgbaImage, at: Vec2| {
            imageops::overlay(frame, picture, at.x as i64, at.y as i64);
        };
        match control.kind {
            Kind::Button { normal, hover } => {
                let target = Target::Button(index);
                let pointed = view.hovered == Some(target) && !state.disabled;
                // The objects of a scene change only while they are held down.
                let pointed = pointed && (!scene || view.pressed == Some(target));
                if let Some(sprite) = classic.images.get(if pointed { &hover } else { &normal }) {
                    paste(&mut frame, sprite, r.min);
                }
                continue;
            }
            Kind::Bitmap => {
                if let Some(picture) = state.picture.as_ref().and_then(|p| classic.bitmaps.get(p)) {
                    paste(&mut frame, picture, r.min);
                }
                continue;
            }
            Kind::Anim => {
                let (shown, next, blend) = state.showing(phase);
                let picture = |i: usize| state.frames.get(i).and_then(|id| classic.images.get(id));
                if let (Some(shown), Some(next)) = (picture(shown), picture(next)) {
                    blend_frames(&mut frame, shown, next, r.min, blend);
                }
                continue;
            }
            Kind::Slider => {
                if let Some(handle) = classic.widgets.get(HANDLE) {
                    paste(&mut frame, handle, slider_handle(control, state).min);
                }
                continue;
            }
            Kind::Bar { ref items } => {
                draw_bar(&mut frame, classic, control, items, state, view, index);
                continue;
            }
            Kind::ListBox if scrolls(control, state) => {
                let (up, down) = scroll_arrows(control);
                for (name, arrow) in [(SCROLL_UP, up), (SCROLL_DOWN, down)] {
                    if let Some(picture) = classic.widgets.get(name) {
                        // Centred in the arrow's square.
                        let gap = (arrow.size()
                            - Vec2::new(picture.width() as f32, picture.height() as f32))
                            / 2.0;
                        paste(&mut frame, picture, (arrow.min + gap).floor());
                    }
                }
                if let Some(handle) = classic.widgets.get(HANDLE) {
                    paste(&mut frame, handle, scroll_thumb(control, state).min);
                }
            }
            _ => {}
        }
        if let Kind::Options { image, items } = &control.kind {
            for (item, it) in items.iter().enumerate() {
                // The chosen option shows lit; the game's buttons of this
                // kind do not light up when pointed at.
                let lit = !state.actions && state.chosen == item;
                if !lit || !state.item_on(item) {
                    continue;
                }
                if *image == 0 {
                    dot(&mut frame, bullet_centre(control, item));
                } else if let Some(strip) = classic.images.get(image) {
                    // The chosen option shows its part of the button strip.
                    let (ix, iy) = (it.rect.min.x as u32, it.rect.min.y as u32);
                    let w = (it.rect.width() as u32).min(strip.width().saturating_sub(ix));
                    let h = (it.rect.height() as u32).min(strip.height().saturating_sub(iy));
                    let part = imageops::crop_imm(strip, ix, iy, w, h).to_image();
                    imageops::overlay(
                        &mut frame,
                        &part,
                        (x + ix as i32).into(),
                        (y + iy as i32).into(),
                    );
                }
            }
        }
        let Some(font) = &classic.font else { continue };
        let cell = font.height() as f32;
        match &control.kind {
            Kind::Static => {
                let pointed = view.hovered == Some(Target::Control(index));
                let color = if pointed { TEXT_HOVER } else { TEXT };
                draw_static(font, &mut frame, control, &state.text, color);
            }
            Kind::Header => {
                let titles = state.text.split("\\t");
                columns(font, &mut frame, (x, y), titles, &state.tabs, &state.aligns, TEXT, right);
            }
            Kind::ListBox => {
                let right =
                    if scrolls(control, state) { right - SCROLLBAR_WIDTH as i32 } else { right };
                let shown = state.rows.iter().skip(state.scroll).take(visible_rows(control));
                for (row, line) in shown.enumerate() {
                    let top = y + 1 + (row as f32 * ROW_HEIGHT) as i32;
                    let selected = state.selected == Some(state.scroll + row);
                    let ink = state.inks.get(state.scroll + row).copied().flatten();
                    let color = if selected {
                        fill(&mut frame, x, top, right - x, ROW_HEIGHT as i32, SELECTED_ROW);
                        SELECTED_TEXT
                    } else if view.hovered == Some(Target::Row { control: index, row }) {
                        TEXT_HOVER
                    } else {
                        ink.map_or(TEXT, |[r, g, b]| Rgba([r, g, b, 255]))
                    };
                    let cells = line.split('\t');
                    let at = (x + 2, top);
                    columns(font, &mut frame, at, cells, &state.tabs, &state.aligns, color, right);
                    for &(left, id) in state.row_icons.get(state.scroll + row).into_iter().flatten()
                    {
                        if let Some(icon) = classic.images.get(&id) {
                            paste(&mut frame, icon, Vec2::new(x as f32 + left, top as f32));
                        }
                    }
                }
            }
            Kind::Player => {
                if let Some(player) = &state.player {
                    draw_player(&mut frame, classic, font, r, player);
                }
            }
            Kind::Edit { .. } => {
                let top = y + ((r.height() - cell) / 2.0).round() as i32;
                font.draw_clipped(&mut frame, x + 2, top, &state.text, TEXT, false, right - 1);
                if caret && view.focus == Some(index) {
                    let at = x + 2 + font.width(&state.text, false) as i32;
                    fill(&mut frame, at.min(right - 2), top, 1, cell as i32, TEXT);
                }
            }
            Kind::Options { image, items } => {
                for (item, (it, label)) in items.iter().zip(&state.labels).enumerate() {
                    let pointed = view.hovered == Some(Target::Option { control: index, item });
                    let color = if pointed { TEXT_HOVER } else { TEXT };
                    if *image == 0 {
                        // Windows centres a radio button's label on its bullet.
                        let top = (bullet_centre(control, item).y - cell / 2.0).round() as i32;
                        let left = x + (it.rect.min.x + LABEL_INSET) as i32;
                        font.draw(&mut frame, left, top, label, color, false);
                    } else {
                        // A label may break into lines where it says so.
                        let lines: Vec<&str> = label.split('\n').collect();
                        let height = cell * lines.len() as f32;
                        let top = r.min.y + it.rect.min.y + (it.rect.height() - height) / 2.0;
                        for (i, line) in lines.iter().enumerate() {
                            let width = font.width(line, false) as f32;
                            let left = r.min.x + it.rect.min.x + (it.rect.width() - width) / 2.0;
                            let top = (top + cell * i as f32).round() as i32;
                            font.draw(&mut frame, left.round() as i32, top, line, color, false);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if let (Some((control, item)), Some(font)) = (view.tip, &classic.font) {
        let text = view.controls.get(control).and_then(|c| c.tips.get(item));
        if let (Some(text), Some(c)) = (text, dialog.controls.get(control)) {
            draw_tip(&mut frame, font, super::layout::bar_symbol(c, item), text);
        }
    }
    frame
}

/// A tooltip above a symbol, as Windows drew them: black on pale yellow in a
/// thin black frame.
fn draw_tip(frame: &mut RgbaImage, font: &BitmapFont, symbol: Rect, text: &str) {
    let width = font.width(text, false) as i32 + 2 * TIP_PADDING + 2;
    let height = font.height() as i32 + 2;
    let left = (symbol.min.x as i32).min(frame.width() as i32 - width).max(0);
    let top = symbol.min.y as i32 - height - TIP_GAP;
    fill(frame, left, top, width, height, TEXT);
    fill(frame, left + 1, top + 1, width - 2, height - 2, TIP_BACK);
    font.draw(frame, left + 1 + TIP_PADDING, top + 1, text, TEXT, false);
}

/// A static control: aligned within its rectangle, wrapped if it has room for
/// more than one line.
fn draw_static(
    font: &BitmapFont,
    frame: &mut RgbaImage,
    control: &Control,
    text: &str,
    color: Rgba<u8>,
) {
    let r = control.rect;
    let (x, y) = (r.min.x as i32, r.min.y as i32);
    if r.height() >= 2.0 * font.height() as f32 {
        font.draw_wrapped(frame, x, y, r.width() as u32, text, color, control.bold);
        return;
    }
    let width = font.width(text, control.bold) as i32;
    let left = match control.align {
        Align::Left => x,
        Align::Centre => x + (r.width() as i32 - width) / 2,
        Align::Right => r.max.x as i32 - width,
    };
    font.draw(frame, left, y, text, color, control.bold);
}

/// An animation frame part-way into the next: every pixel mixes what either
/// frame would show over the background.
fn blend_frames(frame: &mut RgbaImage, shown: &RgbaImage, next: &RgbaImage, at: Vec2, t: f32) {
    let (left, top) = (at.x as i64, at.y as i64);
    for y in 0..shown.height().max(next.height()) {
        for x in 0..shown.width().max(next.width()) {
            let (fx, fy) = (left + i64::from(x), top + i64::from(y));
            let (Ok(fx), Ok(fy)) = (u32::try_from(fx), u32::try_from(fy)) else { continue };
            let Some(&under) = frame.get_pixel_checked(fx, fy) else { continue };
            let over = |picture: &RgbaImage| {
                picture.get_pixel_checked(x, y).filter(|p| p[3] > 0).copied().unwrap_or(under)
            };
            let (a, b) = (over(shown), over(next));
            let mix = |i: usize| {
                (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * t).round() as u8
            };
            frame.put_pixel(fx, fy, Rgba([mix(0), mix(1), mix(2), 255]));
        }
    }
}

/// Tab-separated cells at the given column starts, clipped to the next
/// column — or centred or to the right in a width of their own.
#[allow(clippy::too_many_arguments)]
fn columns<'a>(
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
            _ => {
                let clip = tabs.get(i + 1).map_or(right, |&next| x + next as i32 - 4);
                font.draw_clipped(frame, left, y, cell, color, false, clip.min(right));
            }
        }
    }
}

/// The colour of a player's name on a position: white on his main one,
/// green on a side one, blue on one near it, red anywhere else.
pub(super) fn fit_ink(fit: i32) -> [u8; 3] {
    match fit {
        10.. => [255, 255, 255],
        8..=9 => [96, 255, 96],
        6..=7 => [150, 210, 255],
        _ => [255, 72, 72],
    }
}

/// A player's box, as the game paints it: blue with his name on top, his
/// position and strength in the bottom right and his abilities in the
/// bottom left; yellow with the position's name when the spot is empty.
fn draw_player(
    frame: &mut RgbaImage,
    classic: &ClassicDialog,
    font: &BitmapFont,
    r: Rect,
    player: &PlayerBox,
) {
    let (x, y) = (r.min.x as i32, r.min.y as i32);
    let image = if player.filled { hub::BOX_FILLED } else { hub::BOX_EMPTY };
    if let Some(picture) = classic.images.get(&image) {
        imageops::overlay(frame, picture, x.into(), y.into());
    }
    // The text keeps a pixel clear of either side.
    let (inner, width) = (x + 1, r.width() as u32 - 2);
    let centred = |frame: &mut RgbaImage, top: i32, line: &str, color: Rgba<u8>| {
        let left = inner + (width as i32 - font.width(line, false) as i32) / 2;
        font.draw_clipped(frame, left, top, line, color, false, x + r.width() as i32 - 1);
    };
    if !player.filled {
        let lines = font.wrap(&player.name, width, false);
        let top = y + (EMPTY_BOX_HEIGHT - (lines.len() as u32 * font.height()) as i32) / 2;
        for (i, line) in lines.iter().enumerate() {
            centred(frame, top + (i as u32 * font.height()) as i32, line, TEXT);
        }
        return;
    }
    let [red, green, blue] = fit_ink(player.fit);
    centred(frame, y, &player.name, Rgba([red, green, blue, 255]));
    font.draw(frame, x + BOX_POSITION, y + BOX_DETAIL_TOP, &player.position, BOX_INK, false);
    let at = x + BOX_STRENGTH_RIGHT - font.width(&player.strength, false) as i32;
    font.draw(frame, at, y + BOX_DETAIL_TOP, &player.strength, BOX_INK, false);
    for (i, id) in player.icons.iter().enumerate() {
        if let Some(icon) = classic.images.get(id) {
            let left = x + BOX_ICON_LEFT + BOX_ICON_STEP * i as i32;
            imageops::overlay(frame, icon, left.into(), (y + BOX_ICON_TOP).into());
        }
    }
}

/// The menu bar: its symbols as they stand, runs of those out of reach
/// greyed, the open screen's pressed, and the one pointed at lit.
fn draw_bar(
    frame: &mut RgbaImage,
    classic: &ClassicDialog,
    control: &Control,
    items: &[crate::rch::Item],
    state: &super::ControlState,
    view: &View,
    index: usize,
) {
    let (x, y) = (control.rect.min.x as i64, control.rect.min.y as i64);
    let part = |frame: &mut RgbaImage, image: u16, left: f32, right: f32| {
        let Some(strip) = classic.images.get(&image) else { return };
        let (left, right) = (left.max(0.0) as u32, (right as u32).min(strip.width()));
        if right > left {
            let piece = imageops::crop_imm(strip, left, 0, right - left, strip.height()).to_image();
            imageops::overlay(frame, &piece, x + i64::from(left), y);
        }
    };
    if let Some(bar) = classic.images.get(&hub::BAR_NORMAL) {
        imageops::overlay(frame, bar, x, y);
    }
    let mut item = 0;
    while item < items.len() {
        if state.item_on(item) {
            item += 1;
            continue;
        }
        let first = item;
        while item < items.len() && !state.item_on(item) {
            item += 1;
        }
        part(frame, hub::BAR_OFF, items[first].rect.min.x, items[item - 1].rect.max.x);
    }
    if let Some(chosen) = items.get(state.chosen) {
        part(frame, hub::BAR_PRESSED, chosen.rect.min.x, chosen.rect.max.x);
    }
    if let Some(Target::Option { control: c, item }) = view.hovered
        && c == index
        && item != state.chosen
        && let Some(lit) = items.get(item)
    {
        let strip = hub::BAR_LIT[item % 2];
        part(frame, strip, lit.rect.min.x - 1.0, lit.rect.max.x + 1.0);
    }
}

fn fill(frame: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, color: Rgba<u8>) {
    for py in y.max(0)..(y + h).min(frame.height() as i32) {
        for px in x.max(0)..(x + w).min(frame.width() as i32) {
            frame.put_pixel(px as u32, py as u32, color);
        }
    }
}

/// The dot of a chosen radio option, drawn as Windows drew it: four pixels
/// square with its corners cut, in the middle of the 12 px bullet.
fn dot(frame: &mut RgbaImage, centre: Vec2) {
    const DOT: [[bool; 4]; 4] = [
        [false, true, true, false],
        [true, true, true, true],
        [true, true, true, true],
        [false, true, true, false],
    ];
    let (left, top) = (centre.x as i32 - 2, centre.y as i32 - 2);
    for (dy, row) in DOT.iter().enumerate() {
        for (dx, &on) in row.iter().enumerate() {
            let (x, y) = (left + dx as i32, top + dy as i32);
            if on && let Some(pixel) = frame.get_pixel_mut_checked(x as u32, y as u32) {
                *pixel = DOT_COLOR;
            }
        }
    }
}
