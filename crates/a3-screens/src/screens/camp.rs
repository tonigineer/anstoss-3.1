//! Booking a training camp from the year calendar (JAHRESKALENDER.md §4.3
//! in the RE repo): the world map of the camps' places (dialog 488,
//! `CDiWeltkarte`), the place confirmed with its pictures and cost (285,
//! `CTrainingslagerAuswahl`), and for a group of places sharing one point
//! the choice among them (50, `CTrainingslagerSubAuswahl`). The map closes
//! with the place chosen, from 1, which the calendar books.

use std::collections::HashMap;

use a3_core::calendar;
use a3_core::cycle::dialogs::NOTICE;
use a3_core::season::Rules;
use a3_files::rch::{Align, Control, Dialog, Kind, PLAIN_FONT};
use bevy_math::Rect;
use chrono::Datelike;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::controls::bitmap::Picture;
use crate::pointer::Target;
use crate::view::View;

/// The three dialogs.
pub(crate) const WORLD_MAP: u16 = 488;
pub(crate) const CAMP_CONFIRM: u16 = 285;
pub(crate) const CAMP_GROUP: u16 = 50;

/// The map: its cancel button, a place's point (id 100 + the place, an
/// 8×8 button with these pictures), Atlantis' picture.
const MAP_CANCEL: u16 = 2;
const POINTS: u16 = 100;
const POINT_SIZE: f32 = 8.0;
const POINT_PICTURES: (u16, u16) = (3113, 3114);
const ATLANTIS: u16 = 6156;
const ATLANTIS_PICTURE: u16 = 3115;
/// "Noch nicht möglich!": a group's place whose first season has not come.
const NOT_YET: u16 = 4588;

/// The confirmation: the manager and the day, the place and its cost, its
/// picture, its ball, the three glasses; OK and cancel.
const WHO_AND_WHEN: u16 = 3901;
const PLACE_AND_COST: u16 = 6153;
const PLACE_PICTURE: u16 = 6154;
const BALL: u16 = 6155;
const GLASSES: [u16; 3] = [3914, 3915, 3916];
const OK: u16 = 1;
const CANCEL: u16 = 2;
/// The day as text 1775 writes it, "%1, dem %2.%3.%4", the weekday's short
/// name first (`FUN_00676326`).
const DAY_TEXT: u16 = 1775;
const WEEKDAYS: u16 = 1779;
/// What is drunk at a place: vodka, water, or one to three glasses of
/// sparkling wine (`FUN_005a269f`'s switch, its cases falling through).
const VODKA: u16 = 1404;
const WATER: u16 = 1403;
const SPARKLING: u16 = 1402;

/// The group's choice: the question, the ball, the hidden picture, the
/// three rows; OK stays hidden, so only a row ends it.
const QUESTION: u16 = 1445;
const QUESTION_TEXT: u16 = 4587;
const GROUP_BALL: u16 = 1336;
const GROUP_PICTURE: u16 = 1450;
const ROWS: u16 = 1447;

/// What the map waits for: the confirmation of a place, or the choice
/// among a group's.
enum Asked {
    Confirm(usize),
    Group(Vec<usize>),
}

#[derive(Default)]
pub struct WorldMap {
    day: usize,
    asked: Option<Asked>,
}

impl Screen for WorldMap {
    fn ids(&self) -> &[u16] {
        &[WORLD_MAP]
    }

