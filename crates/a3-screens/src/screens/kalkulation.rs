//! The calculations behind the finances' rows (KALKULATION.md in the RE
//! repo): a dialog for each row, paged through in the rows' order by an
//! arrow bar at the bottom of every one; opened from the finances, they
//! show the season's plan beside what is booked, their plan sliders
//! hidden.

use std::collections::HashMap;

use a3_core::finances::Accounts;
use a3_files::rch::Dialog;

use super::playerinfo::money;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
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
/// The dialogs built so far, which the carousel stops at.
pub(crate) const BUILT: [u16; 6] = FIGURE_DIALOGS;
/// The arrow bar: back gives 7, on gives 6 (`FUN_004a43cb`).
const CAROUSEL: u16 = 5671;
pub(crate) const NEXT: usize = 6;
pub(crate) const PREVIOUS: usize = 7;
const CAROUSEL_RANGE: (i32, i32) = (0, 2);
const CANCEL: u16 = 2;
/// Club offsets c from 0x120 to 0x5f7 lie at c + 0x278 in the team record.
const CLUB_TO_TEAM: usize = 0x278;

/// Their captions.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in FIGURE_DIALOGS {
        if let Some(dialog) = dialogs.get_mut(&id) {
            dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
        }
    }
}

/// The next row's dialog that is built, on or back from a row, if any.
pub(crate) fn step(row: usize, choice: usize) -> Option<usize> {
    let rows = ROW_DIALOGS.len();
    (1..=rows)
        .map(|k| if choice == NEXT { (row + k) % rows } else { (row + rows - k) % rows })
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
    if let Some(bar) = control(dialog, CAROUSEL).and_then(|c| view.value_mut(c)) {
        bar.range = CAROUSEL_RANGE;
        bar.set(1);
    }
    for &id in sliders {
        if let Some(c) = control(dialog, id) {
            view.controls[c].hidden = true;
        }
    }
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
