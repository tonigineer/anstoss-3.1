//! What the game tells of the next opponent (dialogs 81, 82 and 83:
//! `CGegnerInfoLeer`, `CGegnerInfoSonstiges`, `CGegnerInfoStandard`; the RE
//! repo's GEGNERINFO.md), from the save: its board and staff, its finances,
//! its stand in the league, the cup and the European cups, its record at
//! home and away and against your team, its stadium and gates, its titles,
//! and the results of the last ten seasons against it.

use crate::calendar::Calendar;
use crate::records::{Records, Stadium, Team};

/// Which of the three dialogs the game opens (`FUN_004b1eea`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// A club of a country played, in a league: dialog 83.
    Standard,
    /// A club of a country kept only for the European cups: dialog 82.
    Foreign,
    /// Anything else: dialog 81.
    Empty,
}

/// Levels from this on are no league.
const NO_LEAGUE: u8 = 10;

pub fn kind(records: &Records, country: u16, level: u8) -> Kind {
    if records.globals.played(country as i16) && level < NO_LEAGUE {
        Kind::Standard
    } else if records.countries.kind(country.into()) == 2 {
        Kind::Foreign
    } else {
        Kind::Empty
    }
}

/// The nations the game can be played in, whose stadiums it keeps even when
/// they are not played (`DAT_0089ddd8`).
const PLAYABLE: [u16; 11] = [14, 16, 20, 24, 28, 47, 51, 57, 59, 63, 67];

/// A club's stadium (`FUN_00638335`, the two branches the dialogs reach).
pub fn stadium(records: &Records, country: u16, number: u16) -> Option<&Stadium> {
    let countries = &records.countries;
    let count = countries.team_count(country.into());
    if i32::from(number) >= i32::from(count) {
        return None;
    }
    let index = i32::from(countries.first_team(country.into())) + i32::from(number);
    let played = records.globals.played(country as i16) || PLAYABLE.contains(&country);
    if played {
        records.stadiums.get(usize::try_from(index).ok()?)
    } else if countries.kind(country.into()) == 2 {
        let foreign = index - i32::from(countries.foreign_start());
        records.foreign_stadiums.get(usize::try_from(foreign).ok()?)
    } else {
        None
    }
}

/// Who runs a club (`FUN_004b267e` step 5): its trainer, its manager, and
/// how safe the job is (0 safe, 1–2 wobbling, 3 and more about to go);
/// `None` where the dialog keeps its own text.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Staff {
    pub trainer: String,
    pub manager: String,
    pub job: Option<u8>,
}

/// A human manager's name, or `None` while his slot is not in play
/// (`FUN_006a537d`).
fn human_name(records: &Records, number: u8) -> Option<String> {
    let slot = usize::from(number.checked_sub(1)?);
    if records.globals.slots().get(slot).copied().unwrap_or(0) == 0 {
        return None;
    }
    let m = records.managers.get(slot)?;
    Some(format!("{} {}", m.first_name(), m.last_name()))
}

/// What the dialog writes when a human's slot is not in play: text 1996.
pub const UNKNOWN: u16 = 1996;

/// The staff, each step overwriting the one before, as the original fills
/// it. A human's name is `Err(UNKNOWN)` if his slot is not in play.
pub fn staff(
    records: &Records,
    team: &Team,
) -> (Result<String, u16>, Result<String, u16>, Option<u8>) {
    let (mut trainer, mut manager, mut job) = (Ok(String::new()), Ok(String::new()), None);
    let [first, second] = team.humans();
    let human = |n: u8| human_name(records, n).ok_or(UNKNOWN);
    if first != 0 {
        trainer = human(first);
        job = records.managers.get(usize::from(first - 1)).map(|m| m.get::<u8>(0x25));
    }
    if second != 0 {
        manager = human(second);
    }
    let [both, coach, chief] = team.staff();
    let person_job = |id: u16| records.persons.get(usize::from(id)).map(|p| p.job());
    if both != 0 {
        manager = Ok(records.person_name(both));
        job = person_job(both);
    } else {
        if coach != 0 {
            trainer = Ok(records.person_name(coach));
            job = person_job(coach);
        }
        if chief != 0 {
            manager = Ok(records.person_name(chief));
        }
    }
    (trainer, manager, job)
}

