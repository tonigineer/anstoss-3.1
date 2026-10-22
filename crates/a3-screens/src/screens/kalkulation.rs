//! The calculations behind the finances' rows (KALKULATION.md in the RE
//! repo): a dialog for each row, paged through in the rows' order by an
//! arrow bar at the bottom of every one; opened from the finances, they
//! show the season's plan beside what is booked, their plan sliders
//! hidden.

use std::collections::HashMap;

use a3_core::finances::Accounts;
use a3_files::rch::Dialog;

use super::playerinfo::money;
use super::squad::{own_team, own_team_mut};
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::bitmap::Picture;
use crate::pointer::Target;
use crate::view::View;

const CAPTION_ID: u16 = 9935;
/// Each row's dialog, in the rows' order (the table at 0x881090 gives
/// their kinds): tickets, TV, prizes, sponsors, merchandise, other income,
/// transfers, staff, players, the running costs, the stadium, other costs,
/// the shares.
pub(crate) const ROW_DIALOGS: [u16; 13] =
    [137, 448, 450, 125, 115, 451, 458, 447, 454, 459, 457, 122, 446];
/// The six that show a figure or two per item, with a slider each for the
/// plan.
const TV: u16 = 448;
const PRIZES: u16 = 450;
const OTHER_INCOME: u16 = 451;
const TRANSFERS: u16 = 458;
const STADIUM: u16 = 457;
const SHARES: u16 = 446;
const FIGURE_DIALOGS: [u16; 6] = [TV, PRIZES, OTHER_INCOME, TRANSFERS, STADIUM, SHARES];
const STAFF: u16 = 447;
const PLAYERS: u16 = 454;
const AMATEUR_PLAYERS: u16 = 453;
const RUNNING: u16 = 459;
const OTHER_COSTS: u16 = 122;
/// The dialogs built so far, which the carousel stops at.
pub(crate) const BUILT: [u16; 13] = [
    TICKETS,
    TV,
    PRIZES,
    SPONSORS,
    MERCHANDISE,
    OTHER_INCOME,
    TRANSFERS,
    STADIUM,
    SHARES,
    STAFF,
    PLAYERS,
    RUNNING,
    OTHER_COSTS,
];
/// The merchandise's row.
const MERCHANDISE_ROW: usize = 4;
/// The arrow bar: back gives 7, on gives 6 (`FUN_004a43cb`).
const CAROUSEL: u16 = 5671;
pub(crate) const NEXT: usize = 6;
pub(crate) const PREVIOUS: usize = 7;
const CAROUSEL_RANGE: (i32, i32) = (0, 2);
const CANCEL: u16 = 2;
/// Club offsets c from 0x120 to 0x5f7 lie at c + 0x278 in the team record.
const CLUB_TO_TEAM: usize = 0x278;

/// Their captions: the ticket office's, the sponsors', the agency's and
/// the contract's the file's.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let more = [STAFF, PLAYERS, AMATEUR_PLAYERS, RUNNING, OTHER_COSTS, MERCHANDISE];
    let titled = [TICKETS, SPONSORS, AGENCY_ENGAGED, SPONSOR_CONTRACT];
    for id in FIGURE_DIALOGS.into_iter().chain(more).chain(titled) {
        if let Some(dialog) = dialogs.get_mut(&id) {
            let text = if titled.contains(&id) { dialog.title.clone() } else { String::new() };
            dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, text));
        }
    }
}

/// The dialog of a row: the players' of an amateur department is 453
/// (team +0x872 set); the merchandise, with an agency engaged, tells so.
pub(crate) fn row_dialog(cx: &Cx, row: usize) -> u16 {
    let dialog = ROW_DIALOGS[row];
    if dialog == PLAYERS && own_team(cx).is_some_and(|t| t.linked()[0] >= 0) {
        AMATEUR_PLAYERS
    } else if row == MERCHANDISE_ROW && agency_engaged(cx) {
        AGENCY_ENGAGED
    } else {
        dialog
    }
}

/// The next row's dialog that is built, on or back from a row, if any;
/// the merchandise is stepped past while an agency has it.
pub(crate) fn step(cx: &Cx, row: usize, choice: usize) -> Option<usize> {
    let rows = ROW_DIALOGS.len();
    let engaged = agency_engaged(cx);
    (1..=rows)
        .map(|k| if choice == NEXT { (row + k) % rows } else { (row + rows - k) % rows })
        .filter(|&r| !(engaged && r == MERCHANDISE_ROW))
        .find(|&r| BUILT.contains(&ROW_DIALOGS[r]))
}

/// The caption: the file's, the manager's names and the club, no date
/// (`"%s %s %s (%s)"`).
fn caption(cx: &Cx) -> String {
    let (game, dialog) = (cx.game, cx.dialog);
    let Some(season) = cx.state.season.as_ref() else { return dialog.title.clone() };
    let Some(manager) = season.manager() else { return dialog.title.clone() };
    let club = manager.club.and_then(|(slot, c)| game.world.country(slot)?.clubs.get(c));
    let name = club.map_or_else(String::new, |c| c.name.clone());
    format!("{} {} {} ({name})", dialog.title, manager.first_name, manager.last_name)
}

/// What every calculation does at its opening: the caption, the arrow bar
/// in its middle, the plan's sliders hidden.
fn open_calculation(cx: &Cx, view: &mut View, sliders: &[u16]) {
    let dialog = cx.dialog;
    set(view, dialog, CAPTION_ID, caption(cx));
    carousel(view, dialog);
    for &id in sliders {
        if let Some(c) = control(dialog, id) {
            view.controls[c].hidden = true;
        }
    }
}

/// How a calculation is left: by OK or the arrow bar, which write what it
/// changed, or by cancel, which drops it.
fn closing(cx: &Cx, view: &View, target: Target) -> Option<(Outcome, bool)> {
    let outcome = carousel_input(cx, view, target)?;
    Some((outcome, outcome != Outcome::Close(Reply::Cancel)))
}

/// The arrow bar, OK and cancel, as every calculation answers them.
fn carousel_input(cx: &Cx, view: &View, target: Target) -> Option<Outcome> {
    let dialog = cx.dialog;
    let id = |c: usize| dialog.controls[c].id;
    match target {
        Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
            Some(Outcome::Close(Reply::Ok))
        }
        Target::Button(c) if id(c) == Some(CANCEL) => Some(Outcome::Close(Reply::Cancel)),
        Target::SliderStep { control: c, up } if id(c) == Some(CAROUSEL) => {
            Some(Outcome::Close(Reply::Choice(if up { NEXT } else { PREVIOUS })))
        }
        // A press on the track goes the way of the handle's side.
        Target::SliderAt { control: c, .. } if id(c) == Some(CAROUSEL) => {
            let on = view.value(c).is_some_and(|v| v.value > 1);
            Some(Outcome::Close(Reply::Choice(if on { NEXT } else { PREVIOUS })))
        }
        _ => None,
    }
}

/// A figure: the plan's, the booked one's, a club's field, or nothing.
#[derive(Clone, Copy)]
enum Figure {
    Plan(usize),
    Plan64(usize),
    Booked(usize),
    Booked64(usize),
    /// A field of the club, i64 (the transfers' sums).
    Club64(usize),
    /// The difference of two i64 fields of the club.
    ClubDifference(usize, usize),
    /// Last season's stadium costs, read live.
    LastStadium,
    Nothing,
}

/// TV, prizes, other income, transfers, the stadium and the shares
/// (§2): the plan's figure and the booked one per item, the sliders hidden.
#[derive(Default)]
pub struct Figures;

