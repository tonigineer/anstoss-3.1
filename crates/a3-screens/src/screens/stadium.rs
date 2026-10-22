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
/// The 2D view, the 3D flight, the expansion (item 2 is dead).
const VIEWS: u16 = 5852;
const TWO_D: usize = 0;
const THREE_D: usize = 1;
const CRANE: usize = 3;
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

/// The bar takes the OK's place; the caption keeps the file's title, as
/// the expansion's does.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    if let Some(dialog) = dialogs.get_mut(&a3_core::cycle::dialogs::EXPANSION) {
        let title = dialog.title.clone();
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, title));
    }
    // The views' picture lies under their controls, cut to the dialog.
    for id in VIEW_DIALOGS {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        let frame = dialog.frame.unwrap_or(bevy_math::Rect::new(0.0, 0.0, 640.0, 480.0));
        let picture = a3_files::rch::Control {
            kind: a3_files::rch::Kind::Bitmap,
            rect: bevy_math::Rect::from_corners(frame.min + PICTURE_AT, frame.max),
            align: a3_files::rch::Align::Left,
            text: String::new(),
            id: Some(VIEW_PICTURE),
            font: 0,
            ink: [0; 3],
        };
        dialog.controls.insert(0, picture);
        let title = dialog.title.clone();
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, title));
    }
    let Some(dialog) = dialogs.get_mut(&STADIUM) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    let title = dialog.title.clone();
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, title));
    hub::add_bar(dialog, rules);
}

/// The views of a stadium (dialogs 249 and 510, `CStadionAnsicht`,
/// `CStadionAnsichtKlein`; STADION-ANSICHT.md in the RE repo): its name
/// and its picture as the grounds map draws it at half size; OK closes.
#[derive(Default)]
pub struct StadiumView;

const LARGE_VIEW: u16 = 249;
const SMALL_VIEW: u16 = 510;
const VIEW_DIALOGS: [u16; 2] = [LARGE_VIEW, SMALL_VIEW];
const VIEW_NAME: u16 = 6450;
const VIEW_PICTURE: u16 = 9939;
/// Where the picture goes in the dialog (the paint hook's 24, 71); it may
/// run over the popup's black edge.
const PICTURE_AT: bevy_math::Vec2 = bevy_math::Vec2::new(24.0, 71.0);

impl Screen for StadiumView {
    fn ids(&self) -> &[u16] {
        &VIEW_DIALOGS
    }

