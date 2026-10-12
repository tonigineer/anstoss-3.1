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
/// good at, then bad at, nine a goalkeeper can be good at, then bad at.
const ABILITY_ICONS_AT: u32 = 0x89f918;
const ABILITY_ICONS: usize = 50;
const FIELD_ABILITIES: usize = 16;
const KEEPER_ABILITIES: usize = 9;
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
const NEUTRAL_MOOD: i32 = 10;
/// The game's morale of a player, from which his strength counts fully.
pub const FULL_MORALE: i32 = 100;
const MORALE_TOP: i32 = 110;
/// How well a team plays together: in a new game every player has been at
/// his club a year, which gives a base; a player on his main position adds,
/// one on a foreign position takes away. At most 200.
const COHESION_BASE: i32 = 20;
const COHESION_STEP: i32 = 2;
const COHESION_MOST: i32 = 200;

/// The game's tables for the pitch.
pub struct Tables {
    /// Each formation's name and the spots of its eleven.
    pub formations: Vec<(String, [(i16, i16); FIELD])>,
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
        Ok(Self {
            formations,
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
                .u32s(ABILITY_ICONS_AT, ABILITY_ICONS)?
                .into_iter()
                .map(|id| id as u16)
                .collect(),
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

    fn kind(&self, position: u8) -> u8 {
        self.kinds.get(usize::from(position)).copied().unwrap_or(GOALKEEPER)
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
        let morale = FULL_MORALE;
        value =
            if morale <= MORALE_TOP { value * morale } else { value * (2 * MORALE_TOP - morale) };
        (value / 100).clamp(0, MOST_STRENGTH)
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

    /// What the line-up counts for in goal, defence, midfield and attack,
    /// in tenths of strength.
    pub fn parts(&self, lineup: &Lineup, players: &[Player], fitness: &[Fitness]) -> [i32; PARTS] {
        let mut parts = [0; PARTS];
        for (spot, player) in lineup.on_field() {
            let Some(p) = players.get(player) else { continue };
            let value = self.strength_at(p, fitness_of(fitness, player), self.kind_at(spot));
            let row = self.down.iter().position(|&y| y >= spot.1).unwrap_or(DOWN - 1);
            let weights = self.parts.get(row).copied().unwrap_or_default();
            for (part, &weight) in parts.iter_mut().zip(&weights) {
                *part += value * i32::from(weight);
            }
        }
        parts.map(|p| p / 100)
    }

    /// How well the line-up plays together, from 0 to 200.
    pub fn cohesion(&self, lineup: &Lineup, players: &[Player]) -> i32 {
        let mut cohesion = COHESION_BASE;
        for (spot, player) in lineup.on_field() {
            let Some(p) = players.get(player) else { continue };
            let kind = self.kind_at(spot);
            if p.positions[0] == kind {
                cohesion += COHESION_STEP;
            } else if p.positions[1] != kind && p.positions[2] != kind {
                cohesion -= COHESION_STEP;
            }
        }
        cohesion.clamp(0, COHESION_MOST)
    }
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
    let condition = fitness.condition / 100;
    let fresh = (fitness.freshness.iter().sum::<i32>() / 100).clamp(0, 100);
    let (low, high) = FORM_RANGE;
    let fit = ((condition + fresh) / 2).clamp(low, high);
    ((fit - low) / 3 + NEUTRAL_MOOD) / 2
}

pub fn fitness_of(fitness: &[Fitness], player: usize) -> Fitness {
    fitness.get(player).copied().unwrap_or(Fitness { condition: 5000, freshness: [5000, 0, 0] })
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
        let fresh = Fitness { condition: 5000, freshness: [5000, 0, 0] };
        let p = &club.players[keeper];
        assert_eq!(pitch.fit(p, 1), 10);
        let strength = (i32::from(p.strength) - 1) * 10 + super::form(fresh);
        assert_eq!(pitch.strength_at(p, fresh, 1), strength.min(140));
    }

    #[test]
    fn form_follows_condition_and_freshness_around_the_mood() {
        let at = |condition, fresh| super::form(Fitness { condition, freshness: [fresh, 0, 0] });
        assert_eq!(at(0, 0), 5);
        assert_eq!(at(10_000, 10_000), 15);
        assert_eq!(at(4000, 7000), 10);
    }
}
