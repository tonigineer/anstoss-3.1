//! What the club does on its grounds (466, STADION-ANSICHT.md §6.4, §7.5):
//! the cursor over the map and what it would cost, buying land and what
//! stands on it, building, selling and pulling down, and the booking of
//! each.

use super::{
    CLASS, COUNT, ENTRIES, ENTRY, Entry, FIRST_STADIUM, GRID, Grounds, INVESTABLE, LARGE_STADIUM,
    LOOKS, OWNED, ROADS, SIZE, SMALL_STADIUM, STADIUMS, TYPES, long, word,
};
use crate::finances::CLUB_TO_TEAM;
use crate::random::Random;
use crate::records::Team;

/// What the buildings owned employ, by job (`0x1cb5e74 + 4k`).
const STAFF: usize = 0x1d4c4;
/// How many of some kinds stand on the map (`0x1cb5f24 + 4i`, by
/// [`COUNTED`]).
const COUNTERS: usize = 0x1d574;
/// What the buildings bought cost (i64 `0x1cb5f88`, "Sonstige
/// Investitionen").
const INVESTED: usize = 0x1d5d8;
/// The cursor's marks, a byte a tile (`0x1cb5fd0`: 1 grey, 2 red), and its
/// area (`0x1cb86e0`: i32 dx, dy, x, y; a byte whether it shows).
const MARKS: usize = 0x1d620;
const CURSOR: usize = 0x1fd30;
/// A tile's minimap class once bought or cleared.
const CLEARED: u8 = 4;
/// What a tile of land costs and brings (`DAT_00a3b3cc`, `DAT_00a3b3d4`).
const LAND_PRICE: i64 = 5000;
const LAND_RESALE: i64 = 3000;
/// The most positions one click builds on.
const MOST_BUILT: usize = 2000;

/// The kinds counted, in the counters' order (`FUN_007bf695`): Disco,
/// Doping-Labor, Fanshop, Fernsehstation, Flugplatz, Jugendinternat,
/// Konditionsparcours, Fußballmuseum, Präsidentenpalast, Rehaklinik,
/// Schiedsrichterschule, Trainingshalle, the three Trainingsplätze (Asche,
/// Rasen, Sand), Unterdruckhalle, the three Vereinsheime, Kassenhaus,
/// Würstchenbude, Infostand.
const COUNTED: [i16; 22] =
    [42, 23, 16, 40, 57, 27, 70, 43, 15, 13, 55, 37, 29, 28, 30, 48, 21, 65, 64, 31, 26, 24];
/// The kinds a placed one of which shows one of its frames at random.
const VARIANTS: [i16; 15] = [5, 32, 33, 6, 22, 79, 80, 81, 82, 83, 84, 85, 47, 86, 87];
pub const STREET: i16 = 2;
pub const PARKING: i16 = 93;
pub const SMALL_SITE: i16 = 88;
pub const LARGE_SITE: i16 = 89;
/// Where a small site lies in a large one's footprint.
const SITE_IN_SITE: (i32, i32) = (4, 2);

/// The team's marks of some buildings (team +0x95c), which go when the
/// last of them does (`FUN_0069f1b8`, `FUN_0069f41b`, `FUN_0069f74a`,
/// `FUN_0069f815`): the Jugendinternat, the Konditionsparcours — which a
/// Trainingshalle pulled down clears too, by the parcours' count —, the
/// Trainingsplatz (Sand), the Unterdruckhalle.
const TEAM_MARKS: usize = 0x95c;
const YOUTH_HOME: (i16, u32) = (27, 0x2);
const FITNESS_TRAIL: (i16, u32) = (70, 0x80);
const TRAINING_HALL: i16 = 37;
const SAND_PITCH: (i16, u32) = (30, 0x40);
const LOW_PRESSURE_HALL: (i16, u32) = (48, 0x8);

/// The club's money the grounds book to (`FUN_007c630d`): extensions of
/// the grounds (i64 club+0x2e8), and the stadium's income (u32 club+0x2e0)
/// for what they bring.
const EXTENSIONS: usize = 0x2e8;
const INCOME: usize = 0x2e0;

/// The jobs the buildings give, by their text, and their wages a year
/// (`0x7c2aae`).
pub const JOBS: [(u16, i32); 44] = [
    (4314, 25_000),
    (4315, 25_000),
    (4316, 30_000),
    (4317, 30_000),
    (4318, 30_000),
    (4319, 30_000),
    (4320, 30_000),
    (4321, 30_000),
    (4322, 30_000),
    (4323, 30_000),
    (4324, 40_000),
    (4325, 40_000),
    (4326, 40_000),
    (4327, 40_000),
    (4328, 45_000),
    (4329, 50_000),
    (4330, 50_000),
    (4331, 50_000),
    (4332, 50_000),
    (4333, 50_000),
    (4334, 50_000),
    (4335, 50_000),
    (4336, 50_000),
    (4337, 50_000),
    (4338, 60_000),
    (4339, 60_000),
    (4340, 60_000),
    (4341, 60_000),
    (4342, 60_000),
    (4343, 60_000),
    (4344, 60_000),
    (4346, 70_000),
    (4347, 70_000),
    (4348, 70_000),
    (4349, 80_000),
    (4350, 80_000),
    (4351, 90_000),
    (4352, 100_000),
    (4353, 100_000),
    (4354, 120_000),
    (4355, 120_000),
    (4356, 120_000),
    (4357, 180_000),
    (5115, 200_000),
];

