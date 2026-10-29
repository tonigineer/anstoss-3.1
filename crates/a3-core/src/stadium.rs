//! A club's stadium as the stadium screen edits it (STADION.md in the RE
//! repo): three sub-records — the stadium played in and two being built —
//! with the places of the four stands, their age and building weeks, the
//! roofs, tracks and heaters, the comfort, the scoreboard and the pitch.

use std::io;

use a3_files::exe::Exe;

use crate::finances::CLUB_TO_TEAM;
use crate::records::{Records, Team};

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

    /// The stands' average age (`FUN_007180b3(0)`).
    pub fn average_age(&self) -> i32 {
        (0..STANDS).map(|s| i32::from(self.0[AGE + s])).sum::<i32>() / STANDS as i32
    }

    /// The longest building time of a stand.
    pub fn building_weeks(&self) -> u8 {
        (0..STANDS).map(|s| self.weeks(s)).max().unwrap_or(0)
    }

    pub fn set_vip(&mut self, stand: usize, v: u16) {
        self.put_u16(VIP + 2 * stand, v);
    }

    pub fn set_seats(&mut self, stand: usize, v: u16) {
        self.put_u16(SEATS + 2 * stand, v);
    }

    pub fn set_standing(&mut self, stand: usize, v: u16) {
        self.put_u16(STANDING + 2 * stand, v);
    }

    fn set_bit(&mut self, bit: u16, on: bool) {
        let bits = self.u16_(STAND_BITS);
        self.put_u16(STAND_BITS, if on { bits | bit } else { bits & !bit });
    }

    pub fn set_roof(&mut self, stand: usize, on: bool) {
        self.set_bit(ROOF << stand, on);
    }

    pub fn set_track(&mut self, stand: usize, on: bool) {
        self.set_bit(TRACK << stand, on);
    }

    pub fn set_heaters(&mut self, stand: usize, on: bool) {
        self.set_bit(HEATERS << stand, on);
    }

    /// The years into its condition step (`FUN_00718c3d`).
    pub fn started(&self, stand: usize) -> i32 {
        i32::from(self.0[AGE + stand]) - i32::from(AGE_STEP) * self.condition(stand) as i32
    }

    /// A stand put at the start of a condition (`FUN_00718c04`).
    pub fn set_condition(&mut self, stand: usize, condition: usize) {
        self.0[AGE + stand] = (condition as u8).wrapping_mul(AGE_STEP);
    }

    pub fn set_name(&mut self, name: &str) {
        let (at, len) = NAME;
        let bytes: Vec<u8> = name.chars().filter_map(a3_files::bitmap_font::to_cp1252).collect();
        let field = &mut self.0[at..at + len];
        field.fill(0);
        let n = bytes.len().min(len - 1);
        field[..n].copy_from_slice(&bytes[..n]);
    }

    /// Whether its site is a big one (+0x70), where a stand may hold
    /// 50,000 weighted places, not 6,000.
    pub fn big_site(&self) -> bool {
        self.large_view()
    }

    /// A stand's weighted places (`FUN_00717bfa`, `FUN_00717c0d`).
    pub fn weighted(&self, stand: usize) -> i32 {
        i32::from(self.standing(stand))
            + seats_weight(self.seats(stand).into())
            + vip_weight(self.vip(stand).into())
    }

    /// A stand's places as the cost compares them: standing, seats, VIP.
    fn three(&self, stand: usize) -> [i32; 3] {
        [self.standing(stand).into(), self.seats(stand).into(), self.vip(stand).into()]
    }

    /// Whether every stand is being built (`FUN_00718cd5`).
    pub fn all_building(&self) -> bool {
        (0..STANDS).all(|s| self.weeks(s) != 0)
    }
}

/// Seats and VIP places weigh more than standing places
/// (`FUN_00717bfa`, `FUN_00717c0d`), and back.
pub fn seats_weight(x: i32) -> i32 {
    x * 100 / 65
}

pub fn vip_weight(x: i32) -> i32 {
    x * 10 / 3
}

pub fn seats_of(weight: i32) -> i32 {
    weight * 65 / 100
}

pub fn vip_of(weight: i32) -> i32 {
    weight * 3 / 10
}

fn ceil10(x: i32) -> i32 {
    (x + 9) / 10 * 10
}

fn tens(x: i32) -> i32 {
    (x + 9) / 10
}

