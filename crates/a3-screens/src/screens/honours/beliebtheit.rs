//! How popular the manager is (dialog 26 `CBeliebt`; ERFOLGE.md §B.3):
//! the public, the fans, the media and his players each rate him, by a
//! percent, a rank's word and a picture; and below, what the public and
//! the association think of his club.

use std::collections::HashMap;

use a3_core::records::{Manager, Player, Records, Team};
use a3_files::rch::Dialog;

use crate::controls::bitmap::Picture;
use crate::game::Game;
use crate::pointer::Target;
use crate::screens::hub;
use crate::screens::kalkulation::own_manager;
use crate::screens::squad::own_team;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, caption, control, mark_inert, set};
use crate::view::View;

use super::abilities::standing;

pub(crate) const BELIEBT_DIALOG: u16 = 26;
const CAPTION_ID: u16 = 9947;
/// `IDC_BELIEBTHEIT_OK`, the big bitmap button closing the page
/// (`OnClicked_IDC_BELIEBTHEIT_OK` at 0x47f300); its pictures 1004/1005
/// stand in the file, and it is not one of the id-1 OKs the game
/// redresses by season.
const OK: u16 = 1319;
/// The manager's name, and the four rows: picture, percent and rank word
/// of the public, the fans, the media and the players.
const NAME: u16 = 1320;
const PICTURES: [u16; 4] = [1321, 1322, 1323, 5589];
const PERCENTS: [u16; 4] = [5644, 5645, 5646, 5647];
const RANKS: [u16; 4] = [1316, 1317, 1318, 5590];
/// The two club images, the public's and the association's, each with a
/// column at 215 for the word.
const IMAGE_PUBLIC: u16 = 1327;
const IMAGE_ASSOCIATION: u16 = 1328;
/// The rank's words and pictures (`FUN_0047ea8d`): the value passes a
/// step to reach the next word; the pictures are `beliebtheit_1..9`.
const RANK_STEPS: [i32; 8] = [95, 87, 79, 69, 59, 49, 39, 19];
const RANK_TEXTS: u16 = 1491;
const RANK_PICTURES: u16 = 1376;
/// The public's word about the club (`FUN_0047efdd`), by the image
/// byte's steps, and the five cases of ill repute before it: 3072
/// "Scheiß Millionäre", 3073 "Treterpack", 3074 "Mitleid", 3075
/// "Armenhaus", 3076 "Schirilieblinge". Of those the records tell
/// three: the rich club far under its season's goal, the club among
/// its league's first three after enough matchdays, and the club
/// whose last defeat lies back of five matchdays. The pauper's case
/// wants the licence check's flag (a feature of its own, not built
/// yet) and the referees' one the officials' counts no save keeps.
const IMAGE_STEPS: [u8; 4] = [44, 34, 24, 14];
const IMAGE_TEXTS: u16 = 1500;
const MILLIONAIRES: u16 = 3072;
const TRETERPACK: u16 = 3073;
const PITY: u16 = 3074;
/// The pay of a squad player that marks the millionaire club.
const PAID: usize = 600;
/// The association's word, 1505 "verzockt" to 1509 "ausgezeichnet".
const ASSOCIATION_TEXTS: u16 = 1504;
/// The manager's record: the public's and the fans' percent, and the
/// media's base, which his authority at his level lifts (`FUN_006a5a7d`
/// — the first ability of the level's row, four and eighty percent).
const POPULARITY_AT: usize = 0x126;
/// A player's record: how many of his likes and dislikes the crowd
/// counts (`FUN_007008b9`): the byte, the nine traits that cost him ten
/// each and the idol's one that earns ten, and his best friend, who
/// earns ten too when he is the trainer himself.
const LIKED_AT: usize = 0x28f;
/// The traits are the player's flags word (his record's `+0x1b0`; the
/// club's squad list keeps a copy of it at its entry's `+0x20`).
const TRAITS_AT: usize = 0x1b0;
const COSTLY_TRAITS: [u32; 9] = [0x1, 0x2, 0x10, 0x8, 0x100, 0x1000, 0x4000, 0x2000, 0x8000];
const ADMIRABLE: u32 = 0x80000;
const FRIEND_AT: usize = 0x204;
const TRAINER_AS_FRIEND: u16 = 17_000;
/// The players' rate stands around a hundred.
const PLAYERS_BASE: i32 = 103;

/// The screen.
pub struct Beliebt;

pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&BELIEBT_DIALOG) else { return };
    dialog.controls.insert(0, caption(dialog.frame, CAPTION_ID, String::new()));
}

impl Screen for Beliebt {
    fn ids(&self) -> &[u16] {
        &[BELIEBT_DIALOG]
    }

    /// `FUN_0047e668`'s OnInitDialog.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(season) = cx.state.season.as_ref() else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(manager) = own_manager(cx) else { return };
        set(
            view,
            dialog,
            CAPTION_ID,
            hub::manager_title_of(game, season, &dialog.title),
        );
        set(
            view,
            dialog,
            NAME,
            format!("{} {}", manager.first_name(), manager.last_name()),
        );

        let liked = manager.get::<[u8; 3]>(POPULARITY_AT);
        let media = authority(manager, i32::from(liked[2]));
        let players = players_rating(records, own_team(cx));
        let values = [i32::from(liked[0]), i32::from(liked[1]), media, players];
        for ((&value, picture), (rank_id, percent_id)) in
            values.iter().zip(PICTURES).zip(RANKS.into_iter().zip(PERCENTS))
        {
            let rank = rank(value);
            set(view, dialog, percent_id, format!("{value}%"));
            set(view, dialog, rank_id, game.text(RANK_TEXTS + rank as u16).to_owned());
            if let Some(c) = control(dialog, picture) {
                view.set_picture(c, Some(Picture::graphic(RANK_PICTURES + rank as u16)));
            }
        }

