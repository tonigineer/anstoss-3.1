//! The game's fonts, as `anstoss3.exe` builds them when it starts (its
//! table at 0x8adcb8): strips of glyphs in `grafik.cpr`, 224 characters
//! from the space on, measured and drawn by `sgl`
//! ([`BitmapFont::from_strip`]). The dialog file numbers them
//! ([`a3_files::rch::PLAIN_FONT`] …).

use std::path::Path;

use tracing::warn;

use a3_files::bitmap_font::BitmapFont;
use a3_files::rch::{BOLD_FONT, FONTS, PLAIN_FONT};

use crate::game::Game;

/// Wine's MS Sans Serif for 96 dpi and code page 1252, where the game's
/// fonts cannot be read.
const MS_SANS_SERIF: &str = "/usr/share/wine/fonts/sserife.fon";
/// 8 pt at 96 dpi, the size of Windows dialogs.
const CELL_HEIGHT: u32 = 13;

/// One font of the table: its strip, the size of a glyph's cell, the
/// pixels added to every glyph and a blank one's width, and whether it
/// kerns.
struct Face {
    file: &'static str,
    cell: (u32, u32),
    extra: u8,
    blank: u8,
    kerning: bool,
}

const TABLE: [Face; FONTS as usize] = [
    Face { file: "MSSans8_Neu.bmp", cell: (14, 13), extra: 0, blank: 2, kerning: false },
    Face { file: "MSSans8_Neu.bmp", cell: (14, 13), extra: 0, blank: 2, kerning: false },
    Face { file: "MSSans10_Neu.bmp", cell: (14, 16), extra: 0, blank: 2, kerning: false },
    Face { file: "font15_18px.tga", cell: (14, 18), extra: 0, blank: 4, kerning: false },
    Face { file: "MSSans8Kursiv.bmp", cell: (14, 13), extra: 1, blank: 0, kerning: true },
    Face { file: "MSSansBold8.bmp", cell: (14, 13), extra: 0, blank: 2, kerning: false },
];

/// The six fonts, each with the bold one as its bold face.
pub struct Fonts {
    faces: Vec<Option<BitmapFont>>,
}

impl Fonts {
    pub fn load(game: &Game) -> Self {
        let face = |face: &Face| {
            let strip = game.picture(&face.file.to_lowercase()).map_err(|e| e.to_string());
            strip
                .and_then(|strip| {
                    let font = BitmapFont::from_strip(
                        &strip,
                        face.cell,
                        face.extra,
                        face.blank,
                        face.kerning,
                    );
                    font.map_err(|e| e.to_string())
                })
                .inspect_err(|err| warn!("{}: {err}", face.file))
                .ok()
        };
        let mut faces: Vec<Option<BitmapFont>> = TABLE.iter().map(face).collect();
        if faces[usize::from(PLAIN_FONT)].is_none() {
            faces[usize::from(PLAIN_FONT)] =
                BitmapFont::load(Path::new(MS_SANS_SERIF), CELL_HEIGHT)
                    .inspect_err(|err| {
                        warn!("{MS_SANS_SERIF}: {err}; the classic look has no text")
                    })
                    .ok();
        }
        let bold = faces[usize::from(BOLD_FONT)].clone();
        let faces = faces
            .into_iter()
            .enumerate()
            .map(|(i, font)| match (font, &bold) {
                (Some(font), Some(bold)) if i != usize::from(BOLD_FONT) => {
                    Some(font.with_bold(bold.clone()))
                }
                (font, _) => font,
            })
            .collect();
        Self { faces }
    }

    /// Font `number` of the dialog file, or the dialog font where it has
    /// none.
    pub fn get(&self, number: u8) -> Option<&BitmapFont> {
        let plain = || self.faces[usize::from(PLAIN_FONT)].as_ref();
        self.faces.get(usize::from(number)).and_then(Option::as_ref).or_else(plain)
    }
}
