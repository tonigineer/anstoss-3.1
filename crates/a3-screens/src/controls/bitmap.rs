//! Bitmaps: a picture the flow puts in — one of the game's graphics, a file
//! of the installation or a chart the game draws.

use std::error::Error;

use image::{Rgba, RgbaImage};

use crate::classic::{Paint, paste};
use crate::game::{Game, transparent_black};

/// The red the game draws its charts in.
const CHART_INK: Rgba<u8> = Rgba([255, 0, 0, 255]);

#[derive(Clone, Default, Debug)]
pub struct BitmapState {
    /// What it shows.
    pub picture: Option<Picture>,
    /// Whether it stands on the control's bottom edge, as a bar does: the
    /// picture is cut to its height.
    pub bottom: bool,
    /// A left edge further right than the control's, which the screen moved
    /// its window's to: the picture starts there.
    pub left: Option<f32>,
}

impl BitmapState {
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        if let Some(picture) = self.picture.as_ref().and_then(|p| paint.bitmap(p)) {
            let r = paint.control.rect;
            let left = self.left.unwrap_or(r.min.x);
            let at = if self.bottom {
                bevy_math::Vec2::new(left, r.max.y - picture.height() as f32)
            } else {
                bevy_math::Vec2::new(left, r.min.y)
            };
            paste(frame, picture, at);
        }
    }
}

/// A picture a bitmap control shows: one of the game's graphics, a file of
/// the installation or a chart the game draws, whole or a part of it (x, y,
/// width, height), and with black left out or not.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Picture {
    pub source: PictureSource,
    pub part: Option<[u32; 4]>,
    pub black_clear: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PictureSource {
    Graphic(u16),
    File(String),
    /// A file of the graphics archive, by its path there.
    Archive(String),
    /// A red line through values from 0 to 100, one every few pixels, in a
    /// clear picture of the given size.
    Chart {
        values: Vec<u8>,
        size: (u32, u32),
        step: u32,
    },
}

impl Picture {
    pub fn graphic(id: u16) -> Self {
        Self { source: PictureSource::Graphic(id), part: None, black_clear: false }
    }

    pub fn file(path: String) -> Self {
        Self { source: PictureSource::File(path), part: None, black_clear: true }
    }

    pub fn load(&self, game: &Game) -> Result<RgbaImage, Box<dyn Error>> {
        let whole = match &self.source {
            PictureSource::Graphic(id) => game.graphic(*id)?,
            PictureSource::File(path) => image::load_from_memory(&game.file(path)?)?.to_rgba8(),
            PictureSource::Archive(path) => game.picture(path)?,
            PictureSource::Chart { values, size, step } => chart(values, *size, *step),
        };
        let part = match self.part {
            Some([x, y, w, h]) => image::imageops::crop_imm(&whole, x, y, w, h).to_image(),
            None => whole,
        };
        Ok(if self.black_clear { transparent_black(part) } else { part })
    }
}

/// The game's line chart: from the left edge, a point every `step` pixels,
/// 100 at the top and 0 at the bottom.
fn chart(values: &[u8], (w, h): (u32, u32), step: u32) -> RgbaImage {
    let mut picture = RgbaImage::new(w, h);
    let y = |v: u8| h as i32 - i32::from(v) * h as i32 / 101;
    let points: Vec<(i32, i32)> =
        values.iter().enumerate().map(|(i, &v)| (i as i32 * step as i32, y(v))).collect();
    for pair in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for k in 0..=steps {
            let x = x0 + (x1 - x0) * k / steps;
            let y = y0 + (y1 - y0) * k / steps;
            if (0..w as i32).contains(&x) && (0..h as i32).contains(&y) {
                picture.put_pixel(x as u32, y as u32, CHART_INK);
            }
        }
    }
    picture
}
