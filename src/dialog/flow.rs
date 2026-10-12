//! What the dialogs show and where their buttons lead, from the start menu
//! through every path it offers.
//!
//! A new game runs through the game's own sequence, with the rules each of
//! its screens applies: countries and game length, number of players, a name
//! per player, then for every player a club and national team, abilities and
//! birthday, and finally the start settings. Then the first season is
//! calculated and the season goes on in [`super::turn`]; loading or hosting a
//! game ends where the game would load or host it, back at the start menu.

use bevy::prelude::*;
use chrono::{DateTime, Local};

use super::{CANCEL_IMAGE, OK_IMAGE, Target, View, turn, visible_rows};
use crate::game::Game;
use crate::random::Random;
use crate::rch::{Dialog, Kind};
use crate::season::{NewManager, Season};
use crate::world::{BONUS_SLOT, MAIN_SLOTS, SLOTS, World};

pub const START: u16 = 9;
const COUNTRIES: u16 = 140;
const PLAYERS: u16 = 209;
const NAMES: u16 = 212;
const CLUB: u16 = 304;
const NATIONAL_TEAM: u16 = 352;
const ABILITIES: u16 = 374;
const BIRTHDAY: u16 = 491;
const SETTINGS: u16 = 256;
const LOAD: u16 = 424;
const HIGH_SCORES: u16 = 95;
const NET_NAME: u16 = 474;
const NET_DETAILS: u16 = 473;
const NET_GAMES: u16 = 416;
pub const DIALOGS: [u16; 14] = [
    START,
    COUNTRIES,
    PLAYERS,
    NAMES,
    CLUB,
    NATIONAL_TEAM,
    ABILITIES,
    BIRTHDAY,
    SETTINGS,
    LOAD,
    HIGH_SCORES,
    NET_NAME,
    NET_DETAILS,
    NET_GAMES,
];

/// Controls the flow fills in, by their ids in the dialog file.
const SAVE_LIST: u16 = 5144;
const SAVE_INFO: u16 = 5146;
const SCORES_CAREER: u16 = 2010;
const SCORES_FIXED: u16 = 2011;
const PLAYER_ORDER: u16 = 3640;
const GAME_LIST: u16 = 4979;
const BIRTHDAY_FIELDS: [u16; 3] = [6197, 6198, 6199];
const BIRTHDAY_SPINS: [u16; 3] = [6200, 6201, 6202];
const POSITION: u16 = 6204;
/// The country screen: the main countries' flags and names, and the extra
/// countries' names and flags, by country slot.
const MAIN_FLAGS: [u16; 5] = [2478, 2479, 2480, 5349, 5350];
const MAIN_LABELS: [u16; 5] = [2474, 2475, 2476, 5351, 5352];
const EXTRA_LABELS: [u16; SLOTS] =
    [5317, 2488, 2489, 5318, 5319, 5320, 5324, 5321, 5322, 5353, 5354, 5348];
const EXTRA_FLAGS: [u16; SLOTS] =
    [5311, 2490, 2491, 5312, 5313, 5314, 5323, 5315, 5316, 5345, 5346, 5347];
const CAREER: u16 = 4779;
const DURATION: u16 = 2481;
const PLAYER_COUNT: u16 = 4772;
const LEAGUE_LIST: u16 = 4055;
const CLUB_LIST: u16 = 4052;
const NATION_LIST: u16 = 4512;
const BUDGET: u16 = 4692;
const POINTS: u16 = 4701;
/// The first ability is negotiating skill.
const NEGOTIATING: usize = 0;
const ABILITY_VALUES: [u16; 8] = [4695, 4696, 4697, 4698, 4699, 4700, 5611, 6205];
const ABILITY_SPINS: [u16; 8] = [4702, 4703, 4704, 4705, 4706, 4707, 6033, 6206];

/// Each country's waving flag: its first frame's graphic id and how many
/// follow, by country slot.
const FLAG_FRAMES: [(u16, u16); SLOTS] = [
    (1526, 10),
    (1546, 10),
    (1536, 10),
    (1556, 10),
    (1566, 10),
    (2760, 10),
    (2700, 10),
    (2750, 10),
    (2720, 10),
    (2730, 10),
    (2710, 9),
    (2740, 10),
];
/// At most this many countries are played besides the main one.
const MAX_EXTRA: usize = 4;
/// The fixed game lengths in years, by slider position.
const DURATIONS: [u32; 9] = [1, 2, 3, 4, 5, 10, 20, 50, 100];
const DEFAULT_DURATION: usize = 5;
/// The career options: a real career starts without a club.
const REAL_CAREER: usize = 0;
const DEFAULT_DIFFICULTY: usize = 2;
const NAME_MIN: usize = 2;
/// Abilities run from 0 to 15; the playing strength of a player-manager
/// from 1 to 7, but no higher than the points allow.
const ABILITY_MAX: i32 = 15;
const STRENGTH_MAX: i32 = 7;
/// Every point of playing strength above 1 costs this many ability points.
const STRENGTH_COST: i32 = 5;
/// What a trainer's league demands, by league: 9 for a first division, 6
/// for a second, 3 below; 2 without a club. Seven ability points per level.
const LEVELS: [i32; 2] = [9, 6];
const LOWER_LEVEL: i32 = 3;
const NO_CLUB_LEVEL: i32 = 2;
const POINTS_PER_LEVEL: i32 = 7;
/// The year a new game starts in; trainers are at least 18 then.
const START_YEAR: i32 = 1998;
const OLDEST_BIRTH_YEAR: i32 = 1900;
const DEFAULT_BIRTHDAY: [i32; 3] = [1, 1, 1970];
/// The default position as player-manager: striker.
const DEFAULT_POSITION: usize = 9;

