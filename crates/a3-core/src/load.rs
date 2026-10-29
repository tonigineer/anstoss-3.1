//! A saved game read into the season the screens show: the managers, and
//! for every human club its squad, its line-up and how its players are,
//! from the original's records ([`crate::records`]). What the records do
//! not give yet starts as a new game starts it.

use std::cmp::Ordering;

use chrono::NaiveDate;
use std::collections::BTreeMap;

use crate::data::Data;
use crate::lineup::{BENCH, FIELD, Lineup};
use crate::marking::Marking;
use crate::medical::{self, MedicalTables};
use crate::random::Random;
use crate::records::{self, Globals, Records, Team};
use crate::season::{ClubState, Fitness, Manager, Season, Status};
use crate::tactics::{FOUL, PICKS, Pick, Tactics};
use crate::training::{CoTraining, Template, Training};
use crate::world::Player;
use crate::{autolineup, cohesion, duties, individual};

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
                menu: m.menu_view(),
                list_view: m.list_view(),
                formations: m.formations(),
                templates: m
                    .templates()
                    .into_iter()
                    .map(|(name, week)| Template { name, week })
                    .collect(),
                weeks: (0..records::WEEKS).map(|week| m.week(week)).collect(),
                training_pictures: m.training_pictures(),
                individual_rule: m.individual_rule(),
                camp_rules: m.camp_rules(),
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
        planner_message: None,
        // A game saved in the middle of a matchday goes on with the turns
        // before its matches.
        before_match: records.globals.resume() == MATCHES_RESUMED,
        random,
        donations: Vec::new(),
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
    let medical = &data.rules.medical;
    let status = squad.iter().map(|(_, record)| status(records, medical, record)).collect();
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
    let extra = human.and_then(|slot| records.extras.get(slot));
    let line_up_flags = extra.map_or(0, |e| e.line_up_flags());
    let ids: Vec<u16> = squad.iter().map(|&(id, _)| id).collect();
    let duties = extra.map_or_else(Default::default, |e| {
        e.duty_lists().map(|list| duties::from_ids(&ids, &list))
    });
    let mut tactics = Tactics::of(team, extra);
    if let Some(extra) = extra {
        tactics.picks = picks(records, data, team, &ids, extra.picks());
    }
    let pitch = &data.rules.pitch;
    let country = usize::from(team.country());
    let counted = pitch.european.get(country).copied().unwrap_or(false);
    let foreigner_rule = counted && !(country == GERMANY && team.level() > 1);
    let mut club = ClubState {
        cohesion_bonus,
        status,
        strengths,
        formation,
        tickets,
        co_trainer,
        line_up_flags,
        foreigner_rule,
        duties,
        tactics,
        marking: Marking::of(team),
        areas: team.areas(),
        set_piece: team.set_piece(),
        training: Training::new(
            extra.map_or(Training::default().plan(), |e| e.plan()),
            team.training_flags(),
        ),
        individual: human
            .and_then(|slot| records.individual.get(slot))
            .map_or_else(|| state.individual.clone(), |store| store.entries()),
        intensity_states: team.intensity_states(),
        detective: extra.map_or(0, |e| e.detective()),
        co_training: extra.map_or_else(Default::default, |e| CoTraining {
            days: e.co_training_days(),
            units: e.co_training_units(),
            focus: e.co_training_focus(),
        }),
        ..state
    };
    // A game saved in the middle of a matchday goes on with its matches: the
    // human's turn before his, which lets the co-trainer line the team up.
    if records.globals.resume() == MATCHES_RESUMED {
        autolineup::before_match(pitch, &mut club);
    }
    club
}

/// The tactics' players, by their ids: the own two in the squad, the
/// opponent's two in the next opponent's.
fn picks(
    records: &Records,
    data: &Data,
    team: &Team,
    own: &[u16],
    ids: [u16; PICKS],
) -> [Option<Pick>; PICKS] {
    let world = &data.world;
    let own_club = world.slot_of(team.country().into()).map(|slot| (slot, team.number().into()));
    let next = (team.next_country(), u16::try_from(team.next_number()).ok());
    let (theirs, their_club) = match next {
        (country @ 1.., Some(number)) => (
            records.team(country.into(), number).map(Team::players).unwrap_or_default(),
            world.slot_of(country.into()).map(|slot| (slot, usize::from(number))),
        ),
        _ => (Vec::new(), None),
    };
    std::array::from_fn(|i| {
        let (squad, club) = if i < FOUL { (own, own_club) } else { (&theirs[..], their_club) };
        let place = squad.iter().position(|&id| id == ids[i] && id != 0)?;
        Some(Pick { club: club?, place })
    })
}