/// What the cursor does: the modes of 466 (`0x8af4c4`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    Buy,
    Build,
    Sell,
    Demolish,
}

/// Tiles from (x, y) to (x + dx, y + dy), both included, in either
/// direction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
}

impl Area {
    pub fn tile(x: i32, y: i32) -> Self {
        Self { x, y, dx: 0, dy: 0 }
    }

    /// Row by row, each the way the area runs.
    fn tiles(self) -> impl Iterator<Item = (i32, i32)> {
        let (sx, sy) = (self.dx.signum() | 1, self.dy.signum() | 1);
        let rows = (0..=self.dy.abs()).map(move |r| self.y + r * sy);
        rows.flat_map(move |y| (0..=self.dx.abs()).map(move |c| (self.x + c * sx, y)))
    }

    /// The least and the most column and row.
    fn bounds(self) -> (i32, i32, i32, i32) {
        let (x1, y1) = (self.x + self.dx, self.y + self.dy);
        (self.x.min(x1), self.x.max(x1), self.y.min(y1), self.y.max(y1))
    }
}

fn on_map(x: i32, y: i32) -> bool {
    (0..SIZE as i32).contains(&x) && (0..SIZE as i32).contains(&y)
}

/// A tile's index in the grids, which the game takes as it comes: a
/// column past the edge is the next row's first.
fn index(x: i32, y: i32) -> usize {
    usize::try_from(y * SIZE as i32 + x).unwrap_or(usize::MAX)
}

/// A kind's record, if the game has one.
pub(super) fn kind_of(kind: i16) -> Option<&'static super::BuildingType> {
    TYPES.get(usize::try_from(kind).ok()?)
}

/// The price a kind is listed at (+0x68: hundreds).
pub(super) fn listed(kind: i16) -> i64 {
    kind_of(kind).map_or(0, |t| i64::from(t.price / 100 * 100))
}

fn is_stadium(kind: i16) -> bool {
    (FIRST_STADIUM..FIRST_STADIUM + STADIUMS as i16).contains(&kind)
}

/// What a building's staff cost a year (`FUN_007be978`).
pub fn wages(kind: i16) -> i32 {
    kind_of(kind).map_or(0, |t| {
        t.staff.iter().map(|&(job, n)| JOBS[usize::from(job)].1 * i32::from(n)).sum()
    })
}

/// A purchase as the game reckons it before it asks (`FUN_007c3980`): what
/// it costs, the staff's wages a year, how many buildings it takes over,
/// the land and the entries.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Purchase {
    pub sum: i64,
    pub wages: i32,
    pub buildings: i32,
    marked: Vec<bool>,
    entries: Vec<usize>,
}

/// A sale (`FUN_007c48a5`): what it brings, whether a stadium stands in
/// the way, the land and the entries.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sale {
    pub sum: i64,
    pub stadium: bool,
    marked: Vec<bool>,
    entries: Vec<usize>,
}

/// Pulling down (`FUN_007c4411`): what it costs, how many entries go,
/// whether a stadium stands in the way.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Demolition {
    pub cost: i64,
    pub count: i32,
    pub stadium: bool,
    marked: Vec<bool>,
}

/// What a click in building mode comes to (`OnLButtonUp 0x7c2004`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Works {
    /// Nothing to build there.
    Nothing,
    /// The positions to build on and what they cost.
    Build { positions: Vec<(i32, i32)>, cost: i64 },
    /// The club's streets made car parks.
    Paved,
    /// A large site laid over a small one or a stadium (nothing booked).
    Enlarged,
}

impl Grounds {
    fn grid(&self, i: usize) -> i16 {
        self.data.get(GRID + 2 * i..GRID + 2 * i + 2).map_or(-1, |b| word(b, 0))
    }

    fn put_grid(&mut self, i: usize, k: i16) {
        if let Some(b) = self.data.get_mut(GRID + 2 * i..GRID + 2 * i + 2) {
            b.copy_from_slice(&k.to_le_bytes());
        }
    }

    fn put_byte(&mut self, at: usize, v: u8) {
        if let Some(b) = self.data.get_mut(at) {
            *b = v;
        }
    }

