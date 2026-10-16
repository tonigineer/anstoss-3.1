//! Scripts that play the game without a window: for tests, and for running
//! the original under the probe with the same script.
//!
//! A script is a list of lines:
//!
//! - `# …` — a comment;
//! - `seed <n>` — where a new game's random numbers start;
//! - `on <dialog> <actions>` — what to do every time the dialog waits for
//!   the player;
//! - `once <dialog> <actions>` — what to do the next time only, ahead of the
//!   dialog's `on`; several queue up;
//! - `until dialog <id>`, `until day <n>`, `until end` — plays until that
//!   dialog waits, the season reaches that day, or the game ends;
//! - `repeat <n> days` — plays until the season has moved on that far.
//!
//! Actions, separated by `;`, act on the dialog's controls by their ids in
//! the dialog file: `ok`, `cancel`, `press <button>`, `click <static or
//! animation>`, `click bar <symbol>`, `choose <options> <item>`,
//! `row <list> <row>`, `cell <list> <row> <column>`, `type <field> <text>`,
//! `spin <spinner> up|down` (a slider's arrows too), `slide <slider>
//! <value>`, `rclick <control>`, and `wait` until the screen moves on by
//! itself. A dialog the script has no actions for stops it, as does an
//! action for a control the dialog does not have.
//!
//! Dialogs go by their ids in the game's dialog file, as the original knows
//! them: the week is dialog 2, though the replica puts it together under ids
//! of its own.

use std::collections::{HashMap, VecDeque};
use std::fmt;

use a3_files::rch::{Dialog, Kind};

use crate::controls::button::{cancel_button, ok_button};
use crate::game::Game;
use crate::host::{Change, Host};
use crate::pointer::{Grip, Target};
use crate::screens::calendar::{WEEK, is_week};
use crate::screens::control;

/// Time passes in steps this long while a script waits, for at most this
/// long before it gives up.
const STEP_SECS: f32 = 0.05;
const MOST_SECS: f32 = 60.0;
const DEFAULT_SEED: u64 = 1;

/// A parsed script.
pub struct Script {
    pub(crate) seed: u64,
    pub(crate) lines: Vec<(usize, Line)>,
}

pub(crate) enum Line {
    On(u16, Vec<Action>),
    Once(u16, Vec<Action>),
    /// Actions on the dialog on top, at once, whether or not it moves on.
    Now(Vec<Action>),
    Until(Until),
    Repeat(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Until {
    Dialog(u16),
    Day(usize),
    End,
}

#[derive(Clone, Debug)]
pub(crate) enum Action {
    Ok,
    Cancel,
    Press(u16),
    Click(u16),
    Bar(usize),
    Choose(u16, usize),
    Row(u16, usize),
    /// A row's column, in a list that answers by column.
    Cell(u16, usize, usize),
    Type(u16, String),
    Spin(u16, bool),
    Slide(u16, i32),
    /// Drags a control (a player's box) or a list's row onto another, or off
    /// them all.
    Drag(Spot, Option<Spot>),
    /// A right click on a control or a list's row.
    RightClick(Spot),
    Wait,
}

/// A control, or a row of a list, by its id.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Spot {
    Control(u16),
    Row(u16, usize),
}

/// A spot at the start of `words`: `ID` or `ID row N`; and what follows it.
fn spot<'a>(words: &'a [&'a str]) -> Option<(Spot, &'a [&'a str])> {
    let id = words.first()?.parse().ok()?;
    match words.get(1..3) {
        Some(["row", row]) => Some((Spot::Row(id, row.parse().ok()?), &words[3..])),
        _ => Some((Spot::Control(id), &words[1..])),
    }
}

/// Why a script stopped.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// A line it cannot read: its number and what is wrong.
    Line(usize, String),
    /// A dialog it has no actions for.
    NoActions(u16),
    /// A control the dialog does not have.
    NoControl { dialog: u16, control: u16 },
    /// A dialog that did not move on while it waited.
    Stuck(u16),
    /// Actions for a dialog that was gone before they were done.
    LeftOver(u16),
    /// The game ended before the condition was met.
    Ended,
    /// Days to repeat before there is a season.
    NoSeason,
    /// What the probe in the original said went wrong, or that it is gone.
    Probe(String),
    /// An action the original cannot be driven to do yet.
    Unsupported(u16, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Line(line, what) => write!(f, "line {line}: {what}"),
            Self::NoActions(dialog) => {
                write!(f, "dialog {dialog} waits, and the script has no actions for it")
            }
            Self::NoControl { dialog, control } => {
                write!(f, "dialog {dialog} has no control {control}")
            }
            Self::Stuck(dialog) => write!(f, "dialog {dialog} did not move on"),
            Self::LeftOver(dialog) => {
                write!(f, "dialog {dialog} was gone before its actions were done")
            }
            Self::Ended => write!(f, "the game ended first"),
            Self::NoSeason => write!(f, "there is no season to count days in"),
            Self::Probe(what) => write!(f, "the probe: {what}"),
            Self::Unsupported(dialog, action) => {
                write!(f, "dialog {dialog}: the original cannot be driven to {action} yet")
            }
        }
    }
}

impl std::error::Error for Error {}

