//! The start menu, "Spielvorbereitung", and the screens it leads to besides
//! a new game (see [`setup`]) and the network games (see [`network`]): the
//! saved games. The high scores its fifth entry opens are dialog 95, which
//! the honours page runs as well (`crate::screens::honours::scores`) — the
//! original opens the very same `CHighscoreListe` from here.

pub mod network;
pub mod setup;

use chrono::{DateTime, Local};

pub use a3_core::cycle::dialogs::START;
use a3_core::cycle::dialogs::LOAD;
use a3_files::rch::{Dialog, Kind};

use super::{Args, Cx, Outcome, Reply, Screen, control, indices, ok_or_cancel};
use crate::pointer::Target;
use crate::view::View;

/// Controls by their ids in the dialog file.
/// The start menu's title, written into its frame as every window's.
const CAPTION_ID: u16 = 9935;

/// The start menu's caption (shot of the original: "ANSTOSS 3 -
/// Spielvorbereitung").
pub(crate) fn compose(dialogs: &mut std::collections::HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&a3_core::cycle::dialogs::START) else { return };
    let title = dialog.title.clone();
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, title));
}

const SAVE_LIST: u16 = 5144;
const SAVE_INFO: u16 = 5146;
/// Column starts of the lists, in classic pixels from the list's left edge.
const SAVE_TABS: [f32; 3] = [0.0, 225.0, 318.0];
const INFO_TABS: [f32; 2] = [0.0, 95.0];

/// The start menu, which remembers the entry last chosen.
#[derive(Default)]
pub struct StartMenu {
    choice: usize,
}

impl Screen for StartMenu {
    fn ids(&self) -> &[u16] {
        &[START]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        set_chosen(view, cx.dialog, self.choice);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, double: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if ok || double && matches!(target, Target::Option { .. }) {
            let options = indices(cx.dialog, |k| matches!(k, Kind::Options { .. }));
            self.choice = options.first().map_or(0, |&o| view.chosen(o));
            return Outcome::Close(Reply::Choice(self.choice));
        }
        if cancel { Outcome::Close(Reply::Cancel) } else { Outcome::Stay }
    }
}

/// The saved games, with what each one holds.
pub struct LoadGame;

impl Screen for LoadGame {
    fn ids(&self) -> &[u16] {
        &[LOAD]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let rows = cx
            .game
            .save_games()
            .iter()
            .map(|save| {
                let saved = DateTime::<Local>::from(save.saved);
                format!("{}\t{}\t{}", save.name, saved.format("%d.%m.%Y"), saved.format("%H:%M"))
            })
            .collect();
        fill_list(view, cx.dialog, SAVE_LIST, rows, &SAVE_TABS);
        show_save_info(view, cx);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, double: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        match target {
            Target::Row { .. } => {
                show_save_info(view, cx);
                if double { load(view, cx) } else { Outcome::Stay }
            }
            _ if ok => load(view, cx),
            _ if cancel => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }
}

/// The selected saved game is the one to load.
fn load(view: &View, cx: &mut Cx) -> Outcome {
    let Some(state) = control(cx.dialog, SAVE_LIST).and_then(|l| view.list(l)) else {
        return Outcome::Stay;
    };
    let Some(row) = state.selected.and_then(|r| state.rows.get(r)) else { return Outcome::Stay };
    cx.state.save = row.split('\t').next().map(str::to_owned);
    Outcome::Close(Reply::Ok)
}

fn show_save_info(view: &mut View, cx: &Cx) {
    let Some(list) = control(cx.dialog, SAVE_LIST).and_then(|l| view.list(l)) else { return };
    let name = list
        .selected
        .and_then(|r| list.rows.get(r))
        .and_then(|row| row.split('\t').next().map(str::to_owned));
    let save = name.and_then(|name| cx.game.save_games().into_iter().find(|s| s.name == name));
    let rows = save.map_or_else(Vec::new, |save| {
        let saved = DateTime::<Local>::from(save.saved);
        vec![
            format!("Trainer:\t{}", save.trainer),
            format!("Verein:\t{}", save.club),
            format!("Land:\t{}", save.country),
            format!("Gespeichert:\t{}", saved.format("%d.%m.%Y, %H:%M Uhr")),
        ]
    });
    fill_list(view, cx.dialog, SAVE_INFO, rows, &INFO_TABS);
    if let Some(info) = control(cx.dialog, SAVE_INFO).and_then(|i| view.list_mut(i)) {
        info.selected = None;
    }
}

/// Puts the number and name of the player whose turn it is in where the
/// dialog says `%d` and `%s`.
fn fill_in(view: &mut View, dialog: &Dialog, number: usize, name: &str) {
    let number = number.to_string();
    for (i, control) in dialog.controls.iter().enumerate() {
        view.set_text(i, control.text.replace("%d", &number).replace("%s", name));
    }
}

/// Column titles line up with the list below them.
fn line_up_headers(view: &mut View, dialog: &Dialog) {
    for header in indices(dialog, |k| matches!(k, Kind::Header { .. })) {
        let below = dialog.controls[header].rect.max.y;
        let list = indices(dialog, |k| matches!(k, Kind::ListBox { .. }))
            .into_iter()
            .filter(|&l| dialog.controls[l].rect.min.y >= below - 1.0)
            .min_by(|&a, &b| {
                dialog.controls[a].rect.min.y.total_cmp(&dialog.controls[b].rect.min.y)
            });
        if let Some(list) = list {
            let shift = dialog.controls[list].rect.min.x - dialog.controls[header].rect.min.x;
            let tabs = view.list(list).map(|l| l.tabs.iter().map(|t| t + shift).collect());
            if let (Some(tabs), Some(header)) = (tabs, view.header_mut(header)) {
                header.tabs = tabs;
            }
        }
    }
}

/// Chooses an option of the dialog's first group of bulleted options.
fn set_chosen(view: &mut View, dialog: &Dialog, choice: usize) {
    let options = indices(dialog, |k| matches!(k, Kind::Options { image: 0, .. }));
    if let Some(&o) = options.first() {
        view.set_chosen(o, choice);
    }
}

/// The choice in the dialog's first group of bulleted options.
fn chosen(view: &View, dialog: &Dialog) -> Option<usize> {
    let options = indices(dialog, |k| matches!(k, Kind::Options { image: 0, .. }));
    options.first().map(|&o| view.chosen(o))
}

fn fill_list(view: &mut View, dialog: &Dialog, id: u16, rows: Vec<String>, tabs: &[f32]) {
    if let Some(state) = control(dialog, id).and_then(|l| view.list_mut(l)) {
        state.selected = (!rows.is_empty()).then_some(0);
        state.scroll = 0;
        state.rows = rows;
        state.tabs = tabs.to_vec();
    }
}