/// Each dialog's statics and what they show, and its sliders.
fn figures(id: u16) -> (&'static [(u16, Figure)], &'static [u16]) {
    use Figure::*;
    match id {
        TV => (
            &[(5706, Plan(0x18)), (5707, Booked(0x18)), (5156, Plan(0x1c)), (5710, Booked(0x1c))],
            &[5708, 5709],
        ),
        PRIZES => (
            &[
                (5711, Plan(0x20)),
                (5714, Nothing),
                (5712, Booked(0x20)),
                (5715, Plan(0x24)),
                (5718, Nothing),
                (5716, Booked(0x24)),
                (5719, Plan(0x28)),
                (5720, Booked(0x28)),
            ],
            &[5713, 5717, 5721],
        ),
        OTHER_INCOME => (
            &[
                (5722, Plan(0x2c)),
                (5723, Booked(0x2c)),
                (6177, Booked(0xa4)),
                (6178, Booked(0xec)),
                (6179, Booked(0x70)),
            ],
            &[5724],
        ),
        TRANSFERS => (
            &[
                (5749, Plan(0x30)),
                (5750, Club64(0x470)),
                (5753, Club64(0x478)),
                (5755, ClubDifference(0x470, 0x478)),
                (5751, Club64(0x480)),
                (5754, Club64(0x488)),
                (5756, ClubDifference(0x480, 0x488)),
            ],
            &[5752],
        ),
        STADIUM => (
            &[
                (5739, Plan64(0xb0)),
                (5740, Booked64(0xb0)),
                (5742, Plan64(0xc0)),
                (5744, Booked64(0xc0)),
                (5745, Plan(0xc8)),
                (5746, Booked(0xc8)),
                (5748, LastStadium),
            ],
            &[5741, 5743, 5747],
        ),
        SHARES => (
            &[(5669, Plan(0xa8)), (5670, Booked(0xa8)), (5673, Plan(0xac)), (5675, Booked(0xac))],
            &[5672, 5674],
        ),
        _ => (&[], &[]),
    }
}

impl Screen for Figures {
    fn ids(&self) -> &[u16] {
        &FIGURE_DIALOGS
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (statics, sliders) = figures(view.id);
        open_calculation(cx, view, sliders);
        let Some(team) = super::squad::own_team(cx) else { return };
        let plan = Accounts::plan(team);
        let booked = Accounts::booked(team);
        let last = super::finances::own_extra(game, cx).map(|(_, e)| Accounts::last_season_kept(e));
        let club64 = |c: usize| team.get::<i64>(c + CLUB_TO_TEAM);
        let money64 = |v: i64| super::finances::money64(game, v);
        for &(id, figure) in statics {
            let text = match figure {
                Figure::Plan(o) => money(game, plan.i32_(o).into()),
                Figure::Plan64(o) => money64(plan.i64_(o)),
                Figure::Booked(o) => money(game, booked.i32_(o).into()),
                Figure::Booked64(o) => money64(booked.i64_(o)),
                Figure::Club64(c) => money64(club64(c)),
                Figure::ClubDifference(a, b) => money64(club64(a).wrapping_sub(club64(b))),
                Figure::LastStadium => money64(last.as_ref().map_or(0, Accounts::stadium)),
                Figure::Nothing => money(game, 0),
            };
            set(view, dialog, id, text);
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        carousel_input(cx, view, target).unwrap_or(Outcome::Stay)
    }
}

/// The booked accounts (club +0x228) and the plan (club +0x120), by their
/// field, in the team record.
fn booked_at(o: usize) -> usize {
    0x228 + o + CLUB_TO_TEAM
}

fn set_picture(view: &mut View, dialog: &Dialog, id: u16, graphic: u16) {
    if let Some(bitmap) = control(dialog, id).and_then(|c| view.bitmap_mut(c)) {
        bitmap.picture = Some(Picture::graphic(graphic));
    }
}

/// The staff's wages (dialog 447, §6): booked beside planned per post;
/// from the finances only the groundsman can be raised, by his spinner or
/// his picture.
#[derive(Default)]
pub struct Staff {
    /// The groundsman's level and wage as changed, and the level he had.
    level: u8,
    wage: i32,
    floor: u8,
    planned_level: u8,
}

/// The posts: booked figure, planned figure, their field.
const STAFF_POSTS: [(u16, u16, usize); 7] = [
    (5676, 5677, 0x38),
    (5678, 5679, 0x3c),
    (5680, 5681, 0x40),
    (5686, 5687, 0x54),
    (5688, 5689, 0x44),
    (5694, 5695, 0x48),
    (5690, 5691, 0x4c),
];
/// The fitness coaches: their count's field, wage each.
const COACHES: usize = 0xd8;
const COACH_WAGE: i32 = 80_000;
const COACH_FIGURES: (u16, u16, u16) = (5682, 5683, 5684);
/// The groundsman: level and wage fields, the figures, his spinner and
/// picture, the wages and pictures by level.
const GROUNDSMAN: usize = 0xd9;
const GROUNDSMAN_WAGE: usize = 0x50;
const GROUNDSMAN_FIGURES: (u16, u16) = (5692, 5693);
const GROUNDSMAN_SPIN: u16 = 5704;
const GROUNDSMAN_PICTURE: u16 = 5685;
const GROUNDSMAN_WAGES: [i32; 3] = [12_000, 24_000, 72_000];
const GROUNDSMAN_PICTURES: [u16; 3] = [2940, 2941, 2942];
/// The plan's spinners, hidden from the finances.
const STAFF_SPINS: [u16; 9] = [5696, 5697, 5698, 5699, 5700, 5701, 5702, 5703, 5705];
/// The staff's sum the posts add up to (B+0x58), from the first post on.
const STAFF_SUM: usize = 0x58;
const STAFF_FIRST: usize = 0x34;

impl Screen for Staff {
    fn ids(&self) -> &[u16] {
        &[STAFF]
    }

    /// `FUN_004e54ac` in view mode.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        open_calculation(cx, view, &STAFF_SPINS);
        let Some(team) = own_team(cx) else { return };
        let (plan, booked) = (Accounts::plan(team), Accounts::booked(team));
        for (ist, kalk, o) in STAFF_POSTS {
            set(view, dialog, ist, money(game, booked.i32_(o).into()));
            set(view, dialog, kalk, money(game, plan.i32_(o).into()));
        }
        let count = |a: &Accounts| i32::from(a.i32_(COACHES) as u8);
        set(view, dialog, COACH_FIGURES.0, money(game, (count(&booked) * COACH_WAGE).into()));
        set(view, dialog, COACH_FIGURES.1, money(game, (count(&plan) * COACH_WAGE).into()));
        set(view, dialog, COACH_FIGURES.2, count(&plan).to_string());
        self.level = (booked.i32_(GROUNDSMAN) as u8).min(2);
        self.floor = self.level;
        self.planned_level = (plan.i32_(GROUNDSMAN) as u8).min(2);
        self.wage = booked.i32_(GROUNDSMAN_WAGE);
        set(view, dialog, GROUNDSMAN_FIGURES.0, money(game, self.wage.into()));
        if let Some(spin) = control(dialog, GROUNDSMAN_SPIN).and_then(|c| view.value_mut(c)) {
            spin.range = (i32::from(self.floor), 2);
            spin.set(self.level.into());
        }
        if let Some(c) = control(dialog, GROUNDSMAN_PICTURE) {
            view.set_clickable(c, true);
        }
        self.show(cx, view);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some((outcome, write)) = closing(cx, view, target) {
            if write {
                self.write(cx);
            }
            return outcome;
        }
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Spin { control: c, .. } if id(c) == Some(GROUNDSMAN_SPIN) => {
                let v = view.value(c).map_or(0, |v| v.value).max(self.floor.into()) as u8;
                self.level = v;
                self.wage = GROUNDSMAN_WAGES[usize::from(v)];
                self.show(cx, view);
            }
            // A click on him raises him, round to where he was; his wage
            // the plan's level's — the game's slip.
            Target::Control(c) if id(c) == Some(GROUNDSMAN_PICTURE) => {
                self.level = if self.level + 1 >= 3 { self.floor } else { self.level + 1 };
                self.wage = GROUNDSMAN_WAGES[usize::from(self.planned_level)];
                if let Some(spin) = control(dialog, GROUNDSMAN_SPIN).and_then(|s| view.value_mut(s))
                {
                    spin.set(self.level.into());
                }
                self.show(cx, view);
            }
            _ => {}
        }
        Outcome::Stay
    }
}

