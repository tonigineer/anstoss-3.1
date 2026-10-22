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
//! <value>`, `rclick <control>`, `drag <box or row> <box or row>|off` (a
//! box or row is `<control>` or `<list> row <row>`), and `wait` until the
//! screen moves on by itself; or on the dialog's pixels: `click at <x>
//! <y>`, `drag at <x> <y> to <x> <y>`. A dialog the script has no actions
//! for stops it, as does an action for a control the dialog does not have.
//!
//! Dialogs go by their ids in the game's dialog file, as the original knows
//! them: the week is dialog 2, though the replica puts it together under ids
//! of its own.

use std::collections::{HashMap, VecDeque};
use std::fmt;

use a3_files::rch::{Dialog, Kind};
use bevy_math::Vec2;

use crate::controls::button::{cancel_button, ok_button};
use crate::game::Game;
use crate::host::{Change, Host};
use crate::pointer::{Grip, Pixels, Target};
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
    /// Drags whatever is at one point of the dialog to another.
    DragAt((i16, i16), (i16, i16)),
    /// Clicks whatever is at a point of the dialog.
    ClickAt((i16, i16)),
    /// The pointer moved to a point of the dialog, over whatever is there.
    PointAt((i16, i16)),
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
        ["click", "at", x, y] => Action::ClickAt((x.parse().ok()?, y.parse().ok()?)),
        ["point", "at", x, y] => Action::PointAt((x.parse().ok()?, y.parse().ok()?)),
        ["drag", "at", x0, y0, "to", x1, y1] => {
            let at = |x: &str, y: &str| Some((x.parse().ok()?, y.parse().ok()?));
            Action::DragAt(at(x0, y0)?, at(x1, y1)?)
        }
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
                // A screen that takes text as it is typed sees it at once.
                let change = self.host.tick(game, 0.0);
                self.change(change);
                return Ok(());
            }
            Action::Wait => return self.wait(shown),
            Action::PointAt(at) => {
                self.host.view_mut().hovered = self.click_at(shown, at).ok();
                self.host.view_mut().pointer = Some(Vec2::new(at.0.into(), at.1.into()));
                let change = self.host.tick(game, 0.0);
                self.change(change);
                return Ok(());
            }
            // A drag from where nothing can be taken hold of does nothing.
            Action::DragAt(from, to) => match self.drag_at(shown, from, to) {
                // A map's tile is pressed, then let go of where the pointer
                // stays.
                Some(drop @ Target::Drop { from: grip @ Grip::Tile { .. }, to: end, .. }) => {
                    self.host.view_mut().pointer = Some(Vec2::new(from.0.into(), from.1.into()));
                    self.host.view_mut().hovered = Some(grip.target());
                    self.change_by(game, grip.target());
                    self.host.view_mut().pointer = Some(Vec2::new(to.0.into(), to.1.into()));
                    self.host.view_mut().hovered = end.map(Grip::target);
                    drop
                }
                Some(target) => target,
                None => return Ok(()),
            },
            // The pointer goes where it clicks.
            Action::ClickAt(at) => {
                let target = self.click_at(shown, at)?;
                self.host.view_mut().pointer = Some(Vec2::new(at.0.into(), at.1.into()));
                // A map's tile is pressed and let go of again, the pointer
                // staying on it.
                match Grip::of(target) {
                    Some(grip @ Grip::Tile { .. }) => {
                        self.host.view_mut().hovered = Some(target);
                        self.change_by(game, target);
                        Target::Drop { from: grip, to: Some(grip), at, moved: (0, 0) }
                    }
                    _ => target,
                }
            }
            _ => match target_of(controls, named(shown), action)? {
                // The pointer goes from the middle of one to the other's.
                Target::Drop { from, to, .. } => {
                    let view = self.host.view();
                    let middle = |grip: Grip| {
                        let rects = crate::pointer::targets(controls, view, &Pixels);
                        let rect = rects.into_iter().find(|&(t, _)| Grip::of(t) == Some(grip));
                        rect.map_or(Vec2::ZERO, |(_, r)| r.center())
                    };
                    let start = middle(from);
                    let d = to.map_or(Vec2::ZERO, |to| middle(to) - start);
                    let (at, moved) = ((start.x as i16, start.y as i16), (d.x as i16, d.y as i16));
                    Target::Drop { from, to, at, moved }
                }
                target => target,
            },
        };
        let change = self.host.input(game, target, false);
        self.change(change);
        Ok(())
    }

    /// What a drag from one point of the dialog `shown` to another takes
    /// hold of and lets go of on, and how far it goes; nothing if there is
    /// nothing to take hold of.
    fn drag_at(&self, shown: u16, from: (i16, i16), to: (i16, i16)) -> Option<Target> {
        let controls = &self.game.dialogs[&shown];
        let rects = crate::pointer::targets(controls, self.host.view(), &Pixels);
        let under = |(x, y): (i16, i16)| {
            let at = Vec2::new(f32::from(x), f32::from(y));
            rects.iter().rev().filter(|(_, r)| r.contains(at)).find_map(|&(t, _)| Grip::of(t))
        };
        let grip = under(from)?;
        let moved = (to.0 - from.0, to.1 - from.1);
        // A map's tile counts let go of where it was taken too.
        let to = under(to).filter(|&g| g != grip || matches!(g, Grip::Tile { .. }));
        Some(Target::Drop { from: grip, to, at: from, moved })
    }

    /// What a click at a point of the dialog `shown` acts on: the topmost
    /// target there.
    fn click_at(&self, shown: u16, (x, y): (i16, i16)) -> Result<Target, Error> {
        let controls = &self.game.dialogs[&shown];
        let at = Vec2::new(f32::from(x), f32::from(y));
        let rects = crate::pointer::targets(controls, self.host.view(), &Pixels);
        let found = rects.iter().rev().find(|(_, r)| r.contains(at)).map(|&(t, _)| t);
        found.ok_or_else(|| Error::Unsupported(named(shown), format!("a click at {x} {y}")))
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

    /// The screen given a target.
    fn change_by(&mut self, game: &Game, target: Target) {
        let change = self.host.input(game, target, false);
        self.change(change);
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
            Target::Drop {
                from: grip(from)?,
                to: to.map(grip).transpose()?,
                at: (0, 0),
                moved: (0, 0),
            }
        }
        Action::RightClick(spot) => Target::Menu(grip(controls, dialog, spot)?),
        Action::Type(id, _) => Target::Edit(find(id)?),
        Action::Wait => return Err(Error::Unsupported(dialog, "wait on a control".into())),
        Action::DragAt(..) | Action::ClickAt(_) | Action::PointAt(_) => {
            return Err(Error::Unsupported(dialog, "an action by pixels".into()));
        }
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
            runner.host.input(&game, Target::Drop { from, to, at: (0, 0), moved: (0, 0) }, false);
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
        assert_eq!([text(&runner, 3741), text(&runner, 5518)], ["Stärke: 3", " 0 €"]);
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
        assert_eq!(text(&runner, 5518), " 30 €");
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
    fn the_comparison_marks_man_for_man_as_the_original_does() {
        let game = game();
        let dialog = &game.dialogs[&78];
        let text = |runner: &Runner, id: u16| {
            runner.host.view().text(crate::screens::control(dialog, id).unwrap()).to_owned()
        };
        let texts = |runner: &Runner, ids: &[u16]| -> Vec<String> {
            ids.iter().map(|&id| text(runner, id)).collect()
        };
        // As the original shows Union against Wolfsburg.
        let then = "now choose 9930 1\nuntil dialog 78";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        assert_eq!(
            text(&runner, 9920),
            "ANSTOSS 3 - Gegenüberstellung 1.FC Union Berlin (unten) - VfL Wolfsburg (oben)"
        );
        let own = texts(&runner, &[5612, 5613, 5614, 5615, 5616, 5617]);
        assert_eq!(own, ["9", "20", "84", "84", "3.8", "6"]);
        let theirs = texts(&runner, &[1744, 1745, 1746, 1747, 1748, 1749]);
        assert_eq!(theirs, ["8", "11", "45", "68", "5.7", "8"]);
        let parts = texts(&runner, &[1730, 5661, 1732, 1733, 1734, 1735, 1736, 1737]);
        assert_eq!(parts, ["9.5", "31.1", "34.4", "22.9", "9.2", "27.4", "38.2", "23.4"]);
        // Of the save's three pairs one marker has left the eleven.
        assert_eq!(runner.host.view().lines.len(), 2);
        // The original's moves: a defender onto a striker, a defender onto
        // a midfielder, two midfielders onto midfielders, a click on the
        // first defender; a defender forward at set pieces, the strikers
        // no longer marked; away to the line-up and back.
        let then = format!(
            "{then}\nnow drag at 143 404 to 379 214\nnow drag at 379 404 to 63 158\n\
             now drag at 261 351 to 203 179\nnow drag at 319 299 to 319 179\n\
             now click at 143 404\nnow click 5619\nnow click at 560 376\nnow click 5662\n\
             now choose 9930 0\nuntil dialog 147\nnow choose 9930 1\nuntil dialog 78"
        );
        let runner = on_save(&game, "Toni", &then).unwrap();
        assert_eq!(runner.host.view().lines.len(), 2);
        let state = runner.host.state();
        let team = &state.records.as_ref().unwrap().teams[84];
        // Moscardo marks Majer, Haberer Svanberg.
        assert_eq!(team.markers()[..3], [8378, 0, 3275]);
        assert_eq!(team.marked()[..3], [1616, 0, 1617]);
        assert_eq!(team.set_piece(), 3262);
        assert_eq!(team.passing(), 0x609b & !0x4000);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_action_areas_follow_their_handles() {
        let game = game();
        // As the original took them: the areas, Skov picked, his bottom
        // right handle out, his top handle up; away and back.
        let then = "now choose 9930 1\nuntil dialog 78\nnow choose 5618 1\nnow click at 93 248\n\
                    now drag at 166 284 to 200 320\nnow drag at 107 211 to 107 150\n\
                    now choose 9930 0\nuntil dialog 147\nnow choose 9930 1\nuntil dialog 78";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        let view = runner.host.view();
        let area = crate::screens::control(&game.dialogs[&78], 9921).unwrap();
        // The man-marking again, as every opening.
        assert!(view.controls[area].hidden && view.shades.is_empty());
        let state = runner.host.state();
        let team = &state.records.as_ref().unwrap().teams[84];
        assert_eq!(team.areas()[5], [79, 98, 107, 72]);
        assert_eq!(team.areas()[1], [79, 42, 79, 42]);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_short_training_keeps_what_is_set() {
        let game = game();
        // On the week's last day: a template, a player in individual
        // training, the camp; a past day refused; away and back.
        let then = "now choose 9930 4\nuntil dialog 346\nnow choose 5035 4\nnow click 4427\n\
                    now row 4427 3\nnow click 4425\nnow click 3831\nuntil dialog 380\nnow press 4731\n\
                    until dialog 346\nnow choose 9930 0\nuntil dialog 147\nnow choose 9930 4\n\
                    until dialog 346";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        assert!(runner.log.iter().any(|line| line == "screen 380"));
        let dialog = &game.dialogs[&346];
        let view = runner.host.view();
        let text = |id: u16| view.text(crate::screens::control(dialog, id).unwrap()).to_owned();
        assert_eq!(text(3831), "Spaziergang");
        assert_eq!((text(3840), text(3842)), (String::new(), "Spiel".to_owned()));
        let combo = crate::screens::control(dialog, 4427).unwrap();
        assert_eq!(view.combo(combo).and_then(|c| c.selected), Some(3));
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        // "ES/Fähigkeiten VORB": training matches all week, from today.
        let plan = records.extras[records.human_slot(0).unwrap()].plan();
        assert_eq!((&plan[..3], &plan[18..21]), (&[26, 26, 26][..], &[33, 33, 33][..]));
        let week = records.humans().next().unwrap().week(62 / 7);
        assert!(week.template == 5 && week.player != 0);
        assert_ne!(records.teams[84].training_flags() & a3_core::training::CAMP, 0);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_long_and_individual_training_keep_what_is_set() {
        let game = game();
        // On a week's first day: an intensity stepped, the button of those
        // who did not play; a template for the week, the camp's food a
        // notch down, the training secret; away.
        let then = "now click bar 5\nuntil dialog 347\nnow cell 3298 1 8\nnow press 4785\n\
                    now click bar 3\nuntil dialog 345\nnow choose 5035 2\n\
                    now click at 134 130\nnow press 5344\nnow click bar 0\nuntil dialog 147";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let manager = records.humans().next().unwrap();
        let team = records.team(manager.country(), manager.team()).unwrap();
        // 0x94 is 0, 1, 1, 2; the last one stepped round to 0.
        assert_eq!(team.intensity_states(), 0x14);
        assert_ne!(team.passing() & a3_core::training::SECRET, 0);
        let slot = usize::from(team.human()) - 1;
        let kept = records.individual[slot].entries().iter().filter(|e| e.id != 0).count();
        assert_eq!(kept, usize::from(team.player_count()).min(a3_core::individual::ENTRIES));
        let week = usize::from(records.globals.day()) / 7;
        assert_eq!(manager.week(week).template, 3);
        assert_eq!(manager.camp_rules()[0], 3);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_calendar_books_and_the_planner_shows() {
        let game = game();
        // September: the family day dragged onto the 10th, the club party
        // of the 15th clicked away; then the week planner.
        let then = "now click bar 6\nuntil dialog 348\nnow drag at 544 355 to 244 180\n\
                    now cell 4459 14 4\nnow click bar 7\nuntil dialog 413";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        let state = runner.host.state();
        let calendar = &state.records.as_ref().unwrap().calendars[0];
        let booked = a3_core::year::day(calendar, 65);
        assert_eq!((booked.kind(), booked.value()), (a3_core::year::EVENT, 0));
        assert_eq!(a3_core::year::day(calendar, 70).kind(), 0);
        let dialog = &game.dialogs[&413];
        let view = runner.host.view();
        let rows = crate::screens::control(dialog, 5034).unwrap();
        let labels = &view.options(rows).unwrap().labels;
        assert_eq!(labels[1], "Vertragsverhandlung\nTom Rothe");
        assert_eq!(labels[6], "Einzelgespräch\nRobert Skov");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_balance_sheet_shows_the_clubs_money() {
        let game = game();
        // As the original showed it; a club of the third league may not go
        // to the stock market.
        let then = "now click bar 8\nuntil dialog 349";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&349];
        let text =
            |id: u16| view.text(crate::screens::control(dialog, id).unwrap()).trim().to_owned();
        assert_eq!([text(4463), text(4464), text(4465)], ["663.765 €", "0 €", "0 €"]);
        assert_eq!([text(4472), text(4473), text(4474)], ["663.765 €", "2.50 Mio €", "3.16 Mio €"]);
        assert_eq!([text(4469), text(4470), text(4471)], ["15%", "12.50%", "(1.70 Mio €)"]);
        assert_eq!([text(4466), text(4467)], ["0 €", "663.765 €"]);
        // The forecasts, the tickets by the attendance model among them.
        assert_eq!([text(4468), text(4475)], ["1.42 Mio €", "- 2.49 Mio €"]);
        assert_eq!(
            [text(5510), text(5511), text(5512), text(5513)],
            ["Verein", "---", "---", "---"]
        );
        let then = "now click bar 8\nuntil dialog 349\nnow press 6152\nuntil dialog 380";
        let runner = on_save(&game, "Probe1", then).unwrap();
        let view = runner.host.view();
        let notice = view.text(crate::screens::control(&game.dialogs[&380], 4730).unwrap());
        assert_eq!(notice, game.text(4582));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_deposit_is_made_and_dissolved() {
        let game = game();
        let deposits = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            a3_core::finances::deposits_of(team, records.globals.fund_prices())
        };
        // 120.000 € with the third partner: a step up from 100.000 €.
        let open = "now click bar 8\nuntil dialog 349\nnow press 4461\nuntil dialog 16";
        let then = format!("{open}\nnow spin 1278 up\nnow choose 6050 2\nnow press 1276");
        let Some(runner) = on_save(&game, "Probe1", &then) else { return };
        let made = deposits(&runner);
        assert_eq!(made.len(), 1);
        assert_eq!((made[0].slot, made[0].partner, made[0].cost), (0, 2, 120_000));
        let then = format!("{then}\nnow row 1266 0\nnow press 1277\nnow press 1\nuntil dialog 349");
        let runner = on_save(&game, "Probe1", &then).unwrap();
        assert!(deposits(&runner).is_empty());
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_loan_is_taken_and_paid_back() {
        let game = game();
        let loans = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            a3_core::finances::loans_of(team)
        };
        // 30.000 € for two years, not to be paid back early; then 10.000 €
        // for a year, which may.
        let then = "now click bar 8\nuntil dialog 349\nnow press 4462\nuntil dialog 335\n\
                    now spin 4363 up\nnow spin 4363 up\nnow choose 6061 4\nnow press 4361\n\
                    now choose 6061 0\nnow slide 4363 1\nnow press 4361";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let taken = loans(&runner);
        assert_eq!(taken.len(), 2);
        assert_eq!((taken[0].principal, taken[0].term, taken[0].repayable), (30_000, 1, false));
        assert_eq!((taken[1].principal, taken[1].term, taken[1].repayable), (10_000, 0, true));
        assert_eq!(taken[0].rates[0] - taken[1].rates[0], -150);
        // The fixed one cannot be paid back; the other all at once.
        let fixed = format!("{then}\nnow row 4299 0\nnow press 4362\nuntil dialog 380");
        assert_eq!(loans(&on_save(&game, "Probe1", &fixed).unwrap()).len(), 2);
        let then = format!(
            "{then}\nnow row 4299 1\nnow press 4362\nuntil dialog 138\n\
                            now press 2457\nuntil dialog 335"
        );
        let left = loans(&on_save(&game, "Probe1", &then).unwrap());
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].principal, 30_000);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_squad_is_listed_by_position() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 0\nuntil dialog 216";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&216];
        let list = view.list(crate::screens::control(dialog, 3298).unwrap()).unwrap();
        // By main position, the keepers first; fifteen columns a row.
        let positions: Vec<&str> =
            list.rows.iter().map(|r| r.split('\t').nth(1).unwrap()).collect();
        assert_eq!(positions[0], "TO");
        assert!(list.rows.iter().all(|r| r.split('\t').count() == 15));
        let line = view.text(crate::screens::control(dialog, 3299).unwrap());
        assert!(
            line.starts_with(&format!("Derzeit verfügen Sie über {} Spieler", list.rows.len()))
        );
        // The fine's mode on the first row opens its dialog.
        let then = format!("{open}\nnow choose 5657 4\nnow row 3298 0\nuntil dialog 85");
        on_save(&game, "Probe1", &then).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_transfer_list_filters_and_watches() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 6371 0\nuntil dialog 287";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&287];
        let list = |runner: &Runner| {
            let view = runner.host.view();
            view.list(crate::screens::control(dialog, 3937).unwrap()).unwrap().clone()
        };
        let all = list(&runner);
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let listed = a3_core::transfers::listings(records);
        assert_eq!(all.rows.len(), listed.iter().filter(|e| e.id != 0).count());
        let count =
            runner.host.view().text(crate::screens::control(dialog, 3936).unwrap()).to_owned();
        assert!(count.contains(&listed.len().to_string()));
        drop(state);
        // Strength 13 and up: nobody; the least strength is kept on OK.
        let strong = format!("{open}\nnow spin 5660 up{}", "\nnow spin 5660 up".repeat(12));
        assert!(list(&on_save(&game, "Probe1", &strong).unwrap()).rows.is_empty());
        let kept = format!("{strong}\nnow press 3938\nuntil dialog 218");
        let runner = on_save(&game, "Probe1", &kept).unwrap();
        let state = runner.host.state();
        let manager = state.records.as_ref().unwrap().managers[0].least_strength();
        assert_eq!(manager, 13);
        drop(state);
        // A player watched turns green after the next refill.
        let id = all.keys[0] as u16;
        let watch = format!(
            "{open}\nnow rclick 3937 row 0\nnow choose 9950 1\nnow choose 5658 1\nnow choose 5658 0"
        );
        let runner = on_save(&game, "Probe1", &watch).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        assert!(a3_core::transfers::watched(records, 0).contains(&id));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_agreed_transfers_are_read_by_their_pages() {
        let game = game();
        let Some(runner) = on_save(&game, "Toni", "") else { return };
        let state = runner.host.state();
        let agreed = state.records.as_ref().unwrap().agreed();
        // Union buys Reis from Girona; Ilic and Markgraf come back from
        // their loans (KADER-NAECHSTE-VERLIEHEN.md §1.2).
        let union = (14, 84);
        let ids: Vec<(u16, bool)> = agreed.iter().map(|a| (a.player, a.flagged)).collect();
        assert_eq!(ids, [(7897, false), (3281, true), (3267, true)]);
        assert!(agreed.iter().all(|a| a.buyer == union && a.fee == 0));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn next_seasons_squad_marks_and_refills() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 1\nuntil dialog 219";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&219];
        let list = runner
            .host
            .view()
            .list(crate::screens::control(dialog, 3318).unwrap())
            .unwrap()
            .clone();
        assert!(list.rows.iter().all(|r| r.split('\t').count() == 14));
        // By main position: the keepers first.
        assert_eq!(list.rows[0].split('\t').nth(1), Some("TO"));
        // A click marks a player and fills the list anew: nothing selected.
        let then = format!("{open}\nnow row 3318 2");
        let runner = on_save(&game, "Probe1", &then).unwrap();
        let list = runner
            .host
            .view()
            .list(crate::screens::control(dialog, 3318).unwrap())
            .unwrap()
            .clone();
        assert_eq!(list.selected, None);
        // The players out on loan: as many rows as loans coming home.
        let then = "now click bar 10\nuntil dialog 218\nnow choose 5042 5\nuntil dialog 300";
        on_save(&game, "Probe1", then).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_contract_is_offered_a_year_more() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 4\nuntil dialog 92";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&92];
        let list = |runner: &Runner| {
            let view = runner.host.view();
            view.list(crate::screens::control(dialog, 1883).unwrap()).unwrap().clone()
        };
        // The players in their contract's last year, in the squad's order.
        let rows = list(&runner);
        assert_eq!(rows.rows.len(), 5);
        assert!(rows.rows.iter().all(|r| r.split('\t').count() == 13));
        assert!(rows.rows.iter().all(|r| r.ends_with("\t30.6.2026\tNein")));
        let id = rows.keys[0] as u16;
        // A click: he takes a year more, or declines; either way he is off
        // the list once the alert is gone.
        let offer =
            format!("{open}\nnow row 1883 0\nuntil dialog 380\nnow press 4731\nuntil dialog 92");
        let runner = on_save(&game, "Probe1", &offer).unwrap();
        assert_eq!(list(&runner).rows.len(), 4);
        let state = runner.host.state();
        let r = state.records.as_ref().unwrap().player(id).unwrap();
        assert!(r.contract() & 0xfff == 2027 || r.retiring() & 0x40 != 0);
        // Nobody the club holds an option on.
        let options = format!("{open}\nnow choose 1884 1");
        assert!(list(&on_save(&game, "Probe1", &options).unwrap()).rows.is_empty());
        // No youth of another club without a pre-contract.
        let poaching = "now click bar 10\nuntil dialog 218\nnow choose 6371 3\nuntil dialog 107";
        on_save(&game, "Probe1", poaching).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_youth_is_supported_and_signs_a_pre_contract() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 2\nuntil dialog 428";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&428];
        let rows = |runner: &Runner, id: u16| {
            let view = runner.host.view();
            view.list(crate::screens::control(dialog, id).unwrap()).unwrap().rows.clone()
        };
        // Two B youths, no A or C youth.
        assert!(rows(&runner, 5264).is_empty() && rows(&runner, 5262).is_empty());
        let youths = rows(&runner, 5263);
        assert_eq!(youths.len(), 2);
        assert!(youths.iter().all(|r| r.split('\t').count() == 10));
        assert!(youths.iter().all(|r| r.ends_with("\tkein Vorvertrag")));
        // A click in "Förd." gives the first the group's support.
        let support = format!("{open}\nnow click at 397 209");
        let runner = on_save(&game, "Probe1", &support).unwrap();
        assert_eq!(rows(&runner, 5263)[0].split('\t').nth(7), Some("X"));
        // Anywhere else: the parents' pre-contract, then too young for more.
        let sign = format!(
            "{open}\nnow click at 100 209\nuntil dialog 325\nnow press 4190\nuntil dialog 428\n\
             now click at 100 209\nuntil dialog 380\nnow press 4731\nuntil dialog 428"
        );
        let runner = on_save(&game, "Probe1", &sign).unwrap();
        assert!(rows(&runner, 5263)[0].ends_with(" €"));
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let first =
            runner.host.view().list(crate::screens::control(dialog, 5263).unwrap()).unwrap().keys
                [0];
        let r = records.player(first as u16).unwrap();
        assert_eq!(r.contract() & 0xfff, records.globals.start_year() + 10);
        // The amateurs open from the same menu.
        let amateurs = "now click bar 10\nuntil dialog 218\nnow choose 5042 3\nuntil dialog 427";
        on_save(&game, "Probe1", amateurs).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_squads_friends_and_enemies_are_listed() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 0\nuntil dialog 216\n\
                    now press 6280\nuntil dialog 500";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&500];
        let shown = |runner: &Runner| {
            let view = runner.host.view();
            let list = view.list(crate::screens::control(dialog, 3298).unwrap()).unwrap().clone();
            let titles = view.text(crate::screens::control(dialog, 6281).unwrap()).to_owned();
            (list, titles)
        };
        // Every player of the squad, his group, best friend and friends.
        let (list, friends) = shown(&runner);
        let squad =
            runner.host.state().season.as_ref().map_or(0, |s| {
                s.clubs.get(&s.manager().unwrap().club.unwrap()).unwrap().players.len()
            });
        assert_eq!(list.rows.len(), squad);
        assert!(list.rows.iter().all(|r| r.split('\t').count() == 4));
        // The enmities, under titles of their own.
        let enemies = format!("{open}\nnow choose 1884 1");
        let (list, titles) = shown(&on_save(&game, "Probe1", &enemies).unwrap());
        assert_eq!(list.rows.len(), squad);
        assert_ne!(titles, friends);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_stadium_is_expanded() {
        use a3_core::stadium;
        let game = game();
        let crane = "now click bar 11\nuntil dialog 464\nnow click at 311 145\nuntil dialog 463";
        let Some(runner) = on_save(&game, "Probe1", crane) else { return };
        let club = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            let subs = stadium::subs(records, team.country(), u16::from(team.number())).unwrap();
            (team.get::<i64>(0x2d8 + 0x278), subs[0].clone())
        };
        let (costs, sub) = club(&runner);
        // Ten more standing places on the Gegentribüne are far beyond the
        // money: refused.
        let dear = format!("{crane}\nnow spin 5875 up\nnow press 1\nuntil dialog 380");
        on_save(&game, "Probe1", &dear).unwrap();
        // The right stand renovated a step: its cost booked, the stadium
        // carried, once the screen is left.
        let mut plan = sub.clone();
        plan.set_condition(3, sub.condition(3) - 1);
        let renovation = stadium::stand_cost(&plan, &sub, 3).renovation;
        assert!(renovation > 0);
        let renovate = format!(
            "{crane}\nnow choose 5915 3\nnow press 1\nuntil dialog 464\nnow click bar 0\nuntil dialog 147"
        );
        let (after, carried) = club(&on_save(&game, "Probe1", &renovate).unwrap());
        assert_eq!(after - costs, i64::from(renovation));
        assert_eq!(carried.condition(3), sub.condition(3) - 1);
        // The stadium seen from the screen and from the plan.
        let views = format!(
            "{crane}\nnow choose 5916 0\nuntil dialog 249\nnow press 1\nuntil dialog 463\n\
             now press 2\nuntil dialog 464\nnow choose 5852 0\nuntil dialog 249\nnow press 1\n\
             until dialog 464"
        );
        on_save(&game, "Probe1", &views).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_grounds_are_shown_and_their_view_kept() {
        let game = game();
        let open = "now click bar 11\nuntil dialog 464\nnow press 5861\nuntil dialog 466";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&466];
        let text =
            |runner: &Runner, id| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        assert_eq!(text(&runner, 6291), "Kaufkosten:");
        assert!(text(&runner, 5966).starts_with("Apfelbaum\n"), "{}", text(&runner, 5966));
        // The club's objects, sorted, the first chosen, with its weeks.
        let club = format!("{open}\nnow choose 6471 1");
        let runner = on_save(&game, "Probe1", &club).unwrap();
        assert!(text(&runner, 5966).starts_with("Doping-Labor\n"), "{}", text(&runner, 5966));
        // Zoomed in and left, the grounds keep the zoom.
        let zoomed = format!("{open}\nnow press 5969\nnow press 1\nuntil dialog 464");
        let runner = on_save(&game, "Probe1", &zoomed).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let manager = records.humans().next().unwrap();
        let team = records.team(manager.country(), manager.team()).unwrap();
        let grounds =
            a3_core::grounds::Grounds::of(records, team.country(), i16::from(team.number()))
                .unwrap();
        assert_eq!(grounds.view().zoom, 2);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_grounds_sub_dialogs_report_on_them() {
        let game = game();
        let open = "now click bar 11\nuntil dialog 464\nnow press 5861\nuntil dialog 466";
        let Some(runner) = on_save(&game, "Probe1", &format!("{open}\nnow press 5971")) else {
            return;
        };
        // The info: the club's name, its land in hectares.
        let text = |runner: &Runner, dialog: u16, id: u16| {
            let control = control(&game.dialogs[&dialog], id).unwrap();
            runner.host.view().text(control).to_owned()
        };
        assert!(text(&runner, 467, 6250).starts_with("Vereinsgelände "));
        assert!(text(&runner, 467, 6251).ends_with(" ha"), "{}", text(&runner, 467, 6251));
        // The staff, sorted; the fans' complaints or their content; what
        // is being built or that nothing is — each closed by its OK.
        let rows = |runner: &Runner, dialog: u16, id: u16| {
            let control = control(&game.dialogs[&dialog], id).unwrap();
            runner.host.view().list(control).unwrap().rows.clone()
        };
        let then = |button: u16| format!("{open}\nnow press 5971\nnow press 1\nnow press {button}");
        let runner = on_save(&game, "Probe1", &then(5774)).unwrap();
        let staff = rows(&runner, 465, 5261);
        assert!(!staff.is_empty() && staff.is_sorted_by_key(|r| r.to_lowercase()), "{staff:?}");
        let runner = on_save(&game, "Probe1", &then(5972)).unwrap();
        assert!(!rows(&runner, 468, 5996).is_empty());
        let runner = on_save(&game, "Probe1", &format!("{}\nnow press 1", then(5979))).unwrap();
        assert_eq!(runner.host.view().id, 466);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn land_is_bought_built_on_cleared_and_sold() {
        let game = game();
        let open = "now click bar 11\nuntil dialog 464\nnow press 5861\nuntil dialog 466";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let grounds = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            let money = (team.get::<i64>(0x2e8 + 0x278), team.get::<u32>(0x2e0 + 0x278));
            let grounds =
                a3_core::grounds::Grounds::of(records, team.country(), team.number().into());
            (grounds.unwrap(), money)
        };
        // A tile in the first view that is not the club's, with nothing on it.
        let (before, money) = grounds(&runner);
        let view = before.view().zoomed(1);
        let ((ox, oy), (offx, offy)) = view.origin();
        let (c, r) = (0..27)
            .flat_map(|r| (0..27).map(move |c| (c, r)))
            .find(|&(c, r)| {
                let (x, y) = ((ox + c) as usize, (oy + r) as usize);
                !before.owns(x, y) && before.at(x, y).is_none()
            })
            .expect("land to buy");
        let (x, y) = ((ox + c) as usize, (oy + r) as usize);
        let at = format!("now click at {} {}", 2 + c * 16 - offx + 8, 25 + r * 16 - offy + 8);
        // Kaufen asks, at 5.000 € a tile.
        let runner = on_save(&game, "Probe1", &format!("{open}\n{at}")).unwrap();
        let question = runner.host.view().text(control(&game.dialogs[&384], 4738).unwrap());
        assert!(question.contains("5.000"), "{question}");
        // Bought, an Apfelbaum planted, pulled down, the land sold: the
        // extensions take 5.000 + 1.000 + 50 €, the stadium's income 3.000 €.
        let then = format!(
            "{open}\n{at}\nnow press 4741\nnow click at 521 187\n{at}\nnow click at 611 187\n\
             {at}\nnow press 4741\nnow click at 566 187\n{at}\nnow press 4741\n\
             now press 1\nuntil dialog 464"
        );
        let runner = on_save(&game, "Probe1", &then).unwrap();
        let (after, (extensions, income)) = grounds(&runner);
        assert_eq!(extensions - money.0, 6050);
        assert_eq!(income.wrapping_sub(money.1), 3000);
        assert!(!after.owns(x, y) && after.at(x, y).is_none());
        assert_eq!(after.count(), before.count());
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_stadium_is_shown_and_its_comfort_bought() {
        let game = game();
        let open = "now click bar 11\nuntil dialog 464";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&464];
        let text = |runner: &Runner, id: u16| {
            runner.host.view().text(crate::screens::control(dialog, id).unwrap()).to_owned()
        };
        // The places add up to the capacity.
        let places: Vec<i32> = (5846..=5850).map(|id| text(&runner, id).parse().unwrap()).collect();
        assert_eq!(places.iter().sum::<i32>(), text(&runner, 5851).parse::<i32>().unwrap());
        let stands = runner
            .host
            .view()
            .list(crate::screens::control(dialog, 5853).unwrap())
            .unwrap()
            .rows
            .len();
        assert_eq!(stands, 4);
        // Box seats ticked: they cost, are carried out, and stay once left.
        let state = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            let subs =
                a3_core::stadium::subs(records, team.country(), team.number().into()).unwrap();
            (team.get::<i64>(0x2d8 + 0x278), subs[0].has(a3_core::stadium::Comfort::Boxes))
        };
        let (costs, boxes) = state(&runner);
        assert!(!boxes);
        let buy = format!(
            "{open}\nnow click at 150 427\nnow press 5917\nuntil dialog 384\nnow press 4741\n\
             until dialog 464\nnow click bar 10\nuntil dialog 218"
        );
        let runner = on_save(&game, "Probe1", &buy).unwrap();
        assert_eq!(state(&runner), (costs + 250_000, true));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn any_clubs_squad_is_looked_through() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 6371 4\nuntil dialog 306";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&306];
        let list = |runner: &Runner, id: u16| {
            runner.host.view().list(crate::screens::control(dialog, id).unwrap()).unwrap().clone()
        };
        // The main country's clubs but our own, by name; no players yet.
        let clubs = list(&runner, 4060);
        assert!(!clubs.rows.is_empty());
        let key = |name: &str| name.to_lowercase();
        assert!(clubs.rows.windows(2).all(|w| key(&w[0]) <= key(&w[1]) || !w[0].is_ascii()));
        assert!(list(&runner, 4059).rows.is_empty());
        // A club: its players, by position, and the heading.
        let club = format!("{open}\nnow row 4060 0");
        let runner = on_save(&game, "Probe1", &club).unwrap();
        let players = list(&runner, 4059);
        assert!(!players.rows.is_empty());
        assert!(players.rows.iter().all(|r| r.split('\t').count() == 7));
        let heading =
            runner.host.view().text(crate::screens::control(dialog, 6286).unwrap()).to_owned();
        assert!(heading.contains("D-Stärke"), "{heading}");
        // NORACSA needs a computer department, which Ulm lacks.
        let noracsa = "now click bar 10\nuntil dialog 218\nnow choose 6371 6\nuntil dialog 380";
        on_save(&game, "Probe1", noracsa).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_calculations_page_and_book_what_they_may() {
        let game = game();
        let open = "now click bar 9\nuntil dialog 417";
        let staff = format!("{open}\nnow click at 88 225\nuntil dialog 447");
        let Some(runner) = on_save(&game, "Probe1", &staff) else { return };
        let booked = |runner: &Runner, c: usize| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            (team.get::<u8>(c + 0x278), team.get::<i32>(0x228 + 0x50 + 0x278))
        };
        // Ulm's groundsman, level 0: raised by his spinner, booked on OK.
        assert_eq!(booked(&runner, 0x301), (0, 12_000));
        let raise = format!("{staff}\nnow spin 5704 up\nnow click at 606 449\nuntil dialog 417");
        assert_eq!(booked(&on_save(&game, "Probe1", &raise).unwrap(), 0x301), (1, 24_000));
        // The arrow bar pages on to the players' calculation and back.
        let paging = format!(
            "{staff}\nnow click at 479 439\nuntil dialog 454\nnow click at 440 439\n\
             until dialog 447\nnow click at 547 449\nuntil dialog 417"
        );
        on_save(&game, "Probe1", &paging).unwrap();
        // A better statistics department, booked at its yearly price.
        let statistics = format!(
            "{open}\nnow click at 88 318\nuntil dialog 122\nnow click at 273 274\n\
             now click at 606 449\nuntil dialog 417"
        );
        let runner = on_save(&game, "Probe1", &statistics).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let manager = records.humans().next().unwrap();
        let team = records.team(manager.country(), manager.team()).unwrap();
        assert_eq!(team.get::<u8>(0x5e1 + 0x278), 1);
        // The ticket office lists the home matches ahead; its standing
        // price, raised, is written at once and kept though it is
        // cancelled.
        let tickets = format!("{open}\nnow click at 88 64\nuntil dialog 137");
        let runner = on_save(&game, "Probe1", &tickets).unwrap();
        let list = control(&game.dialogs[&137], 2437).unwrap();
        let first = runner.host.view().list(list).unwrap().rows[0].clone();
        assert_eq!(first, "31.8.2025\tLigaspiel\tFC Ingolstadt 04\t0\tnormal");
        let raised = format!("{tickets}\nnow spin 2423 up\nnow click at 547 449\nuntil dialog 417");
        let price = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            records.team(manager.country(), manager.team()).unwrap().get::<i16>(0x49c + 0x278)
        };
        assert_eq!(price(&runner), 15);
        assert_eq!(price(&on_save(&game, "Probe1", &raised).unwrap()), 16);
        // Back from the tickets to the shares, round on to the TV money;
        // from the sponsors on to the merchandise, whose OK closes it.
        let round = format!(
            "{tickets}\nnow click at 440 439\nuntil dialog 446\nnow click at 479 439\n\
             until dialog 137\nnow click at 479 439\nuntil dialog 448"
        );
        on_save(&game, "Probe1", &round).unwrap();
        let merchandise = format!(
            "{open}\nnow click at 88 133\nuntil dialog 125\nnow click at 479 439\n\
             until dialog 115\nnow click at 606 449\nuntil dialog 417"
        );
        on_save(&game, "Probe1", &merchandise).unwrap();
        // The main sponsor's contract, from the sponsors' page and back.
        let contract = format!(
            "{open}\nnow click at 88 133\nuntil dialog 125\nnow press 2351\nuntil dialog 246"
        );
        let runner = on_save(&game, "Probe1", &contract).unwrap();
        let text =
            |id| runner.host.view().text(control(&game.dialogs[&246], id).unwrap()).to_owned();
        assert!(text(3424).starts_with("Sponsorenpool ("), "{}", text(3424));
        assert!(text(3416).ends_with(" %"), "{}", text(3416));
        on_save(&game, "Probe1", &format!("{contract}\nnow press 3422\nuntil dialog 125")).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn players_are_watched_and_dropped() {
        let game = game();
        let menu = "now click bar 10\nuntil dialog 218";
        // Two players from the transfer list onto the watch list.
        let watch = format!(
            "{menu}\nnow choose 6371 0\nuntil dialog 287\nnow rclick 3937 row 0\nnow choose 9950 1\n\
             now rclick 3937 row 1\nnow choose 9950 1\nnow press 3938\nuntil dialog 218\n\
             now choose 6371 2\nuntil dialog 27"
        );
        let Some(runner) = on_save(&game, "Probe1", &watch) else { return };
        let dialog = &game.dialogs[&27];
        let rows = |runner: &Runner| {
            let view = runner.host.view();
            view.list(crate::screens::control(dialog, 1330).unwrap()).unwrap().rows.len()
        };
        assert_eq!(rows(&runner), 2);
        let line =
            runner.host.view().text(crate::screens::control(dialog, 1329).unwrap()).to_owned();
        assert_eq!(line, "Derzeit stehen 2 Spieler unter Beobachtung.");
        // The first dropped again through his choices.
        let drop =
            format!("{watch}\nnow row 1330 0\nuntil dialog 28\nnow choose 1333 1\nuntil dialog 27");
        assert_eq!(rows(&on_save(&game, "Probe1", &drop).unwrap()), 1);
        // The week's appointments open, as public as they are.
        let talks = format!("{menu}\nnow choose 6371 1\nuntil dialog 25");
        on_save(&game, "Probe1", &talks).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_purchase_is_booked_and_a_talk_joined() {
        use a3_core::transfers;
        let game = game();
        let list = "now click bar 10\nuntil dialog 218\nnow choose 6371 0\nuntil dialog 287";
        let Some(runner) = on_save(&game, "Probe1", list) else { return };
        let rows = control(&game.dialogs[&287], 3937).unwrap();
        let keys: Vec<u16> =
            runner.host.view().list(rows).unwrap().keys.iter().map(|&k| k as u16).collect();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let manager = records.humans().next().unwrap();
        let own = (manager.country() as u8, manager.team() as i16);
        let start = records.globals.week_start();
        let listed = transfers::listings(records);
        let sale = |id: u16| listed.iter().any(|e| e.id == id && e.kind == transfers::SALE);
        let theirs = |id: u16| records.player(id).is_some_and(|r| r.club()[0] != 0);
        // A player of another club with no appointment, and one with an
        // appointment this week on a day the club has free.
        let free = keys
            .iter()
            .position(|&id| sale(id) && theirs(id) && transfers::talk_day(records, id).is_none())
            .unwrap();
        let fixed = keys
            .iter()
            .position(|&id| {
                transfers::talk_day(records, id).is_some_and(|d| {
                    (start..start + 7).contains(&d)
                        && records.planners[0].day(d as usize).kind == 0
                        && !transfers::talks_on(records, own, d)
                }) && sale(id)
                    && theirs(id)
            })
            .unwrap();
        let fixed_day = transfers::talk_day(records, keys[fixed]).unwrap();
        drop(state);
        // The first is booked on Saturday, in the planner as a negotiation
        // of the transfer list.
        let buy = format!(
            "{list}\nnow row 3937 {free}\nuntil dialog 412\nnow choose 5034 6\nuntil dialog 287"
        );
        let runner = on_save(&game, "Probe1", &buy).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let talks = transfers::appointments(records);
        let talk = talks.iter().find(|t| t.player == keys[free]).unwrap();
        assert_eq!((talk.kind, talk.day, talk.clubs[1]), (3, start + 6, own));
        let booked = records.planners[0].day(start as usize + 6);
        assert_eq!((booked.kind, booked.player), (0x0e, keys[free]));
        drop(state);
        // The club joins the second's, on its day.
        let join = format!(
            "{list}\nnow row 3937 {fixed}\nuntil dialog 412\nnow press 1\nuntil dialog 287"
        );
        let runner = on_save(&game, "Probe1", &join).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let talks = transfers::appointments(records);
        assert!(talks.iter().find(|t| t.player == keys[fixed]).unwrap().clubs.contains(&own));
        let booked = records.planners[0].day(fixed_day as usize);
        assert_eq!((booked.kind, booked.player), (0x0e, keys[fixed]));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_finances_show_the_season_row_by_row() {
        let game = game();
        let texts = |then: &str, ids: &[u16]| -> Vec<String> {
            let then = format!("now click bar 9\nuntil dialog 417\n{then}");
            let runner = on_save(&game, "Probe1", &then).unwrap();
            let view = runner.host.view();
            let dialog = &game.dialogs[&417];
            ids.iter()
                .map(|&id| view.text(crate::screens::control(dialog, id).unwrap()).to_owned())
                .collect()
        };
        if game.save_games().iter().all(|s| s.name != "Probe1") {
            return;
        }
        // As the original showed them: the tickets, the TV money, the
        // sponsors, the other income, the staff, the players, the matches
        // and training, the stadium, the other costs, the result.
        let rows = [5093, 5094, 5096, 5098, 5086, 5087, 5088, 5089, 5090, 5092, 1616];
        assert_eq!(
            texts("", &rows),
            [
                "85.487", "900.000", "266.000", "19.583", "34.566", "272.833", "170.740",
                "121.666", "7.500", "663.765", "Gewinn"
            ]
        );
        // This week's: the tickets and sponsors since the turn began.
        assert_eq!(texts("now row 2226 3", &[5093, 5096, 5092]), ["8.171", "45.000", "53.171"]);
        // The season carried on: the tickets at the plan's attendance.
        assert_eq!(texts("now row 2226 0", &[5093]), ["1.245.752"]);
        // The season extrapolated.
        let rows = [5096, 5098, 5086, 5087, 5088, 5090];
        assert_eq!(
            texts("now row 2226 1", &rows),
            ["280.000", "230.000", "410.000", "208.431", "397.890", "90.000"]
        );
        assert_eq!(texts("now row 2226 1", &[5093, 5092]), ["1.240.000", "1.422.013"]);
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
            from: at,
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
