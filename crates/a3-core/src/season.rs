//! A season of the game: how the first one is calculated from a new game's
//! setup, and what its screens show.
//!
//! The game does not play an earlier season to start from. Squads, money and
//! fixture lists come from the data files (see [`crate::world`]), the
//! matchdays from the game's calendar (see [`crate::calendar`]); what the
//! calculation adds for the managers' first screens is their board's
//! confidence and every club's next match.

use std::collections::BTreeMap;
use std::io;

use chrono::NaiveDate;

use a3_files::exe::Exe;

use crate::calendar::{self, Calendar};
use crate::duties;
use crate::jobs::Offer;
use crate::lineup::{self, FIELD, FULL_MORALE, Lineup, NEUTRAL_MOOD, Tables};
use crate::random::Random;
use crate::world::{GOALKEEPER, Player, STRIKER, World};

/// A new manager's board starts at 65 to 68 %, plus half the level and three
/// times what the club's file says of its board, up to 100 %.
const CONFIDENCE_BASE: i32 = 65;
const CONFIDENCE_LUCK: u32 = 4;
const CONFIDENCE_PER_BOARD: i32 = 3;
const CONFIDENCE_MAX: i32 = 100;
/// At the start of a season the board warms up: by 10 below 70 %, by 5
/// below 90 %.
const SEASON_WARM_UP: [(i32, i32); 2] = [(70, 10), (90, 5)];
/// Where the game keeps its tables of season goals: by league size (from 10
/// clubs, two by two) and expected rank; one step harder and easier for
/// every goal; a job offer's goal from the club's, for a top league and
/// below; and the amounts of a job offer.
const CLUB_GOALS_AT: u32 = 0x8ac860;
const GOAL_RANKS: usize = 24;
const LEAGUE_SIZES: usize = 8;
const HARDER_AT: u32 = 0x8ac9e0;
const EASIER_AT: u32 = 0x8ac9ec;
const OFFER_GOALS_AT: u32 = 0x8ac9f8;
pub const GOALS: usize = 12;
const AMOUNTS_AT: [u32; 4] = [0x8abb10, 0x8abb28, 0x8abb40, 0x8abb58];
/// Season tickets a club of each league can sell, before its fans count:
/// the first entry of every country slot's leagues, and the numbers.
const TICKET_START_AT: u32 = 0x89dd50;
const TICKET_BASES_AT: u32 = 0x8a0248;
const TICKET_BASES: usize = 24;
/// The phrases a club's name goes into, `v1` to `v20` in the game's texts:
/// a text id for each of the four kinds of article and one for clubs of no
/// kind, 0 for the bare name.
const CLUB_PHRASES_AT: u32 = 0x8a8af0;
pub const CLUB_PHRASES: usize = 20;
pub const ARTICLES: usize = 5;
/// Where crest files get the letters for their nation, by nation code.
const CREST_CODES_AT: u32 = 0x8a4af8;
const NATIONS: usize = 117;
/// The menu bar's symbols: where each starts, its tooltip's text id, and
/// whether a manager without a club finds it greyed out.
const BAR_SYMBOLS_AT: u32 = 0x8a364c;
const BAR_TIPS_AT: u32 = 0x8a3560;
const BAR_WITHOUT_CLUB_AT: u32 = 0x8a3600;
pub const BAR_SYMBOLS: usize = 19;
/// Season goals by number: 0 the championship … 10 staying up.
const PROMOTED_GOAL: usize = 10;
/// A club whose board is not loyal (5) aims one step higher with a chance
/// of 17 in 102 for every step it is short of 6.
const AMBITION: u32 = 17;
const AMBITION_DIE: u32 = 102;
/// Squad rating: the best two goalkeepers, six defenders, five midfielders
/// and three strikers.
const RATED: [(u8, u8, usize); 4] =
    [(GOALKEEPER, GOALKEEPER, 2), (2, 5, 6), (6, 9, 5), (STRIKER, STRIKER, 3)];