impl Staff {
    fn show(&self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        set(view, dialog, GROUNDSMAN_FIGURES.1, money(cx.game, self.wage.into()));
        set_picture(view, dialog, GROUNDSMAN_PICTURE, GROUNDSMAN_PICTURES[usize::from(self.level)]);
    }

    /// OK and the arrow bar (`0x4e6638`): the groundsman's level and wage,
    /// the staff's sum anew (`FUN_00674568`).
    fn write(&self, cx: &mut Cx) {
        let (level, wage) = (self.level, self.wage);
        let Some(team) = own_team_mut(cx) else { return };
        team.put(booked_at(GROUNDSMAN), level);
        team.put(booked_at(GROUNDSMAN_WAGE), wage);
        let posts = (STAFF_FIRST..=0x54)
            .step_by(4)
            .fold(0i32, |s, o| s.wrapping_add(team.get::<i32>(booked_at(o))));
        let coaches = i32::from(team.get::<u8>(booked_at(COACHES))) * COACH_WAGE;
        team.put(booked_at(STAFF_SUM), posts.wrapping_add(coaches));
    }
}

/// The players' costs (dialogs 454 and 453, §7): wages, fines, team
/// bonuses, pre-contracts and, for a club of its own, its support of its
/// amateurs and its youth, which the finances can raise.
#[derive(Default)]
pub struct Players {
    /// The supports as raised, and the salary cap the plan takes.
    amateurs: i32,
    youth: i32,
    cap: Option<i32>,
}

const WAGES_FIGURES: (u16, u16) = (5729, 5731);
const FINES_FIGURES: (u16, u16) = (5726, 5728);
const BONUSES_FIGURES: (u16, u16) = (2941, 5725);
const PRE_CONTRACT_FIGURES: (u16, u16) = (5733, 5735);
const AMATEUR_FIGURES: (u16, u16) = (5736, 5738);
const YOUTH_FIGURES: (u16, u16) = (6180, 6181);
const AMATEUR_SLIDER: u16 = 5737;
const YOUTH_SLIDER: u16 = 6182;
const PLAYER_SLIDERS: [u16; 3] = [5730, 5727, 5734];
/// The team bonuses' dialog (118), not built.
const DETAILS: u16 = 5732;
const PLANNED_WAGES: usize = 0x5c;
const FINES: usize = 0x64;
const TEAM_BONUSES: usize = 0x60;
const PRE_CONTRACTS: usize = 0x68;
const AMATEUR_SUPPORT: usize = 0x6c;
const YOUTH_SUPPORT: usize = 0x88;
const SUPPORT_MOST: i64 = 5_000_000;
/// A salary cap: the club's flag (team +0x12b bit 0x40) and its amount in
/// 100,000s (club +0x5da).
const CAP_FLAGS: usize = 0x12b;
const CAPPED: u8 = 0x40;
const CAP_AT: usize = 0x5da;
const CAP_UNIT: i32 = 100_000;

impl Screen for Players {
    fn ids(&self) -> &[u16] {
        &[PLAYERS, AMATEUR_PLAYERS]
    }

    /// `FUN_004ede84` (453: `FUN_004ece48`) in view mode.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        open_calculation(cx, view, &PLAYER_SLIDERS);
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let (plan, booked) = (Accounts::plan(team), Accounts::booked(team));
        let teams = i32::from(a3_core::tickets::league_teams(records, team));
        let mut bill = a3_core::finances::wage_bill(records, team, teams);
        let capped = team.get::<u8>(CAP_FLAGS) & CAPPED != 0;
        let cap =
            if capped { i32::from(team.get::<u16>(CAP_AT + CLUB_TO_TEAM)) * CAP_UNIT } else { 0 };
        let mut planned = plan.i32_(PLANNED_WAGES);
        if cap != 0 && cap < bill {
            bill = cap;
            planned = cap;
            self.cap = Some(cap);
        }
        let m = |v: i32| money(game, v.into());
        set(view, dialog, WAGES_FIGURES.0, m(planned));
        set(view, dialog, WAGES_FIGURES.1, m(bill));
        set(view, dialog, FINES_FIGURES.0, m(plan.i32_(FINES)));
        set(view, dialog, FINES_FIGURES.1, m(booked.i32_(FINES)));
        set(
            view,
            dialog,
            BONUSES_FIGURES.0,
            m(a3_core::finances::planned_bonuses(records, team, teams)),
        );
        set(view, dialog, BONUSES_FIGURES.1, m(booked.i32_(TEAM_BONUSES)));
        if view.id == PLAYERS {
            set(view, dialog, PRE_CONTRACT_FIGURES.0, m(plan.i32_(PRE_CONTRACTS)));
            set(view, dialog, PRE_CONTRACT_FIGURES.1, m(booked.i32_(PRE_CONTRACTS)));
            self.amateurs = booked.i32_(AMATEUR_SUPPORT);
            self.youth = booked.i32_(YOUTH_SUPPORT);
            let steps = a3_core::finances::steps(SUPPORT_MOST);
            let most = a3_core::finances::step_of(steps, SUPPORT_MOST);
            for (slider, v) in [(AMATEUR_SLIDER, self.amateurs), (YOUTH_SLIDER, self.youth)] {
                if let Some(bar) = control(dialog, slider).and_then(|c| view.value_mut(c)) {
                    let at = a3_core::finances::step_of(steps, v.into());
                    bar.range = (at, most);
                    bar.set(at);
                }
            }
            set(view, dialog, AMATEUR_FIGURES.1, m(self.amateurs));
            set(view, dialog, YOUTH_FIGURES.1, m(self.youth));
            self.show(cx, view);
        }
        mark_inert(view, dialog, |t| !is_button(dialog, t, DETAILS));
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some((outcome, write)) = closing(cx, view, target) {
            if write && view.id == PLAYERS {
                self.write(cx);
            }
            return outcome;
        }
        let id = |c: usize| dialog.controls[c].id;
        let steps = a3_core::finances::steps(SUPPORT_MOST);
        match target {
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. }
                if matches!(id(c), Some(AMATEUR_SLIDER | YOUTH_SLIDER)) =>
            {
                let v = a3_core::finances::amount_at(steps, view.value(c).map_or(0, |v| v.value))
                    as i32;
                if id(c) == Some(AMATEUR_SLIDER) {
                    self.amateurs = v;
                } else {
                    self.youth = v;
                }
                self.show(cx, view);
            }
            _ => {}
        }
        Outcome::Stay
    }
}

impl Players {
    fn show(&self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        set(view, dialog, AMATEUR_FIGURES.0, money(cx.game, self.amateurs.into()));
        set(view, dialog, YOUTH_FIGURES.0, money(cx.game, self.youth.into()));
    }

    /// OK and the arrow bar (`FUN_004eec19`): the supports booked at once,
    /// a salary cap into the plan.
    fn write(&self, cx: &mut Cx) {
        let (amateurs, youth, cap) = (self.amateurs, self.youth, self.cap);
        let Some(team) = own_team_mut(cx) else { return };
        team.put(booked_at(AMATEUR_SUPPORT), amateurs);
        team.put(booked_at(YOUTH_SUPPORT), youth);
        if let Some(cap) = cap {
            team.put(0x120 + PLANNED_WAGES + CLUB_TO_TEAM, cap);
        }
    }
}

/// The running costs (dialog 459, §8): the squad's insurance, extra
/// insurance, travel and the medical department; from the finances a
/// click insures a player or takes his extra insurance off at once, and
/// the medical budget can be raised.
#[derive(Default)]
pub struct Running {
    medical: i32,
}

