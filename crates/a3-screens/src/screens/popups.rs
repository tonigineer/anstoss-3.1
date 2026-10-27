//! The small popups the game asks and tells things in: a question to answer
//! yes or no, a notice to take in, the training strip's "Sind Sie sicher?",
//! and its select of a file of saved training weeks.

use std::time::SystemTime;

use chrono::{DateTime, Local};
use a3_core::cycle::Fill;
pub use a3_core::cycle::dialogs::{
    NOTICE, PLAN_FILES, QUESTION, SAFE_LOAD, SAFE_ORIGINAL, SAFE_SAVE,
};

use crate::pointer::Target;
use crate::screens::{
    Args, Cx, Outcome, Reply, Request, Screen, control, indices, ok_or_cancel, set,
};
use crate::view::View;
use a3_files::rch::Kind;

const QUESTION_TEXT: u16 = 4738;
const BUILDER_OFFER: u16 = 4812;
const QUESTION_OK: u16 = 4741;
const NOTICE_TEXT: u16 = 4730;
/// The notice's caption (`CAlert`): "ANSTOSS 3 - Achtung!".
const NOTICE_CAPTION: u16 = 2502;
/// The ball of the question, playing its frames forth and back.
const QUESTION_BALL: u16 = 4740;
const QUESTION_BALL_FRAMES: std::ops::RangeInclusive<u16> = 1745..=1753;
/// The ball of the other small popups, looping.
const BALLS: [u16; 3] = [1336, 2116, 4732];
const BALL_FRAMES: std::ops::RangeInclusive<u16> = 1762..=1773;
/// The words of the strip's second "Sind Sie sicher?" (dialog 242).
const SAFE_LOAD_TEXT: u16 = 3378;
/// Reading the season's plan asks in other words; its other titles stand in
/// the window title, which the replica does not draw.
const PLAN_LOAD_ASK: u16 = 3669;
/// The file select's list, and the name typed or picked in it.
const FILE_LIST: u16 = 5261;
const FILE_NAME: u16 = 6361;
/// The extension of a file of the manager's training weeks (".Woc").
const WEEK_EXT: u16 = 3470;
/// The select's own notice when the file would not open: written to, read
/// from ("Fehler beim Abspeichern/Einlesen der Daten!").
const SAVE_FAILED: u16 = 3214;
const LOAD_FAILED: u16 = 3213;

/// A question: OK answers yes, the other button no.
pub struct Question;