/// What building costs, by ten places (× `UNIT`): new places, demolished
/// ones, and places converted from standing, seats and VIP to each class
/// (0x8a7bfc …); renovating a step by a place, by the condition
/// (0x8a7c38).
const UNIT: i32 = 10;
const NEW_PLACES: [i32; 3] = [1000, 3000, 9000];
const DEMOLISHED: [i32; 3] = [100, 150, 450];
const CONVERTED: [[i32; 3]; 3] = [[0, 1500, 8000], [750, 0, 7500], [1000, 1500, 0]];
const RENOVATION: [i64; 7] = [200, 400, 700, 1000, 1500, 2000, 3000];
/// A roof taken off and a new one, by ten places of each class; heaters
/// by place; heated seats by seat.
const ROOF_OFF: [i32; 3] = [500, 1000, 3000];
const ROOF_ON: [i32; 3] = [10_000, 20_000, 60_000];
const HEATER: i32 = 20;
const HEATED_SEAT: i32 = 5;

/// The kind of a stand's work as the expansion shows it (texts 4257,
/// 4260): new places, places converted, or nothing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Work {
    New,
    Conversion,
    Nothing,
}

/// What a stand's plan costs: its building, its kind, the roofs, the
/// renovation and heaters, the weeks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StandCost {
    pub cost: i32,
    pub work: Work,
    pub roof: i32,
    pub renovation: i32,
    pub weeks: i16,
}

/// The cost of stand `i` built from `old` to `new` (`FUN_00719f38`;
/// STADION-AUSBAU.md §4).
pub fn stand_cost(new: &Sub, old: &Sub, i: usize) -> StandCost {
    let (o, n) = (old.three(i), new.three(i));
    let mut work = Work::New;
    let (mut c, mut roof, mut reno, mut weeks) = (0i32, 0i32, 0i32, 0i32);
    let mut steps = 0i32;
    let mut renovate = true;
    let (mut ns, mut nse, mut nv) = (0, 0, 0);
    let mut m = [[0i32; 3]; 3];
    let wo = o[0] + seats_weight(o[1]) - vip_weight(o[2]);
    let wn = n[0] + seats_weight(n[1]) - vip_weight(n[2]);
    let same_track = old.track(i) == new.track(i);
    if same_track {
        if n == o {
            work = Work::Nothing;
        } else {
            let ladder = |p: [i32; 3]| {
                let a = p[0];
                let b = a + seats_weight(p[1]);
                [a, b, b + vip_weight(p[2])]
            };
            let (ol, nl) = (ladder(o), ladder(n));
            let (mut a, mut b, mut prev) = (0, 0, 0);
            while a < 3 && b < 3 {
                if nl[b] < ol[a] {
                    m[a][b] = nl[b] - prev;
                    prev = nl[b];
                    b += 1;
                } else {
                    m[a][b] = ol[a] - prev;
                    prev = ol[a];
                    if ol[a] == nl[b] {
                        b += 1;
                    }
                    a += 1;
                }
            }
            if b == 0 {
                let x = nl[0] - prev;
                ns = ceil10(x);
                c += tens(x) * NEW_PLACES[0] * UNIT;
                prev = nl[0];
                b = 1;
            }
            if b == 1 {
                nse = ceil10(seats_of(nl[1] - prev));
                c += nse / 10 * NEW_PLACES[1] * UNIT;
                prev = nl[1];
                b = 2;
            }
            if b == 2 {
                let v = ceil10(vip_of(nl[2] - prev));
                nv += v;
                c += v / 10 * NEW_PLACES[2] * UNIT;
            }
            weeks += (ns + seats_weight(nse) + vip_weight(nv) + 999) / 1000 * 2;
            let (mut ds, mut dse, mut dv) = (0, 0, 0);
            if a == 0 {
                let x = ol[0] - prev;
                ds = ceil10(x);
                c += tens(x) * DEMOLISHED[0] * UNIT;
                prev = ol[0];
                a = 1;
            }
            if a == 1 {
                dse = ceil10(seats_of(ol[1] - prev));
                c += dse / 10 * DEMOLISHED[1] * UNIT;
                prev = ol[1];
                a = 2;
            }
            if a == 2 {
                let v = ceil10(vip_of(ol[2] - prev));
                dv += v;
                c += v / 10 * DEMOLISHED[2] * UNIT;
            }
            weeks += (ds + dse + dv + 999) / 1000 * 2;
            for j in 0..3 {
                c += (m[0][j] + 9) / 10 * CONVERTED[0][j] * UNIT;
                c += seats_of(m[1][j] + 9) / 10 * CONVERTED[1][j] * UNIT;
                c += vip_of(m[2][j] + 9) / 10 * CONVERTED[2][j] * UNIT;
            }
            let converted: i32 = (0..3)
                .flat_map(|a| (0..3).map(move |b| (a, b)))
                .filter(|(a, b)| a != b)
                .map(|(a, b)| m[a][b])
                .sum();
            if converted != 0 {
                work = Work::Conversion;
                weeks += (converted + 999) / 1000 * 2;
            }
        }
    } else {
        // The track changes: the stand is pulled down and built anew.
        [ns, nse, nv] = n.map(ceil10);
        c += ns / 10 * NEW_PLACES[0] * UNIT
            + nse / 10 * NEW_PLACES[1] * UNIT
            + nv / 10 * NEW_PLACES[2] * UNIT;
        weeks += (ns + nse + nv) / 1000 * 2;
        let [ds, dse, dv] = o.map(ceil10);
        c += ds / 10 * DEMOLISHED[0] * UNIT
            + dse / 10 * DEMOLISHED[1] * UNIT
            + dv / 10 * DEMOLISHED[2] * UNIT;
        weeks += (ds + dse + dv + 999) / 1000 * 2;
        renovate = false;
    }
    let co = old.condition(i);
    let cn = if wn > wo { 0 } else { new.condition(i) };
    let (to, tn) = (o.iter().sum::<i32>(), n.iter().sum::<i32>());
    if renovate {
        let step = |k: usize| RENOVATION[k] * i64::from(to) * i64::from(UNIT) / 500;
        for k in cn..co {
            reno = reno.wrapping_add((step(k) * 5) as i32);
        }
        reno = reno.wrapping_add((step(cn.max(co)) * i64::from(old.started(i))) as i32);
        steps = co as i32 - cn as i32;
    }
    let mut had = old.roof(i);
    if had && (ns > 0 || nse > 0 || nv > 0 || !same_track || !new.roof(i)) {
        roof += (0..3).map(|k| tens(o[k]) * ROOF_OFF[k]).sum::<i32>();
        had = false;
        weeks += 1;
    }
    if new.roof(i) && !had {
        roof += (0..3).map(|k| tens(n[k]) * ROOF_ON[k]).sum::<i32>();
        let t = (tn + 999) / 1000;
        weeks += match t {
            20.. => 10 + (t - 20) / 10,
            10.. => 7 + (t - 10) / 5,
            5.. => 6,
            _ => 5,
        };
    }
    if new.heaters(i) {
        let places = if old.heaters(i) { ns + nse + nv } else { tn };
        reno = reno.wrapping_add(places * HEATER);
    }
    if new.has(Comfort::HeatedSeats) {
        c += (nse + m[0][1] + m[2][1]) * HEATED_SEAT;
    }
    if ns > 0 || nse > 0 || nv > 0 {
        work = Work::New;
    }
    StandCost { cost: c, work, roof, renovation: reno, weeks: weeks.max(steps) as i16 }
}