/// How last season ended, in a club's file.
const LAST_SEASON_RESULT: i64 = 7;
pub const PROMOTED: i64 = 4;
pub const CHAMPION: i64 = 1;

/// The game's rules that live in tables of the executable.
pub struct Rules {
    club_goals: Vec<Vec<u8>>,
    pub harder: Vec<u8>,
    pub easier: Vec<u8>,
    /// For a top league and below.
    pub offer_goals: [Vec<u16>; 2],
    /// For salary, point bonus, cup round and title, by class: base, step and
    /// count of steps.
    pub amounts: [Vec<[u16; 3]>; 4],
    ticket_start: Vec<u16>,
    ticket_bases: Vec<u16>,
    /// The letters of a nation's crest files.
    crest_codes: Vec<String>,
    pub club_phrases: Vec<u32>,
    /// The pitch: formations, positions and what players count for on them.
    pub pitch: Tables,
    /// The menu bar: where each symbol starts, its tooltip, and which ones a
    /// manager without a club cannot use.
    pub bar_symbols: Vec<u16>,
    pub bar_tips: Vec<u32>,
    pub bar_without_club: Vec<bool>,
    /// What players are worth.
    pub value: crate::value::ValueTables,
    /// What the screens call a player's things.
    pub words: crate::words::Words,
    /// Bonuses and fines.
    pub discipline: crate::discipline::DisciplineTables,
    /// Injuries: which allow an injection.
    pub medical: crate::medical::MedicalTables,
    /// The tactics screen's states and its special win bonus.
    pub tactics: crate::tactics::TacticsTables,
    /// The training's units, words and knowledge.
    pub training: crate::training::TrainingTables,
}

impl Rules {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let goals = exe.bytes(CLUB_GOALS_AT, GOAL_RANKS * LEAGUE_SIZES)?;
        let offer_goals = exe.u16s(OFFER_GOALS_AT, 2 * GOALS)?;
        let amounts = AMOUNTS_AT
            .map(|at| exe.u16s(at, 12).map(|v| v.chunks(3).map(|c| [c[0], c[1], c[2]]).collect()));
        let [a, b, c, d] = amounts;
        Ok(Self {
            club_goals: goals.chunks(GOAL_RANKS).map(<[u8]>::to_vec).collect(),
            harder: exe.bytes(HARDER_AT, GOALS)?.to_vec(),
            easier: exe.bytes(EASIER_AT, GOALS)?.to_vec(),
            offer_goals: [offer_goals[..GOALS].to_vec(), offer_goals[GOALS..].to_vec()],
            amounts: [a?, b?, c?, d?],
            ticket_start: exe.u16s(TICKET_START_AT, crate::world::SLOTS)?,
            ticket_bases: exe.u16s(TICKET_BASES_AT, TICKET_BASES)?,
            club_phrases: exe.u32s(CLUB_PHRASES_AT, CLUB_PHRASES * ARTICLES)?,
            crest_codes: exe
                .u32s(CREST_CODES_AT, NATIONS)?
                .into_iter()
                .map(|pointer| exe.text_at(pointer).unwrap_or_default())
                .collect(),
            pitch: Tables::load(exe)?,
            bar_symbols: exe.u16s(BAR_SYMBOLS_AT, BAR_SYMBOLS)?,
            bar_tips: exe.u32s(BAR_TIPS_AT, BAR_SYMBOLS)?,
            bar_without_club: exe
                .u32s(BAR_WITHOUT_CLUB_AT, BAR_SYMBOLS)?
                .into_iter()
                .map(|off| off != 0)
                .collect(),
            value: crate::value::ValueTables::load(exe)?,
            words: crate::words::Words::load(exe)?,
            discipline: crate::discipline::DisciplineTables::load(exe)?,
            medical: crate::medical::MedicalTables::load(exe)?,
            tactics: crate::tactics::TacticsTables::load(exe)?,
            training: crate::training::TrainingTables::load(exe)?,
        })
    }

    /// A club's crest, a file of the installation: by the letters of its
    /// nation and its place in its country's file, counted from 1; the bonus
    /// country's start with `bw_`.
    pub fn crest(&self, world: &World, (slot, club): (usize, usize)) -> Option<String> {
        let nation = world.country(slot)?.nation;
        let code = self.crest_codes.get(usize::from(nation)).filter(|c| !c.is_empty())?;
        let prefix = if slot == crate::world::BONUS_SLOT { "bw" } else { "vw" };
        Some(format!("Data.a3/Wappen/{prefix}_{code}{}.bmp", club + 1))
    }

    /// How many season tickets a club of a country slot's league sells at
    /// most, before its fans count.
    fn ticket_base(&self, slot: usize, league: usize) -> i64 {
        let start = self.ticket_start.get(slot).map_or(0, |&s| usize::from(s));
        self.ticket_bases.get(start + league).map_or(100, |&b| i64::from(b))
    }

    /// A club's own goal for the season, from how big its league is and
    /// where its squad ranks in it.
    fn club_goal(
        &self,
        size: usize,
        rank: usize,
        last_season: i64,
        board: i64,
        random: &mut Random,
    ) -> usize {
        let row = (size / 2).saturating_sub(5).min(self.club_goals.len() - 1);
        let mut goal = usize::from(self.club_goals[row][rank.min(GOAL_RANKS - 1)]);
        if last_season & LAST_SEASON_RESULT == PROMOTED {
            goal = PROMOTED_GOAL;
        }
        let short = (6 - board).max(0) as u32;
        if random.below(AMBITION_DIE) < AMBITION * short {
            goal = usize::from(self.harder[goal.min(GOALS - 1)]);
        }
        goal
    }
}

