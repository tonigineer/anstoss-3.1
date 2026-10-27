//! The transfer market as the transfer list shows it (TRANSFERLISTE.md in
//! the RE repo): the players listed for sale or loan, the negotiation
//! appointments of the week, and a club's watch list.

use crate::records::Records;

/// The transfer list (`0x1c49278`, its data from +0x20): an i16 count,
/// then up to 400 entries of 0x1c bytes from +8.
const LIST_AT: u32 = 0x1c49278;
const LIST_FIRST: usize = 8;
const LIST_ENTRY: usize = 0x1c;
const LIST_SIZE: usize = 400;
/// The negotiation appointments (`0x1c54000`): an i16 count, entries of
/// 0x4c bytes from +4, the day at +2, the player at +6.
const TALKS_AT: u32 = 0x1c54000;
const TALKS_FIRST: usize = 4;
const TALK: usize = 0x4c;
/// The human clubs' watch lists (`0xa4d0f8[slot]`): an i16 count, then 50
/// players of 16 bytes from +4, the id first.
const WATCH_AT: u32 = 0xa4d0f8;
const WATCH_FIRST: usize = 4;
const WATCHED: usize = 16;
pub const WATCH_SIZE: usize = 50;

/// What a listing offers: a sale, a loan, a loan with an option to buy.
pub const SALE: u8 = 1;
pub const LOAN: u8 = 2;
pub const LOAN_WITH_OPTION: u8 = 3;
/// A listing's terms: 2 "ablösefrei".
pub const FREE_TRANSFER: u8 = 2;

/// A player on the transfer list, as listed: his club, his main position
/// and strength then, the day, the kind and terms, the loan's fee and the
/// option's price.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Listing {
    pub id: u16,
    pub country: u8,
    pub number: u16,
    pub position: u8,
    pub strength: u8,
    pub day: i16,
    pub kind: u8,
    pub terms: u8,
    pub fee: i32,
    pub option: i32,
}

fn word(b: &[u8], at: usize) -> i16 {
    b.get(at..at + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))
}

fn long(b: &[u8], at: usize) -> i32 {
    b.get(at..at + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))
}

fn list_name() -> String {
    format!("{LIST_AT:#x}")
}

/// The players on the transfer list, in the list's order.
pub fn listings(records: &Records) -> Vec<Listing> {
    let Some(b) = records.block(&list_name()) else { return Vec::new() };
    let count = usize::try_from(word(b, 0)).unwrap_or(0).min(LIST_SIZE);
    (0..count)
        .map(|k| {
            let e = LIST_FIRST + k * LIST_ENTRY;
            let byte = |o: usize| b.get(e + o).copied().unwrap_or(0);
            Listing {
                id: word(b, e) as u16,
                country: byte(2),
                number: word(b, e + 4) as u16,
                position: byte(6),
                strength: byte(7),
                day: word(b, e + 0xa),
                kind: byte(0xc) & 3,
                terms: byte(0xd),
                fee: long(b, e + 0x10),
                option: long(b, e + 0x14),
            }
        })
        .collect()
}

/// The list as the screen sorts it (`FUN_00759d61`): by main position,
/// then the strongest first, by selection and swap — not stably.
pub fn sorted(mut list: Vec<Listing>) -> Vec<Listing> {
    let n = list.len();
    for i in 0..n.saturating_sub(1) {
        let mut m = i;
        for j in i + 1..n {
            if list[j].position < list[m].position
                || (list[m].position == list[j].position && list[m].strength < list[j].strength)
            {
                m = j;
            }
        }
        if m != i {
            list.swap(i, m);
        }
    }
    list
}

/// A player taken off the transfer list: the entries after his move up
/// (`FUN_00758f42`), and his record's mark goes (`FUN_0075a636`).
pub fn take_off(records: &mut Records, id: u16) {
    if let Some(b) = records.block_mut(&list_name()) {
        let count = usize::try_from(word(b, 0)).unwrap_or(0).min(LIST_SIZE);
        let at = (0..count).find(|&k| word(b, LIST_FIRST + k * LIST_ENTRY) as u16 == id);
        if let Some(k) = at {
            let from = LIST_FIRST + (k + 1) * LIST_ENTRY;
            let end = (LIST_FIRST + LIST_SIZE * LIST_ENTRY).min(b.len());
            if from < end {
                b.copy_within(from..end, from - LIST_ENTRY);
            }
            b[0..2].copy_from_slice(&((count - 1) as i16).to_le_bytes());
        }
    }
    if let Some(r) = records.players.get_mut(usize::from(id)) {
        r.set_retiring(r.retiring() & !LISTED);
    }
}

