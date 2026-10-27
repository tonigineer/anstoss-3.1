//! The game's screens, a module per area of the game. The game cycle asks
//! for a screen (see [`a3_core::cycle`]); the screen fills its dialog's view
//! as it opens and answers what the player does there, may ask for a popup
//! over itself, and closes with a reply — to the cycle, or a popup's to the
//! screen below it. The [`crate::host`] keeps them open.

pub mod bank;
pub mod board;
pub mod calendar;
pub mod comparison;
pub mod contacts;
pub mod discipline;
pub mod duties;
pub mod finances;
pub mod formations;
pub mod grounds;
pub mod honours;
pub mod hub;
pub mod jobs;
pub mod kalkulation;
pub mod market;
pub mod medical;
pub mod office;
pub mod opponent;
pub mod planner;
pub mod playerinfo;
pub mod popups;
pub mod search;
pub mod special;
pub mod squad;
pub mod stadium;
pub mod start;
pub mod tactics;
pub mod talk;
pub mod training;
pub mod year;
pub mod youth;

use std::collections::HashMap;

pub use a3_core::cycle::{Args, Reply, Request};
use a3_core::state::GameState;
use a3_files::rch::{
    Align, CAPTION_FONT, Control, Dialog, HELP_FLAG, Kind, PLAIN_FONT, PRINTER_FLAG,
};
use bevy_math::{Rect, Vec2};

use crate::controls::button::{is_cancel, is_ok};
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

/// A screen of the game: one or more dialogs and what they do.
pub trait Screen: Send + Sync {
    /// The dialogs this screen runs, by id in the dialog file.
    fn ids(&self) -> &[u16];

    /// Fills the view as the screen opens, from the state and what it was
    /// asked to show.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args);

    /// Asked for again while it is open: a screen that shows the new request
    /// in the view it has says so; the others open anew.
    fn again(&mut self, _cx: &mut Cx, _view: &mut View, _args: Args) -> bool {
        false
    }

    /// A click or key on one of its targets, once the controls have taken
    /// it in; `double` for the second click of a double click.
    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, double: bool) -> Outcome;

    /// The reply of a popup it asked for.
    fn answer(&mut self, _cx: &mut Cx, _view: &mut View, _reply: Reply) -> Outcome {
        Outcome::Stay
    }

    /// Time passing, for screens that move on by themselves.
    fn tick(&mut self, _cx: &mut Cx, _view: &mut View, _seconds: f32) -> Outcome {
        Outcome::Stay
    }
}

/// What a screen works with: the installation, its own dialog and the state
/// of the game.
pub struct Cx<'a> {
    pub game: &'a Game,
    pub dialog: &'a Dialog,
    pub state: &'a mut GameState,
}

/// What follows an input or a tick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Stay,
    /// Opens a popup over this screen; its reply comes back to
    /// [`Screen::answer`].
    Popup(Request),
    /// Closes this screen with a reply: a popup's to the screen below it,
    /// any other's to the game cycle.
    Close(Reply),
}

