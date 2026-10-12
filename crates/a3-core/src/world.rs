//! The football world a new game starts from, as the data files in
//! `Data.a3` describe it: the nations, the eleven countries that can be played
//! and the bonus country, with their leagues and clubs.
//!
//! A nation's code is its position in `Laender.sav`. Each playable country
//! has a `Land*.sav` with its clubs and their players, in the order the
//! league files count them (from 1), and up to six league files, which the
//! game names in a table of its own; `Bonus1.sav` holds the bonus country and
//! names its nation. A league file lists its clubs and its whole fixture list.

use std::cmp::Ordering;
use std::io;
use std::path::Path;

use chrono::NaiveDate;

use a3_files::cpr::invalid;
use a3_files::sav::Section;

/// Country slots in the game's order: the five main countries first, then
/// the other six, then the bonus country.
pub const BONUS_SLOT: usize = 11;
pub const SLOTS: usize = 12;
/// The main countries a game can be played in, by slot.
pub const MAIN_SLOTS: [usize; 5] = [0, 1, 2, 3, 4];
/// Every main country's own leagues are played, up to this many; of the
/// other chosen countries only the first.
const MAIN_LEAGUES: usize = 6;
/// From the year 2000 Germany plays only its first four leagues. The game
/// starts in 1998, so a new game has all six.
const START_YEAR: u16 = 1998;
const GERMANY_SLOT: usize = 0;
/// Austria and Scotland play every pairing four times: their league files
/// hold two rounds, and the game plays them twice.
const FOUR_ROUNDS: [u16; 2] = [47, 57];
/// The values of a `VEREIN`, by line: name, money in units of 10,000, how
/// many fans it has (0 the most), how kindly the board looks on its manager
/// and how grand a club it is, from 0 to 6.
const CLUB_NAME: usize = 1;
const CLUB_MONEY: usize = 13;
const CLUB_BOARD: usize = 19;
const CLUB_FANS: usize = 15;
const CLUB_ARTICLE: usize = 21;
const CLUB_LAST_SEASON: usize = 22;
const CLUB_AFFILIATE: usize = 27;
const CLUB_CLASS: usize = 29;
/// A `STADION` holds four stands from this value on, each standing places,
/// seats, boxes and how far it is built; a stand still being built counts
/// for less, in percent by stage. A stadium of fewer places counts as the
/// smallest.
const STANDS: usize = 13;
const STAND_VALUES: usize = 4;
const STAND_READY: [i64; 7] = [100, 100, 100, 100, 80, 50, 20];
const SMALLEST_STADIUM: i64 = 6000;
const TOO_SMALL: i64 = 3000;
const MONEY_UNIT: i64 = 10_000;
/// The values of a `SPIELER`, by line.
const PLAYER_LAST_NAME: usize = 0;
const PLAYER_FIRST_NAME: usize = 1;
const PLAYER_STRENGTH: usize = 6;
const PLAYER_POSITIONS: usize = 8;
/// Bits of what a player is particularly good at, and bad at.
const PLAYER_ABILITIES: usize = 11;
/// Bits of a player's character, two values.
const PLAYER_TRAITS: usize = 13;
const PLAYER_BIRTHDAY: usize = 21;
const PLAYER_SHIRT: usize = 28;
/// The files number positions differently from the game: their 6 and 8,
/// defensive and right midfield, are the other way round (a table in the
/// game).
const POSITION_CODES: [u8; 11] = [0, 1, 2, 3, 4, 5, 8, 7, 6, 9, 10];
/// A player shows at most this many abilities, the good ones first.
const MOST_ABILITIES: u32 = 3;
/// Positions run from 1, the goalkeeper, to 10, the striker; players with
/// none of those are left out, as the game does.
pub const GOALKEEPER: u8 = 1;
pub const STRIKER: u8 = 10;
/// The bonus country's `BONUSLAND`: its number of clubs and flags, which pick
/// its row of the game's calendar.
const BONUS_CLUBS: usize = 1;
const BONUS_FLAGS: usize = 3;

