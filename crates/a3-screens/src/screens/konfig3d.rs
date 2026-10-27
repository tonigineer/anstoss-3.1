//! The 3D options of the dressing room's game options (dialog 511,
//! `CKonfig3DDetail`): the resolution, the quality of the figures and of
//! the textures, and five effects. OK keeps what was set for the rest of
//! the session, as the original keeps it in its globals
//! (`FUN_004f9086`); cancel leaves them.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel};
use crate::pointer::Target;
use crate::view::View;

pub const KONFIG_3D: u16 = 511;
const FIGURES: u16 = 6466;
const TEXTURES: u16 = 6475;
/// Beleuchtung, Schatten, Details Rasen/Himmel, Bengalische Feuer,
/// Sonstige Effekte.
const EFFECTS: [u16; 5] = [6472, 6473, 6474, 6467, 6468];
/// The resolutions the 3D device offers; the replica has none to ask, so
/// it shows the one the original showed under Wine and drops no list.
const RESOLUTION: u16 = 6469;
const SHOWN_RESOLUTION: &str = "640x480";
/// "Hoch", the original's choice for both qualities.
const HIGH: usize = 2;

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&KONFIG_3D) else { return };
    let caption = super::caption(dialog.frame, super::discipline::CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

/// What OK last kept: both qualities high and every effect on, as the
/// original shows them on a fresh start.
pub struct Konfig3d {
    figures: usize,
    textures: usize,
    effects: [bool; 5],
}

impl Default for Konfig3d {
    fn default() -> Self {
        Self { figures: HIGH, textures: HIGH, effects: [true; 5] }
    }
}

impl Screen for Konfig3d {
    fn ids(&self) -> &[u16] {
        &[KONFIG_3D]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
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
                combo.items = vec![SHOWN_RESOLUTION.to_owned()];
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
