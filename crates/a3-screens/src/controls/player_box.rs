//! A player's box on the pitch or the bench of the line-up, the game's
//! `CMannStatic`, which draws itself.

use image::{Rgba, RgbaImage, imageops};

use crate::classic::{Paint, TEXT};

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

/// What a box shows: the player's name — or, on an empty spot, the
/// position's — the position he plays and his strength there, up to three
/// abilities, and how well he fits the spot, in tenths.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct PlayerBox {
    pub name: String,
    pub position: String,
    pub strength: String,
    pub fit: i32,
    pub icons: Vec<u16>,
    pub filled: bool,
}

impl PlayerBox {
    /// As the game paints it: blue with his name on top, his position and
    /// strength in the bottom right and his abilities in the bottom left;
    /// yellow with the position's name when the spot is empty.
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let Some(font) = paint.font() else { return };
        let r = paint.control.rect;
        let (x, y) = (r.min.x as i32, r.min.y as i32);
        let image = if self.filled { FILLED } else { EMPTY };
        if let Some(picture) = paint.image(image) {
            imageops::overlay(frame, picture, x.into(), y.into());
        }
        // The text keeps a pixel clear of either side.
        let (inner, width) = (x + 1, r.width() as u32 - 2);
        let centred = |frame: &mut RgbaImage, top: i32, line: &str, color: Rgba<u8>| {
            let left = inner + (width as i32 - font.width(line, false) as i32) / 2;
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
        for (i, &id) in self.icons.iter().enumerate() {
            if let Some(icon) = paint.image(id) {
                let left = x + ICON_LEFT + ICON_STEP * i as i32;
                imageops::overlay(frame, icon, left.into(), (y + ICON_TOP).into());
            }
        }
    }
}

pub(crate) fn graphics(player: Option<&PlayerBox>) -> (Vec<u16>, bool) {
    let icons = player.iter().flat_map(|p| p.icons.iter().copied());
    ([FILLED, EMPTY].into_iter().chain(icons).collect(), false)
}

/// The colour of a player's name on a position: white on his main one,
/// green on a side one, blue on one near it, red anywhere else.
pub fn fit_ink(fit: i32) -> [u8; 3] {
    match fit {
        10.. => [255, 255, 255],
        8..=9 => [96, 255, 96],
        6..=7 => [150, 210, 255],
        _ => [255, 72, 72],
    }
}
