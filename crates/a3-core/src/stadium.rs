//! A club's stadium as the stadium screen edits it (STADION.md in the RE
//! repo): three sub-records — the stadium played in and two being built —
//! with the places of the four stands, their age and building weeks, the
//! roofs, tracks and heaters, the comfort, the scoreboard and the pitch.

use crate::finances::CLUB_TO_TEAM;
use crate::records::{Records, Stadium, Team};

/// The stadium objects (`0x1a48138[i]`, from +0x20): three sub-records of
/// 0xd0 bytes.
const STADIUM_AT: u32 = 0x1a48138;
pub const SUBS: usize = 3;
const SUB: usize = 0xd0;
/// Four stands: the main one, the opposite, left and right of the main.
pub const STANDS: usize = 4;

const KIND: usize = 0;
const NAME: (usize, usize) = (2, 0x1e);
const CITY: (usize, usize) = (0x20, 0x18);
const CAPACITY: usize = 0x3c;
const VIP: usize = 0x40;
const STANDING: usize = 0x48;
const SEATS: usize = 0x50;
const AGE: usize = 0x58;
const WEEKS: usize = 0x5c;
const STAND_BITS: usize = 0x60;
const COMFORT: usize = 0x64;
const SCOREBOARD: usize = 0x66;
const WEAR: usize = 0x68;
const VIEW: usize = 0x70;
/// A stand's roof, running track and heaters: bit i, 4 + i, 8 + i.
const ROOF: u16 = 0x1;
const TRACK: u16 = 0x10;
const HEATERS: u16 = 0x100;
/// The condition by the stands' age: a step every five years, at most six
/// (`FUN_00718bc7`).
const AGE_STEP: u8 = 5;
const WORST: u8 = 6;

/// The comfort options, in the order of the screen's choices: their bit in
/// +0x64 and what they cost (`FUN_0057d64e`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Comfort {
    /// "ausfahrbares Spielfeld".
    Pitch,
    UnderSoilHeating,
    HeatedSeats,
    Floodlights,
    Cushions,
    Boxes,
}

pub const COMFORTS: [Comfort; 6] = [
    Comfort::Pitch,
    Comfort::UnderSoilHeating,
    Comfort::HeatedSeats,
    Comfort::Floodlights,
    Comfort::Cushions,
    Comfort::Boxes,
];

impl Comfort {
    pub fn bit(self) -> u16 {
        match self {
            Comfort::Pitch => 0x800,
            Comfort::UnderSoilHeating => 0x80,
            Comfort::HeatedSeats => 0x400,
            Comfort::Floodlights => 0x40,
            Comfort::Cushions => 0x200,
            Comfort::Boxes => 0x100,
        }
    }

    /// What it costs for a stadium with these seats.
    pub fn cost(self, seats: i32) -> i32 {
        match self {
            Comfort::Pitch => 2_000_000,
            Comfort::UnderSoilHeating | Comfort::Floodlights => 500_000,
            Comfort::HeatedSeats => seats.wrapping_mul(100),
            Comfort::Cushions => seats.wrapping_mul(5),
            Comfort::Boxes => 250_000,
        }
    }
}

/// The scoreboards' prices in thousands, none to the video wall
/// (`0x889f64`); an old one is taken back at half its price.
pub const SCOREBOARDS: [i32; 5] = [0, 5, 300, 1000, 6000];
const PRICE_UNIT: i32 = 1000;
const TAKEN_BACK: i32 = 500;

/// New turf by the pitch's wear (`FUN_005806a0`).
const TURF: [(u8, i32); 4] = [(5, 20_000), (10, 30_000), (15, 50_000), (20, 70_000)];
const WORN_TURF: i32 = 100_000;

/// The club's booked stadium costs (i64), stadium income (u32) and a
/// third sum the changes add to (club +0x2d8, +0x2e0, +0x4f8).
const COSTS: usize = 0x2d8;
const INCOME: usize = 0x2e0;
const SPENT: usize = 0x4f8;

