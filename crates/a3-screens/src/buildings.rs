//! The pictures of buildings as the grounds map draws them
//! (STADION-ANSICHT.md in the RE repo): a stadium composed of loose parts
//! on its foundation, at full size and reduced as the map's zooms show it.

use std::error::Error;

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
    if look[0] != 0 && look[1] >= FIRST_DREAM {
        let path = DREAMS[usize::from(look[1] - FIRST_DREAM).min(DREAMS.len() - 1)];
        return Ok(image::load_from_memory(&game.file(path)?)?.to_rgba8());
    }
    Ok(halve(&stadium_full(game, look)?))
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
