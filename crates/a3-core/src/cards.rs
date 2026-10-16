//! Yellow cards: whether one more suspends a player, by the rules of his
//! club's country for its league and its cup, and in the European cups
//! (`FUN_0074612e`, `FUN_007464a4`, `FUN_00746611`); and before the next
//! match, by its kind (`FUN_0062fc1a`; TAKTIK.md in the RE repo).

use crate::records::{Player, Records};

/// The country whose cup counts the league's cards.
const CUP_COUNTS_LEAGUE: u16 = 0x10;

/// The kinds of a club's next match (team +0x258): the league's, the cup's
/// and the European cups'.
pub const LEAGUE_MATCHES: [u8; 2] = [2, 0xb];
pub const CUP_MATCHES: [u8; 2] = [3, 4];
pub const EUROPEAN_MATCHES: std::ops::RangeInclusive<u8> = 5..=10;

/// In the league of a club of `country` and `level`.
pub fn league(records: &Records, country: u16, level: u8, player: &Player) -> bool {
    let [rule, _] = records.countries.card_rules(usize::from(country));
    let cards = player.league_cards()[0];
    match rule {
        0 => cards % 5 == 4,
        1 => cards & 3 == 3,
        2 => cards == 3 || cards >= 6 && cards.is_multiple_of(3),
        3 => cards == 3 || cards == 6 || cards >= 8,
        4 => level < 2 && cards % 5 == 4,
        5 => cards % 7 == 6,
        6 => cards == 4 || cards >= 6 && cards.is_multiple_of(4),
        _ => cards % 5 == 4,
    }
}

/// In the cup of `country`.
pub fn cup(records: &Records, country: u16, player: &Player) -> bool {
    let [_, rule] = records.countries.card_rules(usize::from(country));
    let cards =
        if country == CUP_COUNTS_LEAGUE { player.league_cards()[0] } else { player.cup_cards()[0] };
    match rule {
        1 => cards & 3 == 3,
        2 => cards == 3 || cards >= 6 && cards.is_multiple_of(3),
        3 => cards == 3 || cards == 6 || cards >= 8,
        5 => cards != 0,
        _ => cards % 5 == 4,
    }
}

/// In the European cups.
pub fn europe(player: &Player) -> bool {
    let cards = player.europe_cards()[0];
    cards == 2 || cards >= 4 && cards.is_multiple_of(2)
}

/// Whether one more yellow card in the next match, of `kind`, suspends a
/// player of a club of `country` and `level`: the line-up's warning, and
/// whom the tactics offer to collect one. In the European cups the game asks
/// for more than one card so far, not its own rule.
pub fn before_match(records: &Records, country: u16, level: u8, kind: u8, player: &Player) -> bool {
    if EUROPEAN_MATCHES.contains(&kind) {
        player.europe_cards()[0] > 1
    } else if LEAGUE_MATCHES.contains(&kind) {
        league(records, country, level, player)
    } else if CUP_MATCHES.contains(&kind) {
        cup(records, country, player)
    } else {
        false
    }
}