/// Texts of the game, by id in `ListeDerTexte.rh`.
const DURATION_ONE_YEAR: u16 = 1719;
const DURATION_YEARS: u16 = 1720;
const NO_NATION: u16 = 3506;
const ONE_POINT_OVER: u16 = 3817;
const POINTS_OVER: u16 = 3818;
const ONE_POINT_LEFT: u16 = 3819;
const POINTS_LEFT: u16 = 3820;
const ALL_POINTS_SPENT: u16 = 3821;

/// Column starts of the lists, in classic pixels from the list's left edge.
const SAVE_TABS: [f32; 3] = [0.0, 225.0, 318.0];
const INFO_TABS: [f32; 2] = [0.0, 95.0];
const SCORE_TABS: [f32; 8] = [0.0, 32.0, 190.0, 240.0, 310.0, 420.0, 478.0, 528.0];
const GAME_TABS: [f32; 3] = [0.0, 150.0, 205.0];

/// What the player has entered so far, carried from dialog to dialog.
#[derive(Resource)]
pub struct Session {
    mode: Mode,
    start_choice: usize,
    /// The player of a network game.
    first_name: String,
    last_name: String,
    difficulty: usize,
    birthday: [i32; 3],
    /// A new game being set up.
    setup: Setup,
    /// The game being played, once it is calculated.
    pub(super) season: Option<Season>,
    /// How long the open screen has stood, for screens that move on by
    /// themselves.
    pub(super) timer: f32,
    /// What the open question popup asks.
    pub(super) question: Option<turn::Question>,
    /// The day the running week started on.
    pub(super) week_start: usize,
    /// The job offer looked at, by its tab.
    pub(super) offer: usize,
    /// Whether the talks under way began with their first round.
    pub(super) fresh_talks: bool,
    /// The text of the notice popup.
    pub(super) notice: Option<u16>,
    /// The negotiation face on its way to a frame.
    pub(super) morph: Option<(usize, usize)>,
    /// The main screen the turn went to last, to come back to.
    pub(super) hub: u16,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            mode: Mode::Single,
            start_choice: 0,
            first_name: String::new(),
            last_name: String::new(),
            difficulty: DEFAULT_DIFFICULTY,
            birthday: [15, 5, 1975],
            setup: Setup::default(),
            season: None,
            timer: 0.0,
            question: None,
            week_start: 0,
            offer: 0,
            fresh_talks: false,
            notice: None,
            morph: None,
            hub: 0,
        }
    }
}

/// A new game as far as it is set up: what the game keeps in its globals
/// and manager records until the first season is calculated.
struct Setup {
    /// The country slot the game is played in.
    main: usize,
    /// The countries played besides, by slot.
    extra: [bool; SLOTS],
    career: usize,
    /// Position of the game-length slider.
    duration: usize,
    trainers: Vec<Trainer>,
    /// The trainer whose turn it is.
    current: usize,
    /// What the rows of the club screen's lists and the national-team list
    /// stand for.
    leagues: Vec<(usize, usize)>,
    clubs: Vec<usize>,
    nations: Vec<Option<u16>>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            main: 0,
            extra: [false; SLOTS],
            career: REAL_CAREER,
            duration: DEFAULT_DURATION,
            trainers: Vec::new(),
            current: 0,
            leagues: Vec::new(),
            clubs: Vec::new(),
            nations: Vec::new(),
        }
    }
}

impl Setup {
    fn chooses_club(&self) -> bool {
        self.career != REAL_CAREER
    }

    fn trainer(&self) -> Option<&Trainer> {
        self.trainers.get(self.current)
    }

    fn trainer_mut(&mut self) -> Option<&mut Trainer> {
        self.trainers.get_mut(self.current)
    }

    /// The countries in play: the main one, then the extra ones by slot.
    fn countries(&self) -> impl Iterator<Item = usize> + '_ {
        let extra = (0..SLOTS).filter(|&s| self.extra[s] && s != self.main);
        std::iter::once(self.main).chain(extra)
    }

    /// Clubs and national teams the other trainers have taken already.
    fn club_taken(&self, slot: usize, club: usize) -> bool {
        self.trainers
            .iter()
            .enumerate()
            .any(|(i, t)| i != self.current && t.club == Some((slot, club)))
    }

    fn national_team_taken(&self, nation: u16) -> bool {
        self.trainers
            .iter()
            .enumerate()
            .any(|(i, t)| i != self.current && t.national_team == Some(nation))
    }
}

#[derive(Clone)]
struct Trainer {
    first_name: String,
    last_name: String,
    difficulty: usize,
    /// Country slot and club.
    club: Option<(usize, usize)>,
    /// What the league demands; the ability points follow from it.
    level: i32,
    national_team: Option<u16>,
    abilities: [i32; 7],
    /// Playing strength as player-manager.
    strength: i32,
    birthday: [i32; 3],
    position: usize,
}

impl Default for Trainer {
    fn default() -> Self {
        Self {
            first_name: String::new(),
            last_name: String::new(),
            difficulty: DEFAULT_DIFFICULTY,
            club: None,
            level: NO_CLUB_LEVEL,
            national_team: None,
            abilities: [NO_CLUB_LEVEL; 7],
            strength: 1,
            birthday: DEFAULT_BIRTHDAY,
            position: DEFAULT_POSITION,
        }
    }
}