    fn put_long(&mut self, at: usize, v: i32) {
        self.data[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn put_entry(&mut self, k: usize, e: &Entry) {
        let b = &mut self.data[k * ENTRY..(k + 1) * ENTRY];
        b[0..2].copy_from_slice(&e.kind.to_le_bytes());
        b[2] = e.x;
        b[3] = e.y;
        b[4] = e.frame;
        b[5] = e.owned.into();
        b[6] = e.used.into();
        b[7] = e.weeks;
    }

    /// The kind of the entry on a tile; a tile with none reads the entry
    /// before the first, which is no road or stadium.
    fn kind_at(&self, i: usize) -> Option<i16> {
        let k = usize::try_from(self.grid(i)).ok().filter(|&k| k < ENTRIES)?;
        Some(self.entry(k).kind)
    }

    /// What the club may spend, as the screen last took it (`0x1cb8710`).
    pub fn investable(&self) -> i64 {
        i64::from_le_bytes(self.data[INVESTABLE..INVESTABLE + 8].try_into().unwrap())
    }

    /// A kind's width, height and depth in tiles; a stadium's by its look.
    pub fn size(&self, kind: i16) -> (i32, i32, i32) {
        let (w, h, d) = if is_stadium(kind) {
            let look = self.look(usize::try_from(kind - FIRST_STADIUM).unwrap_or(0));
            if look[2] != 0 { LARGE_STADIUM } else { SMALL_STADIUM }
        } else {
            kind_of(kind).map_or((0, 0, 0), |t| t.size)
        };
        (w.into(), h.into(), d.into())
    }

    /// The tiles under a kind's footprint at (x, y), by index.
    fn footprint(&self, x: i32, y: i32, kind: i16) -> Vec<usize> {
        let (w, _, d) = self.size(kind);
        (y..y + d).flat_map(|yy| (x..x + w).map(move |xx| index(xx, yy))).collect()
    }

    /// Whether a kind would stand at (x, y) on the club's land with
    /// nothing in the way (`FUN_007bf5eb` = 1).
    pub fn fits(&self, x: i32, y: i32, kind: i16) -> bool {
        self.footprint(x, y, kind).into_iter().all(|i| self.owns_at(i) && self.grid(i) == -1)
    }

    /// Puts a kind on the map (`FUN_007bf695(x, y, kind, own, prebuy,
    /// timed)`): the land bought first if asked; on the club's land only
    /// where it fits; in the first free entry, its weeks of building if
    /// timed; a variant's frame drawn, a counted kind counted, roads joined;
    /// the staff of what the club owns. Whether it was put.
    #[allow(clippy::too_many_arguments)]
    pub fn place(
        &mut self,
        x: i32,
        y: i32,
        kind: i16,
        own: bool,
        prebuy: bool,
        timed: bool,
        random: &mut Random,
    ) -> bool {
        let tiles = self.footprint(x, y, kind);
        if prebuy {
            for &i in &tiles {
                if !self.owns_at(i) {
                    self.put_byte(OWNED + i, 1);
                    self.put_byte(CLASS + i, CLEARED);
                }
            }
        }
        if own && !self.fits(x, y, kind) {
            return false;
        }
        let Some(k) = (0..ENTRIES).find(|&k| !self.entry(k).used) else { return false };
        let record = kind_of(kind);
        let weeks = if timed { record.map_or(0, |t| t.weeks) } else { 0 };
        let entry = Entry { kind, x: x as u8, y: y as u8, frame: 0, owned: own, used: true, weeks };
        self.put_entry(k, &entry);
        self.put_long(COUNT, self.count() + 1);
        let class = record.map_or(0, |t| t.class);
        for &i in &tiles {
            self.put_byte(OWNED + i, own.into());
            self.put_grid(i, k as i16);
            self.put_byte(CLASS + i, class);
        }
        if VARIANTS.contains(&kind) {
            let frames = record.map_or(0, |t| t.frames);
            self.data[k * ENTRY + 4] = random.below(frames.into()) as u8;
        } else if ROADS.contains(&kind) {
            for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                self.rejoin(x + dx, y + dy);
            }
        } else {
            self.count_kind(kind, 1);
        }
        if own {
            self.add_staff(kind, 1);
        }
        true
    }

    /// Whether a road lies on a tile (`FUN_007bf3ac`).
    fn road_at(&self, x: i32, y: i32) -> bool {
        self.kind_at(index(x, y)).is_some_and(|k| ROADS.contains(&k))
    }

    /// A road takes the frame that joins it with the roads beside it
    /// (`FUN_007bf409`).
    fn rejoin(&mut self, x: i32, y: i32) {
        if !on_map(x, y) || !self.road_at(x, y) {
            return;
        }
        let mut sides = 0;
        if x > 0 && self.road_at(x - 1, y) {
            sides |= 4;
        }
        if x < 99 && self.road_at(x + 1, y) {
            sides |= 8;
        }
        if y > 0 && self.road_at(x, y - 1) {
            sides |= 1;
        }
        if y < 99 && self.road_at(x, y + 1) {
            sides |= 2;
        }
        let frame = match sides {
            4 | 8 | 12 => 1,
            5 => 3,
            6 => 5,
            7 => 9,
            9 => 4,
            10 => 6,
            11 => 10,
            13 => 7,
            14 => 8,
            15 => 2,
            _ => 0,
        };
        if let Ok(k) = usize::try_from(self.grid(index(x, y))) {
            self.data[k * ENTRY + 4] = frame;
        }
    }

    /// A counted kind's count moved by `by`; the new count, or none for a
    /// kind not counted.
    fn count_kind(&mut self, kind: i16, by: i32) -> Option<i32> {
        let at = COUNTERS + 4 * COUNTED.iter().position(|&k| k == kind)?;
        let n = long(&self.data, at) + by;
        self.put_long(at, n);
        Some(n)
    }

    fn counted(&self, kind: i16) -> i32 {
        COUNTED.iter().position(|&k| k == kind).map_or(0, |i| long(&self.data, COUNTERS + 4 * i))
    }

    /// A building's staff joins (`FUN_007bec86`) or leaves (`FUN_007bf019`)
    /// the totals.
    fn add_staff(&mut self, kind: i16, by: i32) {
        let Some(t) = kind_of(kind) else { return };
        for &(job, n) in t.staff {
            let at = STAFF + 4 * usize::from(job);
            self.put_long(at, long(&self.data, at) + by * i32::from(n));
        }
    }

    /// What the buildings owned employ, by job.
    pub fn staff(&self, job: usize) -> i32 {
        long(&self.data, STAFF + 4 * job)
    }

    /// Takes an entry off the map (`FUN_007c5171(k, demolish)`): its staff
    /// gone if owned, a stadium pulled down losing its look, the land left
    /// cleared, roads beside it joined anew, its count down — and with the
    /// last of some kinds the team's mark.
    pub fn remove(&mut self, k: usize, demolish: bool, team: &mut Team) {
        let e = self.entry(k);
        if !e.used {
            return;
        }
        if e.owned {
            self.add_staff(e.kind, -1);
        }
        if demolish && is_stadium(e.kind) {
            self.data[LOOKS + 12 * usize::try_from(e.kind - FIRST_STADIUM).unwrap_or(0)] = 0;
        }
        let (w, _, d) = self.size(e.kind);
        let (x, y) = (i32::from(e.x), i32::from(e.y));
        for i in self.footprint(x, y, e.kind) {
            self.put_grid(i, -1);
            self.put_byte(CLASS + i, CLEARED);
        }
        self.data[k * ENTRY..(k + 1) * ENTRY].fill(0);
        self.put_long(COUNT, self.count() - 1);
        if ROADS.contains(&e.kind) {
            let (x, y) = (x + w - 1, y + d - 1);
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                self.rejoin(x + dx, y + dy);
            }
            return;
        }
        let Some(left) = self.count_kind(e.kind, -1) else { return };
        let mark = match e.kind {
            k if k == YOUTH_HOME.0 && left == 0 => YOUTH_HOME.1,
            k if k == FITNESS_TRAIL.0 && left == 0 => FITNESS_TRAIL.1,
            TRAINING_HALL if self.counted(FITNESS_TRAIL.0) == 0 => FITNESS_TRAIL.1,
            k if k == SAND_PITCH.0 && left == 0 => SAND_PITCH.1,
            k if k == LOW_PRESSURE_HALL.0 && left == 0 => LOW_PRESSURE_HALL.1,
            _ => return,
        };
        team.put(TEAM_MARKS, team.get::<u32>(TEAM_MARKS) & !mark);
    }