/// Where the cycle goes on after loading, for a game saved during a
/// matchday's matches (`FUN_006da3a9`).
const MATCHES_RESUMED: u16 = 0x3e6;
/// Germany's nation code: below its second league foreigners count no more
/// (`FUN_0042c802`).
const GERMANY: usize = 14;

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
        feet: record.manner() >> FEET_SHIFT & FEET,
    }
}

pub(crate) fn fitness(record: &records::Player) -> Fitness {
    Fitness {
        condition: record.condition().into(),
        freshness: record.freshness().map(i32::from),
        morale: record.morale().into(),
        mood: record.mood().into(),
        knock: record.knocks() & KNOCKS != 0,
        smoker: record.shown() & SMOKER != 0,
    }
}

/// Which foot a player plays with, in his record's manner.
const FEET_SHIFT: u8 = 2;
const FEET: u8 = 3;
/// Bits of a player's record for knocks he plays with.
const KNOCKS: u16 = 0xe000;
/// The contract's year, in the low twelve bits.
const CONTRACT_YEAR: u16 = 0xfff;
/// Bit 7: the length of a long absence is not known yet.
const UNKNOWN: u8 = 0x80;
/// The automatic line-ups' bits: always pick him, never.
const ALWAYS: u8 = 0x20;
const NEVER: u8 = 0x10;
/// He smokes: his condition counts five less.
const SMOKER: u32 = 0x800;
/// He will not be captain (`FUN_00707588`); his strength is not known yet
/// (`FUN_00696b4c`).
const NO_CAPTAIN: u8 = 0x40;
const STRENGTH_UNKNOWN: u8 = 0x20;
/// The top two bits of `picks` at 1 or 3 (`FUN_00706acd`).
const STRENGTH_HIDDEN: u8 = 0x40;

fn status(records: &Records, medical: &MedicalTables, record: &records::Player) -> Status {
    let globals = &records.globals;
    Status {
        last_form: Some(record.last_form().into()),
        long: days_to(globals, record.long_to()),
        long_unknown: record.long_unknown() & UNKNOWN != 0,
        injured: days_to(globals, record.injured_to()),
        suspended: record.suspension(),
        away: days_to(globals, record.away_to()),
        away_kind: record.away_kind(),
        contract: Some(
            i32::from(record.contract() & CONTRACT_YEAR) - i32::from(globals.start_year()),
        ),
        knocked: record.knocked() != 0,
        dumbbell: i16::from(record.intensity()) == individual::BUILD_UP,
        always: record.picks() & ALWAYS != 0,
        never: record.picks() & NEVER != 0,
        matches: record.league_matches(),
        matches_less: record.league_less(),
        id: record.id(),
        nations: [record.nation(), record.second_nation()],
        understands: record.understands(),
        clashes: record.clashes(),
        joined: record.joined().and_then(|(day, month, year)| {
            NaiveDate::from_ymd_opt(year.into(), month.into(), day.into())
        }),
        no_captain: record.manner() & NO_CAPTAIN != 0,
        strength_unknown: record.retiring() & STRENGTH_UNKNOWN != 0,
        doping: record.doping(),
        strength_hidden: record.picks() & STRENGTH_HIDDEN != 0,
        injected: record.knocks() & medical::INJECTION != 0
            && medical.injection_allowed(record.injury_kind()),
    }
}

/// Any player of the game as a screen shows him, by his record's id: what
/// he is, how fit, how things stand with him.
pub fn any_player(records: &Records, data: &Data, id: u16) -> Option<(Player, Fitness, Status)> {
    let record = records.player(id)?;
    Some((player(records, record), fitness(record), status(records, &data.rules.medical, record)))
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