const LAND_FILES: [&str; SLOTS] = [
    "LandDeut.sav",
    "LandEngl.sav",
    "LandFran.sav",
    "LandItal.sav",
    "LandSpan.sav",
    "LandPort.sav",
    "LandHoll.sav",
    "LandOest.sav",
    "LandScho.sav",
    "LandSchw.sav",
    "LandTuer.sav",
    "Bonus1.sav",
];
const LEAGUE_FILES: [&[&str]; SLOTS] = [
    &[
        "Liga1Deu.sav",
        "Liga2Deu.sav",
        "LigaRLNO.sav",
        "LigaRLOs.sav",
        "LigaRLSu.sav",
        "LigaRLSW.sav",
    ],
    &["Liga1Eng.sav", "Liga2Eng.sav", "Liga3Eng.sav", "Liga4Eng.sav"],
    &["Liga1Fra.sav", "Liga2Fra.sav"],
    &["Liga1Ita.sav", "Liga2Ita.sav"],
    &["Liga1Spa.sav", "Liga2Spa.sav"],
    &["Liga1Por.sav"],
    &["Liga1hol.sav"],
    &["Liga1Oes.sav"],
    &["Liga1Sch.sav"],
    &["Liga1Swz.sav"],
    &["Liga1Tue.sav"],
    &["Liga1Bon.sav"],
];
/// Nations that field a national team, by code (a table in the game).
const NATIONAL_TEAMS: [u16; 46] = [
    2, 6, 9, 13, 14, 16, 17, 18, 19, 20, 21, 23, 24, 25, 26, 27, 28, 34, 35, 38, 39, 42, 45, 46,
    47, 50, 51, 53, 54, 55, 57, 58, 59, 61, 62, 63, 65, 67, 68, 69, 73, 75, 76, 77, 99, 116,
];

pub struct World {
    /// Nation names by code.
    pub nations: Vec<String>,
    /// By slot; the bonus slot is empty when no bonus country is installed.
    pub countries: Vec<Option<Country>>,
    /// Nations whose clubs `Internat.sav` brings in for the European cups.
    internationals: Vec<u16>,
}

pub struct Country {
    pub nation: u16,
    pub clubs: Vec<Club>,
    /// Every league the country has files for.
    pub leagues: Vec<League>,
    /// For the bonus country, its club count and flags from `BONUSLAND`.
    pub bonus: Option<(i64, i64)>,
}

pub struct Club {
    pub name: String,
    /// In the game's money unit.
    pub money: i64,
    /// How kindly the board looks on its manager, from 0 (a powder keg) to 5
    /// (loyal).
    pub board: i64,
    /// How many fans it has, from 0 (hordes) to 6.
    pub fans: i64,
    /// How grand a club it is, from 0 to 6; from 5 its manager has the grand
    /// office.
    pub class: i64,
    /// How last season ended: in the low three bits 1 champion, 3
    /// relegated, 4 promoted.
    pub last_season: i64,
    /// Non-zero for a club tied to another, which cannot go up.
    pub affiliate: i64,
    /// Places in the stadium, standing and seated.
    pub capacity: i64,
    /// How its name takes an article in German: 0 masculine ("der"), 1
    /// plural, 2 none, 3 feminine.
    pub article: i64,
    pub players: Vec<Player>,
}

pub struct Player {
    pub last_name: String,
    pub first_name: String,
    /// From 1 to 12.
    pub strength: u8,
    /// Up to three positions in the game's numbering, the first the main
    /// one; 0 for none.
    pub positions: [u8; 3],
    /// Bits of what the player is particularly good at, and bad at: at most
    /// three in all.
    pub abilities: [u16; 2],
    /// Bits of character and skill.
    pub traits: [i64; 2],
    pub birthday: Option<NaiveDate>,
    pub shirt: u8,
}

pub struct League {
    pub name: String,
    /// Indices into the country's clubs.
    pub clubs: Vec<usize>,
    /// Every matchday's pairings, home first, as positions in `clubs`.
    pub fixtures: Vec<Vec<(usize, usize)>>,
}

