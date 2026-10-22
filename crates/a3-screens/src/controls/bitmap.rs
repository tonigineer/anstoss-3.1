//! Bitmaps: a picture the flow puts in — one of the game's graphics, a file
//! of the installation or a chart the game draws.

use std::error::Error;

use image::{Rgba, RgbaImage};

use crate::classic::{Paint, centred, fill, paste};
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
    /// Whether a click on it reaches the screen (447's groundsman).
    pub clickable: bool,
    /// Where it leaves the dialog's picture showing: a bitmap button there
    /// paints its own background (the views' OK over a dream stadium).
    pub holes: Vec<bevy_math::Rect>,
    /// The tiles a click tells (`Target::Tile`): the grounds' map and its
    /// minimap.
    pub tiles: Option<Tiles>,
    /// The tooltip over the grounds' map.
    pub tip: Option<Tip>,
}

/// The grounds map's tooltip (STADION-ANSICHT.md §6.4): lines of text in
/// a white box edged black, set by where the pointer is in the dialog.
#[derive(Clone, PartialEq, Debug)]
pub struct Tip {
    pub at: bevy_math::Vec2,
    pub lines: Vec<String>,
}

/// Its font, the lines' spacing; it keeps left of x 450 and below y 25,
/// and the map's corner (2, 25) is added when it is drawn.
const TIP_FONT: u8 = 0;
const TIP_LINE: i32 = 14;
const TIP_RIGHT: i32 = 450;
const TIP_TOP: i32 = 25;
const TIP_SHIFT: (i32, i32) = (2, 25);

impl Tip {
    /// Drawn as the map draws it, cut to the map.
    fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let Some(font) = paint.font_of(TIP_FONT) else { return };
        let width = |line: &String| font.width(line, false) as i32;
        let w = self.lines.iter().map(width).max().unwrap_or(0);
        let (mx, my) = (self.at.x.floor() as i32, self.at.y.floor() as i32);
        let h = TIP_LINE * self.lines.len() as i32;
        let (mut l, mut r) = (mx, mx + 4 + w);
        if r > TIP_RIGHT {
            (l, r) = (TIP_RIGHT - w - 5, TIP_RIGHT - 1);
        }
        let mut t = my - 18 - h;
        if t < TIP_TOP {
            t = my + 10;
        }
        let (l, r, t) = (l + TIP_SHIFT.0, r + TIP_SHIFT.0, t + TIP_SHIFT.1);
        // Cut to the map: drawn on a copy of its part of the frame.
        let area = paint.control.rect;
        let (ax, ay) = (area.min.x as i32, area.min.y as i32);
        let (aw, ah) = (area.width() as u32, area.height() as u32);
        let mut part = image::imageops::crop_imm(frame, ax as u32, ay as u32, aw, ah).to_image();
        let black = Rgba([0, 0, 0, 255]);
        fill(&mut part, l - ax, t - ay, r - l, h, black);
        fill(&mut part, l + 1 - ax, t + 1 - ay, r - l - 2, h - 2, Rgba([255, 255, 255, 255]));
        // The text a pixel below the box's top, as the original shows it
        // (shots g21, g22).
        for (i, line) in self.lines.iter().enumerate() {
            let (x, y) = (centred(l, r - l, width(line)), t + 1 + TIP_LINE * i as i32);
            font.draw(&mut part, x - ax, y - ay, line, black, false);
        }
        image::imageops::replace(frame, &part, ax.into(), ay.into());
    }
}

/// Square tiles over a part of a bitmap: their size, how far the first
/// starts left of and above the part, its column and row, and the part.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tiles {
    pub size: f32,
    pub offset: bevy_math::Vec2,
    pub first: (usize, usize),
    pub area: bevy_math::Rect,
}

impl Tiles {
    /// Every tile with its rectangle, cut to the part.
    pub(crate) fn each(&self) -> impl Iterator<Item = ((usize, usize), bevy_math::Rect)> + '_ {
        let count = |room: f32, off: f32| ((room + off) / self.size).ceil().max(0.0) as usize;
        let (columns, rows) =
            (count(self.area.width(), self.offset.x), count(self.area.height(), self.offset.y));
        (0..rows).flat_map(move |r| {
            (0..columns).map(move |c| {
                let min = self.area.min - self.offset
                    + bevy_math::Vec2::new(c as f32, r as f32) * self.size;
                let tile =
                    bevy_math::Rect::from_corners(min, min + bevy_math::Vec2::splat(self.size))
                        .intersect(self.area);
                ((self.first.0 + c, self.first.1 + r), tile)
            })
        })
    }
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
            if self.holes.is_empty() {
                paste(frame, picture, at);
            } else {
                let mut cut = picture.clone();
                for (x, y, p) in cut.enumerate_pixels_mut() {
                    let on = bevy_math::Vec2::new(at.x + x as f32, at.y + y as f32);
                    if self.holes.iter().any(|h| h.contains(on) && on.x < h.max.x && on.y < h.max.y)
                    {
                        p.0[3] = 0;
                    }
                }
                paste(frame, &cut, at);
            }
        }
        if let Some(tip) = &self.tip {
            tip.draw(frame, paint);
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
    /// A stadium as the grounds map draws it at half size, by its look
    /// (`a3_core::stadium::look`).
    Stadium([u8; 12]),
    /// A picture a screen drew itself (the grounds' map).
    Drawn(Drawn),
    /// A red line through values from 0 to 100, one every few pixels, in a
    /// clear picture of the given size.
    Chart {
        values: Vec<u8>,
        size: (u32, u32),
        step: u32,
    },
}

/// A picture drawn by a screen, known by the number it gave it.
#[derive(Clone, Debug)]
pub struct Drawn {
    pub id: u64,
    pub image: std::sync::Arc<RgbaImage>,
}

impl PartialEq for Drawn {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Drawn {}

impl std::hash::Hash for Drawn {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Picture {
    /// A picture a screen drew, as it is.
    pub fn drawn(id: u64, image: RgbaImage) -> Self {
        let drawn = Drawn { id, image: std::sync::Arc::new(image) };
        Self { source: PictureSource::Drawn(drawn), part: None, black_clear: false }
    }

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
            PictureSource::Stadium(look) => crate::buildings::stadium_half(game, look)?,
            PictureSource::Drawn(drawn) => (*drawn.image).clone(),
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
