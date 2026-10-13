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
//! `row <list> <row>`, `type <field> <text>`, `spin <spinner> up|down`,
//! `slide <slider> <value>`, and `wait` until the screen moves on by
//! itself. A dialog the script has no actions for stops it, as does an
//! action for a control the dialog does not have.
//!
//! Dialogs go by their ids in the game's dialog file, as the original knows
//! them: the week is dialog 2, though the replica puts it together under ids
//! of its own.

use std::collections::{HashMap, VecDeque};
use std::fmt;

use a3_files::rch::{Dialog, Kind};

use crate::controls::button::{CANCEL_IMAGE, OK_IMAGE, with_image};
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
    Type(u16, String),
    Spin(u16, bool),
    Slide(u16, i32),
    /// Drags a control (a player's box) or a list's row onto another, or off
    /// them all.
    Drag(Spot, Option<Spot>),
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
        ["spin", spinner, "up"] => Action::Spin(id(spinner)?, true),
        ["spin", spinner, "down"] => Action::Spin(id(spinner)?, false),
        ["slide", slider, value] => Action::Slide(id(slider)?, value.parse().ok()?),
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
    let button =
        |image: u16| with_image(controls, image).ok_or(Error::NoControl { dialog, control: 0 });
    Ok(match *action {
        Action::Ok => Target::Button(button(OK_IMAGE)?),
        Action::Cancel => Target::Button(button(CANCEL_IMAGE)?),
        Action::Press(id) => Target::Button(find(id)?),
        Action::Click(id) => Target::Control(find(id)?),
        Action::Bar(item) => {
            let bar = controls.controls.iter().position(|c| matches!(c.kind, Kind::Bar { .. }));
            Target::Option { control: bar.ok_or(Error::NoControl { dialog, control: 0 })?, item }
        }
        Action::Choose(id, item) => Target::Option { control: find(id)?, item },
        Action::Row(id, row) => Target::Row { control: find(id)?, row },
        Action::Spin(id, up) => Target::Spin { control: find(id)?, up },
        Action::Slide(id, value) => Target::SliderAt { control: find(id)?, value },
        Action::Drag(from, to) => {
            let grip = |spot: Spot| match spot {
                Spot::Control(id) => Ok(Grip::Control(find(id)?)),
                Spot::Row(id, row) => Ok(Grip::Row { control: find(id)?, row }),
            };
            Target::Drop { from: grip(from)?, to: to.map(grip).transpose()? }
        }
        Action::Type(id, _) => Target::Edit(find(id)?),
        Action::Wait => return Err(Error::Unsupported(dialog, "wait on a control".into())),
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

            // The squad as the save has it: the eleven, the bench, the rest.
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            let squad = team.players();
            let mut order: Vec<u16> =
                team.lineup().into_iter().chain(team.bench()).filter(|&id| id != 0).collect();
            let rest: Vec<u16> = squad.iter().copied().filter(|id| !order.contains(id)).collect();
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
