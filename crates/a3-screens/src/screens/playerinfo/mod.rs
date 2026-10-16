//! A player's info, as the game's `CSpielerInfoKomplett` (dialog 214)
//! shows it, a page at a time: "Aktuell" (dialog 399), "Saison" (402),
//! "Historisch" (400) and "Privat" (401). Every page starts with who he is;
//! the upper arrows go through the pages, the lower through the squad by
//! position, keepers first.
//!
//! What only a save's record holds — his contract, foot, portrait, nations,
//! his season so far, his career and his private life — stays empty for a
//! player of a new game.

mod current;
mod history;
mod private;
mod season;

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};

use a3_core::lineup;
use a3_core::records::{Player as Record, Records};
use a3_core::season::{ClubState, Fitness, Season, Status};
use a3_core::world::Player;
use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel, set};
use crate::controls::ControlState;
use crate::controls::bitmap::{Picture, PictureSource};
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

pub const CURRENT: u16 = 399;
pub const SEASON: u16 = 402;
pub const HISTORY: u16 = 400;
pub const PRIVATE: u16 = 401;
/// The pages, in the order the arrows go through them (`FUN_0055dbe9`).
pub const PAGES_IN_ORDER: [u16; 4] = [CURRENT, SEASON, HISTORY, PRIVATE];
/// Each page's name, age and strength.
const HEADERS: [(u16, u16, u16); 4] =
    [(4825, 4826, 4827), (4851, 4852, 4853), (4883, 4884, 4885), (4886, 5496, 4887)];
const PORTRAIT: u16 = 5481;
/// The arrows: through the squad, and through the pages.
const PLAYERS: u16 = 4888;
const PAGES: u16 = 4889;

/// Texts of the game.
const YEARS_OLD: u16 = 4034;
const DATE: u16 = 1997;
const STRENGTH_TEXT: u16 = 1595;
/// Money: up to a thousand, up to a million, and more.
const MONEY: [u16; 3] = [2157, 2156, 2155];
/// The bars' pictures: red, yellow, green.
const BAR_PICTURES: [(i32, u16); 3] = [(60, 2654), (80, 2653), (i32::MAX, 2655)];

/// Bits of a record's `shown`.
const NICKNAME: u32 = 0x8000_0000;
/// The game's players; the ids above are the managers who play.
const LAST_REAL_ID: u16 = 17_000;

/// The caption, at the top: the page's title.
const CAPTION_ID: u16 = 9923;

/// Puts each page's title in its caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for page in PAGES_IN_ORDER {
        let Some(dialog) = dialogs.get_mut(&page) else { continue };
        let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
        dialog.controls.insert(0, caption);
    }
}

/// The player info, on a player of the current club's squad.
#[derive(Default)]
pub struct PlayerInfo {
    /// The squad in the order the arrows go through it, and the player
    /// shown.
    order: Vec<usize>,
    at: usize,
}

impl Screen for PlayerInfo {
    fn ids(&self) -> &[u16] {
        &PAGES_IN_ORDER
    }

    /// Opened on a player, or — on another page — on the one shown.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        if let Args::Player(player) = args {
            let Some(club) = club_state(cx) else { return };
            let by_position = a3_core::squadlist::by_position(&club.players);
            self.order = cx.state.paging.take().unwrap_or(by_position);
            self.at = self.order.iter().position(|&p| p == player).unwrap_or(0);
        }
        for id in [PLAYERS, PAGES] {
            if let Some(arrows) = control(cx.dialog, id).and_then(|c| view.options_mut(c)) {
                arrows.actions = true;
            }
        }
        self.show(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if ok_or_cancel(cx.dialog, target).0 {
            return Outcome::Close(Reply::Ok);
        }
        // The boxes only tell: a click on one changes nothing.
        if let Target::Check(_) = target {
            self.show(cx, view);
            return Outcome::Stay;
        }
        let Target::Option { control: c, item } = target else { return Outcome::Stay };
        match cx.dialog.controls[c].id {
            Some(PLAYERS) if !self.order.is_empty() => {
                let n = self.order.len();
                self.at = if item == 0 { (self.at + n - 1) % n } else { (self.at + 1) % n };
                self.show(cx, view);
            }
            Some(PAGES) => {
                let n = PAGES_IN_ORDER.len();
                let here = PAGES_IN_ORDER.iter().position(|&p| p == view.id).unwrap_or(0);
                let next = if item == 0 { (here + n - 1) % n } else { (here + 1) % n };
                return Outcome::Close(Reply::Goto(PAGES_IN_ORDER[next]));
            }
            _ => {}
        }
        Outcome::Stay
    }
}

