//! The manager's ten best seasons (dialog 174 `CSaisonrekorde`;
//! ERFOLGE.md §A.4): what `FUN_006d6264` kept of his evaluations at the
//! season's ends. No save yet at hand has earned one, so the list stays
//! empty there too.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use crate::pointer::Target;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, caption, control, mark_inert, set};
use crate::view::View;

pub(crate) const SEASON_RECORDS_DIALOG: u16 = 174;
const CAPTION_ID: u16 = 9943;
const LIST: u16 = 2830;
/// "The date" text.
const DATE: u16 = 1997;
const NOTHING: &str = "-";

/// The window's own caption bar, with its stored title.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&SEASON_RECORDS_DIALOG) else { return };
    dialog.controls.insert(0, caption(dialog.frame, CAPTION_ID, String::new()));
}

/// The screen.
pub struct SeasonRecords;

impl Screen for SeasonRecords {
    fn ids(&self) -> &[u16] {
        &[SEASON_RECORDS_DIALOG]
    }

    /// `FUN_0053952c`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        let Some(records) = cx.state.records.as_ref() else { return };
        let rows = records
            .season_marks()
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let club = if m.club.0 == 0 {
                    NOTHING.to_owned()
                } else {
                    game.club_name(Some(records), m.club.0, m.club.1)
                };
                let (day, month, year) = m.date;
                let date = game
                    .text(DATE)
                    .replace("%1", &day.to_string())
                    .replace("%2", &month.to_string())
                    .replace("%3", &year.to_string());
                format!("{}\t{}\t{}\t{}\t{}", i + 1, m.name, m.points, club, date)
            })
            .collect();
        if let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) {
            list.rows = rows;
            list.selected = None;
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&cx.dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}
