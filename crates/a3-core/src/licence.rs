//! The association's licence for the new season (FINANZUEBERSICHT.md §4.3
//! in the RE repo): how likely the club is to get it, the conditions the
//! association set it (club +0x101 and the amounts at +0x5d8 to +0x5dc),
//! and how far the club breaks them. Only German clubs are judged; the
//! others always get it.

use crate::finances::{self, CLUB_TO_TEAM};
use crate::records::{Records, Team};

/// The country whose clubs the association judges.
const GERMANY: u8 = 14;
/// Club +0x8b (team +0xa9): the human manager; the club's class +0x11e0
/// (a byte at team +0x875); the credit class +0xec (team +0x11c); the
/// conditions +0x101 (team +0x12b) and the refusal at +0x100 (team
/// +0x12a).
const MANAGER: usize = 0xa9;
const CLUB_CLASS: usize = 0x875;
const CREDIT_CLASS: usize = 0x11c;
const CONDITIONS: usize = 0x12b;
const VERDICT: usize = 0x12a;
/// The conditions' bits: no stadium building, no training camps, the
/// spectators to reckon with, the most debt next year, a transfer budget,
/// a transfer surplus, the most player salaries.
pub const NO_BUILDING: u8 = 1;
pub const NO_CAMPS: u8 = 2;
pub const SPECTATORS: u8 = 4;
pub const DEBT: u8 = 8;
pub const TRANSFER_BUDGET: u8 = 0x10;
pub const TRANSFER_SURPLUS: u8 = 0x20;
pub const SALARIES: u8 = 0x40;
/// The verdict's bit: the licence refused.
pub const REFUSED: u8 = 0x80;
/// The amounts, in the club object: the most debt in millions, the
/// spectators in thousands, the salaries and transfers in 100.000s.
const MOST_DEBT: usize = 0x5d8;
const MOST_SPECTATORS: usize = 0x5d9;
const MOST_SALARIES: usize = 0x5da;
const TRANSFER_SUM: usize = 0x5dc;
/// What the breach reckons with: the season's transfers net, the players'
/// salaries, the camps', the stadium's building costs.
const TRANSFERS: usize = 0x258;
const PLAYER_SALARIES: usize = 0x284;
const CAMP_COSTS: usize = 0x2a4;
const BUILDING_COSTS: usize = 0x2d8;
/// The money units (`DAT_00a3b330`, `DAT_00a3b37c`, `DAT_00a3b36c`,
/// `DAT_00a3b348`; peeked in a running original).
pub const MILLION: i64 = 1_000_000;
pub const HUNDRED_THOUSAND: i64 = 100_000;
const BUILDING_ALLOWANCE: i64 = 200_000;
const LARGE_BREACH: i64 = 500_000;
/// The chance's bounds by league: where it is certain and where it is
/// gone (`DAT_00a3b2cc`, `DAT_00a3b2bc`, `DAT_00a3b2e4`, `DAT_00a3b330`,
/// `DAT_00a3b2f4`); a first season's club in more debt than 10 Mio with
/// still less cash than its figure gets it (`DAT_00a3b2d0`).
const FIRST_SEASON_DEBT: i64 = 10_000_000;
/// Down to this the figure stands alone; below, it counts the transfers
/// agreed for the season's end (`FUN_006946ac`: `DAT_00a3b2cc`,
/// `DAT_00a3b2e4`, `DAT_00a3b308`).
const TRANSFERS_COUNTED: [i64; 3] = [12_000_000, 6_000_000, 3_000_000];

fn club<T: crate::records::Field>(team: &Team, at: usize) -> T {
    team.get(at + CLUB_TO_TEAM)
}

/// The conditions the association set, by their bits.
pub fn conditions(team: &Team) -> u8 {
    team.get(CONDITIONS)
}

/// Whether the licence was refused at the season's end.
pub fn refused(team: &Team) -> bool {
    team.get::<u8>(VERDICT) & REFUSED != 0
}

/// The most debt allowed next year; the spectators to reckon with at
/// most; the most player salaries; the transfer sum (budget or surplus).
pub fn most_debt(team: &Team) -> i64 {
    i64::from(club::<u8>(team, MOST_DEBT)) * MILLION
}

pub fn most_spectators(team: &Team) -> i32 {
    i32::from(club::<u8>(team, MOST_SPECTATORS)) * 1000
}

pub fn most_salaries(team: &Team) -> i64 {
    i64::from(club::<u16>(team, MOST_SALARIES)) * HUNDRED_THOUSAND
}

pub fn transfer_sum(team: &Team) -> i64 {
    i64::from(club::<i16>(team, TRANSFER_SUM)) * HUNDRED_THOUSAND
}