/// The texts of how safe a job is (`FUN_00732813`).
pub fn job_text(job: u8) -> u16 {
    match job {
        0 => 1485,
        1..=2 => 1486,
        _ => 1487,
    }
}

/// The finances' bands (`FUN_007326cc`): bankrupt, bad, questionable,
/// solid, splendid, blessed.
pub fn finance_text(figure: i64) -> u16 {
    const MILLION: i64 = 1_000_000;
    match figure {
        f if f < -5 * MILLION => 1479,
        f if f < -2 * MILLION => 1480,
        f if f < 0 => 1481,
        f if f < 2 * MILLION => 1482,
        f if f < 5 * MILLION => 1483,
        _ => 1484,
    }
}

/// The board's moods (texts 1040 to 1045 by `board`), or the original's
/// own word for anything else.
pub fn board_text(board: u8) -> Result<u16, &'static str> {
    if board <= 5 { Ok(1040 + u16::from(board)) } else { Err("Vorstand???") }
}

/// A league as the save keeps it while the season runs (0x1a88 bytes at
/// `DAT_01a09d48` + index × 0x1a88; its fields from +0x50): the number of
/// teams, the fixture rows, the matchdays played, whether it plays four
/// rounds, the teams by place, the schedule and the results.
pub struct League<'a> {
    data: &'a [u8],
}

const LEAGUE_AT: u32 = 0x1a09d48;
/// The league's fields start this far into the saved block.
const LEAGUE_FIELDS: usize = 0x50 - 0x20;
const SCHEDULE: usize = 0x50a;
const MATCHDAY_PAIRS: usize = 0x18;
/// The results, [home][away] 4 bytes each: 8 bytes before the notes'
/// G+0xe22 — checked on a save, where every played result lies at 0xe1a by
/// its matchday's pairs.
const RESULTS: usize = 0xe1a;
const RESULT_ROW: usize = 0x60;
/// A result not played yet.
const UNPLAYED: u8 = 0xff;

impl<'a> League<'a> {
    pub fn of(records: &'a Records, country: u16, level: u8) -> Option<Self> {
        let first = records.countries.first_league(country.into());
        let index = i32::from(first) + i32::from(level);
        let data = records.block(&format!("{LEAGUE_AT:#x}[{index}]"))?;
        Some(Self { data })
    }

    fn i16_at(&self, at: usize) -> i16 {
        let at = LEAGUE_FIELDS + at;
        self.data.get(at..at + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))
    }

    fn byte(&self, at: usize) -> u8 {
        self.data.get(LEAGUE_FIELDS + at).copied().unwrap_or(0)
    }

    /// Its name, the record's first field (`FUN_006356a2`).
    pub fn name(&self) -> String {
        const NAME: usize = 0x30;
        let bytes = &self.data[..self.data.len().min(NAME)];
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        a3_files::sav::cp1252(&bytes[..end])
    }

    pub fn teams(&self) -> usize {
        usize::try_from(self.i16_at(0)).unwrap_or(0)
    }

    fn rows(&self) -> usize {
        usize::try_from(self.i16_at(2)).unwrap_or(0)
    }

    pub fn played(&self) -> usize {
        usize::try_from(self.i16_at(0x16)).unwrap_or(0)
    }

    pub fn four_rounds(&self) -> bool {
        self.i16_at(0x1a) > 0
    }

    /// The team number at a place in the league.
    pub fn team(&self, place: usize) -> i16 {
        self.i16_at(0x22 + 2 * place)
    }

    /// The pair a place plays in on a row of the schedule: home, away.
    fn pair(&self, row: usize, place: usize) -> Option<(usize, usize)> {
        let at = SCHEDULE + row * MATCHDAY_PAIRS;
        (0..self.teams() / 2).map(|k| (self.byte(at + 2 * k), self.byte(at + 2 * k + 1))).find_map(
            |(home, away)| {
                let (home, away) = (usize::from(home), usize::from(away));
                (home == place || away == place).then_some((home, away))
            },
        )
    }

    /// A place's match of a matchday from 1, and its result if played
    /// (`FUN_006907da`).
    pub fn result(&self, matchday: usize, place: usize) -> Option<Match> {
        let n = self.teams();
        let (mut matchday, mut offset) = (matchday, 0);
        if self.four_rounds() && n >= 1 && matchday > 2 * n - 2 {
            matchday -= 2 * n - 2;
            offset = n;
        }
        if matchday == 0 || matchday > self.rows() {
            return None;
        }
        let (home, away) = self.pair(matchday - 1, place)?;
        let at = RESULTS + home * RESULT_ROW + (away + offset) * 4;
        let goals = (self.byte(at), self.byte(at + 1));
        Some((home, away, (goals.0 != UNPLAYED).then_some(goals)))
    }
}