/// A player's condition and freshness when a new game loads, in hundredths:
/// a base, a roll of which only the low byte counts, and a shift for some
/// characters. At the easiest difficulty a manager's new squad gets at least
/// 51 to 59.
const CONDITION_BASE: i32 = 3500;
const CONDITION_ROLL: u32 = 1600;
const FRESHNESS_BASE: i32 = 6000;
const FRESHNESS_ROLL: u32 = 2100;
const TRAIT_SHIFT: i32 = 1000;
const EASY_FLOOR: i32 = 5100;
const FULL: i32 = 10_000;
/// Every day without training a player loses half a point of condition and
/// gains freshness, the more the easier the game.
const DAILY_CONDITION_LOSS: i32 = 50;
const RECOVERY: [i32; 5] = [89, 85, 81, 70, 64];
/// Season tickets sell during the preseason, up to half the stadium.
const PRESEASON_DAYS: usize = 42;
const TICKET_SHARE: i64 = 50;
/// Last season's cup winner.
const CUP_WINNER: i64 = 2;

/// The game in progress.
///
/// Clubs are kept in maps ordered by country slot and index, so that a day
/// draws its random numbers in the same order on every run.
#[derive(PartialEq)]
pub struct Season {
    pub year: i32,
    /// Days since the season started.
    pub day: usize,
    pub managers: Vec<Manager>,
    /// The manager whose turn it is.
    pub current: usize,
    /// The country slot the game is played in.
    pub main: usize,
    /// Every club in a league: its squad's rank in the league before a ball
    /// is kicked, and its own goal for the season.
    pub expected: BTreeMap<(usize, usize), (usize, usize)>,
    /// The clubs with a human manager, as the week shows them.
    pub clubs: BTreeMap<(usize, usize), ClubState>,
    /// What the week tells of today's appointment in the planner: the
    /// text of its kind ("Einzelgespräch", "Krankenbesuch") and the last
    /// name of the manager who had it (`IDC_ABLAUF_WOCHE_MELDUNG`).
    pub planner_message: Option<(u16, String)>,
    pub random: Random,
}

