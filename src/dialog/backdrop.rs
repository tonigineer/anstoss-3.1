//! The pictures the remastered look draws on.
//!
//! Most dialogs of the new-game screens share the "Spielvorbereitung"
//! artwork, each with its own interface painted into it. `tools/outpaint.py`
//! makes one wide picture of it: cleaned of any painted interface, the
//! illustration at the left and carried on to the right edge, where the
//! remastered interface goes. Dialogs with pictures of their own — the start
//! settings, the high scores, the season's screens — keep them in the
//! middle, cleaned of their interface and carried on to both sides; the
//! interface is drawn across the whole width. The scenes, pictures to act
//! in, only grow to the sides.
//!
//! `--export-backgrounds` writes each picture with a mask of what to repaint
//! (white) for the tool to read.

use std::error::Error;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use image::imageops::{self, FilterType};
use image::{GrayImage, Luma, RgbaImage};

use crate::game::{Game, cache_dir};
use crate::look::{CANVAS, CLASSIC, CLASSIC_SCALE, fill_wide};
use crate::rch::{Dialog, Kind};

/// Dialogs drawn on a picture of their own: the start settings, the high
/// scores, the job offers and talks, the line-up, the dressing room and the
/// week, all of whose screens share the picture without panels.
const OWN_PICTURES: [u16; 7] =
    [256, 95, 99, 103, super::hub::LINEUP, super::hub::DRESSING_ROOM, WEEK];
const WEEK: u16 = 1000;
/// Screens that are a picture to act in rather than a form — the wait
/// screen and the offices — keep their interface on the picture.
const SCENES: [u16; 3] = [381, 483, 504];
/// The dialog whose background the shared picture is made from — the start
/// menu, whose painted interface is known — and that picture's name.
const SHARED: u16 = super::START;
const SHARED_NAME: &str = "base";
/// The talks' picture is mostly its table, which the remastered look
/// stretches across the whole width: it is painted anew as a whole.
const REPAINT_WHOLE: [u16; 1] = [103];
/// Pictures of their own whose photo is as strongly coloured as their
/// interface — the week's goalkeeper in blue — and how much greener than
/// red and blue the bars they keep apart from it are.
const KEEP_PHOTO: [u16; 1] = [WEEK];
const GREEN_LEAD: f32 = 50.0;
/// The band of the title bar in the pictures of their own, classic rows.
const TITLE_BAND: (u32, u32) = (24, 66);
/// Around the patches of the shared artwork, this much is repainted too.
const PATCH_MARGIN: f32 = 6.0;
/// In a picture of its own, what is repainted around controls: the frame of
/// a list, from this far outside it to this far inside, and the boxes of
/// fields and options with this margin.
const LIST_FRAME: f32 = 8.0;
const LIST_INSIDE: f32 = 3.0;
const BOX_MARGIN: f32 = 4.0;

/// Whether a dialog keeps its own picture in the middle; all others put the
/// shared artwork at the left.
pub(super) fn has_own_picture(id: u16) -> bool {
    OWN_PICTURES.contains(&id) || is_scene(id) || super::turn::is_week(id)
}

/// Whether a dialog is a scene: its picture in the middle, everything on it
/// where the classic look has it.
pub(super) fn is_scene(id: u16) -> bool {
    SCENES.contains(&id)
}

fn name(id: u16) -> String {
    if super::turn::is_week(id) {
        WEEK.to_string()
    } else if has_own_picture(id) {
        id.to_string()
    } else {
        SHARED_NAME.to_owned()
    }
}

/// Writes each picture and its mask to `<cache>/sources`.
pub(super) fn export(game: &Game) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let dir = cache_dir().join("sources");
    std::fs::create_dir_all(&dir)?;
    let mut written = Vec::new();
    for id in std::iter::once(SHARED).chain(OWN_PICTURES).chain(SCENES) {
        let Some(dialog) = game.dialogs.get(&id) else { continue };
        let art = game.graphic(dialog.background)?;
        let (art, mask) = if is_scene(id) {
            // Nothing in a scene is repainted; it only grows to the sides.
            let mask = GrayImage::new(art.width(), art.height());
            (art, mask)
        } else if REPAINT_WHOLE.contains(&id) {
            let mask = GrayImage::from_pixel(art.width(), art.height(), Luma([255]));
            (art, mask)
        } else if has_own_picture(id) {
            let mask = own_mask(dialog, &art, KEEP_PHOTO.contains(&id));
            (art, mask)
        } else {
            let bullets = bullets(dialog);
            let mask = shared_mask(&super::decor::patches(&bullets));
            (super::decor::clean_artwork(&art, &bullets), mask)
        };
        let stem = name(id);
        let (art_path, mask_path) =
            (dir.join(format!("{stem}.png")), dir.join(format!("{stem}_mask.png")));
        art.save(&art_path)?;
        mask.save(&mask_path)?;
        written.extend([art_path, mask_path]);
    }
    Ok(written)
}

