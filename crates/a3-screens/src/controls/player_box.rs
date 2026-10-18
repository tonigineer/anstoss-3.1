//! A player's box on the pitch or the bench of the line-up, the game's
//! `CMannStatic`, which draws itself; or the action area a player of the
//! eleven stands in, its `CGegenStatic`, which draws his box in it.

use a3_files::rch::Control;
use bevy_math::{Rect, Vec2};
use image::{Rgba, RgbaImage, imageops};

use crate::classic::{Paint, TEXT, centred, fill};
use crate::pointer::{Geometry, Target};

/// The box: blue with a player in it, yellow without.
pub const FILLED: u16 = 1253;
pub const EMPTY: u16 = 1255;
/// An empty box centres the position's name on this height; a filled one
/// shows the position, and the strength ending at the right, on a line
/// below the name, and the abilities, one every few pixels, from the left.
const EMPTY_HEIGHT: i32 = 26;
const DETAIL_TOP: i32 = 12;
const POSITION: i32 = 59;
const STRENGTH_RIGHT: i32 = 98;
const ICON_LEFT: i32 = 2;
const ICON_STEP: i32 = 14;
const ICON_TOP: i32 = 11;
const INK: Rgba<u8> = Rgba([255, 255, 255, 255]);
/// An action area shows the raster picture of the pitch (`BgRaster.jpg`),
/// which lies 24 pixels down the dialog; eight blue handles, five pixels
/// square, at its corners and the middles of its sides; and takes hold of
/// them a little more widely, 11 pixels square (`CGegenStatic` paint
/// `FUN_00626660`, `OnLButtonDown_004af588`).
const RASTER: u16 = 2691;
const RASTER_DOWN: f32 = 24.0;
const HANDLE: i32 = 5;
const HANDLE_INK: Rgba<u8> = Rgba([0, 0, 255, 255]);
const HANDLE_REACH: i32 = 11;
pub const HANDLES: usize = 8;

/// What a box shows: the player's name — or, on an empty spot, the
/// position's — the position he plays and his strength there, up to three
/// abilities and the duty he has on the pitch, beside them, and how well he
/// fits the spot, in tenths.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct PlayerBox {
    pub name: String,
    pub position: String,
    pub strength: String,
    pub fit: i32,
    pub icons: Vec<u16>,
    pub duty: Option<u16>,
    pub filled: bool,
    /// Where the box stands, its top left, when the formation puts it
    /// elsewhere than the dialog does.
    pub at: Option<Vec2>,
    /// The action area the box stands in and shows, if it is one.
    pub area: Option<Rect>,
}

impl PlayerBox {
    /// Where the box is drawn, in classic pixels.
    pub fn rect(&self, control: &Control) -> Rect {
        match self.at {
            Some(at) => Rect::from_corners(at, at + control.rect.size()),
            None => control.rect,
        }
    }

    /// The handles of an action area, by their place round it from the
    /// top left, row by row, the middle left out: what the pointer takes
    /// hold of, and what shows.
    pub fn handles(area: Rect, reach: i32) -> [Rect; HANDLES] {
        let (x, y) = (area.min.x as i32, area.min.y as i32);
        let (w, h) = (area.width() as i32, area.height() as i32);
        let (half, end) = ((reach + 1) / 2 - 1, reach);
        let across = [x, x + w / 2 - half, x + w - end];
        let down = [y, y + h / 2 - half, y + h - end];
        let mut rects = [Rect::default(); HANDLES];
        let places = (0..3).flat_map(|row| (0..3).map(move |col| (row, col)));
        for (i, (row, col)) in places.filter(|&p| p != (1, 1)).enumerate() {
            let at = Vec2::new(across[col] as f32, down[row] as f32);
            rects[i] = Rect::from_corners(at, at + Vec2::splat(reach as f32));
        }
        rects
    }

