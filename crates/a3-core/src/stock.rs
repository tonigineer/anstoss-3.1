//! The stock market and what the managers hold of their own club
//! (VEREINSFUEHRUNG.md §2.6): the shares a club has out, the portfolios'
//! places, and the majority which carries a manager's confidence to 100
//! (`FUN_00405e95`, `FUN_0040689a`, `FUN_00406b4f`, `FUN_007701b2`).

use crate::random::Random;
use crate::records::{Records, Team};

/// The stock market object (`0x8be408`, its data from +0x20): an i32
/// count and entries of 0x108 from +0x14.
const MARKET_AT: u32 = 0x8be408;
const MARKET_COUNT: usize = 0;
const MARKET_FIRST: usize = 0x14;
const MARKET_ENTRY: usize = 0x108;
const CLUB_COUNTRY: usize = 0x1c;
const CLUB_NUMBER: usize = 0x1e;
const CLUB_SHARES: usize = 0x20;
/// The managers' portfolios (`0x8dc6e0[slot − 1]`): fifty places each,
/// a used flag, the market entry held and how many shares.
const PORT_AT: u32 = 0x8dc6e0;
const PORT_SLOTS: usize = 50;
const PORT_USED: usize = 0;
const PORT_ENTRY: usize = 0x34;
const PORT_SHARES: usize = 0x3c;
const PORT_STEP: usize = 0x10;
/// The weeks of prices an entry keeps: the current one stands at
/// `0x38 + week % 52 * 4`, the week itself at `0x24` (`FUN_00407520`).
const PRICES: usize = 52;
const PRICES_AT: usize = 0x38;
const CLUB_WEEK: usize = 0x24;

fn word(b: &[u8], at: usize) -> i16 {
    match (b.get(at), b.get(at + 1)) {
        (Some(lo), Some(hi)) => i16::from_le_bytes([*lo, *hi]),
        _ => 0,
    }
}

fn long(b: &[u8], at: usize) -> i32 {
    match b.get(at..at + 4) {
        Some(four) => i32::from_le_bytes(four.try_into().unwrap()),
        _ => 0,
    }
}

/// The club's place on the market and its shares in all
/// (`FUN_00405e95`).
fn listing(records: &Records, country: u8, number: i16) -> Option<(usize, i32)> {
    let b = records.block(&format!("{MARKET_AT:#x}"))?;
    let count = word(b, MARKET_COUNT).max(0) as usize;
    (0..count).find_map(|k| {
        let e = MARKET_FIRST + k * MARKET_ENTRY;
        if word(b, e + CLUB_COUNTRY) != i16::from(country) || word(b, e + CLUB_NUMBER) != number {
            return None;
        }
        Some((k, long(b, e + CLUB_SHARES)))
    })
}

/// What the manager in this slot holds of that market place
/// (`FUN_0040689a`).
fn holding(records: &Records, slot: i16, place: usize) -> i32 {
    let Some(b) = records.block(&format!("{PORT_AT:#x}[{}]", slot - 1)) else {
        return 0;
    };
    (0..PORT_SLOTS).fold(0, |sum, k| {
        let used = b.get(PORT_USED + k).is_some_and(|&u| u != 0);
        let at = PORT_ENTRY + k * PORT_STEP;
        if !used || usize::try_from(long(b, at)).unwrap_or(usize::MAX) != place {
            return sum;
        }
        sum + long(b, at + (PORT_SHARES - PORT_ENTRY))
    })
}

/// The shares as the board screen shows them (§2.6): the shares in all,
/// the manager's own, and the other humans' by their place among the
/// managers. Nothing when the club is not on the market.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Split {
    pub total: i32,
    pub own: i32,
    pub others: Vec<(usize, i32)>,
}

/// The manager's place among the humans, 1 to 4, from the slots the
/// globals keep; 0 for a manager not in play. His portfolio of shares
/// stands at `0x8dc6e0[place − 1]`.
pub fn place(records: &Records, index: usize) -> i16 {
    records.globals.slots().get(index).copied().unwrap_or(0)
}

