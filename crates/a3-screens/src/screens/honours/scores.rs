//! The lists of highscores (dialog 95 `CHighscoreListe`; ERFOLGE.md
//! §A.5): the careers from `Highscore/high09` and the nine fixed games
//! one entry apiece — files of the installation, not of the save. The
//! start menu's entry leads here as well: the original opens the same
//! dialog there, so this screen runs both routes.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use crate::game::HighEntry;
use crate::pointer::Target;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, caption, control, mark_inert, set};
use crate::view::View;

pub(crate) const SCORES_DIALOG: u16 = 95;
pub(crate) const CAPTION_ID: u16 = 9944;
const CAREER: u16 = 2010;
const FIXED: u16 = 2011;
/// "The date" text.
const DATE: u16 = 1997;

/// The caption bar across the top of the full screen.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&SCORES_DIALOG) else { return };
    dialog.controls.insert(0, caption(dialog.frame, CAPTION_ID, String::new()));
}

/// The screen.
pub struct Scores;

impl Scores {
    /// One entry's row (`FUN_004c61f5`): place, both names, the shown
    /// count, points, their rate over `divisor`, level, titles and date.
    fn row(
        game: &crate::game::Game,
        place: usize,
        e: &HighEntry,
        shown: i32,
        divisor: i32,
    ) -> String {
        let rate = if divisor > 0 { e.points / divisor } else { 0 };
        let (day, month, year) = e.date;
        let date = game
            .text(DATE)
            .replace("%1", &day.to_string())
            .replace("%2", &month.to_string())
            .replace("%3", &year.to_string());
        format!(
            "{}.\t{} {}\t{}\t{}\t{}\t{}\t{}\t{}",
            place,
            e.first,
            e.last,
            shown,
            e.points,
            rate,
            e.level,
            i32::from(e.titles) + i32::from(e.more_titles),
            date
        )
    }
}

impl Screen for Scores {
    fn ids(&self) -> &[u16] {
        &[SCORES_DIALOG]
    }

    /// `FUN_004c61f5`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        // The careers: the age at the third column, the rate over the
        // seasons.
        let career = game
            .high_entries(9)
            .iter()
            .enumerate()
            .filter(|(_, e)| e.points != 0)
            .map(|(i, e)| Self::row(game, i + 1, e, i32::from(e.counter), e.seasons))
            .collect();
        // The fixed games: seasons at that place, each file's one entry
        // standing at place 1.
        let fixed = (0..9u8)
            .flat_map(|cat| {
                let entries = game.high_entries(cat);
                entries
                    .into_iter()
                    .enumerate()
                    .filter(|(_, e)| e.points != 0)
                    .map(move |(i, e)| {
                        Self::row(game, i + 1, &e, i32::from(e.counter), i32::from(e.counter))
                    })
            })
            .collect();
        for (id, rows) in [(CAREER, career), (FIXED, fixed)] {
            if let Some(list) = control(dialog, id).and_then(|c| view.list_mut(c)) {
                list.rows = rows;
                list.selected = None;
            }
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
