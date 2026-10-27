//! The 3D options of the dressing room's game options (dialog 511,
//! `CKonfig3DDetail`): the resolution, the quality of the figures and of
//! the textures, and five effects. It opens on what `Konfig3D.cfg` holds,
//! read once as the game reads it (`FUN_00433e30`);
//! OK keeps what was set for the rest of the session, as the original
//! keeps it in its globals (`FUN_004f9086`); cancel leaves them.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel};
use crate::pointer::Target;
use crate::view::View;

pub const KONFIG_3D: u16 = 511;
const FIGURES: u16 = 6466;
const TEXTURES: u16 = 6475;
/// The five effects in the file's order, the dialog's own order of them
/// (Bengalische Feuer, Sonstige Effekte, Beleuchtung, Schatten, Details
/// Rasen/Himmel); the oracle's file has them all on, so the order is the
/// dialog's members', not seen.
const EFFECTS: [u16; 5] = [6467, 6468, 6472, 6473, 6474];
/// The resolutions the 3D device offers; the replica has none to ask, so
/// it shows the file's and drops no list.
const RESOLUTION: u16 = 6469;
/// What the game holds without `Konfig3D.cfg`: 640x480, both qualities
/// "Hoch", every effect on.
const NO_FILE: [u32; 10] = [640, 480, 0, 2, 2, 1, 1, 1, 1, 1];

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&KONFIG_3D) else { return };
    let caption = super::caption(dialog.frame, super::discipline::CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

/// What OK last kept, the file's until then.
#[derive(Default)]
pub struct Konfig3d {
    read: bool,
    resolution: String,
    figures: usize,
    textures: usize,
    effects: [bool; 5],
}

/// `Konfig3D.cfg`: ten words — width, height, depth, the two qualities and
/// the five effects — and two more the dialog does not show.
fn settings(game: &crate::game::Game) -> [u32; 10] {
    let mut words = NO_FILE;
    if let Ok(cfg) = std::fs::read(game.dir.join("Konfig3D.cfg")) {
        for (i, word) in words.iter_mut().enumerate() {
            if let Some(b) = cfg.get(i * 4..i * 4 + 4) {
                *word = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            }
        }
    }
    words
}

impl Screen for Konfig3d {
    fn ids(&self) -> &[u16] {
        &[KONFIG_3D]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        if !self.read {
            let words = settings(cx.game);
            self.resolution = format!("{}x{}", words[0], words[1]);
            (self.figures, self.textures) = (words[3] as usize, words[4] as usize);
            for (on, &word) in self.effects.iter_mut().zip(&words[5..]) {
                *on = word != 0;
            }
            self.read = true;
        }
        let dialog = cx.dialog;
        for (id, chosen) in [(FIGURES, self.figures), (TEXTURES, self.textures)] {
            if let Some(c) = control(dialog, id) {
                view.set_chosen(c, chosen);
            }
        }
        for (id, on) in EFFECTS.into_iter().zip(self.effects) {
            if let Some(check) = control(dialog, id).and_then(|c| view.check_mut(c)) {
                check.checked = on;
            }
        }
        if let Some(c) = control(dialog, RESOLUTION) {
            if let Some(combo) = view.combo_mut(c) {
                combo.items = vec![self.resolution.clone()];
                combo.selected = Some(0);
            }
            view.inert = vec![Target::Combo(c)];
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let (Target::Combo(c), Some(combo)) = (target, control(dialog, RESOLUTION)) {
            if c == combo
                && let Some(state) = view.combo_mut(c)
            {
                state.open = false;
            }
            return Outcome::Stay;
        }
        let (ok, cancel) = ok_or_cancel(dialog, target);
        if cancel {
            return Outcome::Close(Reply::Cancel);
        }
        if ok {
            let chosen = |id| control(dialog, id).map(|c| view.chosen(c));
            self.figures = chosen(FIGURES).unwrap_or(self.figures);
            self.textures = chosen(TEXTURES).unwrap_or(self.textures);
            for (on, id) in self.effects.iter_mut().zip(EFFECTS) {
                if let Some(check) = control(dialog, id).and_then(|c| view.check(c)) {
                    *on = check.checked;
                }
            }
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}