/// What all of this human's holdings come to (`FUN_006ddc73`): his
/// shares in every club still on the market, each at that club's current
/// price, all summed.
pub fn portfolio_value(records: &Records, human: i16) -> i64 {
    let Some(port) = records.block(&format!("{PORT_AT:#x}[{}]", human - 1)) else {
        return 0;
    };
    let Some(market) = records.block(&format!("{MARKET_AT:#x}")) else {
        return 0;
    };
    (0..PORT_SLOTS).fold(0, |sum, k| {
        if port.get(PORT_USED + k).is_none_or(|&used| used == 0) {
            return sum;
        }
        let at = PORT_ENTRY + k * PORT_STEP;
        let Ok(place) = usize::try_from(long(port, at)) else { return sum };
        let Some(entry) = market.get(MARKET_FIRST + place * MARKET_ENTRY..) else {
            return sum;
        };
        if word(entry, CLUB_COUNTRY) == 0 {
            return sum;
        }
        let Ok(ring) = usize::try_from(word(entry, CLUB_WEEK)) else { return sum };
        let Some(four) = entry.get(PRICES_AT + ring % PRICES * 4..PRICES_AT + ring % PRICES * 4 + 4)
        else {
            return sum;
        };
        let price = f32::from_le_bytes(four.try_into().unwrap());
        let shares = long(port, at + (PORT_SHARES - PORT_ENTRY)) as f32;
        sum + (shares * price).round() as i64
    })
}

/// The club's shares among the humans (`FUN_005b548f`'s reading): the
/// manager's own holding and every other human's, in the order the
/// board screen's rows take them.
pub fn split(records: &Records, slot: i16, country: u8, number: i16) -> Option<Split> {
    let (place, total) = listing(records, country, number)?;
    if total <= 0 {
        return None;
    }
    let others = records
        .globals
        .slots()
        .iter()
        .enumerate()
        .filter(|(_, s)| **s > 0 && **s != slot)
        .map(|(i, &s)| (i, holding(records, s, place)))
        .collect();
    Some(Split { total, own: holding(records, slot, place), others })
}

/// Whether the manager at this place among the managers holds the
/// majority of his own club's shares (`FUN_00406b4f`): the club is on
/// the market and he holds more than half of it.
pub fn majority(records: &Records, index: usize) -> bool {
    let Some(manager) = records.managers.get(index) else { return false };
    let slot = place(records, index);
    if slot == 0 || manager.country() == 0 {
        return false;
    }
    listing(records, manager.country() as u8, manager.team() as i16)
        .is_some_and(|(place, total)| total != 0 && holding(records, slot, place) > total / 2)
}

/// The board's confidence as the screens show it: 100 for a manager
/// holding the majority of his own club, else as he stands
/// (`FUN_007701b2`).
pub fn confidence(records: &Records, index: usize) -> i32 {
    match records.managers.get(index) {
        Some(_) if majority(records, index) => 100,
        Some(manager) => manager.shown_confidence(),
        None => 0,
    }
}

/// A club going public on the market (`FUN_00406201` in a game on one
/// machine): one already there takes the shares; a new one goes on at the
/// end with its first price (`FUN_00404dcd`), and the count grows.
pub fn list(records: &mut Records, index: usize, shares: i32, random: &mut Random) {
    let Some(team) = records.teams.get(index) else { return };
    let (country, number) = (i16::from(team.country()), i16::from(team.number()));
    let Some(mut market) = records.block(&format!("{MARKET_AT:#x}")).map(<[u8]>::to_vec) else {
        return;
    };
    let count = usize::try_from(long(&market, MARKET_COUNT)).unwrap_or(0);
    let found = (0..count).map(|k| MARKET_FIRST + k * MARKET_ENTRY).find(|&e| {
        word(&market, e + CLUB_COUNTRY) == country && word(&market, e + CLUB_NUMBER) == number
    });
    if let Some(e) = found {
        put_long(&mut market, e + CLUB_SHARES, shares);
    } else {
        let e = MARKET_FIRST + count * MARKET_ENTRY;
        if market.len() < e + MARKET_ENTRY {
            return;
        }
        let name: Vec<u8> = team.bytes().iter().copied().take_while(|&b| b != 0).collect();
        {
            let entry = &mut market[e..e + MARKET_ENTRY];
            put_word(entry, CLUB_WEEK, NEW_WEEK);
            put_long(entry, CLUB_SHARES, NO_SHARES);
            put_word(entry, CLUB_COUNTRY, country);
            put_word(entry, CLUB_NUMBER, number);
            put_long(entry, NAME_END, 0);
            // The name goes in as `strcpy` puts it, over what follows.
            let end = (name.len() + 1).min(MARKET_ENTRY);
            entry[..end].copy_from_slice(&[&name[..], &[0]].concat()[..end]);
            put_long(entry, CLUB_SHARES, shares);
        }
        let price = first_price(records, team, &mut market, e, random);
        let entry = &mut market[e..e + MARKET_ENTRY];
        entry[HIGH..HIGH + 4].copy_from_slice(&price.to_le_bytes());
        entry[LOW..LOW + 4].copy_from_slice(&price.to_le_bytes());
        put_long(&mut market, MARKET_COUNT, count as i32 + 1);
    }
    if let Some(block) = records.block_mut(&format!("{MARKET_AT:#x}")) {
        block.copy_from_slice(&market);
    }
}

