//! The stadium, the menu bar's twelfth symbol (dialog 464,
//! `CStadionHaupt`; STADION.md in the RE repo): the places of the stadium
//! and its stands, the pitch, the scoreboard and the comfort, changed on a
//! copy of the club's stadiums that goes back when the screen is left; the
//! money of a change is booked at once.

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::{NOTICE, QUESTION, STADIUM};
use a3_core::season::Rules;
use a3_core::stadium::{self, COMFORTS, STANDS, Sub};
use a3_files::rch::Dialog;

use super::hub::{self, STADIUM_SYMBOL};
use super::playerinfo::money;
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::options::NONE;
use crate::pointer::Target;
use crate::view::View;

/// The OK, whose place the bar takes; the caption, the file's title.
const OK: u16 = 1;
const CAPTION_ID: u16 = 9935;
/// The stadium played in and the one shown.
const CURRENT_COMBO: u16 = 5924;
const SHOWN_COMBO: u16 = 5845;
/// The places by roof, the boxes, the sum; the building time.
const PLACES: [u16; 6] = [5846, 5847, 5848, 5849, 5850, 5851];
const BUILDING: u16 = 5927;
const NO_WEEKS: &str = "-";
const ONE_WEEK: u16 = 4142;
const WEEKS: u16 = 4143;
/// The stands' places and conditions, their heaters, roofs and tracks.
const STAND_LIST: u16 = 5853;
const CONDITION_LIST: u16 = 6370;
const CONDITIONS: u16 = 4144;
const HEATER_BOXES: [u16; STANDS] = [5894, 5895, 5896, 5897];
const ROOF_BOXES: [u16; STANDS] = [5901, 5900, 5899, 5898];
const TRACK_BOXES: [u16; STANDS] = [5902, 5903, 5904, 5905];
const BUILDING_INK: [u8; 3] = [255, 0, 0];
const BLACK: [u8; 3] = [0, 0, 0];
/// The pitch: its state by wear, and new turf.
const PITCH: u16 = 5923;
const PITCH_TEXTS: [(u8, u16); 5] = [(2, 2203), (5, 2205), (10, 2204), (20, 2206), (30, 2207)];
const RUINED_PITCH: u16 = 2208;
const TURF: u16 = 5918;
/// The 2D view, the 3D flight, a dead item, the expansion.
const VIEWS: u16 = 5852;
const THREE_D: usize = 1;
const DEAD_ITEM: usize = 2;
const NO_3D: u16 = 5040;
/// The scoreboard and the six comfort boxes, in `COMFORTS`' order.
const SCOREBOARD: u16 = 5919;
const SCOREBOARD_TEXTS: u16 = 1129;
const COMFORT_BOXES: [u16; 6] = [5855, 5856, 5857, 5858, 5859, 5860];
/// The cost of the changes, or what they bring in.
const COST_LABEL: u16 = 5925;
const COST: u16 = 5922;
const INCOME_LABEL: u16 = 4151;
const CARRY_OUT: u16 = 5917;
const TOO_LITTLE: u16 = 4255;
const SURE: u16 = 4256;
/// The club grounds (466), not built.
const GROUNDS: u16 = 5861;
/// A smaller stadium cannot take the season-ticket holders (club +0x498)
/// in the season's first 92 days (`FUN_0071c08d`).
const SEASON_TICKETS_AT: usize = 0x498 + 0x278;
const TICKET_DAYS: u16 = 92;
const TOO_SMALL: u16 = 4809;

/// The bar takes the OK's place; the caption keeps the file's title.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&STADIUM) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    let title = dialog.title.clone();
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, title));
    hub::add_bar(dialog, rules);
}

/// The screen and its copies.
#[derive(Default)]
pub struct StadiumScreen {
    subs: Vec<Sub>,
    /// The stadium shown, a copy of its sub-record, and the one chosen to
    /// play in.
    shown: usize,
    copy: Option<Sub>,
    current: usize,
    /// The comfort ticked and the scoreboard chosen.
    chosen: [bool; 6],
    scoreboard: usize,
    asked: Option<Ask>,
}

#[derive(Clone, Copy)]
enum Ask {
    Whistle,
    CarryOut,
    View,
}

