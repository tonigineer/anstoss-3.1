//! The comparison of the humans with each other (dialog 211
//! `CSpielerbewertung`; ERFOLGE.md §A.3 and §A.6): nine questions down
//! the left, a column for every human with a mark apiece, and the sum of
//! the lit questions under each name. Nothing is kept when it closes.

use crate::controls::options::NONE;
use crate::pointer::Target;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::view::View;

pub(crate) const VERGLEICH_DIALOG: u16 = 211;
/// The nine questions, all lit when the window opens.
const QUESTIONS: [u16; 9] = [3061, 3062, 3063, 3064, 3065, 3066, 3067, 3068, 3069];
/// The twelve statics of a column: first name, last name, the nine
/// marks and their sum (`0x888620[12c + k]`).
const COLUMN: u16 = 3070;
const COLUMN_STEP: u16 = 12;
const COLUMNS: usize = 4;
const MARKS: usize = 9;

#[derive(Default)]
pub struct Vergleich {
    /// The columns that stand for a human, and their marks; the marks
    /// of the lit questions make the sum under the names.
    filled: usize,
    marks: [[i32; MARKS]; COLUMNS],
}

impl Vergleich {
    /// The sums under the names, of the lit questions only
    /// (`FUN_00558e37` — which rewrites nothing else).
    fn sums(&self, cx: &mut Cx, view: &mut View) {
        let dialog = cx.dialog;
        let mut lit = [false; MARKS];
        for (k, q) in QUESTIONS.iter().enumerate() {
            lit[k] = control(dialog, *q).is_some_and(|c| view.chosen(c) != NONE);
        }
        for c in 0..self.filled {
            let sum: i32 = (0..MARKS).filter(|&k| lit[k]).map(|k| self.marks[c][k]).sum();
            let base = COLUMN + c as u16 * COLUMN_STEP;
            set(view, dialog, base + COLUMN_STEP - 1, sum.to_string());
        }
    }
}

impl Screen for Vergleich {
    fn ids(&self) -> &[u16] {
        &[VERGLEICH_DIALOG]
    }

    /// `FUN_00558a9c`: every question lit, a column for every human
    /// among the managers, his mark for every question — and for his
    /// standing also, in brackets — while the spare columns stay blank.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        self.filled = 0;
        self.marks = [[0; MARKS]; COLUMNS];
        for q in QUESTIONS {
            if let Some(c) = control(dialog, q) {
                view.set_chosen(c, 0);
            }
        }
        if let Some(records) = cx.state.records.as_ref() {
            for (c, manager) in records.humans().take(COLUMNS).enumerate() {
                self.marks[c] = a3_core::evaluation::marks(records, manager, c + 1);
                self.filled = c + 1;
                let base = COLUMN + c as u16 * COLUMN_STEP;
                set(view, dialog, base, manager.first_name());
                set(view, dialog, base + 1, manager.last_name());
                for k in 0..MARKS - 1 {
                    set(view, dialog, base + 2 + k as u16, self.marks[c][k].to_string());
                }
                set(
                    view,
                    dialog,
                    base + 2 + (MARKS as u16 - 1),
                    format!("{} ({})", self.marks[c][MARKS - 1], manager.standing()),
                );
            }
        }
        for c in self.filled..COLUMNS {
            let base = COLUMN + c as u16 * COLUMN_STEP;
            for k in 0..COLUMN_STEP {
                set(view, dialog, base + k, String::new());
            }
        }
        self.sums(cx, view);
        let works = |t: Target| match t {
            Target::Button(c) => crate::controls::button::is_ok(&dialog.controls[c]),
            Target::Option { control: c, item } => {
                item == 0 && QUESTIONS.iter().any(|&q| dialog.controls[c].id == Some(q))
            }
            _ => false,
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            // A click puts a question out or back in; only the sums move.
            Target::Option { control: c, .. }
                if QUESTIONS.iter().any(|&q| dialog.controls[c].id == Some(q)) =>
            {
                self.sums(cx, view);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}