    /// `0x57712d`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        let Some(sub) = cx.state.viewed.clone() else { return };
        set(view, dialog, VIEW_NAME, sub.name());
        let ok = dialog.controls.iter().find(|c| crate::controls::button::is_ok(c)).map(|c| c.rect);
        if let Some(c) = control(dialog, VIEW_PICTURE) {
            let r = dialog.controls[c].rect;
            if let Some(bitmap) = view.bitmap_mut(c) {
                bitmap.picture = Some(crate::controls::bitmap::Picture {
                    source: crate::controls::bitmap::PictureSource::Stadium(stadium::look(&sub)),
                    part: Some([0, 0, r.width() as u32, r.height() as u32]),
                    black_clear: false,
                });
                bitmap.holes = ok.into_iter().collect();
            }
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&cx.dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}

/// A stadium's view (`FUN_0057f670` item 0, `FUN_0057b39a` item 0): a
/// dream stadium still being built on every stand cannot be looked at; a
/// big site's in the large view, else the small one.
fn view_of(cx: &mut Cx, sub: &Sub) -> Outcome {
    if sub.kind() >= 2 && sub.all_building() {
        return Outcome::Popup(Request::with(NOTICE, Args::Text(NOT_FINISHED)));
    }
    cx.state.viewed = Some(sub.clone());
    Outcome::Popup(Request::new(if sub.big_site() { LARGE_VIEW } else { SMALL_VIEW }))
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

#[derive(Clone)]
enum Ask {
    Whistle,
    CarryOut,
    View,
    Expansion,
    /// The builder's offer on a carried plan, at its price; the plan's name.
    Offer(stadium::Carried, i32, String),
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
            Target::Option { control: c, .. } if dialog.controls[c].id == Some(VIEWS) => true,
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
                if item == TWO_D
                    && let Some(copy) = self.copy.clone()
                {
                    self.asked = Some(Ask::View);
                    return view_of(cx, &copy);
                }
                if item == THREE_D {
                    self.asked = Some(Ask::View);
                    return Outcome::Popup(Request::with(NOTICE, Args::Text(NO_3D)));
                }
                // The expansion plans on a copy (`FUN_0057f670`, item 3).
                if item == CRANE
                    && let Some(copy) = &self.copy
                {
                    cx.state.expansion = Some((copy.clone(), self.shown == self.current));
                    self.asked = Some(Ask::Expansion);
                    return Outcome::Popup(Request::new(a3_core::cycle::dialogs::EXPANSION));
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
            // The crane stays chosen until all that follows is done.
            Some(Ask::Expansion) => {
                let plan = cx.state.expansion.take();
                if reply == Reply::Ok
                    && let Some((plan, _)) = plan
                {
                    return self.expanded(cx, view, &plan);
                }
                if let Some(c) = control(cx.dialog, VIEWS) {
                    view.set_chosen(c, NONE);
                }
            }
            Some(Ask::Offer(carried, price, name)) => {
                if reply == Reply::Ok
                    && let (Some(team), Some(copy)) = (own_team_mut(cx), self.copy.as_mut())
                {
                    stadium::take_offer(team, copy, &carried, price);
                }
                self.finish_expansion(cx, view, &carried, &name);
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

    /// The plan OKed (0x57f74f): carried into the stadium and booked, the
    /// builder's offer for a long one, then the screen anew.
    fn expanded(&mut self, cx: &mut Cx, view: &mut View, plan: &Sub) -> Outcome {
        let (Some(team), Some(copy)) = (own_team_mut(cx), self.copy.as_mut()) else {
            return Outcome::Stay;
        };
        let carried = stadium::carry_plan(team, copy, plan);
        if let Some(price) = stadium::builder_offer(&carried) {
            self.asked = Some(Ask::Offer(carried, price, plan.name()));
            let args = Args::BuilderOffer {
                price,
                weeks: carried.weeks,
                total: carried.cost.wrapping_add(price),
            };
            return Outcome::Popup(Request::with(QUESTION, args));
        }
        self.finish_expansion(cx, view, &carried, &plan.name());
        Outcome::Stay
    }

    /// After the plan (0x57f9b4): a dream stadium's choices from the
    /// stadium, its name, the arena mark, the shown stadium kept, the fill.
    fn finish_expansion(
        &mut self,
        cx: &Cx,
        view: &mut View,
        carried: &stadium::Carried,
        name: &str,
    ) {
        if carried.dream {
            self.choose_from_copy();
        }
        if let Some(copy) = self.copy.as_mut() {
            copy.set_name(name);
            stadium::check_arena(copy);
            if let Some(sub) = self.subs.get_mut(self.shown) {
                *sub = copy.clone();
            }
        }
        self.fill(cx, view);
        self.fill_combos(cx, view);
        self.fill_cost(cx, view);
        self.fill_scoreboard(cx, view);
        if let Some(c) = control(cx.dialog, VIEWS) {
            view.set_chosen(c, NONE);
        }
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

/// The stadium's expansion (dialog 463, `CStadionAusbau`;
/// STADION-AUSBAU.md in the RE repo): the places of the stands, their
/// heaters, roofs, tracks and condition planned on a copy of the stadium,
/// with what the plan costs; OK hands the plan back, the stadium screen
/// carries it out and books it.
#[derive(Default)]
pub struct Expansion {
    /// The plan, the stadium as it was, whether it is the one played in,
    /// the dream stadium chosen (0 none).
    plan: Option<Sub>,
    old: Option<Sub>,
    played: bool,
    dream: usize,
    /// Each stand's weighted places at the opening.
    opening: [i32; STANDS],
    /// The rooms for standing places, seats and VIP, weighted.
    rooms: [[i32; 3]; STANDS],
    /// The edits' red ink as the last fill set it, which they show only
    /// at the next (§3.2).
    red: [bool; STANDS],
    /// The places' controls disabled; the plan's cost in all.
    disabled: Vec<u16>,
    total: i64,
}

const NAME_EDIT: u16 = 5862;
const CHOICE: u16 = 5910;
/// The stands' edits and spinners for standing places, seats and VIP.
const PLACE_EDITS: [[u16; 3]; STANDS] =
    [[5890, 5886, 5882], [5891, 5887, 5883], [5892, 5888, 5884], [5893, 5889, 5885]];
const PLACE_SPINS: [[u16; 3]; STANDS] =
    [[5874, 5870, 5878], [5875, 5871, 5879], [5876, 5872, 5880], [5877, 5873, 5881]];
const CONDITION: u16 = 5915;
const STAND_COSTS: [u16; STANDS] = [5863, 5864, 5865, 5866];
const STAND_WORK: [u16; STANDS] = [5911, 5912, 5913, 5914];
const ROOF_COST: u16 = 5867;
const RENOVATION_COST: u16 = 5868;
const TOTAL: u16 = 5869;
const EXPANSION_WEEKS: u16 = 5920;
const INVESTABLE: u16 = 5921;
const EXPANSION_VIEWS: u16 = 5916;
const NONE_SHOWN: &str = "-";
/// The kinds of work: new places, conversion (texts 4257, 4260).
const NEW_WORK: u16 = 4257;
const CONVERSION: u16 = 4260;
/// The most weighted places a stand holds: a small site, a big one, a
/// dream stadium.
const SMALL_SITE: i32 = 6000;
const BIG_SITE: i32 = 50_000;
const DREAM_SITE: i32 = 65_000;
const DREAM_WEEKS: i32 = 100;
const RED: [u8; 3] = [255, 0, 0];
/// OK's refusals: a name too short, a plan far too dear or too dear, a
/// stadium too small for the season-ticket holders, the league, the
/// tickets sold ahead.
const LEAST_NAME: usize = 2;
const NAME_TOO_SHORT: u16 = 4261;
const MADNESS: u16 = 4253;
const NO_MONEY: u16 = 4254;
const BELOW_LEAGUE: u16 = 4810;
const BELOW_PRESALE: u16 = 4811;
const LEAST_CAPACITY: [i32; 3] = [10_000, 5_000, 3_000];
const NOT_FINISHED: u16 = 4996;
/// The views: the 2D one (249/510), the 3D flight.
const VIEW_3D: usize = 1;

impl Screen for Expansion {
    fn ids(&self) -> &[u16] {
        &[a3_core::cycle::dialogs::EXPANSION]
    }

    /// `FUN_005781f5`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        let Some((plan, played)) = cx.state.expansion.clone() else { return };
        set(view, dialog, NAME_EDIT, plan.name());
        self.opening = std::array::from_fn(|s| plan.weighted(s));
        self.played = played;
        if let Some(c) = control(dialog, CHOICE) {
            let dream = usize::try_from(plan.kind() - 1).unwrap_or(0).min(stadium::DREAMS);
            self.dream = dream;
            view.set_chosen(c, dream);
            if let Some(options) = view.options_mut(c) {
                options.dimmed = (0..=stadium::DREAMS)
                    .map(|j| {
                        (dream == 0 && j > 0 && !plan.big_site())
                            || (played && i32::from(plan.kind()) != j as i32 + 1)
                    })
                    .collect();
            }
        }
        if let Some(c) = control(dialog, CONDITION) {
            view.set_chosen(c, NONE);
        }
        self.old = Some(plan.clone());
        self.plan = Some(plan);
        self.fill(cx, view);
        // The first paint comes after the fill: its ink shows at once.
        self.show_ink(cx, view);
        let investable = own_team(cx).zip(cx.state.records.as_ref()).map_or(0, |(t, r)| {
            a3_core::finances::Balance::of(t, r.globals.fund_prices(), &r.agreed()).investable()
        });
        set(view, dialog, INVESTABLE, super::finances::money64(game, investable));
        if let Some(c) = control(dialog, EXPANSION_VIEWS) {
            view.set_chosen(c, NONE);
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        self.typed(cx, view);
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                self.ok(cx, view)
            }
            Target::Button(c) if crate::controls::button::is_cancel(&dialog.controls[c]) => {
                Outcome::Close(Reply::Cancel)
            }
            Target::Spin { control: c, .. } => {
                let spin = id(c);
                if self.disabled.contains(&spin) {
                    self.fill_spins(cx, view);
                    return Outcome::Stay;
                }
                let Some((s, k)) = find(&PLACE_SPINS, spin) else { return Outcome::Stay };
                let v = view.value(c).map_or(0, |v| v.value);
                if let Some(plan) = self.plan.as_mut() {
                    let old = [plan.standing(s), plan.seats(s), plan.vip(s)][k];
                    let new = (v * 10 + i32::from(old % 10)) as u16;
                    match k {
                        0 => plan.set_standing(s, new),
                        1 => plan.set_seats(s, new),
                        _ => plan.set_vip(s, new),
                    }
                }
                self.refill(cx, view);
                Outcome::Stay
            }
            Target::Check(c) => {
                let on = view.check(c).is_some_and(|b| b.checked);
                let b = id(c);
                if let Some(plan) = self.plan.as_mut() {
                    if let Some(s) = HEATER_BOXES.iter().position(|&x| x == b) {
                        plan.set_heaters(s, on);
                    } else if let Some(s) = ROOF_BOXES.iter().position(|&x| x == b) {
                        plan.set_roof(s, on);
                    } else if let Some(s) = TRACK_BOXES.iter().position(|&x| x == b) {
                        plan.set_track(s, on);
                    } else {
                        return Outcome::Stay;
                    }
                }
                self.refill(cx, view);
                Outcome::Stay
            }
            // Each click renovates a step; on "Neu" back to where it was
            // (`FUN_0057a56d`).
            Target::Option { control: c, item } if id(c) == CONDITION => {
                if let (Some(plan), Some(old)) = (self.plan.as_mut(), self.old.as_ref()) {
                    let now = plan.condition(item);
                    let to = if now == 0 { old.condition(item) } else { now - 1 };
                    plan.set_condition(item, to);
                }
                self.refill(cx, view);
                view.set_chosen(c, NONE);
                Outcome::Stay
            }
            Target::Option { control: c, item } if id(c) == CHOICE => {
                self.dream = item;
                self.refill(cx, view);
                Outcome::Stay
            }
            // The view stays chosen while an alert shows (`FUN_0057b39a`).
            Target::Option { control: c, item } if id(c) == EXPANSION_VIEWS => {
                let unfinished =
                    self.dream > 0 && self.plan.as_ref().is_some_and(Sub::all_building);
                if unfinished {
                    return Outcome::Popup(Request::with(NOTICE, Args::Text(NOT_FINISHED)));
                }
                if item == VIEW_3D {
                    return Outcome::Popup(Request::with(NOTICE, Args::Text(NO_3D)));
                }
                // The plan as it is edited, not a dream template.
                if let Some(plan) = self.plan.clone() {
                    cx.state.viewed = Some(plan.clone());
                    let view_id = if plan.big_site() { LARGE_VIEW } else { SMALL_VIEW };
                    return Outcome::Popup(Request::new(view_id));
                }
                view.set_chosen(c, NONE);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }

    /// A popup over the edits shows their ink as it is now; the views
    /// are chosen no more.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, _: Reply) -> Outcome {
        self.show_ink(cx, view);
        if let Some(c) = control(cx.dialog, EXPANSION_VIEWS) {
            view.set_chosen(c, NONE);
        }
        Outcome::Stay
    }

    /// What is typed counts as it is typed.
    fn tick(&mut self, cx: &mut Cx, view: &mut View, _: f32) -> Outcome {
        self.typed(cx, view);
        Outcome::Stay
    }
}

/// Where an id stands in a table of the stands' controls.
fn find(table: &[[u16; 3]; STANDS], id: u16) -> Option<(usize, usize)> {
    (0..STANDS).find_map(|s| table[s].iter().position(|&x| x == id).map(|k| (s, k)))
}

/// A number as `atol` reads it: leading blanks, a sign, the digits.
fn atol(text: &str) -> i32 {
    let t = text.trim_start();
    let (sign, digits) = match t.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, t.strip_prefix('+').unwrap_or(t)),
    };
    let n = digits
        .chars()
        .take_while(char::is_ascii_digit)
        .fold(0i32, |n, c| n.wrapping_mul(10).wrapping_add(c as i32 - '0' as i32));
    sign * n
}

impl Expansion {
    /// The most weighted places a stand may hold.
    fn most(&self) -> i32 {
        if self.dream > 0 {
            DREAM_SITE
        } else if self.plan.as_ref().is_some_and(Sub::big_site) {
            BIG_SITE
        } else {
            SMALL_SITE
        }
    }

    /// A number typed into a stand's field (EN_CHANGE, `0x57a932` …): read
    /// as `atol`, held to the stand's room, and the plan filled anew.
    fn typed(&mut self, cx: &Cx, view: &mut View) {
        if self.dream > 0 {
            return;
        }
        let dialog = cx.dialog;
        let Some(plan) = self.plan.as_mut() else { return };
        let mut changed = false;
        for (s, edits) in PLACE_EDITS.iter().enumerate() {
            for (k, &id) in edits.iter().enumerate() {
                let Some(c) = control(dialog, id) else { continue };
                let shown = [plan.standing(s), plan.seats(s), plan.vip(s)][k];
                if view.text(c) == shown.to_string() {
                    continue;
                }
                let room = self.rooms[s][k];
                let mut v = atol(view.text(c));
                let weigh = [|x| x, stadium::seats_weight, stadium::vip_weight][k];
                let back = [|x| x, stadium::seats_of, stadium::vip_of][k];
                if weigh(v) > room {
                    v = back(room);
                }
                match k {
                    0 => plan.set_standing(s, v as u16),
                    1 => plan.set_seats(s, v as u16),
                    _ => plan.set_vip(s, v as u16),
                }
                changed = true;
            }
        }
        if changed {
            self.refill(cx, view);
        }
    }

    /// A fill after a change: the edits draw at once, in the ink the last
    /// fill set.
    fn refill(&mut self, cx: &Cx, view: &mut View) {
        self.show_ink(cx, view);
        self.fill(cx, view);
    }

    fn show_ink(&self, cx: &Cx, view: &mut View) {
        for (s, edits) in PLACE_EDITS.iter().enumerate() {
            for &id in edits {
                if let Some(e) =
                    control(cx.dialog, id).and_then(|c| view.controls[c].state.as_edit_mut())
                {
                    e.ink = self.red[s].then_some(RED);
                }
            }
        }
    }

    /// `FUN_00578ed1`: the places, the boxes, the stands' costs and kinds,
    /// the weeks and the sums.
    fn fill(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(plan), Some(old)) = (self.plan.clone(), self.old.clone()) else { return };
        let dream = (self.dream > 0)
            .then(|| game.rules.stadium.dreams.get(self.dream - 1).cloned())
            .flatten();
        let shown = dream.clone().unwrap_or_else(|| plan.clone());
        let m64 = |v: i64| super::finances::money64(game, v);
        for s in 0..STANDS {
            let numbers = [shown.standing(s), shown.seats(s), shown.vip(s)];
            for (k, &id) in PLACE_EDITS[s].iter().enumerate() {
                set(view, dialog, id, numbers[k].to_string());
            }
            // The dream branch reads the heaters of the last two crossed.
            let heaters =
                if dream.is_some() && s >= 2 { shown.heaters(5 - s) } else { shown.heaters(s) };
            for (id, on) in [
                (HEATER_BOXES[s], heaters),
                (ROOF_BOXES[s], shown.roof(s)),
                (TRACK_BOXES[s], shown.track(s)),
            ] {
                if let Some(b) = control(dialog, id).and_then(|c| view.check_mut(c)) {
                    b.checked = on;
                }
            }
        }
        self.disabled.clear();
        let mut condition_dim = [false; STANDS];
        let total: i64;
        if let Some(template) = &dream {
            for s in 0..STANDS {
                self.disabled.extend(PLACE_EDITS[s]);
                self.disabled.extend(PLACE_SPINS[s]);
                self.disabled.extend([HEATER_BOXES[s], ROOF_BOXES[s], TRACK_BOXES[s]]);
            }
            set(view, dialog, EXPANSION_WEEKS, weeks_text(cx, DREAM_WEEKS));
            if template.kind() != old.kind() {
                total = stadium::dream_price(template.kind());
                for id in STAND_COSTS.into_iter().chain([ROOF_COST, RENOVATION_COST]) {
                    set(view, dialog, id, NONE_SHOWN.to_owned());
                }
                for id in STAND_WORK {
                    set(view, dialog, id, game.text(NEW_WORK).to_owned());
                }
                condition_dim = [true; STANDS];
            } else {
                let mut renovations = 0i64;
                for (s, dim) in condition_dim.iter_mut().enumerate() {
                    renovations += i64::from(self.fill_stand(cx, view, &plan, &old, s).renovation);
                    *dim = plan.weeks(s) != 0;
                }
                total = renovations;
                set(view, dialog, RENOVATION_COST, m64(total));
                set(view, dialog, ROOF_COST, NONE_SHOWN.to_owned());
            }
        } else {
            let (mut costs, mut renovations, mut roofs, mut weeks) = (0i64, 0i64, 0i64, 0i16);
            for s in 0..STANDS {
                if plan.weeks(s) != 0 {
                    self.disabled.extend(PLACE_EDITS[s]);
                    self.disabled.extend(PLACE_SPINS[s]);
                    self.disabled.extend([HEATER_BOXES[s], ROOF_BOXES[s], TRACK_BOXES[s]]);
                } else if plan.roof(s) {
                    // A roof over VIP places cannot be taken off.
                    if plan.vip(s) != 0 {
                        self.disabled.push(ROOF_BOXES[s]);
                    }
                } else {
                    self.disabled.extend([PLACE_EDITS[s][2], PLACE_SPINS[s][2]]);
                }
                let sc = self.fill_stand(cx, view, &plan, &old, s);
                costs += i64::from(sc.cost);
                renovations += i64::from(sc.renovation);
                roofs += i64::from(sc.roof);
                weeks = weeks.max(sc.weeks);
            }
            set(view, dialog, EXPANSION_WEEKS, weeks_text(cx, weeks.into()));
            total = costs + renovations + roofs;
            set(view, dialog, RENOVATION_COST, m64(renovations));
            set(view, dialog, ROOF_COST, m64(roofs));
        }
        set(view, dialog, TOTAL, m64(total));
        if let Some(options) = control(dialog, CONDITION).and_then(|c| view.options_mut(c)) {
            options.dimmed = condition_dim.to_vec();
        }
        for (s, edits) in PLACE_EDITS.iter().enumerate() {
            for &id in edits.iter() {
                if let Some(e) =
                    control(dialog, id).and_then(|c| view.controls[c].state.as_edit_mut())
                {
                    e.disabled = self.disabled.contains(&id);
                }
            }
            for id in [HEATER_BOXES[s], ROOF_BOXES[s], TRACK_BOXES[s]] {
                if let Some(b) = control(dialog, id).and_then(|c| view.check_mut(c)) {
                    b.disabled = self.disabled.contains(&id);
                }
            }
        }
        self.fill_spins(cx, view);
        self.total = total;
    }

    /// A stand's rooms, cost, kind, ink and condition (`FUN_005786b4`).
    fn fill_stand(
        &mut self,
        cx: &Cx,
        view: &mut View,
        plan: &Sub,
        old: &Sub,
        s: usize,
    ) -> stadium::StandCost {
        let (game, dialog) = (cx.game, cx.dialog);
        let most = self.most();
        let (st, se, vip) =
            (i32::from(plan.standing(s)), i32::from(plan.seats(s)), i32::from(plan.vip(s)));
        let (wse, wv) = (stadium::seats_weight(se), stadium::vip_weight(vip));
        let vip_room = if plan.roof(s) { most - st - wse } else { 0 };
        self.rooms[s] = [most - wse - wv, most - st - wv, vip_room];
        let sc = stadium::stand_cost(plan, old, s);
        set(view, dialog, STAND_COSTS[s], money(game, sc.cost.into()));
        let work = match sc.work {
            stadium::Work::New => game.text(NEW_WORK).to_owned(),
            stadium::Work::Conversion => game.text(CONVERSION).to_owned(),
            stadium::Work::Nothing => String::new(),
        };
        set(view, dialog, STAND_WORK[s], work);
        self.red[s] = plan.weighted(s) < self.opening[s];
        if let Some(options) = control(dialog, CONDITION).and_then(|c| view.options_mut(c))
            && let Some(label) = options.labels.get_mut(s)
        {
            *label = game.text(CONDITIONS + plan.condition(s) as u16).to_owned();
        }
        sc
    }

    /// The spinners' ranges by the rooms, at the places as they are
    /// (§3.2): a stand above its most keeps its place, which the first
    /// step brings into the range.
    fn fill_spins(&self, cx: &Cx, view: &mut View) {
        let Some(plan) = &self.plan else { return };
        for (s, spins) in PLACE_SPINS.iter().enumerate() {
            let places = [plan.standing(s), plan.seats(s), plan.vip(s)];
            let rooms = self.rooms[s];
            let most =
                [rooms[0] / 10, stadium::seats_of(rooms[1] / 10), stadium::vip_of(rooms[2] / 10)];
            for ((&id, most), places) in spins.iter().zip(most).zip(places) {
                if let Some(v) = control(cx.dialog, id).and_then(|c| view.value_mut(c)) {
                    v.range = (0, most);
                    v.value = i32::from(places / 10);
                }
            }
        }
    }

    /// OK (`0x57b5fc`): a name, the money, a stadium played in big enough;
    /// then the plan back to the stadium screen.
    fn ok(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let notice = |args| Outcome::Popup(Request::with(NOTICE, args));
        let dialog = cx.dialog;
        let name = control(dialog, NAME_EDIT).map(|c| view.text(c).to_owned()).unwrap_or_default();
        if name.chars().count() < LEAST_NAME {
            return notice(Args::Text(NAME_TOO_SHORT));
        }
        let investable = own_team(cx).zip(cx.state.records.as_ref()).map_or(0, |(t, r)| {
            a3_core::finances::Balance::of(t, r.globals.fund_prices(), &r.agreed()).investable()
        });
        if self.total != 0 && investable < self.total {
            let text = if investable < self.total / 10 { MADNESS } else { NO_MONEY };
            return notice(Args::Text(text));
        }
        let (Some(mut plan), Some(old)) = (self.plan.clone(), self.old.clone()) else {
            return Outcome::Close(Reply::Cancel);
        };
        if self.played {
            if let Some(refusal) = self.too_small(cx, &plan, &old) {
                return notice(refusal);
            }
        } else if self.dream > 0
            && let Some(template) = cx.game.rules.stadium.dreams.get(self.dream - 1)
        {
            plan = template.clone();
        }
        plan.set_name(&name);
        cx.state.expansion = Some((plan, self.played));
        Outcome::Close(Reply::Ok)
    }

    /// The capacity check of a stadium played in (`FUN_0071c08d` with the
    /// places left while building): the league's least, the season-ticket
    /// holders early in the season, the tickets sold ahead.
    fn too_small(&self, cx: &Cx, plan: &Sub, old: &Sub) -> Option<Args> {
        let records = cx.state.records.as_ref()?;
        let team = own_team(cx)?;
        let places = |sub: &Sub, s: usize| {
            i32::from(sub.standing(s)) + i32::from(sub.seats(s)) + i32::from(sub.vip(s))
        };
        let same = |s: usize| {
            [plan.standing(s), plan.seats(s), plan.vip(s)]
                == [old.standing(s), old.seats(s), old.vip(s)]
                && plan.roof(s) == old.roof(s)
        };
        let during: i32 =
            (0..STANDS).filter(|&s| plan.weeks(s) == 0 && same(s)).map(|s| places(plan, s)).sum();
        let before: i32 = (0..STANDS).filter(|&s| old.weeks(s) == 0).map(|s| places(old, s)).sum();
        let least = LEAST_CAPACITY[usize::from(team.level()).min(LEAST_CAPACITY.len() - 1)];
        if before >= least && during < least {
            let total = |sub: &Sub| (0..STANDS).map(|s| places(sub, s)).sum::<i32>();
            return (total(plan) <= total(old)).then_some(Args::TextForNumber(BELOW_LEAGUE, least));
        }
        let holders = team.get::<i32>(SEASON_TICKETS_AT);
        if records.globals.half_weeks() <= TICKET_DAYS && during < holders {
            return Some(Args::Text(TOO_SMALL));
        }
        let club = (team.country(), i16::from(team.number()));
        let today = records.globals.day() as i16;
        a3_core::tickets::presold_above(records, club, today, during)
            .then_some(Args::Text(BELOW_PRESALE))
    }
}

/// The building time: "eine Woche", "%d Wochen", or "-" (`FUN_00578e2f`).
fn weeks_text(cx: &Cx, weeks: i32) -> String {
    match weeks {
        ..=0 => NONE_SHOWN.to_owned(),
        1 => cx.game.text(ONE_WEEK).to_owned(),
        w => cx.game.text(WEEKS).replacen("%d", &w.to_string(), 1),
    }
}