impl Screen for StadiumScreen {
    fn ids(&self) -> &[u16] {
        &[STADIUM]
    }

    /// `FUN_0057cc42`: the bar, the copies, the fill.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        *self = Self::default();
        hub::open_bar(cx, view, STADIUM_SYMBOL);
        let club = own_team(cx).map(|t| (t.country(), u16::from(t.number())));
        if let (Some((country, number)), Some(records)) = (club, cx.state.records.as_ref()) {
            self.subs = stadium::subs(records, country, number).unwrap_or_default();
        }
        self.copy = self.subs.first().cloned();
        self.choose_from_copy();
        self.fill(cx, view);
        self.fill_combos(cx, view);
        self.fill_scoreboard(cx, view);
        self.fill_cost(cx, view);
        if let Some(c) = control(dialog, VIEWS) {
            view.set_chosen(c, NONE);
        }
        let works = |target: Target| match target {
            Target::Option { control: c, item } if dialog.controls[c].id == Some(VIEWS) => {
                matches!(item, THREE_D | DEAD_ITEM)
            }
            Target::Button(c) => dialog.controls[c].id != Some(GROUNDS),
            _ => hub::on_bar(dialog, target).is_none_or(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some(item) = hub::on_bar(dialog, target) {
            // Leaving writes the copies back (`FUN_0057de04`).
            if item != STADIUM_SYMBOL {
                self.write_back(cx);
            }
            let outcome = hub::bar_click(cx, view, item, STADIUM_SYMBOL);
            self.asked = matches!(outcome, Outcome::Popup(_)).then_some(Ask::Whistle);
            return outcome;
        }
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Check(c) => {
                let Some(i) = COMFORT_BOXES.iter().position(|&b| id(c) == Some(b)) else {
                    return Outcome::Stay;
                };
                self.chosen[i] = view.check(c).is_some_and(|b| b.checked);
                self.fill_cost(cx, view);
                Outcome::Stay
            }
            Target::Row { control: c, .. } if id(c) == Some(SCOREBOARD) => {
                if let Some(board) = view.combo(c).and_then(|b| b.selected) {
                    self.scoreboard = board;
                    self.fill_cost(cx, view);
                }
                Outcome::Stay
            }
            // The row taken as the sub-record's index (`FUN_0057fc71`).
            Target::Row { control: c, .. } if id(c) == Some(SHOWN_COMBO) => {
                let Some(row) = view.combo(c).and_then(|b| b.selected) else {
                    return Outcome::Stay;
                };
                if row == self.shown || row >= self.subs.len() {
                    return Outcome::Stay;
                }
                if let Some(copy) = self.copy.take() {
                    self.subs[self.shown] = copy;
                }
                self.shown = row;
                self.copy = self.subs.get(row).cloned();
                self.choose_from_copy();
                self.fill(cx, view);
                self.fill_scoreboard(cx, view);
                self.fill_cost(cx, view);
                Outcome::Stay
            }
            Target::Row { control: c, .. } if id(c) == Some(CURRENT_COMBO) => {
                self.choose_current(cx, view, c)
            }
            Target::Button(c) if id(c) == Some(CARRY_OUT) => self.carry_out(cx),
            Target::Button(c) if id(c) == Some(TURF) => {
                if let (Some(team), Some(copy)) = (own_team_mut(cx), self.copy.as_mut()) {
                    stadium::new_turf(team, copy);
                }
                self.fill(cx, view);
                Outcome::Stay
            }
            Target::Option { control: c, item } if id(c) == Some(VIEWS) => {
                if item == THREE_D {
                    self.asked = Some(Ask::View);
                    return Outcome::Popup(Request::with(NOTICE, Args::Text(NO_3D)));
                }
                view.set_chosen(c, NONE);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }

    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        match self.asked.take() {
            Some(Ask::Whistle) if reply == Reply::Ok => return Outcome::Close(Reply::Ok),
            Some(Ask::CarryOut) if reply == Reply::Ok => {
                if let (Some(team), Some(copy)) = (own_team_mut(cx), self.copy.as_mut()) {
                    stadium::carry_out(team, copy, &self.chosen, self.scoreboard);
                }
                self.fill_cost(cx, view);
            }
            // The views' item is cleared after the popup (0x57fc50).
            Some(Ask::View) => {
                if let Some(c) = control(cx.dialog, VIEWS) {
                    view.set_chosen(c, NONE);
                }
            }
            _ => {}
        }
        Outcome::Stay
    }
}

impl StadiumScreen {
    /// The choices from the copy (`FUN_0057ce6a`).
    fn choose_from_copy(&mut self) {
        let Some(copy) = &self.copy else { return };
        self.scoreboard = copy.scoreboard();
        self.chosen = COMFORTS.map(|c| copy.has(c));
    }

