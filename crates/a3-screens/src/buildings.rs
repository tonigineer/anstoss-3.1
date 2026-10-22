//! The pictures of buildings as the grounds map draws them
//! (STADION-ANSICHT.md in the RE repo): a stadium composed of loose parts
//! on its foundation, at full size and reduced as the map's zooms show it.

use std::error::Error;

use std::collections::HashMap;

use a3_core::grounds::{self, Grounds, View as MapView};
use image::{Rgba, RgbaImage};

use crate::game::Game;

/// Where the parts and the foundations are, and the list of the parts:
/// name, sheet, source x and bottom y, width, height, place x and bottom y.
const PARTS_LIST: &str = "buildingmaps/bauteile2.txt";
const SHEET: &str = "buildingmaps/realtime_scaled/256col_bauteile_00";
const SMALL_FOUNDATION: &str = "buildingmaps/realtime_scaled/fundament_kl.bmp";
const LARGE_FOUNDATION: &str = "buildingmaps/realtime_scaled/fundament_gr.bmp";
/// The dream stadiums' pictures, at half size, by their look 0x14 … 0x16.
const DREAMS: [&str; 3] = [
    "buildingmaps/2/karlsruhe_komplett.bmp",
    "buildingmaps/2/bamabeu_komplett.bmp",
    "buildingmaps/2/maracana_komplett.bmp",
];
const FIRST_DREAM: u8 = 0x14;
/// The sheets' see-through colour.
const CLEAR: Rgba<u8> = Rgba([0, 255, 255, 255]);
/// On the small foundation the parts move up and left.
const SMALL_SHIFT: (i32, i32) = (128, 80);
/// The stands' parts by their look (0x8af5f8): without a roof, then with.
const STANDS: [&str; 20] = [
    "Verl_od",
    "StKickers_od",
    "Manchester_od",
    "Mailand_od",
    "NouCamp_od",
    "VerlMitBahn_od",
    "Karlsruhe_Klein_od",
    "Karlsruhe_gross_od",
    "Rom_od",
    "Sydney_od",
    "Verl_md",
    "StKickers_md",
    "Manchester_md",
    "Mailand_md",
    "NouCamp_md",
    "VerlMitBahn_md",
    "Karlsruhe_Klein_md",
    "Karlsruhe_gross_md",
    "Rom_md",
    "Sydney_md",
];
const NO_STAND: u8 = 0xff;

/// A stadium at the map's middle zoom, half size, as the views show it
/// (`FUN_007c0418` with the look of `FUN_0057e9d7`).
pub fn stadium_half(game: &Game, look: &[u8; 12]) -> Result<RgbaImage, Box<dyn Error>> {
    stadium_at(game, look, 1)
}

/// A stadium at a level of the map's sheets: 0 full size, 1 half, 2 a
/// quarter (each reduction a pixel of each block, as SGL's stretch takes
/// it).
pub fn stadium_at(game: &Game, look: &[u8; 12], level: usize) -> Result<RgbaImage, Box<dyn Error>> {
    if look[0] != 0 && look[1] >= FIRST_DREAM {
        let path = DREAMS[usize::from(look[1] - FIRST_DREAM).min(DREAMS.len() - 1)];
        let path = path.replacen("/2/", &format!("/{}/", level + 1), 1);
        return Ok(image::load_from_memory(&game.file(&path)?)?.to_rgba8());
    }
    let full = stadium_full(game, look)?;
    Ok(match level {
        0 => full,
        1 => halve(&full),
        _ => quarter(&full),
    })
}

/// A quarter of the size: each 4×4 block's pixel at (2, 2).
fn quarter(full: &RgbaImage) -> RgbaImage {
    let (w, h) = (full.width() / 4, full.height() / 4);
    RgbaImage::from_fn(w, h, |x, y| *full.get_pixel(4 * x + 2, 4 * y + 2))
}