fn club_state<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

/// The player a page shows, and what it reads him from.
struct Shown<'a> {
    game: &'a Game,
    dialog: &'a Dialog,
    season: &'a Season,
    records: Option<&'a Records>,
    /// His record, from a save.
    record: Option<&'a Record>,
    player: &'a Player,
    fitness: Fitness,
    status: Status,
    /// His club: its country's slot, and its index there.
    club: (usize, usize),
    today: NaiveDate,
}

impl Shown<'_> {
    fn text(&self, id: u16) -> String {
        self.game.text(id).to_owned()
    }

    fn put(&self, view: &mut View, id: u16, text: impl Into<String>) {
        set(view, self.dialog, id, text.into());
    }

    fn picture(&self, view: &mut View, id: u16, picture: Option<Picture>) {
        if let Some(c) = control(self.dialog, id) {
            view.set_picture(c, picture);
        }
    }

    /// Ticks the box `id`, or not.
    fn tick(&self, view: &mut View, id: u16, on: bool) {
        if let Some(check) = control(self.dialog, id).and_then(|c| view.check_mut(c)) {
            check.checked = on;
        }
    }

    /// Fills the list `id` with `rows`.
    fn rows(&self, view: &mut View, id: u16, rows: Vec<String>) {
        if let Some(list) = control(self.dialog, id).and_then(|c| view.list_mut(c)) {
            *list = Default::default();
            list.rows = rows;
        }
    }

    /// A bar that rises from the bottom as far as `value` is of `most`, red
    /// below 60, yellow below 80, green above (`FUN_00560510`).
    fn bar(&self, view: &mut View, id: u16, value: i32, most: i32) {
        let Some(c) = control(self.dialog, id) else { return };
        let rect = self.dialog.controls[c].rect;
        let shown = value.clamp(0, most);
        let height = (rect.height() as i32 * shown / most).max(0) as u32;
        let graphic = BAR_PICTURES.iter().find(|(below, _)| shown < *below).map_or(0, |&(_, g)| g);
        let picture = Picture {
            source: PictureSource::Graphic(graphic),
            part: Some([0, 0, rect.width() as u32, height.max(1)]),
            black_clear: false,
        };
        if let ControlState::Bitmap(b) = &mut view.controls[c].state {
            b.picture = (height > 0).then_some(picture);
            b.bottom = true;
        }
    }

    /// The name a team-mate goes by: his nickname if he has one shown,
    /// else his first and last name (`FUN_006feb4d`).
    fn known_as(&self, id: u16) -> Option<String> {
        self.records?.known_as(id)
    }

    /// Whether the game follows his country's matches: the main country
    /// and the more ones played (`FUN_00676c03`).
    fn followed(&self) -> bool {
        match (self.records, self.record) {
            (Some(records), Some(r)) => records.globals.played(i16::from(r.club()[0])),
            _ => true,
        }
    }
}

impl PlayerInfo {
    fn show(&self, cx: &mut Cx, view: &mut View) {
        let Some(season) = cx.state.season.as_ref() else { return };
        let Some(club) = season.manager().and_then(|m| m.club) else { return };
        let Some(state) = season.clubs.get(&club) else { return };
        let Some(&p) = self.order.get(self.at) else { return };
        let status = state.status.get(p).copied().unwrap_or_default();
        let records = cx.state.records.as_ref();
        let shown = Shown {
            game: cx.game,
            dialog: cx.dialog,
            season,
            records,
            record: records.and_then(|r| r.player(status.id)),
            player: &state.players[p],
            fitness: lineup::fitness_of(&state.fitness, p),
            status,
            club,
            today: season.date(),
        };
        header(&shown, view);
        match view.id {
            CURRENT => current::show(&shown, view),
            SEASON => season::show(&shown, view),
            HISTORY => history::show(&shown, view),
            PRIVATE => private::show(&shown, view),
            _ => {}
        }
    }
}

