//! A club's line-up: eleven players on the spots of a formation and seven on
//! the bench, and what the line-up screen reckons from them.
//!
//! The pitch is a grid the game keeps in tables of its executable: spots of
//! a formation sit on its lines, and areas of it make the positions — a
//! player on a spot plays the position of the area it lies in. A manager
//! who takes over a club starts with the first formation, 5-3-2, and the
//! game lines up the strongest player of each position on its spots and the
//! strongest of the rest on the bench.

use std::io;

use a3_files::exe::Exe;

use crate::season::Fitness;
use crate::world::{GOALKEEPER, Player};

pub const FIELD: usize = 11;
pub const BENCH: usize = 7;
/// Formations: a name of ten bytes, then the grid lines of each spot.
const FORMATIONS_AT: u32 = 0x89e268;
const FORMATION_SIZE: u32 = 0x36;
const FORMATION_NAME: usize = 10;
const FORMATIONS: usize = 7;
/// The grid: where its lines run across and down the pitch, in pitch
/// pixels, the goal at the bottom.
const ACROSS_AT: u32 = 0x8a4e40;
const ACROSS: usize = 19;
const DOWN_AT: u32 = 0x8a4e68;
const DOWN: usize = 13;
/// The areas of the pitch: their first and last lines across and down and
/// the position they make, and the kind of position each area counts as.
const AREAS_AT: u32 = 0x8a4dc8;
const AREAS: usize = 20;
const AREA_KINDS_AT: u32 = 0x8a4e2c;
/// An area reaches this far beyond its last lines.
const AREA_REACH: (i16, i16) = (21, 17);
/// The kind of position each position counts as: the first ten are their
/// own, the special ones — the sweeper, the playmaker, the wing backs … —
/// count as one of them.
const KINDS_AT: u32 = 0x89e23c;
const POSITIONS: usize = 21;
/// The names of the positions, and their abbreviations by kind.
const POSITION_NAMES_AT: u32 = 0x8a84e4;
const ABBREVIATIONS_AT: u32 = 0x8a8490;
/// The graphics of the abilities a list shows: sixteen a field player can be
/// good at, then bad at, nine a goalkeeper can be good at, then bad at; and
/// the six duties' ([`crate::duties::Duty::ON_PITCH`]).
const ABILITY_ICONS_AT: u32 = 0x89f918;
const ABILITY_ICONS: usize = 50;
const DUTY_ICONS: usize = 6;
const FIELD_ABILITIES: usize = 16;
const KEEPER_ABILITIES: usize = 9;
/// What abilities and character add when the game ranks a player for its
/// automatic line-ups (`FUN_006a1642`): a value by bit and by column for a
/// field player's abilities, a keeper's, and the two words of character.
const BONUSES_AT: [u32; 4] = [0x8a50d8, 0x8a5138, 0x8a5198, 0x8a51f8];
const BONUS_BITS: usize = 16;
const KEEPER_BITS: usize = 9;
/// How much of his strength a player brings on another kind of position
/// than his main one, in tenths: by main position, then the other kind.
const FIT_AT: u32 = 0x8a4ec0;
const KINDS: usize = 11;
/// How much a player counts for the goalkeeper, the defence, the midfield
/// and the attack, in percent, by the line down the pitch he stands on.
const PARTS_AT: u32 = 0x8a5258;
const PART_ROW: usize = 6;
pub const PARTS: usize = 4;
/// On a side position a player still brings 90 %, on a position of the
/// other side of the goal line 10 %.
const SIDE_FIT: i32 = 9;
const FULL_FIT: i32 = 10;
const GOAL_FIT: i32 = 1;
/// The most a player counts for on a position, in tenths of strength.
const MOST_STRENGTH: i32 = 140;
/// Form is reckoned from condition and freshness between these two, in a
/// third, around a mood the game starts every player at.
const FORM_RANGE: (i32, i32) = (20, 80);
pub const NEUTRAL_MOOD: i32 = 10;
/// The game's morale of a player, from which his strength counts fully.
pub const FULL_MORALE: i32 = 100;
const MORALE_TOP: i32 = 110;
/// Whether a nation is of the European Union, by nation code.
/// The kinds a hanging striker's spot suits on the list view's small pitch
/// (10 where it does), and the list view's question whether its formation
/// keeps the eleven's spots, which has no quotes round the name.
const HANGING_AT: u32 = 0x8a50ac;
const HANGING: i16 = 10;
const KEEP_QUESTION_AT: u32 = 0x884990;
const KEEP_QUESTION_LENGTH: usize = 0x80;
const EUROPEAN_AT: u32 = 0x89e0b8;
const NATIONS: usize = 0x80;

