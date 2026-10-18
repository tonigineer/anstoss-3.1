//! The club's tactics, the menu bar's third symbol (dialog 271, `CTaktik`;
//! TAKTIK.md in the RE repo): how the team plays, its hardness and effort,
//! time-wasting and dirty tricks, stepped through on their pictures; the
//! players to spare and to collect a yellow card, the opponent's to foul and
//! to provoke; a special tactic, the offside trap, pressing, the keeper
//! coming out, easing off three goals up, the sides to attack over, the
//! passing and how the team keeps its positions; and a special win bonus.
//! The title names the next match, a line its referee. What the screen sets
//! is the club's at once; the stepping buttons' states when it is left.

use std::collections::HashMap;

use a3_core::calendar::Calendar;
use a3_core::cards;
use a3_core::records::{Records, Team};
use a3_core::season::{ClubState, Rules};
use a3_core::tactics::{
    self, EASE_OFF, KEEPER_UP, OFFSIDE_TRAP, PASSING, PICKS, PRESSING, Pick, SIDES, Tactics,
};
use a3_core::world::Player;
use a3_files::rch::{Dialog, Kind};

use super::calendar::FIXTURE_AHEAD;
use super::duties::short_name;
use super::hub::{self, TACTICS, TACTICS_SYMBOL};
use super::playerinfo::money;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::bitmap::Picture;
use crate::controls::options::NONE;
use crate::pointer::{Grip, Target};
use crate::view::View;

/// Controls by their ids in the dialog file: the title and the referee.
const TITLE: u16 = 3735;
const REFEREE: u16 = 3736;
/// The stepping buttons — "Spielweise", "Härte", "Einsatz", "Zeitspiel",
/// "Miese Tricks" — in the order [`Tactics::keep_buttons`] takes them, and
/// the labels the first four name their state in.
const BUTTONS: [u16; 5] = [3737, 3738, 3739, 3756, 3757];
const LABELS: [u16; 4] = [6278, 6277, 6276, 3754];
/// The combo boxes of the players to spare, to collect a yellow card, and
/// the opponent's to foul and to provoke.
const PLAYERS: [u16; PICKS] = [3767, 5525, 3769, 5524];
const SPECIAL: u16 = 3762;
/// "Ja" or "Nein": the offside trap, pressing, the keeper playing up,
/// easing off three goals up.
const YES_NO: [(u16, u16); 4] =
    [(3742, OFFSIDE_TRAP), (3745, PRESSING), (3751, KEEPER_UP), (5523, EASE_OFF)];
const YES: usize = 0;
const NO: usize = 1;
/// The attack over the right, the middle, the left: each on or off.
const SIDE_OPTIONS: [u16; 3] = [5517, 6293, 6294];
const PASSING_SLIDERS: [u16; 3] = [5520, 5521, 5522];
const BONUS: u16 = 5518;
const BONUS_SPIN: u16 = 5519;
const STRENGTH: u16 = 3741;
/// A picture of how the club fares against the opponent.
const RECORD: u16 = 3770;
/// The OK, whose place the bar takes.
const OK: u16 = 1;

/// Texts of the game.
const NOBODY: u16 = 1091;
const REFEREE_TEXT: u16 = 1510;
const STRENGTH_TEXT: u16 = 1595;
const HOME: u16 = 3664;
const AWAY: u16 = 3665;
const HOME_PLACE: u16 = 4242;
const AWAY_PLACE: u16 = 4243;
const LABEL_SEPARATOR: &str = ": ";
/// The record against the opponent, wins less defeats at either's home:
/// this much better shows the one picture, this much worse the other.
const GOOD_RECORD: u16 = 1807;
const BAD_RECORD: u16 = 1810;
const RECORD_MARGIN: i32 = 3;
/// Kinds of the next match whose title names both clubs plainly, and the
/// league's, which gives their places.
const PLAIN_KINDS: [u8; 2] = [0xa, 0xe];
const LEAGUE: u8 = 2;