    /// `FUN_005ccf5a`: a point for every place open this season; Atlantis'
    /// picture only in the lottery's lucky season.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        *self = Self::default();
        if let Args::CampDay(day) = args {
            self.day = day;
        }
        // The title with the manager's name and his club, no date.
        let title = cx.state.season.as_ref().and_then(|s| s.manager()).map_or_else(
            || dialog.title.clone(),
            |m| {
                let club = m.club.and_then(|(slot, c)| cx.game.world.country(slot)?.clubs.get(c));
                let who = format!("{} {} {}", dialog.title, m.first_name, m.last_name);
                club.map_or(who.clone(), |club| format!("{who} ({})", club.name))
            },
        );
        set(view, dialog, CAPTION_ID, title);
        let tables = &cx.game.rules.year;
        let (year, lottery) = season_year(cx);
        for place in 0..tables.places.len() {
            if let Some(c) = control(dialog, POINTS + place as u16) {
                view.controls[c].hidden = !tables.open(place, year, lottery);
            }
        }
        if let Some(c) = control(dialog, ATLANTIS) {
            view.set_picture(c, Some(Picture::graphic(ATLANTIS_PICTURE)));
            view.controls[c].hidden = lottery != 0;
            view.set_clickable(c, lottery == 0);
        }
        mark_inert(view, dialog, |_| true);
    }

    /// `FUN_005cd5b7`: a place's point, or Atlantis' picture.
    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        let places = &cx.game.rules.year.places;
        let place = match target {
            Target::Button(c) if id(c) == MAP_CANCEL => return Outcome::Close(Reply::Cancel),
            Target::Button(c) => id(c).checked_sub(POINTS).map(usize::from),
            Target::Control(c) => places.iter().position(|p| p.control == id(c) as i16),
            _ => None,
        };
        let Some(place) = place.filter(|&p| p < places.len()) else { return Outcome::Stay };
        self.choose(cx, place)
    }

    /// The confirmation's answer closes the map with the place, from 1; the
    /// group's opens the confirmation of the place chosen, if it is open.
    fn answer(&mut self, cx: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        match (self.asked.take(), reply) {
            (Some(Asked::Confirm(place)), Reply::Ok) => Outcome::Close(Reply::Choice(place + 1)),
            (Some(Asked::Group(places)), Reply::Choice(row)) => {
                let Some(&place) = places.get(row) else { return Outcome::Stay };
                let (year, _) = season_year(cx);
                if !cx.game.rules.year.begun(place, year) {
                    return Outcome::Popup(Request::with(NOTICE, Args::Text(NOT_YET)));
                }
                self.asked = Some(Asked::Confirm(place));
                Outcome::Popup(Request::with(CAMP_CONFIRM, Args::Camp { place, day: self.day }))
            }
            _ => Outcome::Stay,
        }
    }
}

impl WorldMap {
    /// A place picked: its confirmation, or for a group the choice among
    /// its other places — nothing where it has none.
    fn choose(&mut self, cx: &Cx, place: usize) -> Outcome {
        let tables = &cx.game.rules.year;
        if tables.places[place].group < 1 {
            self.asked = Some(Asked::Confirm(place));
            return Outcome::Popup(Request::with(CAMP_CONFIRM, Args::Camp { place, day: self.day }));
        }
        let others = tables.group_of(place);
        if others.is_empty() {
            return Outcome::Stay;
        }
        let mut places = [0; 3];
        for (slot, &p) in places.iter_mut().zip(&others) {
            *slot = p as u8;
        }
        let count = others.len() as u8;
        self.asked = Some(Asked::Group(others));
        Outcome::Popup(Request::with(CAMP_GROUP, Args::CampGroup { places, count }))
    }
}

#[derive(Default)]
pub struct CampConfirm;

impl Screen for CampConfirm {
    fn ids(&self) -> &[u16] {
        &[CAMP_CONFIRM]
    }

    /// `FUN_005a269f`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Args::Camp { place, day } = args else { return };
        let Some(p) = game.rules.year.places.get(place).copied() else { return };
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        super::seasonal_buttons(cx, view);
        let Some(season) = cx.state.season.as_ref() else { return };
        let date = calendar::date(season.year, day);
        let when = game
            .text(DAY_TEXT)
            .replace("%1", game.text(WEEKDAYS + (day % 7) as u16))
            .replace("%2", &date.day().to_string())
            .replace("%3", &date.month().to_string())
            .replace("%4", &date.year().to_string());
        let (first, last) = season.manager().map_or((String::new(), String::new()), |m| {
            (m.first_name.clone(), m.last_name.clone())
        });
        set(view, dialog, WHO_AND_WHEN, format!("{first} {last}  {when}"));
        let cost = super::playerinfo::money(game, i64::from(p.cost));
        set(view, dialog, PLACE_AND_COST, format!("{} ({cost})", game.text(p.name)));
        let tables = &game.rules.year;
        let picture = |c: u16, id: Option<u16>, view: &mut View| {
            if let (Some(c), Some(id)) = (control(dialog, c), id) {
                view.set_picture(c, Some(Picture::graphic(id)));
            }
        };
        picture(PLACE_PICTURE, tables.place_pictures.get(usize::from(p.picture)).copied(), view);
        picture(BALL, tables.ball_pictures.get(usize::from(p.ball)).copied(), view);
        match p.drink {
            0 => picture(GLASSES[0], Some(VODKA), view),
            1 => picture(GLASSES[0], Some(WATER), view),
            n => {
                for &glass in GLASSES.iter().take(usize::from(n.min(4)) - 1) {
                    picture(glass, Some(SPARKLING), view);
                }
            }
        }
        mark_inert(view, dialog, |target| match target {
            Target::Button(c) => matches!(dialog.controls[c].id, Some(OK | CANCEL)),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(OK) => Outcome::Close(Reply::Ok),
            Target::Button(c) if cx.dialog.controls[c].id == Some(CANCEL) => {
                Outcome::Close(Reply::Cancel)
            }
            _ => Outcome::Stay,
        }
    }
}