/// The game's tables for the pitch.
pub struct Tables {
    /// Each formation's name and the spots of its eleven.
    pub formations: Vec<Formation>,
    /// The bonuses of [`BONUSES_AT`], by bit.
    bonuses: [Vec<[i16; 3]>; 4],
    areas: Vec<[u8; 5]>,
    area_kinds: Vec<u8>,
    across: Vec<i16>,
    down: Vec<i16>,
    kinds: Vec<u8>,
    /// Text ids of the positions' names and of the kinds' abbreviations.
    pub position_names: Vec<u32>,
    pub abbreviations: Vec<u32>,
    fit: Vec<i16>,
    parts: Vec<[u8; PARTS]>,
    ability_icons: Vec<u16>,
    /// The co-trainer's free system: how many of each kind he puts up at
    /// most, and the spots for so many defenders, midfielders and strikers.
    pub co_trainer: crate::autolineup::CoTrainerTables,
    /// The nations of the European Union, whose players are no foreigners.
    pub european: Vec<bool>,
    hanging: Vec<bool>,
    pub keep_question: String,
}

impl Tables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let across = exe.u16s(ACROSS_AT, ACROSS)?.into_iter().map(|v| v as i16).collect::<Vec<_>>();
        let down = exe.u16s(DOWN_AT, DOWN)?.into_iter().map(|v| v as i16).collect::<Vec<_>>();
        let line = |grid: &[i16], i: u16| grid.get(usize::from(i)).copied().unwrap_or(0);
        let mut formations = Vec::with_capacity(FORMATIONS);
        for f in 0..FORMATIONS as u32 {
            let at = FORMATIONS_AT + f * FORMATION_SIZE;
            let name = exe.bytes(at, FORMATION_NAME)?;
            let name = a3_files::sav::cp1252(name.split(|&b| b == 0).next().unwrap_or_default());
            let lines = exe.u16s(at + FORMATION_NAME as u32, 2 * FIELD)?;
            let spots = std::array::from_fn(|i| {
                (line(&across, lines[2 * i]), line(&down, lines[2 * i + 1]))
            });
            formations.push((name, spots));
        }
        let areas = exe.bytes(AREAS_AT, 5 * AREAS)?;
        let parts = exe.bytes(PARTS_AT, PART_ROW * DOWN)?;
        let mut bonuses: [Vec<[i16; 3]>; 4] = Default::default();
        for (table, &at) in bonuses.iter_mut().zip(&BONUSES_AT) {
            let values = exe.u16s(at, 3 * BONUS_BITS)?;
            *table = values.chunks(3).map(|v| [v[0] as i16, v[1] as i16, v[2] as i16]).collect();
        }
        Ok(Self {
            formations,
            bonuses,
            areas: areas.chunks(5).map(|a| [a[0], a[1], a[2], a[3], a[4]]).collect(),
            area_kinds: exe.bytes(AREA_KINDS_AT, AREAS)?.to_vec(),
            across,
            down,
            kinds: exe.bytes(KINDS_AT, POSITIONS)?.to_vec(),
            position_names: exe.u32s(POSITION_NAMES_AT, POSITIONS)?,
            abbreviations: exe.u32s(ABBREVIATIONS_AT, POSITIONS)?,
            fit: exe.u16s(FIT_AT, KINDS * KINDS)?.into_iter().map(|v| v as i16).collect(),
            parts: parts.chunks(PART_ROW).map(|r| [r[0], r[1], r[2], r[3]]).collect(),
            ability_icons: exe
                .u32s(ABILITY_ICONS_AT, ABILITY_ICONS + DUTY_ICONS)?
                .into_iter()
                .map(|id| id as u16)
                .collect(),
            co_trainer: crate::autolineup::CoTrainerTables::load(exe)?,
            european: exe.bytes(EUROPEAN_AT, NATIONS)?.iter().map(|&b| b != 0).collect(),
            hanging: exe
                .u16s(HANGING_AT, KINDS)?
                .into_iter()
                .map(|v| v as i16 == HANGING)
                .collect(),
            keep_question: {
                let text = exe.bytes(KEEP_QUESTION_AT, KEEP_QUESTION_LENGTH)?;
                a3_files::sav::cp1252(text.split(|&b| b == 0).next().unwrap_or_default())
            },
        })
    }

    /// The graphics of a player's abilities, the good ones first, three at
    /// most.
    pub fn ability_icons(&self, player: &Player) -> Vec<u16> {
        let keeper = player.positions[0] == GOALKEEPER;
        let (count, first) =
            if keeper { (KEEPER_ABILITIES, 2 * FIELD_ABILITIES) } else { (FIELD_ABILITIES, 0) };
        let [good, bad] = player.abilities;
        let mut icons = Vec::new();
        for (bits, start) in [(good, first), (bad, first + count)] {
            for bit in 0..count {
                if bits & (1 << bit) != 0 && icons.len() < 3 {
                    icons.extend(self.ability_icons.get(start + bit));
                }
            }
        }
        icons
    }

    /// The graphic of a duty, as a player's box and dialog 418 show it.
    pub fn duty_icon(&self, duty: crate::duties::Duty) -> Option<u16> {
        self.ability_icons.get(ABILITY_ICONS + duty.on_pitch()).copied()
    }

    /// The area of the pitch a spot lies in; the goal's when none.
    fn area(&self, (x, y): (i16, i16)) -> Option<usize> {
        let line = |grid: &[i16], i: u8| grid.get(usize::from(i)).copied().unwrap_or(0);
        self.areas.iter().position(|&[x0, y0, x1, y1, _]| {
            (line(&self.across, x0)..=line(&self.across, x1) + AREA_REACH.0).contains(&x)
                && (line(&self.down, y0)..=line(&self.down, y1) + AREA_REACH.1).contains(&y)
        })
    }

    /// The position a spot makes.
    pub fn position(&self, spot: (i16, i16)) -> u8 {
        self.area(spot).map_or(GOALKEEPER, |a| self.areas[a][4])
    }

    /// The kind of position a spot makes.
    fn kind_at(&self, spot: (i16, i16)) -> u8 {
        self.area(spot).map_or(GOALKEEPER, |a| self.area_kinds[a])
    }

    /// The kind of position a position is (the table at 0x89e23c).
    pub fn kind(&self, position: u8) -> u8 {
        self.kinds.get(usize::from(position)).copied().unwrap_or(GOALKEEPER)
    }

    /// Whether a kind of position suits a hanging striker's spot on the
    /// list view's small pitch: the wide midfielders, the attacking one
    /// and the striker.
    pub fn hanging(&self, kind: u8) -> bool {
        self.hanging.get(usize::from(kind)).copied().unwrap_or(false)
    }

    /// How much of his strength a player brings on a position, in tenths.
    pub fn fit(&self, player: &Player, position: u8) -> i32 {
        let kind = self.kind(position);
        let [main, second, third] = player.positions;
        if main == kind {
            FULL_FIT
        } else if (main == GOALKEEPER) != (kind == GOALKEEPER) {
            GOAL_FIT
        } else if kind != 0 && (second == kind || third == kind) {
            SIDE_FIT
        } else {
            let at = usize::from(main) * KINDS + usize::from(kind);
            self.fit.get(at).map_or(GOAL_FIT, |&f| i32::from(f))
        }
    }

    /// What a player counts for on a position, in tenths of strength: his
    /// strength and form, as far as he fits there and his morale lets him.
    pub fn strength_at(&self, player: &Player, fitness: Fitness, position: u8) -> i32 {
        let base = (i32::from(player.strength) - 1) * 10 + form(fitness);
        let mut value = base * self.fit(player, position) / 10;
        let morale = fitness.morale;
        value =
            if morale <= MORALE_TOP { value * morale } else { value * (2 * MORALE_TOP - morale) };
        value = (value / 100).min(MOST_STRENGTH);
        if fitness.knock {
            value = value * 9 / 10;
        }
        value.max(0)
    }

    /// What a player counts for wherever he plays, in tenths of strength
    /// (`FUN_006a115b`): his strength and form as far as his morale lets
    /// him, a doping's lift, no knock and no position.
    pub fn strength(&self, player: &Player, fitness: Fitness, doping: u8) -> i32 {
        let base = (i32::from(player.strength) - 1) * 10 + form(fitness);
        let morale = fitness.morale;
        let mut value = if morale <= MORALE_TOP {
            base * morale / 100
        } else {
            base * (2 * MORALE_TOP - morale) / 100
        };
        // A doping of the third stage.
        if doping & 3 != 0 && (doping >> 2) & 3 == 3 {
            value += if doping & 2 != 0 { 4 } else { 2 };
        }
        value.clamp(0, MOST_STRENGTH)
    }

    /// The strongest eleven of a squad on the spots of a formation, the
    /// strongest of the rest on the bench: each spot takes the best player
    /// left whose main position is its kind, and stays empty when there is
    /// none.
    pub fn line_up(&self, players: &[Player], formation: usize) -> Lineup {
        let spots = self.formations.get(formation).map_or([(0, 0); FIELD], |f| f.1);
        let mut by_kind: Vec<Vec<usize>> = vec![Vec::new(); KINDS];
        for (i, player) in players.iter().enumerate() {
            if let Some(group) = by_kind.get_mut(usize::from(player.positions[0])) {
                group.push(i);
            }
        }
        for group in &mut by_kind {
            group.sort_by_key(|&i| std::cmp::Reverse(players[i].strength));
            group.reverse();
        }
        let mut field = [None; FIELD];
        for (slot, &spot) in field.iter_mut().zip(&spots) {
            *slot = by_kind.get_mut(usize::from(self.kind_at(spot))).and_then(Vec::pop);
        }
        let mut bench = [None; BENCH];
        for slot in &mut bench {
            // The strongest still left of any kind; the first kind on a tie.
            let best = (1..KINDS)
                .filter_map(|k| Some((k, players[*by_kind[k].last()?].strength)))
                .fold(None, |best: Option<(usize, u8)>, (k, s)| match best {
                    Some((_, b)) if b >= s => best,
                    _ => Some((k, s)),
                });
            *slot = best.and_then(|(k, _)| by_kind[k].pop());
        }
        Lineup { spots, field, bench }
    }

    /// What a player's abilities and character add to his rank, in one of
    /// three columns (`FUN_006a1642`): a keeper's own abilities, or a field
    /// player's, then both words of his character.
    pub fn bonus(&self, player: &Player, column: usize) -> i32 {
        let [field, keeper, first, second] = &self.bonuses;
        let add = |table: &[[i16; 3]], bits: u16, count: usize| -> i32 {
            (0..count).filter(|&b| bits & (1 << b) != 0).map(|b| i32::from(table[b][column])).sum()
        };
        // The data files' character, which the record keeps halved.
        let [a, b] = player.traits.map(|t| (t >> 1) as u16);
        let abilities = if player.positions[0] == GOALKEEPER {
            add(keeper, player.abilities[0], KEEPER_BITS)
        } else {
            add(field, player.abilities[0], BONUS_BITS)
        };
        abilities + add(first, a, BONUS_BITS) + add(second, b, BONUS_BITS)
    }

    /// How "Auto." ranks a player for a position: his strength there and
    /// what his abilities and character add (`FUN_006a181e`).
    pub fn rank(&self, player: &Player, fitness: Fitness, position: u8) -> i32 {
        self.strength_at(player, fitness, position) + self.bonus(player, 0)
    }

    /// The kind of position the area under a spot makes (`FUN_00696cc5`).
    pub fn kind_under(&self, spot: (i16, i16)) -> u8 {
        self.kind_at(spot)
    }

    /// What the line-up counts for in goal, defence, midfield and attack,
    /// in tenths of strength.
    pub fn parts(&self, lineup: &Lineup, players: &[Player], fitness: &[Fitness]) -> [i32; PARTS] {
        let mut parts = [0; PARTS];
        for (spot, player) in lineup.on_field() {
            let Some(p) = players.get(player) else { continue };
            let value = self.strength_at(p, fitness_of(fitness, player), self.kind_at(spot));
            // The band of the grid the spot's height lies in: before the
            // first line below it, the last band past the last line (the
            // lookup `FUN_00696b76` builds, `FUN_006a18c2` reads).
            let row = self.down.iter().skip(1).position(|&y| spot.1 < y).unwrap_or(DOWN - 1);
            let weights = self.parts.get(row).copied().unwrap_or_default();
            for (part, &weight) in parts.iter_mut().zip(&weights) {
                *part += value * i32::from(weight);
            }
        }
        parts.map(|p| p / 100)
    }
}

