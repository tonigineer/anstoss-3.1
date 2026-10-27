//! Music and sound effects, from the dressing room's game options (dialog
//! 259, `CStereoanlage`): how the music plays, which pieces it plays, and
//! whether the game makes its sounds. A click on a piece switches it, red
//! when on (`FUN_005833a2`); OK keeps it all for the session
//! (`FUN_00583551`), cancel leaves it. The replica plays no music, so
//! "Musikstücke anspielen" stays crossed out.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel};
use crate::pointer::Target;
use crate::view::View;

pub const MUSIK: u16 = 259;
const MODE: u16 = 6451;
const PIECES: u16 = 6453;
const LISTEN: u16 = 6452;
const SOUNDS: u16 = 3696;
/// The game's sixteen pieces (`PTR_DAT_0088a2f0`); `None` where its table
/// leaves one out (`DAT_0088a330`).
const TITLES: [Option<&str>; 16] = [
    Some("1987"),
    Some("Anstoß 2 Forever!"),
    None,
    Some("I Lost My Job"),
    Some("Suspension"),
    Some("Sad But True"),
    Some("Great Expectations"),
    Some("Getting Better"),
    Some("Deep Depression"),
    Some("High Hopes"),
    Some("Brasil"),
    Some("When All Else Fails"),
    Some("Baby Please Don't Go"),
    Some("Dreaming"),
    None,
    Some("Go West"),
];
const ON: [u8; 3] = [255, 0, 0];
/// "Alle hintereinander abspielen", as the original starts.
const ALL_IN_TURN: usize = 2;
/// "Bestimmte Musikstücke spielen": with no piece left on, OK makes it
/// "Keine Musik".
const CHOSEN_PIECES: usize = 3;
/// What OK keeps of the pieces: the two highest are dropped, "Go West"
/// among them.
const KEPT: u32 = 0x3fff;

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&MUSIK) else { return };
    let caption = super::caption(dialog.frame, super::discipline::CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

/// The settings OK last kept, and the pieces switched in the open dialog.
pub struct Musik {
    mode: usize,
    sounds: bool,
    pieces: u32,
    shown: u32,
}

impl Default for Musik {
    fn default() -> Self {
        Self { mode: ALL_IN_TURN, sounds: true, pieces: u32::from(u16::MAX), shown: 0 }
    }
}

/// The slots of the listed pieces, row by row.
fn slots() -> impl Iterator<Item = usize> {
    TITLES.iter().enumerate().filter(|(_, t)| t.is_some()).map(|(slot, _)| slot)
}

impl Musik {
    fn show_pieces(&self, dialog: &Dialog, view: &mut View) {
        let Some(list) = control(dialog, PIECES).and_then(|c| view.list_mut(c)) else { return };
        list.rows = TITLES.iter().flatten().map(|&t| t.to_owned()).collect();
        list.inks = slots().map(|slot| (self.shown & 1 << slot != 0).then_some(ON)).collect();
        list.selected = None;
    }
}

impl Screen for Musik {
    fn ids(&self) -> &[u16] {
        &[MUSIK]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        if let Some(c) = control(dialog, MODE) {
            view.set_chosen(c, self.mode);
        }
        if let Some(check) = control(dialog, SOUNDS).and_then(|c| view.check_mut(c)) {
            check.checked = self.sounds;
        }
        // Its toggle starts up (style 0x400).
        if let Some(listen) = control(dialog, LISTEN) {
            view.set_chosen(listen, crate::controls::options::NONE);
        }
        self.shown = self.pieces;
        self.show_pieces(dialog, view);
        super::mark_inert(view, dialog, |target| match target {
            Target::Option { control: c, .. } => dialog.controls[c].id != Some(LISTEN),
            _ => true,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let row = match target {
            Target::Cell { control: c, row, .. } | Target::Row { control: c, row } => {
                (dialog.controls[c].id == Some(PIECES)).then_some(row)
            }
            _ => None,
        };
        if let Some(row) = row {
            if let Some(slot) = slots().nth(row) {
                self.shown ^= 1 << slot;
            }
            self.show_pieces(dialog, view);
            return Outcome::Stay;
        }
        let (ok, cancel) = ok_or_cancel(dialog, target);
        if cancel {
            return Outcome::Close(Reply::Cancel);
        }
        if ok {
            self.mode = control(dialog, MODE).map_or(self.mode, |c| view.chosen(c));
            self.sounds = control(dialog, SOUNDS)
                .and_then(|c| view.check(c))
                .map_or(self.sounds, |check| check.checked);
            self.pieces = self.shown & KEPT;
            if self.mode == CHOSEN_PIECES && self.pieces == 0 {
                self.mode = 0;
            }
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}
