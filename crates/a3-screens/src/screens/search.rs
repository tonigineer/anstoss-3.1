//! Looking for players: through any club's squad (dialog 306,
//! `CVereineAbsuchen`), with NORACSA's search (154, `CNoracsaSFB`) and its
//! result (155, `CNoracsaSFBGefunden`), and NORACSA's request (156,
//! `CNoracsaSpielerGesucht`; VEREINE-NORACSA.md §1–4 in the RE repo).

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::{CLUB_SEARCH, NORACSA_FOUND, NORACSA_REQUEST, NORACSA_SEARCH};
use a3_core::noracsa::{self, Criteria, Wanted, field};
use a3_core::records::Records;
use a3_files::rch::Dialog;

use super::hub::{self, Command};
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::{Grip, Target};
use crate::view::View;

const CAPTION_ID: u16 = 9935;
const COUNTRIES: u16 = 4064;
const CLUBS: u16 = 4060;
const PLAYERS: u16 = 4059;
const HEADING: u16 = 6286;
const INVESTABLE: u16 = 4063;
/// A job offer's two wishes, hidden from the transfer market.
const WISHES: [u16; 2] = [4065, 4066];
/// The country numbers there are.
const LAST_COUNTRY: usize = 0x74;
/// "%s (D-Stärke %.1f%s)", a club's average strength and its league.
const HEADING_TEXT: u16 = 4432;
/// A contract running out this season, once the season is late.
const FREE: u16 = 3311;
const LATE_DAY: u16 = 216;
const FEE_UNIT: i64 = 10_000;
const CONTRACT_YEAR: u16 = 0xfff;
/// Players on loan with the club, those not known, and the managers are
/// left out (R+0x15e bit 0x100, R+0x271's class 1).
const ON_LOAN: u16 = 0x100;
const HIDDEN_CLASS: u8 = 1;
const UNKNOWN_CLASS: u8 = 3;
const LAST_REAL_ID: u16 = 0x4268;
/// The leagues: none, and the main country England's deeper ones
/// (`FUN_006755b8`).
const NO_LEAGUE: u8 = 10;
const ENGLAND: i16 = 16;
const BLACK: [u8; 3] = [0, 0, 0];
const ROW_MENU: [Command; 2] = [Command::Info, Command::Watch];

/// The caption keeps the file's title; the row menu.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [CLUB_SEARCH, NORACSA_SEARCH, NORACSA_FOUND, NORACSA_REQUEST] {
        if let Some(dialog) = dialogs.get_mut(&id) {
            let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
            dialog.controls.insert(0, caption);
            if matches!(id, CLUB_SEARCH | NORACSA_FOUND) {
                dialog.controls.push(hub::menu_control());
            }
        }
    }
}

/// A country, its clubs, a club's squad: a click on a player asks for a
/// negotiation, a right click for his info or the watch list.
#[derive(Default)]
pub struct ClubSearch {
    /// The country whose clubs are listed, and the club shown.
    country: u8,
    shown: Option<(u8, u16)>,
    countries: Vec<u8>,
    menu_player: Option<u16>,
}

impl Screen for ClubSearch {
    fn ids(&self) -> &[u16] {
        &[CLUB_SEARCH]
    }

