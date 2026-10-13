//! A saved game read into the season the screens show: the managers, and
//! for every human club its squad, its line-up and how its players are,
//! from the original's records ([`crate::records`]). What the records do
//! not give yet starts as a new game starts it.

use std::cmp::Ordering;

use chrono::NaiveDate;
use std::collections::BTreeMap;

use crate::cohesion;
use crate::data::Data;
use crate::lineup::{BENCH, FIELD, Lineup};
use crate::random::Random;
use crate::records::{self, Globals, Records, Team};
use crate::season::{ClubState, Fitness, Manager, Season, Status};
use crate::world::Player;

/// A manager under contract (`records::Manager::status`).
const HIRED: u8 = 3;
/// The difficulty a new game starts at, until the records name it.
const DIFFICULTY: usize = 2;

pub fn season(records: &Records, data: &Data, random: Random) -> Season {
    let world = &data.world;
    let managers: Vec<Manager> = records
        .humans()
        .map(|m| {
            let club = (m.status() == HIRED)
                .then(|| Some((world.slot_of(m.country())?, usize::from(m.team()))))
                .flatten();
            Manager {
                first_name: m.first_name(),
                last_name: m.last_name(),
                club,
                national_team: None,
                level: m.level().into(),
                difficulty: DIFFICULTY,
                negotiating: 0,
                confidence: m.shown_confidence(),
                offers: Vec::new(),
                menu: 0,
                formations: m.formations(),
            }
        })
        .collect();
    let clubs =
        managers.iter().filter_map(|m| Some((m.club?, club(records, data, m.club?)?))).collect();
    Season {
        year: records.globals.start_year().into(),
        day: records.globals.day().into(),
        // The country of the first club, until the records name it.
        main: managers.iter().find_map(|m| m.club).map_or(0, |(slot, _)| slot),
        managers,
        current: 0,
        expected: BTreeMap::new(),
        clubs,
        random,
    }
}

/// A club of the loaded game, by country slot and index, as its team record
/// has it: the squad in the order the team lists it, the eleven on the spots
/// of its formation, the bench.
pub fn club(records: &Records, data: &Data, (slot, index): (usize, usize)) -> Option<ClubState> {
    let nation = data.world.country(slot)?.nation;
    let team = records.team(nation, u16::try_from(index).ok()?)?;
    Some(club_state(records, team, data))
}

fn club_state(records: &Records, team: &Team, data: &Data) -> ClubState {
    let squad: Vec<(u16, &records::Player)> =
        team.players().into_iter().filter_map(|id| Some((id, records.player(id)?))).collect();
    let players = squad.iter().map(|(_, record)| player(records, record)).collect();
    let fitness = squad.iter().map(|(_, record)| fitness(record)).collect();
    let status = squad.iter().map(|(_, record)| status(records, record)).collect();
    let place = |id: u16| squad.iter().position(|&(other, _)| other == id);
    // Where the team keeps its eleven — the formation's spots, or where
    // the manager moved them; a team that keeps none stands in its
    // formation.
    let formations = &data.rules.pitch.formations;
    let saved = team.spots().map(|[x, y]| (x, y));
    let spots = if saved.iter().any(|&spot| spot != (0, 0)) {
        saved
    } else {
        formations
            .get(usize::from(team.formation()))
            .or(formations.first())
            .map_or([(0, 0); FIELD], |f| f.1)
    };
    let field: [Option<usize>; FIELD] = team.lineup().map(place);
    let bench: [Option<usize>; BENCH] = team.bench().map(place);
    let lineup = Lineup { spots, field, bench };
    let state = ClubState::new(players, lineup, fitness, team.ticket_target().into());
    // A human club's training and its manager's time there.
    let human = usize::from(team.human()).checked_sub(1);
    let training = human.and_then(|slot| records.extras.get(slot)).map_or(0, |e| e.training());
    let signed = human.and_then(|slot| records.managers.get(slot)).and_then(|m| {
        NaiveDate::from_ymd_opt(
            m.signed_year().into(),
            m.signed_month().into(),
            m.signed_day().into(),
        )
    });
    let cohesion_bonus = cohesion::bonus(training, signed, records.globals.start_year().into());
    let strengths = Some(team.strengths().map(i32::from));
    let formation = usize::from(team.formation());
    let tickets = team.tickets().into();
    let co_trainer = team.co_trainer() != 0;
    ClubState { cohesion_bonus, status, strengths, formation, tickets, co_trainer, ..state }
}

/// A player record as the world has the players of the data files.
fn player(records: &Records, record: &records::Player) -> Player {
    Player {
        last_name: records.name(record.last_name()).to_owned(),
        first_name: records.name(record.first_name()).to_owned(),
        strength: record.strength(),
        positions: record.positions(),
        abilities: [record.good(), record.bad()],
        // The record keeps the data file's values halved.
        traits: record.traits().map(|t| i64::from(t) << 1),
        birthday: record.birthday(),
        shirt: record.shirt(),
    }
}

fn fitness(record: &records::Player) -> Fitness {
    Fitness {
        condition: record.condition().into(),
        freshness: record.freshness().map(i32::from),
        morale: record.morale().into(),
        mood: record.mood().into(),
        knock: record.knocks() & KNOCKS != 0,
        worn: record.shown() & WORN != 0,
    }
}

/// Bits of a player's record for knocks he plays with.
const KNOCKS: u16 = 0xe000;
/// Bit 7: the length of a long absence is not known yet.
const UNKNOWN: u8 = 0x80;
/// The automatic line-ups' bits: always pick him, never.
const ALWAYS: u8 = 0x20;
const NEVER: u8 = 0x10;
/// His condition counts five less in them.
const WORN: u32 = 0x800;

fn status(records: &Records, record: &records::Player) -> Status {
    let globals = &records.globals;
    Status {
        last_form: Some(record.last_form().into()),
        long: days_to(globals, record.long_to()),
        long_unknown: record.long_unknown() & UNKNOWN != 0,
        injured: days_to(globals, record.injured_to()),
        suspended: record.suspension(),
        away: days_to(globals, record.away_to()),
        away_kind: record.away_kind(),
        contract: Some(i32::from(record.contract()) - i32::from(globals.start_year())),
        knocked: record.knocked() != 0,
        dumbbell: record.dumbbell() == 1,
        always: record.picks() & ALWAYS != 0,
        never: record.picks() & NEVER != 0,
        matches: record.league_matches(),
        matches_less: record.league_less(),
        id: record.id(),
        friends: record.friends(),
        enemies: record.enemies(),
        joined: record.joined().and_then(|(day, month, year)| {
            NaiveDate::from_ymd_opt(year.into(), month.into(), day.into())
        }),
    }
}

/// Days from today to a year and a day of the year, as `FUN_00636638`
/// reckons them: none for a day gone by. (The original takes the days of
/// the years in between from a table, `DAT_00b1f85e`; here they have 365.)
fn days_to(globals: &Globals, [year, day]: [u16; 2]) -> i32 {
    let [_, _, this_year] = globals.date();
    let today = i32::from(globals.day_of_year());
    let days = match year.cmp(&this_year) {
        Ordering::Less => 0,
        Ordering::Equal => i32::from(day) - today,
        Ordering::Greater => {
            let between = i32::from(year - this_year - 1) * 365;
            between + i32::from(day) + i32::from(globals.year_days()) - today
        }
    };
    days.max(0)
}