/// What the game follows of a club with a human manager.
#[derive(PartialEq)]
pub struct ClubState {
    /// The squad: the world's for a new game, the save's for a loaded one.
    pub players: Vec<Player>,
    pub lineup: Lineup,
    /// Which of the manager's formations the line-up stands in.
    pub formation: usize,
    pub tickets: i64,
    pub ticket_target: i64,
    /// What the club's training and its manager's time there add to its
    /// cohesion ([`crate::cohesion`]): nothing at the start of a new game.
    pub cohesion_bonus: i32,
    /// Every player's condition, freshness, morale and mood.
    pub fitness: Vec<Fitness>,
    /// Every player's trend and absences, as far as the save tells them.
    pub status: Vec<Status>,
    /// Whether the club has a co-trainer, who can line the team up.
    pub co_trainer: bool,
    /// The line-up's special settings: bit 1 the co-trainer lines the team
    /// up before a match in a system of his choosing, bit 2 in the one it
    /// stands in (the club's extra data +0x2824).
    pub line_up_flags: u16,
    /// Whether the club's league lets three foreigners play at most.
    pub foreigner_rule: bool,
    /// What the team counts for in goal, defence, midfield and attack, as
    /// the save has it; `None` while it is reckoned from the line-up.
    pub strengths: Option<[i32; lineup::PARTS]>,
    /// The week so far: the team's condition and freshness at its start and
    /// after each day.
    pub week: Vec<(i32, i32)>,
    /// Who takes the set pieces and who is captain, in the manager's order
    /// ([`crate::duties`]); none for a club without a human manager.
    pub duties: crate::duties::Lists,
    /// How the team plays, as the tactics screen sets it.
    pub tactics: crate::tactics::Tactics,
    /// Who of the eleven marks whom of the next opponent's, where each
    /// acts, and who goes forward at set pieces (a player's id, 0 none), as
    /// the comparison with the opponent sets them.
    pub marking: crate::marking::Marking,
    pub areas: [crate::marking::Area; FIELD],
    pub set_piece: u16,
    /// The training plan, and the camp and meeting before the next match.
    pub training: crate::training::Training,
}

/// How a player is: fit and fresh, in hundredths, and in what spirits.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Fitness {
    pub condition: i32,
    pub freshness: [i32; 3],
    /// 100 is neither happy nor unhappy (`FULL_MORALE`).
    pub morale: i32,
    /// Around 10 (`NEUTRAL_MOOD`).
    pub mood: i32,
    /// He plays with a knock, at 0.9 of his strength.
    pub knock: bool,
    /// He smokes: his condition counts five less in his form
    /// (`FUN_00457e00`).
    pub smoker: bool,
}

/// What the line-up's list tells of a player besides his fitness, as the
/// original reckons it from his record (`FUN_00636638`): nothing for a new
/// game.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Status {
    /// The form the list compares today's with, for its trend.
    pub last_form: Option<i32>,
    /// Days of a long absence, and whether its length is not known yet.
    pub long: i32,
    pub long_unknown: bool,
    /// Days the injury still keeps him out.
    pub injured: i32,
    /// Matches suspended in the low six bits; bit 7: how many is not known.
    pub suspended: u8,
    /// Days away from the club, and why, in the low four bits; bit 4 marks
    /// him on the line-up list.
    pub away: i32,
    pub away_kind: u8,
    /// Years his contract still runs.
    pub contract: Option<i32>,
    /// Knocked about ("angeschlagen"), and marked with the dumbbell.
    pub knocked: bool,
    pub dumbbell: bool,
    /// Whether the automatic line-ups always pick him, or never.
    pub always: bool,
    pub never: bool,
    /// League matches this season, and two counts cohesion takes from
    /// them (meaning not traced yet).
    pub matches: u8,
    pub matches_less: [u8; 2],
    /// His id in the save, 0 in a new game; who he understands blindly on
    /// the pitch, and who he does not get on with there, by id.
    pub id: u16,
    /// His nations, 0 none (or not known, in a new game).
    pub nations: [u8; 2],
    pub understands: [u16; 2],
    pub clashes: [u16; 2],
    /// When he came to the club.
    pub joined: Option<NaiveDate>,
    /// He will not be captain; his strength is not known yet (an amateur's);
    /// his doping, as the record keeps it.
    pub no_captain: bool,
    pub strength_unknown: bool,
    pub doping: u8,
    /// His strength is hidden (`FUN_00706acd`): the list does not mark
    /// his contract's end.
    pub strength_hidden: bool,
    /// He may play with an injection despite his injury (`FUN_006ffa39`).
    pub injected: bool,
}

