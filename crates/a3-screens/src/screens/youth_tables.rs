//! The youth's league tables (dialog 430, `CTabelleJugend`), from the
//! table button beside each age group of the youth screen: the last
//! matchday's results with their half-times, and the table — overall, at
//! home, away — of the league chosen on the tabs below; for a youth in the
//! final round a tab more with how far each club came. Its init
//! `FUN_005863ea`, the lists `FUN_0058685c` and `FUN_00586cbe`.

use std::collections::HashMap;

use a3_core::records::{Records, Team};
use a3_core::youth_league::{self, FINAL_ROUND, Games, Row, YouthLeague};
use a3_files::rch::{Align, Dialog};

use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel};
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

pub const YOUTH_TABLES: u16 = 430;
const RESULTS: u16 = 3728;
const TABLE: u16 = 3729;
const TABS: u16 = 3723;
/// "ANSTOSS 3" and each age group's title (4121–4123).
const GAME_TITLE: u16 = 2759;
const TITLES: [u16; 3] = [4121, 4122, 4123];
/// The tab of the final round, and its texts: not played yet, the places
/// reached, and what the own youth's place means.
const FINAL_TAB: u16 = 4120;
const NOT_PLAYED: u16 = 4124;
const PLACES: [u16; 3] = [4125, 4126, 4127];
const VERDICTS: [u16; 5] = [4128, 4129, 4130, 4131, 4132];
/// The tab strips' pictures by how many tabs there are (`0x88a508`).
const STRIPS: [u16; 5] = [2067, 2067, 2068, 2808, 2807];
/// The main countries with three leagues of youth, and those with two.
const THREE_LEAGUES: [u16; 2] = [14, 16];
const TWO_LEAGUES: [u16; 3] = [20, 28, 63];
/// The colours of a table's lines (COLORREFs): the first of the first
/// league, the top three of a lower one, the last three, a human's club.
const TOP: u32 = 0x00c8c8;
const UP: u32 = 0xd200d2;
const DOWN: u32 = 0x800080;
const OWN: u32 = 0x0000ff;

/// The lists' columns as the game sets them (`0x88a518`, `0x88a578`):
/// places with 0x8000 stand right in the width beside them.
const RESULT_COLUMNS: [(u16, u16); 12] = [
    (0, 0),
    (112, 0),
    (0x80e6, 13),
    (244, 0),
    (0x8108, 15),
    (280, 0),
    (300, 0),
    (412, 0),
    (0x8212, 13),
    (544, 0),
    (0x8232, 15),
    (578, 0),
];
const TABLE_COLUMNS: [(u16, u16); 17] = [
    (0x8002, 18),
    (35, 0),
    (0x80be, 30),
    (0x80dc, 30),
    (0x80fd, 25),
    (280, 0),
    (285, 0),
    (0x813b, 30),
    (0x8159, 30),
    (0x817a, 25),
    (405, 0),
    (410, 0),
    (0x81c2, 30),
    (0x81e0, 30),
    (0x8201, 25),
    (540, 0),
    (545, 0),
];

fn set_columns(view: &mut View, list: usize, columns: &[(u16, u16)]) {
    let Some(list) = view.list_mut(list) else { return };
    list.tabs = columns.iter().map(|&(at, _)| f32::from(at & 0x3fff)).collect();
    // A left column runs to the next one's start; the ":" between the
    // goals, five pixels before the next, shows whole on the original,
    // where the replica's clip would cut it.
    list.aligns = columns
        .iter()
        .zip(columns.iter().skip(1).map(|&(next, _)| f32::from(next & 0x3fff)).chain([0.0]))
        .map(|(&(at, width), next)| {
            let left = f32::from(at & 0x3fff);
            if at & 0x8000 != 0 {
                Some((Align::Right, f32::from(width)))
            } else if next > left && next - left <= COLON_ROOM {
                Some((Align::Left, next - left))
            } else {
                None
            }
        })
        .collect();
}