/// A record's mark of a player on the transfer list (R+0x26f bit 0).
const LISTED: u8 = 1;

/// The day of the week's negotiation appointment a player has, if any
/// club has one with him (`FUN_00762a93`).
pub fn talk_day(records: &Records, id: u16) -> Option<i16> {
    let b = records.block(&format!("{TALKS_AT:#x}"))?;
    let count = usize::try_from(word(b, 0)).unwrap_or(0);
    (0..count)
        .map(|k| TALKS_FIRST + k * TALK)
        .find(|&e| word(b, e + 6) as u16 == id)
        .map(|e| word(b, e + 2))
}

fn watch_name(slot: usize) -> String {
    format!("{WATCH_AT:#x}[{slot}]")
}

/// The players a human club watches, by its slot: all fifty places, as
/// the list's colours read them (`FUN_0069904c`).
pub fn watched(records: &Records, slot: usize) -> Vec<u16> {
    let Some(b) = records.block(&watch_name(slot)) else { return Vec::new() };
    (0..WATCH_SIZE)
        .map(|k| word(b, WATCH_FIRST + k * WATCHED) as u16)
        .filter(|&id| id != 0)
        .collect()
}

/// What putting a player on the watch list came to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Watch {
    Added,
    Already,
    Full,
}

/// A player put on a club's watch list (`FUN_00418741`): after the others,
/// unless he is there already or the list is full.
pub fn watch(records: &mut Records, slot: usize, id: u16) -> Watch {
    let Some(b) = records.block_mut(&watch_name(slot)) else { return Watch::Full };
    let count = usize::try_from(word(b, 0)).unwrap_or(0);
    if count >= WATCH_SIZE {
        return Watch::Full;
    }
    if (0..count).any(|k| word(b, WATCH_FIRST + k * WATCHED) as u16 == id) {
        return Watch::Already;
    }
    let at = WATCH_FIRST + count * WATCHED;
    b[at..at + 2].copy_from_slice(&id.to_le_bytes());
    b[0..2].copy_from_slice(&((count + 1) as i16).to_le_bytes());
    Watch::Added
}

/// A negotiation appointment (TERMINE-BEOBACHTUNG.md §1.1): its kind, the
/// day, the player, and the clubs taking part — his own first.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Appointment {
    pub kind: i16,
    pub day: i16,
    pub player: u16,
    pub clubs: Vec<(u8, i16)>,
}

/// The appointments' kinds: poaching a player not on the transfer list
/// (not shown to others), a purchase, a loan; the first is never booked.
pub const RELEASE_FEE_TALK: i16 = 1;
pub const POACHING: i16 = 2;
pub const PURCHASE: i16 = 3;
pub const LOAN_TALK: i16 = 4;
const MOST_CLUBS: usize = 10;
const CLUB_AT: usize = 0xc;
const CLUB: usize = 6;

/// The week's appointments, in the order kept.
pub fn appointments(records: &Records) -> Vec<Appointment> {
    let Some(b) = records.block(&format!("{TALKS_AT:#x}")) else { return Vec::new() };
    let count = usize::try_from(word(b, 0)).unwrap_or(0);
    (0..count)
        .map(|k| {
            let e = TALKS_FIRST + k * TALK;
            let n = usize::try_from(word(b, e + 4)).unwrap_or(0).min(MOST_CLUBS);
            Appointment {
                kind: word(b, e),
                day: word(b, e + 2),
                player: word(b, e + 6) as u16,
                clubs: (0..n)
                    .map(|i| {
                        let at = e + CLUB_AT + i * CLUB;
                        (b.get(at).copied().unwrap_or(0), word(b, at + 2))
                    })
                    .collect(),
            }
        })
        .collect()
}