/// A league match: the home place, the away place, the goals if played.
pub type Match = (usize, usize, Option<(u8, u8)>);

/// A club's stand in its league (`FUN_00692744`, `FUN_0072c2bb`): its
/// place, points and games, and its last results, oldest first, a letter
/// each — at home "s", "x" (0:0), "u", "n", away the same as capitals.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Stand {
    pub place: u8,
    pub points: u32,
    pub games: usize,
    pub form: String,
}

/// Leagues from this level on keep no stand.
const STAND_LEAGUES: u8 = 6;
const FORM_GAMES: usize = 6;

pub fn stand(records: &Records, team: &Team) -> Option<Stand> {
    let (country, level) = (team.country().into(), team.level());
    if level >= STAND_LEAGUES || !records.globals.played(country as i16) {
        return None;
    }
    let league = League::of(records, country, level)?;
    let games = league.played();
    if games == 0 {
        return None;
    }
    let place = usize::from(team.league_place());
    let mut form = String::new();
    for matchday in games.saturating_sub(FORM_GAMES)..games {
        let Some((home, _, Some((a, b)))) = league.result(matchday + 1, place) else { continue };
        let at_home = home == place;
        let (own, theirs) = if at_home { (a, b) } else { (b, a) };
        let letter = match own.cmp(&theirs) {
            std::cmp::Ordering::Greater => 's',
            std::cmp::Ordering::Equal if own == 0 => 'x',
            std::cmp::Ordering::Equal => 'u',
            std::cmp::Ordering::Less => 'n',
        };
        form.push(if at_home { letter } else { letter.to_ascii_uppercase() });
    }
    let [won, drawn, _] = team.record();
    Some(Stand {
        place: team.table_place(),
        points: 3 * u32::from(won) + u32::from(drawn),
        games,
        form,
    })
}

/// Won, drawn, lost.
pub type Record = [u32; 3];

/// A club's league fixtures through the season, as the calendar has them
/// (`FUN_0062af24`): the opponent's team number and whether at home. Only a
/// club of a country kept in full, in a league.
fn fixtures(records: &Records, calendar: &Calendar, team: &Team) -> Vec<(i16, bool)> {
    const FIRST_DAY: usize = 12;
    const LAST_DAY: usize = 363;
    const MOST: usize = 46;
    let (country, level) = (team.country(), team.level());
    if records.countries.kind(country.into()) != 1 || level >= NO_LEAGUE {
        return Vec::new();
    }
    let (Some(league), Some(row)) =
        (League::of(records, country.into(), level), calendar.row(country.into(), level.into()))
    else {
        return Vec::new();
    };
    let place = usize::from(team.league_place());
    (FIRST_DAY..=LAST_DAY)
        .filter_map(|day| calendar.matchday(row, day))
        .filter_map(|matchday| league.pair(matchday - 1, place))
        .map(|(home, away)| {
            let other = if home == place { away } else { home };
            (league.team(other), home == place)
        })
        .take(MOST)
        .collect()
}

/// A club's league record this season at home or away (`FUN_0072bf77`,
/// `FUN_0072c121`): its fixtures taken as matchdays in their order, as the
/// original takes them.
pub fn season_record(records: &Records, calendar: &Calendar, team: &Team, home: bool) -> Record {
    let mut record = [0; 3];
    let Some(league) = League::of(records, team.country().into(), team.level()) else {
        return record;
    };
    let place = usize::from(team.league_place());
    for (i, &(_, at_home)) in fixtures(records, calendar, team).iter().enumerate() {
        if at_home != home {
            continue;
        }
        if let Some((_, _, Some((a, b)))) = league.result(i + 1, place) {
            let (own, theirs) = if home { (a, b) } else { (b, a) };
            record[match own.cmp(&theirs) {
                std::cmp::Ordering::Greater => 0,
                std::cmp::Ordering::Equal => 1,
                std::cmp::Ordering::Less => 2,
            }] += 1;
        }
    }
    record
}