    /// The places, the building time, the stands, the pitch
    /// (`FUN_0057cf14`, `FUN_0057dac5`).
    fn fill(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(r) = &self.copy else { return };
        for (id, places) in PLACES.iter().zip(r.places()) {
            set(view, dialog, *id, places.to_string());
        }
        let weeks = r.building_weeks();
        let building = match weeks {
            0 => NO_WEEKS.to_owned(),
            1 => game.text(ONE_WEEK).to_owned(),
            w => game.text(WEEKS).replacen("%d", &w.to_string(), 1),
        };
        set(view, dialog, BUILDING, building);
        let ink = |s: usize| Some(if r.weeks(s) != 0 { BUILDING_INK } else { BLACK });
        let places = (0..STANDS)
            .map(|s| format!("{}\t{}\t{}", r.vip(s), r.seats(s), r.standing(s)))
            .collect();
        let conditions =
            (0..STANDS).map(|s| game.text(CONDITIONS + r.condition(s) as u16).to_owned()).collect();
        for (id, rows) in [(STAND_LIST, places), (CONDITION_LIST, conditions)] {
            if let Some(list) = control(dialog, id).and_then(|c| view.list_mut(c)) {
                list.rows = rows;
                list.inks = (0..STANDS).map(ink).collect();
                list.selected = None;
            }
        }
        for s in 0..STANDS {
            let boxes = [
                (HEATER_BOXES[s], r.heaters(s)),
                (ROOF_BOXES[s], r.roof(s)),
                (TRACK_BOXES[s], r.track(s)),
            ];
            for (id, on) in boxes {
                if let Some(b) = control(dialog, id).and_then(|c| view.check_mut(c)) {
                    b.checked = on;
                    b.disabled = true;
                }
            }
        }
        let wear = r.wear();
        let pitch =
            PITCH_TEXTS.iter().find(|&&(below, _)| wear < below).map_or(RUINED_PITCH, |&(_, t)| t);
        set(view, dialog, PITCH, game.text(pitch).to_owned());
        if let Some(c) = control(dialog, TURF) {
            view.set_disabled(c, wear == 0);
        }
    }

    /// The stadiums there are, in both combos (`FUN_0057d366`).
    fn fill_combos(&self, cx: &Cx, view: &mut View) {
        let names: Vec<(usize, String)> = self
            .subs
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind() != 0)
            .map(|(k, s)| (k, s.name()))
            .collect();
        for (id, chosen) in [(CURRENT_COMBO, self.current), (SHOWN_COMBO, self.shown)] {
            if let Some(combo) = control(cx.dialog, id).and_then(|c| view.combo_mut(c)) {
                combo.items = names.iter().map(|(_, n)| n.clone()).collect();
                combo.selected = names.iter().position(|&(k, _)| k == chosen);
            }
        }
    }

    /// The scoreboards, by name only: the price after the tab is not drawn
    /// (`FUN_0057d4bc`).
    fn fill_scoreboard(&self, cx: &Cx, view: &mut View) {
        let game = cx.game;
        if let Some(combo) = control(cx.dialog, SCOREBOARD).and_then(|c| view.combo_mut(c)) {
            combo.items = (0..stadium::SCOREBOARDS.len() as u16)
                .map(|i| game.text(SCOREBOARD_TEXTS + i).to_owned())
                .collect();
            combo.selected = Some(self.scoreboard);
        }
    }