/// A watched player (16 bytes): his id, the marks of his last five
/// matches watched, newest first, the matches watched and his form's sum
/// over them (TERMINE-BEOBACHTUNG.md §3.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Watched {
    pub id: u16,
    pub marks: [u8; 5],
    pub matches: i16,
    pub form_sum: i32,
}

/// The watch list's places with a player, all fifty, in place order.
pub fn watch_list(records: &Records, slot: usize) -> Vec<Watched> {
    let Some(b) = records.block(&watch_name(slot)) else { return Vec::new() };
    (0..WATCH_SIZE)
        .map(|k| WATCH_FIRST + k * WATCHED)
        .filter_map(|e| {
            let id = word(b, e) as u16;
            (id != 0).then(|| Watched {
                id,
                marks: std::array::from_fn(|i| b.get(e + 2 + i).copied().unwrap_or(0)),
                matches: word(b, e + 8),
                form_sum: long(b, e + 0xc),
            })
        })
        .collect()
}

/// A player taken off the watch list: the places after his move up, the
/// last used one is cleared (`FUN_00418831`).
pub fn unwatch(records: &mut Records, slot: usize, id: u16) {
    let Some(b) = records.block_mut(&watch_name(slot)) else { return };
    let count = usize::try_from(word(b, 0)).unwrap_or(0).min(WATCH_SIZE);
    let Some(k) = (0..count).find(|&k| word(b, WATCH_FIRST + k * WATCHED) as u16 == id) else {
        return;
    };
    let end = WATCH_FIRST + count * WATCHED;
    b.copy_within(WATCH_FIRST + (k + 1) * WATCHED..end, WATCH_FIRST + k * WATCHED);
    b[end - WATCHED..end].fill(0);
    b[0..2].copy_from_slice(&((count - 1) as i16).to_le_bytes());
}

/// The scouts out by region: the day each comes back, 0 none
/// (TERMINE-BEOBACHTUNG.md §3.4).
const SCOUTS_AT: usize = 0x32e;
pub const REGIONS: usize = 5;
const SCOUT_DAYS: i16 = 28;

pub fn scouts(records: &Records, slot: usize) -> [i16; REGIONS] {
    let b = records.block(&watch_name(slot)).unwrap_or(&[]);
    std::array::from_fn(|i| word(b, SCOUTS_AT + 2 * i))
}

/// A scout sent to a region for four weeks.
pub fn send_scout(records: &mut Records, slot: usize, region: usize, today: i16) {
    if let Some(b) = records.block_mut(&watch_name(slot)) {
        let at = SCOUTS_AT + 2 * region;
        b[at..at + 2].copy_from_slice(&(today + SCOUT_DAYS).to_le_bytes());
    }
}

/// Two more lists a player can have a talk pending in: the national
/// team's search (6-byte entries, the id first) and another (8-byte
/// entries, the id at +2), each after an i16 count (`FUN_007647b4`).
const PENDING_LISTS: [(u32, usize, usize); 2] = [(0x1c53d98, 6, 0), (0x1c53ee8, 8, 2)];

/// Whether a talk with a player is pending anywhere (`FUN_007647b4`).
pub fn pending(records: &Records, id: u16) -> bool {
    let in_list = |(at, size, field): (u32, usize, usize)| {
        records.block(&format!("{at:#x}")).is_some_and(|b| {
            let count = usize::try_from(word(b, 0)).unwrap_or(0);
            (0..count).any(|k| word(b, 2 + k * size + field) as u16 == id)
        })
    };
    PENDING_LISTS.into_iter().any(in_list) || appointments(records).iter().any(|t| t.player == id)
}

/// A club as the appointments name it: its country and number.
pub type Side = (u8, i16);

/// At most this many appointments; this many clubs at one table.
const MOST_TALKS: usize = 200;
const MOST_JOINED: usize = 7;
/// A human club's weight at the table (`FUN_0077ee79` answers 10).
const HUMAN_WEIGHT: u8 = 10;
/// An appointment's fields: kind, day, how many clubs, the player, his
/// club, the clubs at +0xc (country, number, status, weight), the terms.
const TALK_KIND: usize = 0;
const TALK_DAY: usize = 2;
const TALK_CLUBS: usize = 4;
const TALK_PLAYER: usize = 6;
const TALK_HIS_CLUB: usize = 8;
const TALK_TERMS: usize = 0x38;
pub const TERMS: usize = 20;

