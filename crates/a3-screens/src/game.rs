//! Everything read from the installed game: dialogs, texts, graphics, the
//! football world, save games and the high-score list.

use std::collections::HashMap;
use std::error::Error;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use image::{ImageFormat, Rgba, RgbaImage};

use a3_core::data::Data;
use a3_core::records::Records;
use a3_files::cpr::Archive;
use a3_files::rch::{self, Dialog};

use crate::fonts::Fonts;

/// The installed game, which shows as its [`Data`] too.
pub struct Game {
    pub dir: PathBuf,
    pub dialogs: HashMap<u16, Dialog>,
    /// The world, calendar and rules, which the game cycle shares.
    pub data: Arc<Data>,
    texts: HashMap<u16, String>,
    graphics: HashMap<u16, String>,
    archive: Mutex<Archive>,
    /// The second graphics archive, `Archives/grafik1.cpr`, which holds a
    /// few the first does not (the check boxes' ticks …).
    more: Option<Mutex<Archive>>,
    /// The flags' archive, `Archives/flaggen.cpr` (`klein/Flag14.bmp` …).
    flags: Option<Mutex<Archive>>,
    /// The game's fonts, read when first drawn with.
    fonts: OnceLock<Arc<Fonts>>,
}

pub struct SaveGame {
    pub name: String,
    pub saved: SystemTime,
    pub trainer: String,
    pub club: String,
    pub country: String,
}

pub struct HighScore {
    pub name: String,
    pub age: u16,
    pub seasons: u16,
    pub points: u32,
    pub level: u8,
    pub titles: u8,
    pub date: (u8, u8, u16),
}