/// Who he is, at the top of every page: his name, age and strength, and
/// his portrait.
fn header(s: &Shown, view: &mut View) {
    let page = PAGES_IN_ORDER.iter().position(|&p| p == view.id).unwrap_or(0);
    let (name_id, age_id, strength_id) = HEADERS[page];
    let player = s.player;
    let nickname = s
        .record
        .filter(|r| r.shown() & NICKNAME != 0)
        .and_then(|r| s.records.map(|records| records.name(r.nickname()).to_owned()));
    s.put(
        view,
        name_id,
        match nickname {
            Some(nick) => format!("{} \"{nick}\" {}", player.first_name, player.last_name),
            None => format!("{} {}", player.first_name, player.last_name),
        },
    );
    let born = player.birthday.map_or_else(String::new, |d| date(s.game, d));
    s.put(
        view,
        age_id,
        s.text(YEARS_OLD)
            .replacen("%d", &age(player.birthday, s.today).to_string(), 1)
            .replacen("%s", &born, 1),
    );
    s.put(view, strength_id, s.text(STRENGTH_TEXT).replacen("%s", &player.strength.to_string(), 1));
    let portrait = s.record.and_then(|r| s.game.rules.words.portrait(r.face()));
    s.picture(view, PORTRAIT, portrait.map(Picture::graphic));
}

/// The text a control has in the dialog file, as a template.
fn dialog_text(dialog: &Dialog, id: u16) -> String {
    control(dialog, id).map_or_else(String::new, |c| dialog.controls[c].text.clone())
}

/// A date as the game writes it: "18.1.2006".
fn date(game: &Game, d: NaiveDate) -> String {
    game.text(DATE)
        .replace("%1", &d.day().to_string())
        .replace("%2", &d.month().to_string())
        .replace("%3", &d.year().to_string())
}

/// How old he is, as the game counts it (`FUN_0067582a`).
fn age(birthday: Option<NaiveDate>, today: NaiveDate) -> i32 {
    let Some(born) = birthday else { return 0 };
    let had =
        born.month() < today.month() || born.month() == today.month() && born.day() <= today.day();
    (today.year() - born.year() - 1 + i32::from(had)).max(0)
}

/// Condition and freshness as the bars show them (`FUN_00457e00`,
/// `FUN_006ff649`).
fn bars(f: Fitness) -> (i32, i32) {
    let mut condition = f.condition / 100;
    if f.smoker {
        condition = (condition - 5).max(0);
    }
    let fresh: i32 = f.freshness.iter().sum();
    let freshness = if fresh < 0 {
        0
    } else if fresh <= 10_000 {
        fresh / 100
    } else {
        100
    };
    (condition, freshness)
}

/// A number with one decimal as the original's `printf` writes it: a half
/// rounds up, where Rust's rounds to even. Only quarters fall halfway.
pub(crate) fn one_decimal(v: f64) -> String {
    let halfway = (v * 4.0).fract() == 0.0 && (v * 2.0).fract() != 0.0;
    if halfway { format!("{:.1}", v + 0.05f64.copysign(v)) } else { format!("{v:.1}") }
}

/// Money as the game writes it (`FUN_004daf8f`): "144.000 €", "1.36 Mio €".
pub(crate) fn money(game: &Game, value: i64) -> String {
    let sign = if value < 0 { "- " } else { "" };
    let v = value.abs();
    let template = |i: usize| {
        game.text(MONEY[i]).trim_start().trim_start_matches("%c").trim_start().to_owned()
    };
    let body = if v < 1000 {
        template(0).replacen("%d", &v.to_string(), 1)
    } else if v < 1_000_000 {
        template(1).replacen("%d", &(v / 1000).to_string(), 1).replacen(
            "%03d",
            &format!("{:03}", v % 1000),
            1,
        )
    } else {
        template(2).replacen("%d", &(v / 1_000_000).to_string(), 1).replacen(
            "%02d",
            &format!("{:02}", v % 1_000_000 / 10_000),
            1,
        )
    };
    format!("{sign}{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_decimal_rounds_halves_up_as_the_original() {
        // The original showed a grade average of 13 / 2 / 2 as "3.3".
        assert_eq!(one_decimal(3.25), "3.3");
        assert_eq!(one_decimal(3.75), "3.8");
        assert_eq!(one_decimal(12.8), "12.8");
        assert_eq!(one_decimal(9.0), "9.0");
        assert_eq!(one_decimal(0.15), "0.1");
    }
}