const INSURANCE_LIST: u16 = 2387;
const COMPULSORY: u16 = 2388;
const EXTRA_SUM: u16 = 2389;
const EXTRA_FIGURES: (u16, u16) = (5757, 5758);
const TRAVEL_FIGURES: (u16, u16) = (5760, 5762);
const MEDICAL_FIGURES: (u16, u16) = (5763, 5764);
const MEDICAL_SLIDER: u16 = 5765;
const MEDICAL_MOST: i64 = 2_000_000;
const RUNNING_SLIDERS: [u16; 2] = [5759, 5761];
const TRAVEL_PICTURE: u16 = 5685;
const TRAVEL_PICTURES: [u16; 3] = [2943, 2944, 2945];
const PACKAGE: u16 = 5766;
const EXTRA: usize = 0x78;
const TRAVEL: [usize; 2] = [0x7c, 0x80];
/// The medical department's budget (B+0x84, club +0x2ac).
const MEDICAL_BUDGET: usize = 0x84;
const TRAVEL_MODE: usize = 0xda;
const PACKAGE_TAKEN: usize = 0xdb;
/// R+0x1f4: a player's extra insurance a year, in thousands.
const EXTRA_INSURANCE: usize = 0x1f4;
/// The risk icon, drawn on its dark square; the knocks that make a
/// player a risk (R+0x218 & 0xc).
const RISK_ICON: u16 = 1275;
const RISKY: u16 = 0xc;
const NOT_INSURED: u16 = 3127;
/// The compulsory insurance of a player: 120 for each thousand of salary,
/// in hundreds.
const INSURANCE_RATE: i64 = 120;

impl Screen for Running {
    fn ids(&self) -> &[u16] {
        &[RUNNING]
    }

    /// `FUN_004f2119` in view mode.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        open_calculation(cx, view, &RUNNING_SLIDERS);
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let (plan, booked) = (Accounts::plan(team), Accounts::booked(team));
        let m = |v: i32| money(game, v.into());
        set(view, dialog, COMPULSORY, m(a3_core::finances::compulsory_insurance(records, team)));
        set(view, dialog, EXTRA_FIGURES.0, m(plan.i32_(EXTRA)));
        set(view, dialog, EXTRA_FIGURES.1, m(booked.i32_(EXTRA)));
        let travel = |a: &Accounts| a.i32_(TRAVEL[0]).wrapping_add(a.i32_(TRAVEL[1]));
        set(view, dialog, TRAVEL_FIGURES.0, m(travel(&plan)));
        set(view, dialog, TRAVEL_FIGURES.1, m(travel(&booked)));
        self.medical = booked.i32_(MEDICAL_BUDGET);
        set(view, dialog, MEDICAL_FIGURES.1, m(self.medical));
        let steps = a3_core::finances::steps(MEDICAL_MOST);
        if let Some(bar) = control(dialog, MEDICAL_SLIDER).and_then(|c| view.value_mut(c)) {
            let at = a3_core::finances::step_of(steps, self.medical.into());
            bar.range = (at, a3_core::finances::step_of(steps, MEDICAL_MOST));
            bar.set(at);
        }
        set(view, dialog, MEDICAL_FIGURES.0, m(self.medical));
        let mode = usize::from(plan.i32_(TRAVEL_MODE) as u8).min(2);
        set_picture(view, dialog, TRAVEL_PICTURE, TRAVEL_PICTURES[mode]);
        if let Some(c) = control(dialog, PACKAGE) {
            let taken = plan.i32_(PACKAGE_TAKEN) as u8 != 0;
            view.set_chosen(c, if taken { 0 } else { crate::controls::options::NONE });
        }
        fill_insurance(cx, view);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some((outcome, write)) = closing(cx, view, target) {
            if write {
                let medical = self.medical;
                if let Some(team) = own_team_mut(cx) {
                    team.put(booked_at(MEDICAL_BUDGET), medical);
                }
            }
            return outcome;
        }
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. }
                if id(c) == Some(MEDICAL_SLIDER) =>
            {
                let steps = a3_core::finances::steps(MEDICAL_MOST);
                self.medical =
                    a3_core::finances::amount_at(steps, view.value(c).map_or(0, |v| v.value))
                        as i32;
                set(view, dialog, MEDICAL_FIGURES.0, money(cx.game, self.medical.into()));
            }
            // The package is not to be had from the finances: it stays as
            // it was.
            Target::Option { control: c, .. } if id(c) == Some(PACKAGE) => {
                let taken =
                    own_team(cx).is_some_and(|t| Accounts::plan(t).i32_(PACKAGE_TAKEN) as u8 != 0);
                view.set_chosen(c, if taken { 0 } else { crate::controls::options::NONE });
            }
            // A click insures him or takes his extra insurance off, at once
            // and for good (`0x4f2a12`); the injured are refused.
            Target::Row { control: c, row } if id(c) == Some(INSURANCE_LIST) => {
                let taken =
                    own_team(cx).is_some_and(|t| Accounts::plan(t).i32_(PACKAGE_TAKEN) as u8 != 0);
                let Some(player) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied())
                else {
                    return Outcome::Stay;
                };
                if taken {
                    return Outcome::Stay;
                }
                let injured = own_club(cx).is_some_and(|club| {
                    club.status.iter().any(|s| usize::from(s.id) == player && s.injured > 0)
                });
                if injured {
                    return Outcome::Popup(a3_core::cycle::Request::with(
                        a3_core::cycle::dialogs::NOTICE,
                        Args::Text(NOT_INSURED),
                    ));
                }
                if let Some(r) = cx.state.records.as_mut().and_then(|r| r.players.get_mut(player)) {
                    let premium = if r.get::<i16>(EXTRA_INSURANCE) == 0 {
                        a3_core::finances::insurance_premium(r.salary(), r.knocks())
                    } else {
                        0
                    };
                    r.put(EXTRA_INSURANCE, premium);
                }
                fill_insurance(cx, view);
            }
            _ => {}
        }
        Outcome::Stay
    }
}

/// The manager's club's state.
fn own_club<'a>(cx: &'a Cx) -> Option<&'a a3_core::season::ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

/// The insurance list (`FUN_004f28d5`): the fit players, then the
/// injured, in the squad's order; their compulsory and extra insurance;
/// the risk icon; the extra insurance's sum.
fn fill_insurance(cx: &Cx, view: &mut View) {
    let (game, dialog) = (cx.game, cx.dialog);
    let (Some(records), Some(club), Some(season)) =
        (cx.state.records.as_ref(), own_club(cx), cx.state.season.as_ref())
    else {
        return;
    };
    let Some(c) = control(dialog, INSURANCE_LIST) else { return };
    let slots = crate::controls::list::icon_slots(&dialog.controls[c]);
    let (fit, injured): (Vec<_>, Vec<_>) =
        club.players.iter().zip(&club.status).partition(|(_, s)| s.injured == 0);
    let (mut rows, mut icons, mut keys) = (Vec::new(), Vec::new(), Vec::new());
    let mut extra_sum = 0i64;
    for (player, status) in fit.into_iter().chain(injured) {
        let Some(r) = records.player(status.id) else { continue };
        let extra = i64::from(r.get::<i16>(EXTRA_INSURANCE)) * 1000;
        extra_sum += extra;
        let strength = if matches!(r.picks() >> 6, 1 | 3) {
            "?".to_owned()
        } else {
            player.strength.to_string()
        };
        let compulsory = i64::from(r.salary()) * INSURANCE_RATE / 100 * 100;
        rows.push(
            [
                super::duties::short_name(cx, player, status.id),
                super::hub::abbreviation(game, player.positions[0]),
                strength,
                player.age(season.date()).to_string(),
                money(game, compulsory),
                if extra != 0 { money(game, extra) } else { " ".to_owned() },
            ]
            .join("\t"),
        );
        let risky = r.knocks() & RISKY != 0;
        icons.push(if risky {
            slots.first().map(|&x| vec![(x, RISK_ICON)]).unwrap_or_default()
        } else {
            Vec::new()
        });
        keys.push(usize::from(status.id));
    }
    if let Some(list) = view.list_mut(c) {
        list.inks = vec![Some([0, 0, 0]); rows.len()];
        list.rows = rows;
        list.row_icons = icons;
        list.keys = keys;
        list.selected = None;
    }
    set(view, dialog, EXTRA_SUM, money(game, extra_sum));
}

/// Other costs (dialog 122, §9): lawyers, the statistics and computer
/// departments, administration and special expenses; from the finances a
/// better lawyer or a bigger department can be taken, its yearly price
/// booked at once.
#[derive(Default)]
pub struct OtherCosts {
    lawyer: u8,
    statistics: u8,
    computers: u8,
    hearings: (u16, u16),
}