    pub(crate) fn targets(
        &self,
        index: usize,
        control: &Control,
        geometry: &dyn Geometry,
        out: &mut Vec<(Target, Rect)>,
    ) {
        match self.area {
            // An area answers only by its handles; elsewhere the pointer
            // goes through to what lies under it, as the game passes it on
            // to the dialog.
            Some(area) => {
                for (row, r) in Self::handles(area, HANDLE_REACH).into_iter().enumerate() {
                    out.push((Target::Row { control: index, row }, geometry.map(r)));
                }
            }
            // A player's box can be dragged to another place.
            None => out.push((Target::Control(index), geometry.map(self.rect(control)))),
        }
    }

    /// As the game paints it: blue with his name on top, his position and
    /// strength in the bottom right and his abilities in the bottom left;
    /// yellow with the position's name when the spot is empty. An area
    /// first, under the box.
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        if let (Some(area), Some(raster)) = (self.area, paint.image(RASTER)) {
            let (x, y) = (area.min.x as u32, (area.min.y - RASTER_DOWN).max(0.0) as u32);
            let (w, h) = (area.width() as u32, area.height() as u32);
            let part = imageops::crop_imm(raster, x, y, w, h).to_image();
            imageops::overlay(frame, &part, area.min.x as i64, area.min.y as i64);
        }
        self.draw_box(frame, paint);
        if let Some(area) = self.area {
            for r in Self::handles(area, HANDLE) {
                fill(frame, r.min.x as i32, r.min.y as i32, HANDLE, HANDLE, HANDLE_INK);
            }
        }
    }

    fn draw_box(&self, frame: &mut RgbaImage, paint: &Paint) {
        let Some(font) = paint.font() else { return };
        let r = self.rect(paint.control);
        let (x, y) = (r.min.x as i32, r.min.y as i32);
        let image = if self.filled { FILLED } else { EMPTY };
        if let Some(picture) = paint.image(image) {
            imageops::overlay(frame, picture, x.into(), y.into());
        }
        // The text keeps a pixel clear of either side.
        let (inner, width) = (x + 1, r.width() as u32 - 2);
        let centred = |frame: &mut RgbaImage, top: i32, line: &str, color: Rgba<u8>| {
            let left = centred(inner, width as i32, font.width(line, false) as i32);
            font.draw_clipped(frame, left, top, line, color, false, x + r.width() as i32 - 1);
        };
        if !self.filled {
            let lines = font.wrap(&self.name, width, false);
            let top = y + (EMPTY_HEIGHT - (lines.len() as u32 * font.height()) as i32) / 2;
            for (i, line) in lines.iter().enumerate() {
                centred(frame, top + (i as u32 * font.height()) as i32, line, TEXT);
            }
            return;
        }
        let [red, green, blue] = fit_ink(self.fit);
        centred(frame, y, &self.name, Rgba([red, green, blue, 255]));
        font.draw(frame, x + POSITION, y + DETAIL_TOP, &self.position, INK, false);
        let at = x + STRENGTH_RIGHT - font.width(&self.strength, false) as i32;
        font.draw(frame, at, y + DETAIL_TOP, &self.strength, INK, false);
        for (i, &id) in self.icons.iter().chain(&self.duty).enumerate() {
            if let Some(icon) = paint.image(id) {
                let left = x + ICON_LEFT + ICON_STEP * i as i32;
                imageops::overlay(frame, icon, left.into(), (y + ICON_TOP).into());
            }
        }
    }
}

pub(crate) fn graphics(player: Option<&PlayerBox>) -> (Vec<u16>, bool) {
    let icons = player.iter().flat_map(|p| p.icons.iter().chain(&p.duty).copied());
    let raster = player.and_then(|p| p.area).map(|_| RASTER);
    ([FILLED, EMPTY].into_iter().chain(icons).chain(raster).collect(), false)
}

/// The colour of a player's name on a position: white on his main one,
/// green on a side one, blue on one near it, red anywhere else.
pub fn fit_ink(fit: i32) -> [u8; 3] {
    match fit {
        10.. => [255, 255, 255],
        8..=9 => [0, 255, 0],
        6..=7 => [0, 0, 255],
        _ => [255, 0, 0],
    }
}
