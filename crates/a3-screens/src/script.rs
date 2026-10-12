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

use a3_files::rch::Kind;

use crate::controls::button::{CANCEL_IMAGE, OK_IMAGE, with_image};
use crate::game::Game;
use crate::host::{Change, Host};
use crate::pointer::Target;
use crate::screens::calendar::{WEEK, is_week};
use crate::screens::control;

/// Time passes in steps this long while a script waits, for at most this
/// long before it gives up.
const STEP_SECS: f32 = 0.05;
const MOST_SECS: f32 = 60.0;
const DEFAULT_SEED: u64 = 1;

/// A parsed script.
pub struct Script {
    seed: u64,
    lines: Vec<(usize, Line)>,
}

enum Line {
    On(u16, Vec<Action>),
    Once(u16, Vec<Action>),
    Until(Until),
    Repeat(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Until {
    Dialog(u16),
    Day(usize),
    End,
}

#[derive(Clone, Debug)]
enum Action {
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
    Wait,
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
        let (game, dialog) = (self.game, named(shown));
        let controls = &game.dialogs[&shown];
        let find = |id: u16| control(controls, id).ok_or(Error::NoControl { dialog, control: id });
        let button =
            |image: u16| with_image(controls, image).ok_or(Error::NoControl { dialog, control: 0 });
        let target = match *action {
            Action::Ok => Target::Button(button(OK_IMAGE)?),
            Action::Cancel => Target::Button(button(CANCEL_IMAGE)?),
            Action::Press(id) => Target::Button(find(id)?),
            Action::Click(id) => Target::Control(find(id)?),
            Action::Bar(item) => {
                let bar = controls.controls.iter().position(|c| matches!(c.kind, Kind::Bar { .. }));
                Target::Option {
                    control: bar.ok_or(Error::NoControl { dialog, control: 0 })?,
                    item,
                }
            }
            Action::Choose(id, item) => Target::Option { control: find(id)?, item },
            Action::Row(id, row) => Target::Row { control: find(id)?, row },
            Action::Spin(id, up) => Target::Spin { control: find(id)?, up },
            Action::Slide(id, value) => Target::SliderAt { control: find(id)?, value },
            Action::Type(id, ref text) => {
                let field = find(id)?;
                let change = self.host.input(game, Target::Edit(field), false);
                self.change(change);
                self.host.view_mut().set_text(field, "");
                self.host.view_mut().type_text(controls, text);
                return Ok(());
            }
            Action::Wait => return self.wait(shown),
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
        // Every turn ends with the whistle.
        let whistles = runner.log.iter().filter(|line| line.trim() == "Bar(18)").count();
        assert_eq!(whistles, 2 * 7);
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