impl ClubState {
    /// A club followed from now on: no season tickets sold yet, the week's
    /// record starting today.
    pub fn new(
        players: Vec<Player>,
        lineup: Lineup,
        fitness: Vec<Fitness>,
        ticket_target: i64,
    ) -> Self {
        let week = vec![team_fitness(&lineup, &fitness)];
        Self {
            players,
            lineup,
            formation: 0,
            tickets: 0,
            ticket_target,
            cohesion_bonus: 0,
            fitness,
            status: Vec::new(),
            co_trainer: false,
            line_up_flags: 0,
            foreigner_rule: false,
            strengths: None,
            week,
            duties: Default::default(),
            tactics: Default::default(),
            marking: Default::default(),
            areas: Default::default(),
            set_piece: 0,
            training: Default::default(),
        }
    }
}

impl Fitness {
    /// A player as a new game starts him, with this condition and freshness.
    pub fn new(condition: i32, freshness: i32) -> Self {
        Self {
            condition,
            freshness: [freshness, 0, 0],
            morale: FULL_MORALE,
            mood: NEUTRAL_MOOD,
            knock: false,
            smoker: false,
        }
    }
}

#[derive(PartialEq)]
pub struct Manager {
    pub first_name: String,
    pub last_name: String,
    /// Country slot and club.
    pub club: Option<(usize, usize)>,
    pub national_team: Option<u16>,
    pub level: i32,
    pub difficulty: usize,
    /// The ability the game reads as negotiating skill.
    pub negotiating: i32,
    /// The board's confidence in percent.
    pub confidence: i32,
    /// Job offers on the table, for a manager without a club.
    pub offers: Vec<Offer>,
    /// How the manager gets around between weeks, as the game keeps it:
    /// with the menu bar (0, the default), from the office (1) or a text
    /// menu (2).
    pub menu: u8,
    /// Whether his line-up shows as a list (dialog 429) rather than on the
    /// pitch (147), as he last switched it.
    pub list_view: bool,
    /// The formations he chooses from.
    pub formations: Vec<lineup::Formation>,
    /// His week templates, and the template and the player in individual
    /// training he chose for each week of the game.
    pub templates: Vec<crate::training::Template>,
    pub weeks: Vec<(u16, u16)>,
    /// Whether his short-term training shows as pictures (dialog 513).
    pub training_pictures: bool,
}

/// A club's next match.
pub struct Match {
    pub day: usize,
    pub matchday: usize,
    /// Country slot and club of both teams, home first.
    pub home: (usize, usize),
    pub away: (usize, usize),
    pub league: String,
}

/// What a new game hands to the calculation, per manager.
pub struct NewManager {
    pub first_name: String,
    pub last_name: String,
    pub club: Option<(usize, usize)>,
    pub national_team: Option<u16>,
    pub level: i32,
    pub difficulty: usize,
    pub negotiating: i32,
}