/// Every screen, once.
pub fn registry() -> Vec<Box<dyn Screen>> {
    vec![
        Box::new(start::StartMenu::default()),
        Box::new(start::LoadGame),
        Box::new(start::HighScores),
        Box::new(start::setup::Countries),
        Box::new(start::setup::Players),
        Box::new(start::setup::Names),
        Box::new(start::setup::Club),
        Box::new(start::setup::NationalTeam),
        Box::new(start::setup::Abilities),
        Box::new(start::setup::Birthday),
        Box::new(start::setup::Settings),
        Box::new(start::network::Name),
        Box::new(start::network::Details),
        Box::new(start::network::Games),
        Box::new(jobs::Offers::default()),
        Box::new(jobs::Talks::default()),
        Box::new(jobs::Failed),
        Box::new(jobs::Signed),
        Box::new(popups::Question),
        Box::new(popups::Notice),
        Box::new(calendar::Wait::default()),
        Box::new(calendar::Week::default()),
        Box::new(hub::MenuBar::default()),
        Box::new(formations::Formations::default()),
        Box::new(special::Special),
        Box::new(duties::Takers::default()),
        Box::new(medical::Medical::default()),
        Box::new(medical::Visit),
        Box::new(opponent::Opponent),
        Box::new(playerinfo::PlayerInfo::default()),
        Box::new(office::Office),
        Box::new(discipline::Bonus::default()),
        Box::new(discipline::Fine::default()),
        Box::new(discipline::FineReply),
        Box::new(discipline::Change),
        Box::new(planner::Planner::default()),
        Box::new(planner::WeekPlanner::default()),
        Box::new(year::YearCalendar::default()),
        Box::new(finances::Balance::default()),
        Box::new(finances::Finances::default()),
        Box::new(bank::Investments::default()),
        Box::new(bank::Loans::default()),
        Box::new(bank::Repayment::default()),
        Box::new(bank::NoCredit),
        Box::new(kalkulation::Figures),
        Box::new(kalkulation::Staff::default()),
        Box::new(kalkulation::Players::default()),
        Box::new(kalkulation::Running::default()),
        Box::new(kalkulation::OtherCosts::default()),
        Box::new(kalkulation::Tickets),
        Box::new(kalkulation::Sponsors),
        Box::new(kalkulation::Merchandise),
        Box::new(kalkulation::AgencyEngaged),
        Box::new(kalkulation::SponsorContract),
        Box::new(squad::SquadMenu::default()),
        Box::new(squad::CurrentSquad::default()),
        Box::new(squad::MarketValues::default()),
        Box::new(squad::NextSquad::default()),
        Box::new(squad::LoanedOut),
        Box::new(squad::Extensions::default()),
        Box::new(squad::DrawOption),
        Box::new(squad::Relations),
        Box::new(stadium::StadiumScreen::default()),
        Box::new(stadium::Expansion::default()),
        Box::new(stadium::StadiumView),
        Box::new(grounds::GroundsScreen::default()),
        Box::new(contacts::Contacts::default()),
        Box::new(board::Board::default()),
        Box::new(honours::Honours::default()),
        Box::new(honours::abilities::Abilities),
        Box::new(honours::pay::Pay),
        Box::new(honours::season_records::SeasonRecords),
        Box::new(honours::scores::Scores),
        Box::new(honours::trainer_erfolge::TrainerErfolge),
        Box::new(honours::privates::Privates::default()),
        Box::new(honours::beliebtheit::Beliebt),
        Box::new(contacts::Lawyers::default()),
        Box::new(contacts::AliensOffice::default()),
        Box::new(contacts::Referees::default()),
        Box::new(contacts::FanService::default()),
        Box::new(contacts::Cooperation::default()),
        Box::new(contacts::Reporter::default()),
        Box::new(contacts::ReporterClause),
        Box::new(contacts::Enquiry::default()),
        Box::new(contacts::Trainers::default()),
        Box::new(contacts::TrainerOffer::default()),
        Box::new(contacts::TrainerNews::default()),
        Box::new(contacts::TrainerExtend),
        Box::new(grounds::GroundsInfo),
        Box::new(grounds::GroundsStaff),
        Box::new(grounds::GroundsFans),
        Box::new(grounds::GroundsWorks),
        Box::new(search::ClubSearch::default()),
        Box::new(search::NoracsaSearch::default()),
        Box::new(search::NoracsaFound::default()),
        Box::new(search::NoracsaRequest::default()),
        Box::new(youth::Poaching::default()),
        Box::new(youth::PreContract),
        Box::new(youth::Youths::default()),
        Box::new(youth::Amateurs::default()),
        Box::new(youth::TrialChoice::default()),
        Box::new(market::TransferList::default()),
        Box::new(market::WorldsBest),
        Box::new(market::Appointments::default()),
        Box::new(market::LoanTerms),
        Box::new(market::WatchList::default()),
        Box::new(market::WatchedPlayer::default()),
        Box::new(talk::TalkScreen::default()),
        Box::new(talk::Mates::default()),
        Box::new(tactics::TacticsScreen::default()),
        Box::new(comparison::Comparison::default()),
        Box::new(training::ShortTraining::default()),
        Box::new(training::UnitChoice),
        Box::new(training::long::LongTraining::default()),
        Box::new(training::individual::IndividualTraining::default()),
        Box::new(training::individual::AbilityChoice::default()),
    ]
}