/// The ten seasons gone by, a league each (`DAT_00b21adc`, in the save's
/// block from 0xb1f860): its country, its teams, its results.
const ARCHIVE_BLOCK: &str = "0xb1f860";
const ARCHIVE_AT: usize = 0xb21adc - 0xb1f860;
const ARCHIVE_LEAGUE: usize = 0x5e74;
const ARCHIVE_SEASON: usize = 0x972;
const ARCHIVE_SEASONS: usize = 10;
const ARCHIVE_LEAGUES: usize = 10;
/// Leagues of the main country kept, and where the more countries' start.
const MAIN_LEAGUES: usize = 6;

struct Archived<'a> {
    data: &'a [u8],
}

impl Archived<'_> {
    fn teams(&self) -> usize {
        usize::try_from(i16::from_le_bytes([self.data[2], self.data[3]])).unwrap_or(0)
    }

    fn place_of(&self, number: u8) -> Option<usize> {
        (0..self.teams().min(0x60)).find(|&i| self.data.get(0x12 + i) == Some(&number))
    }

    fn result(&self, home: usize, away: usize) -> (u8, u8) {
        let at = 0x72 + home * RESULT_ROW + away * 4;
        (self.data.get(at).copied().unwrap_or(0), self.data.get(at + 1).copied().unwrap_or(0))
    }
}

fn archived(records: &Records, league: usize, season: usize) -> Option<Archived<'_>> {
    if league >= ARCHIVE_LEAGUES || season >= ARCHIVE_SEASONS {
        return None;
    }
    let at = ARCHIVE_AT + league * ARCHIVE_LEAGUE + season * ARCHIVE_SEASON;
    let data = records.block(ARCHIVE_BLOCK)?.get(at..at + ARCHIVE_SEASON)?;
    Some(Archived { data })
}

/// The main country's league a team number played in, that season
/// (`FUN_0040cf2e`): the first that lists it.
fn archived_league(records: &Records, number: u8, season: usize) -> Option<usize> {
    let main = records.globals.main_country();
    let leagues = usize::try_from(records.countries.leagues(main.max(0) as usize)).unwrap_or(0);
    (0..leagues.min(MAIN_LEAGUES))
        .find(|&l| archived(records, l, season).is_some_and(|a| a.place_of(number).is_some()))
}

/// Which of the more countries a country is, if it is one: its archive's
/// league (`FUN_00677b07`).
fn more_league(records: &Records, country: u16) -> Option<usize> {
    let count = usize::try_from(records.globals.more_count()).unwrap_or(0);
    let i =
        records.globals.more_countries().iter().take(count).position(|&c| c == country as i16)?;
    Some(MAIN_LEAGUES + i)
}

/// A result of seasons ago between two clubs, a's home game, the first or
/// (a league of four rounds) the second (`FUN_0040d7eb`): a's goals first.
pub fn archived_result(
    records: &Records,
    a: &Team,
    b: &Team,
    ago: usize,
    second: bool,
) -> Option<(u8, u8)> {
    let seasons = usize::from(records.globals.seasons());
    if seasons <= ago + 1 {
        return None;
    }
    let season = ARCHIVE_SEASONS - 1 - ago;
    let (na, nb) = (a.number(), b.number());
    let league = if a.country() as i16 == records.globals.main_country() {
        let la = archived_league(records, na, season)?;
        (Some(la) == archived_league(records, nb, season)).then_some(la)?
    } else {
        let more = more_league(records, a.country().into())?;
        let (la, lb) = (archived_league(records, na, season), archived_league(records, nb, season));
        (la == Some(0) && lb == Some(0)).then_some(more)?
    };
    let archive = archived(records, league, season)?;
    let (Some(i), Some(j)) = (archive.place_of(na), archive.place_of(nb)) else {
        return Some((0, 0));
    };
    Some(archive.result(i, if second { j + archive.teams() } else { j }))
}

