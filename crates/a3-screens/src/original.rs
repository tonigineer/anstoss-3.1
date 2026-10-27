//! Plays a script on the original game, through the probe inside it
//! (`tools/probe`): the scripts of [`crate::script`], acted out on the
//! original's own dialogs with the mouse and keyboard, so that the two can be
//! compared run for run.
//!
//! The probe lists the dialogs the original has up, by their ids in the
//! dialog file. A dialog waits for the player once it has stayed on top for a
//! moment; an action becomes a click in the middle of what it acts on, where
//! the dialog file puts it, and keys for a text field. The day comes from the
//! game's memory.
//!
//! A script's `seed` is the replica's: the original's random numbers start
//! where the probe seeds them (`A3PROBE_SRAND`).
//!
//! Exploring, a dialog the script has no actions for is answered with its
//! OK button, or its only button, if it has one, and the log marks it — to
//! find out which dialogs the original shows on the way.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::thread::sleep;
use std::time::{Duration, Instant};

use a3_files::rch::Kind;
use bevy_math::{Rect, Vec2};

use crate::controls::menu;
use crate::game::Game;
use crate::pointer::{self, Grip, Pixels, Target};
use crate::script::{Action, Error, Line, Script, Until, target_of};
use crate::view::View;

/// A dialog waits for the player once it has been on top this long.
const SETTLED: Duration = Duration::from_millis(600);
/// After an action, for the original to take it in.
const PAUSE: Duration = Duration::from_millis(300);
/// How often the probe is asked while waiting.
const POLL: Duration = Duration::from_millis(100);
/// How long a dialog may take to move on before the run gives up.
const MOST: Duration = Duration::from_secs(300);
/// How often the same dialog may be acted on without moving on.
const MOST_TURNS: usize = 20;
/// How long the probe may take to answer before the game counts as gone —
/// a game that crashed stops in Wine's debugger, probe and all.
const ANSWER: Duration = Duration::from_secs(60);
/// Frames the original shows a page of its own in, which the replica shows
/// as the page: the player info (`CSpielerinfo`) around its four pages.
const FRAMES: [u16; 1] = [214];
/// The game's day, counted from the start of the first season (16 bits).
const DAY: u32 = 0xb5cbd4;
/// Keys: to the end of a field, and back over a character.
const VK_END: u8 = 0x23;
const VK_BACK: u8 = 0x08;
/// A field without a length in the dialog file is cleared of this many.
const FIELD_LEN: usize = 40;

/// The probe's command channel: a line out, a line back.
pub struct Probe {
    replies: BufReader<TcpStream>,
    stream: TcpStream,
}

impl Probe {
    pub fn connect(port: u16) -> Result<Self, Error> {
        let stream = TcpStream::connect(("127.0.0.1", port))
            .map_err(|err| Error::Probe(format!("cannot connect on port {port}: {err}")))?;
        stream.set_read_timeout(Some(ANSWER)).map_err(gone)?;
        let replies = BufReader::new(stream.try_clone().map_err(gone)?);
        Ok(Self { replies, stream })
    }

    /// Sends a command; what follows "ok" in the reply.
    fn ask(&mut self, command: &[u8]) -> Result<String, Error> {
        self.stream.write_all(command).and_then(|()| self.stream.write_all(b"\n")).map_err(gone)?;
        let mut reply = String::new();
        if self.replies.read_line(&mut reply).map_err(gone)? == 0 {
            return Err(Error::Probe("the game is gone".into()));
        }
        let reply = reply.trim_end();
        match reply.split_once(' ').unwrap_or((reply, "")) {
            ("ok", rest) => Ok(rest.to_owned()),
            (_, what) => Err(Error::Probe(format!("{}: {what}", String::from_utf8_lossy(command)))),
        }
    }
}

fn gone(err: std::io::Error) -> Error {
    Error::Probe(format!("the game is gone: {err}"))
}

/// The dialog on top: its id, which one it is, and how many dialogs had been
/// shown when it was seen — a dialog shown and gone in between counts as a
/// change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Up {
    id: u16,
    at: u64,
    shown: u64,
}

/// Plays scripts on the original.
pub struct Original<'a> {
    game: &'a Game,
    probe: Probe,
    on: HashMap<u16, Vec<Action>>,
    once: HashMap<u16, VecDeque<Vec<Action>>>,
    /// Every screen that came up and every action, in order.
    pub log: Vec<String>,
    logged: Option<Up>,
    explore: bool,
    /// Where the last right click fell: the menu it opened is there.
    menu_at: Option<Vec2>,
}

impl<'a> Original<'a> {
    /// Plays a script on the original the probe is in. `game` is the same
    /// installation, composed as the replica shows it ([`crate::screens::compose`]),
    /// for where the dialogs have their controls.
    pub fn play(
        game: &'a Game,
        probe: Probe,
        script: &Script,
        explore: bool,
    ) -> (Self, Result<(), Error>) {
        let mut original = Self {
            game,
            probe,
            on: HashMap::new(),
            once: HashMap::new(),
            log: Vec::new(),
            logged: None,
            explore,
            menu_at: None,
        };
        let result = original.run(script);
        (original, result)
    }