/// The room a ":" column has before the next.
const COLON_ROOM: f32 = 5.0;

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&YOUTH_TABLES) else { return };
    let caption = super::caption(dialog.frame, super::discipline::CAPTION_ID, String::new());
    dialog.controls.insert(0, caption);
    // The picture under it all, which the game changes by the number of
    // tabs (`FUN_007a12fc`).
    let whole = a3_files::rch::Control {
        kind: a3_files::rch::Kind::Bitmap,
        rect: bevy_math::Rect::new(0.0, 0.0, 640.0, 480.0),
        align: Align::Left,
        text: String::new(),
        id: Some(BACKGROUND),
        font: a3_files::rch::PLAIN_FONT,
        ink: [0; 3],
    };
    dialog.controls.insert(0, whole);
}

const BACKGROUND: u16 = 9943;

/// What a tab shows: a league's table, or the final round.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    League(usize),
    Final,
}

#[derive(Default)]
pub struct YouthTables {
    age: usize,
    tabs: Vec<Tab>,
}

impl YouthTables {
    fn show(&self, cx: &Cx, view: &mut View, tab: usize) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(own) = super::squad::own_team(cx) else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let (country, club) = super::youth::youth_club(own);
        let lists = [control(dialog, RESULTS), control(dialog, TABLE)];
        for list in lists.into_iter().flatten() {
            if let Some(list) = view.list_mut(list) {
                list.rows.clear();
                list.inks.clear();
            }
        }
        let (Some(results), Some(table)) = (lists[0], lists[1]) else { return };
        match self.tabs.get(tab) {
            Some(&Tab::League(league)) => {
                let Some(youth) = YouthLeague::of(records, country.into(), league, self.age) else {
                    return;
                };
                set_columns(view, results, &RESULT_COLUMNS);
                set_columns(view, table, &TABLE_COLUMNS);
                let rows = result_rows(records, &youth, country);
                if let Some(list) = view.list_mut(results) {
                    list.rows = rows;
                }
                let (rows, inks) = table_rows(records, &youth, country, league);
                if let Some(list) = view.list_mut(table) {
                    list.rows = rows;
                    list.inks = inks;
                }
            }
            Some(Tab::Final) => {
                let rows = final_rows(game, records, country, usize::from(club), self.age);
                if let Some(list) = view.list_mut(table) {
                    list.tabs = vec![0.0, 200.0];
                    list.aligns = Vec::new();
                    list.rows = rows;
                }
            }
            None => {}
        }
    }
}

/// The last matchday's games, two to a line (`FUN_00586cbe`'s first loop):
/// "home - away goals (half-time)", nothing where one is not played.
fn result_rows(records: &Records, youth: &YouthLeague, country: u8) -> Vec<String> {
    let played = youth.played();
    let name = |slot: usize| {
        u16::try_from(youth.number(slot))
            .ok()
            .and_then(|n| records.team(country.into(), n))
            .map(Team::name)
            .unwrap_or_default()
    };
    let game = |(home, away): (usize, usize)| {
        let score = youth.result(played, home).map_or_else(
            || "\t\t\t\t".to_owned(),
            |s| format!("\t{}\t:{}\t({}\t:{})", s.home, s.away, s.half_home, s.half_away),
        );
        format!("{}\t- {}{score}", name(home), name(away))
    };
    let pairs = youth.pairs(played.saturating_sub(1));
    pairs
        .chunks(2)
        .map(|two| {
            let first = game(two[0]);
            match two.get(1) {
                Some(&second) => {
                    // The second game's empty result writes nothing.
                    let text = game(second);
                    let text = text.strip_suffix("\t\t\t\t").unwrap_or(&text).to_owned();
                    format!("{first}\t{text}")
                }
                None => format!("{first}\t"),
            }
        })
        .collect()
}

