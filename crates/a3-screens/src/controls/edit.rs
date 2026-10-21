//! Text fields: what is typed into them, in the characters the game's code
//! page has, up to the length the dialog file allows.

use image::RgbaImage;

use a3_files::bitmap_font::to_cp1252;

use crate::classic::{Paint, TEXT, fill};

#[derive(Clone, Default, Debug)]
pub struct EditState {
    pub text: String,
}

impl EditState {
    /// Types `typed` in, as far as there is room.
    pub fn type_text(&mut self, typed: &str, max_len: usize) {
        for c in typed.chars() {
            if !c.is_control() && to_cp1252(c).is_some() && self.text.chars().count() < max_len {
                self.text.push(c);
            }
        }
    }

    pub fn backspace(&mut self) {
        self.text.pop();
    }

    /// The text from the field's top left corner, as the game's edits
    /// draw it (154's name, checked against the original).
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let Some(font) = paint.font() else { return };
        let r = paint.control.rect;
        let (x, top, right) = (r.min.x as i32, r.min.y as i32, r.max.x as i32);
        let cell = font.height() as i32;
        font.draw_clipped(frame, x, top, &self.text, TEXT, false, right - 1);
        if paint.caret && paint.view.focus == Some(paint.index) {
            let at = x + font.width(&self.text, false) as i32;
            fill(frame, at.min(right - 2), top, 1, cell, TEXT);
        }
    }
}