/// The club's figure for the licence (`FUN_00623024`): its finances
/// (`FUN_00621eac`), and — where they come to little — the fees of the
/// transfers agreed for the season's end, those it sells and those it buys
/// alike, added (`FUN_0070f041`'s two lists).
pub fn figure(records: &Records, team: &Team) -> i64 {
    let f = finances::figure(team, records.globals.fund_prices());
    if stands_alone(team, f) {
        return f;
    }
    let own = (team.country(), i16::from(team.number()));
    records.agreed().iter().fold(f, |sum, a| {
        let counted = a.seller == own || (a.buyer == own && !a.flagged);
        if counted { sum.wrapping_add(i64::from(a.fee)) } else { sum }
    })
}

/// Whether the figure stands on its own (`FUN_006946ac`): any club abroad,
/// a German one above minus 12 Mio in the first league, 6 Mio in the
/// second, 3 Mio below.
fn stands_alone(team: &Team, f: i64) -> bool {
    if team.country() != GERMANY {
        return true;
    }
    let level = usize::from(team.level()).min(2);
    f > -TRANSFERS_COUNTED[level]
}

/// The chance of the licence for the new season, in percent
/// (`FUN_006941c5`): 100 abroad and for a club without a human manager
/// whose class (+0x11e0) is past the fourth. Otherwise by the figure: in the
/// first league certain above minus 12 Mio, gone below minus 24 Mio, in
/// between 220 less a point a 100.000; in the second league certain above
/// minus 6 Mio, gone below minus 12 Mio, 160 less a point a 100.000;
/// further down certain from minus 1 Mio on, gone from minus 4 Mio, 125
/// less two and a half points a 100.000. The first two leagues give it
/// in the first season to a club over 10 Mio in debt with less cash than
/// its figure. Then the credit class and the club's class add 10 or 5
/// for 0 or 1, take 5 or 10 for 5 or 6. Not kept within 0..100.
pub fn chance(records: &Records, team: &Team) -> i32 {
    if team.country() != GERMANY {
        return 100;
    }
    let class = i16::from(team.get::<u8>(CLUB_CLASS));
    if team.get::<u8>(MANAGER) == 0 && class > 3 {
        return 100;
    }
    let f = figure(records, team);
    let cash = club::<i64>(team, 0x210);
    let first_season = records.globals.seasons() == 1;
    let rich_debtor = first_season && cash <= -FIRST_SEASON_DEBT && cash < f;
    let lacking = (-f) / HUNDRED_THOUSAND;
    let mut chance = match team.level() {
        0 => {
            if rich_debtor || f > -12_000_000 {
                return 100;
            }
            if f < -24_000_000 {
                return 0;
            }
            100 - (lacking - 120) as i32
        }
        1 => {
            if rich_debtor || f > -6_000_000 {
                return 100;
            }
            if f < -12_000_000 {
                return 0;
            }
            100 - (lacking - 60) as i32
        }
        _ => {
            if f >= -MILLION {
                return 100;
            }
            if f <= -4_000_000 {
                return 0;
            }
            100 - ((lacking - 10) * 25 / 10) as i32
        }
    };
    for kind in [i16::from(team.get::<u8>(CREDIT_CLASS)), class] {
        chance += match kind {
            0 => 10,
            1 => 5,
            5 => -5,
            6 => -10,
            _ => 0,
        };
    }
    chance
}

/// How far the club breaks its conditions (`FUN_00694e6f`), nought or
/// below: its debt over the limit, the transfers against their budget or
/// surplus, the salaries over their limit (wherever one is set), the camps'
/// costs where it may hold none, the building costs past 200.000 where it
/// may not build.
pub fn breach(records: &Records, team: &Team) -> i64 {
    let flags = conditions(team);
    if flags == 0 {
        return 0;
    }
    let mut room = 0i64;
    if flags & DEBT != 0 {
        room = most_debt(team).wrapping_sub(figure(records, team));
    }
    let transfers = i64::from(club::<i32>(team, TRANSFERS));
    if flags & TRANSFER_BUDGET != 0 {
        room += transfer_sum(team) + transfers;
    } else if flags & TRANSFER_SURPLUS != 0 {
        room += transfers - transfer_sum(team);
    }
    if club::<u16>(team, MOST_SALARIES) != 0 {
        room += most_salaries(team) - i64::from(club::<i32>(team, PLAYER_SALARIES));
    }
    if flags & NO_CAMPS != 0 {
        room -= i64::from(club::<u32>(team, CAMP_COSTS));
    }
    if flags & NO_BUILDING != 0 {
        let building = club::<i64>(team, BUILDING_COSTS);
        if building > BUILDING_ALLOWANCE {
            room -= building - BUILDING_ALLOWANCE;
        }
    }
    room.min(0)
}

/// How bad a breach is (`FUN_006951ae`): 0 within 200.000, 1 within
/// 500.000, 2 beyond.
pub fn breach_size(breach: i64) -> usize {
    if breach >= -BUILDING_ALLOWANCE {
        0
    } else if breach >= -LARGE_BREACH {
        1
    } else {
        2
    }
}

/// Whether the club's licence verdict falls in a German lower league,
/// which sends a refused club down to the amateurs (`FUN_00676d28`).
pub fn lower_league(team: &Team) -> bool {
    team.country() == GERMANY && (2..=5).contains(&team.level())
}
