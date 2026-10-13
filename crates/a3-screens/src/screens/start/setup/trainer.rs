//! The trainer's abilities and playing strength, for the points the league
//! gives, then the trainer's birthday and position as a player.

use a3_files::rch::{Dialog, Kind};

use a3_core::cycle::dialogs::{ABILITIES, BIRTHDAY};
use a3_core::newgame::{DEFAULT_BIRTHDAY, DEFAULT_POSITION, Setup};

use crate::controls::button::OK_IMAGE;
use crate::game::Game;
use crate::pointer::Target;
use crate::screens::start::fill_in;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, control, indices, ok_or_cancel};
use crate::view::View;

/// Controls by their ids in the dialog file.
const BIRTHDAY_FIELDS: [u16; 3] = [6197, 6198, 6199];
pub(in crate::screens::start) const BIRTHDAY_SPINS: [u16; 3] = [6200, 6201, 6202];
const POSITION: u16 = 6204;
const BUDGET: u16 = 4692;
const POINTS: u16 = 4701;
const ABILITY_VALUES: [u16; 8] = [4695, 4696, 4697, 4698, 4699, 4700, 5611, 6205];
const ABILITY_SPINS: [u16; 8] = [4702, 4703, 4704, 4705, 4706, 4707, 6033, 6206];
/// Abilities run from 0 to 15.
const ABILITY_MAX: i32 = 15;
/// Trainers are born from 1900 on and at least this old when the game starts.
const OLDEST_BIRTH_YEAR: i32 = 1900;
const YOUNGEST_AGE: i32 = 18;

/// Texts of the game, by id in `ListeDerTexte.rh`.
const ONE_POINT_OVER: u16 = 3817;
const POINTS_OVER: u16 = 3818;
const ONE_POINT_LEFT: u16 = 3819;
const POINTS_LEFT: u16 = 3820;
const ALL_POINTS_SPENT: u16 = 3821;

/// The trainer's abilities and playing strength, for the points the league
/// gives.
pub struct Abilities;