/// A new entry's week in the ring before its first price, the shares it
/// holds until it takes the club's, and where its name ends.
const NEW_WEEK: i16 = 51;
const NO_SHARES: i32 = 99_999_999;
const NAME_END: usize = 0x14;
/// What else an entry keeps: a bonus on its price in percent, the
/// price's drift, the shares to be had, the change on the week before in
/// percent, the highest and lowest price.
const BONUS: usize = 0x18;
const DRIFT: usize = 0x26;
const TO_BE_HAD: usize = 0x28;
const CHANGE: usize = 0x2c;
const HIGH: usize = 0x30;
const LOW: usize = 0x34;
const DRIFT_MOST: i16 = 50;
/// The least a share is priced at (`DAT_00a3b424`).
const LEAST_PRICE: f32 = 5.0;
/// The team's fields the price goes by: its table place, last season's,
/// its squad's rank (club +0x109, +0x10b, +0x107); the honours it won
/// (`FUN_0069a031`), a cup run's bit (`FUN_0069a261`), its form at
/// home and away by three bits each (club +0xd8).
const PLACE: usize = 0x132;
const LAST_PLACE: usize = 0x134;
const SQUAD_RANK: usize = 0x130;
const HONOURS: usize = 0x8ea;
const RUNS: usize = 0x110;
const CUP_RUN: u16 = 0x1000;
const FORM: usize = 0xfc;
/// The European cups' holders, three of country and number
/// (`DAT_00ae6dd4`, `FUN_0042419d`).
const HOLDERS_AT: u32 = 0xae6dd4;
const HOLDERS: usize = 3;
/// The league's teams and its matchdays played, in its record.
const LEAGUE_AT: u32 = 0x1a09d48;
const LEAGUE_TEAMS: usize = 0x30;
const LEAGUE_PLAYED: usize = 0x46;
const LOWEST_LEAGUE: u8 = 6;
const DEFAULT_TEAMS: i16 = 18;