/// The stadium at full size: the foundation, then the pitch, the tracks
/// beside it and the stands, each over the last.
fn stadium_full(game: &Game, look: &[u8; 12]) -> Result<RgbaImage, Box<dyn Error>> {
    let small = look[2] == 0;
    let foundation = if small { SMALL_FOUNDATION } else { LARGE_FOUNDATION };
    let mut picture = image::load_from_memory(&game.file(foundation)?)?.to_rgba8();
    if look[0] == 0 {
        return Ok(picture);
    }
    let list = a3_files::sav::cp1252(&game.file(PARTS_LIST)?);
    let mut parts = vec!["asche_rasen_m".to_owned()];
    if look[4] != 0 {
        parts.push("asche_l".to_owned());
    }
    if look[5] != 0 {
        parts.push("asche_r".to_owned());
    }
    for (at, side) in [(8, "L"), (11, "ML"), (10, "MR"), (9, "R")] {
        if look[at] != NO_STAND
            && let Some(stand) = STANDS.get(usize::from(look[at]))
        {
            parts.push(format!("{stand}_{side}"));
        }
    }
    for part in parts {
        draw_part(game, &list, &part, &mut picture, small)?;
    }
    Ok(picture)
}

/// A part from its sheet onto the picture (`FUN_007bfdee`): the list's
/// first line of its name, its y values the bottom edges; cyan left out,
/// cut to the picture.
fn draw_part(
    game: &Game,
    list: &str,
    name: &str,
    picture: &mut RgbaImage,
    small: bool,
) -> Result<(), Box<dyn Error>> {
    let Some(fields) = list.lines().filter(|l| !l.starts_with(';')).find_map(|line| {
        let mut words = line.split_whitespace();
        let first = words.next()?;
        if !first.eq_ignore_ascii_case(name) {
            return None;
        }
        let numbers: Vec<i32> = words.take(7).filter_map(|w| w.parse().ok()).collect();
        <[i32; 7]>::try_from(numbers).ok()
    }) else {
        return Ok(());
    };
    let [sheet, sx, sy, w, h, dx, dy] = fields;
    let sheet = image::load_from_memory(&game.file(&format!("{SHEET}{sheet}.bmp"))?)?.to_rgba8();
    let shift = if small { SMALL_SHIFT } else { (0, 0) };
    let (from_x, from_y) = (sx, sy - h);
    let (to_x, to_y) = (dx - shift.0, dy - h - shift.1);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy, tx, ty) = (from_x + x, from_y + y, to_x + x, to_y + y);
            if fx < 0 || fy < 0 || tx < 0 || ty < 0 {
                continue;
            }
            let (fx, fy, tx, ty) = (fx as u32, fy as u32, tx as u32, ty as u32);
            if fx >= sheet.width() || fy >= sheet.height() {
                continue;
            }
            if tx >= picture.width() || ty >= picture.height() {
                continue;
            }
            let p = *sheet.get_pixel(fx, fy);
            if p != CLEAR {
                picture.put_pixel(tx, ty, p);
            }
        }
    }
    Ok(())
}

/// Half the size, each 2×2 block's bottom-right pixel (SGL's stretch).
fn halve(full: &RgbaImage) -> RgbaImage {
    let (w, h) = (full.width() / 2, full.height() / 2);
    RgbaImage::from_fn(w, h, |x, y| *full.get_pixel(2 * x + 1, 2 * y + 1))
}

/// The grounds map's art (STADION-ANSICHT.md §6): the patterns by name,
/// the sheets of each level as they are first needed, the stadiums.
#[derive(Default)]
pub struct MapArt {
    patterns: Option<HashMap<String, (u8, i32, i32)>>,
    sheets: HashMap<(usize, u8), RgbaImage>,
    stadiums: HashMap<([u8; 12], usize), RgbaImage>,
}

const PATTERNS: &str = "buildingmaps/patterns.txt";
const GRASS: &str = "rasengruen";
/// The tint of what is not the club's, and the see-through of what is
/// being built (§6.2).
const NOT_OWNED: u32 = 196;
const BUILDING: u32 = 0x64;
/// The minimap's colours by class, and its outline of the view (§6.6).
const MINIMAP: [[u8; 3]; 11] = [
    [0, 0, 0],
    [0x23, 0x37, 0x0a],
    [0x1e, 0x78, 0x28],
    [0x19, 0xaa, 0xfa],
    [0x46, 0x5a, 0x19],
    [0x10, 0x10, 0x10],
    [0xff, 0xe6, 0x0a],
    [0xc8, 0xaa, 0xb4],
    [0xc8, 0x28, 0x0a],
    [0xe6, 0x28, 0x14],
    [0xa0, 0xa0, 0xa0],
];
const OUTLINE: [u8; 3] = [0xc8, 0xaa, 0xb4];