fn put16(b: &mut [u8], at: usize, v: i16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_side(b: &mut [u8], at: usize, (country, number): Side, status: u8, weight: u8) {
    b[at] = country;
    b[at + 1] = 0;
    put16(b, at + 2, number);
    b[at + 4] = status;
    b[at + 5] = weight;
}

/// An appointment's clubs, by entry (participant k at +0xc + 6k).
fn side(b: &[u8], e: usize, k: usize) -> Side {
    let at = e + CLUB_AT + k * CLUB;
    (b.get(at).copied().unwrap_or(0), word(b, at + 2))
}

/// A new appointment (`FUN_0076254d`): the player, his club at the table
/// and ours after it, a human's weight; only while there is room and the
/// day has not gone by. Whether it was written.
pub fn book(
    records: &mut Records,
    kind: i16,
    day: i16,
    player: u16,
    seller: Side,
    buyer: Side,
    terms: [u8; TERMS],
) -> bool {
    let today = records.globals.day() as i16;
    let Some(b) = records.block_mut(&format!("{TALKS_AT:#x}")) else { return false };
    let count = usize::try_from(word(b, 0)).unwrap_or(0);
    let e = TALKS_FIRST + count * TALK;
    if count >= MOST_TALKS || day < today || b.len() < e + TALK {
        return false;
    }
    b[e..e + TALK].fill(0);
    put16(b, e + TALK_KIND, kind);
    put16(b, e + TALK_DAY, day);
    put16(b, e + TALK_CLUBS, 2);
    put16(b, e + TALK_PLAYER, player as i16);
    b[e + TALK_HIS_CLUB] = seller.0;
    put16(b, e + TALK_HIS_CLUB + 2, seller.1);
    put_side(b, e + CLUB_AT, seller, 0, 0);
    put_side(b, e + CLUB_AT + CLUB, buyer, 0, HUMAN_WEIGHT);
    b[e + TALK_TERMS..e + TALK_TERMS + TERMS].copy_from_slice(&terms);
    put16(b, 0, count as i16 + 1);
    true
}

/// Our club joins the player's appointment (`FUN_00762691`): not a full
/// table, not twice; after the second club, before those weighing less
/// than a human. Whether it joined.
pub fn join(records: &mut Records, player: u16, buyer: Side) -> bool {
    let Some(b) = records.block_mut(&format!("{TALKS_AT:#x}")) else { return false };
    let count = usize::try_from(word(b, 0)).unwrap_or(0);
    let Some(e) = (0..count)
        .map(|k| TALKS_FIRST + k * TALK)
        .find(|&e| word(b, e + TALK_PLAYER) as u16 == player)
    else {
        return false;
    };
    let n = usize::try_from(word(b, e + TALK_CLUBS)).unwrap_or(0);
    if n >= MOST_JOINED || (0..n).any(|k| side(b, e, k) == buyer) {
        return false;
    }
    let mut k = if n == 1 { 0 } else { n - 1 };
    if n != 1 {
        while k > 1 && b[e + CLUB_AT + k * CLUB + 5] < HUMAN_WEIGHT {
            let at = e + CLUB_AT + k * CLUB;
            b.copy_within(at..at + CLUB, at + CLUB);
            k -= 1;
        }
    }
    put_side(b, e + CLUB_AT + (k + 1) * CLUB, buyer, 0, HUMAN_WEIGHT);
    put16(b, e + TALK_CLUBS, n as i16 + 1);
    true
}

/// How many of this week's appointments a club sits at (`FUN_00761f7a`):
/// from the week's first day to its last.
pub fn talks_in_week(records: &Records, club: Side) -> usize {
    let start = records.globals.week_start();
    appointments(records)
        .iter()
        .filter(|t| (start..=start + 6).contains(&t.day))
        .map(|t| t.clubs.iter().filter(|&&c| c == club).count())
        .sum()
}

/// Whether a club sits at an appointment on a day (`FUN_00761c43`).
pub fn talks_on(records: &Records, club: Side, day: i16) -> bool {
    appointments(records).iter().any(|t| t.day == day && t.clubs.contains(&club))
}

/// The contract as the player's object holds it, which a poaching talk
/// takes along (L+0x88 … +0x9b, `FUN_00636638`): salary, bonuses, the
/// release fee and +0x158, the years left, the terms, +0x162 and +0x160.
/// (The original's L+0x98 matched R+0x162 in both talks seen, not the
/// R+0x161 it is said to copy; VERHANDLUNG-BUCHUNG.md keeps it open.)
pub fn terms(records: &Records, player: u16) -> [u8; TERMS] {
    let mut t = [0; TERMS];
    let Some(r) = records.player(player) else { return t };
    let years = (i32::from(r.contract() & 0xfff) - i32::from(records.globals.start_year())).max(0);
    t[0..2].copy_from_slice(&r.salary().to_le_bytes());
    t[2..4].copy_from_slice(&r.appearance_bonus().to_le_bytes());
    t[4..6].copy_from_slice(&r.goal_bonus().to_le_bytes());
    t[6..8].copy_from_slice(&r.fixed_fee().to_le_bytes());
    t[8..12].copy_from_slice(&r.get::<u32>(0x158).to_le_bytes());
    t[12] = years as u8;
    t[14..16].copy_from_slice(&r.terms().to_le_bytes());
    t[16] = r.get::<u8>(0x162);
    t[17] = r.get::<u8>(0x160);
    t
}

/// A listing's terms as the club sets them: 1 for a fee, 2 "ablösefrei".
pub const FOR_A_FEE: u8 = 1;

/// The terms of his listing, if he is on the list (`FUN_0075b308`, the
/// entry's +0xd): the club may take him off only when it is 1.
pub fn listed_terms(records: &Records, id: u16) -> Option<u8> {
    let b = records.block(&list_name())?;
    let count = usize::try_from(word(b, 0)).unwrap_or(0).min(LIST_SIZE);
    (0..count)
        .map(|k| LIST_FIRST + k * LIST_ENTRY)
        .find(|&e| word(b, e) as u16 == id)
        .and_then(|e| b.get(e + 0xd).copied())
}

/// Him onto the transfer list for sale (`FUN_0075a129` → `FUN_007592fa`
/// with kind 1): his club, main position and strength, today, the terms
/// — `free` "ablösefrei" —, and his record marked; false when the list is
/// full or holds him. The entry's +8 (a strength outlook,
/// `FUN_006ff516`) and +0xe (a value factor, `FUN_00708b80`), which the
/// list's screen does not show, stay 0.
pub fn list_for_sale(records: &mut Records, id: u16, free: bool) -> bool {
    let Some(r) = records.player(id) else { return false };
    let [country, number] = r.club();
    let (position, strength) = (r.positions()[0], r.strength());
    let day = records.globals.day() as i16;
    let Some(b) = records.block_mut(&list_name()) else { return false };
    let count = usize::try_from(word(b, 0)).unwrap_or(0).min(LIST_SIZE);
    if count >= LIST_SIZE || (0..count).any(|k| word(b, LIST_FIRST + k * LIST_ENTRY) as u16 == id)
    {
        return false;
    }
    let e = LIST_FIRST + count * LIST_ENTRY;
    if b.len() < e + LIST_ENTRY {
        return false;
    }
    b[e..e + LIST_ENTRY].fill(0);
    put16(b, e, id as i16);
    b[e + 2] = country;
    put16(b, e + 4, i16::from(number));
    b[e + 6] = position;
    b[e + 7] = strength;
    put16(b, e + 0xa, day);
    b[e + 0xc] = SALE;
    b[e + 0xd] = if free { FREE_TRANSFER } else { FOR_A_FEE };
    put16(b, 0, count as i16 + 1);
    if let Some(r) = records.players.get_mut(usize::from(id)) {
        r.set_retiring(r.retiring() | LISTED);
    }
    true
}