/// One stadium, as its sub-record holds it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sub(pub Vec<u8>);

impl Sub {
    fn u16_(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.0[at], self.0[at + 1]])
    }

    fn put_u16(&mut self, at: usize, v: u16) {
        self.0[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }

    fn text(&self, (at, len): (usize, usize)) -> String {
        let bytes = &self.0[at..at + len];
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(len);
        a3_files::sav::cp1252(&bytes[..end])
    }

    /// 0 none, 1 a stadium, more a dream stadium.
    pub fn kind(&self) -> i16 {
        self.u16_(KIND) as i16
    }

    pub fn name(&self) -> String {
        self.text(NAME)
    }

    pub fn city(&self) -> String {
        self.text(CITY)
    }

    pub fn capacity(&self) -> i32 {
        i32::from_le_bytes(self.0[CAPACITY..CAPACITY + 4].try_into().unwrap())
    }

    pub fn vip(&self, stand: usize) -> u16 {
        self.u16_(VIP + 2 * stand)
    }

    pub fn seats(&self, stand: usize) -> u16 {
        self.u16_(SEATS + 2 * stand)
    }

    pub fn standing(&self, stand: usize) -> u16 {
        self.u16_(STANDING + 2 * stand)
    }

    /// Weeks of building left on a stand.
    pub fn weeks(&self, stand: usize) -> u8 {
        self.0[WEEKS + stand]
    }

    /// A stand's condition, 0 new to 6 a ruin.
    pub fn condition(&self, stand: usize) -> usize {
        usize::from((self.0[AGE + stand] / AGE_STEP).min(WORST))
    }

    pub fn roof(&self, stand: usize) -> bool {
        self.u16_(STAND_BITS) & (ROOF << stand) != 0
    }

    pub fn track(&self, stand: usize) -> bool {
        self.u16_(STAND_BITS) & (TRACK << stand) != 0
    }

    pub fn heaters(&self, stand: usize) -> bool {
        self.u16_(STAND_BITS) & (HEATERS << stand) != 0
    }

    pub fn has(&self, comfort: Comfort) -> bool {
        self.u16_(COMFORT) & comfort.bit() != 0
    }

    fn add(&mut self, comfort: Comfort) {
        let bits = self.u16_(COMFORT) | comfort.bit();
        self.put_u16(COMFORT, bits);
    }

    pub fn scoreboard(&self) -> usize {
        usize::from(self.0[SCOREBOARD])
    }

    pub fn wear(&self) -> u8 {
        self.0[WEAR]
    }

    /// Whether its view is the large one (249) or the small (510).
    pub fn large_view(&self) -> bool {
        self.u16_(VIEW) != 0
    }

    /// All its seats, which the heated seats and the cushions cost by.
    pub fn all_seats(&self) -> i32 {
        (0..STANDS).map(|s| i32::from(self.seats(s))).sum()
    }

    /// Places by roof: standing without and with, seats without and with,
    /// the boxes, the sum (`FUN_0057cf14`).
    pub fn places(&self) -> [i32; 6] {
        let sum = |field: fn(&Self, usize) -> u16, roofed: Option<bool>| -> i32 {
            (0..STANDS)
                .filter(|&s| roofed.is_none_or(|r| self.roof(s) == r))
                .map(|s| i32::from(field(self, s)))
                .sum()
        };
        let parts = [
            sum(Self::standing, Some(false)),
            sum(Self::standing, Some(true)),
            sum(Self::seats, Some(false)),
            sum(Self::seats, Some(true)),
            sum(Self::vip, None),
        ];
        let total = parts.iter().fold(0i32, |t, &p| t.wrapping_add(p));
        [parts[0], parts[1], parts[2], parts[3], parts[4], total]
    }

    /// The longest building time of a stand.
    pub fn building_weeks(&self) -> u8 {
        (0..STANDS).map(|s| self.weeks(s)).max().unwrap_or(0)
    }
}