/// The bar takes the OK's place.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&TACTICS) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    hub::add_bar(dialog, rules);
}

#[derive(Default)]
pub struct TacticsScreen {
    /// The players each combo box lists after "keiner", in its rows.
    rows: [Vec<Pick>; PICKS],
    /// Whether the whistle asks.
    whistle: bool,
}

/// The club's next match: its opponent, whether at home, its kind and
/// its referee.
struct Next {
    opponent: (usize, usize),
    home: bool,
    kind: u8,
    referee: i16,
}

impl Screen for TacticsScreen {
    fn ids(&self) -> &[u16] {
        &[TACTICS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        hub::open_bar(cx, view, TACTICS_SYMBOL);
        let Some(club) = own(cx) else { return };
        let tactics = club.tactics;
        let tables = &game.rules.tactics;
        let states = [
            usize::from(tactics.style),
            usize::from(tactics.hardness),
            tactics.effort_state(tables),
            tactics.time_wasting(),
            usize::from(tactics.has(tactics::TRICKS)),
        ];
        for (&id, state) in BUTTONS.iter().zip(states) {
            if let Some(button) =
                control(dialog, id).and_then(|c| view.controls[c].state.as_button_mut())
            {
                button.step = state;
            }
        }
        for i in 0..LABELS.len() {
            label(cx, view, i);
        }
        if let Some(c) = control(dialog, SPECIAL) {
            view.set_chosen(c, tactics.special());
        }
        for (id, flag) in YES_NO {
            if let Some(c) = control(dialog, id) {
                view.set_chosen(c, if tactics.has(flag) { YES } else { NO });
            }
        }
        for (id, side) in SIDE_OPTIONS.into_iter().zip(SIDES) {
            if let Some(c) = control(dialog, id) {
                view.set_chosen(c, if tactics.has(side) { 0 } else { NONE });
            }
        }
        for (field, id) in PASSING_SLIDERS.into_iter().enumerate() {
            if let Some(value) = control(dialog, id).and_then(|c| view.value_mut(c)) {
                value.range = (0, PASSING[field].1);
                value.value = tactics.passing(field);
            }
        }
        if let Some(value) = control(dialog, BONUS_SPIN).and_then(|c| view.value_mut(c)) {
            value.range = (0, tables.bonus_position(tactics::MOST_BONUS));
            value.value = tables.bonus_position(tactics.bonus);
        }
        set(view, dialog, BONUS, money(game, tactics.bonus.into()));
        let strength = (tactics.offside / 10).to_string();
        set(view, dialog, STRENGTH, game.text(STRENGTH_TEXT).replacen("%s", &strength, 1));

        let next = next_match(cx);
        set(view, dialog, TITLE, title(cx, next.as_ref()));
        let referee = next.as_ref().and_then(|n| {
            let (name, competence, hardness) = cx.state.records.as_ref()?.referee(n.referee)?;
            Some(
                game.text(REFEREE_TEXT)
                    .replacen("%s", name, 1)
                    .replacen("%d", &competence.to_string(), 1)
                    .replacen("%d", &hardness.to_string(), 1),
            )
        });
        set(view, dialog, REFEREE, referee.unwrap_or_default());
        self.fill_players(cx, view, next.as_ref());
        show_record(cx, view, next.as_ref());

        let works = |target: Target| match hub::on_bar(dialog, target) {
            Some(item) => hub::bar_works(item),
            None => true,
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        if let Some(item) = hub::on_bar(dialog, target) {
            let outcome = hub::bar_click(cx, view, item, TACTICS_SYMBOL);
            if matches!(outcome, Outcome::Close(_)) {
                keep_buttons(cx, view);
            }
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        match target {
            // A stepping button: on with the left button, back with the
            // right; its label names the state.
            Target::Button(c) | Target::Menu(Grip::Control(c)) if BUTTONS.contains(&id(c)) => {
                let back = matches!(target, Target::Menu(_));
                let count = match &dialog.controls[c].kind {
                    Kind::States { pictures, .. } => pictures.len(),
                    _ => 0,
                };
                if let Some(button) = view.controls[c].state.as_button_mut() {
                    button.step_on(count, back);
                }
                if let Some(i) = LABELS.iter().zip(BUTTONS).position(|(_, b)| b == id(c)) {
                    label(cx, view, i);
                }
            }
            Target::Option { control: c, item } if id(c) == SPECIAL => {
                change(cx, |t| t.set_special(item));
            }
            Target::Option { control: c, item } => {
                if let Some(&(_, flag)) = YES_NO.iter().find(|&&(o, _)| o == id(c)) {
                    change(cx, |t| t.set(flag, item == YES));
                } else if let Some(i) = SIDE_OPTIONS.iter().position(|&o| o == id(c)) {
                    let on = view.options(c).is_some_and(|o| o.selected().is_some());
                    change(cx, |t| t.set(SIDES[i], on));
                }
            }
            Target::Row { control: c, .. } => {
                let Some(i) = PLAYERS.iter().position(|&p| p == id(c)) else {
                    return Outcome::Stay;
                };
                let row = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0);
                let pick = row.checked_sub(1).and_then(|r| self.rows[i].get(r)).copied();
                change(cx, |t| t.picks[i] = pick);
            }
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. } => {
                let Some(field) = PASSING_SLIDERS.iter().position(|&s| s == id(c)) else {
                    return Outcome::Stay;
                };
                let value = view.value(c).map_or(0, |v| v.value);
                change(cx, |t| t.set_passing(field, value));
            }
            Target::Spin { control: c, .. } if id(c) == BONUS_SPIN => {
                let position = view.value(c).map_or(0, |v| v.value);
                let bonus = cx.game.rules.tactics.bonus_at(position);
                change(cx, |t| t.bonus = bonus);
                set(view, dialog, BONUS, money(cx.game, bonus.into()));
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// The whistle's question: a yes ends the turn.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            keep_buttons(cx, view);
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}

impl TacticsScreen {
    /// The combo boxes: "keiner", then the club's players in its order —
    /// for a yellow card only those one more suspends —, the chosen one
    /// selected; the opponent's two empty and shut without an opponent.
    fn fill_players(&mut self, cx: &Cx, view: &mut View, next: Option<&Next>) {
        let Some(key) = own_key(cx) else { return };
        let Some(club) = own(cx) else { return };
        let records = cx.state.records.as_ref();
        let own_team = records.and_then(|r| team(cx, r, key));
        let warned = |player: usize| {
            let (Some(records), Some(team), Some(next)) = (records, own_team, next) else {
                return false;
            };
            let id = club.status.get(player).map_or(0, |s| s.id);
            records.player(id).is_some_and(|r| {
                cards::before_match(records, team.country().into(), team.level(), next.kind, r)
            })
        };
        let theirs = next.map(|n| (n.opponent, squad(cx, n.opponent)));
        for (i, id) in PLAYERS.into_iter().enumerate() {
            let Some(c) = control(cx.dialog, id) else { continue };
            let (club_key, players): (_, Vec<(Player, u16)>) = if i < tactics::FOUL {
                (key, squad(cx, key))
            } else {
                match &theirs {
                    Some((opponent, players)) => (*opponent, players.clone()),
                    None => {
                        self.rows[i].clear();
                        if let Some(combo) = view.combo_mut(c) {
                            *combo = Default::default();
                            combo.disabled = true;
                        }
                        continue;
                    }
                }
            };
            let places: Vec<usize> =
                (0..players.len()).filter(|&p| i != tactics::CARD || warned(p)).collect();
            self.rows[i] = places.iter().map(|&place| Pick { club: club_key, place }).collect();
            let chosen = club.tactics.picks[i];
            let mut items = vec![cx.game.text(NOBODY).to_owned()];
            items.extend(places.iter().map(|&p| short_name(cx, &players[p].0, players[p].1)));
            let selected = chosen.and_then(|pick| self.rows[i].iter().position(|&r| r == pick));
            if let Some(combo) = view.combo_mut(c) {
                combo.items = items;
                combo.selected = Some(selected.map_or(0, |row| row + 1));
                combo.disabled = false;
            }
        }
    }
}

/// The manager's club, as the season follows it, and its key.
fn own<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

fn own_key(cx: &Cx) -> Option<(usize, usize)> {
    cx.state.season.as_ref()?.manager()?.club
}

/// A club's team record by its key.
fn team<'a>(cx: &Cx, records: &'a Records, (slot, club): (usize, usize)) -> Option<&'a Team> {
    let nation = cx.game.world.country(slot)?.nation;
    records.team(nation, u16::try_from(club).ok()?)
}

/// A club's players in its order, with their ids in the save (0 in a new
/// game).
fn squad(cx: &Cx, key: (usize, usize)) -> Vec<(Player, u16)> {
    let with_ids = |club: &ClubState| {
        let ids = club.status.iter().map(|s| s.id).chain(std::iter::repeat(0));
        club.players.iter().cloned().zip(ids).collect()
    };
    if let Some(club) = cx.state.season.as_ref().and_then(|s| s.clubs.get(&key)) {
        return with_ids(club);
    }
    let records = cx.state.records.as_ref();
    if let Some(club) = records.and_then(|r| a3_core::load::club(r, &cx.game.data, key)) {
        return with_ids(&club);
    }
    let world = cx.game.world.country(key.0).and_then(|c| c.clubs.get(key.1));
    world.map_or_else(Vec::new, |c| c.players.iter().cloned().map(|p| (p, 0)).collect())
}

/// The club's next match as the save has it; in a new game its next league
/// match ahead.
fn next_match(cx: &Cx) -> Option<Next> {
    let key = own_key(cx)?;
    if let Some(records) = cx.state.records.as_ref() {
        let own = team(cx, records, key)?;
        let country = own.next_country();
        let number = usize::try_from(own.next_number()).ok()?;
        if country == 0 {
            return None;
        }
        return Some(Next {
            opponent: (cx.game.world.slot_of(country.into())?, number),
            home: own.next_venue() <= 0,
            kind: own.next_kind(),
            referee: own.referee(),
        });
    }
    let (game, season) = (cx.game, cx.state.season.as_ref()?);
    let m = season
        .next_match(&game.world, &game.calendar, key)
        .filter(|m| m.day < season.day + FIXTURE_AHEAD)?;
    let home = m.home == key;
    Some(Next { opponent: if home { m.away } else { m.home }, home, kind: LEAGUE, referee: 0 })
}

/// The title: the club, or both clubs, which plays at home and, in the
/// league, their places.
fn title(cx: &Cx, next: Option<&Next>) -> String {
    let (game, records) = (cx.game, cx.state.records.as_ref());
    let Some(key) = own_key(cx) else { return String::new() };
    let name = |key: (usize, usize)| -> (String, u8) {
        if let Some(team) = records.and_then(|r| team(cx, r, key)) {
            return (team.name(), team.table_place());
        }
        let club = game.world.country(key.0).and_then(|c| c.clubs.get(key.1));
        (club.map_or_else(String::new, |c| c.name.clone()), 0)
    };
    let (own_name, own_place) = name(key);
    let Some(next) = next else { return own_name };
    let (their_name, their_place) = name(next.opponent);
    if PLAIN_KINDS.contains(&next.kind) {
        return format!("{own_name} - {their_name}");
    }
    let side = |name: &str, place: u8, home: bool| {
        if next.kind == LEAGUE && own_place != 0 {
            let text = game.text(if home { HOME_PLACE } else { AWAY_PLACE });
            text.replacen("%s", name, 1).replacen("%d", &place.to_string(), 1)
        } else {
            game.text(if home { HOME } else { AWAY }).replacen("%s", name, 1)
        }
    };
    format!(
        "{} - {}",
        side(&own_name, own_place, next.home),
        side(&their_name, their_place, !next.home)
    )
}

/// How the club fares against the opponent at either's home over the
/// seasons kept and this one (`FUN_0040dc10`): a picture if clearly better
/// or worse.
fn show_record(cx: &Cx, view: &mut View, next: Option<&Next>) {
    let Some(c) = control(cx.dialog, RECORD) else { return };
    let picture = (|| {
        let records = cx.state.records.as_ref()?;
        let next = next?;
        let ours = team(cx, records, own_key(cx)?)?;
        let theirs = team(cx, records, next.opponent)?;
        let calendar: &Calendar = &cx.game.calendar;
        let at_ours = a3_core::opponent::home_record(records, calendar, ours, theirs);
        let at_theirs = a3_core::opponent::home_record(records, calendar, theirs, ours);
        let balance =
            at_ours[0] as i32 + at_theirs[2] as i32 - at_ours[2] as i32 - at_theirs[0] as i32;
        if balance >= RECORD_MARGIN {
            Some(GOOD_RECORD)
        } else if balance <= -RECORD_MARGIN {
            Some(BAD_RECORD)
        } else {
            None
        }
    })();
    view.controls[c].hidden = picture.is_none();
    view.set_picture(c, picture.map(Picture::graphic));
}

/// A stepping button's label: its text, and the name of its state.
fn label(cx: &Cx, view: &mut View, i: usize) {
    let (game, dialog) = (cx.game, cx.dialog);
    let tables = &game.rules.tactics;
    let texts =
        [&tables.style_texts, &tables.hardness_texts, &tables.effort_texts, &tables.time_texts];
    let Some(button) = control(dialog, BUTTONS[i]) else { return };
    let state = view.controls[button].state.as_button().map_or(0, |b| b.step);
    let Some(c) = control(dialog, LABELS[i]) else { return };
    let name = texts[i].get(state).map_or("", |&t| game.text(t));
    view.set_text(c, format!("{}{LABEL_SEPARATOR}{name}", dialog.controls[c].text));
}

/// The stepping buttons' states into the club's tactics, as the screen is
/// left (`FUN_0058c6f5`).
fn keep_buttons(cx: &mut Cx, view: &View) {
    let dialog = cx.dialog;
    let step = |id: u16| {
        control(dialog, id).and_then(|c| view.controls[c].state.as_button()).map_or(0, |b| b.step)
    };
    let states = BUTTONS.map(step);
    let tables = &cx.game.rules.tactics;
    change(cx, |t| t.keep_buttons(tables, states));
}

/// Changes the club's tactics, and its records with them.
fn change(cx: &mut Cx, change: impl FnOnce(&mut Tactics)) {
    let game = cx.game;
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get_mut(&key) else { return };
    change(&mut club.tactics);
    let tactics = club.tactics;
    let own_ids: Vec<u16> = club.status.iter().map(|s| s.id).collect();
    let Some(records) = state.records.as_mut() else { return };
    let nation = |slot: usize| game.world.country(slot).map(|c| c.nation);
    // The players by their ids: the own in the squad, the opponent's in
    // his.
    let id = |records: &Records, pick: Option<Pick>| -> u16 {
        let Some(pick) = pick else { return 0 };
        if pick.club == key {
            return own_ids.get(pick.place).copied().unwrap_or(0);
        }
        let theirs = nation(pick.club.0)
            .zip(u16::try_from(pick.club.1).ok())
            .and_then(|(n, c)| records.team(n, c));
        theirs.and_then(|t| t.players().get(pick.place).copied()).unwrap_or(0)
    };
    let ids = tactics.picks.map(|pick| id(records, pick));
    let Some(nation) = nation(key.0) else { return };
    let Some(team) = u16::try_from(key.1).ok().and_then(|n| records.team_mut(nation, n)) else {
        return;
    };
    tactics.write(team);
    let human = usize::from(team.human()).checked_sub(1);
    if let Some(extra) = human.and_then(|slot| records.extras.get_mut(slot)) {
        tactics.write_extra(extra, ids);
    }
}