impl Script {
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut seed = DEFAULT_SEED;
        let mut lines = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let number = index + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let bad = |what: &str| Error::Line(number, format!("{what}: {line}"));
            let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let rest = rest.trim();
            match word {
                "seed" => seed = rest.parse().map_err(|_| bad("not a seed"))?,
                "on" | "once" => {
                    let (dialog, actions) =
                        rest.split_once(char::is_whitespace).ok_or_else(|| bad("no actions"))?;
                    let dialog = dialog.parse().map_err(|_| bad("not a dialog"))?;
                    let actions = actions
                        .split(';')
                        .map(|a| parse_action(a.trim()).ok_or_else(|| bad("not an action")))
                        .collect::<Result<_, _>>()?;
                    lines.push((
                        number,
                        if word == "on" {
                            Line::On(dialog, actions)
                        } else {
                            Line::Once(dialog, actions)
                        },
                    ));
                }
                "now" => {
                    let actions = rest
                        .split(';')
                        .map(|a| parse_action(a.trim()).ok_or_else(|| bad("not an action")))
                        .collect::<Result<_, _>>()?;
                    lines.push((number, Line::Now(actions)));
                }
                "until" => {
                    let words: Vec<&str> = rest.split_whitespace().collect();
                    let until = match words[..] {
                        ["dialog", id] => {
                            Until::Dialog(id.parse().map_err(|_| bad("not a dialog"))?)
                        }
                        ["day", day] => Until::Day(day.parse().map_err(|_| bad("not a day"))?),
                        ["end"] => Until::End,
                        _ => return Err(bad("until what")),
                    };
                    lines.push((number, Line::Until(until)));
                }
                "repeat" => {
                    let words: Vec<&str> = rest.split_whitespace().collect();
                    let ["days" | "day", ..] = words.get(1..).unwrap_or_default() else {
                        return Err(bad("repeat how many days"));
                    };
                    let days = words[0].parse().map_err(|_| bad("not a number of days"))?;
                    lines.push((number, Line::Repeat(days)));
                }
                _ => return Err(bad("no such line")),
            }
        }
        Ok(Self { seed, lines })
    }
}

fn parse_action(action: &str) -> Option<Action> {
    let words: Vec<&str> = action.split_whitespace().collect();
    let id = |word: &str| word.parse::<u16>().ok();
    Some(match words[..] {
        ["ok"] => Action::Ok,
        ["cancel"] => Action::Cancel,
        ["wait"] => Action::Wait,
        ["press", button] => Action::Press(id(button)?),
        ["click", "bar", symbol] => Action::Bar(symbol.parse().ok()?),
        ["click", control] => Action::Click(id(control)?),
        ["choose", options, item] => Action::Choose(id(options)?, item.parse().ok()?),
        ["row", list, row] => Action::Row(id(list)?, row.parse().ok()?),
        ["cell", list, row, column] => {
            Action::Cell(id(list)?, row.parse().ok()?, column.parse().ok()?)
        }
        ["spin", spinner, "up"] => Action::Spin(id(spinner)?, true),
        ["spin", spinner, "down"] => Action::Spin(id(spinner)?, false),
        ["slide", slider, value] => Action::Slide(id(slider)?, value.parse().ok()?),
        ["rclick", ref rest @ ..] => match spot(rest)? {
            (spot, []) => Action::RightClick(spot),
            _ => return None,
        },
        ["drag", ref rest @ ..] => {
            let (from, rest) = spot(rest)?;
            match rest {
                ["off"] => Action::Drag(from, None),
                _ => match spot(rest)? {
                    (to, []) => Action::Drag(from, Some(to)),
                    _ => return None,
                },
            }
        }
        ["type", field, ..] => {
            let text = action.splitn(3, char::is_whitespace).nth(2).unwrap_or_default();
            Action::Type(id(field)?, text.to_owned())
        }
        _ => return None,
    })
}

/// Plays scripts on a game of its own.
pub struct Runner<'a> {
    game: &'a Game,
    pub host: Host,
    on: HashMap<u16, Vec<Action>>,
    once: HashMap<u16, VecDeque<Vec<Action>>>,
    /// Every screen that came up and every action, in order.
    pub log: Vec<String>,
}

impl<'a> Runner<'a> {
    /// Starts a game whose new games take their random numbers from `seed`.
    pub fn new(game: &'a Game, seed: u64) -> Self {
        let host = Host::start(game, Box::new(move || seed));
        Self { game, host, on: HashMap::new(), once: HashMap::new(), log: Vec::new() }
    }

    /// Plays a script on a game of its own.
    pub fn play(game: &'a Game, script: &Script) -> (Self, Result<(), Error>) {
        let mut runner = Self::new(game, script.seed);
        let result = runner.run(script);
        (runner, result)
    }

    pub fn run(&mut self, script: &Script) -> Result<(), Error> {
        for (_, line) in &script.lines {
            match line {
                Line::On(dialog, actions) => _ = self.on.insert(*dialog, actions.clone()),
                Line::Once(dialog, actions) => {
                    self.once.entry(*dialog).or_default().push_back(actions.clone())
                }
                Line::Until(until) => self.until(*until)?,
                Line::Now(actions) => {
                    self.settle()?;
                    let shown = self.host.view().id;
                    for action in actions {
                        self.act(shown, action)?;
                    }
                }
                Line::Repeat(days) => {
                    self.settle()?;
                    let today = self.host.state().season.as_ref().map(|s| s.day);
                    let today = today.ok_or(Error::NoSeason)?;
                    self.until(Until::Day(today + days))?;
                }
            }
        }
        Ok(())
    }

    /// The dialog on top, by its id in the game's dialog file.
    pub fn dialog(&self) -> Option<u16> {
        self.host.is_open().then(|| named(self.host.view().id))
    }

    /// Plays until the condition is met.
    fn until(&mut self, until: Until) -> Result<(), Error> {
        loop {
            match self.settle() {
                Err(Error::Ended) if until == Until::End => return Ok(()),
                result => result?,
            }
            let shown = self.host.view().id;
            let id = named(shown);
            let met = match until {
                Until::Dialog(dialog) => id == dialog,
                Until::Day(day) => self.host.state().season.as_ref().is_some_and(|s| s.day >= day),
                Until::End => false,
            };
            if met {
                return Ok(());
            }
            let actions = match self.once.get_mut(&id).and_then(VecDeque::pop_front) {
                Some(actions) => actions,
                None => self.on.get(&id).cloned().ok_or(Error::NoActions(id))?,
            };
            for (n, action) in actions.iter().enumerate() {
                self.act(shown, action)?;
                let gone = !self.host.waiting() || self.host.view().id != shown;
                if gone && n + 1 < actions.len() {
                    return Err(Error::LeftOver(id));
                }
                if gone {
                    break;
                }
            }
        }
    }