impl Screen for Question {
    fn ids(&self) -> &[u16] {
        &[QUESTION]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let text = match args {
            Args::Text(text) => Some(cx.game.text(text).to_owned()),
            Args::TextFor(text, player) => {
                let name = cx.state.records.as_ref().and_then(|r| r.known_as(player));
                Some(cx.game.text(text).replacen("%s", &name.unwrap_or_default(), 1))
            }
            Args::TextForMoney(text, amount) => {
                let amount = super::playerinfo::money(cx.game, amount);
                Some(cx.game.text(text).replacen("%s", &amount, 1))
            }
            Args::TextForText(text, other) => {
                Some(cx.game.text(text).replacen("%s", cx.game.text(other), 1))
            }
            Args::Printf(text, fills) => Some(printed(cx, text, fills)),
            Args::ForClub { text, country, number, value } => {
                Some(super::contacts::club_text(cx, text, country, number, value))
            }
            Args::BuilderOffer { price, weeks, total } => {
                let money = |v: i32| super::playerinfo::money(cx.game, v.into());
                Some(super::finances::printf(
                    cx.game.text(BUILDER_OFFER),
                    &[&money(price), &weeks, &(weeks - weeks / 5), &money(total)],
                ))
            }
            Args::TextForFormation(text, formation) => {
                let manager = cx.state.season.as_ref().and_then(|s| s.manager());
                let name = manager.and_then(|m| m.formations.get(formation)).map(|f| f.0.as_str());
                Some(cx.game.text(text).replacen("%s", name.unwrap_or_default(), 1))
            }
            Args::KeepFormation(formation) => {
                let manager = cx.state.season.as_ref().and_then(|s| s.manager());
                let name = manager.and_then(|m| m.formations.get(formation)).map(|f| f.0.as_str());
                let question = &cx.game.rules.pitch.keep_question;
                Some(question.replacen("%s", name.unwrap_or_default(), 1))
            }
            _ => None,
        };
        if let Some(text) = text {
            set(view, cx.dialog, QUESTION_TEXT, text);
        }
        if let Some(ball) = control(cx.dialog, QUESTION_BALL).and_then(|b| view.anim_mut(b)) {
            ball.frames = question_frames();
            ball.playing = true;
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let Target::Button(button) = target else { return Outcome::Stay };
        let ok = cx.dialog.controls[button].id == Some(QUESTION_OK);
        Outcome::Close(if ok { Reply::Ok } else { Reply::Cancel })
    }
}

/// A notice with a button to take it in.
pub struct Notice;

impl Screen for Notice {
    fn ids(&self) -> &[u16] {
        &[NOTICE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        ball(view, cx);
        let caption = cx.game.text(NOTICE_CAPTION).to_owned();
        set(view, cx.dialog, super::discipline::CAPTION_ID, caption);
        let text = match args {
            Args::Text(text) => cx.game.text(text).to_owned(),
            Args::Message(at) => cx.game.rules.messages.get(&at).cloned().unwrap_or_default(),
            Args::TextFor(text, player) => {
                let name = cx.state.records.as_ref().and_then(|r| r.known_as(player));
                cx.game.text(text).replacen("%s", &name.unwrap_or_default(), 1)
            }
            Args::TextForDate(text, day) => {
                let year = cx.state.season.as_ref().map_or(0, |s| s.year);
                let date = super::hub::date_text(cx.game, a3_core::calendar::date(year, day));
                cx.game.text(text).replacen("%s", &date, 1)
            }
            Args::TextForMoney(text, amount) => {
                let amount = super::playerinfo::money(cx.game, amount);
                cx.game.text(text).replacen("%s", &amount, 1)
            }
            Args::TextForNumber(text, n) => super::finances::printf(cx.game.text(text), &[&n]),
            Args::TextForClub(text, country, number) => {
                let name = cx.game.club_name(cx.state.records.as_ref(), country, number);
                cx.game.text(text).replacen("%s", &name, 1)
            }
            Args::Printf(text, fills) => printed(cx, text, fills),
            Args::Cooperation { partner, year } => super::contacts::running_text(cx, partner, year),
            _ => return,
        };
        set(view, cx.dialog, NOTICE_TEXT, text);
    }

    fn input(&mut self, _: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(_) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// The training strip's "Sind Sie sicher?", whether it writes, reads or puts
/// back its weeks (dialogs 241 to 243): OK lets it happen, the other button
/// leaves it be. Its ball plays the question's frames.
pub struct Safe;

impl Screen for Safe {
    fn ids(&self) -> &[u16] {
        &[SAFE_SAVE, SAFE_LOAD, SAFE_ORIGINAL]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        // Only reading the season's plan asks in other words; the manager's
        // own weeks and plan keep what the dialog says (`FUN_0059ffc1`'s
        // mode, which for 241 and 242 only changes the window title too).
        if matches!(args, Args::PlanSafe { plan: true }) && view.id == SAFE_LOAD {
            set(view, cx.dialog, SAFE_LOAD_TEXT, cx.game.text(PLAN_LOAD_ASK).to_owned());
        }
        for ball in indices(cx.dialog, |k| matches!(k, Kind::Anim)) {
            if let Some(ball) = view.anim_mut(ball) {
                ball.frames = question_frames();
                ball.playing = true;
            }
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        match (ok, cancel) {
            (true, _) => Outcome::Close(Reply::Ok),
            (_, true) => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }
}

/// The training strip's select of a file (`CFileSelect`, dialog 506): it
/// lists the game's `plan` folder and writes or reads the file itself, as
/// the game's does — the request that opens it only says whether to save.
#[derive(Default)]
pub struct Files {
    save: bool,
}

impl Screen for Files {
    fn ids(&self) -> &[u16] {
        &[PLAN_FILES]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let Args::PlanFiles { save } = args else { return };
        self.save = save;
        let game = cx.game;
        let rows: Vec<String> = plan_files(&game.dir, game.text(WEEK_EXT))
            .iter()
            .map(|(name, when)| {
                let when = DateTime::<Local>::from(*when);
                format!("{}\t{}\t{}", name, when.format("%d.%m.%Y"), when.format("%H:%M"))
            })
            .collect();
        if let Some(list) = control(cx.dialog, FILE_LIST).and_then(|c| view.list_mut(c)) {
            // No row stands out at first; only a click picks one.
            list.selected = None;
            list.scroll = 0;
            list.rows = rows;
        }
        // Saving types the name in; reading takes it from the list, into an
        // edit that shows nothing.
        if let Some(c) = control(cx.dialog, FILE_NAME) {
            view.controls[c].hidden = !save;
            if let Some(edit) = view.controls[c].state.as_edit_mut() {
                edit.disabled = !save;
                edit.text = String::new();
            }
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        match target {
            // A row's first column is the name; it goes into the edit, which
            // is where reading looks for it (`CFileSelect::OnSelchange`).
            Target::Row { control: c, row } if !self.save => {
                let name = view
                    .list(c)
                    .and_then(|list| list.rows.get(row))
                    .and_then(|row| row.split('\t').next())
                    .unwrap_or_default()
                    .to_owned();
                if let Some(edit) =
                    control(cx.dialog, FILE_NAME).and_then(|c| view.controls[c].state.as_edit_mut())
                {
                    edit.text = name;
                }
            }
            _ if ok => return self.take(cx, view),
            _ if cancel => return Outcome::Close(Reply::Cancel),
            _ => {}
        }
        Outcome::Stay
    }
}

impl Files {
    /// The name in the edit, with the extension appended when it is not
    /// there yet (the game compares without minding the case); the file
    /// under `plan`, written or read. What would not open shows as the
    /// select's own notice, and the select stays up for another try.
    fn take(&self, cx: &mut Cx, view: &mut View) -> Outcome {
        let game = cx.game;
        let ext = game.text(WEEK_EXT);
        let mut name = control(cx.dialog, FILE_NAME)
            .and_then(|c| view.controls[c].state.as_edit())
            .map_or(String::new(), |edit| edit.text.clone());
        if !name.to_lowercase().ends_with(&ext.to_lowercase()) {
            name.push_str(ext);
        }
        let path = game.dir.join("plan").join(&name);
        let done = if self.save { write_weeks(cx, &path) } else { read_weeks(cx, &path) };
        if done {
            return Outcome::Close(Reply::Ok);
        }
        let text = if self.save { SAVE_FAILED } else { LOAD_FAILED };
        Outcome::Popup(Request::with(NOTICE, Args::Text(text)))
    }
}

/// The `plan` folder's files ending in the extension, by name: the select
/// lists each with the date and time the directory gives it.
pub(crate) fn plan_files(dir: &std::path::Path, ext: &str) -> Vec<(String, SystemTime)> {
    let mut files: Vec<(String, SystemTime)> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let when = entry.metadata().ok()?.modified().ok()?;
                    name.to_lowercase().ends_with(&ext.to_lowercase()).then(|| (name, when))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// The manager's 18 training weeks to the file, which the game writes as one
/// block, exactly as it keeps them (`FUN_0059f989`).
fn write_weeks(cx: &mut Cx, path: &std::path::Path) -> bool {
    let Some(templates) =
        cx.state.season.as_ref().and_then(|s| s.manager()).map(|m| m.templates.clone())
    else {
        return false;
    };
    std::fs::write(path, a3_core::training::week_file(&templates)).is_ok()
}

/// The 18 weeks of a `plan\*.Woc` file, into the manager and his record with
/// it: the game reads the block whole and sets every unit above the last one
/// to it (`FUN_0059fb37`).
fn read_weeks(cx: &mut Cx, path: &std::path::Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else { return false };
    let Some(block) = a3_core::training::read_week_file(&bytes) else { return false };
    let templates = a3_core::training::week_templates(&block);
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return false };
    let current = season.current;
    if let Some(manager) = season.managers.get_mut(current) {
        manager.templates = templates;
    }
    if let Some(records) = state.records.as_mut()
        && let Some(slot) = records.human_slot(current)
        && let Some(record) = records.managers.get_mut(slot)
    {
        record.set_templates(&block);
    }
    true
}

/// A text with what its `%`s stand for.
fn printed(cx: &Cx, text: u16, fills: [Fill; 2]) -> String {
    let parts: Vec<String> = fills
        .iter()
        .filter_map(|&fill| match fill {
            Fill::None => None,
            Fill::Number(n) => Some(n.to_string()),
            Fill::Money64(v) => Some(super::finances::money64(cx.game, v)),
            Fill::Money(v) => Some(super::playerinfo::money(cx.game, v)),
        })
        .collect();
    let args: Vec<&dyn std::fmt::Display> = parts.iter().map(|p| p as _).collect();
    super::finances::printf(cx.game.text(text), &args)
}

/// The frames of a question's ball, played forth and back.
fn question_frames() -> Vec<u16> {
    let mut frames: Vec<u16> =
        QUESTION_BALL_FRAMES.chain(QUESTION_BALL_FRAMES.rev().skip(1)).collect();
    frames.pop();
    frames
}

/// The looping ball of a small popup, if it has one.
pub(crate) fn ball(view: &mut View, cx: &Cx) {
    for id in BALLS {
        if let Some(ball) = control(cx.dialog, id).and_then(|b| view.anim_mut(b)) {
            ball.frames = BALL_FRAMES.collect();
            ball.playing = true;
        }
    }
}
