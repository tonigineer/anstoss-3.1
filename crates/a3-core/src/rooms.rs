//! The training camp's rooms (`CZimmerbelegung`, dialog 407; TRAINING-LANG.md
//! §5.1 in the RE repo): twenty rooms of two beds, each player's room kept
//! in his record, and the colour a room's pair shows in.
//!
//! The dialog also has a place of Bianca's own, beside her picture: a
//! player let go of on the picture swaps places with her, so that she
//! shows in his room. The original prepares all of it — her name (text
//! 4695), the place's number, the hook OK calls for a room she is in
//! (`FUN_005d87ef`, empty) — but reads her picture's size as its place,
//! so it only takes a drop in the window's top left corner, and keeps no
//! place for her: the swap there overwrites the pointer to the first
//! room's name. Here she has her place, and her picture takes the drop.

use crate::records::Records;

/// How many rooms, of two beds each.
pub const ROOMS: usize = 20;
/// The places: two a room, room by room, then Bianca's beside her
/// picture.
pub const PLACES: usize = 2 * ROOMS + 1;
pub const BIANCA_PLACE: usize = 2 * ROOMS;
/// Who a place holds besides a player's id: nobody, or Bianca.
pub const NOBODY: u16 = 0;
pub const BIANCA: u16 = 0x7fff;
/// From this id on the players are the managers, who get no room.
const MANAGERS_FROM: u16 = 17001;

/// A room pair's colours, as Windows colours: the first's best friend,
/// the one he hates, and a pair that does not get on.
pub const FRIENDS: u32 = 0xff0000;
pub const ENEMIES: u32 = 0x0000ff;
pub const AT_ODDS: u32 = 0x8000ff;
/// R+0x1b4 bit 12: he holds a grudge against the trainer.
const GRUDGE: u32 = 0x1000;

/// The places as the dialog opens (`FUN_005d9517`): each of the squad, in
/// its order, in the room his record keeps while it has a bed left; the
/// rest — without a room, or in a full one — each in the first room with
/// a bed, which his record keeps at once. Bianca stands at her picture.
/// A room past the twentieth counts as none (the original would read
/// past its counts).
pub fn seat(records: &mut Records, squad: &[u16]) -> [u16; PLACES] {
    let mut places = [NOBODY; PLACES];
    places[BIANCA_PLACE] = BIANCA;
    let mut beds = [0usize; ROOMS];
    let mut waiting = Vec::new();
    for &id in squad.iter().filter(|&&id| id < MANAGERS_FROM) {
        let room = records.player(id).map_or(-1, |p| p.room());
        match usize::try_from(room).ok().filter(|&r| r < ROOMS) {
            Some(r) if beds[r] < 2 => {
                places[2 * r + beds[r]] = id;
                beds[r] += 1;
            }
            _ => waiting.push(id),
        }
    }
    for id in waiting {
        let Some(r) = (0..ROOMS).find(|&r| beds[r] < 2) else { continue };
        places[2 * r + beds[r]] = id;
        beds[r] += 1;
        if let Some(p) = records.players.get_mut(usize::from(id)) {
            p.set_room(r as i8);
        }
    }
    places
}

/// Whether a place holds a player, whom the dialog can drag and show.
pub fn is_player(id: u16) -> bool {
    id != NOBODY && id != BIANCA
}

/// OK (`FUN_005d97ec`): every player in a room keeps it in his record;
/// whoever stands at Bianca's place keeps the room he had. For a room she
/// is in the original calls `FUN_005d87ef`, which does nothing.
pub fn keep(records: &mut Records, places: &[u16; PLACES]) {
    for (place, &id) in places[..2 * ROOMS].iter().enumerate() {
        if is_player(id)
            && let Some(p) = records.players.get_mut(usize::from(id))
        {
            p.set_room((place / 2) as i8);
        }
    }
}

/// The colour a room's pair shows in (`FUN_005d9f20`), a Windows colour,
/// 0 for the plain black: none unless both are players; blue where the
/// first's best friend is the second, red where the first hates him;
/// otherwise violet where the first dislikes him, where both come from one
/// nation whose language is not the club country's, or where either holds
/// a grudge against the trainer.
pub fn pair_colour(records: &Records, pair: [u16; 2], country: u8, languages: &[u8]) -> u32 {
    let [a, b] = pair;
    if !is_player(a) || !is_player(b) {
        return 0;
    }
    let (Some(first), Some(second)) = (records.player(a), records.player(b)) else { return 0 };
    if first.best_friend() == b {
        return FRIENDS;
    }
    if first.hates() == b {
        return ENEMIES;
    }
    let language = |nation: u8| languages.get(usize::from(nation)).copied();
    let foreign = first.nation() == second.nation()
        && language(first.nation()) != language(country);
    let grudge = (first.shown() | second.shown()) & GRUDGE != 0;
    if first.dislikes().contains(&b) || foreign || grudge { AT_ODDS } else { 0 }
}
