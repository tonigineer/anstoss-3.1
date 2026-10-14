//! Bitmap fonts, drawn pixel for pixel: the game's own, and Windows `.fon`
//! fonts as GDI drew them.
//!
//! ANSTOSS 3 draws its dialogs' text with fonts of its own: strips of
//! glyphs in `grafik.cpr` (`MSSans8_Neu.bmp` …), measured and drawn by the
//! `sgl` library in `D3D_Dll.dll` ([`BitmapFont::from_strip`]).
//!
//! A `.fon` is a 16-bit NE executable whose `RT_FONT` resources are FNT
//! fonts, one per size. Glyphs are stored as bands 8 pixels wide: each byte is
//! one row of one band, bit 7 leftmost. Wine's `sserife.fon`, its clone of
//! "MS Sans Serif", stands in where the game's fonts cannot be read.

use std::io;
use std::path::Path;

use image::{Rgba, RgbaImage};

use crate::cpr::invalid;

const RT_FONT: u16 = 0x8008;
/// The first character of a strip's glyphs.
const STRIP_FIRST: u8 = 0x20;
/// Characters `sgl` never kerns (`D3D_Dll.dll`, 0x5094b68).
const UNKERNED: &[u8] = b"+-*/\\#~!\"\xa7$%&()= ";

#[derive(Clone)]
pub struct BitmapFont {
    height: u32,
    first: u8,
    /// Index of the glyph drawn for characters the font lacks.
    default_glyph: usize,
    glyphs: Vec<Glyph>,
    /// How far the pen moves from one glyph to the next, by pairs: a row
    /// of `glyphs.len()` for each glyph.
    kerning: Option<Vec<u8>>,
    /// The face bold text is drawn in; without one, bold strikes every
    /// glyph twice.
    bold: Option<Box<BitmapFont>>,
}

#[derive(Clone)]
struct Glyph {
    /// The columns drawn.
    width: u32,
    /// How far the pen moves on.
    advance: u32,
    /// `height` rows of `width` pixels: white where the text's colour
    /// shows, at the coverage of their alpha.
    pixels: Vec<Rgba<u8>>,
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

    /// A font of the game: a strip of `cell`-sized glyphs from the space
    /// on, black or transparent where they are blank. As `sgl` measures it
    /// (`FUN_0500fe84`): a glyph is as wide as its rightmost pixel, plus
    /// `extra`; a blank one `blank` wide; and one more pixel to the next.
    /// A kerning font moves glyphs closer by their shapes, row by row.
    pub fn from_strip(
        strip: &RgbaImage,
        (cell_width, height): (u32, u32),
        extra: u8,
        blank: u8,
        kerning: bool,
    ) -> io::Result<Self> {
        let count = strip.width() / cell_width.max(1);
        if count == 0 || strip.height() < height {
            return Err(invalid("no glyphs in the strip"));
        }
        // Black is see-through, as the game keys it.
        let pixel = |cell: u32, col: u32, row: u32| {
            let p = *strip.get_pixel(cell * cell_width + col, row);
            if p.0[..3] == [0, 0, 0] && p.0[3] == 255 { Rgba([0; 4]) } else { p }
        };
        // `sgl` reads its surface's green, red and alpha bytes as one byte.
        let lit = |p: Rgba<u8>| p.0[1].wrapping_add(p.0[0]).wrapping_add(p.0[3]) != 0;
        let mut widths: Vec<u32> = (0..count)
            .map(|cell| {
                let rightmost = (0..height)
                    .flat_map(|row| (0..cell_width).map(move |col| (col, row)))
                    .filter(|&(col, row)| lit(pixel(cell, col, row)))
                    .map(|(col, _)| col)
                    .max()
                    .unwrap_or(0);
                u32::from(extra) + rightmost
            })
            .map(|w| if w <= u32::from(extra) { u32::from(blank) } else { w })
            .collect();
        let pairs = kerning.then(|| {
            // Each glyph's room on the left and the right, row by row.
            let room: Vec<Vec<(u32, u32)>> = (0..count)
                .map(|cell| {
                    let w = widths[cell as usize];
                    (0..height)
                        .map(|row| {
                            let cols = (0..=w.min(cell_width - 1))
                                .filter(|&col| pixel(cell, col, row).0 != [0; 4]);
                            cols.fold((w, w), |(left, right), col| {
                                (left.min(col), right.min(w - col))
                            })
                        })
                        .collect()
                })
                .collect();
            let kerned = |cell: usize| {
                !UNKERNED.iter().any(|&c| usize::from(c.saturating_sub(STRIP_FIRST)) == cell)
            };
            let mut pairs = Vec::with_capacity((count * count) as usize);
            for a in 0..count as usize {
                for b in 0..count as usize {
                    let advance = u32::from(extra) + widths[a];
                    pairs.push(if kerned(a) && kerned(b) {
                        let gap = (0..height as usize)
                            .map(|row| room[a][row].1 + room[b][row].0)
                            .fold(widths[a], u32::min);
                        advance - gap + 1
                    } else {
                        advance
                    } as u8);
                }
            }
            pairs
        });
        widths.iter_mut().for_each(|w| *w += 1);
        let glyphs = widths
            .iter()
            .enumerate()
            .map(|(cell, &width)| {
                let width = width.min(cell_width);
                let pixels = (0..height)
                    .flat_map(|row| (0..width).map(move |col| (col, row)))
                    .map(|(col, row)| pixel(cell as u32, col, row))
                    .collect();
                Glyph { width, advance: widths[cell], pixels }
            })
            .collect();
        Ok(Self {
            height,
            first: STRIP_FIRST,
            default_glyph: 0,
            glyphs,
            kerning: pairs,
            bold: None,
        })
    }

