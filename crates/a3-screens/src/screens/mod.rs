//! The game's screens, a module per area of the game. The game cycle asks
//! for a screen (see [`a3_core::cycle`]); the screen fills its dialog's view
//! as it opens and answers what the player does there, may ask for a popup
//! over itself, and closes with a reply — to the cycle, or a popup's to the
//! screen below it. The [`crate::host`] keeps them open.

pub mod bank;
pub mod calendar;
pub mod comparison;
pub mod discipline;
pub mod duties;
pub mod finances;
pub mod formations;
pub mod hub;
pub mod jobs;
pub mod market;
pub mod medical;
pub mod office;
pub mod opponent;
pub mod planner;
pub mod playerinfo;
pub mod popups;
pub mod special;
pub mod squad;
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
        Box::new(squad::SquadMenu::default()),
        Box::new(squad::CurrentSquad::default()),
        Box::new(squad::MarketValues::default()),
        Box::new(squad::NextSquad::default()),
        Box::new(squad::LoanedOut),
        Box::new(squad::Extensions::default()),
        Box::new(squad::DrawOption),
        Box::new(youth::Poaching::default()),
        Box::new(youth::PreContract),
        Box::new(youth::Youths::default()),
        Box::new(youth::Amateurs::default()),
        Box::new(youth::TrialChoice::default()),
        Box::new(market::TransferList::default()),
        Box::new(market::WorldsBest),
        Box::new(market::Appointments::default()),
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
    squad::compose(&mut game.dialogs, &game.data.rules);
    squad::compose_squad(&mut game.dialogs);
    squad::compose_values(&mut game.dialogs);
    squad::compose_next(&mut game.dialogs);
    market::compose(&mut game.dialogs);
    market::compose_best(&mut game.dialogs);
    market::compose_talks(&mut game.dialogs);
    youth::compose(&mut game.dialogs);
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
