//! What the comparison with the opponent sets (dialog 78,
//! `CGegenueberstellungMann`; GEGENUEBERSTELLUNG.md in the RE repo): which
//! of the eleven mark which of the opponent's players man for man — the
//! defenders its strikers, the midfielders its midfielders —, where each of
//! the eleven acts, and who goes forward at set pieces.

use crate::records::Team;

/// How many marking pairs a club keeps.
pub const PAIRS: usize = 10;
/// The kinds of position under a spot (`FUN_00696cc5`) the marking knows:
/// the defenders, the midfielders, and the striker.
const DEFENDERS: std::ops::RangeInclusive<u8> = 2..=5;
const MIDFIELDERS: std::ops::RangeInclusive<u8> = 6..=9;
pub const STRIKER: u8 = 10;
/// The midfielders' kind a marker cannot be dropped on, though a pair with
/// it stands.
const NOT_DROPPED_ON: u8 = 8;

/// The pairs: a player of the eleven and the opponent's player he marks,
/// by their ids; 0 a free pair.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Marking {
    pub markers: [u16; PAIRS],
    pub marked: [u16; PAIRS],
}

impl Marking {
    pub fn of(team: &Team) -> Self {
        Self { markers: team.markers(), marked: team.marked() }
    }

    pub fn write(&self, team: &mut Team) {
        team.set_markers(self.markers);
        team.set_marked(self.marked);
    }

    /// A player marks another: in his pair, else in the first free one
    /// (`FUN_00698357`).
    pub fn set(&mut self, marker: u16, marked: u16) {
        if marker == 0 || marked == 0 {
            return;
        }
        let pair = self.markers.iter().position(|&m| m == marker);
        let pair = pair.or_else(|| self.markers.iter().position(|&m| m == 0));
        if let Some(i) = pair {
            self.markers[i] = marker;
            self.marked[i] = marked;
        }
    }

    /// A player marks nobody (`FUN_00698404`).
    pub fn remove(&mut self, marker: u16) {
        for i in 0..PAIRS {
            if self.markers[i] == marker {
                self.markers[i] = 0;
                self.marked[i] = 0;
            }
        }
    }

    /// Whether a player marks another (`FUN_00698458`).
    pub fn has(&self, marker: u16, marked: u16) -> bool {
        marker != 0 && marked != 0 && self.pairs().any(|pair| pair == (marker, marked))
    }

    /// Whom a player marks (`FUN_006984c3`).
    pub fn marked_by(&self, marker: u16) -> Option<u16> {
        if marker == 0 {
            return None;
        }
        self.pairs().find(|&(m, _)| m == marker).map(|(_, marked)| marked)
    }

    fn pairs(&self) -> impl Iterator<Item = (u16, u16)> + '_ {
        self.markers.iter().copied().zip(self.marked.iter().copied())
    }

    /// The pairs that stand, as the comparison draws them, by the places
    /// of both elevens; the others it drops (`FUN_004af10f`): a marker not
    /// in the eleven, a marked player not in the opponent's, kinds of
    /// position that do not go together. The elevens are the players' ids
    /// and the kinds under their spots.
    pub fn standing(&mut self, ours: &[(u16, u8)], theirs: &[(u16, u8)]) -> Vec<(usize, usize)> {
        let mut lines = Vec::new();
        for i in 0..PAIRS {
            let (marker, marked) = (self.markers[i], self.marked[i]);
            if marker == 0 {
                continue;
            }
            let own = ours.iter().position(|&(id, _)| id == marker);
            let their = theirs.iter().position(|&(id, _)| id == marked);
            match (own, their) {
                (Some(o), Some(t)) if may_mark(ours[o].1, theirs[t].1) => lines.push((o, t)),
                _ => self.remove(marker),
            }
        }
        lines
    }
}

/// Whether a kind of position is a defender's or a midfielder's
/// (`FUN_00677c87`, `FUN_00677ccf`).
pub fn defender(kind: u8) -> bool {
    DEFENDERS.contains(&kind)
}

pub fn midfielder(kind: u8) -> bool {
    MIDFIELDERS.contains(&kind)
}

/// Whether a player on one kind of position may mark one on another: a
/// defender a striker, a midfielder a midfielder (`FUN_00696fbe`).
pub fn may_mark(own: u8, theirs: u8) -> bool {
    if defender(own) { theirs == STRIKER } else { midfielder(own) && midfielder(theirs) }
}

/// Whether a marker may be dropped on a player of the opponent's: of the
/// kinds he may mark, but a midfielder not on the kind 8.
pub fn drops_on(own: u8, theirs: u8) -> bool {
    if defender(own) {
        theirs == STRIKER
    } else {
        midfielder(own) && midfielder(theirs) && theirs != NOT_DROPPED_ON
    }
}

/// Where the eleven act: left, up, right and down from a spot, in pitch
/// pixels.
pub type Area = [u8; 4];
/// The keeper's, which nobody changes, and the bounds of the others'
/// reach across and up and down (`FUN_006980a0`).
const KEEPER_AREA: Area = [0x31, 0xc, 0x31, 0xc];
const ACROSS: (u8, u8) = (0x45, 0x7d);
const ALONG: (u8, u8) = (0x20, 0x7d);

/// The areas as the comparison opens them: the keeper's his, the others'
/// within their bounds (`FUN_006981be`).
pub fn normalise(areas: &mut [Area; 11]) {
    areas[0] = KEEPER_AREA;
    for area in &mut areas[1..] {
        let [left, up, right, down] = area;
        for side in [left, right] {
            *side = (*side).clamp(ACROSS.0, ACROSS.1);
        }
        for side in [up, down] {
            *side = (*side).clamp(ALONG.0, ALONG.1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_are_set_kept_and_dropped_as_the_original_does() {
        let mut m = Marking::default();
        m.set(7, 70);
        m.set(8, 80);
        m.set(7, 71);
        assert_eq!((m.markers[..2].to_vec(), m.marked[..2].to_vec()), (vec![7, 8], vec![71, 80]));
        assert!(m.has(7, 71) && !m.has(7, 70));
        assert_eq!(m.marked_by(8), Some(80));
        // A defender on a striker stands; a midfielder on a striker goes,
        // and so does a marker who left the eleven.
        m.set(9, 90);
        let ours = [(7, 3), (8, 6)];
        let theirs = [(71, STRIKER), (80, STRIKER)];
        assert_eq!(m.standing(&ours, &theirs), vec![(0, 0)]);
        assert_eq!(m.markers[..3], [7, 0, 0]);
    }

    #[test]
    fn areas_keep_within_their_bounds() {
        let mut areas = [[0, 0xff, 0x50, 0x30]; 11];
        normalise(&mut areas);
        assert_eq!(areas[0], KEEPER_AREA);
        assert_eq!(areas[1], [0x45, 0x7d, 0x50, 0x30]);
    }
}