pub(crate) const LAWYERS: [u16; 5] = [2314, 2315, 2316, 2317, 2318];
const STATISTICS: [u16; 4] = [2319, 2320, 2321, 6217];
const COMPUTER_OPTIONS: [u16; 2] = [2322, 2323];
const HEARINGS_FIGURES: (u16, u16) = (2297, 2299);
const LAWYER_COSTS: u16 = 2301;
const STATISTICS_COSTS: u16 = 2303;
const COMPUTER_COSTS: u16 = 2305;
const ADMINISTRATION: u16 = 2307;
const SPECIAL: u16 = 2309;
const INTEREST: u16 = 6191;
const TAXES: u16 = 6192;
const OTHER_SPINS: [u16; 4] = [2310, 2311, 2312, 2313];
/// The lawyers' tariffs a hearing, in thousands: for a red card, for
/// anything else (0x89e45c); the departments' yearly prices (0x89e450,
/// 0x89e458).
pub(crate) const TARIFFS: [(i32, i32); 5] = [(10, 30), (15, 50), (20, 80), (30, 120), (50, 200)];
const STATISTICS_PRICES: [i32; 4] = [0, 20_000, 200_000, 1_000_000];
const COMPUTER_PRICES: [i32; 2] = [0, 200_000];
/// The levels (club +0x5e0…), the plan's departments (P+0x94), its
/// administration and special expenses, the booked departments
/// (B+0x94), interest and taxes.
pub(crate) const LAWYER_AT: usize = 0x5e0;
const STATISTICS_AT: usize = 0x5e1;
const COMPUTERS_AT: usize = 0x5e2;
const PLAN_DEPARTMENTS: usize = 0x94;
const PLAN_ADMINISTRATION: usize = 0x90;
const PLAN_SPECIAL: usize = 0x98;
const SPECIAL_UNIT: i32 = 100_000;
const BOOKED_DEPARTMENTS: usize = 0x94;
const BOOKED_INTEREST: usize = 0xa0;
const BOOKED_TAXES: usize = 0xe8;
/// The manager record's estimated hearings: red cards, others.
pub(crate) const HEARINGS_AT: (usize, usize) = (0x1f68, 0x1f6a);
const LOWER: &str = "---";

impl Screen for OtherCosts {
    fn ids(&self) -> &[u16] {
        &[OTHER_COSTS]
    }

    /// `FUN_004ebab8` in view mode.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        open_calculation(cx, view, &OTHER_SPINS);
        // This one's caption has the date, as the finances'.
        if let Some(season) = cx.state.season.as_ref() {
            set(
                view,
                dialog,
                CAPTION_ID,
                super::hub::manager_title_of(game, season, &dialog.title),
            );
        }
        let Some(team) = own_team(cx) else { return };
        let level = |c: usize, most: usize| {
            (team.get::<i8>(c + CLUB_TO_TEAM).max(0) as usize).min(most) as u8
        };
        self.lawyer = level(LAWYER_AT, LAWYERS.len() - 1);
        self.statistics = level(STATISTICS_AT, STATISTICS.len() - 1);
        self.computers = level(COMPUTERS_AT, COMPUTER_OPTIONS.len() - 1);
        self.hearings = own_manager(cx)
            .map_or((0, 0), |m| (m.get::<u16>(HEARINGS_AT.0), m.get::<u16>(HEARINGS_AT.1)));
        let (plan, booked) = (Accounts::plan(team), Accounts::booked(team));
        let m = |v: i32| money(game, v.into());
        set(view, dialog, ADMINISTRATION, m(plan.i32_(PLAN_ADMINISTRATION)));
        set(view, dialog, SPECIAL, m(plan.i32_(PLAN_SPECIAL) / SPECIAL_UNIT * SPECIAL_UNIT));
        set(view, dialog, INTEREST, m(booked.i32_(BOOKED_INTEREST)));
        set(view, dialog, TAXES, m(booked.i32_(BOOKED_TAXES)));
        set(view, dialog, HEARINGS_FIGURES.0, self.hearings.0.to_string());
        set(view, dialog, HEARINGS_FIGURES.1, self.hearings.1.to_string());
        // The choices below the club's own are out of reach.
        for (ids, level) in [
            (&LAWYERS[..], self.lawyer),
            (&STATISTICS[..], self.statistics),
            (&COMPUTER_OPTIONS[..], self.computers),
        ] {
            for &id in &ids[..usize::from(level)] {
                if let Some(options) = control(dialog, id).and_then(|c| view.options_mut(c)) {
                    options.off = vec![true];
                    if let Some(label) = options.labels.first_mut() {
                        *label = LOWER.to_owned();
                    }
                }
            }
        }
        self.show(cx, view);
        self.departments(cx);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some((outcome, write)) = closing(cx, view, target) {
            if write {
                self.write(cx);
            }
            return outcome;
        }
        let Target::Option { control: c, .. } = target else { return Outcome::Stay };
        let Some(id) = dialog.controls[c].id else { return Outcome::Stay };
        if let Some(k) = LAWYERS.iter().position(|&i| i == id) {
            self.lawyer = k as u8;
        } else if let Some(k) = STATISTICS.iter().position(|&i| i == id) {
            self.statistics = k as u8;
            self.departments(cx);
        } else if let Some(k) = COMPUTER_OPTIONS.iter().position(|&i| i == id) {
            self.computers = k as u8;
            self.departments(cx);
        }
        self.show(cx, view);
        Outcome::Stay
    }
}

impl OtherCosts {
    fn statistics_price(&self) -> i32 {
        STATISTICS_PRICES[usize::from(self.statistics)]
    }

    fn computers_price(&self) -> i32 {
        COMPUTER_PRICES[usize::from(self.computers)]
    }

    /// Each group's choice lit, the costs (`FUN_004ec755`, `…9cb`, `…8d2`).
    fn show(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        for (ids, level) in [
            (&LAWYERS[..], self.lawyer),
            (&STATISTICS[..], self.statistics),
            (&COMPUTER_OPTIONS[..], self.computers),
        ] {
            for (k, &id) in ids.iter().enumerate() {
                if let Some(c) = control(dialog, id) {
                    let chosen =
                        if k == usize::from(level) { 0 } else { crate::controls::options::NONE };
                    view.set_chosen(c, chosen);
                }
            }
        }
        let (red, other) = TARIFFS[usize::from(self.lawyer)];
        let lawyers =
            (i32::from(self.hearings.0) * red + i32::from(self.hearings.1) * other) * 1000;
        set(view, dialog, LAWYER_COSTS, money(game, lawyers.into()));
        set(view, dialog, STATISTICS_COSTS, money(game, self.statistics_price().into()));
        set(view, dialog, COMPUTER_COSTS, money(game, self.computers_price().into()));
    }

    /// The plan's departments, written at once: at the opening and at
    /// every department's choice.
    fn departments(&self, cx: &mut Cx) {
        let sum = self.statistics_price() + self.computers_price();
        if let Some(team) = own_team_mut(cx) {
            team.put(0x120 + PLAN_DEPARTMENTS + CLUB_TO_TEAM, sum);
        }
    }

    /// OK and the arrow bar in view mode (`FUN_004ec312`): the lawyer, and
    /// a bigger department booked at its yearly price.
    fn write(&self, cx: &mut Cx) {
        let (lawyer, statistics, computers) = (self.lawyer, self.statistics, self.computers);
        let (s, c) = (self.statistics_price(), self.computers_price());
        let Some(team) = own_team_mut(cx) else { return };
        team.put(LAWYER_AT + CLUB_TO_TEAM, lawyer);
        let mut booked = team.get::<i32>(booked_at(BOOKED_DEPARTMENTS));
        if statistics > team.get::<u8>(STATISTICS_AT + CLUB_TO_TEAM) {
            team.put(STATISTICS_AT + CLUB_TO_TEAM, statistics);
            booked = booked.wrapping_add(s);
        }
        if computers > team.get::<u8>(COMPUTERS_AT + CLUB_TO_TEAM) {
            team.put(COMPUTERS_AT + CLUB_TO_TEAM, computers);
            booked = booked.wrapping_add(c);
        }
        team.put(booked_at(BOOKED_DEPARTMENTS), booked);
    }
}