impl World {
    pub fn load(dir: &Path) -> io::Result<Self> {
        let nations = Section::read(&find(dir, "Laender.sav")?)?;
        let nations: Vec<String> = nations
            .child("NATION")
            .ok_or_else(|| invalid("Laender.sav has no nations"))?
            .all("NATION")
            .map(|n| n.text(0).unwrap_or_default().to_owned())
            .collect();
        let countries =
            (0..SLOTS).map(|slot| Country::load(dir, slot)).collect::<io::Result<Vec<_>>>()?;
        let internationals = Section::read(&find(dir, "Internat.sav")?)?
            .all("LAND")
            .filter_map(|land| land.child("VEREIN")?.number(0))
            .map(|code| code as u16)
            .collect();
        Ok(Self { nations, countries, internationals })
    }

    pub fn nation(&self, code: u16) -> &str {
        self.nations.get(usize::from(code)).map_or("", String::as_str)
    }

    pub fn country(&self, slot: usize) -> Option<&Country> {
        self.countries.get(slot)?.as_ref()
    }

    /// The slot of a nation's country, if it can be played.
    pub fn slot_of(&self, nation: u16) -> Option<usize> {
        self.countries.iter().position(|c| c.as_ref().is_some_and(|c| c.nation == nation))
    }

    /// The leagues of `slot` a game in `main` plays.
    pub fn leagues_in_play(&self, slot: usize, main: usize) -> &[League] {
        let Some(country) = self.country(slot) else { return &[] };
        let count = match slot {
            _ if slot != main => 1,
            GERMANY_SLOT if START_YEAR >= 2000 => 4,
            _ => MAIN_LEAGUES,
        };
        &country.leagues[..count.min(country.leagues.len())]
    }

    /// Whether the national team of `nation` can be coached in a game with
    /// these countries: the game loads every playable country and the
    /// nations of the international clubs, and only knows those.
    pub fn has_national_team(&self, nation: u16) -> bool {
        NATIONAL_TEAMS.contains(&nation)
            && (self.slot_of(nation).is_some() || self.internationals.contains(&nation))
    }

    /// Nations with a national team, sorted by name the way the game does: by
    /// the bytes of the names.
    pub fn national_teams(&self) -> Vec<u16> {
        let mut teams: Vec<u16> =
            (1..self.nations.len() as u16).filter(|&n| self.has_national_team(n)).collect();
        teams.sort_by(|&a, &b| by_bytes(self.nation(a), self.nation(b)));
        teams
    }
}

impl Country {
    fn load(dir: &Path, slot: usize) -> io::Result<Option<Self>> {
        let land = Section::read(&find(dir, LAND_FILES[slot])?)?;
        let mut bonus = None;
        let nation = if slot == BONUS_SLOT {
            // `BONUS` says whether there is a bonus country, `BONUSLAND` which.
            let enabled = land.child("BONUS").and_then(|s| s.number(0)).unwrap_or(0) != 0;
            let header = land.child("BONUSLAND");
            let nation = header.and_then(|s| s.number(5)).unwrap_or(0);
            if !enabled || nation <= 0 {
                return Ok(None);
            }
            let value = |i: usize| header.and_then(|s| s.number(i)).unwrap_or(0);
            bonus = Some((value(BONUS_CLUBS), value(BONUS_FLAGS)));
            nation as u16
        } else {
            let first = land.child("LAND").and_then(|l| l.child("VEREIN"));
            first.and_then(|club| club.number(0)).unwrap_or(0) as u16
        };
        // The clubs sit inside `LAND`, or `BONUSLAND` in the bonus file.
        let clubs = land.child("LAND").or_else(|| land.child("BONUSLAND")).unwrap_or(&land);
        // Each club's stadium follows it.
        let stadiums: Vec<&Section> = clubs.all("STADION").collect();
        let clubs = clubs
            .all("VEREIN")
            .enumerate()
            .map(|(i, verein)| Club::load(verein, stadiums.get(i).copied()))
            .collect();
        let rounds = if FOUR_ROUNDS.contains(&nation) { 2 } else { 1 };
        let leagues = LEAGUE_FILES[slot]
            .iter()
            .map(|file| League::load(&find(dir, file)?, rounds))
            .collect::<io::Result<_>>()?;
        Ok(Some(Self { nation, clubs, leagues, bonus }))
    }
}