impl Season {
    /// The calculation of the first season.
    pub fn first(
        world: &World,
        rules: &Rules,
        countries: Vec<usize>,
        main: usize,
        managers: Vec<NewManager>,
        mut random: Random,
    ) -> Self {
        let mut expected = BTreeMap::new();
        for &slot in &countries {
            let Some(country) = world.country(slot) else { continue };
            for league in world.leagues_in_play(slot, main) {
                let mut ranked: Vec<usize> = league.clubs.clone();
                ranked.sort_by_key(|&c| std::cmp::Reverse(rating(&country.clubs[c].players)));
                for (rank, &c) in ranked.iter().enumerate() {
                    let club = &country.clubs[c];
                    let goal = rules.club_goal(
                        league.clubs.len(),
                        rank,
                        club.last_season,
                        club.board,
                        &mut random,
                    );
                    expected.insert((slot, c), (rank, goal));
                }
            }
        }
        let managers = managers
            .into_iter()
            .map(|m| {
                let confidence = m.club.map_or(0, |club| {
                    let taken = taking_confidence(world, club, m.level, &mut random);
                    season_start_confidence(world, club, taken)
                });
                Manager {
                    first_name: m.first_name,
                    last_name: m.last_name,
                    club: m.club,
                    national_team: m.national_team,
                    level: m.level,
                    difficulty: m.difficulty,
                    negotiating: m.negotiating,
                    confidence,
                    offers: Vec::new(),
                    menu: 0,
                    list_view: false,
                    formations: rules.pitch.new_formations(),
                    templates: Vec::new(),
                    weeks: Vec::new(),
                    training_pictures: false,
                }
            })
            .collect();
        let mut season = Self {
            year: world.start_year,
            day: 0,
            managers,
            current: 0,
            main,
            expected,
            clubs: BTreeMap::new(),
            planner_message: None,
            random,
        };
        for m in 0..season.managers.len() {
            if let Some(club) = season.managers[m].club {
                season.follow(world, rules, m, club);
            }
        }
        crate::jobs::offer_jobs(&mut season, world, rules);
        season
    }

    /// Starts following a human club: its season tickets and its players'
    /// fitness, which the game rolls for every player as it loads a new
    /// game, and which the easiest difficulty raises when a manager takes
    /// the club.
    fn follow(
        &mut self,
        world: &World,
        rules: &Rules,
        manager: usize,
        (slot, index): (usize, usize),
    ) {
        let Some(country) = world.country(slot) else { return };
        let Some(club) = country.clubs.get(index) else { return };
        let easiest = self.managers[manager].difficulty == 0;
        let random = &mut self.random;
        let fitness = club
            .players
            .iter()
            .map(|player| {
                // Only the low byte of the roll counts, with its sign.
                let mut condition =
                    CONDITION_BASE + i32::from(random.below(CONDITION_ROLL) as u8 as i8);
                let mut fresh =
                    FRESHNESS_BASE + i32::from(random.below(FRESHNESS_ROLL) as u8 as i8);
                let [a, b] = player.traits;
                let shift = if a & 8 != 0 || b & 0x40 != 0 {
                    -TRAIT_SHIFT
                } else if a & 4 != 0 || b & 0x202 != 0 {
                    TRAIT_SHIFT
                } else {
                    0
                };
                condition += shift;
                fresh += shift;
                if easiest && condition < EASY_FLOOR {
                    condition = (random.below(9) as i32 + 51) * 100;
                }
                if easiest && fresh < EASY_FLOOR + 100 {
                    fresh = (random.below(9) as i32 + 51) * 100;
                }
                Fitness::new(condition, fresh)
            })
            .collect();
        let league =
            world.leagues_in_play(slot, self.main).iter().position(|l| l.clubs.contains(&index));
        let size =
            league.map_or(18, |l| world.leagues_in_play(slot, self.main)[l].clubs.len()) as i64;
        let rank = self.expected.get(&(slot, index)).map_or(0, |&(rank, _)| rank) as i64;
        let mut target = rules.ticket_base(slot, league.unwrap_or(0)) * (8 - club.fans);
        target = target * (100 + size % 2 - rank) / 100;
        target = match club.last_season & 7 {
            1 => target * 135 / 100,
            6 => target * 120 / 100,
            2 => target * 11 / 10,
            PROMOTED => target * 115 / 100,
            _ => target,
        };
        if club.last_season & 0x40 != 0 {
            target = target * 110 / 100;
        }
        target = target.min(club.capacity * TICKET_SHARE / 100).max(0);
        // A new manager starts with the first formation.
        let lineup = rules.pitch.line_up(&club.players, 0);
        let mut state = ClubState::new(club.players.clone(), lineup, fitness, target);
        // Everyone joined the club the day the game began, as its first
        // calculation records it.
        let joined = Status { joined: Some(calendar::date(self.year, 0)), ..Status::default() };
        state.status = vec![joined; club.players.len()];
        state.duties = duties::choose(&state.players, &state.fitness, &mut state.status);
        self.clubs.insert((slot, index), state);
    }