    /// The cursor's area, if it shows.
    pub fn cursor(&self) -> Option<Area> {
        (self.data[CURSOR + 16] != 0).then(|| Area {
            dx: long(&self.data, CURSOR),
            dy: long(&self.data, CURSOR + 4),
            x: long(&self.data, CURSOR + 8),
            y: long(&self.data, CURSOR + 12),
        })
    }

    /// Where the cursor starts, shown or not (`0x1cb86e8`, `0x1cb86ec`).
    pub fn cursor_start(&self) -> (i32, i32) {
        (long(&self.data, CURSOR + 8), long(&self.data, CURSOR + 12))
    }

    /// A tile's cursor mark by its index in the grid, read as the game
    /// reads it past the grid's end: 1 grey, 2 red, 0 none.
    pub fn mark_at(&self, i: usize) -> u8 {
        self.data.get(MARKS + i).copied().unwrap_or(0)
    }

    /// Moves the cursor (`FUN_007c5699`): the old marks cleared; the area,
    /// held to the map, marked grey where the mode can act and red where
    /// it cannot. `kind` is what building mode builds.
    pub fn set_cursor(&mut self, area: Option<Area>, mode: Mode, kind: i16) {
        if let Some(old) = self.cursor() {
            for (x, y) in old.tiles().filter(|&(x, y)| on_map(x, y)) {
                self.data[MARKS + index(x, y)] = 0;
            }
        }
        let Some(mut area) = area else {
            self.data[CURSOR + 16] = 0;
            return;
        };
        area.y = area.y.min(99 - area.dy);
        area.x = area.x.min(99 - area.dx);
        for (i, v) in [area.dx, area.dy, area.x, area.y].into_iter().enumerate() {
            self.put_long(CURSOR + 4 * i, v);
        }
        self.data[CURSOR + 16] = 1;
        for (x, y) in area.tiles().filter(|&(x, y)| on_map(x, y)) {
            let i = index(x, y);
            let (owned, empty) = (self.owns_at(i), self.grid(i) == -1);
            let can = match mode {
                Mode::Buy => !owned,
                Mode::Sell => owned,
                Mode::Build if kind == PARKING => owned && self.kind_at(i) == Some(STREET),
                Mode::Build => owned && empty,
                Mode::Demolish => owned && !empty,
            };
            self.data[MARKS + i] = if can { 1 } else { 2 };
        }
    }

    /// What the land and buildings under the cursor would cost
    /// (`FUN_007b9823`).
    pub fn purchase_cost(&self) -> Option<i64> {
        self.cursor().map(|_| self.purchase().map_or(0, |p| p.sum))
    }