    fn run(&mut self, script: &Script) -> Result<(), Error> {
        for (_, line) in &script.lines {
            match line {
                Line::On(dialog, actions) => _ = self.on.insert(*dialog, actions.clone()),
                Line::Once(dialog, actions) => {
                    self.once.entry(*dialog).or_default().push_back(actions.clone())
                }
                Line::Until(until) => self.until(*until)?,
                Line::Now(actions) => {
                    let up = self.settle()?;
                    for action in actions {
                        self.act(up, action)?;
                    }
                }
                Line::Repeat(days) => {
                    self.settle()?;
                    let today = self.day()?;
                    self.until(Until::Day(today + days))?;
                }
            }
        }
        Ok(())
    }

    /// Plays until the condition is met.
    fn until(&mut self, until: Until) -> Result<(), Error> {
        let mut turns = 0;
        let mut last = None;
        loop {
            let up = match self.settle() {
                Err(Error::Probe(_)) if until == Until::End => return Ok(()),
                result => result?,
            };
            let met = match until {
                Until::Dialog(dialog) => up.id == dialog,
                Until::Day(day) => self.day()? >= day,
                Until::End => false,
            };
            if met {
                return Ok(());
            }
            turns = if last == Some(up) { turns + 1 } else { 0 };
            if turns > MOST_TURNS {
                return Err(Error::Stuck(up.id));
            }
            last = Some(up);
            let actions = match self.once.get_mut(&up.id).and_then(VecDeque::pop_front) {
                Some(actions) => actions,
                None => match self.on.get(&up.id) {
                    Some(actions) => actions.clone(),
                    None => match self.explore.then(|| self.answer(up.id)).flatten() {
                        Some(action) => {
                            self.log.push("  (exploring)".into());
                            vec![action]
                        }
                        None => return Err(Error::NoActions(up.id)),
                    },
                },
            };
            for (n, action) in actions.iter().enumerate() {
                self.act(up, action)?;
                let gone = self.top()? != Some(up);
                if gone && n + 1 < actions.len() {
                    return Err(Error::LeftOver(up.id));
                }
                if gone {
                    break;
                }
            }
        }
    }

    /// What exploring answers a dialog with: its OK button, or its only
    /// button.
    fn answer(&self, id: u16) -> Option<Action> {
        let dialog = self.game.dialogs.get(&id)?;
        if target_of(dialog, id, &Action::Ok).is_ok() {
            return Some(Action::Ok);
        }
        let mut buttons = dialog.controls.iter().filter(|c| matches!(c.kind, Kind::Button { .. }));
        match (buttons.next(), buttons.next()) {
            (Some(button), None) => button.id.map(Action::Press),
            _ => None,
        }
    }

    /// The dialog on top, if one is up: the last one shown that runs
    /// modally or does not belong to another that is up — the panels of the
    /// week belong to the week, as the replica has them.
    fn top(&mut self) -> Result<Option<Up>, Error> {
        let reply = self.probe.ask(b"dialogs")?;
        let mut words = reply.split_whitespace();
        let shown = words.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        let hex = |word: &str| u64::from_str_radix(word.trim_start_matches("0x"), 16).ok();
        let up: Vec<(Up, u64, bool)> = words
            .filter_map(|word| {
                let parts: Vec<&str> = word.split(':').collect();
                let [id, at, flags, parent, modal] = parts[..] else { return None };
                let up = Up { id: id.parse().ok()?, at: hex(at)?, shown };
                (hex(flags)? & 0x10 != 0).then_some((up, hex(parent)?, modal == "1"))
            })
            .collect();
        let belongs = |parent: u64| up.iter().any(|(other, _, _)| other.at == parent);
        let top = up.iter().rev().find(|&&(_, parent, modal)| modal || !belongs(parent));
        // A frame shows as its page.
        let page = |frame: &Up| up.iter().rev().find(|&&(_, parent, _)| parent == frame.at);
        Ok(top.map(|&(top, _, _)| match page(&top) {
            Some(&(page, _, _)) if FRAMES.contains(&top.id) => page,
            _ => top,
        }))
    }

    /// Waits until a dialog has stayed on top for a moment, and returns it.
    fn settle(&mut self) -> Result<Up, Error> {
        let start = Instant::now();
        let mut since = (None, Instant::now());
        loop {
            let up = self.top()?;
            if up != since.0 {
                since = (up, Instant::now());
            } else if let Some(up) = up.filter(|_| since.1.elapsed() >= SETTLED) {
                if self.logged.is_none_or(|logged| logged != up) {
                    self.log.push(format!("screen {}", up.id));
                    self.logged = Some(up);
                }
                return Ok(up);
            }
            if start.elapsed() > MOST {
                return Err(Error::Stuck(up.map_or(0, |up| up.id)));
            }
            sleep(POLL);
        }
    }

    /// Today, counted from the start of the first season.
    fn day(&mut self) -> Result<usize, Error> {
        let hex = self.probe.ask(format!("peek {DAY:#x} 2").as_bytes())?;
        let day = u16::from_str_radix(&hex, 16)
            .map_err(|_| Error::Probe(format!("not a day: {hex}")))?
            .swap_bytes();
        Ok(day as i16 as usize)
    }

