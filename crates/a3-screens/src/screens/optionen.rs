//! The game options of the dressing room (dialog 503, `COptionenAnstoss3`):
//! thirty switches over the game's flow, the match's presentation, the
//! time limit and the optional game elements, each a push button that
//! steps through its values and names the one it stands on; the autosave's
//! weeks on a spinner; the manager's start screen and his difficulty.
//!
//! The switches are the 39 bytes of the save's globals at `0xb5d13c`, read
//! as the dialog opens and written back by OK (`FUN_0052a46e`); the start
//! screen is bits 6–7 of the manager's options, the difficulty the low
//! seven bits of his +0x114, which OK changes once only: then its bit 0x80
//! locks the button (`FUN_006a655e`). Cancel keeps nothing. A changed
//! start screen counts from his next turn; the dressing room stays
//! (`CMyStartDialog` opens it again on −2).

use std::collections::HashMap;

use a3_core::records::Records;
use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel};
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

pub const OPTIONEN: u16 = 503;
const MENU: u16 = 6346;
const DIFFICULTY: u16 = 6352;
const DIFFICULTY_TITLE: u16 = 6351;
const AUTOSAVE_WEEKS: u16 = 6348;
const AUTOSAVE_SHOWN: u16 = 6347;
/// The block's byte of the autosave's weeks, 1 to 10.
const WEEKS_BYTE: usize = 0x1f;
const WEEKS: (i32, i32) = (1, 10);
/// The labels' first text; each value names its label by its place in the
/// table (`DAT_00886120`), past 32 three texts of their own.
const LABELS: u16 = 4653;
const LATER_LABELS: [u16; 3] = [4973, 5116, 5146];
/// Where the start screens' and the difficulties' labels begin.
const MENU_LABELS: u8 = 0x17;
const DIFFICULTY_LABELS: u8 = 0x1c;
const LOCKED: u16 = 0x80;