    /// The purchase under the cursor (`FUN_007c3980`, its first part):
    /// each tile not the club's, once — empty, at the land's price; built
    /// on, the building at its price and its whole footprint as land.
    pub fn purchase(&self) -> Option<Purchase> {
        let area = self.cursor()?;
        let mut p = Purchase {
            sum: 0,
            wages: 0,
            buildings: 0,
            marked: vec![false; SIZE * SIZE],
            entries: Vec::new(),
        };
        let mut tiles = 0;
        for (x, y) in area.tiles() {
            let i = index(x, y);
            if p.marked.get(i).copied().unwrap_or(true) || self.owns_at(i) {
                continue;
            }
            let Ok(k) = usize::try_from(self.grid(i)) else {
                p.marked[i] = true;
                tiles += 1;
                continue;
            };
            let e = self.entry(k);
            p.entries.push(k);
            p.sum += kind_of(e.kind).map_or(0, |t| i64::from(t.price));
            p.wages += wages(e.kind);
            p.buildings += i32::from(kind_of(e.kind).is_some_and(|t| t.building));
            for j in self.footprint(e.x.into(), e.y.into(), e.kind) {
                if let Some(m) = p.marked.get_mut(j).filter(|m| !**m) {
                    *m = true;
                    tiles += 1;
                }
            }
        }
        p.sum += tiles * LAND_PRICE;
        Some(p)
    }

    /// The purchase made: the land the club's, the buildings on it with
    /// their staff and counts, their price among the investments.
    pub fn buy(&mut self, p: &Purchase) {
        for (i, _) in p.marked.iter().enumerate().filter(|&(_, &m)| m) {
            self.data[OWNED + i] = 1;
        }
        for &k in p.entries.iter().rev() {
            self.data[k * ENTRY + 5] = 1;
            let kind = self.entry(k).kind;
            self.add_staff(kind, 1);
            self.count_kind(kind, 1);
            let price = kind_of(kind).map_or(0, |t| i64::from(t.price));
            let invested =
                i64::from_le_bytes(self.data[INVESTED..INVESTED + 8].try_into().unwrap());
            self.data[INVESTED..INVESTED + 8].copy_from_slice(&(invested + price).to_le_bytes());
        }
    }

    /// The positions under the cursor a kind fits on, a footprint apart
    /// (`FUN_007b9b4a`, `0x7c2768`), at most 2,000.
    pub fn positions(&self, kind: i16) -> Vec<(i32, i32)> {
        let Some(area) = self.cursor() else { return Vec::new() };
        let (w, _, d) = self.size(kind);
        let (x0, x1, y0, y1) = area.bounds();
        let rows = (y0..=y1).step_by(d.max(1) as usize);
        let spots = rows.flat_map(|y| (x0..=x1).step_by(w.max(1) as usize).map(move |x| (x, y)));
        spots.filter(|&(x, y)| self.fits(x, y, kind)).take(MOST_BUILT).collect()
    }

    /// What building the kind under the cursor costs, as the frame shows
    /// it while the button is held (`FUN_007b9b4a`). For a car park the
    /// preview itself makes the club's streets car parks, booking nothing.
    pub fn build_cost(&mut self, kind: i16) -> Option<i64> {
        self.cursor()?;
        Some(if kind == PARKING {
            self.pave(None)
        } else {
            self.positions(kind).len() as i64 * listed(kind)
        })
    }

    /// The club's streets under the cursor made car parks, each booked if
    /// a team is given (0x7c21d3); what they come to.
    fn pave(&mut self, mut team: Option<&mut Team>) -> i64 {
        let Some(area) = self.cursor() else { return 0 };
        let (x0, x1, y0, y1) = area.bounds();
        let mut cost = 0;
        for (x, y) in (y0..=y1).flat_map(|y| (x0..=x1).map(move |x| (x, y))) {
            let Ok(k) = usize::try_from(self.grid(index(x, y))) else { continue };
            if k >= ENTRIES || self.entry(k).kind != STREET || !self.entry(k).owned {
                continue;
            }
            self.data[k * ENTRY..k * ENTRY + 2].copy_from_slice(&PARKING.to_le_bytes());
            cost += listed(PARKING);
            if let Some(team) = team.as_deref_mut() {
                book(team, listed(PARKING));
            }
        }
        cost
    }

    /// What a click in building mode does with the kind (`OnLButtonUp`):
    /// a car park paves the club's streets, paying for each — but the
    /// preview has paved them already —; a
    /// large site goes where it fits, or over a small site or a stadium
    /// lying in it; anything else on every position it fits.
    pub fn works(&mut self, kind: i16, team: &mut Team, random: &mut Random) -> Works {
        let Some(area) = self.cursor() else { return Works::Nothing };
        if kind == PARKING {
            self.pave(Some(team));
            return Works::Paved;
        }
        let (x0, _, y0, _) = area.bounds();
        if kind == LARGE_SITE && !self.fits(x0, y0, kind) {
            return if self.enlarge(area, team, random) { Works::Enlarged } else { Works::Nothing };
        }
        let positions = if kind == LARGE_SITE { vec![(x0, y0)] } else { self.positions(kind) };
        if positions.is_empty() {
            return Works::Nothing;
        }
        let cost = positions.len() as i64 * listed(kind);
        Works::Build { positions, cost }
    }

    /// The works paid for: each position built on, timed, the cost booked.
    pub fn build(
        &mut self,
        kind: i16,
        positions: &[(i32, i32)],
        team: &mut Team,
        random: &mut Random,
    ) {
        for &(x, y) in positions {
            self.place(x, y, kind, true, false, true, random);
        }
        book(team, positions.len() as i64 * listed(kind));
    }