impl Club {
    fn load(verein: &Section, stadium: Option<&Section>) -> Self {
        let number = |i: usize| verein.number(i).unwrap_or(0);
        let players = verein
            .all("SPIELER")
            .map(Player::load)
            .filter(|p| (GOALKEEPER..=STRIKER).contains(&p.positions[0]))
            .collect();
        Self {
            name: verein.text(CLUB_NAME).unwrap_or_default().to_owned(),
            money: number(CLUB_MONEY) * MONEY_UNIT,
            board: number(CLUB_BOARD),
            fans: number(CLUB_FANS),
            class: number(CLUB_CLASS),
            last_season: number(CLUB_LAST_SEASON),
            affiliate: number(CLUB_AFFILIATE),
            capacity: stadium.map_or(SMALLEST_STADIUM, capacity),
            article: number(CLUB_ARTICLE),
            players,
        }
    }
}

impl Player {
    fn load(spieler: &Section) -> Self {
        let number = |i: usize| spieler.number(i).unwrap_or(0);
        let text = |i: usize| spieler.text(i).unwrap_or_default().trim().to_owned();
        Self {
            last_name: text(PLAYER_LAST_NAME),
            first_name: text(PLAYER_FIRST_NAME),
            strength: number(PLAYER_STRENGTH).clamp(1, 12) as u8,
            positions: positions([0, 1, 2].map(|i| number(PLAYER_POSITIONS + i))),
            abilities: abilities(number(PLAYER_ABILITIES), number(PLAYER_ABILITIES + 1)),
            traits: [number(PLAYER_TRAITS), number(PLAYER_TRAITS + 1)],
            birthday: NaiveDate::parse_from_str(&text(PLAYER_BIRTHDAY), "%d.%m.%Y").ok(),
            shirt: number(PLAYER_SHIRT).clamp(0, 255) as u8,
        }
    }

    /// How old the player is on a day.
    pub fn age(&self, on: NaiveDate) -> u32 {
        let Some(born) = self.birthday else { return 0 };
        on.years_since(born).unwrap_or(0)
    }
}

/// A player's positions as the game reads them: the main one as it stands
/// (the files leave out players without one), the others only if they are
/// field positions and not one already held.
fn positions([main, second, third]: [i64; 3]) -> [u8; 3] {
    let code = |p: i64| usize::try_from(p).ok().and_then(|p| POSITION_CODES.get(p)).copied();
    let main = code(main).unwrap_or(main.clamp(0, 255) as u8);
    let side = |p: i64| if (2..=10).contains(&p) { code(p).unwrap_or(0) } else { 0 };
    let (mut second, mut third) = (side(second), side(third));
    if third == second || third == main {
        third = 0;
    }
    if second == main {
        second = 0;
    }
    [main, second, third]
}

/// The abilities a player shows: the files keep them shifted by a bit, the
/// bad ones only where the player is not good; three at most, bit by bit.
fn abilities(good: i64, bad: i64) -> [u16; 2] {
    let good = (good >> 1) as u16;
    let bad = (bad >> 1) as u16 & !good;
    let mut kept = [0, 0];
    let mut count = 0;
    for bit in 0..16 {
        for (k, bits) in [good, bad].into_iter().enumerate() {
            if bits & (1 << bit) != 0 && count < MOST_ABILITIES {
                kept[k] |= 1 << bit;
                count += 1;
            }
        }
    }
    kept
}