/// A formation: its name and the spots of its eleven, in pitch pixels.
pub type Formation = (String, [(i16, i16); FIELD]);

/// Every manager has this many formations to choose from: the game's own,
/// and the rest like its first, without a name.
pub const FORMATIONS_OF_A_MANAGER: usize = 9;

impl Tables {
    /// A new manager's formations.
    pub fn new_formations(&self) -> Vec<Formation> {
        let mut formations = self.formations.clone();
        let first = formations.first().map(|f| (String::new(), f.1));
        while formations.len() < FORMATIONS_OF_A_MANAGER {
            formations.extend(first.clone());
        }
        formations
    }
}

/// A place on the line-up screen a player is dragged from or let go of on:
/// a spot of the eleven, a place on the bench, or — in the squad's list — the
/// player himself.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    Field(usize),
    Bench(usize),
    Player(usize),
}

/// Where a club's players stand.
#[derive(Clone, PartialEq)]
pub struct Lineup {
    /// The spots of the eleven on the pitch, in pitch pixels.
    pub spots: [(i16, i16); FIELD],
    /// Who stands on each spot and sits on each place of the bench, by
    /// index into the club's players.
    pub field: [Option<usize>; FIELD],
    pub bench: [Option<usize>; BENCH],
}

impl Lineup {
    /// A player dragged from one place and let go of on another, or off the
    /// boxes, as the line-up screen moves him (`FUN_0050de6f`): two boxes
    /// swap their players — on the pitch, on the bench or one of each; a
    /// player of the list takes a box, leaving his own place and sending
    /// the box's player back to the list; a player let go of off the boxes,
    /// or on the list, leaves the line-up. An empty place on the bench
    /// cannot be picked up; an empty spot on the pitch can, and the player
    /// it is let go of on moves into it. Whether anything changed.
    pub fn drop(&mut self, from: Place, to: Option<Place>) -> bool {
        let before = (self.field, self.bench);
        match (from, to) {
            (Place::Bench(b), _) if self.bench.get(b).is_none_or(Option::is_none) => {}
            (Place::Player(p), Some(to @ (Place::Field(_) | Place::Bench(_)))) => {
                self.remove(p);
                if let Some(place) = self.place_mut(to) {
                    *place = Some(p);
                }
            }
            (
                from @ (Place::Field(_) | Place::Bench(_)),
                Some(to @ (Place::Field(_) | Place::Bench(_))),
            ) => {
                let (Some(a), Some(b)) = (self.get(from), self.get(to)) else { return false };
                *self.place_mut(from).expect("a place of the line-up") = b;
                *self.place_mut(to).expect("a place of the line-up") = a;
            }
            (from @ (Place::Field(_) | Place::Bench(_)), _) => {
                if let Some(place) = self.place_mut(from) {
                    *place = None;
                }
            }
            (Place::Player(_), _) => {}
        }
        before != (self.field, self.bench)
    }