    /// Acts on the controls of the dialog `up`.
    fn act(&mut self, up: Up, action: &Action) -> Result<(), Error> {
        self.log.push(format!("  {action:?}"));
        if let Action::Wait = action {
            return self.wait(up);
        }
        if let Action::ClickAt((x, y)) = *action {
            self.click(Vec2::new(f32::from(x), f32::from(y)))?;
            sleep(PAUSE);
            return Ok(());
        }
        if let Action::DragAt((x0, y0), (x1, y1)) = *action {
            self.probe.ask(format!("drag {x0} {y0} {x1} {y1}").as_bytes())?;
            sleep(PAUSE);
            return Ok(());
        }
        let dialog = self.game.dialogs.get(&up.id).ok_or(Error::NoActions(up.id))?;
        let target = target_of(dialog, up.id, action)?;
        let view = View::new(up.id, dialog);
        let rect = |target: Target| {
            pointer::targets(dialog, &view, &Pixels)
                .into_iter()
                .find(|(t, _)| *t == target)
                .map(|(_, rect)| rect)
                .or_else(|| match target {
                    Target::Button(i) | Target::Control(i) | Target::Edit(i) => {
                        Some(dialog.controls[i].rect)
                    }
                    Target::Cell { control, row, column } => {
                        crate::controls::list::cell_rect(&dialog.controls[control], row, column)
                    }
                    // An option of a `HighButton` group: its rect stands
                    // inside the control's, as the classic renderer draws it.
                    Target::Row { control, row } | Target::Option { control, item: row } => {
                        let c = &dialog.controls[control];
                        let Kind::Options { items, .. } = &c.kind else { return None };
                        let it = items.get(row)?;
                        Some(Rect::from_corners(
                            c.rect.min + it.rect.min,
                            (c.rect.min + it.rect.max).min(c.rect.max),
                        ))
                    }
                    _ => None,
                })
                .ok_or_else(|| Error::Unsupported(up.id, format!("{action:?}")))
        };
        let grip = Grip::target;
        if let Target::Menu(at) = target {
            let at = rect(grip(at))?.center();
            self.probe.ask(format!("rclick {} {}", at.x as i32, at.y as i32).as_bytes())?;
            self.menu_at = Some(at);
            sleep(PAUSE);
            return Ok(());
        }
        // An entry of the menu a right click opened (dialog 415).
        if let Target::Option { control, item } = target
            && matches!(dialog.controls[control].kind, Kind::Menu)
        {
            let at =
                self.menu_at.ok_or_else(|| Error::Unsupported(up.id, format!("{action:?}")))?;
            self.click(menu::entry(at, item).center())?;
            sleep(PAUSE);
            return Ok(());
        }
        if let Target::Drop { from, to, .. } = target {
            // Off the boxes is on the list, which takes a player out as
            // well; a free spot of the pitch would move the box instead.
            let to = match to {
                Some(to) => rect(grip(to))?,
                None => dialog
                    .controls
                    .iter()
                    .find(|c| matches!(c.kind, Kind::ListBox { .. }))
                    .map(|c| c.rect)
                    .ok_or_else(|| Error::Unsupported(up.id, format!("{action:?}")))?,
            };
            let (from, to) = (rect(grip(from))?.center(), to.center());
            let command =
                format!("drag {} {} {} {}", from.x as i32, from.y as i32, to.x as i32, to.y as i32);
            self.probe.ask(command.as_bytes())?;
            sleep(PAUSE);
            return Ok(());
        }
        self.click(rect(target)?.center())?;
        if let (Action::Type(_, text), Target::Edit(field)) = (action, target) {
            let len = match dialog.controls[field].kind {
                Kind::Edit { max_len } if max_len > 0 => max_len,
                _ => FIELD_LEN,
            };
            self.probe.ask(format!("key {VK_END}").as_bytes())?;
            for _ in 0..len {
                self.probe.ask(format!("key {VK_BACK}").as_bytes())?;
            }
            let mut command = b"text ".to_vec();
            command.extend(text.chars().map(windows_1252));
            self.probe.ask(&command)?;
        }
        sleep(PAUSE);
        Ok(())
    }

    fn click(&mut self, at: Vec2) -> Result<(), Error> {
        self.probe.ask(format!("click {} {}", at.x as i32, at.y as i32).as_bytes()).map(drop)
    }

    /// Waits until the dialog `up` moves on by itself.
    fn wait(&mut self, up: Up) -> Result<(), Error> {
        let start = Instant::now();
        while self.top()? == Some(up) {
            if start.elapsed() > MOST {
                return Err(Error::Stuck(up.id));
            }
            sleep(POLL);
        }
        Ok(())
    }
}

/// A character as the original's fields take it; `?` for those Windows-1252
/// has no byte for.
fn windows_1252(c: char) -> u8 {
    match c as u32 {
        code @ (0..0x80 | 0xa0..0x100) => code as u8,
        _ => b'?',
    }
}
