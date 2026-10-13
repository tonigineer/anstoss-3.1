//! The game's screens, a module per area of the game. The game cycle asks
//! for a screen (see [`a3_core::cycle`]); the screen fills its dialog's view
//! as it opens and answers what the player does there, may ask for a popup
//! over itself, and closes with a reply — to the cycle, or a popup's to the
//! screen below it. The [`crate::host`] keeps them open.

pub mod calendar;
pub mod discipline;
pub mod formations;
pub mod hub;
pub mod jobs;
pub mod office;
pub mod planner;
pub mod playerinfo;
pub mod popups;
pub mod special;
pub mod start;

pub use a3_core::cycle::{Args, Reply, Request};
use a3_core::state::GameState;
use a3_files::rch::{Dialog, Kind};

use crate::controls::button::{CANCEL_IMAGE, OK_IMAGE};
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
        Box::new(playerinfo::PlayerInfo::default()),
        Box::new(office::Office),
        Box::new(discipline::Bonus::default()),
        Box::new(discipline::Fine::default()),
        Box::new(discipline::FineReply),
        Box::new(discipline::Change),
        Box::new(planner::Planner::default()),
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
    playerinfo::compose(&mut game.dialogs);
    discipline::compose(&mut game.dialogs);
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
}

/// Sets the text of the dialog's control with `id`.
pub(crate) fn set(view: &mut View, dialog: &Dialog, id: u16, text: String) {
    if let Some(i) = control(dialog, id) {
        view.set_text(i, text);
    }
}

/// Whether `target` is the OK or the cancel button of the dialog.
pub(crate) fn ok_or_cancel(dialog: &Dialog, target: Target) -> (bool, bool) {
    let image = match target {
        Target::Button(i) => match dialog.controls[i].kind {
            Kind::Button { normal, .. } => normal,
            _ => 0,
        },
        _ => 0,
    };
    (image == OK_IMAGE, image == CANCEL_IMAGE)
}