/// The manager's record.
pub(crate) fn own_manager<'a>(cx: &'a Cx) -> Option<&'a a3_core::records::Manager> {
    let current = cx.state.season.as_ref()?.current;
    let records = cx.state.records.as_ref()?;
    records.managers.get(records.human_slot(current)?)
}

/// A club field past the team record's end, in the club's extra data.
const CLUB_TO_EXTRA: usize = 0x148;

/// Whether a target is the button with `id`.
fn is_button(dialog: &Dialog, target: Target, id: u16) -> bool {
    matches!(target, Target::Button(c) if dialog.controls[c].id == Some(id))
}

/// A push button that cannot be pressed: "---", disabled.
fn lower_button(view: &mut View, dialog: &Dialog, id: u16) {
    if let Some(b) = control(dialog, id).and_then(|c| view.controls[c].state.as_button_mut()) {
        b.disabled = true;
        b.text = Some(LOWER.to_owned());
    }
}

/// A term in years: "ein Jahr" … "zehn Jahre", else "%d Jahre"
/// (`FUN_00730bdc`).
fn years_text(cx: &Cx, years: u8) -> String {
    match years {
        1..=10 => cx.game.text(YEARS + u16::from(years) - 1).to_owned(),
        _ => super::finances::printf(cx.game.text(MANY_YEARS), &[&years]),
    }
}

const YEARS: u16 = 1081;
const MANY_YEARS: u16 = 1587;

/// The ticket office (dialog 137, §3.1): this season's gates by
/// competition, the season tickets, the league's prices, which the
/// spinners change at once and for good, and the advance sales of the home
/// matches ahead.
#[derive(Default)]
pub struct Tickets;

const TICKETS: u16 = 137;
const GATES_HEADING: u16 = 2439;
const AVERAGE_GATE: u16 = 3668;
const PRICE_FIGURES: [u16; 4] = [2402, 2403, 5790, 5793];
const PRICE_SPINS: [u16; 4] = [2423, 2424, 5791, 5794];
/// The league's prices of standing, the seats behind the goal and along
/// the side, the boxes: the first of five each; the cups' side and
/// standing prices the second.
const LEAGUE_PRICES: [usize; 4] = [0x49c, 0x4a6, 0x4b0, 0x4ba];
const CUP_PRICES: (usize, usize) = (0x4b2, 0x49e);
/// The gates booked by competition: league, Europe, cup, friendlies.
const GATE_FIGURES: [(u16, usize); 4] = [(2440, 0x8), (2441, 0x10), (2446, 0xc), (2442, 0x14)];
const SEASON_TICKETS_FIGURE: u16 = 2444;
const SEASON_TICKETS: usize = 0x498;
const SALES_HEADING: u16 = 2445;
const SALES_AVERAGE: u16 = 3312;
const SALES_LIST: u16 = 2437;
const MATCH_DATE: u16 = 1777;
const LEAGUE_MATCH: u16 = 1692;
const CUP_MATCH: u16 = 1694;
/// The marks' texts: normal, +50%, +25%, halved (0x883f2c).
const MARK_TEXTS: [u16; 4] = [1005, 1003, 1002, 1004];

/// A row of the advance sales: the day, the kind, the opponent, the
/// tickets sold, the league's mark or the cup's prices.
struct SaleRow {
    day: i16,
    kind: u16,
    opponent: (u8, i16),
    sold: i32,
    mark: Option<usize>,
    prices: (i16, i16),
}

impl Screen for Tickets {
    fn ids(&self) -> &[u16] {
        &[TICKETS]
    }

    /// `0x4f7432`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        carousel(view, dialog);
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let booked = Accounts::booked(team);
        for (id, o) in GATE_FIGURES {
            set(view, dialog, id, money(game, booked.i32_(o).into()));
        }
        let season_tickets = team.get::<i32>(SEASON_TICKETS + CLUB_TO_TEAM);
        set(view, dialog, SEASON_TICKETS_FIGURE, super::finances::count(season_tickets));
        let games = team.home_games();
        if games > 0
            && let Some(c) = control(dialog, GATES_HEADING)
        {
            let sum =
                team.gates().iter().take(usize::from(games)).fold(0i32, |s, &g| s.wrapping_add(g));
            let text = super::finances::printf(
                game.text(AVERAGE_GATE),
                &[&dialog.controls[c].text, &(sum / i32::from(games))],
            );
            view.set_text(c, text);
        }
        for (k, (&figure, &spin)) in PRICE_FIGURES.iter().zip(&PRICE_SPINS).enumerate() {
            let price = team.get::<i16>(LEAGUE_PRICES[k] + CLUB_TO_TEAM);
            let table = a3_core::tickets::price_table(k);
            if let Some(value) = control(dialog, spin).and_then(|c| view.value_mut(c)) {
                value.range = (0, table.len() as i32 - 1);
                value.set(a3_core::tickets::price_step(&table, price) as i32);
            }
            set(view, dialog, figure, money(game, price.into()));
        }
        super::seasonal_buttons(cx, view);
        let (rows, average) = sales(cx, records, team);
        if let Some(average) = average {
            set(
                view,
                dialog,
                SALES_HEADING,
                super::finances::printf(game.text(SALES_AVERAGE), &[&average.to_string()]),
            );
        }
        let texts = rows
            .iter()
            .map(|r| {
                let date =
                    records.globals.date_of(r.day).map_or_else(String::new, |d| match_date(cx, d));
                let price = match r.mark {
                    Some(m) => game.text(MARK_TEXTS[m]).to_owned(),
                    None => format!("{}/{}", r.prices.0, money(game, r.prices.1.into())),
                };
                [
                    date,
                    game.text(r.kind).to_owned(),
                    game.club_name(Some(records), r.opponent.0, r.opponent.1),
                    super::finances::count(r.sold),
                    price,
                ]
                .join("\t")
            })
            .collect::<Vec<_>>();
        if let Some(list) = control(dialog, SALES_LIST).and_then(|c| view.list_mut(c)) {
            list.inks = vec![None; texts.len()];
            list.rows = texts;
            list.selected = None;
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some(outcome) = carousel_input(cx, view, target) {
            return outcome;
        }
        // A price written at once into the club (`0x4f8798`).
        let Target::Spin { control: c, .. } = target else { return Outcome::Stay };
        let Some(k) = PRICE_SPINS.iter().position(|&id| dialog.controls[c].id == Some(id)) else {
            return Outcome::Stay;
        };
        let table = a3_core::tickets::price_table(k);
        let at = view.value(c).map_or(0, |v| v.value) as usize;
        let price = table[at.min(table.len() - 1)];
        if let Some(team) = own_team_mut(cx) {
            team.put(LEAGUE_PRICES[k] + CLUB_TO_TEAM, price);
        }
        set(view, dialog, PRICE_FIGURES[k], money(cx.game, price.into()));
        Outcome::Stay
    }
}

/// The date of a match: "14.9.2025" (text 1777).
fn match_date(cx: &Cx, date: chrono::NaiveDate) -> String {
    cx.game
        .text(MATCH_DATE)
        .replace("%1", &date.format("%-d").to_string())
        .replace("%2", &date.format("%-m").to_string())
        .replace("%3", &date.format("%Y").to_string())
}

