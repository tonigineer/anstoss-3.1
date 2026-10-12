//! Windows `.fon` bitmap fonts, drawn pixel for pixel the way GDI drew them.
//!
//! ANSTOSS 3 asks for "MS Sans Serif", the bitmap face of the Windows 95/2000
//! user interface. Under Wine that request is served by Wine's own clone,
//! `sserife.fon`, so that is the font the classic look draws with.
//!
//! A `.fon` is a 16-bit NE executable whose `RT_FONT` resources are FNT
//! fonts, one per size. Glyphs are stored as bands 8 pixels wide: each byte is
//! one row of one band, bit 7 leftmost.

use std::io;
use std::path::Path;

use image::{Rgba, RgbaImage};

use crate::cpr::invalid;

const RT_FONT: u16 = 0x8008;

pub struct BitmapFont {
    height: u32,
    first: u8,
    /// Index of the glyph drawn for characters the font lacks.
    default_glyph: usize,
    glyphs: Vec<Glyph>,
}

struct Glyph {
    width: u32,
    /// `height` rows of `width` pixels.
    pixels: Vec<bool>,
}

impl BitmapFont {
    /// Loads the size with the given cell height in pixels (13 for 8 pt at
    /// 96 dpi) from a `.fon` file.
    pub fn load(path: &Path, height: u32) -> io::Result<Self> {
        let data = std::fs::read(path)?;
        let mut sizes = Vec::new();
        for fnt in font_resources(&data)? {
            let font = parse_fnt(fnt)?;
            if font.height == height {
                return Ok(font);
            }
            sizes.push(font.height);
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no {height} px size in {} (has {sizes:?})", path.display()),
        ))
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Advance width of `text` in pixels.
    pub fn width(&self, text: &str, bold: bool) -> u32 {
        text.chars().map(|c| self.glyph(c).width + u32::from(bold)).sum()
    }

    /// Draws `text` with the top-left of its cell at (x, y). `bold` strikes
    /// every glyph twice, one pixel apart, which is how GDI emboldens a
    /// bitmap font.
    pub fn draw(
        &self,
        canvas: &mut RgbaImage,
        x: i32,
        y: i32,
        text: &str,
        color: Rgba<u8>,
        bold: bool,
    ) {
        self.draw_clipped(canvas, x, y, text, color, bold, i32::MAX);
    }

    /// Like [`Self::draw`], leaving out every pixel at or right of `clip_right`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_clipped(
        &self,
        canvas: &mut RgbaImage,
        x: i32,
        y: i32,
        text: &str,
        color: Rgba<u8>,
        bold: bool,
        clip_right: i32,
    ) {
        let mut pen = x;
        for c in text.chars() {
            let glyph = self.glyph(c);
            for dx in 0..=i32::from(bold) {
                self.blit(canvas, glyph, pen + dx, y, color, clip_right);
            }
            pen += (glyph.width + u32::from(bold)) as i32;
        }
    }

    /// Draws `text` word-wrapped to `max_width`, one cell height per line.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_wrapped(
        &self,
        canvas: &mut RgbaImage,
        x: i32,
        y: i32,
        max_width: u32,
        text: &str,
        color: Rgba<u8>,
        bold: bool,
    ) {
        for (i, line) in self.wrap(text, max_width, bold).iter().enumerate() {
            self.draw(canvas, x, y + (i as u32 * self.height) as i32, line, color, bold);
        }
    }

    /// The lines `text` breaks into at spaces to fit `max_width`.
    pub fn wrap(&self, text: &str, max_width: u32, bold: bool) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split(' ') {
            let candidate =
                if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if !line.is_empty() && self.width(&candidate, bold) > max_width {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = candidate;
            }
        }
        lines.push(line);
        lines
    }

    fn glyph(&self, c: char) -> &Glyph {
        let index = to_cp1252(c).and_then(|byte| byte.checked_sub(self.first)).map(usize::from);
        index.and_then(|i| self.glyphs.get(i)).unwrap_or(&self.glyphs[self.default_glyph])
    }

    fn blit(
        &self,
        canvas: &mut RgbaImage,
        glyph: &Glyph,
        x: i32,
        y: i32,
        color: Rgba<u8>,
        clip_right: i32,
    ) {
        for row in 0..self.height {
            for col in 0..glyph.width {
                let (px, py) = (x + col as i32, y + row as i32);
                let inside = px >= 0
                    && px < clip_right
                    && py >= 0
                    && (px as u32) < canvas.width()
                    && (py as u32) < canvas.height();
                if inside && glyph.pixels[(row * glyph.width + col) as usize] {
                    canvas.put_pixel(px as u32, py as u32, color);
                }
            }
        }
    }
}