/// Adds the dialogs the game puts together from others, like the week screen
/// with a panel per club, and the controls it makes itself, like the menu
/// bar of the main screens.
pub fn compose(game: &mut Game) {
    calendar::compose(&mut game.dialogs);
    office::compose(&mut game.dialogs);
    hub::compose(&mut game.dialogs, &game.data.rules);
    formations::compose(&mut game.dialogs, &game.data.rules);
    special::compose(&mut game.dialogs);
    duties::compose(&mut game.dialogs);
    medical::compose(&mut game.dialogs);
    medical::compose_visit(&mut game.dialogs);
    opponent::compose(&mut game.dialogs);
    playerinfo::compose(&mut game.dialogs);
    discipline::compose(&mut game.dialogs);
    tactics::compose(&mut game.dialogs, &game.data.rules);
    comparison::compose(&mut game.dialogs, &game.data.rules);
    training::compose(&mut game.dialogs, &game.data.rules);
    training::long::compose(&mut game.dialogs, &game.data.rules);
    training::individual::compose(&mut game.dialogs, &game.data.rules);
    planner::compose(&mut game.dialogs, &game.data.rules);
    year::compose(&mut game.dialogs, &game.data.rules);
    finances::compose(&mut game.dialogs, &game.data.rules);
    bank::compose(&mut game.dialogs);
    kalkulation::compose(&mut game.dialogs);
    squad::compose(&mut game.dialogs, &game.data.rules);
    squad::compose_squad(&mut game.dialogs);
    squad::compose_values(&mut game.dialogs);
    squad::compose_next(&mut game.dialogs);
    stadium::compose(&mut game.dialogs, &game.data.rules);
    grounds::compose(&mut game.dialogs);
    contacts::compose(&mut game.dialogs, &game.data.rules);
    board::compose(&mut game.dialogs, &game.data.rules);
    honours::compose(&mut game.dialogs, &game.data.rules);
    honours::pay::compose(&mut game.dialogs);
    honours::season_records::compose(&mut game.dialogs);
    honours::scores::compose(&mut game.dialogs);
    honours::trainer_erfolge::compose(&mut game.dialogs);
    honours::privates::compose(&mut game.dialogs);
    honours::beliebtheit::compose(&mut game.dialogs);
    market::compose(&mut game.dialogs);
    market::compose_best(&mut game.dialogs);
    market::compose_talks(&mut game.dialogs);
    market::compose_loans(&mut game.dialogs);
    youth::compose(&mut game.dialogs);
    search::compose(&mut game.dialogs);
    start::compose(&mut game.dialogs);
    corner_buttons(&mut game.dialogs);
}

/// The help button and the printer's that a dialog's flags ask for, in
/// its top right corner, as `CDialogH` puts them there (`FUN_0043fb82`):
/// 30 and 40 pixels wide, two in from the edge, the printer's left of the
/// help button.
fn corner_buttons(dialogs: &mut HashMap<u16, Dialog>) {
    const TOP: f32 = 2.0;
    const BOTTOM: f32 = 23.0;
    const EDGE: f32 = 2.0;
    let buttons = [
        (HELP_FLAG, HELP, 30.0, (HELP_IMAGE, HELP_IMAGE + 1)),
        (PRINTER_FLAG, PRINTER, 40.0, (PRINTER_IMAGE, PRINTER_IMAGE + 1)),
    ];
    for dialog in dialogs.values_mut() {
        let frame = dialog.frame.unwrap_or(Rect::new(0.0, 0.0, 640.0, 480.0));
        let mut right = frame.max.x - EDGE;
        for (flag, id, width, (normal, hover)) in buttons {
            if dialog.flags & flag == 0 {
                continue;
            }
            dialog.controls.push(Control {
                kind: Kind::Button { normal, hover },
                rect: Rect::new(right - width, frame.min.y + TOP, right, frame.min.y + BOTTOM),
                align: Align::Left,
                text: String::new(),
                id: Some(id),
                font: PLAIN_FONT,
                ink: [0; 3],
            });
            right -= width + 1.0;
        }
    }
}