/// A club's home record against another (`FUN_0040d3a0`): in the seasons
/// archived, and this season's league.
pub fn home_record(records: &Records, calendar: &Calendar, a: &Team, b: &Team) -> Record {
    let mut record = [0; 3];
    let mut count = |own: u8, theirs: u8| {
        record[match own.cmp(&theirs) {
            std::cmp::Ordering::Greater => 0,
            std::cmp::Ordering::Equal => 1,
            std::cmp::Ordering::Less => 2,
        }] += 1;
    };
    let seasons = usize::from(records.globals.seasons());
    for season in (0..ARCHIVE_SEASONS).filter(|&s| s + seasons > ARCHIVE_SEASONS) {
        let league = if a.country() as i16 == records.globals.main_country() {
            let la = archived_league(records, a.number(), season);
            la.filter(|&la| Some(la) == archived_league(records, b.number(), season))
        } else {
            more_league(records, a.country().into())
        };
        let Some(archive) = league.and_then(|l| archived(records, l, season)) else { continue };
        if let (Some(i), Some(j)) = (archive.place_of(a.number()), archive.place_of(b.number())) {
            let (own, theirs) = archive.result(i, j);
            count(own, theirs);
        }
    }
    let Some(league) = League::of(records, a.country().into(), a.level()) else { return record };
    let place = usize::from(a.league_place());
    for (i, &(other, home)) in fixtures(records, calendar, a).iter().enumerate() {
        if !home || other != i16::from(b.number()) {
            continue;
        }
        if let Some((_, _, Some((own, theirs)))) = league.result(i + 1, place) {
            count(own, theirs);
        }
    }
    record
}

/// A club's titles (`FUN_0063499e` …): league, cup, European cup, and the
/// promotions and relegations — both the same sum, as the original shows
/// them.
pub fn titles(records: &Records, team: &Team) -> [i32; 4] {
    const BLOCK: &str = "0xb0b6b8";
    const BASE: u32 = 0xb0b6b8;
    let country = usize::from(team.country());
    let number = usize::from(team.number());
    if !records.globals.played(country as i16)
        || number >= usize::try_from(records.countries.team_count(country)).unwrap_or(0)
    {
        return [0; 4];
    }
    let Some(data) = records.block(BLOCK) else { return [0; 4] };
    let slot = usize::from(records.countries.slot(country));
    let i16_at =
        |at: usize| data.get(at..at + 2).map_or(0, |b| i32::from(i16::from_le_bytes([b[0], b[1]])));
    let table = |at: u32| i16_at((at - BASE) as usize + slot * 0xd8 + number * 2);
    let moves = (0xb1c508 - BASE) as usize + slot * 0x288 + number * 6;
    [table(0xb1b860), table(0xb1bc98), table(0xb1c0d0), (0..3).map(|k| i16_at(moves + 2 * k)).sum()]
}

/// How far a club has got in its country's cup (`FUN_006cdb88`): not in
/// it, out already, won it or lost the final, or in a round against a club.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CupStand {
    NotIn,
    Out,
    Won,
    LostFinal,
    /// The round's text and the opponent's name.
    Round(u16, String),
}

/// The cups' rounds by mode (the table at 0x8a5e10): where each round's
/// pairings start, and how many.
const ROUNDS: [[(usize, usize); 6]; 6] = [
    [(0, 0), (0, 32), (32, 16), (48, 8), (56, 4), (60, 2)],
    [(0, 0), (0, 0), (32, 16), (48, 8), (56, 4), (60, 2)],
    [(0, 20), (20, 32), (52, 16), (68, 8), (76, 4), (80, 2)],
    [(0, 0), (20, 32), (52, 16), (68, 8), (76, 4), (80, 2)],
    [(0, 35), (35, 25), (60, 16), (76, 8), (84, 4), (88, 2)],
    [(0, 0), (35, 25), (60, 16), (76, 8), (84, 4), (88, 2)],
];
/// The rounds' names (the table at 0x8a8a78), one step low from the
/// quarter-finals on, as the original has them; a pairing in no round reads
/// text 1443.
const ROUND_NAMES: [u16; 6] = [1528, 1519, 1520, 1521, 1521, 1522];
const NO_ROUND: u16 = 1443;
const CUPS_AT: u32 = 0x1ba5390;
const PAIRINGS: usize = 0x2c - 0x20;
const PAIRING: usize = 0xe;
const HIGHEST_PAIRING: usize = 62;
const PLAYED: u8 = 0x08;

