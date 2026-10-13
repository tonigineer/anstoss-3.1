//! A network game: the player's name and difficulty, birthday, and the list
//! of games to host or join.

use a3_core::cycle::dialogs::{NET_DETAILS, NET_GAMES, NET_NAME};
use a3_core::newgame::{Mode, Player};
use a3_files::rch::{Dialog, Kind};

use super::setup::{BIRTHDAY_SPINS, show_birthday, valid_date};
use super::{chosen, fill_in, fill_list, line_up_headers, set_chosen};
use crate::pointer::Target;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, indices, ok_or_cancel};
use crate::view::View;

const GAME_LIST: u16 = 4979;
const GAME_TABS: [f32; 3] = [0.0, 150.0, 205.0];

/// The player's name and difficulty.
pub struct Name;

impl Screen for Name {
    fn ids(&self) -> &[u16] {
        &[NET_NAME]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let state = &cx.state;
        fill_in(view, cx.dialog, state.setup.current + 1, &state.player.name());
        let edits = edits(cx.dialog);
        if let [first, last, ..] = edits[..] {
            view.set_text(first, state.player.first_name.clone());
            view.set_text(last, state.player.last_name.clone());
            view.focus = Some(first);
        }
        set_chosen(view, cx.dialog, state.player.difficulty);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if ok {
            let state = &mut cx.state;
            if let [first, last, ..] = edits(cx.dialog)[..] {
                state.player.first_name = view.text(first).trim().to_owned();
                state.player.last_name = view.text(last).trim().to_owned();
            }
            remember_difficulty(view, cx.dialog, &mut state.player);
            return Outcome::Close(Reply::Ok);
        }
        if cancel { Outcome::Close(Reply::Cancel) } else { Outcome::Stay }
    }
}

/// The player's difficulty and birthday, for hosting a game.
pub struct Details;

impl Screen for Details {
    fn ids(&self) -> &[u16] {
        &[NET_DETAILS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let state = &cx.state;
        fill_in(view, cx.dialog, state.setup.current + 1, &state.player.name());
        set_chosen(view, cx.dialog, state.player.difficulty);
        show_birthday(view, cx.dialog, state.player.birthday, cx.game.world.start_year);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, _) = ok_or_cancel(cx.dialog, target);
        let player = &mut cx.state.player;
        match target {
            Target::Spin { control, up } => {
                let id = cx.dialog.controls[control].id.unwrap_or(0);
                if let Some(part) = BIRTHDAY_SPINS.iter().position(|&s| s == id) {
                    player.birthday[part] += if up { 1 } else { -1 };
                    player.birthday = valid_date(player.birthday);
                    show_birthday(view, cx.dialog, player.birthday, cx.game.world.start_year);
                }
                Outcome::Stay
            }
            _ if ok => {
                remember_difficulty(view, cx.dialog, player);
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}

/// The games on the network: the player's own when hosting, none found when
/// joining.
pub struct Games;

impl Screen for Games {
    fn ids(&self) -> &[u16] {
        &[NET_GAMES]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let state = &cx.state;
        let rows = if state.mode == Mode::Client {
            Vec::new()
        } else {
            let host = format!("{} {}", state.player.first_name, state.player.last_name);
            let host = if host.trim().is_empty() { "Trainer" } else { host.trim() };
            vec![format!("{host}\t1/4\toffen")]
        };
        fill_list(view, cx.dialog, GAME_LIST, rows, &GAME_TABS);
        line_up_headers(view, cx.dialog);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match ok_or_cancel(cx.dialog, target) {
            (true, _) => Outcome::Close(Reply::Ok),
            (_, true) => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }
}

fn edits(dialog: &Dialog) -> Vec<usize> {
    indices(dialog, |k| matches!(k, Kind::Edit { .. }))
}

fn remember_difficulty(view: &View, dialog: &Dialog, player: &mut Player) {
    if let Some(difficulty) = chosen(view, dialog) {
        player.difficulty = difficulty;
    }
}