/// What the chosen changes come to: the cost of the comfort chosen that the
/// stadium lacks, a new scoreboard's price, and an old one taken back.
pub fn change(sub: &Sub, chosen: &[bool; 6], scoreboard: usize) -> (i32, i32) {
    let seats = sub.all_seats();
    let mut cost = COMFORTS
        .iter()
        .zip(chosen)
        .filter(|&(&c, &on)| on && !sub.has(c))
        .fold(0i32, |sum, (&c, _)| sum.wrapping_add(c.cost(seats)));
    let mut taken_back = 0;
    if scoreboard != sub.scoreboard() {
        let price = |i: usize| SCOREBOARDS.get(i).copied().unwrap_or(0);
        taken_back = price(sub.scoreboard()).wrapping_mul(TAKEN_BACK);
        cost = cost.wrapping_add(price(scoreboard).wrapping_mul(PRICE_UNIT));
    }
    (cost, taken_back)
}

/// The chosen changes carried out and booked at once: the costs and the
/// income on the club, the comfort and the scoreboard on the stadium
/// (`FUN_00580004`).
pub fn carry_out(team: &mut Team, sub: &mut Sub, chosen: &[bool; 6], scoreboard: usize) {
    let (cost, taken_back) = change(sub, chosen, scoreboard);
    let at = |c: usize| c + CLUB_TO_TEAM;
    team.put(at(INCOME), team.get::<u32>(at(INCOME)).wrapping_add(taken_back as u32));
    team.put(at(COSTS), team.get::<i64>(at(COSTS)).wrapping_add(i64::from(cost)));
    let spent = cost.wrapping_sub(taken_back);
    team.put(at(SPENT), team.get::<i32>(at(SPENT)).wrapping_add(spent));
    for (&c, _) in COMFORTS.iter().zip(chosen).filter(|&(_, &on)| on) {
        sub.add(c);
    }
    sub.0[SCOREBOARD] = scoreboard as u8;
}

/// New turf: the pitch as new, booked at once, without asking
/// (`FUN_005806a0`).
pub fn new_turf(team: &mut Team, sub: &mut Sub) {
    let wear = sub.wear();
    if wear == 0 {
        return;
    }
    let cost = TURF.iter().find(|&&(most, _)| wear <= most).map_or(WORN_TURF, |&(_, c)| c);
    sub.0[WEAR] = 0;
    let at = COSTS + CLUB_TO_TEAM;
    team.put(at, team.get::<i64>(at).wrapping_add(i64::from(cost)));
}

fn block(records: &Records, country: u8, number: u16) -> Option<String> {
    let index = records.team_index(country.into(), number)?;
    Some(format!("{STADIUM_AT:#x}[{index}]"))
}

/// A club's three stadiums (`FUN_006381d3`).
pub fn subs(records: &Records, country: u8, number: u16) -> Option<Vec<Sub>> {
    let block = records.block(&block(records, country, number)?)?;
    (0..SUBS).map(|k| Some(Sub(block.get(k * SUB..(k + 1) * SUB)?.to_vec()))).collect()
}

/// Stores them (`FUN_0063827e`), the stadium played in first.
pub fn store(records: &mut Records, country: u8, number: u16, subs: &[Sub]) {
    let Some(index) = records.team_index(country.into(), number) else { return };
    let Some(name) = block(records, country, number) else { return };
    let Some(block) = records.block_mut(&name) else { return };
    for (k, sub) in subs.iter().enumerate().take(SUBS) {
        if let Some(to) = block.get_mut(k * SUB..(k + 1) * SUB) {
            to.copy_from_slice(&sub.0);
        }
    }
    if let (Some(first), Some(cached)) = (subs.first(), records.stadiums.get_mut(index)) {
        *cached = Stadium { name: first.name(), capacity: first.capacity() };
    }
}
