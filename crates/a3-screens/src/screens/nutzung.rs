//! "Wie oft & Wie lange" of the dressing room's game options (dialog 158,
//! `CNutzungAnstoss2`): this session, the game, everything since the
//! installation and the longest session, each as days and a clock, and how
//! often the program was started and how many seasons were played. A
//! thread of the original's refreshes the times every second
//! (`OnUser480`); the screen does so on its ticks.

use std::collections::HashMap;
use std::time::Instant;

use a3_core::playtime::{Figures, split};
use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Screen, ok_or_cancel, set};
use crate::pointer::Target;
use crate::view::View;

pub const NUTZUNG: u16 = 158;
/// Each time's days and clock: the session, the game, the total, the
/// longest session.
const TIMES: [(u16, u16); 4] = [(2685, 2690), (2686, 2691), (2688, 2692), (6411, 6412)];
const STARTS: u16 = 2687;
const SEASONS: u16 = 2689;
const ONE_DAY: u16 = 1597;
const DAYS: u16 = 1598;
const REFRESH: f32 = 1.0;

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&NUTZUNG) else { return };
    let caption = super::caption(dialog.frame, super::discipline::CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct Nutzung {
    since: f32,
}

fn show(cx: &mut Cx, view: &mut View) {
    let Figures { session, game, total, longest } = cx.state.play.figures(Instant::now());
    for ((days_id, clock_id), seconds) in TIMES.into_iter().zip([session, game, total, longest]) {
        let (days, clock) = split(seconds);
        let days = match days {
            0 => String::new(),
            1 => cx.game.text(ONE_DAY).to_owned(),
            n => cx.game.text(DAYS).replacen("%d", &n.to_string(), 1),
        };
        set(view, cx.dialog, days_id, days);
        set(view, cx.dialog, clock_id, clock);
    }
    set(view, cx.dialog, STARTS, cx.state.play.starts.to_string());
    set(view, cx.dialog, SEASONS, cx.state.play.seasons.to_string());
}

impl Screen for Nutzung {
    fn ids(&self) -> &[u16] {
        &[NUTZUNG]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        self.since = 0.0;
        show(cx, view);
    }

    fn tick(&mut self, cx: &mut Cx, view: &mut View, seconds: f32) -> Outcome {
        self.since += seconds;
        if self.since >= REFRESH {
            self.since %= REFRESH;
            show(cx, view);
        }
        Outcome::Stay
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if ok || cancel { Outcome::Close(Reply::Ok) } else { Outcome::Stay }
    }
}