    /// A large site over the small site or stadium whose corner lies at
    /// (4, 2) in it (0x7c2317): the scan stops at land not the club's or
    /// anything else built; the small site gives way, a stadium takes the
    /// large site's place and size. Nothing is booked.
    fn enlarge(&mut self, area: Area, team: &mut Team, random: &mut Random) -> bool {
        let (x0, x1, y0, y1) = area.bounds();
        let corner = (x0 + SITE_IN_SITE.0, y0 + SITE_IN_SITE.1);
        let mut found = false;
        'scan: for y in y0..=y1 {
            for x in x0..=x1 {
                let i = index(x, y);
                if !self.owns_at(i) {
                    found = false;
                    break 'scan;
                }
                if self.grid(i) == -1 {
                    continue;
                }
                let site = self.kind_at(i).is_some_and(|k| k == SMALL_SITE || is_stadium(k));
                if site && (x, y) == corner {
                    found = true;
                } else {
                    break 'scan;
                }
            }
        }
        if !found {
            return false;
        }
        let at = |e: Entry| (i32::from(e.x), i32::from(e.y)) == corner;
        let Some(k) = (0..ENTRIES).find(|&k| at(self.entry(k))) else { return false };
        match self.kind_at(index(corner.0, corner.1)) {
            Some(SMALL_SITE) => {
                self.remove(k, true, team);
                self.place(x0, y0, LARGE_SITE, true, false, true, random);
            }
            Some(kind) if is_stadium(kind) => {
                self.remove(k, false, team);
                self.place(x0, y0, LARGE_SITE, true, false, true, random);
                let same = (0..ENTRIES).find(|&j| {
                    let e = self.entry(j);
                    (i32::from(e.x), i32::from(e.y)) == (x0, y0)
                });
                if let Some(j) = same {
                    self.data[j * ENTRY..j * ENTRY + 2].copy_from_slice(&kind.to_le_bytes());
                }
                self.data[LOOKS + 12 * usize::try_from(kind - FIRST_STADIUM).unwrap_or(0) + 2] = 1;
            }
            _ => {}
        }
        true
    }

    /// What selling the club's land and buildings under the cursor would
    /// bring (`FUN_007b9e6b`).
    pub fn sale_value(&self) -> Option<i64> {
        self.cursor().map(|_| self.sale().map_or(0, |s| s.sum))
    }

    /// The sale under the cursor (`FUN_007c48a5`): each tile of the club's
    /// once — empty, at the land's resale; built on, the building at its
    /// resale percentage and its footprint as land; a stadium cannot go.
    pub fn sale(&self) -> Option<Sale> {
        let area = self.cursor()?;
        let mut s =
            Sale { sum: 0, stadium: false, marked: vec![false; SIZE * SIZE], entries: Vec::new() };
        for (x, y) in area.tiles() {
            let i = index(x, y);
            if s.marked.get(i).copied().unwrap_or(true) || !self.owns_at(i) {
                continue;
            }
            let Ok(k) = usize::try_from(self.grid(i)) else {
                s.marked[i] = true;
                s.sum += LAND_RESALE;
                continue;
            };
            let e = self.entry(k);
            let Some(t) = kind_of(e.kind).filter(|t| t.sellable) else {
                s.stadium = true;
                continue;
            };
            s.entries.push(k);
            s.sum += listed(e.kind) * i64::from(t.resale) / 100;
            for j in self.footprint(e.x.into(), e.y.into(), e.kind) {
                if let Some(m) = s.marked.get_mut(j).filter(|m| !**m) {
                    *m = true;
                    s.sum += LAND_RESALE;
                }
            }
        }
        Some(s)
    }

    /// The sale made: the land no longer the club's, the buildings left
    /// standing but not its, their staff and counts gone.
    pub fn sell(&mut self, s: &Sale) {
        for (i, _) in s.marked.iter().enumerate().filter(|&(_, &m)| m) {
            self.data[OWNED + i] = 0;
        }
        for &k in s.entries.iter().rev() {
            self.data[k * ENTRY + 5] = 0;
            let kind = self.entry(k).kind;
            self.add_staff(kind, -1);
            self.count_kind(kind, -1);
        }
    }

    /// What pulling down the club's buildings under the cursor would cost,
    /// counting a building once a tile it covers (`FUN_007ba1aa`).
    pub fn demolition_cost(&self) -> Option<i64> {
        let area = self.cursor()?;
        let cost = area
            .tiles()
            .filter_map(|(x, y)| {
                let i = index(x, y);
                let kind = self.kind_at(i).filter(|&k| !is_stadium(k))?;
                self.owns_at(i).then(|| listed(kind) / 20)
            })
            .sum();
        Some(cost)
    }

    /// Pulling down under the cursor (`FUN_007c4411`): each of the club's
    /// entries once, at a twentieth of its price; a stadium cannot go.
    pub fn demolition(&self) -> Option<Demolition> {
        let area = self.cursor()?;
        let mut d = Demolition { cost: 0, count: 0, stadium: false, marked: vec![false; ENTRIES] };
        for (x, y) in area.tiles() {
            let i = index(x, y);
            let Ok(k) = usize::try_from(self.grid(i)) else { continue };
            if !self.owns_at(i) || d.marked.get(k).copied().unwrap_or(true) {
                continue;
            }
            let kind = self.entry(k).kind;
            if is_stadium(kind) {
                d.stadium = true;
                continue;
            }
            d.marked[k] = true;
            d.count += 1;
            d.cost += listed(kind) / 20;
        }
        Some(d)
    }

    /// The buildings pulled down; the land stays the club's.
    pub fn demolish(&mut self, d: &Demolition, team: &mut Team) {
        for k in (0..ENTRIES).rev().filter(|&k| d.marked[k]) {
            self.remove(k, true, team);
        }
    }
}