/// The advance sales (`FUN_004f794f`): the home league matches from today
/// on, then the home cup ties, sorted by day; and the mean sold of the
/// league matches with a record. The European ties' lookup
/// (`FUN_0062bd73`) is not traced: none are listed.
fn sales(
    cx: &Cx,
    records: &a3_core::records::Records,
    team: &a3_core::records::Team,
) -> (Vec<SaleRow>, Option<i32>) {
    let game = cx.game;
    let today = records.globals.day();
    let own = (team.country(), i16::from(team.number()));
    let mut rows = Vec::new();
    let (mut sold, mut counted) = (0i32, 0i32);
    let fixtures = a3_core::opponent::fixture_days(records, &game.calendar, team);
    for (i, f) in fixtures.iter().enumerate() {
        let day = f.day as i16;
        if !f.home || f.day < usize::from(today) {
            continue;
        }
        let opponent = (own.0, f.opponent);
        let sale = a3_core::tickets::sale(records, own, opponent, day);
        if let Some(s) = sale {
            sold = sold.wrapping_add(s.sold);
            counted += 1;
        }
        rows.push(SaleRow {
            day,
            kind: LEAGUE_MATCH,
            opponent,
            sold: sale.map_or(0, |s| s.sold),
            mark: Some(a3_core::tickets::matchday_mark(team, i as i32 + 1)),
            prices: (0, 0),
        });
    }
    let ties = a3_core::tickets::cup_ties(
        records,
        team,
        &game.rules.forecast.tickets,
        &game.calendar,
        usize::from(today),
    );
    for t in ties.iter().filter(|t| t.home && !t.final_tie) {
        let (day, opponent) = (t.day as i16, (own.0, t.number as i16));
        let sale = a3_core::tickets::sale(records, own, opponent, day);
        let price = |o: usize| team.get::<i16>(o + CLUB_TO_TEAM);
        rows.push(SaleRow {
            day,
            kind: CUP_MATCH,
            opponent: (t.country, t.number as i16),
            sold: sale.map_or(0, |s| s.sold),
            mark: None,
            prices: sale
                .map_or((price(CUP_PRICES.0), price(CUP_PRICES.1)), |s| (s.prices[2], s.prices[0])),
        });
    }
    // The game's exchange sort.
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            if rows[i].day > rows[j].day {
                rows.swap(i, j);
            }
        }
    }
    (rows, (counted > 0).then(|| sold / counted))
}

/// The arrow bar in its middle.
fn carousel(view: &mut View, dialog: &Dialog) {
    if let Some(bar) = control(dialog, CAROUSEL).and_then(|c| view.value_mut(c)) {
        bar.range = CAROUSEL_RANGE;
        bar.set(1);
    }
}

/// The sponsors (dialog 125, §4.1): the main sponsor's contract, the
/// sponsor pool, the boards and the outfitter. From the finances every
/// input is disabled; only the contract's details (not built), the arrow
/// bar, OK and cancel answer.
#[derive(Default)]
pub struct Sponsors;

const SPONSORS: u16 = 125;
const SPONSOR_NAME: u16 = 2349;
const SPONSOR_LOGO: u16 = 2348;
const SPONSOR_TERM: u16 = 2350;
const CONTRACT_DETAILS: u16 = 2351;
const LAST_FEE_FIGURE: u16 = 2361;
const FEE_FIGURE: u16 = 2360;
const LAST_MEMBERS_FIGURE: u16 = 2359;
const MEMBERS_FIGURE: u16 = 2358;
const POOL_FIGURE: u16 = 2357;
const BOARDS_FIGURE: u16 = 5784;
const BOARDS_DETAILS: u16 = 5732;
const OUTFITTER_FIGURE: u16 = 6193;
const OUTFITTER_DETAILS: u16 = 6194;
/// The pool's fee and its planned members, the members booked; last
/// season's fee (in the extra data).
const FEE: usize = 0x504;
const PLANNED_MEMBERS: usize = 0x1fc;
const BOOKED_MEMBERS: usize = 0x304;
const LAST_FEE: usize = 0x8ec;
/// The main sponsor: the years its contract runs, its number.
const CONTRACT_YEARS: usize = 0x509;
const SPONSOR: usize = 0x4ea;
/// A logo for each sponsor; numbers past the last have the blank one.
const LOGO: &str = "Banden/Kom";
const LAST_LOGO: i16 = 480;
const NO_LOGO: i16 = 999;
/// The boards: let to an agency for a sum, or one by one (fourteen of
/// eight bytes: let, how many, the price); the outfitter and its sum.
const BOARDS_AGENCY: usize = 0x5a1;
const BOARDS_SUM: usize = 0x5a4;
const BOARDS: usize = 0x530;
const BOARD_COUNT: usize = 14;
const OUTFITTER: usize = 0x5b1;
const OUTFITTER_SUM: usize = 0x5b4;

impl Screen for Sponsors {
    fn ids(&self) -> &[u16] {
        &[SPONSORS]
    }

    /// `0x5728e4` in view mode.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        carousel(view, dialog);
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let club = |o: usize| o + CLUB_TO_TEAM;
        let last_fee = super::finances::own_extra(game, cx)
            .map_or(0, |(_, e)| e.get::<i32>(LAST_FEE - CLUB_TO_EXTRA));
        set(view, dialog, LAST_FEE_FIGURE, money(game, last_fee.into()));
        let booked = team.get::<i16>(club(BOOKED_MEMBERS));
        set(view, dialog, LAST_MEMBERS_FIGURE, booked.to_string());
        let (members, fee) = (team.get::<u16>(club(PLANNED_MEMBERS)), team.get::<i32>(club(FEE)));
        set(view, dialog, MEMBERS_FIGURE, members.to_string());
        set(view, dialog, FEE_FIGURE, money(game, fee.into()));
        let pool = i32::from(members).wrapping_mul(fee);
        set(view, dialog, POOL_FIGURE, money(game, pool.into()));
        let years = team.get::<u8>(club(CONTRACT_YEARS));
        if years != 0 {
            let number = team.get::<i16>(club(SPONSOR));
            sponsor_logo(view, dialog, SPONSOR_LOGO, number);
            set(view, dialog, SPONSOR_NAME, records.sponsor_name(number).unwrap_or_default());
            if let Some(c) = control(dialog, SPONSOR_TERM) {
                let text =
                    super::finances::printf(&dialog.controls[c].text, &[&years_text(cx, years)]);
                view.set_text(c, text);
            }
        } else {
            set(view, dialog, SPONSOR_NAME, LOWER.to_owned());
            set(view, dialog, SPONSOR_TERM, LOWER.to_owned());
            lower_button(view, dialog, CONTRACT_DETAILS);
        }
        // `FUN_005730e5`: what the boards and the outfitter bring.
        let boards = if team.get::<u8>(club(BOARDS_AGENCY)) != 0 {
            team.get::<i32>(club(BOARDS_SUM))
        } else {
            (0..BOARD_COUNT).fold(0i32, |sum, i| {
                let at = club(BOARDS + 8 * i);
                if team.get::<u8>(at + 1) == 0 {
                    return sum;
                }
                let count = i32::from(team.get::<u8>(at + 2));
                sum.wrapping_add(count.wrapping_mul(team.get::<i32>(at + 4)))
            })
        };
        set(view, dialog, BOARDS_FIGURE, money(game, boards.into()));
        let outfitter = if team.get::<u8>(club(OUTFITTER)) != 0 {
            team.get::<i32>(club(OUTFITTER_SUM))
        } else {
            0
        };
        set(view, dialog, OUTFITTER_FIGURE, money(game, outfitter.into()));
        lower_button(view, dialog, BOARDS_DETAILS);
        lower_button(view, dialog, OUTFITTER_DETAILS);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if is_button(cx.dialog, target, CONTRACT_DETAILS) {
            return Outcome::Popup(a3_core::cycle::Request::new(SPONSOR_CONTRACT));
        }
        carousel_input(cx, view, target).unwrap_or(Outcome::Stay)
    }
}

/// A sponsor's logo (`FUN_006960ed`), drawn as it is.
fn sponsor_logo(view: &mut View, dialog: &Dialog, id: u16, number: i16) {
    let logo = if number > LAST_LOGO { NO_LOGO } else { number };
    if let Some(bitmap) = control(dialog, id).and_then(|c| view.bitmap_mut(c)) {
        bitmap.picture =
            Some(Picture { black_clear: false, ..Picture::file(format!("{LOGO}{logo}.bmp")) });
    }
}

/// The main sponsor's contract (dialog 246, `CSponsorAktuellerVertrag`;
/// KALKULATION.md §4.1): its amounts, the relegation cut, the term, the
/// most it can pay and the sponsor pool; it writes nothing.
#[derive(Default)]
pub struct SponsorContract;