pub fn cup_stand(records: &Records, team: &Team) -> CupStand {
    let slot = records.countries.slot(team.country().into());
    let Some(cup) = records.block(&format!("{CUPS_AT:#x}[{slot}]")) else { return CupStand::NotIn };
    let mode = usize::from(cup.get(0x24 - 0x20).copied().unwrap_or(0)).min(ROUNDS.len() - 1);
    let round =
        |i: usize| ROUNDS[mode].iter().position(|&(start, n)| (start..start + n).contains(&i));
    let pairing = |i: usize| cup.get(PAIRINGS + i * PAIRING..PAIRINGS + (i + 1) * PAIRING);
    let u16_at = |p: &[u8], at: usize| u16::from_le_bytes([p[at], p[at + 1]]);
    let mut latest = None;
    for i in (0..=HIGHEST_PAIRING).rev() {
        let Some(p) = pairing(i).filter(|p| p[0] != 0) else { continue };
        latest.get_or_insert(i);
        let (home, away) = (u16_at(p, 2), u16_at(p, 6));
        let number = u16::from(team.number());
        if home != number && away != number {
            continue;
        }
        let r = round(i);
        if round(latest.unwrap_or(i)) != r {
            return CupStand::Out;
        }
        if p[0xc] & PLAYED != 0 {
            let won = (p[0xd] == 0) == (home == number);
            return if won { CupStand::Won } else { CupStand::LostFinal };
        }
        let other = if home == number { away } else { home };
        let name = records.team(p[0].into(), other).map(|t| t.name()).unwrap_or_default();
        let text = r.map_or(NO_ROUND, |r| ROUND_NAMES[r]);
        return CupStand::Round(text, name);
    }
    CupStand::NotIn
}

/// A club in the European cups: its country and its number there (the
/// four-byte id of EUROPAPOKAL-STAND.md, the byte between unused).
pub type Id = (u8, i16);

/// How far a club has got in the European cups (`FUN_00424e74`): not in
/// them, in a round of one (0 the Champions League, 1 the UEFA cup) with
/// the opponent the search found, or out in a round.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EuropeStand {
    NotIn,
    In(usize, u16, Option<Id>),
    Out(usize, u16),
}

/// The rounds' texts by code (the tables at 0x8772f4 and 0x87731c): the
/// Champions League's two low from the quarter-finals on, as the original
/// has them.
const CHAMPIONS_ROUNDS: [u16; 9] = [0, 1864, 1871, 1871, 1871, 1871, 1871, 1871, 1868];
const UEFA_ROUNDS: [u16; 9] = [0, 1864, 1865, 1866, 1871, 1867, 1868, 1869, 1870];
/// The entrants (`CDatenEuropapokal`): the limit of the direct entrants to
/// the UEFA cup, the Champions League's 16 seeds, the UEFA cup's direct
/// entrants and its 82 qualifier entrants.
const ENTRANTS_AT: &str = "0x8e7c88";
const DIRECT_LIMIT: usize = 0x08;
const SEEDS: (usize, usize) = (0x12, 16);
const DIRECT: usize = 0x112;
const QUALIFIERS: (usize, usize) = (0x2b2, 82);
/// The Champions League: its pairings (0..15 the qualifier, 160..163 the
/// quarter-finals, 164 and 165 the semi-finals, 166 the final) and its two
/// group stages, eight and four groups of four.
const CHAMPIONS_AT: &str = "0x8e91e0";
const CHAMPIONS_QUALIFIER: usize = 16;
const QUARTER_FINALS: usize = 160;
const SEMI_FINALS: usize = 164;
const FINAL: usize = 166;
const GROUPS: [(usize, usize); 2] = [(0x922, 8), (0xaa2, 4)];
const GROUP: usize = 0x30;
const GROUP_TEAM: usize = 0xc;
/// The UEFA cup: its pairings after the round starts, which are these.
const UEFA_AT: &str = "0x8e8940";
const UEFA_PAIRINGS: usize = 0x20;
const UEFA_STARTS: [usize; 8] = [0, 50, 98, 122, 138, 146, 150, 152];

