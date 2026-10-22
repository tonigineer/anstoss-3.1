//! The club grounds (dialog 466, `CMapDialog`; STADION-ANSICHT.md §4–7 in
//! the RE repo): a map of 100×100 tiles with the club's land and the
//! buildings on it, cached for up to ten clubs in the save; and the 103
//! kinds of building the game knows.

use crate::records::Records;

/// The cached grounds' keys (u8 country, a byte, i16 number) and the
/// grounds themselves, one block each (`0x8ef338`, `0x8ef380[k]`).
const KEYS: &str = "0x8ef338";
const KEY: usize = 4;
const CACHED: usize = 10;
/// The grounds block: the entries, their count, the tile grid of entry
/// indices, the club's land, the minimap's classes, the stadiums' looks,
/// the view, the investable amount.
pub const SIZE: usize = 100;
pub const ENTRIES: usize = 10_000;
const ENTRY: usize = 8;
const COUNT: usize = 0x13880;
const GRID: usize = 0x13884;
const OWNED: usize = 0x186a4;
const CLASS: usize = 0x1adb4;
const LOOKS: usize = 0x1d5e0;
const SCROLL: usize = 0x1d608;
const TILE: usize = 0x1fd44;
const INVESTABLE: usize = 0x1fd60;
/// The stadiums' kinds of building (90, 91, 92).
pub const FIRST_STADIUM: i16 = 90;
pub const STADIUMS: usize = 3;
/// A small and a large stadium's footprint and height, in tiles.
pub const SMALL_STADIUM: (u8, u8, u8) = (19, 12, 12);
pub const LARGE_STADIUM: (u8, u8, u8) = (27, 17, 17);

/// Where a kind of building is offered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Landscape,
    Club,
    Entertainment,
}

/// A kind of building (`FUN_007bb71b`'s record): whether it counts as a
/// building, its name's text, its pictures' file pattern, its width,
/// height and depth in tiles, where it is offered, its minimap class, its
/// sound, its frames and how often it moves, its price, weeks of building,
/// resale percentage, upkeep and rent a season, whether it can be sold,
/// and the staff it needs by kind.
#[derive(Clone, Copy, Debug)]
pub struct BuildingType {
    pub building: bool,
    pub text: u16,
    pub file: Option<&'static str>,
    pub size: (u8, u8, u8),
    pub category: Option<Category>,
    pub class: u8,
    pub sound: u8,
    pub frames: u8,
    pub animation: u8,
    pub price: i32,
    pub weeks: u8,
    pub resale: u8,
    pub upkeep: i32,
    pub rent: i32,
    pub sellable: bool,
    pub staff: &'static [(u8, u8)],
}

/// A road's frames by its neighbours (`FUN_007bf409`): the file's own
/// first, then these.
const ROAD_FRAMES: [&str; 10] =
    ["L-R", "L-R-O-U", "L-O", "O-R", "L-U", "R-U", "L-R-O", "L-R-U", "L-O-U", "O-R-U"];
const ROADS: [i16; 3] = [2, 3, 93];
const FIELD_PATH: i16 = 3;

impl BuildingType {
    /// The pattern a frame draws (§6.3): the file pattern with the frame's
    /// number, without its ".bmp"; a road's by its neighbours.
    pub fn pattern(&self, kind: i16, frame: u8) -> Option<String> {
        let file = self.file?.strip_suffix(".bmp")?;
        if ROADS.contains(&kind) {
            let base = file.strip_suffix("-O-U")?;
            return Some(match frame {
                0 => file.to_owned(),
                10 if kind == FIELD_PATH => format!("{base}-R-O-U"),
                f => format!("{base}-{}", ROAD_FRAMES.get(usize::from(f) - 1)?),
            });
        }
        Some(if file.contains("%d") {
            file.replace("%d", &frame.to_string())
        } else {
            file.to_owned()
        })
    }
}

/// A building on the map: its kind, the top left of its footprint, its
/// frame, whether it stands on the club's land, whether the entry is in
/// use, its weeks of building left.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry {
    pub kind: i16,
    pub x: u8,
    pub y: u8,
    pub frame: u8,
    pub owned: bool,
    pub used: bool,
    pub weeks: u8,
}

/// A club's grounds, as cached in the save.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Grounds {
    pub slot: usize,
    pub data: Vec<u8>,
}

/// The view as kept with the grounds: the scroll in 1/32 tiles, the
/// first tile shown and the pixel offset by axis; the tile's size, the
/// zoom and the tiles in view.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct View {
    pub scroll: (i32, i32),
    pub tile: i32,
    pub zoom: i32,
    pub tiles: i32,
}

fn word(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}