/// How a switch steps: between two values, the second whenever it is not
/// on the first; round a cycle of its values; or along a table.
#[derive(Clone, Copy)]
enum Step {
    Pair(u8, u8),
    Cycle(&'static [u8]),
}

/// Each switch's button, its byte in the block and how it steps
/// (`OnClicked_IDC_OPTIONEN_…`).
const SWITCHES: [(u16, usize, Step); 28] = [
    (6320, 0x00, Step::Pair(1, 0)),
    (6321, 0x01, Step::Pair(1, 0)),
    (6322, 0x02, Step::Pair(1, 0)),
    (6323, 0x03, Step::Pair(9, 8)),
    (6324, 0x04, Step::Pair(1, 0)),
    (6325, 0x05, Step::Pair(1, 0)),
    (6326, 0x06, Step::Pair(1, 0)),
    (6327, 0x07, Step::Pair(1, 0)),
    // Tabellen: an, alle, aus.
    (6328, 0x08, Step::Cycle(&[1, 2, 0])),
    (6329, 0x09, Step::Pair(1, 0)),
    (6331, 0x0a, Step::Pair(1, 0)),
    (6330, 0x0b, Step::Pair(1, 0)),
    (6332, 0x0c, Step::Pair(1, 0)),
    (6333, 0x0d, Step::Pair(1, 0)),
    (6334, 0x0e, Step::Pair(1, 0)),
    (6335, 0x0f, Step::Pair(1, 0)),
    // Spieldarstellung, as a single game with the 3D view steps it:
    // Grafikmodus, Nur Szenen, Textmodus, Torboxen, Aus.
    (6336, 0x10, Step::Cycle(&[0x1a, 0x21, 0x1b, 0x22, 0])),
    (6337, 0x11, Step::Pair(5, 6)),
    (6338, 0x12, Step::Pair(1, 0)),
    (6339, 0x13, Step::Pair(1, 0)),
    // Unbegrenzt, then one to nine minutes (`DAT_0088615f`).
    (6340, 0x14, Step::Cycle(&[13, 14, 15, 16, 17, 18, 19, 20, 21, 22])),
    (6341, 0x15, Step::Pair(10, 11)),
    // Vorbereitung: schnell, nur Pflichtspiele, normal.
    (6342, 0x16, Step::Cycle(&[7, 12, 5])),
    (6343, 0x17, Step::Pair(1, 0)),
    (6344, 0x18, Step::Pair(1, 0)),
    (6345, 0x19, Step::Pair(1, 0)),
    (6350, 0x1d, Step::Pair(1, 0)),
    (6349, 0x1e, Step::Pair(1, 0)),
];

impl Step {
    fn next(self, value: u8) -> u8 {
        match self {
            Step::Pair(on, other) => {
                if value == on {
                    other
                } else {
                    on
                }
            }
            Step::Cycle(values) => match values.iter().position(|&v| v == value) {
                Some(i) => values[(i + 1) % values.len()],
                None => values[0],
            },
        }
    }
}

fn label(game: &Game, value: u8) -> String {
    let id = match value.checked_sub(33) {
        Some(later) => LATER_LABELS.get(usize::from(later)).copied().unwrap_or(LABELS),
        None => LABELS + u16::from(value),
    };
    game.text(id).to_owned()
}

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&OPTIONEN) else { return };
    let caption = super::caption(dialog.frame, super::discipline::CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

/// Whether the game has the options to show: a loaded one, whose records
/// hold them.
pub(crate) fn available(cx: &Cx) -> bool {
    current(cx).is_some()
}

fn current(cx: &Cx) -> Option<usize> {
    cx.state.records.as_ref()?.human_slot(cx.state.season.as_ref()?.current)
}

/// The block, start screen and difficulty while the dialog is open.
pub struct Optionen {
    block: [u8; 39],
    menu: u8,
    difficulty: u16,
    locked: bool,
    title: String,
}

impl Default for Optionen {
    fn default() -> Self {
        Self { block: [0; 39], menu: 0, difficulty: 0, locked: false, title: String::new() }
    }
}

impl Optionen {
    fn show(&self, game: &Game, dialog: &Dialog, view: &mut View) {
        for (id, byte, _) in SWITCHES {
            if let Some(c) = control(dialog, id) {
                view.set_text(c, label(game, self.block[byte]));
            }
        }
        if let Some(c) = control(dialog, MENU) {
            view.set_text(c, label(game, self.menu + MENU_LABELS));
        }
        if let Some(c) = control(dialog, DIFFICULTY) {
            view.set_text(c, label(game, self.difficulty as u8 + DIFFICULTY_LABELS));
        }
        if let Some(c) = control(dialog, AUTOSAVE_SHOWN) {
            view.set_text(c, self.block[WEEKS_BYTE].to_string());
        }
    }
}

impl Screen for Optionen {
    fn ids(&self) -> &[u16] {
        &[OPTIONEN]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        let Some(slot) = current(cx) else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(manager) = records.managers.get(slot) else { return };
        self.block = records.globals.game_options();
        self.menu = manager.menu_view().min(2);
        self.difficulty = (manager.difficulty() & !LOCKED).min(4);
        self.locked = manager.difficulty() & LOCKED != 0;
        // "Schwierigkeitsgrad (bisher %s)", with the one he has.
        let format = control(dialog, DIFFICULTY_TITLE).map(|c| view.text(c).to_owned());
        if self.title.is_empty() {
            self.title = format.unwrap_or_default();
        }
        if let Some(c) = control(dialog, DIFFICULTY_TITLE) {
            let name = label(cx.game, self.difficulty as u8 + DIFFICULTY_LABELS);
            view.set_text(c, self.title.replacen("%s", &name, 1));
        }
        if let Some(c) = control(dialog, DIFFICULTY) {
            view.set_disabled(c, self.locked);
        }
        if let Some(weeks) = control(dialog, AUTOSAVE_WEEKS).and_then(|c| view.value_mut(c)) {
            weeks.range = WEEKS;
            weeks.set(i32::from(self.block[WEEKS_BYTE]));
            self.block[WEEKS_BYTE] = weeks.value as u8;
        }
        self.show(cx.game, dialog, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Spin { control: c, .. } if id(c) == Some(AUTOSAVE_WEEKS) => {
                if let Some(weeks) = view.value(c) {
                    self.block[WEEKS_BYTE] = weeks.value as u8;
                }
            }
            Target::Button(b) if id(b) == Some(MENU) => self.menu = (self.menu + 1) % 3,
            Target::Button(b) if id(b) == Some(DIFFICULTY) => {
                self.difficulty = (self.difficulty + 1) % 5;
            }
            Target::Button(b) => {
                let Some(&(_, byte, step)) = SWITCHES.iter().find(|s| id(b) == Some(s.0)) else {
                    return self.close(cx, target);
                };
                self.block[byte] = step.next(self.block[byte]);
            }
            _ => return Outcome::Stay,
        }
        self.show(cx.game, dialog, view);
        Outcome::Stay
    }
}

impl Optionen {
    fn close(&mut self, cx: &mut Cx, target: Target) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if cancel {
            return Outcome::Close(Reply::Cancel);
        }
        if !ok {
            return Outcome::Stay;
        }
        let Some(slot) = current(cx) else { return Outcome::Close(Reply::Ok) };
        if let Some(records) = cx.state.records.as_mut() {
            write(records, slot, self);
        }
        if let Some(season) = cx.state.season.as_mut() {
            let current = season.current;
            season.managers[current].menu = self.menu;
            if !self.locked {
                season.managers[current].difficulty = usize::from(self.difficulty);
            }
        }
        Outcome::Close(Reply::Ok)
    }
}

/// OK's writes (`FUN_0052a46e`, and `CUmkleide::FUN_005b1516` after it).
fn write(records: &mut Records, slot: usize, options: &Optionen) {
    records.globals.set_game_options(options.block);
    let display = options.block[0x10];
    if display != 0x23 {
        records.globals.set_last_display(display.into());
    }
    let Some(manager) = records.managers.get_mut(slot) else { return };
    manager.set_menu_view(options.menu);
    let had = manager.difficulty();
    if had & LOCKED == 0 && had & 0x7f != options.difficulty {
        manager.set_difficulty(had & 0xff80 | options.difficulty | LOCKED);
    }
}
