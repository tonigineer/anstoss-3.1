//! The short-term training, the menu bar's fifth symbol (dialog 346,
//! `CTrainingKurzfristig`; TRAINING-KURZ.md in the RE repo): the week's
//! sessions, three a day, a unit each, chosen one by one in a popup (362)
//! or for the rest of the week from one of the manager's templates; the
//! player in individual training; a training camp and a team meeting
//! before the match; the team's condition, freshness and offside trap and
//! what it can at set pieces, beside the next opponent's as far as the
//! club knows them. The same as pictures (dialog 513, `CTrainingKurzGrafik`),
//! which "Grafik" and "Text" switch between.

pub mod editor;
pub mod individual;
pub mod long;
pub mod performance;
pub mod rooms;
pub mod settings;

use std::collections::HashMap;

use bevy_math::Rect;

use a3_core::cycle::dialogs::NOTICE;
use a3_core::lineup;
use a3_core::records::{Records, Team};
use a3_core::season::{ClubState, Rules};
use a3_core::training::{
    self, ANTI_OFFSIDE, CAMP, CORNERS, DAYS, FREE_KICKS, MEETING, PRESSING, SESSIONS, SKILLS,
    THROW_INS, TRICKS, Training, UNKNOWN, Week,
};
use a3_files::rch::{Align, Control, Dialog, Kind, PLAIN_FONT};

use super::duties::short_name;
use super::hub::{self, SHORT_TRAINING, SHORT_TRAINING_SYMBOL, TRAINING_PICTURES, UNIT_CHOICE};
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::controls::bitmap::Picture;
use crate::controls::button::Label;
use crate::controls::options::NONE;
use crate::pointer::{Grip, Target};
use crate::view::View;

/// Controls by their ids in the dialog file: the day's sessions, a day
/// after the other; the templates; the player in individual training.
const FIRST_CELL: u16 = 3831;
const CELLS: u16 = (DAYS * SESSIONS) as u16;
const TEMPLATES: u16 = 5035;
const INDIVIDUAL: u16 = 4427;
const CAMP_BOX: u16 = 4425;
const MEETING_BOX: u16 = 4441;
/// The opponent: his crest and name, which switch to the week's next
/// opponent.
const CREST: u16 = 4440;
const OPPONENT: u16 = 4426;
/// Condition, freshness and the offside trap: the own, the opponent's.
const OWN_FIGURES: [u16; 3] = [4419, 4420, 4421];
const THEIR_FIGURES: [u16; 3] = [4422, 4423, 4424];
/// What each team can, the own and the opponent's, in the order of
/// [`TRICKS`] ….
const SKILL_WORDS: [(u16, u16); SKILLS] = {
    let mut words = [(0, 0); SKILLS];
    words[TRICKS] = (4434, 4435);
    words[PRESSING] = (4430, 4431);
    words[FREE_KICKS] = (4436, 4437);
    words[THROW_INS] = (4432, 4433);
    words[CORNERS] = (4438, 4439);
    words[ANTI_OFFSIDE] = (4428, 4429);
    words
};
/// "Grafik" and "Text": the week as pictures, as text.
const SWITCH: u16 = 6480;
/// The picture view's texts on its cells.
const CELL_INK: image::Rgba<u8> = image::Rgba([255, 255, 255, 255]);
/// The OK, whose place the bar takes.
const OK: u16 = 1;
const CAPTION_ID: u16 = 9926;
const UNIT_CAPTION_ID: u16 = 9927;

/// Texts of the game.
const CAPTION: u16 = 3653;
const NOBODY: u16 = 1091;
const MATCH: u16 = 3215;
const RECOVERY: u16 = 4943;
const GONE: u16 = 4944;

/// Days of a year calendar: a match, the club's party.
const MATCH_DAY: u8 = 6;
const PARTY: u8 = 7;
/// The last day a calendar holds.
const LAST_DAY: usize = 363;
/// Players out for longer than this are not trained alone.
const MOST_DAYS_OUT: i32 = 7;
/// A national team's own: its country number.
const NATIONAL: u8 = 200;