impl League {
    /// `LIGA`: name, short name, number of clubs, then the clubs by their
    /// position in the country's file, counted from 1, then the fixture list:
    /// for each of the two rounds' matchdays, every club's position in the
    /// league, counted from 1, in pairs of home and away. The list is played
    /// `rounds` times.
    fn load(path: &Path, rounds: usize) -> io::Result<Self> {
        let file = Section::read(path)?;
        let liga = file.child("LIGA").ok_or_else(|| invalid("no LIGA section"))?;
        let count = liga.number(2).unwrap_or(0).max(0) as usize;
        let clubs: Vec<usize> = (0..count)
            .filter_map(|i| liga.number(3 + i))
            .filter(|&club| club >= 1)
            .map(|club| club as usize - 1)
            .collect();
        let slot = |i: usize| {
            let value = liga.number(3 + count + i)?;
            (1..=clubs.len() as i64).contains(&value).then(|| value as usize - 1)
        };
        let matchdays = 2 * count.saturating_sub(1);
        let fixtures: Vec<Vec<(usize, usize)>> = (0..matchdays)
            .map(|day| {
                (0..count / 2)
                    .filter_map(|pair| {
                        let at = day * count + 2 * pair;
                        Some((slot(at)?, slot(at + 1)?))
                    })
                    .collect()
            })
            .collect();
        let fixtures = fixtures.iter().cycle().take(fixtures.len() * rounds).cloned().collect();
        Ok(Self { name: liga.text(0).unwrap_or_default().to_owned(), clubs, fixtures })
    }
}

/// The places of a stadium.
fn capacity(stadium: &Section) -> i64 {
    let total: i64 = (0..4)
        .map(|stand| {
            let value = |i: usize| stadium.number(STANDS + stand * STAND_VALUES + i).unwrap_or(0);
            let stage = (value(3) - 1).clamp(0, STAND_READY.len() as i64 - 1) as usize;
            (value(0) + value(1) + value(2)) * STAND_READY[stage] / 100
        })
        .sum();
    if total < TOO_SMALL { SMALLEST_STADIUM } else { total }
}

/// A file in `dir` by name, ignoring case as Windows does.
fn find(dir: &Path, name: &str) -> io::Result<std::path::PathBuf> {
    let exact = dir.join(name);
    if exact.is_file() {
        return Ok(exact);
    }
    std::fs::read_dir(dir)?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.file_name().is_some_and(|f| f.eq_ignore_ascii_case(name)))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{name} not found")))
}

/// `strcoll` in the C locale: byte order of the Windows-1252 names.
fn by_bytes(a: &str, b: &str) -> Ordering {
    use a3_files::bitmap_font::to_cp1252;
    let bytes = |s: &str| s.chars().map(|c| to_cp1252(c).unwrap_or(b'?')).collect::<Vec<u8>>();
    bytes(a).cmp(&bytes(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_installed_world_is_read_completely() {
        let dir = std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR");
        let world = World::load(&Path::new(&dir).join("Data.a3")).unwrap();
        assert_eq!(world.nation(14), "Deutschland");
        let main_nations: Vec<u16> =
            MAIN_SLOTS.iter().map(|&s| world.country(s).unwrap().nation).collect();
        assert_eq!(main_nations, [14, 16, 20, 28, 63]);
        for slot in 0..SLOTS {
            let Some(country) = world.country(slot) else { continue };
            for league in world.leagues_in_play(slot, slot) {
                assert!(!league.clubs.is_empty(), "{} has no clubs", league.name);
                assert!(league.clubs.iter().all(|&c| c < country.clubs.len()), "{}", league.name);
            }
        }
    }

    #[test]
    fn positions_read_as_the_game_reads_them() {
        // Defensive and right midfield swap; a side position may be neither
        // the goal nor one already held.
        assert_eq!(positions([6, 8, 0]), [8, 6, 0]);
        assert_eq!(positions([10, 1, 9]), [10, 0, 9]);
        assert_eq!(positions([3, 3, 4]), [3, 0, 4]);
        assert_eq!(positions([5, 7, 7]), [5, 7, 0]);
    }

    #[test]
    fn a_player_shows_three_abilities_at_most() {
        // Shifted by a bit in the files; bad only where not good.
        assert_eq!(abilities(0b110, 0b1010), [0b11, 0b100]);
        // Bit by bit, good before bad, until three are shown.
        assert_eq!(abilities(0b1111 << 1, 0b1_0000 << 1), [0b111, 0]);
        assert_eq!(abilities(0b101 << 1, 0b10 << 1), [0b101, 0b10]);
    }

    #[test]
    fn umlauts_sort_after_z_as_in_the_game() {
        assert_eq!(by_bytes("Österreich", "Zypern"), Ordering::Greater);
        assert_eq!(by_bytes("Albanien", "Belgien"), Ordering::Less);
    }
}