fn put_word(b: &mut [u8], at: usize, v: i16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_long(b: &mut [u8], at: usize, v: i32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn float(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// A club's price for the week (`FUN_00405024` for a club whose country
/// keeps its teams): from 120, 60 or 30 by its league's level, six points a
/// place its squad ranks above the league's last, four a place it stands
/// above the last, now or last season as the season has gone on, ten for
/// the top; 30, 20 and 10 for its honours, half or one and a half of
/// those again for a cup run or a European cup held; its form at home and
/// away rolled in; its drift; up to a euro more. Then the bonus, at least
/// five euros, the change on the week before, and the highest and lowest.
fn first_price(
    records: &Records,
    team: &Team,
    market: &mut [u8],
    e: usize,
    random: &mut Random,
) -> f32 {
    let mut week = word(&market[e..], CLUB_WEEK) - 1;
    if week < 0 {
        week = NEW_WEEK;
    }
    put_word(&mut market[e..], CLUB_WEEK, week);
    let mut base: f32 = match team.level() {
        0 => 120.0,
        1 => 60.0,
        _ => 30.0,
    };
    let (teams, played) = league(records, team);
    let (now, last): (f32, f32) = match played {
        0 => (0.0, 1.0),
        1 => (0.2, 0.8),
        2 => (0.4, 0.6),
        3 => (0.6, 0.4),
        4 => (0.8, 0.2),
        _ => (1.0, 0.0),
    };
    let above = |at: usize| f64::from(i32::from(teams) - i32::from(team.get::<u8>(at)));
    base = (above(PLACE) * f64::from(now) * 4.0 + f64::from(base)) as f32;
    base = (above(LAST_PLACE) * f64::from(last) * 4.0 + f64::from(base)) as f32;
    base = (above(SQUAD_RANK) * 6.0 + f64::from(base)) as f32;
    if team.get::<u8>(PLACE) == 1 {
        base += 10.0;
    }
    let honours = team.get::<u8>(HONOURS);
    let mut won = 0.0f32;
    for (bits, points) in [(1, 30.0), (2, 20.0), (3, 10.0)] {
        if honours & bits != 0 {
            base += points;
            won += points;
        }
    }
    if team.get::<u16>(RUNS) & CUP_RUN != 0 {
        base = (f64::from(won) * 0.5 + f64::from(base)) as f32;
    }
    if holds_cup(records, team) {
        base = (f64::from(won) * 1.5 + f64::from(base)) as f32;
    }
    let form = team.get::<i16>(FORM);
    for (bits, most) in [(form & 7, 20), ((form >> 3) & 7, 10)] {
        base = match bits {
            0 | 1 => base + random.below(most) as f32,
            2 => base - random.below(most) as f32,
            3 => (f64::from(random.below(10)) - 5.0 + f64::from(base)) as f32,
            _ => base,
        };
    }
    let entry = &mut market[e..e + MARKET_ENTRY];
    let drift = word(entry, DRIFT).wrapping_add(2 - random.below(5) as i16);
    let drift = drift.clamp(-DRIFT_MOST, DRIFT_MOST);
    put_word(entry, DRIFT, drift);
    base = (f64::from(drift) + f64::from(base)) as f32;
    let ring = |w: i16| PRICES_AT + usize::try_from(w).unwrap_or(0) % PRICES * 4;
    let mut price = (f64::from(random.below(100)) / 100.0 + f64::from(base)) as f32;
    let to_be_had = random.below(0x97) + random.below(0x47) + 0x5a + random.below(0x1f);
    let to_be_had = to_be_had as i32 * 10;
    put_long(entry, TO_BE_HAD, to_be_had);
    let held: i32 = (0..crate::records::MANAGERS as i16)
        .map(|slot| held(records, market, slot + 1, team))
        .fold(0, i32::wrapping_add);
    let entry = &mut market[e..e + MARKET_ENTRY];
    let free = long(entry, CLUB_SHARES).wrapping_sub(held);
    if long(entry, TO_BE_HAD) > free {
        put_long(entry, TO_BE_HAD, free);
    }
    price = (f64::from(float(entry, BONUS)) / 100.0 * f64::from(price) + f64::from(price)) as f32;
    if price < LEAST_PRICE {
        price = LEAST_PRICE;
    }
    entry[ring(week)..ring(week) + 4].copy_from_slice(&price.to_le_bytes());
    let before = float(entry, ring((week + 1) % PRICES as i16));
    let change = if before == 0.0 {
        0.0
    } else {
        let points = ((f64::from(price) - f64::from(before)) * 100.0) as f32;
        (f64::from(points) / f64::from(before)) as f32
    };
    entry[CHANGE..CHANGE + 4].copy_from_slice(&change.to_le_bytes());
    if price > float(entry, HIGH) {
        entry[HIGH..HIGH + 4].copy_from_slice(&price.to_le_bytes());
    }
    if price < float(entry, LOW) {
        entry[LOW..LOW + 4].copy_from_slice(&price.to_le_bytes());
    }
    price
}

/// The club's league: how many teams it has (`FUN_00692a88`) and how many
/// matchdays it has played where the game follows it (`FUN_00692744`).
fn league(records: &Records, team: &Team) -> (i16, i16) {
    if team.level() >= LOWEST_LEAGUE {
        return (DEFAULT_TEAMS, 0);
    }
    let country = usize::from(team.country());
    let index = records.countries.first_league(country) + i16::from(team.level());
    let block = records.block(&format!("{LEAGUE_AT:#x}[{index}]"));
    let teams = block.map_or(0, |b| word(b, LEAGUE_TEAMS));
    let played = records.globals.played(country as i16);
    (teams, if played { block.map_or(0, |b| word(b, LEAGUE_PLAYED)) } else { 0 })
}

/// Whether the club holds a European cup (`FUN_0042419d`).
fn holds_cup(records: &Records, team: &Team) -> bool {
    records.block(&format!("{HOLDERS_AT:#x}")).is_some_and(|b| {
        (0..HOLDERS).any(|k| {
            b.get(k * 4) == Some(&team.country())
                && word(b, k * 4 + 2) as u16 == u16::from(team.number())
        })
    })
}

/// What the human at this place among the managers holds of a club, by
/// the first of his places that holds it (`FUN_0040689a`), reading the
/// market as it now is.
fn held(records: &Records, market: &[u8], slot: i16, team: &Team) -> i32 {
    let Some(b) = records.block(&format!("{PORT_AT:#x}[{}]", slot - 1)) else {
        return 0;
    };
    (0..PORT_SLOTS)
        .find(|&k| {
            let at = PORT_ENTRY + k * PORT_STEP;
            let e = (long(b, at) as usize).wrapping_mul(MARKET_ENTRY).wrapping_add(MARKET_FIRST);
            b.get(PORT_USED + k).is_some_and(|&u| u != 0)
                && word(market, e.wrapping_add(CLUB_COUNTRY)) == i16::from(team.country())
                && word(market, e.wrapping_add(CLUB_NUMBER)) == i16::from(team.number())
        })
        .map_or(0, |k| long(b, PORT_SHARES + k * PORT_STEP))
}