/// The `RT_FONT` resources of an NE executable.
fn font_resources(data: &[u8]) -> io::Result<Vec<&[u8]>> {
    let ne = u32_at(data, 0x3c)? as usize;
    if data.get(ne..ne + 2) != Some(b"NE") {
        return Err(invalid("not an NE executable"));
    }
    let table = ne + usize::from(u16_at(data, ne + 0x24)?);
    let shift = u16_at(data, table)?;
    let mut pos = table + 2;
    let mut fonts = Vec::new();
    loop {
        let (kind, count) = (u16_at(data, pos)?, u16_at(data, pos + 2)?);
        pos += 8;
        if kind == 0 {
            return Ok(fonts);
        }
        for _ in 0..count {
            let offset = usize::from(u16_at(data, pos)?) << shift;
            let len = usize::from(u16_at(data, pos + 2)?) << shift;
            pos += 12;
            if kind == RT_FONT {
                fonts.push(
                    data.get(offset..offset + len)
                        .ok_or_else(|| invalid("font resource out of bounds"))?,
                );
            }
        }
    }
}

/// Parses an FNT v2 or v3 font.
fn parse_fnt(fnt: &[u8]) -> io::Result<BitmapFont> {
    let v3 = match u16_at(fnt, 0)? {
        0x200 => false,
        0x300 => true,
        version => return Err(invalid(&format!("unsupported FNT version {version:#x}"))),
    };
    let height = u32::from(u16_at(fnt, 88)?);
    // The default character is stored relative to the first one.
    let (first, last, default_char) = (byte_at(fnt, 95)?, byte_at(fnt, 96)?, byte_at(fnt, 97)?);
    let (table, entry_len) = if v3 { (148, 6) } else { (118, 4) };

    let mut glyphs = Vec::new();
    for i in 0..=usize::from(last.saturating_sub(first)) {
        let entry = table + i * entry_len;
        let width = u32::from(u16_at(fnt, entry)?);
        let offset = if v3 {
            u32_at(fnt, entry + 2)? as usize
        } else {
            usize::from(u16_at(fnt, entry + 2)?)
        };
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for row in 0..height as usize {
            for col in 0..width as usize {
                let byte = byte_at(fnt, offset + (col / 8) * height as usize + row)?;
                pixels.push(byte & (0x80 >> (col % 8)) != 0);
            }
        }
        glyphs.push(Glyph { width, pixels });
    }
    if glyphs.is_empty() {
        return Err(invalid("font has no glyphs"));
    }
    let default_glyph = usize::from(default_char).min(glyphs.len() - 1);
    Ok(BitmapFont { height, first, default_glyph, glyphs })
}

/// Windows-1252 from 0x80 to 0x9F, where it differs from Latin-1.
pub const CP1252_HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// Windows-1252, the code page of the Western `.fon` files.
pub fn to_cp1252(c: char) -> Option<u8> {
    match c as u32 {
        0..=0x7f | 0xa0..=0xff => Some(c as u8),
        _ => CP1252_HIGH.iter().position(|&h| h == c).map(|i| 0x80 + i as u8),
    }
}

fn byte_at(data: &[u8], pos: usize) -> io::Result<u8> {
    data.get(pos).copied().ok_or_else(|| invalid("truncated font"))
}

fn u16_at(data: &[u8], pos: usize) -> io::Result<u16> {
    Ok(u16::from_le_bytes([byte_at(data, pos)?, byte_at(data, pos + 1)?]))
}

fn u32_at(data: &[u8], pos: usize) -> io::Result<u32> {
    crate::cpr::u32_at(data, pos)
}