    /// Waits for a screen that waits for the player.
    fn settle(&mut self) -> Result<(), Error> {
        while !self.host.waiting() {
            match self.host.next(self.game) {
                Change::Quit => return Err(Error::Ended),
                Change::Screen => self.log.push(format!("screen {}", named(self.host.view().id))),
                Change::Same => {}
            }
        }
        Ok(())
    }

    /// Acts on the controls of the dialog `shown`.
    fn act(&mut self, shown: u16, action: &Action) -> Result<(), Error> {
        self.log.push(format!("  {action:?}"));
        let game = self.game;
        let controls = &game.dialogs[&shown];
        let target = match *action {
            Action::Type(_, ref text) => {
                let Target::Edit(field) = target_of(controls, named(shown), action)? else {
                    unreachable!("a field to type into");
                };
                let change = self.host.input(game, Target::Edit(field), false);
                self.change(change);
                self.host.view_mut().set_text(field, "");
                self.host.view_mut().type_text(controls, text);
                return Ok(());
            }
            Action::Wait => return self.wait(shown),
            _ => target_of(controls, named(shown), action)?,
        };
        let change = self.host.input(game, target, false);
        self.change(change);
        Ok(())
    }

    /// Lets time pass until the screen `shown` moves on by itself.
    fn wait(&mut self, shown: u16) -> Result<(), Error> {
        let mut waited = 0.0;
        while self.host.waiting() && self.host.view().id == shown {
            if waited > MOST_SECS {
                return Err(Error::Stuck(named(shown)));
            }
            let change = self.host.tick(self.game, STEP_SECS);
            self.change(change);
            waited += STEP_SECS;
        }
        Ok(())
    }

    fn change(&mut self, change: Change) {
        if change == Change::Screen {
            self.log.push(format!("screen {}", named(self.host.view().id)));
        }
    }
}

/// What an action acts on in `controls`, the dialog `dialog`; `wait` acts on
/// nothing.
pub(crate) fn target_of(controls: &Dialog, dialog: u16, action: &Action) -> Result<Target, Error> {
    let find = |id: u16| control(controls, id).ok_or(Error::NoControl { dialog, control: id });
    let button = |found: Option<usize>| found.ok_or(Error::NoControl { dialog, control: 0 });
    Ok(match *action {
        Action::Ok => Target::Button(button(ok_button(controls))?),
        Action::Cancel => Target::Button(button(cancel_button(controls))?),
        Action::Press(id) => Target::Button(find(id)?),
        Action::Click(id) => {
            let control = find(id)?;
            match controls.controls[control].kind {
                Kind::Check => Target::Check(control),
                Kind::Combo { .. } => Target::Combo(control),
                _ => Target::Control(control),
            }
        }
        Action::Bar(item) => {
            let bar = controls.controls.iter().position(|c| matches!(c.kind, Kind::Bar { .. }));
            Target::Option { control: bar.ok_or(Error::NoControl { dialog, control: 0 })?, item }
        }
        Action::Choose(id, item) => Target::Option { control: find(id)?, item },
        Action::Row(id, row) => Target::Row { control: find(id)?, row },
        Action::Cell(id, row, column) => Target::Cell { control: find(id)?, row, column },
        Action::Spin(id, up) => {
            let control = find(id)?;
            match controls.controls[control].kind {
                Kind::Slider { .. } => Target::SliderStep { control, up },
                _ => Target::Spin { control, up },
            }
        }
        Action::Slide(id, value) => Target::SliderAt { control: find(id)?, value },
        Action::Drag(from, to) => {
            let grip = |spot: Spot| grip(controls, dialog, spot);
            Target::Drop { from: grip(from)?, to: to.map(grip).transpose()? }
        }
        Action::RightClick(spot) => Target::Menu(grip(controls, dialog, spot)?),
        Action::Type(id, _) => Target::Edit(find(id)?),
        Action::Wait => return Err(Error::Unsupported(dialog, "wait on a control".into())),
    })
}

/// What a spot of a script takes hold of in `controls`.
fn grip(controls: &Dialog, dialog: u16, spot: Spot) -> Result<Grip, Error> {
    let find = |id: u16| control(controls, id).ok_or(Error::NoControl { dialog, control: id });
    Ok(match spot {
        Spot::Control(id) => Grip::Control(find(id)?),
        Spot::Row(id, row) => Grip::Row { control: find(id)?, row },
    })
}