/// The popup's units: their ids, from this on, a hundred a list.
const FIRST_UNIT: u16 = 35000;
const UNIT_LIST: u16 = 100;
const UNIT_SIZE: (f32, f32) = (100.0, 13.0);
const UNIT_ROW: f32 = 12.0;
const ABORT: u16 = 3882;

/// The bar takes the OK's place; the caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    for id in [SHORT_TRAINING, TRAINING_PICTURES] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        dialog.controls.retain(|c| c.id != Some(OK));
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
        hub::add_bar(dialog, rules);
    }
    // The popup's units, as it makes them as it opens (`FUN_005a1ffa`):
    // centred statics of 100×13 down each list.
    if let Some(dialog) = dialogs.get_mut(&UNIT_CHOICE) {
        let caption = super::caption(dialog.frame, UNIT_CAPTION_ID, dialog.title.clone());
        dialog.controls.insert(0, caption);
        let origin = dialog.frame.map_or(bevy_math::Vec2::ZERO, |f| f.min);
        for (c, category) in rules.training.categories.iter().enumerate() {
            let top =
                |i: usize| origin.y + category.at.1 as f32 + UNIT_ROW * i as f32 + (i / 2) as f32;
            let count = category.units.len();
            for i in 0..count {
                let x = origin.x + category.at.0 as f32;
                // The next unit's window lies over this one's last line
                // where they meet.
                let next = if i + 1 < count { top(i + 1) } else { f32::MAX };
                let bottom = (top(i) + UNIT_SIZE.1).min(next);
                dialog.controls.push(Control {
                    kind: Kind::Static,
                    rect: Rect::new(x, top(i), x + UNIT_SIZE.0, bottom),
                    align: Align::Centre,
                    text: String::new(),
                    id: Some(FIRST_UNIT + c as u16 * UNIT_LIST + i as u16),
                    font: PLAIN_FONT,
                    ink: [0; 3],
                });
            }
        }
    }
}

#[derive(Default)]
pub struct ShortTraining {
    /// The week's opponents from today on, and the one shown; how much the
    /// club knows of the first.
    opponents: Vec<(usize, usize)>,
    shown: usize,
    level: usize,
    /// The players the individual training lists after "keiner", by id.
    players: Vec<u16>,
    /// The sessions a click opens the popup for, and the one it is open
    /// for.
    open: [[bool; SESSIONS]; DAYS],
    choosing: Option<(usize, usize)>,
    /// Whether the templates' editor is open.
    editing: bool,
    /// Whether the whistle asks.
    whistle: bool,
}

