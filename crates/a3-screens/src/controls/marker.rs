//! A marker of the list view's small pitch (dialog 429's 2510 to 2520, the
//! game's bitmap buttons that `FUN_00503420` moves, pictures and labels): a
//! ringed disc for the kind of the spot, the shirt number on it in an ink
//! for how well the player fits there.

use bevy_math::Vec2;
use image::{Rgba, RgbaImage};

use crate::classic::{Paint, centred, paste};

/// The window the game moves a marker into.
pub const SIZE: Vec2 = Vec2::new(19.0, 20.0);

#[derive(Clone, Default, PartialEq, Debug)]
pub struct Marker {
    /// Its picture, its top left, its text and the text's ink.
    pub image: u16,
    pub at: Vec2,
    pub text: String,
    pub ink: [u8; 3],
}

impl Marker {
    /// The picture at the window's corner, the text centred in the window.
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        if let Some(picture) = paint.image(self.image) {
            paste(frame, picture, self.at);
        }
        let Some(font) = paint.font() else { return };
        let (x, y) = (self.at.x as i32, self.at.y as i32);
        let left = centred(x, SIZE.x as i32, font.width(&self.text, false) as i32);
        let top = y + (SIZE.y as i32 - font.height() as i32) / 2;
        let [r, g, b] = self.ink;
        font.draw(frame, left, top, &self.text, Rgba([r, g, b, 255]), false);
    }
}