/// Whether a plan changes a stand not being built: its places, its roof or
/// its track (`FUN_00718d14`).
pub fn plan_changed(stadium: &Sub, plan: &Sub) -> bool {
    (0..STANDS).filter(|&s| plan.weeks(s) == 0).any(|s| {
        stadium.three(s) != plan.three(s)
            || stadium.roof(s) != plan.roof(s)
            || stadium.track(s) != plan.track(s)
    })
}

/// The places a stand gives by its condition, in percent (0x8a7b50).
const BY_CONDITION: [i32; 7] = [100, 100, 100, 100, 80, 50, 20];

/// The capacity anew (`FUN_00718b1c`): the stands not being built, fewer
/// places for a run-down one.
pub fn recount(sub: &mut Sub) {
    let capacity = (0..STANDS)
        .filter(|&s| sub.weeks(s) == 0)
        .map(|s| sub.three(s).iter().sum::<i32>() * BY_CONDITION[sub.condition(s)] / 100)
        .fold(0i32, i32::wrapping_add);
    sub.0[CAPACITY..CAPACITY + 4].copy_from_slice(&capacity.to_le_bytes());
}

/// A dream stadium's price in millions, by its kind (0x889e50); its
/// building time.
const DREAM_PRICES: [i64; 3] = [1000, 1500, 2000];
const MILLION: i64 = 1_000_000;
const DREAM_WEEKS: u8 = 100;
/// The fields a changed plan carries besides the stands (`FUN_0071b2e3`):
/// the kind, comfort, scoreboard, +0x67, the pitch, +0x69, +0x6a, +0x6b.
const CARRIED: [(usize, usize); 2] = [(KIND, 2), (COMFORT, 8)];

