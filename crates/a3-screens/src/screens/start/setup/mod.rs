//! A new game, through the game's own sequence with the rules each of its
//! screens applies: countries and game length, number of players, a name per
//! player, then for every player a club and national team (see [`Club`]),
//! abilities and birthday (see [`Abilities`]), and finally the start
//! settings, after which the game cycle calculates the first season. The
//! setup itself is the game's (see [`a3_core::newgame`]).

mod club;
mod trainer;

pub use club::{Club, NationalTeam};
pub use trainer::{Abilities, Birthday};
pub(super) use trainer::{BIRTHDAY_SPINS, show_birthday, valid_date};

use a3_core::cycle::dialogs::{COUNTRIES, NAMES, PLAYERS, SETTINGS};
use a3_core::newgame::{DEFAULT_DIFFICULTY, DURATIONS, Setup, Trainer};
use a3_core::world::{BONUS_SLOT, MAIN_SLOTS, SLOTS};
use a3_files::rch::{Dialog, Kind};

use super::{chosen, fill_in, fill_list, set_chosen};
use crate::game::Game;
use crate::pointer::Target;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, control, indices, ok_or_cancel};
use crate::view::View;

/// Controls by their ids in the dialog file.
const PLAYER_ORDER: u16 = 3640;
/// The country screen: the main countries' flags and names, and the extra
/// countries' names and flags, by country slot.
const MAIN_FLAGS: [u16; 5] = [2478, 2479, 2480, 5349, 5350];
const MAIN_LABELS: [u16; 5] = [2474, 2475, 2476, 5351, 5352];
const EXTRA_LABELS: [u16; SLOTS] =
    [5317, 2488, 2489, 5318, 5319, 5320, 5324, 5321, 5322, 5353, 5354, 5348];
const EXTRA_FLAGS: [u16; SLOTS] =
    [5311, 2490, 2491, 5312, 5313, 5314, 5323, 5315, 5316, 5345, 5346, 5347];
const CAREER: u16 = 4779;
const DURATION: u16 = 2481;
const PLAYER_COUNT: u16 = 4772;

/// Each country's waving flag: its first frame's graphic id and how many
/// follow, by country slot.
const FLAG_FRAMES: [(u16, u16); SLOTS] = [
    (1526, 10),
    (1546, 10),
    (1536, 10),
    (1556, 10),
    (1566, 10),
    (2760, 10),
    (2700, 10),
    (2750, 10),
    (2720, 10),
    (2730, 10),
    (2710, 9),
    (2740, 10),
];
/// At most this many countries are played besides the main one.
const MAX_EXTRA: usize = 4;
const NAME_MIN: usize = 2;
/// Texts of the game, by id in `ListeDerTexte.rh`.
const DURATION_ONE_YEAR: u16 = 1719;
const DURATION_YEARS: u16 = 1720;

/// The countries played, the career and the game length.
pub struct Countries;

impl Screen for Countries {
    fn ids(&self) -> &[u16] {
        &[COUNTRIES]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        show_countries(view, cx.dialog, cx.game, &cx.state.setup);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        let setup = &mut cx.state.setup;
        match target {
            Target::Control(i) => {
                pick_country(setup, cx.dialog.controls[i].id.unwrap_or(0));
                show_countries(view, cx.dialog, cx.game, setup);
                Outcome::Stay
            }
            Target::SliderStep { control, .. } | Target::SliderAt { control, .. } => {
                setup.duration = view.value(control).map_or(0, |v| v.value).max(0) as usize;
                show_countries(view, cx.dialog, cx.game, setup);
                Outcome::Stay
            }
            Target::Option { control, .. } => {
                setup.career = view.chosen(control);
                Outcome::Stay
            }
            _ if ok => Outcome::Close(Reply::Ok),
            _ if cancel => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }
}

/// How many play.
pub struct Players;

impl Screen for Players {
    fn ids(&self) -> &[u16] {
        &[PLAYERS]
    }

    fn open(&mut self, _: &mut Cx, _: &mut View, _: Args) {}

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if ok {
            let count = control(cx.dialog, PLAYER_COUNT).map_or(0, |i| view.chosen(i)) + 1;
            let setup = &mut cx.state.setup;
            setup.trainers = vec![Trainer::default(); count];
            setup.current = 0;
            return Outcome::Close(Reply::Ok);
        }
        if cancel { Outcome::Close(Reply::Cancel) } else { Outcome::Stay }
    }
}

/// Each player's name and difficulty, one after the other.
pub struct Names;

impl Screen for Names {
    fn ids(&self) -> &[u16] {
        &[NAMES]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let state = &cx.state;
        fill_in(view, cx.dialog, state.setup.current + 1, &state.player.name());
        view.focus = indices(cx.dialog, |k| matches!(k, Kind::Edit { .. })).first().copied();
        set_chosen(view, cx.dialog, DEFAULT_DIFFICULTY);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if cancel {
            return Outcome::Close(Reply::Cancel);
        }
        if !ok {
            return Outcome::Stay;
        }
        let [first, last, ..] = indices(cx.dialog, |k| matches!(k, Kind::Edit { .. }))[..] else {
            return Outcome::Stay;
        };
        let first_name = view.text(first).trim().to_owned();
        let last_name = view.text(last).trim().to_owned();
        // Both names need two letters; the field that lacks them gets the focus.
        if first_name.chars().count() < NAME_MIN {
            view.focus = Some(first);
            return Outcome::Stay;
        }
        if last_name.chars().count() < NAME_MIN {
            view.focus = Some(last);
            return Outcome::Stay;
        }
        let difficulty = chosen(view, cx.dialog).unwrap_or(DEFAULT_DIFFICULTY);
        let setup = &mut cx.state.setup;
        if let Some(trainer) = setup.trainer_mut() {
            trainer.first_name = first_name;
            trainer.last_name = last_name;
            trainer.difficulty = difficulty;
        }
        Outcome::Close(Reply::Ok)
    }
}