/// The table with its home and away columns, and each line's colour.
fn table_rows(
    records: &Records,
    youth: &YouthLeague,
    country: u8,
    league: usize,
) -> (Vec<String>, Vec<Option<[u8; 3]>>) {
    let all = youth.table(Games::All);
    let home = youth.table(Games::Home);
    let away = youth.table(Games::Away);
    let find = |rows: &[Row], number: i16| {
        rows.iter().find(|r| r.number == number).copied().unwrap_or_default()
    };
    let figures = |r: &Row| {
        format!("{}\t{}\t{}\t:\t{}", r.points, r.difference(), r.goals_for, r.goals_against)
    };
    let n = all.len();
    all.iter()
        .enumerate()
        .map(|(place, row)| {
            let team = u16::try_from(row.number).ok().and_then(|t| records.team(country.into(), t));
            let name = team.map(Team::name).unwrap_or_default();
            let text = format!(
                "{}.\t{name}\t{}\t{}\t{}",
                place + 1,
                figures(row),
                figures(&find(&home, row.number)),
                figures(&find(&away, row.number)),
            );
            let colour = if league == 0 && place == 0 {
                Some(TOP)
            } else if league > 0 && place < 3 {
                Some(UP)
            } else if place + 3 >= n {
                Some(DOWN)
            } else {
                team.is_some_and(|t| t.human() != 0).then_some(OWN)
            };
            (text, colour.map(rgb))
        })
        .unzip()
}

fn rgb(colorref: u32) -> [u8; 3] {
    let [r, g, b, _] = colorref.to_le_bytes();
    [r, g, b]
}

/// The final round's tab (`FUN_0058685c`): the clubs that reached it with
/// their places, and what the own youth's place means.
fn final_rows(game: &Game, records: &Records, country: u8, club: usize, age: usize) -> Vec<String> {
    let Some(own) = youth_league::stage(records, country.into(), club, age) else {
        return Vec::new();
    };
    if !own.played {
        return vec![game.text(NOT_PLAYED).to_owned()];
    }
    let teams = usize::try_from(records.countries.team_count(country.into())).unwrap_or(0);
    let mut rows: Vec<String> = (0..teams)
        .filter_map(|c| {
            let stage = youth_league::stage(records, country.into(), c, age)?;
            if stage.stage != FINAL_ROUND || stage.place >= 2 {
                return None;
            }
            let name = records.team(country.into(), u16::try_from(c).ok()?)?.name();
            let place = PLACES.get(usize::from(stage.place)).map_or("", |&t| game.text(t));
            Some(format!("{name}\t{place}"))
        })
        .collect();
    let verdict = VERDICTS.get(usize::from(own.place)).copied().unwrap_or(VERDICTS[4]);
    rows.push(game.text(verdict).to_owned());
    rows
}

impl Screen for YouthTables {
    fn ids(&self) -> &[u16] {
        &[YOUTH_TABLES]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let Args::AgeGroup(age) = args else { return };
        self.age = age.min(2);
        let (game, dialog) = (cx.game, cx.dialog);
        let title = format!("{} - {}", game.text(GAME_TITLE), game.text(TITLES[self.age]));
        super::set(view, dialog, super::discipline::CAPTION_ID, title);
        let Some(own) = super::squad::own_team(cx) else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let (country, club) = super::youth::youth_club(own);
        let main = records.globals.main_country() == i16::from(country);
        let leagues = if !main {
            1
        } else if THREE_LEAGUES.contains(&u16::from(country)) {
            3
        } else if TWO_LEAGUES.contains(&u16::from(country)) {
            2
        } else {
            1
        };
        self.tabs = (0..leagues).map(Tab::League).collect();
        let stage = youth_league::stage(records, country.into(), usize::from(club), self.age);
        let final_round = stage.is_some_and(|s| s.stage == FINAL_ROUND);
        if final_round {
            self.tabs.push(Tab::Final);
        }
        if let Some(tabs) = control(dialog, TABS).and_then(|c| view.options_mut(c)) {
            if final_round {
                if let Some(label) = tabs.labels.get_mut(leagues) {
                    *label = game.text(FINAL_TAB).to_owned();
                }
            }
            let count = self.tabs.len();
            tabs.off = (0..tabs.labels.len()).map(|i| i >= count).collect();
            tabs.chosen = 0;
        }
        let strip = STRIPS.get(self.tabs.len()).copied();
        if let Some(c) = control(dialog, BACKGROUND) {
            view.set_picture(c, strip.map(crate::controls::bitmap::Picture::graphic));
        }
        self.show(cx, view, 0);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Target::Option { control: c, item } = target
            && cx.dialog.controls[c].id == Some(TABS)
        {
            self.show(cx, view, item);
            return Outcome::Stay;
        }
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if ok || cancel { Outcome::Close(Reply::Ok) } else { Outcome::Stay }
    }
}