impl Game {
    pub fn load(dir: &Path) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            dir: dir.to_owned(),
            dialogs: rch::load_dialogs(&dir.join("deb/MeineDialoge.rch"))?,
            data: Arc::new(Data::load(dir)?),
            // The dialog file's string table, as the game reads it, over
            // the header the texts were written in.
            texts: {
                let mut texts = rch::load_texts(&dir.join("deb/ListeDerTexte.rh"))?;
                texts.extend(rch::load_string_table(&dir.join("deb/MeineDialoge.rch"))?);
                texts
            },
            graphics: rch::load_graphics(&dir.join("deb/ListeDerGrafik.rh"))?,
            archive: Mutex::new(Archive::open(&dir.join("Archives/grafik.cpr"))?),
            more: Archive::open(&dir.join("Archives/grafik1.cpr")).ok().map(Mutex::new),
            flags: Archive::open(&dir.join("Archives/flaggen.cpr")).ok().map(Mutex::new),
            fonts: OnceLock::new(),
        })
    }

    /// One of the game's texts by its id in `ListeDerTexte.rh`.
    pub fn text(&self, id: u16) -> &str {
        self.texts.get(&id).map_or("", String::as_str)
    }

    /// A text with a club's name put in where it says `v1` … `v20`: with the
    /// article or preposition its kind of name takes ("beim", "bei der",
    /// "bei den" …), from a phrase whose `1` stands for the name. A lower `v`
    /// starts the phrase small, a capital `V` capitalised.
    pub fn with_club(&self, text: &str, (slot, club): (usize, usize)) -> String {
        use a3_core::season::{ARTICLES, CLUB_PHRASES};
        let club = self.world.country(slot).and_then(|c| c.clubs.get(club));
        let name = club.map_or("", |c| c.name.as_str());
        let kind =
            club.map_or(ARTICLES - 1, |c| usize::try_from(c.article).unwrap_or(ARTICLES - 1));
        let kind = kind.min(ARTICLES - 1);
        let mut out = String::new();
        let mut rest = text;
        while let Some(at) = rest.find(['v', 'V']) {
            out.push_str(&rest[..at]);
            let capital = rest[at..].starts_with('V');
            let after = &rest[at + 1..];
            let digits = after.chars().take_while(char::is_ascii_digit).take(2).collect::<String>();
            let number: usize = digits.parse().unwrap_or(0);
            if !(1..=CLUB_PHRASES).contains(&number) {
                out.push(if capital { 'V' } else { 'v' });
                rest = after;
                continue;
            }
            let id = self.rules.club_phrases[(number - 1) * ARTICLES + kind];
            let phrase = if id == 0 { "1" } else { self.text(id as u16) };
            let phrase = if phrase.starts_with('1') {
                phrase.to_owned()
            } else {
                let mut chars = phrase.chars();
                let first = chars.next().map(|c| {
                    if capital {
                        c.to_uppercase().collect::<String>()
                    } else {
                        c.to_lowercase().collect()
                    }
                });
                first.unwrap_or_default() + chars.as_str()
            };
            match phrase.split_once('1') {
                Some((before, behind)) => out.push_str(&format!("{before}{name}{behind}")),
                None => out.push_str(&format!("{phrase} {name}")),
            }
            rest = &after[digits.len()..];
        }
        out.push_str(rest);
        out
    }

    /// A club's name by its country and number (`FUN_0063e52f`): none,
    /// the national team, a team of the country, or what the save does not
    /// name yet — an amateur club, a club abroad.
    pub fn club_name(&self, records: Option<&Records>, country: u8, number: i16) -> String {
        const NO_CLUB: u16 = 4079;
        const AMATEUR: u16 = 3491;
        const ABROAD: u16 = 3492;
        const NATIONAL: i16 = 200;
        /// A country of the first kind has forty amateur clubs after its teams.
        const AMATEUR_CLUBS: i16 = 40;
        if country == 0 {
            return self.text(NO_CLUB).to_owned();
        }
        if number == NATIONAL {
            return self.world.nation(country.into()).to_owned();
        }
        let Some(records) = records else { return String::new() };
        let countries = &records.countries;
        let teams = countries.team_count(country.into());
        let team = u16::try_from(number).ok().filter(|_| number < teams);
        match (countries.kind(country.into()), team) {
            (1 | 2, Some(number)) => {
                records.team(country.into(), number).map_or_else(String::new, |t| t.name())
            }
            // The amateur clubs' and the clubs abroad's names are not read yet.
            (1, None) if number < teams + AMATEUR_CLUBS => self.text(AMATEUR).to_owned(),
            (1 | 2, None) => self.text(AMATEUR).to_owned(),
            _ => self.text(ABROAD).to_owned(),
        }
    }

    /// A graphic by its id in `ListeDerGrafik.rh`, from `Archives/grafik.cpr`
    /// or the archives after it.
    pub fn graphic(&self, id: u16) -> Result<RgbaImage, Box<dyn Error>> {
        let path = self.graphics.get(&id).ok_or_else(|| format!("no graphic {id}"))?;
        let bytes = self.read(path)?;
        let format = if path.to_lowercase().ends_with(".bmp") {
            ImageFormat::Bmp
        } else {
            ImageFormat::Jpeg
        };
        Ok(image::load_from_memory_with_format(&bytes, format)?.to_rgba8())
    }

    /// A file of the game by its relative path, which ignores case as
    /// Windows does.
    pub fn file(&self, path: &str) -> std::io::Result<Vec<u8>> {
        let mut found = self.dir.clone();
        for part in path.split('/') {
            let exact = found.join(part);
            found = if exact.exists() {
                exact
            } else {
                std::fs::read_dir(&found)?
                    .flatten()
                    .map(|entry| entry.path())
                    .find(|p| p.file_name().is_some_and(|f| f.eq_ignore_ascii_case(part)))
                    .unwrap_or(exact)
            };
        }
        std::fs::read(found)
    }

    /// A file from the first archive that holds it.
    fn read(&self, path: &str) -> std::io::Result<Vec<u8>> {
        let mut found = self.archive.lock().unwrap().read(path);
        for other in [&self.more, &self.flags].into_iter().flatten() {
            if found.is_err() {
                found = other.lock().unwrap().read(path);
            }
        }
        found
    }

    /// A graphic from the archives by its path.
    pub fn picture(&self, path: &str) -> Result<RgbaImage, Box<dyn Error>> {
        let bytes = self.read(path)?;
        // A TGA does not say what it is.
        let picture = if path.to_lowercase().ends_with(".tga") {
            image::load_from_memory_with_format(&bytes, ImageFormat::Tga)?
        } else {
            image::load_from_memory(&bytes)?
        };
        Ok(picture.to_rgba8())
    }

    /// The fonts the game draws its dialogs' text with.
    pub fn fonts(&self) -> Arc<Fonts> {
        self.fonts.get_or_init(|| Arc::new(Fonts::load(self))).clone()
    }

    /// A button bitmap, with its black background made transparent.
    pub fn sprite(&self, id: u16) -> Result<RgbaImage, Box<dyn Error>> {
        Ok(transparent_black(self.graphic(id)?))
    }

    /// The save games in `saves/`, newest first. Their header holds the
    /// trainer's first and last name, the club and its country.
    pub fn save_games(&self) -> Vec<SaveGame> {
        let Ok(entries) = std::fs::read_dir(self.dir.join("saves")) else { return Vec::new() };
        let mut saves: Vec<SaveGame> = entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext.eq_ignore_ascii_case("an3")))
            .filter_map(|entry| {
                let header = read_head(&entry.path(), 0x48)?;
                let field = |range: std::ops::Range<usize>| cstring(&header[range]);
                Some(SaveGame {
                    name: entry.path().file_stem()?.to_string_lossy().into_owned(),
                    saved: entry.metadata().ok()?.modified().ok()?,
                    trainer: format!("{} {}", field(0x06..0x10), field(0x10..0x20)),
                    club: field(0x20..0x34),
                    country: field(0x34..0x48),
                })
            })
            .collect();
        saves.sort_by_key(|save| std::cmp::Reverse(save.saved));
        saves
    }

    /// `Highscore/high09`: 48-byte entries of first and last name, age,
    /// seasons, points, manager level, titles and the date.
    pub fn high_scores(&self) -> Vec<HighScore> {
        let Ok(data) = std::fs::read(self.dir.join("Highscore/high09")) else { return Vec::new() };
        data.as_chunks::<48>()
            .0
            .iter()
            .map(|e| HighScore {
                name: format!("{} {}", cstring(&e[0..10]), cstring(&e[10..26])),
                age: u16::from_le_bytes([e[26], e[27]]),
                seasons: u16::from_le_bytes([e[28], e[29]]),
                points: u32::from_le_bytes([e[32], e[33], e[34], e[35]]),
                level: e[36],
                titles: e[37],
                date: (e[42], e[43], u16::from_le_bytes([e[44], e[45]])),
            })
            .collect()
    }
}