impl MapArt {
    fn pattern(&mut self, game: &Game, name: &str) -> Option<(u8, i32, i32)> {
        if self.patterns.is_none() {
            let text = game.file(PATTERNS).map(|b| a3_files::sav::cp1252(&b)).unwrap_or_default();
            let mut patterns = HashMap::new();
            for line in text.lines() {
                let words: Vec<&str> = line.split_whitespace().collect();
                if let [name, sheet, x, y, ..] = words[..]
                    && let (Ok(sheet), Ok(x), Ok(y)) = (sheet.parse(), x.parse(), y.parse())
                {
                    patterns.entry(name.to_lowercase()).or_insert((sheet, x, y));
                }
            }
            self.patterns = Some(patterns);
        }
        self.patterns.as_ref()?.get(&name.to_lowercase()).copied()
    }

    fn sheet(&mut self, game: &Game, level: usize, sheet: u8) -> Option<&RgbaImage> {
        Some(self.sheets.entry((level, sheet)).or_insert_with(|| {
            let path = format!("buildingmaps/{}/256col_palette_{sheet:02}.bmp", level + 1);
            game.file(&path)
                .ok()
                .and_then(|b| image::load_from_memory(&b).ok())
                .map(|i| i.to_rgba8())
                .unwrap_or_default()
        }))
    }

    fn stadium(&mut self, game: &Game, look: [u8; 12], level: usize) -> Option<&RgbaImage> {
        Some(
            self.stadiums
                .entry((look, level))
                .or_insert_with(|| stadium_at(game, &look, level).unwrap_or_default()),
        )
    }

    /// A frame of the map (`FUN_007ba408`): the grass of every tile in
    /// view, then the buildings in the order the scan meets them.
    pub fn map(&mut self, game: &Game, grounds: &Grounds, view: &MapView, size: u32) -> RgbaImage {
        let mut frame = RgbaImage::new(size, size);
        let level = (2 - view.zoom).clamp(0, 2) as usize;
        let t = view.tile;
        let n = view.tiles;
        let ((ox, oy), (offx, offy)) = view.origin();
        let grass = self.pattern(game, GRASS);
        let mut seen = vec![false; grounds::ENTRIES];
        let mut list = Vec::new();
        for r in 0..=n {
            for c in (0..=n).rev() {
                let (x, y) = (ox + c, oy + r);
                let i = (y * grounds::SIZE as i32 + x) as usize;
                if let Some((sheet, sx, sy)) = grass {
                    let tint = if grounds.owns_at(i) { None } else { Some(NOT_OWNED) };
                    let at = (c * t - offx, r * t - offy);
                    let from = (sx >> level, sy >> level);
                    if let Some(sheet) = self.sheet(game, level, sheet) {
                        copy(&mut frame, sheet, from, at, (t, t), tint, None);
                    }
                }
                if let Some(k) = grounds.at_index(i)
                    && !seen[k]
                {
                    seen[k] = true;
                    list.push(k);
                }
            }
        }
        // Tall buildings below the view reaching up into it.
        let lim = (oy + 2 * n).min(grounds::SIZE as i32);
        for y in oy + n..lim {
            for x in ox..ox + n {
                let i = (y * grounds::SIZE as i32 + x) as usize;
                let Some(k) = grounds.at_index(i) else { continue };
                if seen[k] {
                    continue;
                }
                let (_, h, d) = self.size_of(grounds, k);
                if i32::from(grounds.entry(k).y) - (h - d) < lim {
                    seen[k] = true;
                    list.push(k);
                }
            }
        }
        for k in list {
            let e = grounds.entry(k);
            let (w, h, d) = self.size_of(grounds, k);
            let at = ((i32::from(e.x) - ox) * t - offx, (i32::from(e.y) - oy - (h - d)) * t - offy);
            let tint = if e.owned { None } else { Some(NOT_OWNED) };
            let alpha = (e.weeks != 0).then_some(BUILDING);
            if (grounds::FIRST_STADIUM..grounds::FIRST_STADIUM + 3).contains(&e.kind) {
                let look =
                    grounds.look(usize::try_from(e.kind - grounds::FIRST_STADIUM).unwrap_or(0));
                if let Some(picture) = self.stadium(game, look, level) {
                    let size = (picture.width() as i32, picture.height() as i32);
                    copy(&mut frame, picture, (0, 0), at, size, tint, alpha);
                }
                continue;
            }
            let Some(kind) = grounds::TYPES.get(usize::try_from(e.kind).unwrap_or(usize::MAX))
            else {
                continue;
            };
            let Some((sheet, sx, sy)) =
                kind.pattern(e.kind, e.frame).and_then(|p| self.pattern(game, &p))
            else {
                continue;
            };
            if let Some(sheet) = self.sheet(game, level, sheet) {
                copy(
                    &mut frame,
                    sheet,
                    (sx >> level, sy >> level),
                    at,
                    (w * t, h * t),
                    tint,
                    alpha,
                );
            }
        }
        frame
    }