/// A plan carried out (`FUN_0071aee2`): a changed plan rebuilds the stands
/// not being built and books their whole cost; an unchanged one only the
/// renovations and heaters; a dream stadium its price and a hundred weeks.
#[derive(Clone, Copy, Debug)]
pub struct Carried {
    pub weeks: i16,
    pub cost: i32,
    /// The stands that got weeks of building.
    pub mask: u8,
    pub dream: bool,
}

pub fn carry_plan(team: &mut Team, stadium: &mut Sub, plan: &Sub) -> Carried {
    let changed = plan_changed(stadium, plan);
    if changed {
        for (at, len) in CARRIED {
            stadium.0[at..at + len].copy_from_slice(&plan.0[at..at + len]);
        }
    }
    let (mut all, mut renovations) = (0i64, 0i64);
    let mut out = Carried { weeks: 0, cost: 0, mask: 0, dream: false };
    for s in 0..STANDS {
        if stadium.weeks(s) != 0 {
            continue;
        }
        let sc = stand_cost(plan, stadium, s);
        all += i64::from(sc.cost);
        if changed {
            all += i64::from(sc.roof) + i64::from(sc.renovation);
            stadium.set_vip(s, plan.vip(s));
            stadium.set_seats(s, plan.seats(s));
            stadium.set_standing(s, plan.standing(s));
            stadium.set_roof(s, plan.roof(s));
            stadium.set_track(s, plan.track(s));
            stadium.set_heaters(s, plan.heaters(s));
            stadium.0[AGE + s] = plan.0[AGE + s];
            stadium.0[WEEKS + s] = sc.weeks as u8;
            out.weeks = out.weeks.max(sc.weeks);
            if sc.weeks > 0 {
                out.mask |= 1 << s;
            }
        } else {
            renovations += i64::from(sc.renovation);
            stadium.set_condition(s, plan.condition(s));
            stadium.set_heaters(s, plan.heaters(s));
        }
    }
    out.cost = all as i32;
    recount(stadium);
    let booked = if plan.kind() >= 2 && changed {
        stadium.0[WEEKS..WEEKS + STANDS].fill(DREAM_WEEKS);
        out.dream = true;
        let k = usize::try_from(plan.kind() - 2).unwrap_or(0).min(DREAM_PRICES.len() - 1);
        DREAM_PRICES[k] * MILLION
    } else if changed {
        all
    } else {
        renovations
    };
    book(team, booked);
    out
}

/// Stadium costs booked on the club (+0x2d8, i64, and +0x4f8).
fn book(team: &mut Team, amount: i64) {
    let costs = COSTS + CLUB_TO_TEAM;
    team.put(costs, team.get::<i64>(costs).wrapping_add(amount));
    let spent = SPENT + CLUB_TO_TEAM;
    team.put(spent, team.get::<i32>(spent).wrapping_add(amount as i32));
}

/// The builder's offer after a long plan (`0x57f74f`): for a quarter of
/// the cost, truncated to 5,000s, a fifth of the weeks less. None when the
/// plan is short or dear.
const OFFER_WEEKS: i16 = 10;
const OFFER_MOST: i32 = 250_000_000;
const OFFER_STEP: i32 = 5000;

pub fn builder_offer(carried: &Carried) -> Option<i32> {
    (!carried.dream && carried.weeks > OFFER_WEEKS && carried.cost < OFFER_MOST)
        .then(|| carried.cost / 4 / OFFER_STEP * OFFER_STEP)
}

/// The offer taken: the stands built a fifth faster, its price booked.
pub fn take_offer(team: &mut Team, stadium: &mut Sub, carried: &Carried, price: i32) {
    let weeks = carried.weeks - carried.weeks / 5;
    for s in (0..STANDS).filter(|&s| carried.mask & (1 << s) != 0) {
        stadium.0[WEEKS + s] = weeks as u8;
    }
    book(team, price.into());
}

/// The stadium's "Arena" mark (+0x64 bit 0x2) goes unless every stand
/// holds 6,000 to 17,999 weighted places under a roof with a track
/// (`FUN_00719047`).
const ARENA: u16 = 0x2;

pub fn check_arena(sub: &mut Sub) {
    let fits = (0..STANDS)
        .all(|s| (6000..=17_999).contains(&sub.weighted(s)) && sub.roof(s) && sub.track(s));
    if !fits {
        let bits = sub.u16_(COMFORT) & !ARENA;
        sub.put_u16(COMFORT, bits);
    }
}

