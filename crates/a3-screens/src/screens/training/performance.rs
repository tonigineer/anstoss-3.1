//! The training statistics, the button 4418 in the long-term training
//! (dialog 406, `CTrainingLeistung`; TRAINING-LANG.md §6.2 in the RE repo):
//! the players whose training goes well and badly by their morale,
//! condition and freshness, the season's training units with last week's,
//! and the team's fitness week by week as three curves. Only OK leaves it;
//! a right click on a player offers his info.

use std::collections::HashMap;

use a3_core::lineup;
use a3_core::season::ClubState;
use a3_files::rch::{Align, Control, Dialog, Kind, PLAIN_FONT};
use bevy_math::Rect;

use super::super::discipline::CAPTION_ID;
use super::super::duties::short_name;
use super::super::playerinfo;
use super::super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::pointer::{Grip, Target};
use crate::controls::ControlState;
use crate::controls::paint::PaintState;
use crate::view::View;

/// The dialog, which the long-term training opens.
pub(crate) const TRAINING_STATISTICS: u16 = 406;

/// Controls by their ids in the dialog file: the two player lists, the
/// units, the graph and OK.
const GOOD: u16 = 4821;
const BAD: u16 = 4822;
const UNITS: u16 = 4823;
const GRAPH: u16 = 5526;
const OK: u16 = 1;
/// The menu a right click on a player opens: "Spielerinfo" (text 4036),
/// the one entry of `CListBoxSpielerInfo`'s.
const MENU_ID: u16 = 9954;
const PLAYER_INFO: u16 = 4036;

/// The reasons a row gives: morale, a super condition, tired, not fit
/// (texts 0x114c to 0x114f).
const MORALE: u16 = 4428;
const SUPER_CONDITION: u16 = 4429;
const TIRED: u16 = 4430;
const NOT_FIT: u16 = 4431;
/// Morale above the first trains well, below the second badly; condition
/// and freshness both above the third well, either below the fourth badly.
const KEEN: i32 = 105;
const LISTLESS: i32 = 85;
const FIT: i32 = 75;
const WORN: i32 = 40;
/// A player out for a week or more is not looked at; one out for fewer
/// days counts ten more morale.
const OUT_DAYS: i32 = 7;
const SHORT_OUT_MORALE: i32 = 10;
/// The kinds of position, the keeper's first, that order the squad.
const KINDS: std::ops::RangeInclusive<u8> = 1..=10;
/// Each list's most rows.
const MOST_ROWS: usize = 56;
/// R+0x271 bit 6: his strength shows as "?" (`FUN_006ff294`).
const STRENGTH_HIDDEN: u8 = 0x40;
/// The training units, all but the eleventh listed.
const UNIT_COUNT: usize = 39;
const UNLISTED_UNIT: usize = 11;
/// The graph's curves, in the order drawn: the byte of a week's record,
/// what fills the graph's height, and the colour.
const CURVES: [(usize, i32, u32); 3] = [(3, 200, 0xe1483d), (4, 200, 0xe6d756), (6, 40, 0x3232f0)];
/// The first week stands this far in, each next one this far right.
const FIRST_X: i32 = 2;
const WEEK_STEP: i32 = 5;

#[derive(Default)]
pub struct TrainingStatistics {
    /// The list a right click opened the menu over, and the player.
    menu: Option<(u16, usize)>,
}

impl Screen for TrainingStatistics {
    fn ids(&self) -> &[u16] {
        &[TRAINING_STATISTICS]
    }