    /// A day passes for the followed clubs: season tickets sell before the
    /// league starts, fitness recovers — there is no training to wear it.
    pub fn pass_day(&mut self) {
        let preseason = self.day < PRESEASON_DAYS;
        let difficulties: BTreeMap<(usize, usize), usize> =
            self.managers.iter().filter_map(|m| Some((m.club?, m.difficulty))).collect();
        for (&(slot, index), state) in &mut self.clubs {
            if preseason && state.tickets < state.ticket_target {
                let sold = (i64::from(self.random.below(5)) + 1) * state.ticket_target / 100;
                state.tickets += sold / 5 * 5;
            }
            let recovery = RECOVERY[difficulties.get(&(slot, index)).copied().unwrap_or(2).min(4)];
            for f in &mut state.fitness {
                f.freshness[1] = f.freshness[2];
                f.freshness[2] = 0;
                f.freshness[0] = (f.freshness[0] + recovery).min(FULL);
                f.condition = (f.condition - DAILY_CONDITION_LOSS).max(0);
            }
            state.week.push(team_fitness(&state.lineup, &state.fitness));
        }
    }

    /// A new week starts the panels' records afresh.
    pub fn start_week(&mut self) {
        for state in self.clubs.values_mut() {
            state.week = vec![team_fitness(&state.lineup, &state.fitness)];
        }
    }

    /// A manager takes over a club.
    pub fn take_club(
        &mut self,
        world: &World,
        rules: &Rules,
        manager: usize,
        club: (usize, usize),
    ) {
        let level = self.managers[manager].level;
        let confidence = taking_confidence(world, club, level, &mut self.random);
        let m = &mut self.managers[manager];
        m.club = Some(club);
        m.confidence = season_start_confidence(world, club, confidence);
        m.offers.clear();
        self.follow(world, rules, manager, club);
    }

    pub fn date(&self) -> NaiveDate {
        calendar::date(self.year, self.day)
    }

    pub fn manager(&self) -> Option<&Manager> {
        self.managers.get(self.current)
    }

    /// A club's next league match from today on.
    pub fn next_match(
        &self,
        world: &World,
        calendar: &Calendar,
        club: (usize, usize),
    ) -> Option<Match> {
        self.next_match_from(world, calendar, club, self.day)
    }

    /// A club's next league match from a day of the season on.
    pub fn next_match_from(
        &self,
        world: &World,
        calendar: &Calendar,
        (slot, club): (usize, usize),
        from: usize,
    ) -> Option<Match> {
        let country = world.country(slot)?;
        let leagues = world.leagues_in_play(slot, self.main);
        let (index, league, position) = leagues.iter().enumerate().find_map(|(i, league)| {
            Some((i, league, league.clubs.iter().position(|&c| c == club)?))
        })?;
        let row = calendar.row(country.nation, index)?;
        let mut day = from;
        loop {
            let (found, matchday) = calendar.next_matchday(row, day)?;
            let pairings = league.fixtures.get(matchday - 1);
            if let Some(&(home, away)) = pairings
                .and_then(|p| p.iter().find(|&&(home, away)| home == position || away == position))
            {
                return Some(Match {
                    day: found,
                    matchday,
                    home: (slot, league.clubs[home]),
                    away: (slot, league.clubs[away]),
                    league: league.name.clone(),
                });
            }
            day = found + 1;
        }
    }
}