/// The start settings, with the order the players take their turns in.
pub struct Settings;

impl Screen for Settings {
    fn ids(&self) -> &[u16] {
        &[SETTINGS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let order = cx
            .state
            .setup
            .trainers
            .iter()
            .enumerate()
            .map(|(i, t)| format!("{}. {} {}", i + 1, t.first_name, t.last_name))
            .collect();
        fill_list(view, cx.dialog, PLAYER_ORDER, order, &[0.0]);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match ok_or_cancel(cx.dialog, target) {
            (true, _) => Outcome::Close(Reply::Ok),
            (_, true) => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }
}

/// A click on the country screen. A main country becomes the main country,
/// and no longer an extra one; an extra country is added or removed — at
/// most four, and never the main country.
///
/// The game answers only on a main country's flag and an extra country's
/// name; here flag and name both do.
fn pick_country(setup: &mut Setup, id: u16) {
    let slot_of = |ids: &[u16]| ids.iter().position(|&i| i == id);
    if let Some(slot) = slot_of(&MAIN_FLAGS).or_else(|| slot_of(&MAIN_LABELS)) {
        setup.extra[slot] = false;
        setup.main = MAIN_SLOTS[slot];
    } else if let Some(slot) = slot_of(&EXTRA_LABELS).or_else(|| slot_of(&EXTRA_FLAGS)) {
        if slot == setup.main {
            return;
        }
        let chosen = setup.extra.iter().filter(|&&e| e).count();
        if setup.extra[slot] || chosen < MAX_EXTRA {
            setup.extra[slot] = !setup.extra[slot];
        }
    }
}

/// The country screen: flags wave for the main country and the chosen extra
/// ones; the bonus country shows only if one is installed; the last career
/// option names the game length the slider sets.
fn show_countries(view: &mut View, dialog: &Dialog, game: &Game, setup: &Setup) {
    let frames = |slot: usize| {
        let (first, count) = FLAG_FRAMES[slot];
        (first..first + count).collect::<Vec<u16>>()
    };
    for (slot, id) in MAIN_FLAGS.iter().enumerate() {
        if let Some(flag) = control(dialog, *id).and_then(|i| view.anim_mut(i)) {
            flag.frames = frames(slot);
            flag.playing = setup.main == slot;
        }
    }
    for (slot, id) in EXTRA_FLAGS.iter().enumerate() {
        if let Some(flag) = control(dialog, *id).and_then(|i| view.anim_mut(i)) {
            flag.frames = frames(slot);
            flag.playing = setup.extra[slot];
        }
    }
    for id in MAIN_FLAGS.iter().chain(&MAIN_LABELS).chain(&EXTRA_FLAGS).chain(&EXTRA_LABELS) {
        if let Some(i) = control(dialog, *id) {
            view.set_clickable(i, true);
        }
    }
    let bonus = game.world.country(BONUS_SLOT).map(|c| game.world.nation(c.nation).to_owned());
    for id in [EXTRA_LABELS[BONUS_SLOT], EXTRA_FLAGS[BONUS_SLOT]] {
        if let Some(i) = control(dialog, id) {
            view.controls[i].hidden = bonus.is_none();
        }
    }
    if let (Some(i), Some(name)) = (control(dialog, EXTRA_LABELS[BONUS_SLOT]), bonus) {
        view.set_text(i, name);
    }
    if let Some(i) = control(dialog, CAREER) {
        view.set_chosen(i, setup.career);
        let years = DURATIONS[setup.duration.min(DURATIONS.len() - 1)];
        let label = if setup.duration == 0 {
            game.text(DURATION_ONE_YEAR).to_owned()
        } else {
            game.text(DURATION_YEARS).replace("%d", &years.to_string())
        };
        if let Some(last) = view.options_mut(i).and_then(|o| o.labels.last_mut()) {
            *last = label;
        }
    }
    if let Some(slider) = control(dialog, DURATION).and_then(|i| view.value_mut(i)) {
        slider.range = (0, DURATIONS.len() as i32 - 1);
        slider.value = setup.duration as i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_most_four_extra_countries_and_never_the_main_one() {
        let mut setup = Setup::default();
        // Germany is the main country; its extra entry does nothing.
        pick_country(&mut setup, EXTRA_LABELS[0]);
        assert!(!setup.extra[0]);
        for &label in &EXTRA_LABELS[1..=5] {
            pick_country(&mut setup, label);
        }
        assert_eq!(setup.extra.iter().filter(|&&e| e).count(), MAX_EXTRA);
        assert!(!setup.extra[5]);
        // Making England the main country drops it from the extra ones.
        pick_country(&mut setup, MAIN_FLAGS[1]);
        assert_eq!(setup.main, 1);
        assert!(!setup.extra[1]);
        pick_country(&mut setup, EXTRA_LABELS[1]);
        assert!(!setup.extra[1]);
        // Flags and names do the same.
        pick_country(&mut setup, EXTRA_FLAGS[2]);
        assert!(!setup.extra[2]);
        pick_country(&mut setup, EXTRA_FLAGS[2]);
        assert!(setup.extra[2]);
        pick_country(&mut setup, MAIN_LABELS[0]);
        assert_eq!(setup.main, 0);
    }
}
