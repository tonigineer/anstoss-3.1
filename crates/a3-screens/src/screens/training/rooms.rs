//! The training camp's rooms, the button 4890 in the long-term training
//! (dialog 407, `CZimmerbelegung`; TRAINING-LANG.md §5.1 in the RE repo):
//! twenty rooms of two, each player shown by name in his room, a pair in
//! colour where it gets on well or badly. A name dragged onto another place
//! swaps the two; onto Bianca's picture it swaps with her, and she shows in
//! his room ([`a3_core::rooms`]). A right click opens a player's info; OK
//! keeps the rooms.

use std::collections::HashMap;

use a3_core::rooms::{self, BIANCA, BIANCA_PLACE, PLACES, ROOMS};
use a3_core::season::ClubState;
use a3_files::rch::Dialog;

use super::super::discipline::CAPTION_ID;
use super::super::duties::short_name;
use super::super::playerinfo;
use super::super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::controls::ControlState;
use crate::pointer::{Grip, Target};
use crate::view::View;

/// The dialog, which the long-term training opens.
pub(crate) const ROOMS_DIALOG: u16 = 407;

/// Each room's two names, in the order the dialog keeps them (its table
/// at +0x24f0): down the left column, the middle, then the right.
const NAMES: [[u16; 2]; ROOMS] = [
    [4810, 4811],
    [4812, 4813],
    [4814, 4815],
    [4816, 4817],
    [4910, 4911],
    [4908, 4909],
    [4906, 4907],
    [4892, 4893],
    [4894, 4895],
    [4896, 4897],
    [4898, 4899],
    [4900, 4901],
    [4902, 4903],
    [4904, 4905],
    [4912, 4913],
    [4914, 4915],
    [4916, 4917],
    [4918, 4919],
    [4920, 4921],
    [4922, 4923],
];
/// Bianca's picture, and OK.
const PICTURE: u16 = 4824;
const OK: u16 = 1;
/// What an empty bed and Bianca show (`FUN_005d9f20`).
const FREE: u16 = 3311;
const BIANCA_NAME: u16 = 4695;
/// A name being dragged: a little window of this picture with the name
/// on it (`CZimmerWnd`).
const DRAGGED: u16 = 1254;

#[derive(Default)]
pub struct Rooms {
    /// Who stands in each place: two a room, then Bianca's own.
    places: Vec<u16>,
}

impl Screen for Rooms {
    fn ids(&self) -> &[u16] {
        &[ROOMS_DIALOG]
    }

    /// `FUN_005d90b5`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        *self = Self::default();
        // OK keeps its own picture: unlike most, this dialog does not ask
        // for the day's (`FUN_0043ff5f`).
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        let squad: Vec<u16> = own(cx).map(|c| c.status.iter().map(|s| s.id).collect()).unwrap_or_default();
        if let Some(records) = cx.state.records.as_mut() {
            self.places = rooms::seat(records, &squad).to_vec();
        }
        for id in NAMES.into_iter().flatten().chain([PICTURE]) {
            let Some(c) = control(dialog, id) else { continue };
            view.set_clickable(c, true);
            if let Some(ControlState::Static(s)) = view.controls.get_mut(c).map(|v| &mut v.state) {
                s.quiet = true;
            }
        }
        self.show(cx, view);
        mark_inert(view, dialog, |target| match target {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if dialog.controls[c].id == Some(OK) => {
                if let (Some(records), Ok(places)) =
                    (cx.state.records.as_mut(), <[u16; PLACES]>::try_from(self.places.as_slice()))
                {
                    rooms::keep(records, &places);
                }
                return Outcome::Close(Reply::Ok);
            }
            // A player let go of on another place, or on Bianca's picture,
            // swaps with whoever stands there (`OnLButtonUp_005d9ab4`).
            Target::Drop { from: Grip::Control(a), to: Some(Grip::Control(b)), .. } => {
                let (Some(from), Some(to)) = (place_of(dialog, a), place_of(dialog, b)) else {
                    return Outcome::Stay;
                };
                if from != BIANCA_PLACE && rooms::is_player(self.places[from]) {
                    self.places.swap(from, to);
                    self.show(cx, view);
                }
            }
            // A right click on a player opens his info, paging through
            // the squad (`OnRButtonDown_005d9d74`).
            Target::Menu(Grip::Control(c)) => {
                let Some(place) = place_of(dialog, c).filter(|&p| p != BIANCA_PLACE) else {
                    return Outcome::Stay;
                };
                let Some(club) = own(cx) else { return Outcome::Stay };
                let index = |id: u16| club.status.iter().position(|s| s.id == id);
                let Some(player) = index(self.places[place]) else { return Outcome::Stay };
                cx.state.paging = Some((0..club.status.len()).collect());
                return Outcome::Popup(Request::with(playerinfo::CURRENT, Args::Player(player)));
            }
            _ => {}
        }
        Outcome::Stay
    }
}

impl Rooms {
    /// Each room's names, in the pair's colour (`FUN_005d9f20`): a player
    /// by his short name, an empty bed "Frei", Bianca by hers; a player's
    /// name drags as a little window, the others not.
    fn show(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(club)) = (cx.state.records.as_ref(), own(cx)) else { return };
        let country = super::super::squad::own_team(cx).map_or(0, |t| t.country());
        view.drag_pictures.clear();
        for (room, names) in NAMES.iter().enumerate() {
            let pair = [self.places[2 * room], self.places[2 * room + 1]];
            let colour = rooms::pair_colour(records, pair, country, &game.rules.training.languages);
            let [r, g, b, _] = colour.to_le_bytes();
            for (&id, &name) in pair.iter().zip(names) {
                let Some(c) = control(dialog, name) else { continue };
                let text = match id {
                    rooms::NOBODY => game.text(FREE).to_owned(),
                    BIANCA => game.text(BIANCA_NAME).to_owned(),
                    _ => {
                        let p = club.status.iter().position(|s| s.id == id);
                        p.and_then(|p| club.players.get(p))
                            .map_or_else(String::new, |player| short_name(cx, player, id))
                    }
                };
                view.set_text(c, text);
                if let Some(ControlState::Static(s)) = view.controls.get_mut(c).map(|v| &mut v.state) {
                    s.ink = Some([r, g, b]);
                }
                if rooms::is_player(id) {
                    view.drag_pictures.push((c, DRAGGED));
                }
            }
        }
    }
}

/// The place a control stands for: a room's bed, or Bianca's picture.
fn place_of(dialog: &Dialog, c: usize) -> Option<usize> {
    let id = dialog.controls.get(c)?.id?;
    if id == PICTURE {
        return Some(BIANCA_PLACE);
    }
    NAMES.iter().flatten().position(|&n| n == id)
}

/// The manager's club, as the season follows it.
fn own<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

/// Puts the dialog's caption in its frame.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&ROOMS_DIALOG) else { return };
    let caption = super::super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}