    /// Takes a player out of the eleven and off the bench.
    pub fn remove(&mut self, player: usize) {
        for place in self.field.iter_mut().chain(&mut self.bench) {
            if *place == Some(player) {
                *place = None;
            }
        }
    }

    fn get(&self, place: Place) -> Option<Option<usize>> {
        match place {
            Place::Field(i) => self.field.get(i).copied(),
            Place::Bench(i) => self.bench.get(i).copied(),
            Place::Player(_) => None,
        }
    }

    fn place_mut(&mut self, place: Place) -> Option<&mut Option<usize>> {
        match place {
            Place::Field(i) => self.field.get_mut(i),
            Place::Bench(i) => self.bench.get_mut(i),
            Place::Player(_) => None,
        }
    }

    /// The players on the pitch with their spots.
    pub fn on_field(&self) -> impl Iterator<Item = ((i16, i16), usize)> + '_ {
        self.spots.iter().zip(&self.field).filter_map(|(&spot, &p)| Some((spot, p?)))
    }

    /// Everyone in the squad for the next match: the eleven, then the bench.
    pub fn squad(&self) -> impl Iterator<Item = usize> + '_ {
        self.field.iter().chain(&self.bench).flatten().copied()
    }

    pub fn on_bench(&self, player: usize) -> bool {
        self.bench.contains(&Some(player))
    }

    pub fn playing(&self, player: usize) -> bool {
        self.field.contains(&Some(player))
    }
}