/// The board's confidence in a manager who takes over a club: 65 to 68 %,
/// plus half the level and three times what the club's file says of its
/// board, up to 100 %.
fn taking_confidence(
    world: &World,
    (slot, club): (usize, usize),
    level: i32,
    random: &mut Random,
) -> i32 {
    let board = world.country(slot).and_then(|c| c.clubs.get(club)).map_or(0, |c| c.board as i32);
    let taken = CONFIDENCE_BASE
        + random.below(CONFIDENCE_LUCK) as i32
        + level / 2
        + CONFIDENCE_PER_BOARD * board;
    taken.clamp(CONFIDENCE_BASE, CONFIDENCE_MAX)
}

/// At the start of a season the board warms up, and counts the time the
/// manager has been at the club — a year, the way the game reckons it for
/// someone new — and a cup win the season before.
fn season_start_confidence(world: &World, (slot, club): (usize, usize), confidence: i32) -> i32 {
    let warmed = match SEASON_WARM_UP.iter().find(|&&(below, _)| confidence < below) {
        Some(&(_, warm_up)) => (confidence + warm_up).min(CONFIDENCE_MAX),
        None => confidence,
    };
    let cup_winner = world
        .country(slot)
        .and_then(|c| c.clubs.get(club))
        .is_some_and(|c| c.last_season & 7 == CUP_WINNER);
    let tenure = if cup_winner { 3 } else { 1 };
    (warmed + tenure).min(CONFIDENCE_MAX)
}

/// The team's condition and freshness as the week panel shows them: the
/// average over the line-up and the bench, plus one, up to 100.
fn team_fitness(lineup: &Lineup, fitness: &[Fitness]) -> (i32, i32) {
    let squad: Vec<usize> = lineup.squad().collect();
    if squad.is_empty() {
        return (50, 50);
    }
    let (mut condition, mut freshness) = (0, 0);
    for &i in &squad {
        let f = lineup::fitness_of(fitness, i);
        condition += f.condition / 100;
        freshness += (f.freshness.iter().sum::<i32>()).clamp(0, FULL) / 100;
    }
    let n = squad.len() as i32;
    ((condition / n + 1).min(100), (freshness / n + 1).min(100))
}

/// How good a squad is on paper: its best players by position, ten points
/// for every point of strength, averaged.
fn rating(players: &[Player]) -> i32 {
    let mut values = Vec::new();
    for (from, to, wanted) in RATED {
        let mut fitting: Vec<i32> = players
            .iter()
            .filter(|p| (from..=to).contains(&p.positions[0]))
            .map(|p| i32::from(p.strength) * 10)
            .collect();
        fitting.sort_unstable_by(|a, b| b.cmp(a));
        values.extend(fitting.into_iter().take(wanted));
    }
    if values.is_empty() { 0 } else { values.iter().sum::<i32>() * 10 / values.len() as i32 }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_same_seed_gives_the_same_week() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let world = World::load(&dir.join("Data.a3")).unwrap();
        let rules = Rules::load(&Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let germany = world.country(0).unwrap();
        let club = |name: &str| germany.clubs.iter().position(|c| c.name == name).map(|c| (0, c));
        let manager = |club, difficulty| NewManager {
            first_name: "A".into(),
            last_name: "Test".into(),
            club,
            national_team: None,
            level: 2,
            difficulty,
            negotiating: 2,
        };
        // Several followed clubs, so that the order they are passed in counts.
        let week = |seed| {
            let managers = vec![
                manager(club("FC Bayern München"), 0),
                manager(club("SpVgg Unterhaching"), 2),
                manager(club("Hamburger SV"), 4),
                manager(None, 1),
            ];
            let mut season = Season::first(&world, &rules, vec![0], 0, managers, Random::new(seed));
            assert_eq!(season.clubs.len(), 3);
            season.start_week();
            for _ in 0..7 {
                season.pass_day();
                season.day += 1;
            }
            season
        };
        assert!(week(7) == week(7), "two weeks from the same seed differ");
        assert!(week(7) != week(8), "two weeks from different seeds are the same");
    }
}
