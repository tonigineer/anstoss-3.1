//! A new game as its screens take it in — the countries, the career, the
//! players with their clubs, abilities and birthdays — and the calculation
//! of the first season from it; the player of a network game.

use tracing::info;

use crate::data::Data;
use crate::random::Random;
use crate::season::{NewManager, Season};
use crate::world::{SLOTS, World};

/// The fixed game lengths in years, by slider position.
pub const DURATIONS: [u32; 9] = [1, 2, 3, 4, 5, 10, 20, 50, 100];
const DEFAULT_DURATION: usize = 5;
/// The career options: a real career starts without a club.
const REAL_CAREER: usize = 0;
pub const DEFAULT_DIFFICULTY: usize = 2;
/// The playing strength of a player-manager runs from 1 to 7, but no higher
/// than the points allow.
const STRENGTH_MAX: i32 = 7;
/// Every point of playing strength above 1 costs this many ability points.
const STRENGTH_COST: i32 = 5;
/// What a trainer's league demands, by league: 9 for a first division, 6
/// for a second, 3 below; 2 without a club. Seven ability points per level.
pub const LEVELS: [i32; 2] = [9, 6];
pub const LOWER_LEVEL: i32 = 3;
const NO_CLUB_LEVEL: i32 = 2;
const POINTS_PER_LEVEL: i32 = 7;
pub const DEFAULT_BIRTHDAY: [i32; 3] = [1, 1, 1970];
/// The default position as player-manager: striker.
pub const DEFAULT_POSITION: usize = 9;
/// The first ability is negotiating skill.
const NEGOTIATING: usize = 0;

/// How the start menu set out to begin a game.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Mode {
    #[default]
    Single,
    Load,
    ServerNew,
    ServerLoad,
    Client,
}

/// A new game as far as it is set up: what the game keeps in its globals
/// and manager records until the first season is calculated.
pub struct Setup {
    /// The country slot the game is played in.
    pub main: usize,
    /// The countries played besides, by slot.
    pub extra: [bool; SLOTS],
    pub career: usize,
    /// Position of the game-length slider.
    pub duration: usize,
    pub trainers: Vec<Trainer>,
    /// The trainer whose turn it is.
    pub current: usize,
    /// What the rows of the club screen's lists and the national-team list
    /// stand for.
    pub leagues: Vec<(usize, usize)>,
    pub clubs: Vec<usize>,
    pub nations: Vec<Option<u16>>,
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
    /// Whether the players choose their clubs, rather than starting a real
    /// career without one.
    pub fn chooses_club(&self) -> bool {
        self.career != REAL_CAREER
    }

    pub fn trainer(&self) -> Option<&Trainer> {
        self.trainers.get(self.current)
    }

    pub fn trainer_mut(&mut self) -> Option<&mut Trainer> {
        self.trainers.get_mut(self.current)
    }

    /// The first name of the trainer whose turn it is.
    pub fn name(&self) -> String {
        self.trainer().map_or_else(String::new, |t| t.first_name.clone())
    }

    /// The countries in play: the main one, then the extra ones by slot.
    pub fn countries(&self) -> impl Iterator<Item = usize> + '_ {
        let extra = (0..SLOTS).filter(|&s| self.extra[s] && s != self.main);
        std::iter::once(self.main).chain(extra)
    }

    /// Clubs and national teams the other trainers have taken already.
    pub fn club_taken(&self, slot: usize, club: usize) -> bool {
        self.trainers
            .iter()
            .enumerate()
            .any(|(i, t)| i != self.current && t.club == Some((slot, club)))
    }

    pub fn national_team_taken(&self, nation: u16) -> bool {
        self.trainers
            .iter()
            .enumerate()
            .any(|(i, t)| i != self.current && t.national_team == Some(nation))
    }
}

#[derive(Clone)]
pub struct Trainer {
    pub first_name: String,
    pub last_name: String,
    pub difficulty: usize,
    /// Country slot and club.
    pub club: Option<(usize, usize)>,
    /// What the league demands; the ability points follow from it.
    pub level: i32,
    pub national_team: Option<u16>,
    pub abilities: [i32; 7],
    /// Playing strength as player-manager.
    pub strength: i32,
    pub birthday: [i32; 3],
    pub position: usize,
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
    pub fn budget(&self) -> i32 {
        (self.level * POINTS_PER_LEVEL - (self.strength - 1) * STRENGTH_COST).max(0)
    }

    /// The highest playing strength the level allows.
    pub fn strongest(&self) -> i32 {
        let points = self.level * POINTS_PER_LEVEL;
        if points < STRENGTH_MAX * STRENGTH_COST {
            points / STRENGTH_COST + 1
        } else {
            STRENGTH_MAX
        }
    }
}

/// The player of a network game.
pub struct Player {
    pub first_name: String,
    pub last_name: String,
    pub difficulty: usize,
    pub birthday: [i32; 3],
}

impl Default for Player {
    fn default() -> Self {
        Self {
            first_name: String::new(),
            last_name: String::new(),
            difficulty: DEFAULT_DIFFICULTY,
            birthday: [15, 5, 1975],
        }
    }
}

impl Player {
    /// How the dialogs address the player: by first name, or as "Trainer".
    pub fn name(&self) -> String {
        if self.first_name.is_empty() { "Trainer".to_owned() } else { self.first_name.clone() }
    }
}

/// The calculation of the first season from a finished setup.
pub fn calculate(data: &Data, setup: &Setup, seed: u64) -> Season {
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
    Season::first(&data.world, &data.rules, countries, setup.main, managers, Random::new(seed))
}

/// What a finished setup hands to the season calculation.
pub fn log(world: &World, setup: &Setup) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
