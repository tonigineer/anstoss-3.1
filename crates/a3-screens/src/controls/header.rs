//! Column titles above a list, separated by tabs, lined up with its columns.

use image::RgbaImage;

use a3_files::rch::Align;

use crate::classic::{Paint, TEXT, columns};

const TOP: i32 = 2;

#[derive(Clone, Debug)]
pub struct HeaderState {
    /// The titles, separated by `\t` as the dialog file writes it.
    pub text: String,
    /// Where the columns start, relative to the control, and how the
    /// centred and right-aligned ones sit in their width.
    pub tabs: Vec<f32>,
    pub aligns: Vec<Option<(Align, f32)>>,
}

impl Default for HeaderState {
    fn default() -> Self {
        Self { text: String::new(), tabs: vec![0.0], aligns: Vec::new() }
    }
}

impl HeaderState {
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let Some(font) = paint.font() else { return };
        let r = paint.control.rect;
        let titles = self.text.split("\\t");
        // Two pixels below the top (`CMyTabStatic::FUN_007afa80`).
        let (at, right) = ((r.min.x as i32, r.min.y as i32 + TOP), r.max.x as i32);
        columns(font, frame, at, titles, &self.tabs, &self.aligns, TEXT, right);
    }
}