/// A dialog the replica shows by its id in the game's dialog file.
fn named(shown: u16) -> u16 {
    if is_week(shown) { WEEK } else { shown }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::screens::compose;

    fn game() -> Game {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut game = Game::load(&dir).unwrap();
        compose(&mut game);
        game
    }

    fn seen(runner: &Runner, dialog: u16) -> usize {
        let screen = format!("screen {dialog}");
        runner.log.iter().filter(|line| **line == screen).count()
    }

    /// A new game for two, played to the first matchday.
    const NEW_GAME: &str = include_str!("../scripts/new-game.txt");

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_new_game_plays_to_the_first_matchday() {
        let game = game();
        let script = Script::parse(NEW_GAME).unwrap();
        let (runner, result) = Runner::play(&game, &script);
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        // The Bundesliga starts on day 48, in the seventh week: the season
        // stops before its first match.
        assert_eq!(season.day, 42);
        assert!(season.managers[0].club.is_some() && season.managers[1].club.is_none());
        assert_eq!(seen(&runner, WEEK), 6, "{}", runner.log.join("\n"));
        // Every turn ends with the whistle, one for each manager before each
        // week; the script stops at the first turn of day 42. (The original
        // gives the manager without a club no turns.)
        let whistles = runner.log.iter().filter(|line| line.trim() == "Bar(18)").count();
        assert_eq!(whistles, 2 * 6);
    }

    /// Loads the newest save and stops on the line-up.
    const LOAD: &str = include_str!("../scripts/load.txt");

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn every_save_opens_on_its_line_up() {
        let game = game();
        let saves = game.save_games();
        assert!(!saves.is_empty(), "no saves");
        for (row, save) in saves.iter().enumerate() {
            let script =
                Script::parse(&LOAD.replace("row 5144 0", &format!("row 5144 {row}"))).unwrap();
            let (runner, result) = Runner::play(&game, &script);
            result.unwrap_or_else(|err| panic!("{}: {err}\n{}", save.name, runner.log.join("\n")));
            let view = runner.host.view();
            assert_eq!(view.id, 147, "{}", save.name);
            let state = runner.host.state();
            let (season, records) =
                (state.season.as_ref().unwrap(), state.records.as_ref().unwrap());
            assert_eq!(season.day, usize::from(records.globals.day()));
            let [day, month, year] = records.globals.date();
            let dated = (0..view.controls.len())
                .any(|c| view.text(c).ends_with(&format!(", {day}.{month}.{year}")));
            assert!(dated, "{}: no caption with the date {day}.{month}.{year}", save.name);

            // The squad as the save has it: the eleven, the bench, the rest
            // backwards from the squad by position, as the original's list
            // opens — unless the save goes on with a matchday's matches,
            // before which the co-trainer may line the team up anew (see
            // `the_co_trainer_lines_up_a_resumed_matchday`).
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            let squad = team.players();
            let position = |id: u16| records.player(id).unwrap().positions()[0];
            let by_position: Vec<u16> = (1..=10)
                .flat_map(|p| squad.iter().copied().filter(move |&id| position(id) == p))
                .collect();
            let mut order: Vec<u16> =
                team.lineup().into_iter().chain(team.bench()).filter(|&id| id != 0).collect();
            if records.globals.resume() == MATCHES_RESUMED {
                let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
                order = club.lineup.squad().map(|p| club.status[p].id).collect();
            }
            let rest: Vec<u16> =
                by_position.iter().rev().copied().filter(|id| !order.contains(id)).collect();
            order.extend(rest);
            let list = view.controls.iter().find_map(|c| match &c.state {
                crate::controls::ControlState::List(list) if list.rows.len() == squad.len() => {
                    Some(list)
                }
                _ => None,
            });
            let list =
                list.unwrap_or_else(|| panic!("{}: no list of {} players", save.name, squad.len()));
            for (row, id) in list.rows.iter().zip(order) {
                let player = records.player(id).unwrap();
                let columns: Vec<&str> = row.split('\t').collect();
                let initial = records
                    .name(player.first_name())
                    .chars()
                    .next()
                    .map_or(String::new(), |c| format!(", {c}."));
                assert_eq!(
                    columns[0],
                    format!("{}{initial}", records.name(player.last_name())),
                    "{}",
                    save.name
                );
                assert_eq!(columns[3], player.strength().to_string());
                assert_eq!(columns[6], player.morale().to_string());
                assert_eq!(columns[8], player.shirt().to_string());
                let age = season.date().years_since(player.birthday().unwrap()).unwrap();
                assert_eq!(columns[9], age.to_string());
            }
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn dragged_players_move_as_the_game_moves_them() {
        let game = game();
        let (mut runner, result) = Runner::play(&game, &Script::parse(LOAD).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let dialog = &game.dialogs[&147];
        let at = |id: u16| crate::screens::control(dialog, id).unwrap();
        let lineup = |runner: &Runner| {
            let state = runner.host.state();
            let season = state.season.as_ref().unwrap();
            season.clubs[&season.manager().unwrap().club.unwrap()].lineup.clone()
        };
        let drop = |runner: &mut Runner, from: Grip, to: Option<Grip>| {
            runner.host.input(&game, Target::Drop { from, to }, false);
        };
        let before = lineup(&runner);
        // Two boxes on the pitch swap their players …
        drop(&mut runner, Grip::Control(at(2510)), Some(Grip::Control(at(2511))));
        drop(&mut runner, Grip::Control(at(2512)), Some(Grip::Control(at(2521))));
        // … one let go of off the boxes leaves the line-up …
        drop(&mut runner, Grip::Control(at(2520)), None);
        // … and one of the list's rest takes a box: the 19th row, after the
        // eleven and the bench (scrolled out of sight here).
        drop(&mut runner, Grip::Row { control: at(2509), row: 17 }, Some(Grip::Control(at(2527))));
        let after = lineup(&runner);
        assert_eq!((after.field[0], after.field[1]), (before.field[1], before.field[0]));
        assert_eq!((after.field[2], after.bench[0]), (before.bench[0], before.field[2]));
        assert_eq!(after.field[10], None);
        let rest: Vec<usize> =
            (0..).take_while(|&p| p < 40).filter(|p| !before.squad().any(|q| q == *p)).collect();
        assert!(after.bench[6].is_some() && after.bench[6] != before.bench[6]);
        assert!(rest.contains(&after.bench[6].unwrap()), "the box took one of the rest");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_formation_chosen_stands_the_line_up_in_it() {
        let game = game();
        let script = format!(
            "{}\nnow choose 5266 1\nuntil dialog 244\nnow choose 3386 3; press 1\nuntil dialog 147",
            LOAD.trim_end()
        );
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let manager = season.manager().unwrap();
        let club = &season.clubs[&manager.club.unwrap()];
        assert_eq!(manager.formations[3].0, "4-4-2");
        assert_eq!(club.formation, 3);
        assert_eq!(club.lineup.spots, manager.formations[3].1);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_special_variant_puts_up_players_on_their_own_positions() {
        let game = game();
        let script = format!(
            "{}\nnow choose 2572 0\nuntil dialog 386\nnow choose 5330 7\nuntil dialog 147",
            LOAD.trim_end()
        );
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        let pitch = &game.rules.pitch;
        let lineup = &club.lineup;
        for (spot, player) in lineup.on_field() {
            assert_eq!(club.players[player].positions[0], pitch.kind_under(spot));
        }
        let squad: Vec<usize> = lineup.squad().collect();
        let mut unique = squad.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), squad.len(), "nobody twice");
        let first = lineup.bench[0].unwrap();
        assert_eq!(club.players[first].positions[0], a3_core::world::GOALKEEPER);
    }

    /// The cohesion the original showed for the gate's save (P0's check.sh)
    /// after each of these, on the line-up (`tools/oracle`, 13.10.2026).
    const COHESION_OF_THE_ORIGINAL: [(&str, i32); 5] = [
        ("", 14),
        ("now drag 2510 2511; drag 2512 2521; drag 2520 off; drag 2509 row 9 2527", 2),
        ("now choose 5266 1\nuntil dialog 244\nnow choose 3386 3; press 1\nuntil dialog 147", 6),
        ("now choose 2572 0\nuntil dialog 386\nnow choose 5330 1\nuntil dialog 147", 21),
        ("now choose 2572 0\nuntil dialog 386\nnow choose 5330 7\nuntil dialog 147", 21),
    ];

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_line_ups_cohesion_is_the_originals() {
        let game = game();
        let Some(row) = game.save_games().iter().position(|s| s.name == "Probe1") else {
            eprintln!("no Probe1 among the saves; the oracle's copy has it");
            return;
        };
        for (actions, expected) in COHESION_OF_THE_ORIGINAL {
            let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}"));
            let script = format!("{}\n{actions}", load.trim_end());
            let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
            result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
            let state = runner.host.state();
            let season = state.season.as_ref().unwrap();
            let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
            let cohesion = a3_core::cohesion::of(&game.rules.pitch, club, season.date());
            assert_eq!(cohesion, expected, "after {actions:?}");
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_medical_department_lists_the_injured_and_the_knocked() {
        let game = game();
        let script = format!("{}\nnow choose 5266 6\nuntil dialog 151", LOAD.trim_end());
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        let records = state.records.as_ref().unwrap();
        let knocks = |p: usize| records.player(club.status[p].id).map_or(0, |r| r.knocks());
        let sick = a3_core::medical::sick_list(club, knocks);
        let view = runner.host.view();
        let list = view.list(control(&game.dialogs[&151], 2586).unwrap()).unwrap();
        if sick.is_empty() {
            assert_eq!(list.rows, ["Alle Spieler sind derzeit einsetzbar."]);
        } else {
            assert_eq!(list.keys, sick);
            assert_eq!(list.rows.len(), sick.len());
        }
        // OK goes back to the line-up.
        let script = format!("{script}\nnow ok\nuntil dialog 147");
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_formation_keeps_where_the_boxes_stand() {
        let game = game();
        let script = format!(
            "{}\nnow choose 5266 2\nuntil dialog 384\nnow ok\nuntil dialog 147",
            LOAD.trim_end()
        );
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let manager = season.manager().unwrap();
        let club = &season.clubs[&manager.club.unwrap()];
        assert_eq!(manager.formations[club.formation].1, club.lineup.spots);
        let records = state.records.as_ref().unwrap();
        let record = &records.managers[records.human_slot(season.current).unwrap()];
        assert_eq!(record.formations()[club.formation].1, club.lineup.spots);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_penalty_taker_picked_takes_the_penalties() {
        use a3_core::duties::Duty;
        let game = game();
        let open =
            format!("{}\nnow choose 5266 3\nuntil dialog 418\nnow choose 5340 2", LOAD.trim_end());
        let (runner, result) = Runner::play(&game, &Script::parse(&open).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let view = runner.host.view();
        let list = control(&game.dialogs[&418], 5339).unwrap();
        let rows = &view.list(list).unwrap().rows;
        let name = rows[3].split('\t').nth(1).unwrap().to_owned();
        assert_eq!(rows[3].split('\t').next(), Some("4"));

        let script = format!("{open}\nnow row 5339 3; ok\nuntil dialog 147");
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        // He goes first, the three above him one down; the record has it.
        let taker = club.duties[Duty::Penalty.stored()][0];
        let p = &club.players[taker];
        let initial = p.first_name.chars().next().map_or(String::new(), |c| format!(", {c}."));
        assert_eq!(format!("{}{initial}", p.last_name), name);
        assert_eq!(club.duties[Duty::Penalty.stored()].len(), club.players.len().min(35));
        let records = state.records.as_ref().unwrap();
        let human = records.humans().next().unwrap();
        let team = records.team(human.country(), human.team()).unwrap();
        let extra = &records.extras[usize::from(team.human()) - 1];
        assert_eq!(extra.duty_lists()[Duty::Penalty.stored()][0], club.status[taker].id);
        // On the pitch his box shows the penalty, if he plays.
        let view = runner.host.view();
        if let Some(slot) = club.lineup.field.iter().position(|&f| f == Some(taker)) {
            let c = control(&game.dialogs[&147], 2510 + slot as u16).unwrap();
            let shown = view.controls[c].state.as_player().unwrap().duty;
            assert_eq!(shown, game.rules.pitch.duty_icon(Duty::Penalty));
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_player_info_shows_what_the_originals_does() {
        let game = game();
        let Some(row) = game.save_games().iter().position(|s| s.name == "Probe1") else {
            eprintln!("no Probe1 among the saves; the oracle's copy has it");
            return;
        };
        let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}"));
        let script =
            format!("{}\nnow rclick 2510; choose 9950 0\nuntil dialog 399", load.trim_end());
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let view = runner.host.view();
        let dialog = &game.dialogs[&399];
        let text = |id: u16| view.text(crate::screens::control(dialog, id).unwrap()).to_owned();
        // As the original showed the gate's keeper (P0's check.sh).
        assert_eq!(text(4825), "Max Schmitt");
        assert_eq!(text(4826), "19 Jahre (18.1.2006)");
        assert_eq!(text(4834), "144.000 €");
        assert_eq!(text(4843), "1.36 Mio €");
        assert_eq!(text(4844), "Vertrag bis 30.6.2028 (noch drei Jahre).");
        assert_eq!(text(4878), "Er ist gesund.");
        assert_eq!([text(4971), text(4972)], ["Geldgeier", "Skandalnudel"]);
    }

    /// Plays the save `save` to its line-up, then `then`; the runner, or
    /// none if the installation has no such save.
    fn on_save<'a>(game: &'a Game, save: &str, then: &str) -> Option<Runner<'a>> {
        let Some(row) = game.save_games().iter().position(|s| s.name == save) else {
            eprintln!("no {save} among the saves; the oracle's copy has it");
            return None;
        };
        let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}"));
        let script = format!("{}\n{then}", load.trim_end());
        let (runner, result) = Runner::play(game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        Some(runner)
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_bonus_for_the_team_costs_and_takes_the_edge_off() {
        let game = game();
        // The keeper's menu: "Spielerinfo", a blank line, "Prämie" (no talk
        // on the week's last day), the bonus for the team at its start,
        // 100.000 €.
        let then = "now rclick 2510; choose 9950 2\nuntil dialog 195\nnow choose 2947 0; ok\n\
                    until dialog 380\nnow press 4731\nuntil dialog 147";
        let Some(before) = on_save(&game, "Toni", "") else { return };
        let runner = on_save(&game, "Toni", then).unwrap();
        let team = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            (team.bonuses_paid(), team.will_to_win())
        };
        let ((paid, will), (paid_now, will_now)) = (team(&before), team(&runner));
        assert_eq!(paid_now, paid + 100_000);
        // Two tries in the first league, two off the will to win each.
        assert_eq!(will_now, will - 4);
        assert!(runner.log.iter().any(|l| l == "screen 380"));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_fine_for_nothing_is_refused() {
        let game = game();
        let then =
            "now rclick 2510; choose 9950 3\nuntil dialog 85\nnow press 1867\nuntil dialog 88";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&88];
        let text = view.text(crate::screens::control(dialog, 1873).unwrap());
        assert_eq!(
            text,
            "Frederik Rönnow hat absolut nichts gemacht, daß eine Geldstrafe rechtfertigen würde!"
        );
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_opponent_info_tells_what_the_originals_does() {
        let game = game();
        let then = "now choose 5266 7\nuntil dialog 83";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        // As the original shows Union's next opponent on day 62.
        let view = runner.host.view();
        let dialog = &game.dialogs[&83];
        let text = |id: u16| view.text(crate::screens::control(dialog, id).unwrap());
        assert_eq!(text(1828), "VfL Wolfsburg");
        assert_eq!(text(1829), "9. Platz (3 Punkte,2 Spiele) (zuletzt: sN)");
        assert_eq!(text(1830), "Runde 2 (RB Leipzig)");
        assert_eq!(text(1831), "nicht dabei");
        assert_eq!(text(1848), "gesegnet");
        assert_eq!(text(1825), "Volkswagen Arena, 30000 Zuschauer");
        // Clubs further in Europe, as EUROPAPOKAL-STAND.md reads the save:
        // a seed, a qualifier winner, a UEFA qualifier, a direct entrant
        // before the first round's draw.
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let stand = |name: &str| {
            let team = records.teams.iter().find(|t| t.name() == name).unwrap();
            a3_core::opponent::europe_stand(records, team)
        };
        use a3_core::opponent::EuropeStand::{In, NotIn};
        assert_eq!(stand("FC Bayern München"), In(0, 1871, None));
        let silkeborg = records.teams.iter().find(|t| t.name() == "Silkeborg IF").unwrap();
        let silkeborg = (silkeborg.country(), i16::from(silkeborg.number()));
        assert_eq!(stand("Eintracht Frankfurt"), In(0, 1871, Some(silkeborg)));
        assert!(matches!(stand("Paksi FC"), In(1, 1864, Some(_))));
        assert_eq!(stand("Borussia Dortmund"), NotIn);
        assert_eq!(stand("Silkeborg IF"), NotIn);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_list_view_takes_players_in_and_out_as_the_originals_does() {
        let game = game();
        let dialog = &game.dialogs[&429];
        let text = |runner: &Runner, id: u16| {
            runner.host.view().text(crate::screens::control(dialog, id).unwrap()).to_owned()
        };
        // As the original shows Union's line-up as a list on day 62: the
        // parts the pitch view reckoned, all eleven in.
        let then = "now choose 5266 0\nuntil dialog 429";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        assert_eq!(text(&runner, 5277), "Derzeit sind 11 Spieler aufgestellt!");
        let parts = [2549, 2550, 2551, 2552, 2555].map(|id| text(&runner, id));
        assert_eq!(parts, ["9.5", "31.1", "34.4", "22.9", "97.9"]);
        assert_eq!(text(&runner, 2556), "3.8");
        // The keeper out (the bench is full), then the spare keeper in: he
        // takes the empty slot, the parts stay as they were.
        let then = format!("{then}\nnow row 2509 0\nuntil dialog 429");
        let runner = on_save(&game, "Toni", &then).unwrap();
        assert_eq!(text(&runner, 5277), "Derzeit sind 10 Spieler aufgestellt!");
        assert_eq!(text(&runner, 2556), "3.6");
        let then = format!("{then}\nnow row 2509 24\nuntil dialog 429");
        let runner = on_save(&game, "Toni", &then).unwrap();
        assert_eq!(text(&runner, 5277), "Derzeit sind 11 Spieler aufgestellt!");
        assert_eq!(text(&runner, 2556), "3.7");
        assert_eq!(text(&runner, 2549), "9.5");
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        let keeper = club.lineup.field[0].map(|p| club.players[p].last_name.clone());
        assert_eq!(keeper.as_deref(), Some("Klaus"));
        // The view is kept with the manager, in his record too: the switch
        // goes back to the pitch.
        assert!(season.manager().unwrap().list_view);
        assert!(state.records.as_ref().unwrap().managers[0].list_view());
        drop(state);
        let then = format!("{then}\nnow choose 5266 0\nuntil dialog 147");
        let runner = on_save(&game, "Toni", &then).unwrap();
        let state = runner.host.state();
        assert!(!state.season.as_ref().unwrap().manager().unwrap().list_view);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_tactics_show_and_keep_what_the_originals_do() {
        let game = game();
        let dialog = &game.dialogs[&271];
        let text = |runner: &Runner, id: u16| {
            runner.host.view().text(crate::screens::control(dialog, id).unwrap()).to_owned()
        };
        // As the original shows Union's tactics before Wolfsburg away.
        let then = "now choose 9930 2\nuntil dialog 271";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        assert_eq!(text(&runner, 3735), "1.FC Union Berlin (A, 6.) - VfL Wolfsburg (H, 9.)");
        assert_eq!(text(&runner, 3736), "Schiedsrichter: Schmidt (Komp. 8, H. 5)");
        let labels = [6278, 6277, 6276, 3754].map(|id| text(&runner, id));
        assert_eq!(
            labels,
            ["Spielweise: Vorsichtig", "Härte: Normal", "Einsatz: Normal", "Zeitspiel: nie"]
        );
        assert_eq!([text(&runner, 3741), text(&runner, 5518)], ["Stärke: 3", "0 €"]);
        // Every kind of control once, as the original took them; then away
        // to the line-up and back.
        let then = format!(
            "{then}\nnow press 3737; rclick 3738; press 3739; press 3756; press 3739; press 3757\n\
             now choose 3762 5; choose 3742 0; choose 5517 0; choose 6293 0; choose 5517 0\n\
             now spin 5519 up; spin 5519 up; spin 5519 up; spin 5520 up; spin 5522 down; \
             spin 5521 down\nnow choose 9930 0\nuntil dialog 147\nnow choose 9930 2\n\
             until dialog 271"
        );
        let runner = on_save(&game, "Toni", &then).unwrap();
        let labels = [6278, 6277, 6276, 3754].map(|id| text(&runner, id));
        assert_eq!(
            labels,
            ["Spielweise: Normal", "Härte: Rauh", "Einsatz: 150%", "Zeitspiel: immer"]
        );
        assert_eq!(text(&runner, 5518), "30 €");
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        // Union's team record.
        let team = &records.teams[84];
        assert_eq!([team.style(), team.hardness(), team.effort()], [3, 1, 0]);
        // "Zeitspiel" immer, tricks, the offside trap, "Langsamer
        // Spielaufbau", the middle; passing as the original's dump has it.
        assert_eq!(team.tactic_flags(), 0x1 | 0x4 | 0x8 | 5 << 7 | 0x800);
        assert_eq!(team.passing(), 0x6054);
        assert_eq!(records.extras[0].special_bonus(), 30);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_talk_is_booked_in_the_planner() {
        let game = game();
        // The keeper's menu on the first day of a week: the talk is its
        // second entry; the first free day is today.
        let then =
            "now rclick 2510; choose 9950 1\nuntil dialog 412\nnow choose 5034 0\nuntil dialog 147";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let day = usize::from(records.globals.day());
        let booked = records.planners[0].day(day);
        assert_eq!((booked.kind, booked.flag, booked.player), (0x19, 1, 3204));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn spiel_beenden_ends_the_game() {
        let game = game();
        // The dressing room's game options: "Spiel beenden", and yes.
        let then = "now click bar 17\nuntil dialog 302\nnow choose 4035 4\nuntil dialog 384\n\
                    now press 4741\nuntil end";
        // `until end` holds only once the game cycle has ended: after the
        // question, the dressing room it closes with.
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let screens: Vec<&str> =
            runner.log.iter().filter_map(|l| l.strip_prefix("screen ")).collect();
        assert_eq!(screens[screens.len() - 3..], ["302", "384", "302"]);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_dragged_player_goes_with_the_pointer() {
        let game = game();
        let Some(runner) = on_save(&game, "Probe1", "") else { return };
        let dialog = &game.dialogs[&147];
        let mut view = runner.host.view().clone();
        let list = crate::screens::control(dialog, 2509).unwrap();
        let at = bevy_math::Vec2::new(300.0, 120.0);
        let draw = |view: &crate::view::View| {
            let background = crate::classic::picture(&game, dialog);
            crate::classic::Classic::new(&game, dialog, view, background)
                .draw(dialog, view, false, 0.0)
        };
        let before = draw(&view);
        view.dragging = Some(crate::view::Dragging {
            grip: crate::pointer::Grip::Row { control: list, row: 3 },
            at,
        });
        let after = draw(&view);
        // The box's picture lies under the pointer, and nowhere else changed.
        let changed = |x: u32, y: u32| before.get_pixel(x, y) != after.get_pixel(x, y);
        assert!(changed(at.x as u32, at.y as u32));
        assert!(!changed(at.x as u32 + 80, at.y as u32));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_booked_talk_comes_on_its_day() {
        let game = game();
        // The talk booked for today, the turn ended with the whistle and
        // yes to no training: the week's first day brings the talk; the
        // keeper is praised.
        // The other manager's turn, in the dressing room, ends the same.
        let then = "now rclick 2510; choose 9950 1\nuntil dialog 412\nnow choose 5034 0\n\
                    until dialog 147\non 302 click bar 18\non 384 press 4741\non 2 wait\n\
                    now click bar 18\nuntil dialog 461\nnow choose 5839 0";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&461];
        let text = |id| view.text(crate::screens::control(dialog, id).unwrap()).to_owned();
        assert_eq!(text(5833), "Max Schmitt");
        assert!(!text(1416).is_empty(), "his reaction");
        // One topic a talk.
        let topics = view.options(crate::screens::control(dialog, 5839).unwrap()).unwrap();
        assert!(topics.off.iter().all(|&off| off));
    }

    /// Where the cycle goes on after loading a game saved during a
    /// matchday's matches.
    const MATCHES_RESUMED: u16 = 0x3e6;

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_co_trainer_lines_up_a_resumed_matchday() {
        let game = game();
        // Saved during a matchday, with "Automatische Aufstellung freies
        // System" ticked: the original opened it on this eleven and bench
        // (`tools/oracle`, 13.10.2026).
        let Some(runner) = on_save(&game, "Toni", "") else { return };
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let records = state.records.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        let name = |p: Option<usize>| {
            p.map_or_else(String::new, |p| {
                records.name(records.player(club.status[p].id).unwrap().last_name()).to_owned()
            })
        };
        let eleven: Vec<String> = club.lineup.field.iter().map(|&p| name(p)).collect();
        let bench: Vec<String> = club.lineup.bench.iter().map(|&p| name(p)).collect();
        assert_eq!(
            eleven,
            [
                "Rönnow",
                "Leite",
                "Nsoki",
                "Juranovic",
                "Moore",
                "Skov",
                "Moscardo",
                "Lubiniecki",
                "Haberer",
                "Burke",
                "Ljubicic"
            ]
        );
        assert_eq!(bench, ["Raab", "Král", "Schäfer", "Querfeld", "Kemlein", "Khedira", "Köhn"]);
    }

    /// A page, a control of it, and the text it shows.
    type Shown = (u16, u16, &'static str);

    /// What the original's other pages showed of the first player of the
    /// line-up, in the gate's save and in one a few matchdays on
    /// (`tools/oracle`, 13.10.2026).
    const PAGES_OF_THE_ORIGINAL: [(&str, &[Shown]); 2] = [
        (
            "Probe1",
            &[
                (402, 4879, "9.0"),
                (402, 4880, "9"),
                (402, 4882, "-"),
                (402, 4875, "-"),
                (400, 5019, "1"),
                (400, 5029, "Im Verein seit 2025."),
                (401, 5648, "Freundin"),
                (401, 4961, "Karin"),
                (401, 4962, "zusammen"),
                (401, 4977, "Familie"),
                (401, 6003, "Er ist zufrieden. Positiv: toller Vertrag"),
            ],
        ),
        (
            "Toni",
            &[
                (402, 4862, "2"),
                (402, 4879, "15.0"),
                (402, 4880, "16"),
                (402, 4881, "12"),
                (402, 4882, "4.0"),
                (400, 5009, "2"),
                (400, 5017, "1"),
                (400, 5021, "Tore England"),
                (401, 5648, "Ehefrau"),
                (401, 4961, "Tatjana"),
                (401, 4969, "101"),
                (401, 4964, "98"),
                (401, 6003, "Er ist zufrieden."),
            ],
        ),
    ];

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_player_infos_pages_show_what_the_originals_do() {
        let game = game();
        for (save, expected) in PAGES_OF_THE_ORIGINAL {
            let Some(row) = game.save_games().iter().position(|s| s.name == save) else {
                eprintln!("no {save} among the saves; the oracle's copy has it");
                continue;
            };
            let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}"));
            let mut script =
                format!("{}\nnow rclick 2510; choose 9950 0\nuntil dialog 399", load.trim_end());
            for page in [402, 400, 401] {
                script += &format!("\nnow choose 4889 1\nuntil dialog {page}");
                let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
                result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
                let view = runner.host.view();
                let dialog = &game.dialogs[&page];
                for &(_, id, text) in expected.iter().filter(|(p, ..)| *p == page) {
                    // Some of the game's texts end in spaces ("Ehefrau        ").
                    let shown = view.text(crate::screens::control(dialog, id).unwrap());
                    assert_eq!(shown.trim_end(), text, "{save}, page {page}, control {id}");
                }
            }
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_start_menu_leads_where_the_game_does() {
        let game = game();
        let script = Script::parse(
            "
            once 9 choose 4771 5; ok
            on 95 ok
            once 9 choose 4771 1; ok
            on 424 cancel
            once 9 choose 4771 4; ok
            on 474 cancel
            once 9 choose 4771 6; ok
            until end
            ",
        )
        .unwrap();
        let (runner, result) = Runner::play(&game, &script);
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let screens: Vec<&str> =
            runner.log.iter().filter_map(|l| l.strip_prefix("screen ")).collect();
        assert_eq!(screens, ["9", "95", "9", "424", "9", "474", "9"]);
    }

    #[test]
    fn scripts_say_what_is_wrong_with_them() {
        assert_eq!(
            Script::parse("on 9 dance").err(),
            Some(Error::Line(1, "not an action: on 9 dance".into()))
        );
        assert!(matches!(Script::parse("\n# fine\nuntil soon").err(), Some(Error::Line(3, _))));
        let script = Script::parse("seed 3\non 9 type 3121 Anna Alpha; ok\nrepeat 7 days").unwrap();
        assert_eq!(script.seed, 3);
        assert!(matches!(&script.lines[0].1, Line::On(9, actions)
            if matches!(&actions[0], Action::Type(3121, text) if text == "Anna Alpha")));
        assert!(matches!(script.lines[1].1, Line::Repeat(7)));
    }
}