        let Some(team) = own_team(cx) else { return };
        for (id, word) in [
            (IMAGE_PUBLIC, public_word(game, records, team)),
            (
                IMAGE_ASSOCIATION,
                game.text(ASSOCIATION_TEXTS + association(team)).to_owned(),
            ),
        ] {
            let own = control(dialog, id).map_or("", |i| dialog.controls[i].text.as_str());
            set(view, dialog, id, format!("{own}\\t{word}"));
        }
        // Everything here shows; the only button is the OK.
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(OK) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// A value's rank: the words and pictures count up the steps it passes.
fn rank(value: i32) -> usize {
    RANK_STEPS.iter().take_while(|&&step| value <= step).count()
}

/// The media's rate: the manager's base lifted by his authority — the
/// first ability of his level's row — to four times its worth plus
/// eighty percent (`FUN_006a5a7d`).
fn authority(manager: &Manager, base: i32) -> i32 {
    let level = usize::try_from(standing(manager).min(15)).unwrap_or(0);
    let ability = i32::from(manager.abilities()[level][0]);
    (base * (ability * 4 + 80) / 100).clamp(0, 100)
}

/// What a player is worth to the crowd (`FUN_007008b9`): his byte,
/// cheapened by each costly trait and heartened by being somebody's
/// idol or the trainer's friend — the boosts only while he is under a
/// hundred and never past it — and only over a hundred and twenty is
/// he pulled back.
fn liked_by_crowd(player: &Player) -> i32 {
    let mut v = i32::from(player.get::<u8>(LIKED_AT));
    let traits = player.get::<u32>(TRAITS_AT);
    for bit in COSTLY_TRAITS {
        if traits & bit != 0 {
            v -= 10;
        }
    }
    if traits & ADMIRABLE != 0 && v < 100 {
        v = (v + 10).min(100);
    }
    if player.get::<u16>(FRIEND_AT) > TRAINER_AS_FRIEND && v < 100 {
        v = (v + 10).min(100);
    }
    v.min(120)
}

/// What the squad makes of the manager (`CBeliebt`'s loop): the mean of
/// their worth about a hundred and three, three times as loud; without
/// a squad, nothing.
fn players_rating(records: &Records, team: Option<&Team>) -> i32 {
    let Some(team) = team else { return 0 };
    let squad = team.players();
    if squad.is_empty() {
        return 0;
    }
    let sum: i32 = squad
        .iter()
        .filter_map(|&id| records.player(id))
        .map(|p| liked_by_crowd(p) - PLAYERS_BASE)
        .sum();
    (sum * 3 / i32::try_from(squad.len()).unwrap_or(1) + 95).clamp(0, 100)
}

/// What the public says of the club (`FUN_0047efdd`): the first of the
/// five cases of ill repute that holds, else the word its image byte
/// reaches.
fn public_word(game: &Game, records: &Records, team: &Team) -> String {
    let played = a3_core::league::matchdays(records, team);
    if millionaires(game, records, team, played) {
        return game.text(MILLIONAIRES).to_owned();
    }
    let place = a3_core::league::place(team);
    if played >= 3 && (1..=3).contains(&place) {
        return game.text(TRETERPACK).to_owned();
    }
    if played >= 10 && long_since_defeat(records, team, played) {
        return game.text(PITY).to_owned();
    }
    let v = team.public_image();
    let steps = IMAGE_STEPS.iter().take_while(|&&step| v <= step).count();
    game.text(IMAGE_TEXTS + steps as u16).to_owned()
}

/// "Scheiß Millionäre" (`FUN_0047eb1f`): ten matchdays in, two places
/// short of the season's goal or worse (`FUN_00775fa0` under −2: the
/// goal, team `+0x136`, not reached and more than two places beyond
/// it), and more than two squad players paid over 600 (their `+0x88`).
fn millionaires(game: &Game, records: &Records, team: &Team, played: usize) -> bool {
    if played < 10 {
        return false;
    }
    let tables = &game.rules.league;
    if a3_core::league::goal_reached(records, tables, team) > 0 {
        return false;
    }
    if a3_core::league::goal_missing(records, tables, team) <= 2 {
        return false;
    }
    let over = team
        .players()
        .iter()
        .filter_map(|&id| records.player(id))
        .filter(|p| usize::from(p.salary()) > PAID)
        .count();
    over > 2
}

/// "Mitleid" (`FUN_0047ec75`, `FUN_00660cda`): five matchdays or more
/// since the club's last defeat — the league's results walked back
/// from the last one played, the club by its place in the league's
/// team list (`+0x106`); the whole season without a defeat counts.
fn long_since_defeat(records: &Records, team: &Team, played: usize) -> bool {
    let Some(league) =
        a3_core::opponent::League::of(records, team.country().into(), team.level())
    else {
        return false;
    };
    let place = usize::from(team.league_place());
    for matchday in (1..=played).rev() {
        let Some((home, _, Some((a, b)))) = league.result(matchday, place) else { continue };
        let (own, theirs) = if home == place { (a, b) } else { (b, a) };
        if own < theirs {
            return played - matchday >= 5;
        }
    }
    true
}

/// The association's word (`0x87f8c4`): the club's manners byte, with
/// bits `0x20` and `0x40` pulling it one step down.
fn association(team: &Team) -> u16 {
    let manners = team.association();
    let mut word = match manners & 0x1f {
        0..=1 => 5,
        2..=4 => 4,
        5..=8 => 3,
        9..=22 => 2,
        _ => 1,
    };
    if manners & 0x60 != 0 && word > 1 {
        word -= 1;
    }
    word
}