pub fn europe_stand(records: &Records, team: &Team) -> EuropeStand {
    let club = (team.country(), i16::from(team.number()));
    let (entrants, champions, uefa) =
        (records.block(ENTRANTS_AT), records.block(CHAMPIONS_AT), records.block(UEFA_AT));
    let (Some(entrants), Some(champions), Some(uefa)) = (entrants, champions, uefa) else {
        return EuropeStand::NotIn;
    };
    let id = |data: &[u8], at: usize| -> Id {
        data.get(at..at + 4).map_or((0, 0), |b| (b[0], i16::from_le_bytes([b[2], b[3]])))
    };
    let listed = |at: usize, n: usize| (0..n).any(|k| id(entrants, at + 4 * k) == club);
    // The other side of the latest pairing of these the club is in.
    let against = |data: &[u8], at: usize, pairings: &mut dyn Iterator<Item = usize>| {
        for i in pairings {
            let (a, b) = (id(data, at + PAIRING * i), id(data, at + PAIRING * i + 4));
            if a == club {
                return Some((i, b));
            }
            if b == club {
                return Some((i, a));
            }
        }
        None
    };
    let stage = records.globals.europe_stage();

    // The Champions League (`FUN_0042451e`).
    let mut info: Option<Id> = None;
    let champions_code: i16 = 'code: {
        if !listed(SEEDS.0, SEEDS.1) {
            match against(champions, 0, &mut (0..CHAMPIONS_QUALIFIER)) {
                Some((_, other)) => info = Some(other),
                None => break 'code 0,
            }
            if stage <= 0 {
                break 'code 1;
            }
        } else if stage <= 0 {
            break 'code 2;
        }
        let in_group = |(at, groups): (usize, usize)| {
            (0..groups * 4)
                .any(|k| id(champions, at + GROUP * (k / 4) + GROUP_TEAM * (k % 4)) == club)
        };
        if !in_group(GROUPS[0]) {
            break 'code -1;
        }
        if stage == 1 {
            break 'code 2;
        }
        if !in_group(GROUPS[1]) {
            break 'code 0;
        }
        if stage == 2 {
            break 'code 3;
        }
        let Some((i, other)) = against(champions, 0, &mut (QUARTER_FINALS..=FINAL).rev()) else {
            break 'code -6;
        };
        info = Some(other);
        match i {
            FINAL => 8,
            i if i < SEMI_FINALS => {
                if stage < 6 {
                    6
                } else {
                    -6
                }
            }
            _ => {
                if stage < 7 {
                    7
                } else {
                    -7
                }
            }
        }
    };

    let mut stand = match champions_code {
        0 | -1 => EuropeStand::NotIn,
        c if c > 0 => EuropeStand::In(0, CHAMPIONS_ROUNDS[c as usize], info),
        c => EuropeStand::Out(0, CHAMPIONS_ROUNDS[c.unsigned_abs() as usize]),
    };

    // The UEFA cup (`FUN_0042462b`), by the Champions League's stage first;
    // where it finds the club it has the say.
    let limit = entrants
        .get(DIRECT_LIMIT..DIRECT_LIMIT + 2)
        .map_or(0, |b| i16::from_le_bytes([b[0], b[1]]));
    let limit = usize::try_from(limit).unwrap_or(0);
    let mut info: Option<Id> = None;
    let uefa_code: i16 = 'code: {
        if stage <= 0 && listed(DIRECT, limit) {
            break 'code 1;
        }
        if stage < 0 && listed(QUALIFIERS.0, QUALIFIERS.1) {
            break 'code 0;
        }
        let last = UEFA_STARTS[UEFA_STARTS.len() - 1];
        let Some((i, other)) = against(uefa, UEFA_PAIRINGS, &mut (0..=last).rev()) else {
            break 'code 0;
        };
        info = Some(other);
        if i == last {
            break 'code 8;
        }
        let r = (0..UEFA_STARTS.len() - 1).rev().find(|&k| UEFA_STARTS[k] <= i).unwrap_or(0) as i16;
        if r < records.globals.uefa_round() { -(r + 1) } else { r + 1 }
    };
    if uefa_code > 0 {
        stand = EuropeStand::In(1, UEFA_ROUNDS[uefa_code as usize], info);
    } else if uefa_code < 0 {
        stand = EuropeStand::Out(1, UEFA_ROUNDS[uefa_code.unsigned_abs() as usize]);
    }
    stand
}