const SPONSOR_CONTRACT: u16 = 246;
/// The contract's amounts (club +0x510 …) and their figures (0x889778):
/// the base, title or promotion, international, the cup won, each cup
/// round, the European cup won, the attendance bonus.
const CONTRACT_FIGURES: [(usize, u16); 7] = [
    (0x510, 3412),
    (0x514, 3414),
    (0x518, 3415),
    (0x51c, 3418),
    (0x520, 3417),
    (0x524, 3419),
    (0x528, 3413),
];
const TITLE: usize = 0x514;
const INTERNATIONAL: usize = 0x518;
const CUP_ROUND: usize = 0x520;
/// The most counts five cup rounds.
const CUP_ROUNDS: i32 = 5;
/// The relegation cut in percent, the term, the most it pays.
const RELEGATION_CUT: (usize, u16) = (0x50a, 3416);
const TERM_FIGURE: u16 = 3420;
const MOST_FIGURE: u16 = 3421;
/// The attendance the bonus asks for, in its label.
const ATTENDANCE_LABEL: u16 = 3403;
const ATTENDANCE: usize = 0x52c;
const POOL_LABEL: u16 = 3424;
const CONTRACT_POOL_FIGURE: u16 = 3425;
const CONTRACT_LOGO: u16 = 3423;
/// The title row, hidden for a club with a professional club above it.
const TITLE_ROW: [u16; 2] = [3404, 3414];

impl Screen for SponsorContract {
    fn ids(&self) -> &[u16] {
        &[SPONSOR_CONTRACT]
    }

    /// `0x571911`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let club = |o: usize| team.get::<i32>(o + CLUB_TO_TEAM);
        let mut most = 0i32;
        for (o, id) in CONTRACT_FIGURES {
            set(view, dialog, id, money(game, club(o).into()));
            let counted = match o {
                TITLE if club(INTERNATIONAL) >= club(TITLE) => 0,
                INTERNATIONAL if club(TITLE) > club(INTERNATIONAL) => 0,
                CUP_ROUND => club(o).wrapping_mul(CUP_ROUNDS),
                _ => club(o),
            };
            most = most.wrapping_add(counted);
        }
        let cut = team.get::<u8>(RELEGATION_CUT.0 + CLUB_TO_TEAM);
        set(view, dialog, RELEGATION_CUT.1, format!("{cut} %"));
        set(
            view,
            dialog,
            TERM_FIGURE,
            years_text(cx, team.get::<u8>(CONTRACT_YEARS + CLUB_TO_TEAM)),
        );
        set(view, dialog, MOST_FIGURE, money(game, most.into()));
        let printf = super::finances::printf;
        for (id, arg) in [
            (ATTENDANCE_LABEL, club(ATTENDANCE)),
            (POOL_LABEL, i32::from(team.get::<i16>(PLANNED_MEMBERS + CLUB_TO_TEAM))),
        ] {
            if let Some(c) = control(dialog, id) {
                let own = &dialog.controls[c].text;
                let text =
                    if id == POOL_LABEL { format!("{own} ({arg}):") } else { printf(own, &[&arg]) };
                view.set_text(c, text);
            }
        }
        let members = i32::from(team.get::<i16>(PLANNED_MEMBERS + CLUB_TO_TEAM));
        set(
            view,
            dialog,
            CONTRACT_POOL_FIGURE,
            money(game, members.wrapping_mul(club(FEE)).into()),
        );
        sponsor_logo(view, dialog, CONTRACT_LOGO, team.get::<i16>(SPONSOR + CLUB_TO_TEAM));
        // `FUN_0063e7cf`: a club of a country played, below a professional
        // one (team +0x873).
        if records.globals.played(i16::from(team.country())) && team.linked()[1] >= 0 {
            for id in TITLE_ROW {
                if let Some(c) = control(dialog, id) {
                    view.controls[c].hidden = true;
                }
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

/// The merchandise (dialog 115, §5.1): the planned and the actual income,
/// the agencies' offers (none from the finances) and the range of one's
/// own (not built).
#[derive(Default)]
pub struct Merchandise;

const MERCHANDISE: u16 = 115;
const AGENCY_FIGURES: [u16; 4] = [2205, 2207, 2206, 2208];
const AGENCY_DETAILS: [u16; 2] = [6020, 6021];
const PLANNED_FIGURE: u16 = 2216;
const ACTUAL_FIGURE: u16 = 6019;
const MERCHANDISE_SLIDER: u16 = 2203;
const RANGE_DETAILS: u16 = 2204;
const MERCHANDISE_OK: u16 = 2214;
/// The merchandise in the plan and as booked; an agency's: which one
/// (from 1), its years, its sum.
const MERCHANDISE_INCOME: usize = 0xe4;
const AGENCY: usize = 0x5a8;
const AGENCY_YEARS: usize = 0x5a9;
const AGENCY_SUM: usize = 0x5ac;

impl Screen for Merchandise {
    fn ids(&self) -> &[u16] {
        &[MERCHANDISE]
    }

    /// `0x4e6d3c` in view mode, no agency engaged.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        open_calculation(cx, view, &[MERCHANDISE_SLIDER]);
        let Some(team) = own_team(cx) else { return };
        let (plan, booked) = (Accounts::plan(team), Accounts::booked(team));
        set(view, dialog, PLANNED_FIGURE, money(game, plan.i32_(MERCHANDISE_INCOME).into()));
        for id in AGENCY_FIGURES {
            set(view, dialog, id, LOWER.to_owned());
        }
        for id in AGENCY_DETAILS {
            lower_button(view, dialog, id);
        }
        let engaged = team.get::<u8>(AGENCY_YEARS + CLUB_TO_TEAM) != 0;
        let actual = if engaged {
            team.get::<i32>(AGENCY_SUM + CLUB_TO_TEAM)
        } else {
            booked.i32_(MERCHANDISE_INCOME)
        };
        set(view, dialog, ACTUAL_FIGURE, money(game, actual.into()));
        if engaged {
            lower_button(view, dialog, RANGE_DETAILS);
        }
        // The own range (56) is not built.
        mark_inert(view, dialog, |t| !is_button(dialog, t, RANGE_DETAILS));
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Target::Button(c) = target
            && cx.dialog.controls[c].id == Some(MERCHANDISE_OK)
        {
            return Outcome::Close(Reply::Ok);
        }
        carousel_input(cx, view, target).unwrap_or(Outcome::Stay)
    }
}

/// An agency has taken the merchandise on (dialog 130, §5.2): which, and
/// for how long — the sum the game passes along has no place in the text.
#[derive(Default)]
pub struct AgencyEngaged;

pub(crate) const AGENCY_ENGAGED: u16 = 130;
const ENGAGED_AGENCY: u16 = 2384;
const ENGAGED_TERM: u16 = 2385;
const AGENCY_NAMES: u16 = 1627;

impl Screen for AgencyEngaged {
    fn ids(&self) -> &[u16] {
        &[AGENCY_ENGAGED]
    }

    /// `0x4e2384`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        let Some(team) = own_team(cx) else { return };
        let agency = team.get::<u8>(AGENCY + CLUB_TO_TEAM);
        let years = team.get::<u8>(AGENCY_YEARS + CLUB_TO_TEAM);
        if let Some(k) = agency.checked_sub(1) {
            set(view, dialog, ENGAGED_AGENCY, cx.game.text(AGENCY_NAMES + u16::from(k)).to_owned());
        }
        if let Some(c) = control(dialog, ENGAGED_TERM) {
            let text = super::finances::printf(&dialog.controls[c].text, &[&years_text(cx, years)]);
            view.set_text(c, text);
        }
        super::popups::ball(view, cx);
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

/// Whether an agency has the merchandise: its row then shows 130 when
/// opened and is stepped past when paged to.
fn agency_engaged(cx: &Cx) -> bool {
    own_team(cx).is_some_and(|t| t.get::<i8>(AGENCY_YEARS + CLUB_TO_TEAM) > 0)
}