/// Books what the grounds cost (> 0, to the extensions) or bring (to the
/// stadium's income) (`FUN_007c630d`; what the club may spend is to be
/// taken anew after).
pub fn book(team: &mut Team, amount: i64) {
    if amount > 0 {
        let at = EXTENSIONS + CLUB_TO_TEAM;
        team.put(at, team.get::<i64>(at).wrapping_add(amount));
    } else {
        let at = INCOME + CLUB_TO_TEAM;
        team.put(at, team.get::<u32>(at).wrapping_sub(amount as u32));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_area_runs_both_ways_and_includes_its_ends() {
        let tiles: Vec<_> = Area { x: 5, y: 5, dx: -1, dy: 1 }.tiles().collect();
        assert_eq!(tiles, [(5, 5), (4, 5), (5, 6), (4, 6)]);
        assert_eq!(Area { x: 5, y: 5, dx: -1, dy: 1 }.bounds(), (4, 5, 5, 6));
    }

    /// Grounds with nothing on them and none of it the club's.
    fn empty() -> Grounds {
        let mut g = Grounds { slot: 0, data: vec![0; super::super::BLOCK] };
        (0..SIZE * SIZE).for_each(|i| g.put_grid(i, -1));
        g
    }

    fn team() -> Team {
        Team::new(&[0; Team::SIZE]).unwrap()
    }

    fn own(g: &mut Grounds, x: i32, y: i32, w: i32, d: i32) {
        for (xx, yy) in (y..y + d).flat_map(|yy| (x..x + w).map(move |xx| (xx, yy))) {
            g.data[OWNED + index(xx, yy)] = 1;
        }
    }

    #[test]
    fn a_building_is_bought_with_its_land() {
        let mut g = empty();
        let mut random = Random::new(1);
        // A hotel, 3×2, not the club's.
        assert!(g.place(10, 10, 4, false, false, false, &mut random));
        g.set_cursor(Some(Area::tile(11, 10)), Mode::Buy, 0);
        let p = g.purchase().unwrap();
        assert_eq!((p.sum, p.buildings, p.wages), (1_500_000 + 6 * LAND_PRICE, 1, wages(4)));
        assert_eq!(g.purchase_cost(), Some(p.sum));
        g.buy(&p);
        assert!(g.owns(12, 11) && g.entry(0).owned);
        // A Hotelleiter (job 38) and three Zimmermädchen (job 9).
        assert_eq!((g.staff(38), g.staff(9)), (1, 3));
    }

    #[test]
    fn trees_go_on_every_free_tile_of_the_club_s() {
        let mut g = empty();
        let mut random = Random::new(1);
        own(&mut g, 0, 1, 4, 1);
        g.place(1, 1, 22, true, false, false, &mut random);
        // A Laubbaum (1×2×1) over four tiles, one taken by a rock.
        g.set_cursor(Some(Area { x: 0, y: 1, dx: 3, dy: 0 }), Mode::Build, 5);
        assert_eq!(g.positions(5), [(0, 1), (2, 1), (3, 1)]);
        assert_eq!(g.build_cost(5), Some(3 * 1000));
        let mut team = team();
        let Works::Build { positions, cost } = g.works(5, &mut team, &mut random) else {
            panic!("trees to plant");
        };
        assert_eq!(cost, 3000);
        g.build(5, &positions, &mut team, &mut random);
        assert_eq!(g.count(), 4);
        assert!((1..4).all(|k| g.entry(k).kind == 5 && g.entry(k).frame < 21));
        assert_eq!(team.get::<i64>(EXTENSIONS + CLUB_TO_TEAM), 3000);
    }

    #[test]
    fn roads_join_their_neighbours() {
        let mut g = empty();
        let mut random = Random::new(1);
        own(&mut g, 5, 5, 3, 2);
        for (x, y) in [(5, 5), (6, 5), (7, 5), (6, 6)] {
            g.place(x, y, STREET, true, false, false, &mut random);
        }
        // Left and right; left, right and down; up only.
        let frame = |g: &Grounds, x: usize, y: usize| g.entry(g.at(x, y).unwrap()).frame;
        assert_eq!((frame(&g, 5, 5), frame(&g, 6, 5), frame(&g, 6, 6)), (1, 8, 0));
        let k = g.at(6, 6).unwrap();
        g.remove(k, true, &mut team());
        assert_eq!(frame(&g, 6, 5), 1);
        assert_eq!(g.class(6, 6), CLEARED);
    }

    #[test]
    fn a_stadium_is_neither_sold_nor_pulled_down() {
        let mut g = empty();
        let mut random = Random::new(1);
        own(&mut g, 20, 20, 19, 12);
        assert!(g.place(20, 20, FIRST_STADIUM, true, false, false, &mut random));
        g.set_cursor(Some(Area::tile(25, 25)), Mode::Sell, 0);
        assert!(g.sale().unwrap().stadium);
        g.set_cursor(Some(Area::tile(25, 25)), Mode::Demolish, 0);
        assert!(g.demolition().unwrap().stadium);
        assert_eq!(g.demolition_cost(), Some(0));
    }

    #[test]
    fn pulling_down_counts_each_building_once() {
        let mut g = empty();
        let mut random = Random::new(1);
        own(&mut g, 0, 0, 4, 2);
        // A Pizzeria (2×2, 150.000) and a tree.
        g.place(0, 0, 35, true, false, false, &mut random);
        g.place(3, 0, 5, true, false, false, &mut random);
        g.set_cursor(Some(Area { x: 0, y: 0, dx: 3, dy: 1 }), Mode::Demolish, 0);
        let d = g.demolition().unwrap();
        assert_eq!((d.count, d.cost), (2, 150_000 / 20 + 1000 / 20));
        // The preview counts a building once a tile.
        assert_eq!(g.demolition_cost(), Some(4 * 150_000 / 20 + 1000 / 20));
        g.demolish(&d, &mut team());
        assert_eq!(g.count(), 0);
        assert!(g.owns(1, 1));
        // What is left sells as land.
        g.set_cursor(Some(Area { x: 0, y: 0, dx: 3, dy: 1 }), Mode::Sell, 0);
        assert_eq!(g.sale_value(), Some(8 * LAND_RESALE));
    }

    #[test]
    fn the_info_counts_the_club_s_buildings_and_land() {
        let mut g = empty();
        let mut random = Random::new(1);
        own(&mut g, 0, 0, 10, 10);
        // A Pizzeria being built, a Konditionsparcours (no building, with
        // upkeep), a tree not the club's.
        g.place(0, 0, 35, true, false, true, &mut random);
        g.place(2, 0, 70, true, false, false, &mut random);
        g.place(30, 30, 5, false, false, false, &mut random);
        let info = g.info();
        assert_eq!((info.finished, info.building, info.upkeep), (0, 1, 0));
        assert_eq!(info.area, 2 * 2 + 7 * 6);
        assert_eq!(g.works_in_progress(), [(12, 35, 150_000)]);
        // A Filialleiter, two Köche, two Verkäufer.
        assert_eq!(g.wages(), 100_000 + 2 * 60_000 + 2 * 30_000);
    }

    #[test]
    fn fans_complain_of_what_is_missing() {
        let mut g = empty();
        let mut random = Random::new(1);
        own(&mut g, 0, 0, 20, 20);
        let stadium = crate::stadium::Sub(vec![0; 0xd0]);
        let fans = |month| super::super::Fans {
            look: [0; 12],
            stadium: &stadium,
            country: 0,
            level: 0,
            month,
        };
        // An empty stadium: too small, no standing, no roof — and nothing
        // to eat, do or see; in summer no ice cream; no parking needed.
        let said = g.complaints(&fans(7));
        assert_eq!(said, [5036, 5054, 5055, 5056, 5059, 5060, 5062, 5066]);
        // A Fanshop and a Disco, two kinds of food with the ice cream, a
        // Spielerstatue: those complaints go.
        for (x, kind) in [(0, 16), (3, 42), (6, 41), (8, 25), (10, 45)] {
            assert!(g.place(x, 2, kind, true, false, false, &mut random));
        }
        assert_eq!(g.complaints(&fans(7)), [5056, 5059, 5060]);
    }

    #[test]
    fn the_view_glides_and_slows() {
        let mut g = empty();
        let mut view = super::super::View::default().zoomed(1);
        let right = [false, true, false, false];
        let scrolls: Vec<i32> = [right, right, [false; 4], [false; 4], [false; 4], [false; 4]]
            .into_iter()
            .map(|pushed| {
                g.glide(&mut view, pushed);
                view.scroll.0
            })
            .collect();
        // 2.4, 5.28, then 3.17, 1.9, 1.14 and below 1 nothing.
        assert_eq!(scrolls, [2, 7, 10, 11, 12, 12]);
        // Back left, each step cut towards nought: 12 − 2.4, 9 − 5.28; then
        // held at the edge.
        let left = [true, false, false, false];
        let back: Vec<i32> = (0..3)
            .map(|_| {
                g.glide(&mut view, left);
                view.scroll.0
            })
            .collect();
        assert_eq!(back, [9, 3, 0]);
    }

    #[test]
    fn a_lake_ripples() {
        let mut g = empty();
        let mut random = Random::new(1);
        g.place(0, 0, 0, false, false, false, &mut random);
        // See (groß): five frames, moving always.
        g.animate(&mut random);
        assert_eq!(g.entry(0).frame, 1);
        let frames: Vec<u8> = (0..40)
            .map(|_| {
                g.animate(&mut random);
                g.entry(0).frame
            })
            .collect();
        assert!(frames.iter().all(|&f| f <= 5) && frames.contains(&5));
    }

    #[test]
    fn a_hotel_s_staff_cost_what_the_table_says() {
        // Hotelleiter, Empfangsdame, three Zimmermädchen, two Pagen, two
        // Köche, a Tellerwäscher.
        let hotel = 100_000 + 30_000 + 3 * 30_000 + 2 * 30_000 + 2 * 60_000 + 25_000;
        assert_eq!(wages(4), hotel);
        assert_eq!(wages(5), 0);
    }
}