/// The wide picture of a dialog, from the tool's results in `<cache>/wide`;
/// until the tool has run, the artwork carried on with a blur.
pub(super) fn load(id: u16, background: &RgbaImage) -> RgbaImage {
    let path = cache_dir().join("wide").join(format!("{}.png", name(id)));
    if let Some(wide) = read(&path) {
        return wide;
    }
    let (w, h) = ((CLASSIC.x * CLASSIC_SCALE) as u32, (CLASSIC.y * CLASSIC_SCALE) as u32);
    let art = imageops::resize(background, w, h, FilterType::Lanczos3);
    if has_own_picture(id) {
        return fill_wide(&art);
    }
    // The illustration at the left, the rest a blurred stretch of the artwork.
    let mut wide = imageops::resize(background, CANVAS.x as u32, h, FilterType::Triangle);
    wide = imageops::blur(&wide, 24.0);
    imageops::overlay(&mut wide, &art, 0, 0);
    wide
}

/// The centres of the bullets painted into the start menu.
fn bullets(dialog: &Dialog) -> Vec<Vec2> {
    dialog
        .controls
        .iter()
        .filter_map(|c| match &c.kind {
            Kind::Options { image: 0, items } => {
                Some((0..items.len()).map(move |i| super::bullet_centre(c, i)))
            }
            _ => None,
        })
        .flatten()
        .collect()
}

fn read(path: &Path) -> Option<RgbaImage> {
    let wide = image::open(path).ok()?.to_rgba8();
    if wide.dimensions() == (CANVAS.x as u32, CANVAS.y as u32) {
        Some(wide)
    } else {
        warn!("{} is not {}×{}; ignoring it", path.display(), CANVAS.x, CANVAS.y);
        None
    }
}

/// The patches of the cleaned start menu artwork.
fn shared_mask(patches: &[Rect]) -> GrayImage {
    GrayImage::from_fn(CLASSIC.x as u32, CLASSIC.y as u32, |x, y| {
        let point = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
        let repaint = patches.iter().any(|p| p.inflate(PATCH_MARGIN).contains(point));
        Luma([if repaint { 255 } else { 0 }])
    })
}

/// A picture's painted interface: the band of the title bar, the bars and
/// stripes — the strongly coloured pixels the pale pictures otherwise do not
/// have — the frames around lists, the boxes of fields and options, and the
/// lines under labels. A picture whose photo is strongly coloured itself
/// keeps it: only its green bars and the places of its controls go.
fn own_mask(dialog: &Dialog, art: &RgbaImage, photo: bool) -> GrayImage {
    let (top, bottom) = TITLE_BAND;
    let mut mask = GrayImage::from_fn(art.width(), art.height(), |x, y| {
        let [r, g, b, _] = art.get_pixel(x, y).0.map(f32::from);
        let (high, low) = (r.max(g).max(b), r.min(g).min(b));
        let painted = if photo {
            g > r + GREEN_LEAD && g > b + GREEN_LEAD
        } else {
            high > 100.0 && (high - low) / high > 0.32 || (top..=bottom).contains(&y)
        };
        Luma([if painted { 255 } else { 0 }])
    });
    let mut paint = |r: Rect, grow: f32, keep: Option<f32>| {
        let outer = r.inflate(grow);
        let inner = keep.map(|shrink| r.inflate(-shrink));
        for y in outer.min.y.max(0.0) as u32..(outer.max.y as u32).min(art.height()) {
            for x in outer.min.x.max(0.0) as u32..(outer.max.x as u32).min(art.width()) {
                let point = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                if !inner.is_some_and(|inner| inner.contains(point)) {
                    mask.put_pixel(x, y, Luma([255]));
                }
            }
        }
    };
    for control in &dialog.controls {
        let r = control.rect;
        match &control.kind {
            // Lists keep the picture inside their frame.
            Kind::ListBox => paint(r, LIST_FRAME, Some(LIST_INSIDE)),
            // Buttons of a strip, with whatever the picture has around them.
            Kind::Options { image, .. } if *image != 0 => paint(r, BOX_MARGIN, None),
            Kind::Edit { .. } | Kind::Spin | Kind::Bitmap | Kind::Player | Kind::Bar { .. } => {
                paint(r, BOX_MARGIN, None)
            }
            // Labels are underlined from the left edge.
            Kind::Static if !photo => {
                paint(Rect::new(0.0, r.min.y, r.max.x, r.max.y + 4.0), BOX_MARGIN, None)
            }
            Kind::Static => paint(r, BOX_MARGIN, None),
            _ => {}
        }
    }
    dilate(&mask, 4)
}

/// Grows the white of a mask by `radius` pixels.
fn dilate(mask: &GrayImage, radius: i32) -> GrayImage {
    GrayImage::from_fn(mask.width(), mask.height(), |x, y| {
        let hit = (-radius..=radius).any(|dy| {
            (-radius..=radius).any(|dx| {
                let (px, py) = (x as i32 + dx, y as i32 + dy);
                px >= 0
                    && py >= 0
                    && (px as u32) < mask.width()
                    && (py as u32) < mask.height()
                    && mask.get_pixel(px as u32, py as u32)[0] > 0
            })
        });
        Luma([if hit { 255 } else { 0 }])
    })
}