/// Whether `target` is a corner button, which does nothing yet.
pub(crate) fn is_corner_button(dialog: &Dialog, target: Target) -> bool {
    let Target::Button(i) = target else { return false };
    dialog.controls.get(i).is_some_and(|c| matches!(c.id, Some(HELP | PRINTER)))
}

/// The corner buttons, crossed out on every screen.
pub(crate) fn cross_corner_buttons(view: &mut View, dialog: &Dialog) {
    for i in 0..dialog.controls.len() {
        let target = Target::Button(i);
        if is_corner_button(dialog, target) && !view.inert.contains(&target) {
            view.inert.push(target);
        }
    }
}

/// The help button's id and pictures (the cap, held down); the printer's.
const HELP: u16 = 5417;
const HELP_IMAGE: u16 = 2446;
const PRINTER: u16 = 3945;
const PRINTER_IMAGE: u16 = 3378;

/// Where a window's title starts, in the caption font, as the original
/// writes it (`CMyDialog`, `FUN_007a0330`); and the font's height.
const CAPTION_AT: Vec2 = Vec2::new(8.0, 2.0);
const CAPTION_HEIGHT: f32 = 18.0;

/// A static for the caption of a window in `frame`, the whole screen
/// without one.
pub(crate) fn caption(frame: Option<Rect>, id: u16, text: String) -> Control {
    let frame = frame.unwrap_or(Rect::new(0.0, 0.0, 640.0, 480.0));
    let at = frame.min + CAPTION_AT;
    Control {
        kind: Kind::Static,
        rect: Rect::from_corners(at, Vec2::new(frame.max.x, at.y + CAPTION_HEIGHT)),
        align: Align::Left,
        text,
        id: Some(id),
        font: CAPTION_FONT,
        ink: [0; 3],
    }
}

/// The index of the dialog's control with `id`.
pub(crate) fn control(dialog: &Dialog, id: u16) -> Option<usize> {
    dialog.controls.iter().position(|c| c.id == Some(id))
}

/// The indices of the dialog's controls of a kind.
pub(crate) fn indices(dialog: &Dialog, kind: impl Fn(&Kind) -> bool) -> Vec<usize> {
    (0..dialog.controls.len()).filter(|&i| kind(&dialog.controls[i].kind)).collect()
}

/// OK's and cancel's pictures by the day (`FUN_0043ff5f`, the tables at
/// 0x878ed8 and 0x878f68): small and big, normal and pointed at, for
/// Christmas, the manager's birthday, autumn, carnival, spring, Easter,
/// summer, St Valentine's and winter.
const SEASONAL_OK: [[u16; 4]; 9] = [
    [2891, 2892, 1086, 2511],
    [2883, 2884, 1291, 1292],
    [2879, 2880, 1004, 1005],
    [2887, 2888, 1272, 1273],
    [2887, 2888, 1272, 1273],
    [2893, 2894, 2514, 2515],
    [2877, 2878, 1002, 1003],
    [2889, 2890, 1274, 1287],
    [2881, 2882, 2591, 2513],
];
const SEASONAL_CANCEL: [[u16; 4]; 9] = [
    [2909, 2910, 1268, 1269],
    [2901, 2902, 1226, 1227],
    [2897, 2898, 1256, 1257],
    [2905, 2906, 1260, 1261],
    [2905, 2906, 1260, 1261],
    [2911, 2912, 1262, 1263],
    [2895, 2896, 1264, 1265],
    [2907, 2908, 1266, 1267],
    [2899, 2900, 1270, 1271],
];
/// A button narrower than this takes the small pictures.
const SMALL_BUTTON: f32 = 32.0;

