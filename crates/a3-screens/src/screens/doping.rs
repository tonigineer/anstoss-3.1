//! Doping (dialog 394, `CMediDoping`), from the medical department: the
//! squad with strength, age, condition and freshness, whether each is on
//! the list, doped, ready (`FUN_00516a2a`); a click in a row doses him at
//! once, a click in its "Liste" column puts him on the list or takes him
//! off (`OnUserC000_00516dad` on the list's `MSG_SEND_TAB_POS`,
//! `FUN_00516f7a`), the pills dose all on it
//! (`OnClicked_IDC_DOPING_ALLE_DOPEN_005173ff`). What to raise and how
//! hard the stuff is to find stand on the two option groups. Telling the
//! association of other humans' players (`CDopingMitspieler`) is not
//! built.

use std::collections::HashMap;

use a3_core::doping;
use a3_core::records::Records;
use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, ok_or_cancel, set};
use crate::pointer::Target;
use crate::view::View;

pub const DOPING: u16 = 394;
const LIST: u16 = 4926;
/// The column whose click puts on the list or takes off.
const LIST_COLUMN: usize = 6;
const KINDS: u16 = 4928;
const SUBSTANCES: u16 = 4927;
const ALL: u16 = 4930;
/// Telling the association (`FUN_00517588`): hidden without other humans
/// (`FUN_0051689b`), crossed out with them.
const REPORT: u16 = 5485;
const CAPTION_ID: u16 = 9945;
/// "Aggression steigern" and the normal stuff, as it opens.
const FIRST_KIND: usize = 0;
const YES: u16 = 1028;
const NO: u16 = 1029;
const NOT_YOURSELF: u16 = 3133;
const WONT_BE_LISTED: u16 = 4068;
const WONT_TAKE: u16 = 4069;
const NOT_AGAIN: u16 = 4070;
const DOSED: u16 = 4071;
const INJURED_INK: [u8; 3] = [255, 0, 0];
/// The medical budget planned (P+0x84) and spent (B+0x84): the club's
/// +0x1a4 and +0x2ac, in the team record 0x278 further on.
const MEDICAL_PLAN: usize = 0x1a4 + a3_core::finances::CLUB_TO_TEAM;
const MEDICAL_SPENT: usize = 0x2ac + a3_core::finances::CLUB_TO_TEAM;
/// The manager's own record as a player: 17,000 and his number.
const MANAGER_PLAYERS: u16 = 17_000;

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&DOPING) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct Doping;

/// The squad's ids, in its order.
fn squad(cx: &Cx) -> Vec<u16> {
    let Some(season) = cx.state.season.as_ref() else { return Vec::new() };
    let club = season.manager().and_then(|m| m.club).and_then(|k| season.clubs.get(&k));
    club.map_or_else(Vec::new, |c| c.status.iter().map(|s| s.id).collect())
}

/// The manager's authority at his level, which a player's luck must lie
/// below when only luck makes him ready.
fn authority(records: &Records, slot: usize) -> i32 {
    records.managers.get(slot).map_or(0, |m| {
        let level = usize::from(m.level()).min(15);
        i32::from(m.abilities()[level][3])
    })
}

fn player_mut<'a>(cx: &'a mut Cx, id: u16) -> Option<&'a mut a3_core::records::Player> {
    cx.state.records.as_mut()?.players.get_mut(usize::from(id))
}

fn slot(cx: &Cx) -> Option<usize> {
    cx.state.records.as_ref()?.human_slot(cx.state.season.as_ref()?.current)
}

fn willing(cx: &Cx, id: u16) -> bool {
    let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
    else {
        return false;
    };
    let Some(r) = records.player(id) else { return false };
    let club = season.manager().and_then(|m| m.club).and_then(|k| season.clubs.get(&k));
    let age = club
        .and_then(|c| c.status.iter().position(|s| s.id == id).and_then(|p| c.players.get(p)))
        .map_or(0, |p| p.age(season.date()));
    let readiness = doping::readiness(r, id, age, records.globals.start_year());
    let authority = slot(cx).map_or(0, |s| authority(records, s));
    doping::willing(records, readiness, id, authority)
}