fn long(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

impl Grounds {
    /// A club's cached grounds (`FUN_0057e4f0`).
    pub fn of(records: &Records, country: u8, number: i16) -> Option<Self> {
        let keys = records.block(KEYS)?;
        let slot = (0..CACHED).find(|&k| {
            keys.get(k * KEY..(k + 1) * KEY)
                .is_some_and(|b| b[0] == country && word(b, 2) == number && b[0] != 0)
        })?;
        let data = records.block(&format!("0x8ef380[{slot}]"))?.to_vec();
        (data.len() >= INVESTABLE + 8).then_some(Self { slot, data })
    }

    /// Back into the save (`FUN_0057e590`).
    pub fn store(&self, records: &mut Records) {
        if let Some(block) = records.block_mut(&format!("0x8ef380[{}]", self.slot)) {
            block.copy_from_slice(&self.data);
        }
    }

    pub fn entry(&self, k: usize) -> Entry {
        let b = &self.data[k * ENTRY..(k + 1) * ENTRY];
        Entry {
            kind: word(b, 0),
            x: b[2],
            y: b[3],
            frame: b[4],
            owned: b[5] != 0,
            used: b[6] != 0,
            weeks: b[7],
        }
    }

    /// The entry of a tile by its index in the grid, read as the game
    /// reads it: past the grid's end into what follows it.
    pub fn at_index(&self, i: usize) -> Option<usize> {
        let at = GRID + 2 * i;
        let k = word(self.data.get(at..at + 2)?, 0);
        usize::try_from(k).ok().filter(|&k| k < ENTRIES)
    }

    /// Whether a tile by its index is the club's, read likewise.
    pub fn owns_at(&self, i: usize) -> bool {
        self.data.get(OWNED + i).is_some_and(|&b| b != 0)
    }

    pub fn count(&self) -> i32 {
        long(&self.data, COUNT)
    }

    /// The entry covering a tile, if any.
    pub fn at(&self, x: usize, y: usize) -> Option<usize> {
        let k = word(&self.data, GRID + 2 * (y * SIZE + x));
        usize::try_from(k).ok().filter(|&k| k < ENTRIES)
    }

    /// Whether the club owns a tile.
    pub fn owns(&self, x: usize, y: usize) -> bool {
        self.data[OWNED + y * SIZE + x] != 0
    }

    /// A tile's minimap class.
    pub fn class(&self, x: usize, y: usize) -> u8 {
        self.data[CLASS + y * SIZE + x]
    }

    /// The stadiums' looks as 464 last wrote them.
    pub fn set_looks(&mut self, looks: &[[u8; 12]; STADIUMS]) {
        for (k, look) in looks.iter().enumerate() {
            self.data[LOOKS + 12 * k..LOOKS + 12 * (k + 1)].copy_from_slice(look);
        }
    }

    pub fn look(&self, k: usize) -> [u8; 12] {
        self.data[LOOKS + 12 * k..LOOKS + 12 * (k + 1)].try_into().unwrap()
    }

    pub fn view(&self) -> View {
        View {
            scroll: (long(&self.data, SCROLL), long(&self.data, SCROLL + 4)),
            tile: long(&self.data, TILE),
            zoom: long(&self.data, TILE + 4),
            tiles: long(&self.data, TILE + 8),
        }
    }

    /// The view kept: the scroll, the first tiles and offsets it gives,
    /// the zoom.
    pub fn set_view(&mut self, view: &View) {
        let (sx, sy) = view.scroll;
        let fields = [sx, sy, sx >> 5, offset(sx, view.zoom), sy >> 5, offset(sy, view.zoom)];
        for (i, v) in fields.into_iter().enumerate() {
            self.data[SCROLL + 4 * i..SCROLL + 4 * (i + 1)].copy_from_slice(&v.to_le_bytes());
        }
        for (i, v) in [view.tile, view.zoom, view.tiles].into_iter().enumerate() {
            self.data[TILE + 4 * i..TILE + 4 * (i + 1)].copy_from_slice(&v.to_le_bytes());
        }
    }

    pub fn set_investable(&mut self, amount: i64) {
        self.data[INVESTABLE..INVESTABLE + 8].copy_from_slice(&amount.to_le_bytes());
    }
}

/// The tile's size and the tiles in view by zoom (§6.1).
pub const TILES: [(i32, i32); 3] = [(8, 56), (16, 28), (32, 14)];

/// A scroll's pixel offset into its first tile: zoom 0 divides by three,
/// not four (§6.1).
pub fn offset(scroll: i32, zoom: i32) -> i32 {
    (scroll & 31) / (3 - zoom)
}

impl View {
    /// The view at another zoom, keeping its centre, held to the map
    /// (`FUN_007c5ed0`, `FUN_007c5f96`).
    pub fn zoomed(&self, zoom: i32) -> Self {
        let (tile, tiles) = TILES[zoom.clamp(0, 2) as usize];
        let shift = (self.tiles - tiles) * 16;
        let mut view =
            Self { scroll: (self.scroll.0 + shift, self.scroll.1 + shift), tile, zoom, tiles };
        view.clamp();
        view
    }

    pub fn clamp(&mut self) {
        let most = (SIZE as i32 - self.tiles) * 32;
        self.scroll = (self.scroll.0.clamp(0, most), self.scroll.1.clamp(0, most));
    }

    /// The first tile shown and the pixel offset, by axis.
    pub fn origin(&self) -> ((i32, i32), (i32, i32)) {
        let (sx, sy) = self.scroll;
        ((sx >> 5, sy >> 5), (offset(sx, self.zoom), offset(sy, self.zoom)))
    }
}

pub const TYPES: [BuildingType; 103] = [
    BuildingType {
        building: false,
        text: 4814,
        file: Some("see-%d.bmp"),
        size: (4, 2, 2),
        category: Some(Category::Landscape),
        class: 3,
        sound: 4,
        frames: 5,
        animation: 100,
        price: 2000000,
        weeks: 0,
        resale: 15,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 0 See (groß)
    BuildingType {
        building: true,
        text: 4815,
        file: Some("park-%d.bmp"),
        size: (8, 5, 5),
        category: Some(Category::Landscape),
        class: 3,
        sound: 4,
        frames: 22,
        animation: 100,
        price: 500000,
        weeks: 0,
        resale: 15,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[(16, 1)],
    }, // 1 Park
    BuildingType {
        building: false,
        text: 4816,
        file: Some("beton-O-U.bmp"),
        size: (1, 1, 1),
        category: Some(Category::Landscape),
        class: 5,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 20000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 2 Straße
    BuildingType {
        building: false,
        text: 4817,
        file: Some("feld-O-U.bmp"),
        size: (1, 1, 1),
        category: Some(Category::Landscape),
        class: 5,
        sound: 5,
        frames: 0,
        animation: 0,
        price: 5000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 3 Feldweg
    BuildingType {
        building: true,
        text: 4818,
        file: Some("hotelmitrauch-%d.bmp"),
        size: (3, 2, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 4,
        animation: 30,
        price: 1500000,
        weeks: 36,
        resale: 70,
        upkeep: 0,
        rent: 100000,
        sellable: true,
        staff: &[(38, 1), (4, 1), (9, 3), (3, 2), (29, 2), (0, 1)],
    }, // 4 Hotel
    BuildingType {
        building: false,
        text: 4819,
        file: Some("baum%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Landscape),
        class: 2,
        sound: 2,
        frames: 21,
        animation: 0,
        price: 1000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 5 Laubbaum
    BuildingType {
        building: false,
        text: 4820,
        file: Some("Nadelbaum%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Landscape),
        class: 2,
        sound: 2,
        frames: 4,
        animation: 0,
        price: 1000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 6 Nadelbaum
    BuildingType {
        building: true,
        text: 4821,
        file: Some("haus1-%d.bmp"),
        size: (2, 3, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 1500000,
        weeks: 36,
        resale: 60,
        upkeep: 0,
        rent: 60000,
        sellable: true,
        staff: &[(17, 1)],
    }, // 7 Mehrfamilienhaus
    BuildingType {
        building: true,
        text: 4822,
        file: Some("hochhaus1-%d.bmp"),
        size: (2, 4, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 8000000,
        weeks: 72,
        resale: 70,
        upkeep: 0,
        rent: 300000,
        sellable: true,
        staff: &[(17, 1)],
    }, // 8 Hochhaus (3)
    BuildingType {
        building: true,
        text: 4823,
        file: Some("hochhaus2-%d.bmp"),
        size: (3, 9, 3),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 80000000,
        weeks: 96,
        resale: 70,
        upkeep: 0,
        rent: 3000000,
        sellable: true,
        staff: &[(17, 1)],
    }, // 9 Hochhaus (4)
    BuildingType {
        building: true,
        text: 4824,
        file: Some("hochhaus3-%d.bmp"),
        size: (4, 3, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 2500000,
        weeks: 48,
        resale: 70,
        upkeep: 0,
        rent: 100000,
        sellable: true,
        staff: &[(17, 1)],
    }, // 10 Hochhaus (1)
    BuildingType {
        building: true,
        text: 4825,
        file: Some("hochhaus4-%d.bmp"),
        size: (3, 13, 3),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 200000000,
        weeks: 144,
        resale: 70,
        upkeep: 0,
        rent: 8000000,
        sellable: true,
        staff: &[(17, 2)],
    }, // 11 Hochhaus (5)
    BuildingType {
        building: true,
        text: 4826,
        file: Some("hochhaus5-%d.bmp"),
        size: (4, 3, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 4000000,
        weeks: 60,
        resale: 70,
        upkeep: 0,
        rent: 150000,
        sellable: true,
        staff: &[(17, 2)],
    }, // 12 Hochhaus (2)
    BuildingType {
        building: true,
        text: 4827,
        file: Some("rehaklinik-%d.bmp"),
        size: (4, 3, 2),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 2,
        animation: 100,
        price: 1500000,
        weeks: 48,
        resale: 20,
        upkeep: 1000000,
        rent: 0,
        sellable: true,
        staff: &[(43, 1), (41, 2), (10, 3), (6, 1)],
    }, // 13 Rehaklinik
    BuildingType {
        building: true,
        text: 4828,
        file: Some("kirche-%d.bmp"),
        size: (2, 2, 1),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 3,
        frames: 0,
        animation: 0,
        price: 2000000,
        weeks: 24,
        resale: 80,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[(13, 1), (6, 1)],
    }, // 14 Kirche
    BuildingType {
        building: true,
        text: 4829,
        file: Some("präsidentenpalast-%d.bmp"),
        size: (5, 5, 4),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 7,
        animation: 100,
        price: 10000000,
        weeks: 48,
        resale: 20,
        upkeep: 100000,
        rent: 0,
        sellable: true,
        staff: &[(32, 1), (28, 1), (17, 1), (16, 1), (6, 1)],
    }, // 15 Präsidentenpalast
    BuildingType {
        building: true,
        text: 4830,
        file: Some("fanshop-%d.bmp"),
        size: (2, 3, 2),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 4,
        animation: 100,
        price: 800000,
        weeks: 24,
        resale: 10,
        upkeep: 20000,
        rent: 0,
        sellable: true,
        staff: &[(5, 3), (37, 1)],
    }, // 16 Fanshop
    BuildingType {
        building: true,
        text: 4831,
        file: Some("bierzelt-%d.bmp"),
        size: (3, 2, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 6,
        frames: 5,
        animation: 100,
        price: 120000,
        weeks: 4,
        resale: 80,
        upkeep: 0,
        rent: 50000,
        sellable: true,
        staff: &[(31, 1), (11, 4), (23, 2), (6, 1)],
    }, // 17 Bierzelt
    BuildingType {
        building: false,
        text: 4832,
        file: Some("berg-%d.bmp"),
        size: (8, 6, 4),
        category: Some(Category::Landscape),
        class: 10,
        sound: 2,
        frames: 0,
        animation: 0,
        price: 5000000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 18 Künstlicher Berg
    BuildingType {
        building: true,
        text: 4833,
        file: Some("chinahaus-%d.bmp"),
        size: (4, 3, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 7,
        frames: 3,
        animation: 100,
        price: 300000,
        weeks: 4,
        resale: 75,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[(13, 1)],
    }, // 19 China-Haus
    BuildingType {
        building: true,
        text: 4834,
        file: Some("chinahausturm-%d.bmp"),
        size: (2, 4, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 7,
        frames: 4,
        animation: 100,
        price: 250000,
        weeks: 8,
        resale: 75,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[(13, 1)],
    }, // 20 China-Turm
    BuildingType {
        building: true,
        text: 4835,
        file: Some("vereinsheim-%d.bmp"),
        size: (4, 4, 3),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 4,
        animation: 100,
        price: 800000,
        weeks: 32,
        resale: 30,
        upkeep: 50000,
        rent: 0,
        sellable: true,
        staff: &[(4, 1), (26, 1), (6, 1)],
    }, // 21 Vereinsheim
    BuildingType {
        building: false,
        text: 4836,
        file: Some("felsbrocken-%d.bmp"),
        size: (1, 1, 1),
        category: Some(Category::Landscape),
        class: 10,
        sound: 2,
        frames: 2,
        animation: 0,
        price: 5000,
        weeks: 0,
        resale: 5,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 22 Felsbrocken
    BuildingType {
        building: true,
        text: 4837,
        file: Some("dopinglabor-%d.bmp"),
        size: (3, 3, 2),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 4,
        animation: 100,
        price: 1500000,
        weeks: 24,
        resale: 20,
        upkeep: 200000,
        rent: 0,
        sellable: true,
        staff: &[(39, 3), (14, 3), (6, 1)],
    }, // 23 Doping-Labor
    BuildingType {
        building: true,
        text: 4838,
        file: Some("infostand-%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 3,
        animation: 100,
        price: 75000,
        weeks: 4,
        resale: 10,
        upkeep: 50000,
        rent: 0,
        sellable: true,
        staff: &[(19, 2)],
    }, // 24 Infostand
    BuildingType {
        building: true,
        text: 4839,
        file: Some("pommesbude-%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 4,
        animation: 20,
        price: 50000,
        weeks: 4,
        resale: 80,
        upkeep: 0,
        rent: 25000,
        sellable: true,
        staff: &[(5, 1)],
    }, // 25 Pommes-Bude
    BuildingType {
        building: true,
        text: 4840,
        file: Some("wuerstchenbude-%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 5,
        animation: 100,
        price: 50000,
        weeks: 4,
        resale: 80,
        upkeep: 0,
        rent: 25000,
        sellable: true,
        staff: &[(5, 2)],
    }, // 26 Würstchenbude
    BuildingType {
        building: true,
        text: 4841,
        file: Some("jugendinternat-%d.bmp"),
        size: (8, 6, 5),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 5,
        animation: 8,
        price: 2500000,
        weeks: 48,
        resale: 25,
        upkeep: 500000,
        rent: 0,
        sellable: true,
        staff: &[(36, 3), (17, 1)],
    }, // 27 Jugendinternat
    BuildingType {
        building: true,
        text: 4842,
        file: Some("trainingsplatzRasen-%d.bmp"),
        size: (8, 5, 5),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 7,
        animation: 40,
        price: 400000,
        weeks: 12,
        resale: 25,
        upkeep: 40000,
        rent: 0,
        sellable: true,
        staff: &[(12, 1)],
    }, // 28 Trainingsplatz (Rasen)
    BuildingType {
        building: true,
        text: 4843,
        file: Some("trainingsplatzAsche-%d.bmp"),
        size: (8, 5, 5),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 4,
        animation: 40,
        price: 200000,
        weeks: 12,
        resale: 25,
        upkeep: 20000,
        rent: 0,
        sellable: true,
        staff: &[(12, 1)],
    }, // 29 Trainingsplatz (Asche)
    BuildingType {
        building: true,
        text: 4844,
        file: Some("trainingsplatzSand-%d.bmp"),
        size: (7, 5, 5),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 4,
        animation: 40,
        price: 300000,
        weeks: 12,
        resale: 25,
        upkeep: 20000,
        rent: 0,
        sellable: true,
        staff: &[(12, 1)],
    }, // 30 Trainingsplatz (Sand)
    BuildingType {
        building: true,
        text: 4845,
        file: Some("kassenhaus-%d.bmp"),
        size: (4, 3, 2),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 100000,
        weeks: 4,
        resale: 10,
        upkeep: 0,
        rent: 20000,
        sellable: true,
        staff: &[(7, 2)],
    }, // 31 Kassenhaus
    BuildingType {
        building: false,
        text: 4846,
        file: Some("blumen-%d.bmp"),
        size: (1, 1, 1),
        category: Some(Category::Landscape),
        class: 2,
        sound: 2,
        frames: 7,
        animation: 0,
        price: 500,
        weeks: 0,
        resale: 15,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 32 Blumenbeet
    BuildingType {
        building: false,
        text: 4847,
        file: Some("palme-%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Landscape),
        class: 2,
        sound: 2,
        frames: 2,
        animation: 0,
        price: 2000,
        weeks: 0,
        resale: 15,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 33 Palme
    BuildingType {
        building: true,
        text: 4848,
        file: Some("triumphbogen-%d.bmp"),
        size: (3, 3, 2),
        category: Some(Category::Club),
        class: 6,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 500000,
        weeks: 12,
        resale: 75,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 34 Triumphbogen
    BuildingType {
        building: true,
        text: 4849,
        file: Some("pizza-%d.bmp"),
        size: (2, 2, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 6,
        animation: 100,
        price: 150000,
        weeks: 12,
        resale: 80,
        upkeep: 0,
        rent: 40000,
        sellable: true,
        staff: &[(37, 1), (29, 2), (5, 2)],
    }, // 35 Pizzeria
    BuildingType {
        building: false,
        text: 4850,
        file: Some("seemitnessi-%d.bmp"),
        size: (2, 2, 2),
        category: Some(Category::Landscape),
        class: 3,
        sound: 4,
        frames: 7,
        animation: 2,
        price: 1000000,
        weeks: 0,
        resale: 15,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 36 See (klein)
    BuildingType {
        building: true,
        text: 4851,
        file: Some("trainingshalle-%d.bmp"),
        size: (5, 4, 3),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 4,
        animation: 10,
        price: 5000000,
        weeks: 48,
        resale: 25,
        upkeep: 200000,
        rent: 0,
        sellable: true,
        staff: &[(17, 1), (6, 1)],
    }, // 37 Trainingshalle
    BuildingType {
        building: true,
        text: 4852,
        file: Some("180gradkino-%d.bmp"),
        size: (4, 4, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 6,
        animation: 100,
        price: 1500000,
        weeks: 24,
        resale: 85,
        upkeep: 0,
        rent: 150000,
        sellable: true,
        staff: &[(7, 1), (20, 1), (8, 1), (6, 1)],
    }, // 38 180-Grad-Kino
    BuildingType {
        building: false,
        text: 4853,
        file: Some("betonmischer-%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 10,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 39 Betonmischer
    BuildingType {
        building: true,
        text: 4854,
        file: Some("fernsehstation-%d.bmp"),
        size: (4, 4, 2),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 8000000,
        weeks: 48,
        resale: 75,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[(30, 2), (24, 2), (21, 2), (22, 2), (6, 1)],
    }, // 40 Fernsehstation
    BuildingType {
        building: true,
        text: 4855,
        file: Some("eisdiele-%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 4,
        animation: 100,
        price: 50000,
        weeks: 4,
        resale: 80,
        upkeep: 0,
        rent: 25000,
        sellable: true,
        staff: &[(5, 2)],
    }, // 41 Eisdiele
    BuildingType {
        building: true,
        text: 4856,
        file: Some("disco-%d.bmp"),
        size: (3, 3, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 6,
        frames: 4,
        animation: 100,
        price: 700000,
        weeks: 24,
        resale: 80,
        upkeep: 0,
        rent: 120000,
        sellable: true,
        staff: &[(37, 1), (34, 2), (15, 1), (11, 4), (6, 1), (23, 2)],
    }, // 42 Disco
    BuildingType {
        building: true,
        text: 4857,
        file: Some("museum-%d.bmp"),
        size: (4, 3, 2),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 2000000,
        weeks: 24,
        resale: 18,
        upkeep: 20000,
        rent: 0,
        sellable: true,
        staff: &[(7, 1), (13, 1), (6, 1)],
    }, // 43 Fußballmuseum
    BuildingType {
        building: false,
        text: 4858,
        file: Some("brunnen-%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Landscape),
        class: 3,
        sound: 4,
        frames: 2,
        animation: 100,
        price: 20000,
        weeks: 0,
        resale: 18,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 44 Brunnen
    BuildingType {
        building: false,
        text: 4859,
        file: Some("fussballerstatue-%d.bmp"),
        size: (2, 4, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 11,
        animation: 100,
        price: 1000000,
        weeks: 4,
        resale: 15,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 45 Spielerstatue
    BuildingType {
        building: false,
        text: 4860,
        file: Some("karussell-%d.bmp"),
        size: (2, 2, 1),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 6,
        frames: 0,
        animation: 0,
        price: 750000,
        weeks: 4,
        resale: 80,
        upkeep: 0,
        rent: 60000,
        sellable: true,
        staff: &[(25, 1), (7, 1)],
    }, // 46 Karussell
    BuildingType {
        building: false,
        text: 4861,
        file: Some("statue%d.bmp"),
        size: (1, 1, 1),
        category: Some(Category::Landscape),
        class: 6,
        sound: 2,
        frames: 2,
        animation: 0,
        price: 10000,
        weeks: 0,
        resale: 80,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 47 Statue
    BuildingType {
        building: true,
        text: 4862,
        file: Some("unterdruckhalle-%d.bmp"),
        size: (5, 4, 4),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 8,
        animation: 100,
        price: 8000000,
        weeks: 72,
        resale: 18,
        upkeep: 400000,
        rent: 0,
        sellable: true,
        staff: &[(17, 1), (6, 1)],
    }, // 48 Unterdruckhalle
    BuildingType {
        building: true,
        text: 4863,
        file: Some("trainingsplatz_schnee-%d.bmp"),
        size: (8, 5, 5),
        category: None,
        class: 9,
        sound: 0,
        frames: 11,
        animation: 40,
        price: 700000,
        weeks: 12,
        resale: 18,
        upkeep: 70000,
        rent: 0,
        sellable: true,
        staff: &[(12, 1)],
    }, // 49 Trainingsplatz (beheizt)
    BuildingType {
        building: false,
        text: 4864,
        file: Some("spielwiese-%d.bmp"),
        size: (3, 3, 3),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 4,
        animation: 100,
        price: 100000,
        weeks: 4,
        resale: 80,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 50 Spielwiese
    BuildingType {
        building: true,
        text: 4865,
        file: Some("spielwiese-%d.bmp"),
        size: (3, 3, 3),
        category: None,
        class: 6,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 51 Parkhaus
    BuildingType {
        building: false,
        text: 4866,
        file: Some("römerstatue-%d.bmp"),
        size: (1, 3, 1),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 150000,
        weeks: 4,
        resale: 75,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 52 Römerstatue
    BuildingType {
        building: true,
        text: 4867,
        file: Some("fernsehturm-%d.bmp"),
        size: (1, 13, 1),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 30000000,
        weeks: 48,
        resale: 28,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[(7, 1), (2, 1)],
    }, // 53 Fernsehturm
    BuildingType {
        building: false,
        text: 4868,
        file: Some("hinkelstein-%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Landscape),
        class: 10,
        sound: 2,
        frames: 0,
        animation: 0,
        price: 20000,
        weeks: 0,
        resale: 20,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 54 Hinkelstein
    BuildingType {
        building: true,
        text: 4869,
        file: Some("schirischule-%d.bmp"),
        size: (3, 3, 2),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 600000,
        weeks: 0,
        resale: 20,
        upkeep: 50000,
        rent: 0,
        sellable: true,
        staff: &[(36, 2), (17, 1)],
    }, // 55 Schiedsrichterschule
    BuildingType {
        building: false,
        text: 4870,
        file: Some("bagger-%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 10,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 56 Bagger
    BuildingType {
        building: true,
        text: 4871,
        file: Some("flugplatz-%d.bmp"),
        size: (9, 7, 7),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 7,
        animation: 100,
        price: 20000000,
        weeks: 48,
        resale: 30,
        upkeep: 800000,
        rent: 0,
        sellable: true,
        staff: &[(42, 2), (25, 4), (33, 1), (35, 1), (6, 1)],
    }, // 57 Flugplatz
    BuildingType {
        building: false,
        text: 4872,
        file: Some("kipper-%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 10,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 58 Kipper
    BuildingType {
        building: true,
        text: 4873,
        file: Some("torfabrik-%d.bmp"),
        size: (3, 3, 2),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 34,
        animation: 8,
        price: 2000000,
        weeks: 36,
        resale: 18,
        upkeep: 0,
        rent: 100000,
        sellable: true,
        staff: &[(40, 1), (18, 3), (19, 1), (6, 1)],
    }, // 59 Torfabrik
    BuildingType {
        building: true,
        text: 4874,
        file: Some("tankstelle-%d.bmp"),
        size: (3, 2, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 1,
        frames: 3,
        animation: 8,
        price: 500000,
        weeks: 16,
        resale: 80,
        upkeep: 0,
        rent: 50000,
        sellable: true,
        staff: &[(27, 1), (1, 1)],
    }, // 60 Tankstelle
    BuildingType {
        building: true,
        text: 4875,
        file: Some("einkaufszentrum-%d.bmp"),
        size: (5, 5, 4),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 5,
        animation: 100,
        price: 5000000,
        weeks: 36,
        resale: 80,
        upkeep: 0,
        rent: 300000,
        sellable: true,
        staff: &[(37, 1), (5, 5), (6, 1)],
    }, // 61 Einkaufszentrum
    BuildingType {
        building: true,
        text: 4876,
        file: Some("pagode-%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 6,
        animation: 100,
        price: 200000,
        weeks: 8,
        resale: 80,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[(13, 1)],
    }, // 62 Pagode
    BuildingType {
        building: true,
        text: 4877,
        file: Some("leinwand-%d.bmp"),
        size: (4, 3, 3),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 4,
        animation: 100,
        price: 200000,
        weeks: 4,
        resale: 80,
        upkeep: 30000,
        rent: 0,
        sellable: true,
        staff: &[(7, 1), (20, 1)],
    }, // 63 Leinwand
    BuildingType {
        building: true,
        text: 4878,
        file: Some("teuresvereinszentrum-%d.bmp"),
        size: (7, 6, 3),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 9,
        animation: 100,
        price: 40000000,
        weeks: 96,
        resale: 20,
        upkeep: 200000,
        rent: 0,
        sellable: true,
        staff: &[(4, 1), (26, 2), (29, 2), (6, 2), (13, 1)],
    }, // 64 Teures Vereinsheim
    BuildingType {
        building: true,
        text: 4879,
        file: Some("vereinshausMittel-%d.bmp"),
        size: (6, 4, 3),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 11,
        animation: 100,
        price: 10000000,
        weeks: 48,
        resale: 20,
        upkeep: 100000,
        rent: 0,
        sellable: true,
        staff: &[(4, 1), (26, 1), (29, 1), (6, 1)],
    }, // 65 Mittleres Vereinsheim
    BuildingType {
        building: true,
        text: 4880,
        file: Some("reihenhäuser1-%d.bmp"),
        size: (3, 3, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 4,
        animation: 100,
        price: 400000,
        weeks: 24,
        resale: 75,
        upkeep: 0,
        rent: 18000,
        sellable: true,
        staff: &[],
    }, // 66 Reihenhaus (1)
    BuildingType {
        building: true,
        text: 4881,
        file: Some("reihenhäuser2-%d.bmp"),
        size: (3, 3, 2),
        category: Some(Category::Entertainment),
        class: 6,
        sound: 0,
        frames: 4,
        animation: 100,
        price: 400000,
        weeks: 24,
        resale: 75,
        upkeep: 0,
        rent: 18000,
        sellable: true,
        staff: &[],
    }, // 67 Reihenhaus (2)
    BuildingType {
        building: true,
        text: 4882,
        file: Some("ritterburg-%d.bmp"),
        size: (8, 6, 3),
        category: None,
        class: 6,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 75,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 68 Ritterburg
    BuildingType {
        building: true,
        text: 4883,
        file: Some("schloss-%d.bmp"),
        size: (6, 5, 3),
        category: None,
        class: 6,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 75,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 69 Schloss
    BuildingType {
        building: false,
        text: 4884,
        file: Some("konditionsparcour-%d.bmp"),
        size: (7, 6, 6),
        category: Some(Category::Club),
        class: 9,
        sound: 2,
        frames: 0,
        animation: 0,
        price: 200000,
        weeks: 4,
        resale: 25,
        upkeep: 10000,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 70 Konditionsparcours
    BuildingType {
        building: false,
        text: 4885,
        file: Some("mauer-%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 10,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 71 Mauer
    BuildingType {
        building: false,
        text: 4886,
        file: Some("kleinerzementmischer-%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 10,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 72 Zementmischer
    BuildingType {
        building: false,
        text: 4887,
        file: Some("holzstapel-%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 10,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 73 Holzstafel
    BuildingType {
        building: false,
        text: 4888,
        file: Some("kieshaufen-%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 10,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 74 Kieshaufen
    BuildingType {
        building: false,
        text: 4889,
        file: Some("ziegelsteine1-%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 10,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 75 Ziegelsteine (1)
    BuildingType {
        building: false,
        text: 4890,
        file: Some("ziegelsteine2-%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 10,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 76 Ziegelsteine (2)
    BuildingType {
        building: false,
        text: 4891,
        file: Some("sandhaufen-%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 10,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 77 Sandhaufen
    BuildingType {
        building: false,
        text: 4892,
        file: Some("erdhaufen-%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 10,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 78 Erdhaufen
    BuildingType {
        building: false,
        text: 4893,
        file: Some("wald%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Landscape),
        class: 2,
        sound: 2,
        frames: 5,
        animation: 0,
        price: 10000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 79 Wald
    BuildingType {
        building: false,
        text: 4894,
        file: Some("büsche%d.bmp"),
        size: (1, 1, 1),
        category: Some(Category::Landscape),
        class: 2,
        sound: 2,
        frames: 2,
        animation: 0,
        price: 500,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 80 Büsche
    BuildingType {
        building: false,
        text: 4895,
        file: Some("blumendicht%d.bmp"),
        size: (1, 1, 1),
        category: Some(Category::Landscape),
        class: 2,
        sound: 2,
        frames: 5,
        animation: 0,
        price: 1000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 81 Blumenbeet (dicht)
    BuildingType {
        building: false,
        text: 4896,
        file: Some("wald_schnee%d.bmp"),
        size: (1, 2, 1),
        category: None,
        class: 2,
        sound: 2,
        frames: 5,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 82 Wald (Schnee)
    BuildingType {
        building: false,
        text: 4897,
        file: Some("apfel%d.bmp"),
        size: (1, 2, 1),
        category: Some(Category::Landscape),
        class: 2,
        sound: 2,
        frames: 4,
        animation: 0,
        price: 1000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 83 Apfelbaum
    BuildingType {
        building: false,
        text: 4898,
        file: Some("apfelschnee-%d.bmp"),
        size: (1, 2, 1),
        category: None,
        class: 2,
        sound: 2,
        frames: 4,
        animation: 0,
        price: 1000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 84 Apfelbaum (Schnee)
    BuildingType {
        building: false,
        text: 4899,
        file: Some("baum_schnee%d.bmp"),
        size: (1, 2, 1),
        category: None,
        class: 2,
        sound: 2,
        frames: 21,
        animation: 0,
        price: 1000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 85 Laubbaum (Schnee)
    BuildingType {
        building: false,
        text: 4900,
        file: Some("nadelbaum_schnee%d.bmp"),
        size: (1, 2, 1),
        category: None,
        class: 2,
        sound: 2,
        frames: 4,
        animation: 0,
        price: 1000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 86 Nadelbaum (Schnee)
    BuildingType {
        building: false,
        text: 4901,
        file: Some("büsche_schnee%d.bmp"),
        size: (1, 1, 1),
        category: None,
        class: 2,
        sound: 2,
        frames: 2,
        animation: 0,
        price: 500,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 87 Büsche (Schnee)
    BuildingType {
        building: false,
        text: 4902,
        file: Some("fundament_klein-%d.bmp"),
        size: (19, 12, 12),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 250000,
        weeks: 0,
        resale: 20,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 88 Fundament (Klein)
    BuildingType {
        building: false,
        text: 4903,
        file: Some("fundament_gross-%d.bmp"),
        size: (27, 17, 17),
        category: Some(Category::Club),
        class: 9,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 500000,
        weeks: 0,
        resale: 20,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 89 Fundament (Gross)
    BuildingType {
        building: true,
        text: 4904,
        file: None,
        size: (0, 0, 0),
        category: None,
        class: 9,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: false,
        staff: &[],
    }, // 90 1. Stadion
    BuildingType {
        building: true,
        text: 4905,
        file: None,
        size: (0, 0, 0),
        category: None,
        class: 9,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: false,
        staff: &[],
    }, // 91 2. Stadion
    BuildingType {
        building: true,
        text: 4906,
        file: None,
        size: (0, 0, 0),
        category: None,
        class: 9,
        sound: 0,
        frames: 0,
        animation: 0,
        price: 0,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: false,
        staff: &[],
    }, // 92 3. Stadion
    BuildingType {
        building: false,
        text: 4907,
        file: Some("betonparkplatz-O-U.bmp"),
        size: (1, 1, 1),
        category: Some(Category::Landscape),
        class: 5,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 40000,
        weeks: 0,
        resale: 10,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 93 Parkplatz
    BuildingType {
        building: false,
        text: 4908,
        file: Some("autobahn1.bmp"),
        size: (2, 1, 1),
        category: None,
        class: 5,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 60000,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 94 Autobahn 1
    BuildingType {
        building: false,
        text: 4909,
        file: Some("autobahn2.bmp"),
        size: (2, 1, 1),
        category: None,
        class: 5,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 60000,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 95 Autobahn 2
    BuildingType {
        building: false,
        text: 4910,
        file: Some("autobahn3.bmp"),
        size: (1, 2, 2),
        category: None,
        class: 5,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 60000,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 96 Autobahn 3
    BuildingType {
        building: false,
        text: 4911,
        file: Some("autobahn4.bmp"),
        size: (1, 2, 2),
        category: None,
        class: 5,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 60000,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 97 Autobahn 4
    BuildingType {
        building: false,
        text: 4912,
        file: Some("autobahnabfahrt1.bmp"),
        size: (1, 2, 2),
        category: None,
        class: 5,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 60000,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 98 Abfahrt 1
    BuildingType {
        building: false,
        text: 4913,
        file: Some("autobahnabfahrt2.bmp"),
        size: (1, 2, 2),
        category: None,
        class: 5,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 60000,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 99 Abfahrt 2
    BuildingType {
        building: false,
        text: 4914,
        file: Some("autobahnabfahrt3.bmp"),
        size: (2, 1, 1),
        category: None,
        class: 5,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 60000,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 100 Abfahrt 3
    BuildingType {
        building: false,
        text: 4915,
        file: Some("autobahnabfahrt4.bmp"),
        size: (2, 1, 1),
        category: None,
        class: 5,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 60000,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 101 Abfahrt 4
    BuildingType {
        building: false,
        text: 4916,
        file: Some("Autobahnabfahrt-O-R.bmp"),
        size: (2, 2, 2),
        category: None,
        class: 5,
        sound: 1,
        frames: 0,
        animation: 0,
        price: 150000,
        weeks: 0,
        resale: 0,
        upkeep: 0,
        rent: 0,
        sellable: true,
        staff: &[],
    }, // 102 Autobahnkurve
];