#[derive(Default)]
pub struct CampGroup;

impl Screen for CampGroup {
    fn ids(&self) -> &[u16] {
        &[CAMP_GROUP]
    }

    /// `FUN_005a2bf8`: the title with the manager's name, the question, a
    /// bouncing ball, and a row for each of the group's other places.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Args::CampGroup { places, count } = args else { return };
        let name = cx.state.season.as_ref().and_then(|s| s.manager()).map_or_else(String::new, |m| {
            format!("{} {}", m.first_name, m.last_name)
        });
        set(view, dialog, CAPTION_ID, format!("{} {name}", dialog.title));
        set(view, dialog, QUESTION, game.text(QUESTION_TEXT).to_owned());
        if let Some(ball) = control(dialog, GROUP_BALL).and_then(|b| view.anim_mut(b)) {
            ball.frames = (1745u16..=1753).chain((1746..=1752).rev()).collect();
            ball.playing = true;
        }
        for id in [GROUP_PICTURE, OK] {
            if let Some(c) = control(dialog, id) {
                view.controls[c].hidden = true;
            }
        }
        let tables = &game.rules.year;
        if let Some(rows) = control(dialog, ROWS).and_then(|c| view.options_mut(c)) {
            // Answers, not a choice: none stands chosen, the one pointed
            // at lights up.
            rows.actions = true;
            rows.chosen = crate::controls::options::NONE;
            let count = usize::from(count);
            for (row, &place) in places.iter().enumerate() {
                if let Some(label) = rows.labels.get_mut(row) {
                    let name = tables.places.get(usize::from(place)).map_or(0, |p| p.name);
                    *label = if row < count { game.text(name).to_owned() } else { String::new() };
                }
                if let Some(off) = rows.off.get_mut(row) {
                    *off = row >= count;
                }
            }
        }
        mark_inert(view, dialog, |_| true);
    }

    /// A row ends it with its place (`OnClicked_IDC_EREIGNISFRAGE_ANTW1`).
    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Option { control: c, item } if cx.dialog.controls[c].id == Some(ROWS) => {
                Outcome::Close(Reply::Choice(item))
            }
            _ => Outcome::Stay,
        }
    }
}

/// The season's year (`DAT_00b5cbca`) and its lottery (`DAT_00b5d174`).
fn season_year(cx: &Cx) -> (i32, u16) {
    cx.state.records.as_ref().map_or((0, 1), |r| {
        (i32::from(r.globals.start_year()), r.globals.lottery())
    })
}

/// The captions, and the map's points: an 8×8 button for every place
/// with a point, whether open or not (`FUN_005ccf5a` makes only the open
/// ones; the others stay hidden here).
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    for id in [WORLD_MAP, CAMP_CONFIRM, CAMP_GROUP] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
        dialog.controls.insert(0, caption);
    }
    let Some(map) = dialogs.get_mut(&WORLD_MAP) else { return };
    for (place, p) in rules.year.places.iter().enumerate() {
        let (x, y) = (f32::from(p.at.0), f32::from(p.at.1));
        if p.at.0 < 0 {
            continue;
        }
        map.controls.push(Control {
            kind: Kind::Button { normal: POINT_PICTURES.0, hover: POINT_PICTURES.1 },
            rect: Rect::new(x, y, x + POINT_SIZE, y + POINT_SIZE),
            align: Align::Left,
            text: String::new(),
            id: Some(POINTS + place as u16),
            font: PLAIN_FONT,
            ink: [0; 3],
        });
    }
}