/// Which pictures the day gives OK and cancel.
fn season_look(cx: &Cx) -> Option<usize> {
    let records = cx.state.records.as_ref()?;
    let [d, m, year] = records.globals.date();
    let today = i32::from(records.globals.day_of_year());
    let easter = easter(i32::from(year));
    let manager = records.managers.get(records.human_slot(cx.state.season.as_ref()?.current)?);
    let birthday =
        manager.is_some_and(|r| u16::from(r.birth_day()) == d && u16::from(r.birth_month()) == m);
    let between = |from: (u16, u16), to: (u16, u16)| {
        let at = (m, d);
        (from.1, from.0) <= at && at <= (to.1, to.0)
    };
    Some(if today == easter {
        5
    } else if today == easter - 49 {
        3
    } else if birthday {
        1
    } else if (d, m) == (24, 12) {
        0
    } else if (d, m) == (14, 2) {
        7
    } else if between((21, 3), (20, 6)) {
        4
    } else if between((21, 6), (22, 9)) {
        6
    } else if between((23, 9), (20, 12)) {
        2
    } else {
        8
    })
}

/// Easter Sunday's day of the year as the game reckons it
/// (`FUN_00435d1f`).
fn easter(year: i32) -> i32 {
    let golden = year % 19 + 1;
    let century = year / 100 + 1;
    let shift = century * 3 / 4 - 12;
    let mut epact = (golden * 11 + (century * 8 + 5) / 25 + 15 - shift) % 30;
    if (epact == 25 && golden > 11) || epact == 24 {
        epact += 1;
    }
    let mut moon = 44 - epact;
    if moon < 21 {
        moon = 74 - epact;
    }
    let leap = i32::from((year % 4 == 0 && year % 100 != 0) || year % 400 == 0);
    moon - (year * 5 / 4 - shift - 10 + moon) % 7 + leap + 66
}

/// Gives OK and cancel the day's pictures (`FUN_0043ff5f`).
pub(crate) fn seasonal_buttons(cx: &Cx, view: &mut View) {
    let Some(look) = season_look(cx) else { return };
    let dialog = cx.dialog;
    for (id, table) in [(1, &SEASONAL_OK), (2, &SEASONAL_CANCEL)] {
        let Some(c) = control(dialog, id) else { continue };
        if !matches!(dialog.controls[c].kind, Kind::Button { .. }) {
            continue;
        }
        let [small, small_hover, big, big_hover] = table[look];
        let pictures = if dialog.controls[c].rect.width() < SMALL_BUTTON {
            (small, small_hover)
        } else {
            (big, big_hover)
        };
        if let Some(button) = view.controls[c].state.as_button_mut() {
            button.images = Some(pictures);
            button.ready = vec![pictures.0, pictures.1];
        }
    }
}

/// Marks the dialog's buttons and options that do nothing yet — all those
/// `works` does not answer for — for the looks to cross out.
pub(crate) fn mark_inert(view: &mut View, dialog: &Dialog, works: impl Fn(Target) -> bool) {
    let targets = crate::pointer::targets(dialog, view, &crate::pointer::Pixels);
    view.inert = targets
        .into_iter()
        .map(|(target, _)| target)
        .filter(|&t| matches!(t, Target::Button(_) | Target::Option { .. }) && !works(t))
        .collect();
    cross_corner_buttons(view, dialog);
}

/// Sets the text of the dialog's control with `id`.
pub(crate) fn set(view: &mut View, dialog: &Dialog, id: u16, text: String) {
    if let Some(i) = control(dialog, id) {
        view.set_text(i, text);
    }
}

/// Whether `target` is the OK or the cancel button of the dialog.
pub(crate) fn ok_or_cancel(dialog: &Dialog, target: Target) -> (bool, bool) {
    match target {
        Target::Button(i) => (is_ok(&dialog.controls[i]), is_cancel(&dialog.controls[i])),
        _ => (false, false),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn easter_falls_as_the_game_reckons_it() {
        // 20 April 2025 and 5 April 2026, counted from 1 January as 1.
        assert_eq!(super::easter(2025), 110);
        assert_eq!(super::easter(2026), 95);
    }
}