    /// A building's width, height and depth in tiles; a stadium's by its
    /// size.
    fn size_of(&self, grounds: &Grounds, k: usize) -> (i32, i32, i32) {
        let kind = grounds.entry(k).kind;
        let (w, h, d) = if (grounds::FIRST_STADIUM..grounds::FIRST_STADIUM + 3).contains(&kind) {
            let look = grounds.look(usize::try_from(kind - grounds::FIRST_STADIUM).unwrap_or(0));
            if look[2] != 0 { grounds::LARGE_STADIUM } else { grounds::SMALL_STADIUM }
        } else {
            grounds::TYPES
                .get(usize::try_from(kind).unwrap_or(usize::MAX))
                .map_or((0, 0, 0), |t| t.size)
        };
        (w.into(), h.into(), d.into())
    }
}

/// The minimap (`FUN_007b9595`): a pixel a tile in its class's colour, the
/// view outlined.
pub fn minimap(grounds: &Grounds, view: &MapView) -> RgbaImage {
    let size = grounds::SIZE as u32;
    let mut picture = RgbaImage::from_fn(size, size, |x, y| {
        let class = usize::from(grounds.class(x as usize, y as usize)).min(MINIMAP.len() - 1);
        let [r, g, b] = MINIMAP[class];
        Rgba([r, g, b, if class == 0 { 0 } else { 255 }])
    });
    let ((ox, oy), _) = view.origin();
    let n = view.tiles;
    let mut dot = |x: i32, y: i32| {
        if (0..size as i32).contains(&x) && (0..size as i32).contains(&y) {
            let [r, g, b] = OUTLINE;
            picture.put_pixel(x as u32, y as u32, Rgba([r, g, b, 255]));
        }
    };
    for x in ox..ox + n {
        dot(x, oy);
        dot(x, oy + n - 1);
    }
    for y in oy..oy + n {
        dot(ox, y);
        dot(ox + n - 1, y);
    }
    picture
}

/// Part of a sheet onto the frame: cyan left out, cut to both; tinted
/// for what is not the club's, half see-through while being built.
fn copy(
    frame: &mut RgbaImage,
    from: &RgbaImage,
    (sx, sy): (i32, i32),
    (dx, dy): (i32, i32),
    (w, h): (i32, i32),
    tint: Option<u32>,
    alpha: Option<u32>,
) {
    for y in 0..h {
        for x in 0..w {
            let (fx, fy, tx, ty) = (sx + x, sy + y, dx + x, dy + y);
            if fx < 0 || fy < 0 || tx < 0 || ty < 0 {
                continue;
            }
            let (fx, fy, tx, ty) = (fx as u32, fy as u32, tx as u32, ty as u32);
            if fx >= from.width()
                || fy >= from.height()
                || tx >= frame.width()
                || ty >= frame.height()
            {
                continue;
            }
            let p = *from.get_pixel(fx, fy);
            if p == CLEAR || p.0[3] == 0 {
                continue;
            }
            let mut out = p;
            if let Some(t) = tint {
                for c in &mut out.0[..3] {
                    *c = ((u32::from(*c) * t + 127) / 255) as u8;
                }
            }
            if let Some(a) = alpha {
                let under = *frame.get_pixel(tx, ty);
                for i in 0..3 {
                    out.0[i] =
                        ((u32::from(out.0[i]) * a + u32::from(under.0[i]) * (255 - a)) / 255) as u8;
                }
            }
            out.0[3] = 255;
            frame.put_pixel(tx, ty, out);
        }
    }
}