/// A stadium's place on the grounds map (+0x6c, +0x6e), its site's size
/// (+0x70) and the two bytes a site sets (+0x6a, +0x6b), as the stadium
/// screen gives them from the map (STADION.md §5.4).
pub fn place(sub: &mut Sub, x: u8, y: u8, large: bool) {
    sub.0[0x6a] = 2;
    sub.0[0x6b] = 3;
    sub.put_u16(0x6c, x.into());
    sub.put_u16(0x6e, y.into());
    sub.put_u16(VIEW, large.into());
}

/// A stand's look on the grounds map (`FUN_0057c464`): a dream stadium's
/// own, else its size class by weighted places (none, up to 3,000, 6,000,
/// 18,000, 32,000, more), 5 more with a track, 10 more with a roof; 0xff
/// for no stand.
pub const NO_STAND: u8 = 0xff;
const SIZE_CLASSES: [i32; 4] = [3000, 6000, 18_000, 32_000];
const DREAM_LOOK: u8 = 0x12;

pub fn stand_look(sub: &Sub, stand: usize) -> u8 {
    if sub.kind() >= 2 {
        return (sub.kind() as u8).wrapping_add(DREAM_LOOK);
    }
    let n = sub.weighted(stand);
    if n == 0 {
        return NO_STAND;
    }
    let class = SIZE_CLASSES.iter().position(|&most| n <= most).unwrap_or(SIZE_CLASSES.len()) as u8;
    class + 5 * u8::from(sub.track(stand)) + 10 * u8::from(sub.roof(stand))
}

/// What the views and the grounds draw of a stadium (`FUN_0057e9d7`):
/// whether it stands (a dream stadium only when built), a dream stadium's
/// look, its size, floodlights (always), the cinder tracks of the side
/// stands, and the four stands' looks — left, right, opposite, main.
pub fn look(sub: &Sub) -> [u8; 12] {
    let mut d = [0u8; 12];
    match sub.kind() {
        0 => {}
        1 => {
            d[0] = 1;
            d[2] = u8::from(sub.big_site());
            d[3] = 1;
            for (k, stand) in [2, 3, 1, 0].into_iter().enumerate() {
                d[4 + k] = u8::from(sub.track(stand));
                d[8 + k] = stand_look(sub, stand);
            }
        }
        kind => {
            let built = (0..STANDS).all(|s| sub.weeks(s) == 0);
            d[0] = u8::from(built);
            if built {
                d[1] = (kind as u8).wrapping_add(DREAM_LOOK);
            }
            d[2] = 1;
            for b in &mut d[3..8] {
                *b = 1;
            }
            for (k, stand) in [2, 3, 1, 0].into_iter().enumerate() {
                d[8 + k] = stand_look(sub, stand);
            }
        }
    }
    d
}

/// The dream stadiums one can build instead (0x889bf0).
const DREAMS_AT: u32 = 0x889bf0;
pub const DREAMS: usize = 3;

pub struct StadiumTables {
    pub dreams: Vec<Sub>,
    /// What a stadium of each kind is worth in millions, as its value
    /// reads it: by the kind itself where the prices go by the kind less
    /// two, so a dream stadium reads past the prices (0x889e60).
    values: Vec<i64>,
}

/// Where the values lie, and how many kinds they cover.
const VALUES_AT: u32 = 0x889e60;
const KINDS: usize = 5;

impl StadiumTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let bytes = exe.bytes(DREAMS_AT, DREAMS * SUB)?;
        let values = exe.bytes(VALUES_AT, KINDS * 8)?;
        Ok(Self {
            dreams: bytes.chunks(SUB).map(|b| Sub(b.to_vec())).collect(),
            values: values.chunks(8).map(|b| i64::from_le_bytes(b.try_into().unwrap())).collect(),
        })
    }
}