impl Trainer {
    /// Ability points to give out: seven per level, less five for every
    /// point of playing strength above 1.
    fn budget(&self) -> i32 {
        (self.level * POINTS_PER_LEVEL - (self.strength - 1) * STRENGTH_COST).max(0)
    }

    /// The highest playing strength the level allows.
    fn strongest(&self) -> i32 {
        let points = self.level * POINTS_PER_LEVEL;
        if points < STRENGTH_MAX * STRENGTH_COST {
            points / STRENGTH_COST + 1
        } else {
            STRENGTH_MAX
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Single,
    Load,
    ServerNew,
    ServerLoad,
    Client,
}

pub enum Action {
    Stay,
    Open(u16),
    Quit,
}

/// Fills in a dialog that has just opened.
pub fn setup(view: &mut View, dialog: &Dialog, game: &Game, session: &mut Session) {
    if turn::handles(view.id) {
        return turn::setup(view, dialog, game, session);
    }
    let player = (session.setup.current + 1).to_string();
    let name = match view.id {
        CLUB | NATIONAL_TEAM | ABILITIES | BIRTHDAY => {
            session.setup.trainer().map_or("", |t| t.first_name.as_str())
        }
        _ if session.first_name.is_empty() => "Trainer",
        _ => &session.first_name,
    };
    for (state, control) in view.controls.iter_mut().zip(&dialog.controls) {
        state.text = control.text.replace("%d", &player).replace("%s", name);
    }
    let edits = indices(dialog, |k| matches!(k, Kind::Edit { .. }));
    let options = indices(dialog, |k| matches!(k, Kind::Options { image: 0, .. }));

    match view.id {
        START => set_chosen(view, &options, session.start_choice),
        COUNTRIES => show_countries(view, dialog, game, &session.setup),
        NAMES => {
            view.focus = edits.first().copied();
            set_chosen(view, &options, DEFAULT_DIFFICULTY);
        }
        NET_NAME => {
            if let [first, last, ..] = edits[..] {
                view.controls[first].text = session.first_name.clone();
                view.controls[last].text = session.last_name.clone();
                view.focus = Some(first);
            }
            set_chosen(view, &options, session.difficulty);
        }
        CLUB => {
            let main = session.setup.main;
            show_leagues(view, dialog, &game.world, &mut session.setup);
            show_clubs(view, dialog, &game.world, &mut session.setup, (main, 0));
        }
        NATIONAL_TEAM => show_national_teams(view, dialog, game, &mut session.setup),
        ABILITIES => {
            if let Some(trainer) = session.setup.trainer_mut() {
                // Every ability starts at the level, which spends the points.
                trainer.abilities = [trainer.level; 7];
                trainer.strength = 1;
                let budget = trainer.budget().to_string();
                if let Some(i) = control(dialog, BUDGET) {
                    view.controls[i].text = dialog.controls[i].text.replace("%s", &budget);
                }
            }
            show_abilities(view, dialog, game, &session.setup);
        }
        BIRTHDAY => {
            if let Some(trainer) = session.setup.trainer_mut() {
                trainer.birthday = DEFAULT_BIRTHDAY;
                trainer.position = DEFAULT_POSITION;
                show_birthday(view, dialog, trainer.birthday);
                if let Some(i) = control(dialog, POSITION) {
                    view.controls[i].chosen = trainer.position;
                }
            }
        }
        NET_DETAILS => {
            set_chosen(view, &options, session.difficulty);
            show_birthday(view, dialog, session.birthday);
        }
        SETTINGS => {
            let order = session
                .setup
                .trainers
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{}. {} {}", i + 1, t.first_name, t.last_name))
                .collect();
            fill_list(view, dialog, PLAYER_ORDER, order, &[0.0]);
        }
        LOAD => {
            let rows = game
                .save_games()
                .iter()
                .map(|save| {
                    let saved = DateTime::<Local>::from(save.saved);
                    format!(
                        "{}\t{}\t{}",
                        save.name,
                        saved.format("%d.%m.%Y"),
                        saved.format("%H:%M")
                    )
                })
                .collect();
            fill_list(view, dialog, SAVE_LIST, rows, &SAVE_TABS);
            show_save_info(view, dialog, game);
        }
        HIGH_SCORES => {
            let rows = game
                .high_scores()
                .iter()
                .enumerate()
                .map(|(rank, s)| {
                    let (day, month, year) = s.date;
                    format!(
                        "{}.\t{}\t{}\t{}\t{}\t{}\t{}\t{day:02}.{month:02}.{year}",
                        rank + 1,
                        s.name,
                        s.age,
                        s.points,
                        s.points / u32::from(s.seasons.max(1)),
                        s.level,
                        s.titles,
                    )
                })
                .collect();
            fill_list(view, dialog, SCORES_CAREER, rows, &SCORE_TABS);
            fill_list(view, dialog, SCORES_FIXED, Vec::new(), &SCORE_TABS);
            // A table to read, not to pick from.
            for id in [SCORES_CAREER, SCORES_FIXED] {
                if let Some(list) = control(dialog, id) {
                    view.controls[list].selected = None;
                }
            }
        }
        NET_GAMES => {
            let rows = if session.mode == Mode::Client {
                Vec::new()
            } else {
                let host = format!("{} {}", session.first_name, session.last_name);
                let host = if host.trim().is_empty() { "Trainer" } else { host.trim() };
                vec![format!("{host}\t1/4\toffen")]
            };
            fill_list(view, dialog, GAME_LIST, rows, &GAME_TABS);
        }
        _ => {}
    }
    // Column titles line up with the list below them.
    for header in indices(dialog, |k| matches!(k, Kind::Header)) {
        let below = dialog.controls[header].rect.max.y;
        let list = indices(dialog, |k| matches!(k, Kind::ListBox))
            .into_iter()
            .filter(|&l| dialog.controls[l].rect.min.y >= below - 1.0)
            .min_by(|&a, &b| {
                dialog.controls[a].rect.min.y.total_cmp(&dialog.controls[b].rect.min.y)
            });
        if let Some(list) = list {
            let shift = dialog.controls[list].rect.min.x - dialog.controls[header].rect.min.x;
            view.controls[header].tabs =
                view.controls[list].tabs.iter().map(|t| t + shift).collect();
        }
    }
}

/// Reacts to a click or key on `target`; the generic part — choosing an
/// option, selecting a row, focusing a field, stepping a spinner or slider —
/// has already happened.
pub fn act(
    view: &mut View,
    dialog: &Dialog,
    game: &Game,
    session: &mut Session,
    target: Target,
    double: bool,
) -> Action {
    if turn::handles(view.id) {
        return turn::act(view, dialog, game, session, target);
    }
    let image = match target {
        Target::Button(i) => match dialog.controls[i].kind {
            Kind::Button { normal, .. } => normal,
            _ => 0,
        },
        _ => 0,
    };
    let (ok, cancel) = (image == OK_IMAGE, image == CANCEL_IMAGE);
    let double_on = |kind: fn(&Target) -> bool| double && kind(&target);
    let id_of = |i: usize| dialog.controls[i].id.unwrap_or(0);

    match view.id {
        START if ok || double_on(|t| matches!(t, Target::Option { .. })) => {
            let options = indices(dialog, |k| matches!(k, Kind::Options { .. }));
            let choice = options.first().map_or(0, |&o| view.controls[o].chosen);
            session.start_choice = choice;
            start(choice, session)
        }
        COUNTRIES => {
            let setup = &mut session.setup;
            match target {
                Target::Control(i) => {
                    pick_country(setup, id_of(i));
                    show_countries(view, dialog, game, setup);
                    Action::Stay
                }
                Target::SliderStep { control, .. } | Target::SliderAt { control, .. } => {
                    setup.duration = view.controls[control].value.max(0) as usize;
                    show_countries(view, dialog, game, setup);
                    Action::Stay
                }
                Target::Option { control, .. } => {
                    setup.career = view.controls[control].chosen;
                    Action::Stay
                }
                _ if ok => Action::Open(PLAYERS),
                _ if cancel => Action::Open(START),
                _ => Action::Stay,
            }
        }
        PLAYERS if ok => {
            let count = control(dialog, PLAYER_COUNT).map_or(0, |i| view.controls[i].chosen) + 1;
            session.setup.trainers = vec![Trainer::default(); count];
            session.setup.current = 0;
            Action::Open(NAMES)
        }
        NAMES if ok => {
            let [first, last, ..] = indices(dialog, |k| matches!(k, Kind::Edit { .. }))[..] else {
                return Action::Stay;
            };
            let (first_name, last_name) =
                (view.controls[first].text.trim(), view.controls[last].text.trim());
            // Both names need two letters; the field that lacks them gets the focus.
            if first_name.chars().count() < NAME_MIN {
                view.focus = Some(first);
                return Action::Stay;
            }
            if last_name.chars().count() < NAME_MIN {
                view.focus = Some(last);
                return Action::Stay;
            }
            let difficulty = indices(dialog, |k| matches!(k, Kind::Options { image: 0, .. }))
                .first()
                .map_or(DEFAULT_DIFFICULTY, |&o| view.controls[o].chosen);
            let setup = &mut session.setup;
            if let Some(trainer) = setup.trainer_mut() {
                trainer.first_name = first_name.to_owned();
                trainer.last_name = last_name.to_owned();
                trainer.difficulty = difficulty;
            }
            next_trainer(setup, NAMES).unwrap_or_else(|| {
                setup.current = 0;
                Action::Open(if setup.chooses_club() { CLUB } else { ABILITIES })
            })
        }
        CLUB => {
            let setup = &mut session.setup;
            match target {
                Target::Row { control, .. } if id_of(control) == LEAGUE_LIST => {
                    let picked =
                        view.controls[control].selected.and_then(|r| setup.leagues.get(r).copied());
                    if let Some(league) = picked {
                        show_clubs(view, dialog, &game.world, setup, league);
                    }
                    Action::Stay
                }
                _ if ok => {
                    let league = control(dialog, LEAGUE_LIST)
                        .and_then(|i| view.controls[i].selected)
                        .and_then(|r| setup.leagues.get(r).copied());
                    let club = control(dialog, CLUB_LIST)
                        .and_then(|i| view.controls[i].selected)
                        .and_then(|r| setup.clubs.get(r).copied());
                    match (league, club, setup.trainers.get_mut(setup.current)) {
                        (Some((slot, league)), Some(club), Some(trainer)) => {
                            trainer.club = Some((slot, club));
                            trainer.level = LEVELS.get(league).copied().unwrap_or(LOWER_LEVEL);
                            Action::Open(NATIONAL_TEAM)
                        }
                        // Without a club the game goes on to the next trainer.
                        _ => after_club(setup),
                    }
                }
                _ if cancel => Action::Open(START),
                _ => Action::Stay,
            }
        }
        NATIONAL_TEAM if ok => {
            let setup = &mut session.setup;
            let nation = control(dialog, NATION_LIST)
                .and_then(|i| view.controls[i].selected)
                .and_then(|r| setup.nations.get(r).copied())
                .flatten();
            if let Some(trainer) = setup.trainer_mut() {
                trainer.national_team = nation;
            }
            after_club(setup)
        }
        ABILITIES => match target {
            Target::Spin { control, .. } => {
                let value = view.controls[control].value;
                let spin = ABILITY_SPINS.iter().position(|&s| s == id_of(control));
                if let (Some(spin), Some(trainer)) = (spin, session.setup.trainer_mut()) {
                    match trainer.abilities.get_mut(spin) {
                        Some(ability) => *ability = value,
                        None => trainer.strength = value,
                    }
                }
                show_abilities(view, dialog, game, &session.setup);
                Action::Stay
            }
            _ if ok => Action::Open(BIRTHDAY),
            _ => Action::Stay,
        },
        BIRTHDAY => match target {
            Target::Spin { .. } => {
                let mut date = [0; 3];
                for (part, id) in BIRTHDAY_SPINS.iter().enumerate() {
                    date[part] = control(dialog, *id).map_or(1, |i| view.controls[i].value);
                }
                show_birthday(view, dialog, date);
                if let Some(trainer) = session.setup.trainer_mut() {
                    // The day may have been pulled back into a shorter month.
                    trainer.birthday = BIRTHDAY_SPINS
                        .map(|id| control(dialog, id).map_or(1, |i| view.controls[i].value));
                }
                Action::Stay
            }
            _ if ok => {
                let setup = &mut session.setup;
                let position = control(dialog, POSITION).map(|i| view.controls[i].chosen);
                if let (Some(position), Some(trainer)) = (position, setup.trainer_mut()) {
                    trainer.position = position;
                }
                next_trainer(setup, ABILITIES).unwrap_or(Action::Open(SETTINGS))
            }
            _ if cancel => Action::Open(START),
            _ => Action::Stay,
        },
        NAMES | PLAYERS if cancel => Action::Open(START),
        NET_NAME if ok => {
            remember_names(view, dialog, session);
            if session.mode == Mode::Client {
                Action::Open(NET_GAMES)
            } else {
                Action::Open(NET_DETAILS)
            }
        }
        NET_DETAILS => match target {
            Target::Spin { control, up } => {
                let id = dialog.controls[control].id.unwrap_or(0);
                if let Some(part) = BIRTHDAY_SPINS.iter().position(|&s| s == id) {
                    session.birthday[part] += if up { 1 } else { -1 };
                    session.birthday = valid_date(session.birthday);
                    show_birthday(view, dialog, session.birthday);
                }
                Action::Stay
            }
            _ if ok => {
                remember_difficulty(view, dialog, session);
                Action::Open(NET_GAMES)
            }
            _ => Action::Stay,
        },
        SETTINGS if ok => {
            log_setup(&game.world, &session.setup);
            session.season = Some(calculate(game, &session.setup));
            turn::start(session)
        }
        LOAD => match target {
            Target::Row { .. } => {
                show_save_info(view, dialog, game);
                if double { load(view, dialog, session) } else { Action::Stay }
            }
            _ if ok => load(view, dialog, session),
            _ if cancel => Action::Open(START),
            _ => Action::Stay,
        },
        NET_GAMES if ok => the_game_would_start("Netzwerkspiel"),
        HIGH_SCORES if ok => Action::Open(START),
        _ if cancel => Action::Open(START),
        _ => Action::Stay,
    }
}

/// Arrow keys: through the bulleted options if the dialog has some, else
/// through the rows of its first list.
pub fn step(view: &mut View, dialog: &Dialog, step: i32) {
    if let Some(&options) = indices(dialog, |k| matches!(k, Kind::Options { image: 0, .. })).first()
    {
        let Kind::Options { items, .. } = &dialog.controls[options].kind else { return };
        let n = items.len() as i32;
        let state = &mut view.controls[options];
        state.chosen = (state.chosen as i32 + step).rem_euclid(n) as usize;
    } else if let Some(&list) = indices(dialog, |k| matches!(k, Kind::ListBox)).first() {
        let visible = visible_rows(&dialog.controls[list]);
        let state = &mut view.controls[list];
        if !state.rows.is_empty() {
            let n = state.rows.len() as i32;
            let row = state.selected.map_or(0, |r| (r as i32 + step).rem_euclid(n));
            state.selected = Some(row as usize);
            state.reveal(visible);
        }
    }
}

/// Where each entry of the start menu leads.
fn start(choice: usize, session: &mut Session) -> Action {
    let (mode, dialog) = match choice {
        0 => (Mode::Single, COUNTRIES),
        1 => (Mode::Load, LOAD),
        2 => (Mode::ServerNew, NET_NAME),
        3 => (Mode::ServerLoad, LOAD),
        4 => (Mode::Client, NET_NAME),
        5 => return Action::Open(HIGH_SCORES),
        _ => return Action::Quit,
    };
    session.mode = mode;
    if mode == Mode::Single {
        session.setup = Setup::default();
    }
    Action::Open(dialog)
}

/// The same screen again for the next trainer, if there is one.
fn next_trainer(setup: &mut Setup, screen: u16) -> Option<Action> {
    (setup.current + 1 < setup.trainers.len()).then(|| {
        setup.current += 1;
        Action::Open(screen)
    })
}

/// After a trainer's club and national team: the next trainer's club, or
/// everyone's abilities.
fn after_club(setup: &mut Setup) -> Action {
    next_trainer(setup, CLUB).unwrap_or_else(|| {
        setup.current = 0;
        Action::Open(ABILITIES)
    })
}

/// A click on the country screen. A main country becomes the main country,
/// and no longer an extra one; an extra country is added or removed — at
/// most four, and never the main country.
///
/// The game answers only on a main country's flag and an extra country's
/// name; here flag and name both do.
fn pick_country(setup: &mut Setup, id: u16) {
    let slot_of = |ids: &[u16]| ids.iter().position(|&i| i == id);
    if let Some(slot) = slot_of(&MAIN_FLAGS).or_else(|| slot_of(&MAIN_LABELS)) {
        setup.extra[slot] = false;
        setup.main = MAIN_SLOTS[slot];
    } else if let Some(slot) = slot_of(&EXTRA_LABELS).or_else(|| slot_of(&EXTRA_FLAGS)) {
        if slot == setup.main {
            return;
        }
        let chosen = setup.extra.iter().filter(|&&e| e).count();
        if setup.extra[slot] || chosen < MAX_EXTRA {
            setup.extra[slot] = !setup.extra[slot];
        }
    }
}

/// The country screen: flags wave for the main country and the chosen extra
/// ones; the bonus country shows only if one is installed; the last career
/// option names the game length the slider sets.
fn show_countries(view: &mut View, dialog: &Dialog, game: &Game, setup: &Setup) {
    let frames = |slot: usize| {
        let (first, count) = FLAG_FRAMES[slot];
        (first..first + count).collect::<Vec<u16>>()
    };
    for (slot, id) in MAIN_FLAGS.iter().enumerate() {
        if let Some(i) = control(dialog, *id) {
            view.controls[i].frames = frames(slot);
            view.controls[i].playing = setup.main == slot;
        }
    }
    for (slot, flag) in EXTRA_FLAGS.iter().enumerate() {
        if let Some(i) = control(dialog, *flag) {
            view.controls[i].frames = frames(slot);
            view.controls[i].playing = setup.extra[slot];
        }
    }
    for id in MAIN_FLAGS.iter().chain(&MAIN_LABELS).chain(&EXTRA_FLAGS).chain(&EXTRA_LABELS) {
        if let Some(i) = control(dialog, *id) {
            view.controls[i].clickable = true;
        }
    }
    let bonus = game.world.country(BONUS_SLOT).map(|c| game.world.nation(c.nation).to_owned());
    for id in [EXTRA_LABELS[BONUS_SLOT], EXTRA_FLAGS[BONUS_SLOT]] {
        if let Some(i) = control(dialog, id) {
            view.controls[i].hidden = bonus.is_none();
        }
    }
    if let (Some(i), Some(name)) = (control(dialog, EXTRA_LABELS[BONUS_SLOT]), bonus) {
        view.controls[i].text = name;
    }
    if let Some(i) = control(dialog, CAREER) {
        view.controls[i].chosen = setup.career;
        let years = DURATIONS[setup.duration.min(DURATIONS.len() - 1)];
        let label = if setup.duration == 0 {
            game.text(DURATION_ONE_YEAR).to_owned()
        } else {
            game.text(DURATION_YEARS).replace("%d", &years.to_string())
        };
        if let Some(last) = view.controls[i].labels.last_mut() {
            *last = label;
        }
    }
    if let Some(i) = control(dialog, DURATION) {
        view.controls[i].range = (0, DURATIONS.len() as i32 - 1);
        view.controls[i].value = setup.duration as i32;
    }
}

/// The club screen's leagues: those of the main country, then the first
/// league of each extra country, named with its country.
fn show_leagues(view: &mut View, dialog: &Dialog, world: &World, setup: &mut Setup) {
    let mut rows = Vec::new();
    setup.leagues.clear();
    for slot in setup.countries().collect::<Vec<_>>() {
        for (index, league) in world.leagues_in_play(slot, setup.main).iter().enumerate() {
            rows.push(if slot == setup.main {
                league.name.clone()
            } else {
                let nation = world.country(slot).map_or(0, |c| c.nation);
                format!("{} ({})", league.name, world.nation(nation))
            });
            setup.leagues.push((slot, index));
        }
    }
    fill_list(view, dialog, LEAGUE_LIST, rows, &[0.0]);
}

/// The clubs of a league that no other trainer has taken, in the league's
/// order; the first is selected.
fn show_clubs(
    view: &mut View,
    dialog: &Dialog,
    world: &World,
    setup: &mut Setup,
    (slot, index): (usize, usize),
) {
    let (Some(country), Some(league)) =
        (world.country(slot), world.leagues_in_play(slot, setup.main).get(index))
    else {
        return;
    };
    setup.clubs = league.clubs.iter().copied().filter(|&c| !setup.club_taken(slot, c)).collect();
    let rows = setup.clubs.iter().map(|&c| country.clubs[c].name.clone()).collect();
    fill_list(view, dialog, CLUB_LIST, rows, &[0.0]);
}

/// "kein Land", then the national teams no other trainer coaches, by name.
fn show_national_teams(view: &mut View, dialog: &Dialog, game: &Game, setup: &mut Setup) {
    let teams = game.world.national_teams();
    let teams = teams.into_iter().filter(|&n| !setup.national_team_taken(n));
    setup.nations = std::iter::once(None).chain(teams.map(Some)).collect();
    let rows = setup
        .nations
        .iter()
        .map(|n| n.map_or(game.text(NO_NATION), |n| game.world.nation(n)).to_owned())
        .collect();
    fill_list(view, dialog, NATION_LIST, rows, &[0.0]);
}

/// The abilities and playing strength with their spinners' ranges, what is
/// left of the points, and OK only once they are all given out.
fn show_abilities(view: &mut View, dialog: &Dialog, game: &Game, setup: &Setup) {
    let Some(trainer) = setup.trainer() else { return };
    let values = trainer.abilities.iter().copied().chain(std::iter::once(trainer.strength));
    for (n, value) in values.enumerate() {
        if let Some(i) = control(dialog, ABILITY_VALUES[n]) {
            view.controls[i].text = value.to_string();
        }
        if let Some(i) = control(dialog, ABILITY_SPINS[n]) {
            let is_strength = n == trainer.abilities.len();
            view.controls[i].range =
                if is_strength { (1, trainer.strongest()) } else { (0, ABILITY_MAX) };
            view.controls[i].value = value;
        }
    }
    let (spent, budget) = (trainer.abilities.iter().sum::<i32>(), trainer.budget());
    let points = match spent - budget {
        0 => game.text(ALL_POINTS_SPENT).to_owned(),
        -1 => game.text(ONE_POINT_LEFT).to_owned(),
        1 => game.text(ONE_POINT_OVER).to_owned(),
        over if over > 0 => game.text(POINTS_OVER).replace("%d", &over.to_string()),
        under => game.text(POINTS_LEFT).replace("%d", &(-under).to_string()),
    };
    if let Some(i) = control(dialog, POINTS) {
        view.controls[i].text = points;
    }
    for button in indices(dialog, |k| matches!(k, Kind::Button { normal: OK_IMAGE, .. })) {
        view.controls[button].disabled = spent != budget;
    }
}

fn the_game_would_start(what: &str) -> Action {
    info!("{what}: here the game itself would begin; back to the start menu");
    Action::Open(START)
}

/// The calculation of the first season from a finished setup.
fn calculate(game: &Game, setup: &Setup) -> Season {
    let managers = setup
        .trainers
        .iter()
        .map(|t| NewManager {
            first_name: t.first_name.clone(),
            last_name: t.last_name.clone(),
            club: t.club,
            national_team: t.national_team,
            level: t.level,
            difficulty: t.difficulty,
            negotiating: t.abilities[NEGOTIATING],
        })
        .collect();
    let countries = setup.countries().collect();
    Season::first(&game.world, &game.rules, countries, setup.main, managers, Random::from_clock())
}

/// What a finished setup hands to the season calculation.
fn log_setup(world: &World, setup: &Setup) {
    let countries: Vec<&str> = setup
        .countries()
        .filter_map(|slot| world.country(slot).map(|c| world.nation(c.nation)))
        .collect();
    let length = match setup.career {
        REAL_CAREER => "echte Managerkarriere".to_owned(),
        1 => "Karriere mit Vereinsauswahl".to_owned(),
        _ => format!("feste Spieldauer, {} Jahre", DURATIONS[setup.duration]),
    };
    info!("Neues Spiel: {} — {length}", countries.join(", "));
    for t in &setup.trainers {
        let club = t
            .club
            .and_then(|(slot, club)| world.country(slot)?.clubs.get(club))
            .map_or("ohne Verein", |club| club.name.as_str());
        let national = t.national_team.map_or("—", |n| world.nation(n));
        let [day, month, year] = t.birthday;
        info!(
            "  {} {}: {club} (Niveau {}), Nationalteam {national}, Fähigkeiten {:?}, \
             Spielstärke {}, geboren {day}.{month}.{year}, Position {}, Schwierigkeit {}",
            t.first_name, t.last_name, t.level, t.abilities, t.strength, t.position, t.difficulty
        );
    }
}

fn load(view: &View, dialog: &Dialog, session: &Session) -> Action {
    let Some(list) = control(dialog, SAVE_LIST) else { return Action::Stay };
    let state = &view.controls[list];
    let Some(row) = state.selected.and_then(|r| state.rows.get(r)) else { return Action::Stay };
    let name = row.split('\t').next().unwrap_or_default();
    if session.mode == Mode::ServerLoad {
        Action::Open(NET_GAMES)
    } else {
        the_game_would_start(&format!("Spielstand \"{name}\""))
    }
}

fn show_save_info(view: &mut View, dialog: &Dialog, game: &Game) {
    let Some(list) = control(dialog, SAVE_LIST) else { return };
    let name = view.controls[list]
        .selected
        .and_then(|r| view.controls[list].rows.get(r))
        .and_then(|row| row.split('\t').next().map(str::to_owned));
    let save = name.and_then(|name| game.save_games().into_iter().find(|s| s.name == name));
    let rows = save.map_or_else(Vec::new, |save| {
        let saved = DateTime::<Local>::from(save.saved);
        vec![
            format!("Trainer:\t{}", save.trainer),
            format!("Verein:\t{}", save.club),
            format!("Land:\t{}", save.country),
            format!("Gespeichert:\t{}", saved.format("%d.%m.%Y, %H:%M Uhr")),
        ]
    });
    fill_list(view, dialog, SAVE_INFO, rows, &INFO_TABS);
    if let Some(info) = control(dialog, SAVE_INFO) {
        view.controls[info].selected = None;
    }
}

/// Day, month and year, with their spinners' ranges: a trainer is born from
/// 1900 on and at least 18 when the game starts, on a day the month has.
fn show_birthday(view: &mut View, dialog: &Dialog, [day, month, year]: [i32; 3]) {
    let ranges = [(1, days_in_month(month, year)), (1, 12), (OLDEST_BIRTH_YEAR, START_YEAR - 18)];
    // A shorter month pulls the day back.
    let date = [day.min(ranges[0].1), month, year];
    for part in 0..3 {
        if let Some(i) = control(dialog, BIRTHDAY_FIELDS[part]) {
            view.controls[i].text = date[part].to_string();
        }
        if let Some(i) = control(dialog, BIRTHDAY_SPINS[part]) {
            view.controls[i].range = ranges[part];
            view.controls[i].value = date[part];
        }
    }
}

fn days_in_month(month: i32, year: i32) -> i32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Wraps day and month around and keeps the year plausible for a trainer.
fn valid_date([day, month, year]: [i32; 3]) -> [i32; 3] {
    let year = year.clamp(1930, 1985);
    let month = (month - 1).rem_euclid(12) + 1;
    [(day - 1).rem_euclid(days_in_month(month, year)) + 1, month, year]
}

fn remember_names(view: &View, dialog: &Dialog, session: &mut Session) {
    if let [first, last, ..] = indices(dialog, |k| matches!(k, Kind::Edit { .. }))[..] {
        session.first_name = view.controls[first].text.trim().to_owned();
        session.last_name = view.controls[last].text.trim().to_owned();
    }
    remember_difficulty(view, dialog, session);
}

fn remember_difficulty(view: &View, dialog: &Dialog, session: &mut Session) {
    let options = indices(dialog, |k| matches!(k, Kind::Options { image: 0, .. }));
    if let Some(&o) = options.first() {
        session.difficulty = view.controls[o].chosen;
    }
}

fn set_chosen(view: &mut View, options: &[usize], choice: usize) {
    if let Some(&o) = options.first() {
        view.controls[o].chosen = choice;
    }
}

fn fill_list(view: &mut View, dialog: &Dialog, id: u16, rows: Vec<String>, tabs: &[f32]) {
    if let Some(list) = control(dialog, id) {
        let state = &mut view.controls[list];
        state.selected = (!rows.is_empty()).then_some(0);
        state.scroll = 0;
        state.rows = rows;
        state.tabs = tabs.to_vec();
    }
}

pub(super) fn control(dialog: &Dialog, id: u16) -> Option<usize> {
    dialog.controls.iter().position(|c| c.id == Some(id))
}

pub(super) fn indices(dialog: &Dialog, kind: impl Fn(&Kind) -> bool) -> Vec<usize> {
    (0..dialog.controls.len()).filter(|&i| kind(&dialog.controls[i].kind)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_menu_entries_lead_where_the_game_does() {
        let mut session = Session::default();
        assert!(matches!(start(0, &mut session), Action::Open(COUNTRIES)));
        assert!(matches!(start(3, &mut session), Action::Open(LOAD)));
        assert!(session.mode == Mode::ServerLoad);
        assert!(matches!(start(5, &mut session), Action::Open(HIGH_SCORES)));
        assert!(matches!(start(6, &mut session), Action::Quit));
    }

    #[test]
    fn birthday_wraps_like_a_calendar() {
        assert_eq!(valid_date([32, 1, 1975]), [1, 1, 1975]);
        assert_eq!(valid_date([0, 3, 1975]), [31, 3, 1975]);
        assert_eq!(valid_date([30, 2, 1976]), [1, 2, 1976]);
        assert_eq!(valid_date([1, 13, 1990]), [1, 1, 1985]);
    }

    #[test]
    fn at_most_four_extra_countries_and_never_the_main_one() {
        let mut setup = Setup::default();
        // Germany is the main country; its extra entry does nothing.
        pick_country(&mut setup, EXTRA_LABELS[0]);
        assert!(!setup.extra[0]);
        for &label in &EXTRA_LABELS[1..=5] {
            pick_country(&mut setup, label);
        }
        assert_eq!(setup.extra.iter().filter(|&&e| e).count(), MAX_EXTRA);
        assert!(!setup.extra[5]);
        // Making England the main country drops it from the extra ones.
        pick_country(&mut setup, MAIN_FLAGS[1]);
        assert_eq!(setup.main, 1);
        assert!(!setup.extra[1]);
        pick_country(&mut setup, EXTRA_LABELS[1]);
        assert!(!setup.extra[1]);
        // Flags and names do the same.
        pick_country(&mut setup, EXTRA_FLAGS[2]);
        assert!(!setup.extra[2]);
        pick_country(&mut setup, EXTRA_FLAGS[2]);
        assert!(setup.extra[2]);
        pick_country(&mut setup, MAIN_LABELS[0]);
        assert_eq!(setup.main, 0);
    }

    #[test]
    fn ability_points_follow_the_league() {
        let mut trainer = Trainer { level: LEVELS[0], ..Trainer::default() };
        assert_eq!(trainer.budget(), 63);
        assert_eq!(trainer.strongest(), 7);
        trainer.strength = 3;
        assert_eq!(trainer.budget(), 53);
        let unemployed = Trainer::default();
        assert_eq!(unemployed.budget(), 14);
        assert_eq!(unemployed.strongest(), 3);
        let third = Trainer { level: LOWER_LEVEL, ..Trainer::default() };
        assert_eq!(third.strongest(), 5);
    }
}
