//! The notepad of the week planner (dialog 397, `CDiNotizen`): the
//! manager's own page of notes, which he may type over and keep, or empty
//! with löschen and keep that. Every human has his own page, the
//! manager's number counting from the first (`FUN_00528dfd`).

use std::collections::HashMap;

use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel};
use crate::pointer::Target;
use crate::view::View;

pub const NOTEPAD: u16 = 397;
/// The page: one big edit over the picture's middle.
const PAGE: u16 = 5414;
/// Löschen: empties the edit, which OK then keeps.
const CLEAR: u16 = 5416;

/// The dialog keeps its title to itself; the caption bar takes it, as the
/// original writes it there (`CMyDialog`).
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&NOTEPAD) else { return };
    let caption = super::caption(dialog.frame, super::discipline::CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

/// Whose page this is: the current manager's slot — his club's record
/// knows him by that number plus one, and the pages of the four humans
/// follow it (`DAT_008ed400` + (human − 1) × 0x420).
fn page(cx: &Cx) -> Option<usize> {
    let season = cx.state.season.as_ref()?;
    cx.state.records.as_ref()?.human_slot(season.current)
}

#[derive(Default)]
pub struct Notepad;

impl Screen for Notepad {
    fn ids(&self) -> &[u16] {
        &[NOTEPAD]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let text = page(cx)
            .and_then(|page| Some(cx.state.records.as_ref()?.notes.get(page)?.text()))
            .unwrap_or_default();
        let Some(edit) = control(cx.dialog, PAGE) else { return };
        view.set_text(edit, text);
        view.focus = Some(edit);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if matches!(target, Target::Button(b) if cx.dialog.controls[b].id == Some(CLEAR)) {
            if let Some(edit) = control(cx.dialog, PAGE) {
                view.set_text(edit, String::new());
            }
            return Outcome::Stay;
        }
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if cancel {
            return Outcome::Close(Reply::Cancel);
        }
        if ok {
            let page = page(cx);
            let text = control(cx.dialog, PAGE).map_or_else(String::new, |c| view.text(c).to_owned());
            if let Some(records) = cx.state.records.as_mut()
                && let Some(page) = page
                && let Some(note) = records.notes.get_mut(page)
            {
                note.set_text(&text);
            }
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}