    /// `FUN_005a12fd`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        *self = Self::default();
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        super::super::seasonal_buttons(cx, view);
        if let Some(club) = own(cx) {
            let [good, bad] = performance(cx, club);
            for (list, rows) in [(GOOD, good), (BAD, bad)] {
                if let Some(l) = control(dialog, list).and_then(|c| view.list_mut(c)) {
                    (l.rows, l.keys) = rows.into_iter().unzip();
                }
            }
        }
        let units = units(cx);
        if let Some(l) = control(dialog, UNITS).and_then(|c| view.list_mut(c)) {
            l.rows = units;
        }
        let graph = curves(cx);
        if let Some(paint) = control(dialog, GRAPH).and_then(|c| view.paint_mut(c)) {
            *paint = graph;
        }
        mark_inert(view, dialog, |target| match target {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        // An open menu takes the next click: on its entry, or anywhere else
        // to close it.
        if let Some((list, player)) = self.menu.take() {
            let menu = control(dialog, MENU_ID);
            if let Some(ControlState::Menu(m)) = menu.map(|m| &mut view.controls[m].state) {
                m.at = None;
            }
            if matches!(target, Target::Option { control: c, item: 0 } if Some(c) == menu) {
                // The info pages through the list's players backwards, as
                // the list box keeps them.
                let rows = control(dialog, list).and_then(|l| view.list(l));
                cx.state.paging = rows.map(|l| l.keys.iter().rev().copied().collect());
                return Outcome::Popup(Request::with(playerinfo::CURRENT, Args::Player(player)));
            }
            return Outcome::Stay;
        }
        match target {
            Target::Button(c) if id(c) == OK => return Outcome::Close(Reply::Ok),
            Target::Row { control: c, row } if [GOOD, BAD, UNITS].contains(&id(c)) => {
                if let Some(list) = view.list_mut(c) {
                    list.press(row);
                }
            }
            Target::Menu(Grip::Row { control: c, row }) if [GOOD, BAD].contains(&id(c)) => {
                self.menu = open_menu(cx, view, c, row).map(|player| (id(c), player));
            }
            _ => {}
        }
        Outcome::Stay
    }
}

/// The players whose training goes well and those whose goes badly
/// (`FUN_005a1682`), each row with his place in the squad: the squad by
/// the kind of his main position, the club's order within each
/// (`FUN_00696590`); a player out for a week or more is skipped, one out
/// for fewer days counts ten more morale. Well: morale above 105, or
/// condition and freshness both above 75; badly: morale below 85,
/// freshness or condition below 40. One with reasons both ways shows in
/// neither, each list holds 56 at most. A row is "%s\t%s\t%s\t%s" of his
/// name, his position's short name, his strength or "?", and the reason —
/// the last that holds, as each one's text is copied over the one before
/// (`FUN_00730316`).
fn performance(cx: &Cx, club: &ClubState) -> [Vec<(String, usize)>; 2] {
    let game = cx.game;
    let records = cx.state.records.as_ref();
    let mut lists: [Vec<(String, usize)>; 2] = Default::default();
    let order = KINDS.flat_map(|kind| {
        club.players.iter().enumerate().filter(move |(_, p)| p.positions[0] == kind)
    });
    for (p, player) in order {
        let status = club.status.get(p).copied().unwrap_or_default();
        if status.injured >= OUT_DAYS {
            continue;
        }
        let fitness = lineup::fitness_of(&club.fitness, p);
        let morale = fitness.morale + if status.injured > 0 { SHORT_OUT_MORALE } else { 0 };
        let (fresh, condition) = (lineup::freshness(fitness), lineup::condition(fitness));
        let mut good = None;
        if morale > KEEN {
            good = Some(MORALE);
        }
        if fresh > FIT && condition > FIT {
            good = Some(SUPER_CONDITION);
        }
        let mut bad = None;
        if morale < LISTLESS {
            bad = Some(MORALE);
        }
        if fresh < WORN {
            bad = Some(TIRED);
        }
        if condition < WORN {
            bad = Some(NOT_FIT);
        }
        let (list, reason) = match (good, bad) {
            (Some(reason), None) => (0, reason),
            (None, Some(reason)) => (1, reason),
            _ => continue,
        };
        if lists[list].len() >= MOST_ROWS {
            continue;
        }
        let kind = usize::from(player.positions[0]);
        let position = game.rules.pitch.abbreviations.get(kind).map_or("", |&t| game.text(t as u16));
        let hidden = records
            .and_then(|r| r.player(status.id))
            .is_some_and(|r| r.picks() & STRENGTH_HIDDEN != 0);
        let strength = if hidden { "?".to_owned() } else { player.strength.to_string() };
        let name = short_name(cx, player, status.id);
        let row = format!("{name}\t{position}\t{strength}\t{}", game.text(reason));
        lists[list].push((row, p));
    }
    lists
}

/// The season's training units (`FUN_005a1a76`): every unit but the
/// eleventh, "%s\t%d\t%d" of its name, how often the season gave it and
/// how often last week.
fn units(cx: &Cx) -> Vec<String> {
    let game = cx.game;
    let Some(extra) = extra(cx) else { return Vec::new() };
    let (season, week) = (extra.season_units(), extra.week_units());
    (0..UNIT_COUNT)
        .filter(|&u| u != UNLISTED_UNIT)
        .map(|u| {
            let name = game.text(game.rules.training.unit_text(u as u8));
            format!("{name}\t{}\t{}", season[u], week[u])
        })
        .collect()
}

/// The graph (`FUN_005a12fd`'s pen): for each curve a line through the
/// weeks, the first at x 2 and each next five further, at y the graph's
/// height less one, less the value's share of that height less two —
/// starting with a line from the first point to itself. The lines go
/// onto the graph's own surface, which shows the dialog's background.
fn curves(cx: &Cx) -> PaintState {
    let mut paint = PaintState::default();
    let Some(rect) = control(cx.dialog, GRAPH).map(|c| cx.dialog.controls[c].rect) else {
        return paint;
    };
    let weeks = extra(cx).map(|e| e.fitness_weeks()).unwrap_or_default();
    let high = rect.height() as i32;
    for (byte, full, colour) in CURVES {
        let at = |k: usize| {
            let y = high - 1 - i32::from(weeks[k][byte]) * (high - 2) / full;
            (FIRST_X + WEEK_STEP * k as i32, y)
        };
        let Some(mut from) = (!weeks.is_empty()).then(|| at(0)) else { continue };
        for k in 0..weeks.len() {
            let to = at(k);
            paint.line(from, to, colour);
            from = to;
        }
    }
    paint
}

/// Opens the menu of the player a right click fell on, there: only
/// "Spielerinfo" (`CListBoxSpielerInfo`); the player.
fn open_menu(cx: &mut Cx, view: &mut View, list: usize, row: usize) -> Option<usize> {
    let scroll = view.list(list).map_or(0, |l| l.scroll);
    let player = *view.list(list)?.keys.get(scroll + row)?;
    let target = Target::Row { control: list, row };
    let (_, rect) = crate::pointer::targets(cx.dialog, view, &crate::pointer::Pixels)
        .into_iter()
        .find(|(t, _)| *t == target)?;
    let menu = control(cx.dialog, MENU_ID)?;
    if let ControlState::Menu(m) = &mut view.controls[menu].state {
        m.at = Some(rect.center());
        m.entries = vec![cx.game.text(PLAYER_INFO).to_owned()];
    }
    Some(player)
}

/// The manager's club, as the season follows it.
fn own<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

/// The extra data of the manager's club, by its team's human slot.
fn extra<'a>(cx: &'a Cx) -> Option<&'a a3_core::records::Extra> {
    let records = cx.state.records.as_ref()?;
    let team = super::super::squad::own_team(cx)?;
    records.extras.get(usize::from(team.human()).checked_sub(1)?)
}

/// Puts the dialog's caption in its frame, and the menu over everything.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&TRAINING_STATISTICS) else { return };
    let caption = super::super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
    dialog.controls.push(Control {
        kind: Kind::Menu,
        rect: Rect::new(0.0, 0.0, 0.0, 0.0),
        align: Align::Left,
        text: String::new(),
        id: Some(MENU_ID),
        font: PLAIN_FONT,
        ink: [0; 3],
    });
}