    /// The font with `bold` as its bold face.
    pub fn with_bold(mut self, bold: BitmapFont) -> Self {
        self.bold = Some(Box::new(bold));
        self
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Advance width of `text` in pixels.
    pub fn width(&self, text: &str, bold: bool) -> u32 {
        if let (true, Some(face)) = (bold, &self.bold) {
            return face.width(text, false);
        }
        let mut width = 0;
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            width += self.advance(c, chars.peek().copied()) + u32::from(bold);
        }
        width
    }

    /// How far the pen moves from `c` when `next` follows: by the pair
    /// where the font kerns and something is next.
    fn advance(&self, c: char, next: Option<char>) -> u32 {
        let (this, next) = (self.index(c), next.map_or(0, |n| self.index(n)));
        match &self.kerning {
            Some(pairs) if next != 0 => u32::from(pairs[this * self.glyphs.len() + next]),
            _ => self.glyphs[this].advance,
        }
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
        if let (true, Some(face)) = (bold, &self.bold) {
            return face.draw_clipped(canvas, x, y, text, color, false, clip_right);
        }
        let mut pen = x;
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let glyph = self.glyph(c);
            for dx in 0..=i32::from(bold) {
                self.blit(canvas, glyph, pen + dx, y, color, clip_right);
            }
            pen += (self.advance(c, chars.peek().copied()) + u32::from(bold)) as i32;
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

    /// The lines `text` breaks into at spaces to fit `max_width`: a line
    /// fits while it is narrower (the game's `FUN_0079c87f`).
    pub fn wrap(&self, text: &str, max_width: u32, bold: bool) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split(' ') {
            let candidate =
                if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if !line.is_empty() && self.width(&candidate, bold) >= max_width {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = candidate;
            }
        }
        lines.push(line);
        lines
    }

    fn glyph(&self, c: char) -> &Glyph {
        &self.glyphs[self.index(c)]
    }