    /// The comfort boxes and what the changes cost (`FUN_0057d64e`): what
    /// the stadium has is ticked and off.
    fn fill_cost(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(r) = &self.copy else { return };
        for ((id, comfort), chosen) in COMFORT_BOXES.iter().zip(COMFORTS).zip(self.chosen) {
            if let Some(b) = control(dialog, *id).and_then(|c| view.check_mut(c)) {
                b.checked = chosen;
                b.disabled = r.has(comfort);
            }
        }
        let (cost, taken_back) = stadium::change(r, &self.chosen, self.scoreboard);
        let net = cost.wrapping_sub(taken_back);
        let label = if net < 0 {
            game.text(INCOME_LABEL).to_owned()
        } else {
            control(dialog, COST_LABEL).map(|c| dialog.controls[c].text.clone()).unwrap_or_default()
        };
        set(view, dialog, COST_LABEL, label);
        set(view, dialog, COST, money(game, i64::from(net.wrapping_abs())));
    }

    /// Another stadium to play in, if it takes the season-ticket holders
    /// (`FUN_0057fd1c`); else the combo goes back.
    fn choose_current(&mut self, cx: &Cx, view: &mut View, c: usize) -> Outcome {
        let ks: Vec<usize> =
            self.subs.iter().enumerate().filter(|(_, s)| s.kind() != 0).map(|(k, _)| k).collect();
        let Some(k) = view.combo(c).and_then(|b| b.selected).and_then(|row| ks.get(row).copied())
        else {
            return Outcome::Stay;
        };
        if k == self.current {
            return Outcome::Stay;
        }
        let capacity = self.subs.get(k).map_or(0, Sub::capacity);
        let early =
            cx.state.records.as_ref().is_some_and(|r| r.globals.half_weeks() <= TICKET_DAYS);
        let holders = own_team(cx).map_or(0, |t| t.get::<i32>(SEASON_TICKETS_AT));
        if early && capacity < holders {
            if let Some(combo) = view.combo_mut(c) {
                combo.selected = ks.iter().position(|&x| x == self.current);
            }
            return Outcome::Popup(Request::with(NOTICE, Args::Text(TOO_SMALL)));
        }
        self.current = k;
        Outcome::Stay
    }

    /// `FUN_00580004`: nothing chosen, nothing; too little money, an
    /// alert; else the question.
    fn carry_out(&mut self, cx: &Cx) -> Outcome {
        let Some(r) = &self.copy else { return Outcome::Stay };
        let (cost, taken_back) = stadium::change(r, &self.chosen, self.scoreboard);
        if cost == 0 && self.scoreboard == r.scoreboard() {
            return Outcome::Stay;
        }
        let investable =
            own_team(cx).zip(cx.state.records.as_ref()).map_or(0, |(team, records)| {
                a3_core::finances::Balance::of(
                    team,
                    records.globals.fund_prices(),
                    &records.agreed(),
                )
                .investable()
            });
        if investable < i64::from(cost.wrapping_sub(taken_back)) {
            return Outcome::Popup(Request::with(NOTICE, Args::Text(TOO_LITTLE)));
        }
        self.asked = Some(Ask::CarryOut);
        Outcome::Popup(Request::with(QUESTION, Args::Text(SURE)))
    }

    /// The copies back to the club (`FUN_0057de04`): the one shown, and
    /// the stadium chosen to play in first.
    fn write_back(&mut self, cx: &mut Cx) {
        if let Some(copy) = &self.copy
            && let Some(sub) = self.subs.get_mut(self.shown)
        {
            *sub = copy.clone();
        }
        if self.current != 0 && self.current < self.subs.len() {
            self.subs.swap(0, self.current);
            if self.shown == 0 {
                self.shown = self.current;
            } else if self.shown == self.current {
                self.shown = 0;
            }
            self.current = 0;
        }
        let Some((country, number)) = own_team(cx).map(|t| (t.country(), u16::from(t.number())))
        else {
            return;
        };
        if let Some(records) = cx.state.records.as_mut() {
            stadium::store(records, country, number, &self.subs);
        }
    }
}

/// The manager's club's team record, to change.
fn own_team_mut<'a>(cx: &'a mut Cx) -> Option<&'a mut a3_core::records::Team> {
    let (country, number) = own_team(cx).map(|t| (t.country(), u16::from(t.number())))?;
    cx.state.records.as_mut()?.team_mut(country.into(), number)
}