impl Screen for Abilities {
    fn ids(&self) -> &[u16] {
        &[ABILITIES]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let setup = &mut cx.state.setup;
        fill_in(view, cx.dialog, setup.current + 1, &setup.name());
        if let Some(trainer) = setup.trainer_mut() {
            // Every ability starts at the level, which spends the points.
            trainer.abilities = [trainer.level; 7];
            trainer.strength = 1;
            let budget = trainer.budget().to_string();
            if let Some(i) = control(cx.dialog, BUDGET) {
                view.set_text(i, cx.dialog.controls[i].text.replace("%s", &budget));
            }
        }
        show_abilities(view, cx.dialog, cx.game, setup);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, _) = ok_or_cancel(cx.dialog, target);
        match target {
            Target::Spin { control, .. } => {
                let value = view.value(control).map_or(0, |v| v.value);
                let id = cx.dialog.controls[control].id.unwrap_or(0);
                let spin = ABILITY_SPINS.iter().position(|&s| s == id);
                let setup = &mut cx.state.setup;
                if let (Some(spin), Some(trainer)) = (spin, setup.trainer_mut()) {
                    match trainer.abilities.get_mut(spin) {
                        Some(ability) => *ability = value,
                        None => trainer.strength = value,
                    }
                }
                show_abilities(view, cx.dialog, cx.game, setup);
                Outcome::Stay
            }
            _ if ok => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// The trainer's birthday and position as a player.
pub struct Birthday;

impl Screen for Birthday {
    fn ids(&self) -> &[u16] {
        &[BIRTHDAY]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let setup = &mut cx.state.setup;
        fill_in(view, cx.dialog, setup.current + 1, &setup.name());
        if let Some(trainer) = setup.trainer_mut() {
            trainer.birthday = DEFAULT_BIRTHDAY;
            trainer.position = DEFAULT_POSITION;
            show_birthday(view, cx.dialog, trainer.birthday, cx.game.world.start_year);
            if let Some(i) = control(cx.dialog, POSITION) {
                view.set_chosen(i, trainer.position);
            }
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        let spin = |view: &View, id: u16| {
            control(cx.dialog, id).and_then(|i| view.value(i)).map_or(1, |v| v.value)
        };
        let setup = &mut cx.state.setup;
        match target {
            Target::Spin { .. } => {
                let date = BIRTHDAY_SPINS.map(|id| spin(view, id));
                show_birthday(view, cx.dialog, date, cx.game.world.start_year);
                if let Some(trainer) = setup.trainer_mut() {
                    // The day may have been pulled back into a shorter month.
                    trainer.birthday = BIRTHDAY_SPINS.map(|id| spin(view, id));
                }
                Outcome::Stay
            }
            _ if ok => {
                let position = control(cx.dialog, POSITION).map(|i| view.chosen(i));
                if let (Some(position), Some(trainer)) = (position, setup.trainer_mut()) {
                    trainer.position = position;
                }
                Outcome::Close(Reply::Ok)
            }
            _ if cancel => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }
}

/// The abilities and playing strength with their spinners' ranges, what is
/// left of the points, and OK only once they are all given out.
fn show_abilities(view: &mut View, dialog: &Dialog, game: &Game, setup: &Setup) {
    let Some(trainer) = setup.trainer() else { return };
    let values = trainer.abilities.iter().copied().chain(std::iter::once(trainer.strength));
    for (n, value) in values.enumerate() {
        if let Some(i) = control(dialog, ABILITY_VALUES[n]) {
            view.set_text(i, value.to_string());
        }
        if let Some(spin) = control(dialog, ABILITY_SPINS[n]).and_then(|i| view.value_mut(i)) {
            let is_strength = n == trainer.abilities.len();
            spin.range = if is_strength { (1, trainer.strongest()) } else { (0, ABILITY_MAX) };
            spin.value = value;
        }
    }
    let (spent, budget) = (trainer.abilities.iter().sum::<i32>(), trainer.budget());
    let points = match spent - budget {
        0 => game.text(ALL_POINTS_SPENT).to_owned(),
        -1 => game.text(ONE_POINT_LEFT).to_owned(),
        1 => game.text(ONE_POINT_OVER).to_owned(),
        over if over > 0 => game.text(POINTS_OVER).replace("%d", &over.to_string()),
        under => game.text(POINTS_LEFT).replace("%d", &(-under).to_string()),
    };
    if let Some(i) = control(dialog, POINTS) {
        view.set_text(i, points);
    }
    for button in indices(dialog, |k| matches!(k, Kind::Button { normal: OK_IMAGE, .. })) {
        view.set_disabled(button, spent != budget);
    }
}

/// Day, month and year, with their spinners' ranges: a trainer is born from
/// 1900 on and at least 18 when the game starts in `start_year`, on a day the
/// month has.
pub(in crate::screens::start) fn show_birthday(
    view: &mut View,
    dialog: &Dialog,
    [day, month, year]: [i32; 3],
    start_year: i32,
) {
    let ranges =
        [(1, days_in_month(month, year)), (1, 12), (OLDEST_BIRTH_YEAR, start_year - YOUNGEST_AGE)];
    // A shorter month pulls the day back.
    let date = [day.min(ranges[0].1), month, year];
    for part in 0..3 {
        if let Some(i) = control(dialog, BIRTHDAY_FIELDS[part]) {
            view.set_text(i, date[part].to_string());
        }
        if let Some(spin) = control(dialog, BIRTHDAY_SPINS[part]).and_then(|i| view.value_mut(i)) {
            spin.range = ranges[part];
            spin.value = date[part];
        }
    }
}

fn days_in_month(month: i32, year: i32) -> i32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Wraps day and month around and keeps the year plausible for a trainer.
pub(in crate::screens::start) fn valid_date([day, month, year]: [i32; 3]) -> [i32; 3] {
    let year = year.clamp(1930, 1985);
    let month = (month - 1).rem_euclid(12) + 1;
    [(day - 1).rem_euclid(days_in_month(month, year)) + 1, month, year]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birthday_wraps_like_a_calendar() {
        assert_eq!(valid_date([32, 1, 1975]), [1, 1, 1975]);
        assert_eq!(valid_date([0, 3, 1975]), [31, 3, 1975]);
        assert_eq!(valid_date([30, 2, 1976]), [1, 2, 1976]);
        assert_eq!(valid_date([1, 13, 1990]), [1, 1, 1985]);
    }
}