impl Doping {
    fn fill(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(season) = cx.state.season.as_ref() else { return };
        let Some(club) = season.manager().and_then(|m| m.club).and_then(|k| season.clubs.get(&k))
        else {
            return;
        };
        let records = cx.state.records.as_ref();
        let today = season.date();
        let yes_no = |b: bool, no: &str| if b { game.text(YES).to_owned() } else { no.to_owned() };
        let mut rows = Vec::new();
        let mut inks = Vec::new();
        let mut keys = Vec::new();
        for (p, (player, status)) in club.players.iter().zip(&club.status).enumerate() {
            let fitness = a3_core::lineup::fitness_of(&club.fitness, p);
            let r = records.and_then(|r| r.player(status.id));
            let listed = r.is_some_and(doping::on_list);
            let doped = r.is_some_and(doping::doped);
            rows.push(
                [
                    super::hub::name(player),
                    super::hub::abbreviation(game, player.positions[0]),
                    player.strength.to_string(),
                    player.age(today).to_string(),
                    a3_core::lineup::condition(fitness).to_string(),
                    a3_core::lineup::freshness(fitness).to_string(),
                    yes_no(listed, "-"),
                    yes_no(doped, "-"),
                    yes_no(willing(cx, status.id), game.text(NO)),
                ]
                .join("\t"),
            );
            inks.push((status.injured > 0).then_some(INJURED_INK));
            keys.push(usize::from(status.id));
        }
        if let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) {
            // Filled anew, the list has no row chosen (`FUN_007aa082`).
            list.by_column = true;
            list.selected = None;
            list.rows = rows;
            list.inks = inks;
            list.keys = keys;
        }
    }

    /// A dose of what the options say for `id`, with its price and its
    /// roll (`FUN_00516f7a`'s last branch, the loop of the pills).
    fn dose(cx: &mut Cx, view: &View, id: u16) {
        let dialog = cx.dialog;
        let chosen = |group| control(dialog, group).map_or(0, |c| view.chosen(c));
        let esn = chosen(SUBSTANCES) == 1;
        let kind = chosen(KINDS).min(2) as u8;
        let own = super::squad::own_team(cx).map(|t| (t.country(), u16::from(t.number())));
        let state = &mut *cx.state;
        let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_mut()) else {
            return;
        };
        let Some((country, number)) = own else { return };
        let plan = records.team(country.into(), number).map_or(0, |t| t.get::<i32>(MEDICAL_PLAN));
        let risk = doping::risk(plan, esn);
        let roll = season.random.below(100);
        let Some(r) = records.players.get_mut(usize::from(id)) else { return };
        doping::dose(r, kind, roll, risk);
        let condition = r.condition();
        if let Some(team) = records.team_mut(country.into(), number) {
            let spent = team.get::<i32>(MEDICAL_SPENT).wrapping_add(doping::price(esn));
            team.put(MEDICAL_SPENT, spent);
        }
        // The club follows its player's condition.
        let key = season.manager().and_then(|m| m.club);
        if let Some(club) = key.and_then(|k| season.clubs.get_mut(&k))
            && let Some(p) = club.status.iter().position(|s| s.id == id)
            && let Some(fitness) = club.fitness.get_mut(p)
        {
            fitness.condition = condition.into();
        }
    }

    fn refuse(cx: &mut Cx, id: u16) {
        if let Some(r) = player_mut(cx, id) {
            a3_core::mood::change_trainer(r, doping::REFUSAL);
        }
    }
}

impl Screen for Doping {
    fn ids(&self) -> &[u16] {
        &[DOPING]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        if let Some(c) = control(dialog, KINDS) {
            view.set_chosen(c, FIRST_KIND);
        }
        if let Some(c) = control(dialog, SUBSTANCES) {
            view.set_chosen(c, 0);
        }
        let humans = cx.state.season.as_ref().map_or(0, |s| s.managers.len());
        if let Some(c) = control(dialog, REPORT) {
            view.controls[c].hidden = humans < 2;
        }
        let report = |t| matches!(t, Target::Button(b) if dialog.controls[b].id == Some(REPORT));
        mark_inert(view, dialog, |t| !report(t));
        self.fill(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        // The refusals with the alert's style 4, the dose with 3.
        let notice = |text, player| {
            let style = if text == DOSED { 3 } else { 4 };
            let args = Args::StyledFor(text, player, style);
            Outcome::Popup(Request::with(super::popups::NOTICE, args))
        };
        match target {
            Target::Button(b) if id(b) == Some(ALL) => {
                for player in squad(cx) {
                    let r = cx.state.records.as_ref().and_then(|r| r.player(player));
                    if !r.is_some_and(|r| doping::on_list(r) && !doping::doped(r)) {
                        continue;
                    }
                    if willing(cx, player) {
                        Self::dose(cx, view, player);
                    } else {
                        Self::refuse(cx, player);
                    }
                }
                self.fill(cx, view);
                Outcome::Stay
            }
            Target::Cell { control: c, row, column } if id(c) == Some(LIST) => {
                let Some(player) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied())
                else {
                    return Outcome::Stay;
                };
                let player = player as u16;
                let r = cx.state.records.as_ref().and_then(|r| r.player(player));
                let listed = r.is_some_and(doping::on_list);
                let doped = r.is_some_and(doping::doped);
                if column == LIST_COLUMN {
                    // On the list, or off it; the unready refuse (and like
                    // the trainer the less for it).
                    if !listed && !willing(cx, player) {
                        Self::refuse(cx, player);
                        return notice(WONT_BE_LISTED, player);
                    }
                    if let Some(r) = player_mut(cx, player) {
                        doping::toggle_list(r);
                    }
                    self.fill(cx, view);
                    return Outcome::Stay;
                }
                let own = slot(cx).map(|s| MANAGER_PLAYERS + s as u16 + 1);
                if own == Some(player) {
                    let args = Args::Text(NOT_YOURSELF);
                    return Outcome::Popup(Request::with(super::popups::NOTICE, args));
                }
                if !willing(cx, player) {
                    return notice(WONT_TAKE, player);
                }
                if doped {
                    Self::refuse(cx, player);
                    return notice(NOT_AGAIN, player);
                }
                // The list is filled anew once the alert is answered.
                Self::dose(cx, view, player);
                notice(DOSED, player)
            }
            _ => {
                if ok_or_cancel(dialog, target).0 {
                    return Outcome::Close(Reply::Ok);
                }
                Outcome::Stay
            }
        }
    }

    fn answer(&mut self, cx: &mut Cx, view: &mut View, _: Reply) -> Outcome {
        self.fill(cx, view);
        Outcome::Stay
    }
}