/// What a stadium is worth (`FUN_0057e859`): its stands built anew from
/// nothing, or a dream stadium's value by its kind, with their roofs, less
/// what renovating and heating them would cost, at least nought
/// (`FUN_0057baba`); its scoreboard's price added.
pub fn value(tables: &StadiumTables, sub: &Sub) -> i64 {
    let empty = Sub(vec![0; SUB]);
    let costs: Vec<StandCost> = (0..STANDS).map(|s| stand_cost(sub, &empty, s)).collect();
    let built = if sub.kind() < 2 {
        costs.iter().map(|c| i64::from(c.cost)).sum()
    } else {
        let value = usize::try_from(sub.kind()).ok().and_then(|k| tables.values.get(k));
        value.copied().unwrap_or(0).wrapping_mul(MILLION)
    };
    let roofs = costs.iter().fold(0i32, |sum, c| sum.wrapping_add(c.roof));
    let renovation = costs.iter().fold(0i32, |sum, c| sum.wrapping_add(c.renovation));
    let stands = built.wrapping_add(roofs.into()).wrapping_sub(renovation.into()).max(0);
    let scoreboard = SCOREBOARDS.get(sub.scoreboard()).copied().unwrap_or(0);
    stands + i64::from(scoreboard.wrapping_mul(PRICE_UNIT))
}

/// A dream stadium's price.
pub fn dream_price(kind: i16) -> i64 {
    let k = usize::try_from(kind - 2).unwrap_or(0).min(DREAM_PRICES.len() - 1);
    DREAM_PRICES[k] * MILLION
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
        cached.name = first.name();
        cached.capacity = first.capacity();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A.an3's stadium: the main stand 27 VIP and 3,590 seats, the others
    /// about 6,130 standing, all roofed, in their first condition step.
    fn alte_foersterei() -> Sub {
        let mut sub = Sub(vec![0; SUB]);
        sub.set_vip(0, 27);
        sub.set_seats(0, 3590);
        for (s, standing) in [(1, 6133), (2, 6131), (3, 6131)] {
            sub.set_standing(s, standing);
        }
        for s in 0..STANDS {
            sub.set_roof(s, true);
            sub.set_condition(s, 1);
        }
        sub
    }

    fn cost(plan: impl Fn(&mut Sub), stand: usize) -> (i32, i32, i32, i16) {
        let old = alte_foersterei();
        let mut new = old.clone();
        plan(&mut new);
        let c = stand_cost(&new, &old, stand);
        (c.cost, c.roof, c.renovation, c.weeks)
    }

    #[test]
    fn a_stands_plan_costs_what_the_original_showed() {
        // STADION-AUSBAU.md §4 and §8, peeked on A.an3.
        assert_eq!(cost(|s| s.set_standing(1, 6143), 1), (10_000, 6_457_000, 122_660, 9));
        assert_eq!(cost(|s| s.set_standing(1, 6123), 1), (1_000, 0, 0, 2));
        assert_eq!(cost(|s| s.set_roof(1, false), 1), (0, 307_000, 0, 1));
        assert_eq!(cost(|s| s.set_heaters(0, true), 0), (0, 0, 72_340, 0));
        assert_eq!(cost(|s| s.set_condition(0, 0), 0), (0, 0, 72_340, 1));
        assert_eq!(cost(|s| s.set_track(0, true), 0), (11_592_000, 7_728_000, 0, 20));
        assert_eq!(cost(|s| s.set_track(1, true), 1), (6_754_000, 6_447_000, 0, 33));
        let old = alte_foersterei();
        let mut fewer = old.clone();
        fewer.set_standing(1, 6123);
        assert_eq!(stand_cost(&fewer, &old, 1).work, Work::New);
        assert_eq!(stand_cost(&old, &old, 1).work, Work::Nothing);
    }

    #[test]
    fn a_stadium_looks_as_the_map_draws_it() {
        // A.an3: StKickers with a roof for the main stand, Manchester for
        // the others, on a big site (STADION-ANSICHT.md §3.1).
        let mut sub = alte_foersterei();
        sub.0[KIND] = 1;
        sub.0[VIEW] = 1;
        assert_eq!(look(&sub), [1, 0, 1, 1, 0, 0, 0, 0, 12, 12, 12, 11]);
        sub.set_track(2, true);
        sub.set_roof(3, false);
        assert_eq!(&look(&sub)[4..], &[1, 0, 0, 0, 17, 2, 12, 11]);
        sub.0[KIND] = 4;
        assert_eq!(look(&sub)[..3], [1, 0x16, 1]);
        sub.0[WEEKS] = 1;
        assert_eq!(look(&sub)[..3], [0, 0, 1]);
    }

    #[test]
    fn a_long_plan_gets_the_builders_offer() {
        let carried = Carried { weeks: 33, cost: 13_201_000, mask: 2, dream: false };
        assert_eq!(builder_offer(&carried), Some(3_300_000));
        assert_eq!(builder_offer(&Carried { weeks: 10, ..carried }), None);
    }
}