impl Deref for Game {
    type Target = Data;

    fn deref(&self) -> &Data {
        &self.data
    }
}

/// Bitmaps the game draws over the background leave out their black pixels.
pub fn transparent_black(mut sprite: RgbaImage) -> RgbaImage {
    for pixel in sprite.pixels_mut() {
        if pixel.0[..3] == [0, 0, 0] {
            *pixel = Rgba([0, 0, 0, 0]);
        }
    }
    sprite
}

/// Where derived images are kept between runs: `$XDG_CACHE_HOME/anstoss3-menu`,
/// or `~/.cache/anstoss3-menu`.
pub fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(&std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
    base.join("anstoss3-menu")
}

fn read_head(path: &Path, len: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut head = vec![0; len];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    Some(head)
}

fn cstring(bytes: &[u8]) -> String {
    bytes.iter().take_while(|&&b| b != 0).map(|&b| char::from(b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn club_names_take_their_article() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let game = Game::load(&dir).unwrap();
        let germany = game.world.country(0).unwrap();
        let club = |name: &str| germany.clubs.iter().position(|c| c.name == name).unwrap();
        let text = game.text(1545);
        assert_eq!(
            game.with_club(text, (0, club("FC Bayern München"))),
            "Sie sind jetzt Manager beim FC Bayern München."
        );
        assert_eq!(
            game.with_club(text, (0, club("SpVgg Unterhaching"))),
            "Sie sind jetzt Manager bei der SpVgg Unterhaching."
        );
    }
}