/// A player's form, from 0 to 20: how fit and fresh he is, around his mood.
pub fn form(fitness: Fitness) -> i32 {
    let (low, high) = FORM_RANGE;
    let fit = ((condition(fitness) + freshness(fitness)) / 2).clamp(low, high);
    ((fit - low) / 3 + fitness.mood) / 2
}

/// His condition as a list shows it ("Ko", `FUN_00457e00`): a smoker's
/// counts five less.
pub fn condition(fitness: Fitness) -> i32 {
    let condition = fitness.condition / 100;
    if fitness.smoker { (condition - 5).max(0) } else { condition }
}

/// His freshness as a list shows it ("Fr", `FUN_006ff649`), 0 to 100.
pub fn freshness(fitness: Fitness) -> i32 {
    (fitness.freshness.iter().sum::<i32>() / 100).clamp(0, 100)
}

pub fn fitness_of(fitness: &[Fitness], player: usize) -> Fitness {
    fitness.get(player).copied().unwrap_or(Fitness::new(5000, 5000))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use a3_files::exe::Exe;

    use crate::season::{Fitness, Rules};
    use crate::world::World;

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_new_manager_lines_up_the_strongest_in_a_5_3_2() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let rules = Rules::load(&Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let world = World::load(&dir.join("Data.a3")).unwrap();
        let pitch = &rules.pitch;
        assert_eq!(pitch.formations[0].0, "5-3-2");
        let germany = world.country(0).unwrap();
        let club = germany.clubs.iter().find(|c| c.name == "FC Bayern München").unwrap();
        let lineup = pitch.line_up(&club.players, 0);
        // The goalkeeper's spot, at the bottom, takes the strongest keeper.
        let keeper = lineup.field[0].unwrap();
        let keepers = club.players.iter().filter(|p| p.positions[0] == 1);
        assert_eq!(club.players[keeper].strength, keepers.map(|p| p.strength).max().unwrap());
        // Every player stands on his main position, and nobody twice.
        for (spot, p) in lineup.on_field() {
            assert_eq!(pitch.kind_at(spot), club.players[p].positions[0]);
        }
        let squad: Vec<usize> = lineup.squad().collect();
        let mut unique = squad.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), squad.len());
        assert!(lineup.bench.iter().all(Option::is_some));
        // On his main position a player brings his whole strength.
        let fresh = Fitness::new(5000, 5000);
        let p = &club.players[keeper];
        assert_eq!(pitch.fit(p, 1), 10);
        let strength = (i32::from(p.strength) - 1) * 10 + super::form(fresh);
        assert_eq!(pitch.strength_at(p, fresh, 1), strength.min(140));
    }

    #[test]
    fn form_follows_condition_and_freshness_around_the_mood() {
        let at = |condition, fresh| super::form(Fitness::new(condition, fresh));
        assert_eq!(at(0, 0), 5);
        assert_eq!(at(10_000, 10_000), 15);
        assert_eq!(at(4000, 7000), 10);
    }
}
