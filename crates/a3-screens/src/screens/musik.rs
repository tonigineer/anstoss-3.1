//! Music and sound effects, from the dressing room's game options (dialog
//! 259, `CStereoanlage`): how the music plays, which pieces it plays, and
//! whether the game makes its sounds. A click on a piece switches it, red
//! when on (`FUN_005833a2`). It opens on what `musik.cfg` holds, read once
//! as the game reads it at its start (`CMusik::FUN_006eca39`); OK keeps it
//! all for the session (`FUN_00583551`), cancel leaves it. Playing a
//! piece on its own, "Musikstücke anspielen", stays crossed out.

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
/// What the game holds without `musik.cfg`: no music, no sounds, every
/// piece but the third and the fifteenth.
const NO_FILE: (u16, u16, u32) = (0, 0, 0xbffb);
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
#[derive(Default)]
pub struct Musik {
    read: bool,
    mode: usize,
    sounds: bool,
    pieces: u32,
    shown: u32,
}

/// `musik.cfg`: the mode (16 bits, 0–3), the sounds (16 bits, 0–1) and the
/// pieces (32 bits), each kept in its range as the game keeps it.
fn settings(game: &crate::game::Game) -> (u16, u16, u32) {
    let Ok(cfg) = std::fs::read(game.dir.join("musik.cfg")) else { return NO_FILE };
    let word = |at: usize| cfg.get(at..at + 2).map(|b| i16::from_le_bytes([b[0], b[1]]));
    let mode = word(0).map_or(NO_FILE.0, |m| m.clamp(0, 3) as u16);
    let sounds = word(2).map_or(NO_FILE.1, |s| s.clamp(0, 1) as u16);
    let pieces =
        cfg.get(4..8).map_or(NO_FILE.2, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    (mode, sounds, pieces)
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
        if !self.read {
            let (mode, sounds, pieces) = settings(cx.game);
            (self.mode, self.sounds, self.pieces) = (mode.into(), sounds != 0, pieces);
            self.read = true;
        }
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
            cx.state.music = Some((self.mode as u16, u16::from(self.sounds), self.pieces));
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}