    /// The glyph a character shows.
    fn index(&self, c: char) -> usize {
        let index = to_cp1252(c).and_then(|byte| byte.checked_sub(self.first)).map(usize::from);
        index.filter(|&i| i < self.glyphs.len()).unwrap_or(self.default_glyph)
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
                let texel = glyph.pixels[(row * glyph.width + col) as usize];
                if inside && texel.0[3] != 0 {
                    // The text's colour, tinted by the glyph and laid on
                    // at its coverage.
                    let under = canvas.get_pixel(px as u32, py as u32).0;
                    let alpha = u32::from(texel.0[3]);
                    let mut out = [0, 0, 0, 255];
                    for i in 0..3 {
                        let ink = u32::from(color.0[i]) * u32::from(texel.0[i]) / 255;
                        out[i] = ((ink * alpha + u32::from(under[i]) * (255 - alpha)) / 255) as u8;
                    }
                    canvas.put_pixel(px as u32, py as u32, Rgba(out));
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
                let on = byte & (0x80 >> (col % 8)) != 0;
                pixels.push(if on { Rgba([255; 4]) } else { Rgba([0; 4]) });
            }
        }
        glyphs.push(Glyph { width, advance: width, pixels });
    }
    if glyphs.is_empty() {
        return Err(invalid("font has no glyphs"));
    }
    let default_glyph = usize::from(default_char).min(glyphs.len() - 1);
    Ok(BitmapFont { height, first, default_glyph, glyphs, kerning: None, bold: None })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip of 5×3 cells from the space on, white where `cells` has a `#`.
    fn strip(cells: &[[&str; 3]]) -> RgbaImage {
        let mut strip = RgbaImage::from_pixel(5 * cells.len() as u32, 3, Rgba([0, 0, 0, 255]));
        for (i, rows) in cells.iter().enumerate() {
            for (y, row) in rows.iter().enumerate() {
                for (x, c) in row.chars().enumerate() {
                    if c == '#' {
                        strip.put_pixel(5 * i as u32 + x as u32, y as u32, Rgba([255; 4]));
                    }
                }
            }
        }
        strip
    }

    #[test]
    fn a_strip_measures_as_sgl_does() {
        let cells = [
            [".....", ".....", "....."], // space
            ["#....", "#....", "....."], // !: only the first column
            ["#.#..", ".#...", "#.#.."], // ": three wide
        ];
        let font = BitmapFont::from_strip(&strip(&cells), (5, 3), 0, 2, false).unwrap();
        assert_eq!(font.height(), 3);
        // A blank glyph, and one lit only in its first column, are as wide
        // as a blank one; the rest reach their rightmost pixel; each one more.
        assert_eq!(font.width(" ", false), 3);
        assert_eq!(font.width("!", false), 3);
        assert_eq!(font.width("\"", false), 3);
        assert_eq!(font.width("\" !", false), 9);
        // What the strip lacks shows as its first glyph.
        assert_eq!(font.width("Z", false), 3);
    }

    #[test]
    fn a_kerning_strip_moves_shapes_closer() {
        // From the space to the apostrophe, the one of them sgl kerns: lit
        // only in its fourth column, it is four wide with no room on its
        // right and three on its left.
        let blank = [".....", ".....", "....."];
        let mut cells = [blank; 8];
        cells[1] = ["#....", "#....", "#...."];
        cells[7] = ["...#.", "...#.", "...#."];
        let font = BitmapFont::from_strip(&strip(&cells), (5, 3), 0, 2, true).unwrap();
        // Two apostrophes close up to the room between them, plus one.
        assert_eq!(font.advance('\'', Some('\'')), 1);
        // Pairs sgl does not kern move by the width without the extra pixel;
        // the last glyph by its width.
        assert_eq!(font.advance('!', Some('\'')), 2);
        assert_eq!(font.advance('\'', Some('!')), 3);
        assert_eq!(font.advance('\'', None), 4);
    }

    #[test]
    fn bold_text_takes_the_bold_face() {
        let plain = [[".....", ".....", "....."], ["#....", ".#...", "....."]];
        let bold = [[".....", ".....", "....."], ["...#.", "...#.", "....."]];
        let font = BitmapFont::from_strip(&strip(&plain), (5, 3), 0, 2, false)
            .unwrap()
            .with_bold(BitmapFont::from_strip(&strip(&bold), (5, 3), 0, 2, false).unwrap());
        assert_eq!(font.width("!", false), 2);
        assert_eq!(font.width("!", true), 4);
    }
}