impl Screen for ShortTraining {
    fn ids(&self) -> &[u16] {
        &[SHORT_TRAINING, TRAINING_PICTURES]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        hub::open_bar(cx, view, SHORT_TRAINING_SYMBOL);
        let Some(season) = cx.state.season.as_ref() else { return };
        set(view, dialog, CAPTION_ID, hub::manager_title(game, season, CAPTION));
        let Some(club) = own(cx) else { return };
        let records = cx.state.records.as_ref();
        let own_team = records.and_then(|r| team(cx, r, own_key(cx)?));

        self.opponents = opponents(cx);
        self.shown = 0;
        self.level = match self.opponents.first() {
            None => 0,
            Some(&opponent) => {
                let secret = records.and_then(|r| team(cx, r, opponent)).is_some_and(secret);
                let cost = own_team.map_or(0, |t| t.observation_cost());
                let national = own_team.is_some_and(|t| t.number() == NATIONAL);
                training::level(cost, national, secret)
            }
        };
        self.show_week(cx, view);
        self.fill_players(cx, view);

        for (&id, value) in OWN_FIGURES.iter().zip(figures(club)) {
            set(view, dialog, id, value.to_string());
        }
        let skills = own_team.map_or([0; SKILLS], training::skills);
        for (&(id, _), value) in SKILL_WORDS.iter().zip(skills) {
            show_skill(cx, view, id, value);
        }
        self.show_opponent(cx, view);
        for (id, flag) in [(CAMP_BOX, CAMP), (MEETING_BOX, MEETING)] {
            let Some(c) = control(dialog, id) else { continue };
            let on = club.training.has(flag);
            if let Some(check) = view.check_mut(c) {
                check.checked = on;
            } else if let Some(button) = view.controls[c].state.as_button_mut() {
                button.step = usize::from(on);
            }
        }
        show_template_names(cx, view);

        let works = |target: Target| hub::on_bar(dialog, target).is_none_or(hub::bar_works);
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        if let Some(item) = hub::on_bar(dialog, target) {
            let outcome = hub::bar_click(cx, view, item, SHORT_TRAINING_SYMBOL);
            if matches!(outcome, Outcome::Close(_)) {
                keep_boxes(cx, view);
            }
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        match target {
            Target::Control(c) | Target::Button(c)
                if (FIRST_CELL..FIRST_CELL + CELLS).contains(&id(c)) =>
            {
                let cell = usize::from(id(c) - FIRST_CELL);
                let (day, session) = (cell / SESSIONS, cell % SESSIONS);
                if !self.open[day][session] {
                    return Outcome::Stay;
                }
                if day < today(cx) % DAYS {
                    return Outcome::Popup(Request::with(NOTICE, Args::Text(GONE)));
                }
                self.choosing = Some((day, session));
                return Outcome::Popup(Request::new(UNIT_CHOICE));
            }
            // The camp and the meeting as pictures: a click switches.
            Target::Button(c) | Target::Menu(Grip::Control(c))
                if [CAMP_BOX, MEETING_BOX].contains(&id(c)) =>
            {
                if let Some(button) = view.controls[c].state.as_button_mut() {
                    button.step_on(2, matches!(target, Target::Menu(_)));
                }
            }
            Target::Button(c) if id(c) == SWITCH => {
                keep_boxes(cx, view);
                let pictures = view.id == SHORT_TRAINING;
                switch_view(cx, pictures);
                return Outcome::Close(Reply::Goto(SHORT_TRAINING));
            }
            Target::Control(c) if [CREST, OPPONENT].contains(&id(c)) => {
                if self.opponents.len() > 1 {
                    self.shown = (self.shown + 1) % self.opponents.len();
                    self.show_opponent(cx, view);
                }
            }
            // A right click on a template edits it (`OnRButtonDown_0059a06d`).
            Target::Menu(Grip::Option { control: c, item }) if id(c) == TEMPLATES => {
                self.editing = true;
                return Outcome::Popup(Request::with(editor::TEMPLATE_EDITOR, Args::Template(item)));
            }
            Target::Option { control: c, item } if id(c) == TEMPLATES => {
                let (week, day) = (today(cx) / DAYS, today(cx) % DAYS);
                let template = cx
                    .state
                    .season
                    .as_ref()
                    .and_then(|s| s.manager())
                    .and_then(|m| m.templates.get(item))
                    .map(|t| t.week);
                change_week(cx, week, |w| w.template = item as i16 + 1);
                if let Some(sessions) = template {
                    change(cx, |t| t.apply(0, &sessions, day));
                }
                view.set_chosen(c, NONE);
                self.show_week(cx, view);
            }
            Target::Row { control: c, .. } if id(c) == INDIVIDUAL => {
                let row = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0);
                if let Some(&player) = row.checked_sub(1).and_then(|r| self.players.get(r)) {
                    change_week(cx, today(cx) / DAYS, |w| w.player = player);
                }
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// The popup's unit, or the whistle's question: a yes ends the turn.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        // The labels anew after the editor (`FUN_00599b39`).
        if std::mem::take(&mut self.editing) {
            show_template_names(cx, view);
            return Outcome::Stay;
        }
        if let Some((day, session)) = self.choosing.take() {
            if let Reply::Choice(unit) = reply {
                change(cx, |t| t.set_unit(day, session, unit as u8));
                self.show_week(cx, view);
            }
            return Outcome::Stay;
        }
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            keep_boxes(cx, view);
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}

impl ShortTraining {
    /// The week's sessions (`FUN_00598c79`): a match day's last says
    /// "Spiel", a party's next day's first "Erholung", the others their
    /// units, which a click changes.
    fn show_week(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(club) = own(cx) else { return };
        let start = today(cx) - today(cx) % DAYS;
        let kind = |day: Option<usize>| day.map_or(0, |d| day_kind(cx, d));
        self.open = Default::default();
        for day in 0..DAYS {
            let mut texts: [Option<String>; SESSIONS] = Default::default();
            let mut first = 0;
            if kind((start + day).checked_sub(1)) == PARTY {
                texts[0] = Some(game.text(RECOVERY).to_owned());
                first = 1;
            }
            if kind(Some(start + day)) == MATCH_DAY {
                if first == 0 {
                    texts[0] = Some(String::new());
                }
                texts[1] = Some(String::new());
                texts[2] = Some(game.text(MATCH).to_owned());
            } else {
                for (session, text) in texts.iter_mut().enumerate().skip(first) {
                    let unit = club.training.unit(day, session);
                    *text = Some(game.text(game.rules.training.unit_text(unit)).to_owned());
                    self.open[day][session] = true;
                }
            }
            for (session, text) in texts.into_iter().enumerate() {
                let id = FIRST_CELL + (day * SESSIONS + session) as u16;
                let Some(c) = control(dialog, id) else { continue };
                let open = self.open[day][session];
                if let Some(button) = view.controls[c].state.as_button_mut() {
                    // A unit's picture; a match day's last, white and in the
                    // middle, "Spiel"; a party's next day's first, white,
                    // "Erholung" — the game puts the middle on the last
                    // (`FUN_0059ca7c`).
                    let unit = club.training.unit(day, session);
                    let picture = if open { game.rules.training.unit_picture(unit) } else { 0 };
                    button.images = Some((picture, picture));
                    button.ready = vec![picture];
                    button.disabled = !open;
                    button.label = text.filter(|t| !open && !t.is_empty()).map(|text| Label {
                        text,
                        ink: CELL_INK,
                        centred: session == SESSIONS - 1,
                    });
                    continue;
                }
                if let Some(text) = text {
                    view.set_text(c, text);
                }
                view.set_clickable(c, open);
            }
        }
    }

    /// The combo of the player to single out: "keiner", then the squad as
    /// [`pickable`] lists it, the one chosen for this week selected.
    fn fill_players(&mut self, cx: &Cx, view: &mut View) {
        let Some(c) = control(cx.dialog, INDIVIDUAL) else { return };
        let rows = pickable(cx);
        if rows.is_empty() {
            return;
        }
        self.players = rows.iter().skip(1).map(|&(id, _)| id).collect();
        let chosen = cx
            .state
            .season
            .as_ref()
            .and_then(|s| s.manager())
            .and_then(|m| m.weeks.get(today(cx) / DAYS))
            .map_or(0, |w| w.player);
        let selected = self.players.iter().position(|&id| id != 0 && id == chosen);
        if let Some(combo) = view.combo_mut(c) {
            combo.items = rows.into_iter().map(|(_, row)| row).collect();
            combo.selected = Some(selected.map_or(0, |row| row + 1));
        }
    }

    /// The opponent shown: his name and crest, his figures, and what the
    /// club knows of his set pieces (`FUN_0059957a`).
    fn show_opponent(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(&key) = self.opponents.get(self.shown) else {
            for id in THEIR_FIGURES.into_iter().chain([OPPONENT]) {
                set(view, dialog, id, String::new());
            }
            // The picture view leaves the opponent's pictures as they are.
            for (_, id) in SKILL_WORDS {
                if view.id == SHORT_TRAINING {
                    set(view, dialog, id, String::new());
                }
            }
            return;
        };
        let records = cx.state.records.as_ref();
        let their_team = records.and_then(|r| team(cx, r, key));
        let name = their_team.map(Team::name).or_else(|| {
            game.world.country(key.0).and_then(|c| c.clubs.get(key.1)).map(|c| c.name.clone())
        });
        set(view, dialog, OPPONENT, name.unwrap_or_default());
        let theirs = match cx.state.season.as_ref().and_then(|s| s.clubs.get(&key)) {
            Some(club) => figures(club),
            None => records
                .and_then(|r| a3_core::load::club(r, &game.data, key))
                .map_or([0; 3], |club| figures(&club)),
        };
        for (&id, value) in THEIR_FIGURES.iter().zip(theirs) {
            set(view, dialog, id, value.to_string());
        }
        let skills = their_team.map_or([0; SKILLS], training::skills);
        let luck =
            their_team.zip(records).map_or(0, |(t, r)| r.luck.of(u16::from(t.number())) as usize);
        let tables = &game.rules.training;
        for (skill, (&(_, id), value)) in SKILL_WORDS.iter().zip(skills).enumerate() {
            let value = if tables.known(self.level, luck, skill) { value } else { UNKNOWN };
            show_skill(cx, view, id, value);
        }
        if let Some(c) = control(dialog, CREST) {
            let crest = game.rules.crest(&game.world, cx.state.records.as_ref(), key);
            view.set_picture(c, crest.map(Picture::file));
        }
        if let Some(c) = control(dialog, OPPONENT) {
            view.set_clickable(c, self.opponents.len() > 1);
        }
    }
}

/// The templates' names on their buttons, none chosen.
fn show_template_names(cx: &Cx, view: &mut View) {
    let Some(c) = control(cx.dialog, TEMPLATES) else { return };
    let names = cx.state.season.as_ref().and_then(|s| s.manager()).map(|m| &m.templates);
    if let Some(options) = view.options_mut(c) {
        for (i, label) in options.labels.iter_mut().enumerate() {
            *label = names.and_then(|t| t.get(i)).map_or_else(String::new, |t| t.name.clone());
        }
    }
    view.set_chosen(c, NONE);
}

/// The popup of the units (dialog 362, `CTrainingseinheitAuswahl`): seven
/// lists of them; a click chooses one, "Abbruch" none.
pub struct UnitChoice;

impl Screen for UnitChoice {
    fn ids(&self) -> &[u16] {
        &[UNIT_CHOICE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        for (c, category) in game.rules.training.categories.iter().enumerate() {
            for (i, &(text_unit, _)) in category.units.iter().enumerate() {
                let id = FIRST_UNIT + c as u16 * UNIT_LIST + i as u16;
                let Some(control) = control(dialog, id) else { continue };
                view.set_text(control, game.text(game.rules.training.unit_text(text_unit)));
                view.set_clickable(control, true);
            }
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if dialog.controls[c].id == Some(ABORT) => {
                Outcome::Close(Reply::Cancel)
            }
            Target::Control(c) => {
                let Some(id) = dialog.controls[c].id.and_then(|id| id.checked_sub(FIRST_UNIT))
                else {
                    return Outcome::Stay;
                };
                let (c, i) = (usize::from(id / UNIT_LIST), usize::from(id % UNIT_LIST));
                let categories = &cx.game.rules.training.categories;
                match categories.get(c).and_then(|c| c.units.get(i)) {
                    Some(&(_, unit)) => Outcome::Close(Reply::Choice(usize::from(unit))),
                    None => Outcome::Stay,
                }
            }
            _ => Outcome::Stay,
        }
    }
}

/// What a team can of a skill: its word, or in the picture view its ball
/// (`FUN_005994d8`, `FUN_0059d525`).
fn show_skill(cx: &Cx, view: &mut View, id: u16, value: u8) {
    let (game, dialog) = (cx.game, cx.dialog);
    let Some(c) = control(dialog, id) else { return };
    let tables = &game.rules.training;
    if view.id == TRAINING_PICTURES {
        view.set_picture(c, Some(Picture::graphic(tables.ball(value))));
    } else {
        view.set_text(c, game.text(tables.word(value)));
    }
}

/// Switches the current manager's view of the training, and his record
/// with it (`FUN_006a5cfb`).
fn switch_view(cx: &mut Cx, pictures: bool) {
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let current = season.current;
    let Some(manager) = season.managers.get_mut(current) else { return };
    manager.training_pictures = pictures;
    if let Some(records) = state.records.as_mut()
        && let Some(slot) = records.human_slot(current)
        && let Some(record) = records.managers.get_mut(slot)
    {
        record.set_training_pictures(pictures);
    }
}

/// Today, counted from the first season's start.
fn today(cx: &Cx) -> usize {
    cx.state.season.as_ref().map_or(0, |s| s.day)
}

/// What a day of the club's year calendar is: a match, a party … (0
/// nothing); without a save, the club's next match.
fn day_kind(cx: &Cx, day: usize) -> u8 {
    if let Some(records) = cx.state.records.as_ref() {
        let calendar = own_key(cx)
            .and_then(|key| team(cx, records, key))
            .and_then(|t| usize::from(t.human()).checked_sub(1))
            .and_then(|slot| records.calendars.get(slot));
        return calendar.map_or(0, |c| c.day(day).kind);
    }
    let (game, season) = (cx.game, cx.state.season.as_ref());
    let next =
        season.zip(own_key(cx)).and_then(|(s, key)| s.next_match(&game.world, &game.calendar, key));
    if next.is_some_and(|m| m.day == day) { MATCH_DAY } else { 0 }
}

/// The week's opponents from today on, in the order of their days
/// (`FUN_0062b360`, which asks the fixtures; the year calendar holds the
/// same matches); none without a next match.
fn opponents(cx: &Cx) -> Vec<(usize, usize)> {
    let Some(key) = own_key(cx) else { return Vec::new() };
    let game = cx.game;
    let Some(records) = cx.state.records.as_ref() else {
        let season = cx.state.season.as_ref();
        let next = season.and_then(|s| s.next_match(&game.world, &game.calendar, key));
        let start = today(cx) - today(cx) % DAYS;
        return next
            .filter(|m| m.day < start + DAYS)
            .map(|m| if m.home == key { m.away } else { m.home })
            .into_iter()
            .collect();
    };
    let Some(own) = team(cx, records, key) else { return Vec::new() };
    if own.next_country() == 0 {
        return Vec::new();
    }
    let calendar = usize::from(own.human()).checked_sub(1).and_then(|s| records.calendars.get(s));
    let start = today(cx) - today(cx) % DAYS;
    let own_id = (u16::from(own.country()), i16::from(own.number()));
    let mut found: Vec<(usize, usize)> = (today(cx)..start + DAYS)
        .filter(|&day| day <= LAST_DAY)
        .filter(|&day| calendar.is_some_and(|c| c.day(day).kind == MATCH_DAY))
        .filter_map(|day| {
            let teams = calendar?.teams(day)?;
            let (country, number) = if teams[0] == own_id { teams[1] } else { teams[0] };
            Some((game.world.slot_of(country)?, usize::try_from(number).ok()?))
        })
        .collect();
    if found.is_empty() {
        let next = game.world.slot_of(own.next_country().into());
        let number = usize::try_from(own.next_number()).ok();
        found.extend(next.zip(number));
    }
    found
}

/// Whether a team trains in secret, which hides it from its opponents.
fn secret(team: &Team) -> bool {
    const SECRET: u16 = 0x8000;
    team.passing() & SECRET != 0
}

/// The manager's club, as the season follows it, and its key.
fn own<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

fn own_key(cx: &Cx) -> Option<(usize, usize)> {
    cx.state.season.as_ref()?.manager()?.club
}

/// The players a week may single out: "keiner" first, then the club's own
/// who are out for a week at most, the least in form first — dialog 346's
/// combo and dialog 281's list the same (`FUN_005979f1`, `FUN_005a03fe`).
/// A row is the player's id, 0 for "keiner", and its text.
pub(super) fn pickable(cx: &Cx) -> Vec<(u16, String)> {
    let Some(club) = own(cx) else { return Vec::new() };
    let mut squad: Vec<(i32, usize)> = (0..club.players.len())
        .filter(|&p| club.status.get(p).is_none_or(|s| s.injured <= MOST_DAYS_OUT))
        .map(|p| (lineup::form(lineup::fitness_of(&club.fitness, p)), p))
        .collect();
    squad.sort_by_key(|&(form, _)| form);
    let pitch = &cx.game.rules.pitch;
    let mut rows = vec![(0, cx.game.text(NOBODY).to_owned())];
    for &(form, p) in &squad {
        let player = &club.players[p];
        let id = club.status.get(p).map_or(0, |s| s.id);
        let kind = pitch.abbreviations.get(usize::from(player.positions[0])).copied();
        let position = kind.map_or("", |k| cx.game.text(k as u16));
        rows.push((id, format!("{}\t{position}\t{form}", short_name(cx, player, id))));
    }
    rows
}

/// A team's condition, freshness and offside trap, as the screen shows
/// them (`FUN_00676f16`, `FUN_006771b8`).
fn figures(club: &ClubState) -> [i32; 3] {
    [
        lineup::average(club, lineup::condition),
        lineup::average(club, lineup::freshness),
        i32::from(club.tactics.offside / 10),
    ]
}

/// A club's team record by its key.
fn team<'a>(cx: &Cx, records: &'a Records, (slot, club): (usize, usize)) -> Option<&'a Team> {
    let nation = cx.game.world.country(slot)?.nation;
    records.team(nation, u16::try_from(club).ok()?)
}

/// The check boxes into the club's flags, as the screen is left
/// (`FUN_005992e6`).
fn keep_boxes(cx: &mut Cx, view: &View) {
    let dialog = cx.dialog;
    let ticked = |id: u16| {
        let Some(c) = control(dialog, id) else { return false };
        match view.check(c) {
            Some(check) => check.checked,
            None => view.controls[c].state.as_button().is_some_and(|b| b.step != 0),
        }
    };
    let (camp, meeting) = (ticked(CAMP_BOX), ticked(MEETING_BOX));
    change(cx, |t| {
        t.set(CAMP, camp);
        t.set(MEETING, meeting);
    });
}

/// Changes the club's training, and its records with it: the plan in its
/// extra data, the flags in its team.
pub(super) fn change(cx: &mut Cx, change: impl FnOnce(&mut Training)) {
    let game = cx.game;
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get_mut(&key) else { return };
    change(&mut club.training);
    let training = club.training;
    let Some(records) = state.records.as_mut() else { return };
    let Some(nation) = game.world.country(key.0).map(|c| c.nation) else { return };
    let Some(team) = u16::try_from(key.1).ok().and_then(|n| records.team_mut(nation, n)) else {
        return;
    };
    team.set_training_flags(training.flags);
    let human = usize::from(team.human()).checked_sub(1);
    if let Some(extra) = human.and_then(|slot| records.extras.get_mut(slot)) {
        extra.set_plan(training.plan());
    }
}

/// Changes what the manager planned for a week of the season — the
/// template, the player in individual training … —, and his record with
/// it; the season has fifty.
pub(super) fn change_week(cx: &mut Cx, week: usize, change: impl FnOnce(&mut Week)) {
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let current = season.current;
    let Some(manager) = season.managers.get_mut(current) else { return };
    let Some(entry) = manager.weeks.get_mut(week) else { return };
    change(entry);
    let entry = entry.clone();
    if let Some(records) = state.records.as_mut()
        && let Some(slot) = records.human_slot(current)
        && let Some(record) = records.managers.get_mut(slot)
    {
        record.set_week(week, &entry);
    }
}