    /// `FUN_005b346f`: the investable amount, the countries with the main
    /// one chosen, its clubs; no players yet.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        let Some(records) = cx.state.records.as_ref() else { return };
        if let Some(team) = own_team(cx) {
            let figures = a3_core::finances::Balance::of(
                team,
                records.globals.fund_prices(),
                &records.agreed(),
            );
            set(view, dialog, INVESTABLE, super::finances::money64(game, figures.investable()));
        }
        let main = records.globals.main_country();
        self.countries = (1..=LAST_COUNTRY)
            .filter(|&c| records.globals.played(c as i16) || records.countries.has_teams(c))
            .map(|c| c as u8)
            .collect();
        self.country = main as u8;
        if let Some(combo) = control(dialog, COUNTRIES).and_then(|c| view.combo_mut(c)) {
            combo.items = self
                .countries
                .iter()
                .map(|&c| game.world.nations.get(usize::from(c)).cloned().unwrap_or_default())
                .collect();
            combo.selected = self.countries.iter().position(|&c| c == self.country);
        }
        for id in WISHES {
            set(view, dialog, id, String::new());
        }
        self.fill_clubs(cx, view);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some(id) = self.menu_player.take() {
            return match hub::close_menu(cx, view, target).and_then(|i| ROW_MENU.get(i)) {
                Some(Command::Info) => {
                    let rows = control(dialog, PLAYERS).and_then(|l| view.list(l));
                    cx.state.paging_ids =
                        rows.map(|l| l.keys.iter().rev().map(|&k| k as u16).collect());
                    Outcome::Popup(Request::with(super::playerinfo::CURRENT, Args::Record(id)))
                }
                Some(Command::Watch) => super::market::watch_player(cx, id),
                _ => Outcome::Stay,
            };
        }
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            // A country change refills only the clubs.
            Target::Row { control: c, .. } if id(c) == Some(COUNTRIES) => {
                let chosen = view.combo(c).and_then(|b| b.selected);
                if let Some(&country) = chosen.and_then(|i| self.countries.get(i)) {
                    self.country = country;
                    self.fill_clubs(cx, view);
                }
                Outcome::Stay
            }
            Target::Row { control: c, row } if id(c) == Some(CLUBS) => {
                let Some(key) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied())
                else {
                    return Outcome::Stay;
                };
                let club = ((key >> 16) as u8, (key & 0xffff) as u16);
                if self.shown != Some(club) {
                    self.shown = Some(club);
                    fill_players(cx, view, club);
                }
                Outcome::Stay
            }
            // `OnSelChange_IDC_VEREINE_ABSUCHEN_SPIELERLISTE`: the row stays
            // selected, nothing is filled anew.
            Target::Row { control: c, row } if id(c) == Some(PLAYERS) => {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                match player {
                    Some(player) => super::market::negotiate(cx, player as u16, true, false),
                    None => Outcome::Stay,
                }
            }
            Target::Menu(grip @ Grip::Row { control: c, row }) if id(c) == Some(PLAYERS) => {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                if let Some(player) = player
                    && hub::show_menu(cx, view, grip, &ROW_MENU).is_some()
                {
                    self.menu_player = Some(player as u16);
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

impl ClubSearch {
    /// The country's clubs but our own, by name as `_stricmp` orders them,
    /// those of one name in their order (`FUN_005b389d`, the list's sorted
    /// style); the list at its top, nothing selected.
    fn fill_clubs(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        let own = own_team(cx).map(|t| (t.country(), u16::from(t.number())));
        let country = self.country;
        let count = records.countries.team_count(usize::from(country)).max(0) as u16;
        let mut clubs: Vec<(String, usize)> = (0..count)
            .filter(|&n| own != Some((country, n)))
            .map(|n| {
                let name = game.club_name(Some(records), country, n as i16);
                (name, usize::from(country) << 16 | usize::from(n))
            })
            .collect();
        clubs.sort_by_key(|(name, _)| stricmp(name));
        if let Some(list) = control(dialog, CLUBS).and_then(|c| view.list_mut(c)) {
            list.inks = vec![Some(BLACK); clubs.len()];
            list.keys = clubs.iter().map(|&(_, k)| k).collect();
            list.rows = clubs.into_iter().map(|(name, _)| name).collect();
            list.scroll = 0;
            list.selected = None;
        }
    }
}

/// A name as `_stricmp` compares it in the C locale: ASCII letters folded,
/// the bytes of Windows-1252 above them.
fn stricmp(name: &str) -> Vec<u32> {
    name.chars()
        .map(|c| if c.is_ascii() { u32::from(c.to_ascii_lowercase()) } else { u32::from(c) })
        .collect()
}

/// The club's squad but those on loan with it, those not known and the
/// managers, by main position up and strength down, as the game's
/// exchange sort leaves them; the heading with the average strength of
/// the whole squad and the league (`FUN_005b39f8`).
fn fill_players(cx: &Cx, view: &mut View, (country, number): (u8, u16)) {
    let (game, dialog) = (cx.game, cx.dialog);
    let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
    else {
        return;
    };
    let Some(team) = records.team(country.into(), number) else { return };
    let squad = team.players();
    let strength = |id: u16| records.player(id).map_or(0, |r| i32::from(r.strength()));
    let sum: i32 = squad.iter().map(|&id| strength(id)).sum();
    let mut rows: Vec<u16> = squad
        .iter()
        .copied()
        .filter(|&id| {
            records.player(id).is_some_and(|r| {
                r.terms() & ON_LOAN == 0 && r.picks() >> 6 != HIDDEN_CLASS && id <= LAST_REAL_ID
            })
        })
        .collect();
    let main = |id: u16| records.player(id).map_or(0, |r| r.positions()[0]);
    super::market::swap_sort(&mut rows, |&q, &p| {
        main(q) < main(p) || (main(q) == main(p) && strength(q) > strength(p))
    });
    let year = i32::from(records.globals.start_year());
    let late = records.globals.day() > LATE_DAY;
    let mut texts = Vec::new();
    for &id in &rows {
        let (Some(r), Some((player, _, status))) =
            (records.player(id), a3_core::load::any_player(records, &game.data, id))
        else {
            continue;
        };
        let years = ((i32::from(r.contract() & CONTRACT_YEAR) - year) as i8).max(0);
        let contract = match years {
            2.. => (year + i32::from(years)).to_string(),
            1 if late => game.text(FREE).to_owned(),
            1 => (year + 1).to_string(),
            _ => "-".to_owned(),
        };
        let fee = if r.fixed_fee() == 0 {
            "-".to_owned()
        } else {
            super::playerinfo::money(game, i64::from(r.fixed_fee()) * FEE_UNIT)
        };
        let strength =
            if r.picks() >> 6 == UNKNOWN_CLASS { "?".to_owned() } else { r.strength().to_string() };
        texts.push(
            [
                super::duties::short_name(cx, &player, id),
                hub::abbreviation(game, player.positions[0]),
                strength,
                player.age(season.date()).to_string(),
                game.world
                    .nation_codes
                    .get(usize::from(status.nations[0]))
                    .cloned()
                    .unwrap_or_default(),
                contract,
                fee,
            ]
            .join("\t"),
        );
    }
    if let Some(list) = control(dialog, PLAYERS).and_then(|c| view.list_mut(c)) {
        list.inks = vec![Some(BLACK); texts.len()];
        list.rows = texts;
        list.keys = rows.iter().map(|&id| usize::from(id)).collect();
        list.scroll = 0;
        list.selected = None;
    }
    let name = game.club_name(Some(records), country, number as i16);
    let heading = if squad.is_empty() {
        name
    } else {
        let average = f64::from(sum) / squad.len() as f64;
        let league = if records.globals.played(country.into()) {
            format!(
                ", {}",
                records.countries.league_name(country.into(), league_place(records, team))
            )
        } else {
            String::new()
        };
        game.text(HEADING_TEXT)
            .replacen("%s", &name, 1)
            .replacen("%.1f", &super::playerinfo::one_decimal(average), 1)
            .replacen("%s", &league, 1)
    };
    set(view, dialog, HEADING, heading);
}

/// Which of its country's league names a club's is (`FUN_0063e388`,
/// `FUN_006755b8`): the main country's up to the third (England's the
/// fourth), every other country's the first; no league one deeper.
fn league_place(records: &Records, team: &a3_core::records::Team) -> usize {
    let country = team.country();
    let level = team.level();
    let place = |level: u8| -> u8 {
        let main = records.globals.main_country();
        if i16::from(country) == main { level.min(if main == ENGLAND { 3 } else { 2 }) } else { 0 }
    };
    let place = match records.countries.kind(usize::from(country)) {
        1 if level == NO_LEAGUE => place(level) + 1,
        1 => place(level),
        _ => level,
    };
    usize::from(place)
}

/// The toggles, each one bit of a criterion: strengths 1 to 13, the age
/// bands, the main positions, the goal bands (`FUN_00524c9f`, `…bbb`,
/// `…c2d`, `…d11`).
const STRENGTH_BUTTONS: [(u16, u8); 13] = [
    (2603, 1),
    (2604, 2),
    (2605, 3),
    (2606, 4),
    (2607, 5),
    (2608, 6),
    (2609, 7),
    (2610, 8),
    (6038, 9),
    (6039, 10),
    (6173, 11),
    (6174, 12),
    (6175, 13),
];
const AGE_BUTTONS: [(u16, u8); 6] =
    [(2611, 0), (2612, 1), (2613, 2), (2614, 3), (2615, 4), (2616, 5)];
const POSITION_BUTTONS: [(u16, u8); 10] = [
    (2617, 1),
    (2619, 2),
    (2618, 3),
    (2620, 4),
    (2621, 5),
    (2624, 6),
    (2622, 7),
    (2623, 8),
    (2625, 9),
    (2626, 10),
];
const GOAL_BUTTONS: [(u16, u8); 6] =
    [(6040, 1), (6166, 2), (6169, 3), (6170, 4), (6171, 5), (6172, 6)];
/// The boxes: the contract must end, a fixed fee, only the unhappy, only
/// national players.
const CONTRACT_BOX: u16 = 6041;
const FLAG_BOXES: [(u16, u8); 3] =
    [(6042, noracsa::FIXED_FEE), (6287, noracsa::UNHAPPY_ONLY), (6288, noracsa::NATIONAL)];
const NAME_EDIT: u16 = 2602;
/// The name counts 19 letters.
const NAME_LETTERS: usize = 19;
/// The least talent: its spinner (up is more), its pictures, the default.
const TALENT_SPIN: u16 = 6148;
const TALENT_PICTURE: u16 = 6149;
const TALENT_PICTURES: [u16; 5] = [2541, 2540, 2539, 2538, 2537];
const DEFAULT_TALENT: i8 = 2;
const LEAGUES: u16 = 2631;
const NATIONALITIES: u16 = 2630;
const NATIONALITY_TEXTS: std::ops::RangeInclusive<u16> = 2271..=2277;
const TRAITS: u16 = 2632;
const ABILITIES: u16 = 2633;
/// A trait's or an ability's "egal"; a keeper's ability.
const ANY: u32 = 100;
const KEEPER_ABILITY: u32 = 0x8000;
const SEARCH: u16 = 2634;
const CANCEL: u16 = 2;

/// NORACSA's search form: the criteria ticked and chosen; "Suchen" shows
/// what it finds in 155, the form staying as it is. Only its cancel button
/// closes it.
#[derive(Default)]
pub struct NoracsaSearch {
    criteria: Criteria,
    /// The league combo's choices: a country and a code.
    leagues: Vec<(u8, u8)>,
}

impl Screen for NoracsaSearch {
    fn ids(&self) -> &[u16] {
        &[NORACSA_SEARCH]
    }

    /// `FUN_00524871`: nothing ticked, the leagues all, any nationality,
    /// no trait or ability chosen, the talent "norm".
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self {
            criteria: Criteria { talent: DEFAULT_TALENT, ..Criteria::default() },
            ..Self::default()
        };
        let tables = &game.rules.noracsa;
        let items = league_choices(cx);
        self.leagues = items.iter().map(|&(_, l)| l).collect();
        let combos = [
            (LEAGUES, items.into_iter().map(|(t, _)| t).collect::<Vec<_>>(), Some(0)),
            (NATIONALITIES, NATIONALITY_TEXTS.map(|t| game.text(t).to_owned()).collect(), Some(0)),
            (TRAITS, tables.traits.iter().map(|&(t, _)| game.text(t).to_owned()).collect(), None),
            (
                ABILITIES,
                tables.abilities.iter().map(|&(t, _)| game.text(t).to_owned()).collect(),
                None,
            ),
        ];
        for (id, items, selected) in combos {
            if let Some(combo) = control(dialog, id).and_then(|c| view.combo_mut(c)) {
                combo.items = items;
                combo.selected = selected;
            }
        }
        if let Some(spin) = control(dialog, TALENT_SPIN).and_then(|c| view.value_mut(c)) {
            spin.range = (0, TALENT_PICTURES.len() as i32 - 1);
            spin.set(DEFAULT_TALENT.into());
        }
        show_talent(cx, view, self.criteria.talent);
        let buttons = STRENGTH_BUTTONS
            .iter()
            .chain(&AGE_BUTTONS)
            .chain(&POSITION_BUTTONS)
            .chain(&GOAL_BUTTONS);
        for &(id, _) in buttons {
            if let Some(c) = control(dialog, id) {
                view.set_chosen(c, crate::controls::options::NONE);
            }
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        let c = &mut self.criteria;
        match target {
            Target::Button(b) if id(b) == CANCEL => Outcome::Close(Reply::Cancel),
            Target::Button(b) if id(b) == SEARCH => self.search(cx, view),
            // A toggle sets its bit, or clears it as it goes out.
            Target::Option { control: b, .. } => {
                let on = view.chosen(b) != crate::controls::options::NONE;
                let toggle = |bits: &mut u16, buttons: &[(u16, u8)]| {
                    if let Some(&(_, bit)) = buttons.iter().find(|&&(i, _)| i == id(b)) {
                        if on { *bits |= 1 << bit } else { *bits &= !(1 << bit) }
                    }
                };
                toggle(&mut c.strengths, &STRENGTH_BUTTONS);
                toggle(&mut c.ages, &AGE_BUTTONS);
                toggle(&mut c.positions, &POSITION_BUTTONS);
                toggle(&mut c.goals, &GOAL_BUTTONS);
                Outcome::Stay
            }
            Target::Check(b) => {
                let ticked = view.check(b).is_some_and(|x| x.checked);
                if id(b) == CONTRACT_BOX {
                    c.contract_ending = ticked;
                } else if let Some(&(_, flag)) = FLAG_BOXES.iter().find(|&&(i, _)| i == id(b)) {
                    if ticked { c.flags |= flag } else { c.flags &= !flag }
                }
                Outcome::Stay
            }
            // Up is more talent: a smaller number (`FUN_006e01e0`).
            Target::Spin { control: s, up } if id(s) == TALENT_SPIN => {
                c.talent =
                    (c.talent + if up { -1 } else { 1 }).clamp(0, TALENT_PICTURES.len() as i8 - 1);
                if let Some(spin) = view.value_mut(s) {
                    spin.set(c.talent.into());
                }
                show_talent(cx, view, self.criteria.talent);
                Outcome::Stay
            }
            Target::Row { control: b, .. } => {
                let Some(row) = view.combo(b).and_then(|x| x.selected) else {
                    return Outcome::Stay;
                };
                let tables = &cx.game.rules.noracsa;
                match id(b) {
                    LEAGUES => c.league = self.leagues.get(row).copied().unwrap_or_default(),
                    NATIONALITIES => c.nationality = row as u8,
                    TRAITS => {
                        let data = tables.traits.get(row).map_or(ANY, |&(_, d)| d);
                        c.traits = if data == ANY { 0 } else { 1 << (data & 31) };
                    }
                    ABILITIES => {
                        let data = tables.abilities.get(row).map_or(ANY, |&(_, d)| d);
                        (c.abilities, c.keeper_abilities) = match data {
                            ANY => (0, 0),
                            d if d & KEEPER_ABILITY != 0 => (0, 1 << (d & 31)),
                            d => (1 << (d & 31), 0),
                        };
                    }
                    _ => {}
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

impl NoracsaSearch {
    /// "Suchen" (`0x5250f4`): the name, the own country, the search, its
    /// result over the form — none found, an empty list.
    fn search(&mut self, cx: &mut Cx, view: &View) -> Outcome {
        let typed =
            control(cx.dialog, NAME_EDIT).map(|c| view.text(c).to_owned()).unwrap_or_default();
        self.criteria.name = typed.chars().take(NAME_LETTERS).collect();
        self.criteria.own_country = own_team(cx).map_or(0, |t| t.country());
        let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
        else {
            return Outcome::Stay;
        };
        cx.state.found = noracsa::search(records, &self.criteria, season.date());
        Outcome::Popup(Request::new(NORACSA_FOUND))
    }
}

/// The search's result: a row a player, red when he signed elsewhere; a
/// click asks for a negotiation, a right click for his info or the watch
/// list.
#[derive(Default)]
pub struct NoracsaFound {
    menu_player: Option<u16>,
}

const FOUND_LIST: u16 = 2636;
const SIGNED_INK: [u8; 3] = [255, 0, 0];
/// R+0x14c: the club he signed with for next season.
const NEXT_CLUB: usize = 0x14c;
/// The leagues shown, the third for every lower one.
const LOWEST_SHOWN: i32 = 3;

impl Screen for NoracsaFound {
    fn ids(&self) -> &[u16] {
        &[NORACSA_FOUND]
    }

    /// `FUN_00525f9f`: the hits in their order.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.menu_player = None;
        let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
        else {
            return;
        };
        let year = i32::from(records.globals.start_year());
        let late = records.globals.day() > LATE_DAY;
        let june = |y: i32| {
            chrono::NaiveDate::from_ymd_opt(y, 6, 30)
                .map_or_else(String::new, |d| hub::date_text(game, d))
        };
        let (mut texts, mut inks, mut keys) = (Vec::new(), Vec::new(), Vec::new());
        for hit in &cx.state.found {
            let id = hit.id;
            let (Some(r), Some((player, fitness, status))) =
                (records.player(id), a3_core::load::any_player(records, &game.data, id))
            else {
                continue;
            };
            let [main, second, third] = player.positions;
            let sides = match (second, third) {
                (0, _) => " ".to_owned(),
                (a, 0) => hub::abbreviation(game, a),
                (a, b) => format!("{}/{}", hub::abbreviation(game, a), hub::abbreviation(game, b)),
            };
            let strength = if matches!(r.picks() >> 6, HIDDEN_CLASS | UNKNOWN_CLASS) {
                "?".to_owned()
            } else {
                r.strength().to_string()
            };
            let [country, number] = r.club();
            let level = super::market::league_level(records, country, number.into());
            let league = if level < LOWEST_SHOWN { level + 1 } else { LOWEST_SHOWN };
            let years = ((i32::from(r.contract() & CONTRACT_YEAR) - year) as i8).max(0);
            let contract = match years {
                2.. => june(year + i32::from(years)),
                1 if late => game.text(FREE).to_owned(),
                1 => june(year + 1),
                _ => "-".to_owned(),
            };
            texts.push(
                [
                    super::duties::short_name(cx, &player, id),
                    hub::abbreviation(game, main),
                    sides,
                    strength,
                    a3_core::lineup::form(fitness).to_string(),
                    player.age(season.date()).to_string(),
                    game.world
                        .nation_codes
                        .get(usize::from(status.nations[0]))
                        .cloned()
                        .unwrap_or_default(),
                    game.club_name(Some(records), country, number.into()),
                    league.to_string(),
                    contract,
                ]
                .join("\t"),
            );
            inks.push(Some(if r.get::<u8>(NEXT_CLUB) != 0 { SIGNED_INK } else { BLACK }));
            keys.push(usize::from(id));
        }
        if let Some(list) = control(dialog, FOUND_LIST).and_then(|c| view.list_mut(c)) {
            list.rows = texts;
            list.inks = inks;
            list.keys = keys;
            list.scroll = 0;
            list.selected = None;
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some(id) = self.menu_player.take() {
            return match hub::close_menu(cx, view, target).and_then(|i| ROW_MENU.get(i)) {
                Some(Command::Info) => {
                    let rows = control(dialog, FOUND_LIST).and_then(|l| view.list(l));
                    cx.state.paging_ids =
                        rows.map(|l| l.keys.iter().rev().map(|&k| k as u16).collect());
                    Outcome::Popup(Request::with(super::playerinfo::CURRENT, Args::Record(id)))
                }
                Some(Command::Watch) => super::market::watch_player(cx, id),
                _ => Outcome::Stay,
            };
        }
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Row { control: c, row } if id(c) == Some(FOUND_LIST) => {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                match player {
                    Some(player) => super::market::negotiate(cx, player as u16, false, false),
                    None => Outcome::Stay,
                }
            }
            Target::Menu(grip @ Grip::Row { control: c, row }) if id(c) == Some(FOUND_LIST) => {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                if let Some(player) = player
                    && hub::show_menu(cx, view, grip, &ROW_MENU).is_some()
                {
                    self.menu_player = Some(player as u16);
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

/// The league combo's choices (`FUN_005252db`): the main country's special
/// codes, its leagues, the further countries played, the rest abroad.
fn league_choices(cx: &Cx) -> Vec<(String, (u8, u8))> {
    let game = cx.game;
    let tables = &game.rules.noracsa;
    let Some(records) = cx.state.records.as_ref() else { return Vec::new() };
    let main = records.globals.main_country();
    let text =
        |t: usize| tables.code_texts.get(t).map_or_else(String::new, |&t| game.text(t).to_owned());
    let league_name = |c: i16, i: i16| {
        a3_core::opponent::League::of(records, c as u16, i as u8)
            .map(|l| l.name())
            .unwrap_or_default()
    };
    let mut items = Vec::new();
    for &code in tables.codes_of(main) {
        items.push((text(usize::from(code)), (0, code)));
    }
    for i in 0..records.countries.leagues(main as usize) {
        items.push((league_name(main, i), (main as u8, noracsa::MAIN_LEAGUE + i as u8)));
    }
    let more = usize::try_from(records.globals.more_count()).unwrap_or(0);
    let further: Vec<i16> = records.globals.more_countries().into_iter().take(more).collect();
    if !further.is_empty() {
        items.push((text(usize::from(noracsa::BONUS)), (0, noracsa::BONUS)));
        for c in further {
            items.push((league_name(c, 0), (c as u8, 0)));
        }
    }
    items.push((text(usize::from(noracsa::ABROAD)), (0, noracsa::ABROAD)));
    items
}

/// "Bitte melden!": a request NORACSA takes up at the next week start.
/// Its form always opens at its defaults, whatever was stored; only some of
/// its controls change the request (`FUN_00526b9e` and its handlers).
#[derive(Default)]
pub struct NoracsaRequest {
    request: Option<Wanted>,
    leagues: Vec<(u8, u8)>,
    /// The traits' and abilities' choices shown: a keeper's or a field
    /// player's.
    keeper_lists: bool,
    talent: i8,
}

const REQUEST_NAME: u16 = 6150;
const REQUEST_STRENGTH: u16 = 2642;
const REQUEST_AGES: u16 = 2650;
const REQUEST_POSITIONS: u16 = 2656;
const REQUEST_GOALS: u16 = 6040;
const REQUEST_TRAITS: u16 = 2667;
const REQUEST_ABILITIES: u16 = 2669;
const REQUEST_LEAGUES: u16 = 2668;
const REQUEST_NATIONALITIES: u16 = 2666;
const REQUEST_CONTRACT: u16 = 2640;
const FEE_SLIDER: u16 = 2670;
const FEE_TEXT: u16 = 2641;
const NO_FEE: u16 = 2504;
const REQUEST_OK: u16 = 2671;
const REQUEST_CANCEL: u16 = 2672;
/// The defaults over the stored request: strength 1, 17 to 19, a keeper,
/// any trait and ability.
const DEFAULT_STRENGTHS: u16 = 2;
const DEFAULT_AGES: u16 = 1;
const DEFAULT_POSITIONS: u16 = 2;
const ANY_MASK: u32 = 100;
/// The fee slider's positions and the amounts they stand for, in 10.000s
/// (`FUN_005c69c9` with the table at 0x88cb50).
const FEE_STEPS: [(i32, i32); 7] =
    [(50, 1), (75, 2), (155, 5), (205, 10), (285, 50), (335, 100), (365, 500)];

fn fee_amount(position: i32) -> i32 {
    let (mut amount, mut from) = (0, 0);
    for (to, step) in FEE_STEPS {
        amount += (position.min(to) - from).max(0) * step;
        from = to;
    }
    amount
}

impl Screen for NoracsaRequest {
    fn ids(&self) -> &[u16] {
        &[NORACSA_REQUEST]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self { talent: DEFAULT_TALENT, keeper_lists: true, ..Self::default() };
        let mut request = own_manager(cx).map(Wanted::of);
        if let Some(r) = request.as_mut() {
            r.put_u8(field::NAME, 0);
            r.put_u16(field::AGES, DEFAULT_AGES);
            r.put_u16(field::STRENGTHS, DEFAULT_STRENGTHS);
            r.put_u16(field::POSITIONS, DEFAULT_POSITIONS);
            r.put_u32(field::TRAITS, ANY_MASK);
            r.put_u32(field::ABILITIES, ANY_MASK);
            r.put_u32(field::KEEPER_ABILITIES, ANY_MASK);
            r.put_u16(field::TALENT, DEFAULT_TALENT as u16);
        }
        self.request = request;
        let items = league_choices(cx);
        self.leagues = items.iter().map(|&(_, l)| l).collect();
        let combos = [
            (REQUEST_LEAGUES, items.into_iter().map(|(t, _)| t).collect::<Vec<_>>()),
            (REQUEST_NATIONALITIES, NATIONALITY_TEXTS.map(|t| game.text(t).to_owned()).collect()),
        ];
        for (id, items) in combos {
            if let Some(combo) = control(dialog, id).and_then(|c| view.combo_mut(c)) {
                combo.items = items;
                combo.selected = Some(0);
            }
        }
        self.fill_lists(cx, view);
        if let Some(slider) = control(dialog, FEE_SLIDER).and_then(|c| view.value_mut(c)) {
            slider.range = (0, FEE_STEPS[FEE_STEPS.len() - 1].0);
            slider.set(0);
        }
        set(view, dialog, FEE_TEXT, game.text(NO_FEE).to_owned());
        if let Some(spin) = control(dialog, TALENT_SPIN).and_then(|c| view.value_mut(c)) {
            spin.range = (0, TALENT_PICTURES.len() as i32 - 1);
            spin.set(DEFAULT_TALENT.into());
        }
        show_talent(cx, view, self.talent);
        for id in [REQUEST_STRENGTH, REQUEST_AGES, REQUEST_POSITIONS, REQUEST_GOALS] {
            if let Some(c) = control(dialog, id) {
                view.set_chosen(c, 0);
            }
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (game, dialog) = (cx.game, cx.dialog);
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        let Some(r) = self.request.as_mut() else { return Outcome::Stay };
        match target {
            Target::Button(b) if id(b) == REQUEST_CANCEL => return Outcome::Close(Reply::Cancel),
            // `0x527506`: the name, the club's country, stored.
            Target::Button(b) if id(b) == REQUEST_OK => {
                let typed = control(dialog, REQUEST_NAME).map(|c| view.text(c).to_owned());
                r.set_name(&typed.unwrap_or_default());
                r.put_u16(field::OWN_COUNTRY, own_team(cx).map_or(0, |t| u16::from(t.country())));
                r.set_state(noracsa::STORED);
                let request = self.request.take();
                if let (Some(request), Some(manager)) = (request, own_manager_mut(cx)) {
                    request.store(manager);
                }
                return Outcome::Close(Reply::Ok);
            }
            Target::Option { control: b, item } if id(b) == REQUEST_AGES => {
                r.put_u16(field::AGES, 1 << item);
            }
            Target::Option { control: b, item } if id(b) == REQUEST_POSITIONS => {
                r.put_u16(field::POSITIONS, 1 << (item + 1));
                self.keeper_lists = item == 0;
                self.fill_lists(cx, view);
            }
            // The box is read from an object never bound: always unticked.
            Target::Check(b) if id(b) == REQUEST_CONTRACT => r.put_u8(field::CONTRACT, 0),
            Target::Spin { control: s, up } if id(s) == TALENT_SPIN => {
                self.talent = (self.talent + if up { -1 } else { 1 })
                    .clamp(0, TALENT_PICTURES.len() as i8 - 1);
                r.put_u16(field::TALENT, self.talent as u16);
                if let Some(spin) = view.value_mut(s) {
                    spin.set(self.talent.into());
                }
                show_talent(cx, view, self.talent);
            }
            Target::SliderStep { control: s, .. } | Target::SliderAt { control: s, .. }
                if id(s) == FEE_SLIDER =>
            {
                let amount = fee_amount(view.value(s).map_or(0, |v| v.value));
                let text = if amount == 0 {
                    game.text(NO_FEE).to_owned()
                } else {
                    super::playerinfo::money(game, i64::from(amount) * FEE_UNIT)
                };
                set(view, dialog, FEE_TEXT, text);
            }
            Target::Row { control: b, .. } => {
                let Some(row) = view.combo(b).and_then(|x| x.selected) else {
                    return Outcome::Stay;
                };
                let (traits, abilities) = lists(self.keeper_lists, cx);
                match id(b) {
                    REQUEST_LEAGUES => {
                        let (country, code) = self.leagues.get(row).copied().unwrap_or_default();
                        r.put_u8(field::LEAGUE, country);
                        r.put_u8(field::LEAGUE + 1, code);
                    }
                    REQUEST_NATIONALITIES => r.put_u16(field::NATIONALITY, row as u16),
                    REQUEST_TRAITS => {
                        let data = traits.get(row).map_or(ANY, |&(_, d)| d);
                        r.put_u32(
                            field::TRAITS,
                            if data == ANY { ANY_MASK } else { 1 << (data & 31) },
                        );
                    }
                    // An ability sets only its own mask.
                    REQUEST_ABILITIES => match abilities.get(row).map_or(ANY, |&(_, d)| d) {
                        ANY => {
                            r.put_u32(field::ABILITIES, ANY_MASK);
                            r.put_u32(field::KEEPER_ABILITIES, ANY_MASK);
                        }
                        d if d & KEEPER_ABILITY != 0 => {
                            r.put_u32(field::KEEPER_ABILITIES, 1 << (d & 31))
                        }
                        d => r.put_u32(field::ABILITIES, 1 << (d & 31)),
                    },
                    _ => {}
                }
            }
            _ => {}
        }
        Outcome::Stay
    }
}

impl NoracsaRequest {
    /// The traits and abilities of a keeper or a field player, each at the
    /// choice the request holds, else "egal" (`FUN_005270c1`).
    fn fill_lists(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(r) = &self.request else { return };
        let (traits, abilities) = lists(self.keeper_lists, cx);
        let ability_mask =
            r.u32_at(if self.keeper_lists { field::KEEPER_ABILITIES } else { field::ABILITIES });
        let lists = [
            (REQUEST_TRAITS, traits, r.u32_at(field::TRAITS)),
            (REQUEST_ABILITIES, abilities, ability_mask),
        ];
        for (id, list, mask) in lists {
            let chosen =
                list.iter().position(|&(_, d)| d != ANY && 1u32 << (d & 31) == mask).unwrap_or(0);
            if let Some(combo) = control(dialog, id).and_then(|c| view.combo_mut(c)) {
                combo.items = list.iter().map(|&(t, _)| game.text(t).to_owned()).collect();
                combo.selected = Some(chosen);
            }
        }
    }
}

/// A combo's choices: text and data.
type Choices<'a> = &'a [(u16, u32)];

/// The traits' and abilities' choices of a keeper or of a field player.
fn lists<'a>(keeper: bool, cx: &Cx<'a>) -> (Choices<'a>, Choices<'a>) {
    let tables = &cx.game.rules.noracsa;
    if keeper {
        (&tables.keeper_traits, &tables.keeper_abilities)
    } else {
        (&tables.field_traits, &tables.field_abilities)
    }
}

fn show_talent(cx: &Cx, view: &mut View, talent: i8) {
    let picture = TALENT_PICTURES[usize::try_from(talent).unwrap_or(0).min(4)];
    if let Some(bitmap) = control(cx.dialog, TALENT_PICTURE).and_then(|c| view.bitmap_mut(c)) {
        bitmap.picture = Some(crate::controls::bitmap::Picture::graphic(picture));
    }
}

/// The manager's record, and to change.
fn own_manager<'a>(cx: &'a Cx) -> Option<&'a a3_core::records::Manager> {
    let current = cx.state.season.as_ref()?.current;
    let records = cx.state.records.as_ref()?;
    records.managers.get(records.human_slot(current)?)
}

fn own_manager_mut<'a>(cx: &'a mut Cx) -> Option<&'a mut a3_core::records::Manager> {
    let current = cx.state.season.as_ref()?.current;
    let records = cx.state.records.as_mut()?;
    let slot = records.human_slot(current)?;
    records.managers.get_mut(slot)
}
