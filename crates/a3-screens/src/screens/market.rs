//! The transfer list (dialog 287, `CTransferlisteDialog`; TRANSFERLISTE.md
//! in the RE repo): every player listed for sale or loan, by position and
//! strength, narrowed by league or country, by new or own players and by
//! a least strength; the club's own players to take off the list, any
//! player to watch.

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::{
    APPOINTMENTS, CURRENT_SQUAD, NOTICE, QUESTION, TRANSFER_LIST, WATCH_LIST, WATCHED_PLAYER,
    WORLDS_BEST,
};
use a3_core::finances::Balance as Figures;
use a3_core::records::Records;
use a3_core::transfers::{self, LOAN, LOAN_WITH_OPTION, Listing, Watch};
use a3_files::rch::Dialog;
use tracing::info;

use super::finances::money64;
use super::hub::{self, Command};
use super::playerinfo::money;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::{Grip, Target};
use crate::view::View;

const CAPTION_ID: u16 = 9936;
/// The list, the leagues' combo, all/new/own, the squad's button, the
/// least strength's spinner and figure, the count, the most to invest.
const LIST: u16 = 3937;
const LEAGUES: u16 = 3939;
const SHOW: u16 = 5658;
const SQUAD_BUTTON: u16 = 3940;
const STRENGTH_SPIN: u16 = 5660;
const STRENGTH: u16 = 5659;
const COUNT: u16 = 3936;
const INVESTABLE: u16 = 3941;
/// What the combo offers, by its values: all, the three leagues, the five
/// countries played, the rest abroad (the table at 0x88bd50).
const ALL_TEXT: u16 = 2345;
const LEAGUE_TEXTS: [u16; 3] = [2346, 2347, 2348];
const ABROAD_TEXT: u16 = 2349;
const ALL: i16 = 0;
const FIRST_COUNTRY: i16 = 4;
const COUNTRIES: i16 = 5;
const ABROAD: i16 = 9;
/// All, new, own.
const NEW: i16 = 1;
const OWN: i16 = 2;
/// A listing is new for six days.
const NEW_DAYS: i16 = 6;
/// The least strength's range.
const STRENGTHS: (i32, i32) = (1, 13);
/// The count line: none, one, and the dialog file's format for more.
const NONE_LISTED: u16 = 2392;
const ONE_LISTED: u16 = 2393;
/// "neu", the kinds of listing, "Kauf", "Frei", "ablösefrei", "Nein", the
/// weekdays from Sunday.
const NEW_TEXT: u16 = 2178;
const SALE_TEXT: u16 = 2217;
const LOAN_TEXT: u16 = 2218;
const OPTION_TEXT: u16 = 2219;
const FREE_TEXT: u16 = 3311;
const FREE_TRANSFER_TEXT: u16 = 4806;
const NO: u16 = 1029;
const WEEKDAYS: u16 = 1779;
/// After this day of the season a contract's last year counts as over.
const LATE: u16 = 216;
/// The rows' inks: the club's own players, the watched, the rest.
const OWN_INK: [u8; 3] = [255, 0, 0];
const WATCHED_INK: [u8; 3] = [0, 160, 0];
const OTHER_INK: [u8; 3] = [0, 0, 0];
/// An own player's refusals and the question to take him off.
const HAS_TALK: u16 = 3217;
const STAYS_LISTED: u16 = 3190;
const TAKE_OFF: u16 = 4072;
/// The watch list's: an own player, full, there already; and what
/// watching him again costs (`FUN_00418ab2`).
const IN_SQUAD: u16 = 3053;
const WATCH_FULL: u16 = 2436;
const WATCHED_ALREADY: u16 = 3052;
const WATCH_COST: i32 = 10_000;
/// The team record's observation cost (the club object's +0x27c).
const OBSERVATION_COST: usize = 0x4f4;
/// The row menu: info and the watch list.
const ROW_MENU: [Command; 2] = [Command::Info, Command::Watch];

/// The caption; the row menu.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    if let Some(dialog) = dialogs.get_mut(&TRANSFER_LIST) {
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
        dialog.controls.push(hub::menu_control());
    }
}

/// The transfer list.
#[derive(Default)]
pub struct TransferList {
    /// The filters: the combo's value, all/new/own, the least strength;
    /// the combo's values by its items.
    leagues: i16,
    show: i16,
    least: i16,
    values: Vec<i16>,
    /// The player whose menu is open, and the one asked about.
    menu_player: Option<u16>,
    asking: Option<u16>,
}

impl Screen for TransferList {
    fn ids(&self) -> &[u16] {
        &[TRANSFER_LIST]
    }

    /// The filters as the manager left them (`FUN_005a6616`).
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.menu_player = None;
        self.asking = None;
        let Some(records) = cx.state.records.as_ref() else { return };
        let manager = own_manager(cx, records);
        self.least = manager.map_or(records.globals.least_strength(), |m| m.least_strength());
        self.leagues = manager.map_or(ALL, |m| m.market_filter());
        self.show = manager.map_or(0, |m| m.market_show());
        let today = season_date(cx);
        let title = dialog.title.replacen("%s", &hub::date_text(game, today), 1);
        set(view, dialog, CAPTION_ID, title);
        if let Some(spin) = control(dialog, STRENGTH_SPIN).and_then(|c| view.value_mut(c)) {
            spin.range = STRENGTHS;
            spin.set(self.least.into());
        }
        set(view, dialog, STRENGTH, self.least.to_string());
        if let Some(c) = control(dialog, SHOW) {
            view.set_chosen(c, usize::try_from(self.show).unwrap_or(0));
        }
        if let Some(options) = control(dialog, SQUAD_BUTTON).and_then(|c| view.options_mut(c)) {
            options.actions = true;
        }
        self.fill(cx, view);
        self.fill_leagues(cx, view);
        if let Some((records, team)) = own_team(cx) {
            let figures = Figures::of(team, records.globals.fund_prices(), &records.agreed());
            set(view, dialog, INVESTABLE, money64(game, figures.investable()));
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some(id) = self.menu_player.take() {
            return match hub::close_menu(cx, view, target).and_then(|i| ROW_MENU.get(i)) {
                Some(Command::Info) => {
                    // The info pages through the rows backwards (`AddHead`).
                    let rows = control(dialog, LIST).and_then(|l| view.list(l));
                    cx.state.paging_ids =
                        rows.map(|l| l.keys.iter().rev().map(|&k| k as u16).collect());
                    Outcome::Popup(Request::with(super::playerinfo::CURRENT, Args::Record(id)))
                }
                Some(Command::Watch) => watch_player(cx, id),
                _ => Outcome::Stay,
            };
        }
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                // The least strength is kept as the list is left.
                let least = self.least;
                let Some(records) = cx.state.records.as_mut() else {
                    return Outcome::Close(Reply::Ok);
                };
                records.globals.set_least_strength(least);
                if let Some(m) = own_manager_mut(cx) {
                    m.set_least_strength(least);
                }
                Outcome::Close(Reply::Ok)
            }
            Target::Option { control: c, item } if id(c) == Some(SHOW) => {
                // "Neuzugänge" refills even when it is lit.
                let show = item as i16;
                if show != self.show || show == NEW {
                    self.show = show;
                    self.fill(cx, view);
                    if let Some(m) = own_manager_mut(cx) {
                        m.set_market_show(show);
                    }
                }
                Outcome::Stay
            }
            Target::Option { control: c, .. } if id(c) == Some(SQUAD_BUTTON) => {
                Outcome::Popup(Request::new(CURRENT_SQUAD))
            }
            Target::Row { control: c, .. } if id(c) == Some(LEAGUES) => {
                let chosen = view.combo(c).and_then(|combo| combo.selected);
                let value = chosen.and_then(|i| self.values.get(i).copied());
                if let Some(value) = value.filter(|&v| v != self.leagues) {
                    self.leagues = value;
                    self.fill(cx, view);
                    if let Some(m) = own_manager_mut(cx) {
                        m.set_market_filter(value);
                    }
                }
                Outcome::Stay
            }
            Target::Spin { control: c, .. } if id(c) == Some(STRENGTH_SPIN) => {
                self.least = view.value(c).map_or(self.least, |v| v.value as i16);
                set(view, dialog, STRENGTH, self.least.to_string());
                self.fill(cx, view);
                Outcome::Stay
            }
            Target::Menu(grip @ Grip::Row { control: c, row }) if id(c) == Some(LIST) => {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                if let Some(player) = player
                    && hub::show_menu(cx, view, grip, &ROW_MENU).is_some()
                {
                    self.menu_player = Some(player as u16);
                }
                Outcome::Stay
            }
            Target::Row { control: c, row } if id(c) == Some(LIST) => {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                match player {
                    Some(player) => self.click(cx, player as u16),
                    None => Outcome::Stay,
                }
            }
            _ => Outcome::Stay,
        }
    }

    /// The question to take an own player off: a yes does, and the list
    /// is filled anew.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if let Some(id) = self.asking.take()
            && reply == Reply::Ok
        {
            if let Some(records) = cx.state.records.as_mut() {
                transfers::take_off(records, id);
            }
            self.fill(cx, view);
        }
        Outcome::Stay
    }
}

impl TransferList {
    /// The rows (`FUN_005a6b26`): the list sorted, then narrowed by the
    /// combo, by all/new/own and by the least strength as it is now; the
    /// scroll kept, no row selected; the count of all listed below.
    fn fill(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(season) = cx.state.season.as_ref() else { return };
        let today = records.globals.day() as i16;
        let late = records.globals.day() > LATE;
        let year = i32::from(records.globals.start_year());
        let own = own_team(cx).map(|(_, t)| (t.country(), u16::from(t.number())));
        let watched = own_team(cx)
            .and_then(|(_, t)| usize::from(t.human()).checked_sub(1))
            .map_or_else(Vec::new, |slot| transfers::watched(records, slot));
        let listed = transfers::sorted(transfers::listings(records));
        let mut rows = Vec::new();
        let mut keys = Vec::new();
        let mut inks = Vec::new();
        for e in &listed {
            if e.id == 0 || !self.reads(records, e) {
                continue;
            }
            let shown = match self.show {
                NEW => e.day + NEW_DAYS >= today,
                OWN => own == Some((e.country, e.number)),
                _ => true,
            };
            if !shown {
                continue;
            }
            let Some((player, _, status)) = a3_core::load::any_player(records, &game.data, e.id)
            else {
                continue;
            };
            if i16::from(player.strength) < self.least {
                continue;
            }
            let record = records.player(e.id);
            let weeks = if e.day + NEW_DAYS < today {
                ((today - e.day + NEW_DAYS) / 7).to_string()
            } else {
                game.text(NEW_TEXT).to_owned()
            };
            let [main, second, third] = player.positions;
            let sides = match (second, third) {
                (0, _) => " ".to_owned(),
                (s, 0) => hub::abbreviation(game, s),
                (s, t) => format!("{}/{}", hub::abbreviation(game, s), hub::abbreviation(game, t)),
            };
            let nation = game.world.nation_codes.get(usize::from(status.nations[0])).cloned();
            let club = game.club_name(Some(records), e.country, e.number as i16);
            let years = status.contract.unwrap_or(0).clamp(0, i32::from(i8::MAX)) as i8;
            let kind = if years == 0 || (years == 1 && late) {
                game.text(FREE_TEXT).to_owned()
            } else {
                match e.kind {
                    LOAN => game.text(LOAN_TEXT).replacen("%s", &money(game, e.fee.into()), 1),
                    LOAN_WITH_OPTION => {
                        game.text(OPTION_TEXT).replacen("%s", &money(game, e.fee.into()), 1)
                    }
                    _ if e.terms == transfers::FREE_TRANSFER => {
                        game.text(FREE_TRANSFER_TEXT).to_owned()
                    }
                    _ => format!("{} ({})", game.text(SALE_TEXT), year + i32::from(years)),
                }
            };
            let talk = match transfers::talk_day(records, e.id) {
                Some(day) => game.text(WEEKDAYS + day.rem_euclid(7) as u16).to_owned(),
                None => game.text(NO).to_owned(),
            };
            rows.push(
                [
                    super::duties::short_name(cx, &player, e.id),
                    weeks,
                    hub::abbreviation(game, main),
                    sides,
                    player.strength.to_string(),
                    player.age(season.date()).to_string(),
                    nation.unwrap_or_default(),
                    club,
                    kind,
                    talk,
                ]
                .join("\t"),
            );
            let at_own = record.is_some_and(|r| {
                let [country, number] = r.club();
                own == Some((country, u16::from(number)))
            });
            let ink = if at_own {
                OWN_INK
            } else if watched.contains(&e.id) {
                WATCHED_INK
            } else {
                OTHER_INK
            };
            inks.push(Some(ink));
            keys.push(usize::from(e.id));
        }
        if let Some(c) = control(dialog, LIST) {
            let visible = crate::controls::list::visible_rows(&dialog.controls[c]);
            if let Some(list) = view.list_mut(c) {
                list.scroll = list.scroll.min(rows.len().saturating_sub(visible));
                list.selected = None;
                list.rows = rows;
                list.keys = keys;
                list.inks = inks;
            }
        }
        let line = match listed.len() {
            0 => game.text(NONE_LISTED).to_owned(),
            1 => game.text(ONE_LISTED).to_owned(),
            n => control(dialog, COUNT)
                .map(|c| dialog.controls[c].text.replacen("%d", &n.to_string(), 1))
                .unwrap_or_default(),
        };
        set(view, dialog, COUNT, line);
    }

    /// Whether the combo's choice takes a listing in: its club's league,
    /// a country played, the rest (`FUN_0075988a`).
    fn reads(&self, records: &Records, e: &Listing) -> bool {
        let globals = &records.globals;
        let played = globals.played(e.country.into());
        match self.leagues {
            1..=3 => {
                played
                    && league_level(records, e.country, e.number).min(2)
                        == i32::from(self.leagues - 1)
            }
            FIRST_COUNTRY => i16::from(e.country) == globals.main_country(),
            5..=8 => {
                let k = usize::try_from(self.leagues - 5).unwrap_or(0);
                globals.more_count() >= self.leagues - 4
                    && i16::from(e.country) == globals.more_countries()[k]
            }
            ABROAD => !played,
            _ => true,
        }
    }

    /// The combo's items: all, the leagues, the countries played, abroad;
    /// the filter's chosen (`FUN_005a7b12`).
    fn fill_leagues(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        let mut items = vec![(ALL, game.text(ALL_TEXT).to_owned())];
        for (i, &text) in LEAGUE_TEXTS.iter().enumerate() {
            items.push((i as i16 + 1, game.text(text).to_owned()));
        }
        let more = records.globals.more_count();
        for slot in 1..=COUNTRIES {
            if slot <= more + 1 {
                let country = records.countries.slot_country(usize::try_from(slot).unwrap_or(0));
                let name = game.world.nation(u16::try_from(country).unwrap_or(0)).to_owned();
                items.push((FIRST_COUNTRY + slot - 1, name));
            }
        }
        items.push((ABROAD, game.text(ABROAD_TEXT).to_owned()));
        self.values = items.iter().map(|&(v, _)| v).collect();
        let chosen = self.values.iter().position(|&v| v == self.leagues);
        if let Some(combo) = control(dialog, LEAGUES).and_then(|c| view.combo_mut(c)) {
            combo.items = items.into_iter().map(|(_, t)| t).collect();
            combo.selected = chosen;
        }
    }

    /// A row clicked (`OnSelChange_IDC_TRANSFERLISTE_LISTE`): an own
    /// player has an appointment, will not come off, or is asked about;
    /// another club's player is not built yet.
    fn click(&mut self, cx: &Cx, id: u16) -> Outcome {
        let Some(records) = cx.state.records.as_ref() else { return Outcome::Stay };
        let own = own_team(cx).map(|(_, t)| (t.country(), u16::from(t.number())));
        let at_own = records.player(id).is_some_and(|r| {
            let [country, number] = r.club();
            own == Some((country, u16::from(number)))
        });
        if !at_own {
            info!("Verhandlungen ({id}): not built yet");
            return Outcome::Stay;
        }
        let notice = |text: u16| Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
        if transfers::talk_day(records, id).is_some() {
            return notice(HAS_TALK);
        }
        let listing = transfers::listings(records).into_iter().find(|e| e.id == id);
        if listing.is_some_and(|e| e.terms != 1) {
            return notice(STAYS_LISTED);
        }
        let years = records.player(id).map_or(0, |r| {
            i32::from(r.contract() & 0xfff) - i32::from(records.globals.start_year())
        });
        if years > 1 || (years == 1 && records.globals.day() <= LATE) {
            self.asking = Some(id);
            return Outcome::Popup(Request::with(QUESTION, Args::TextFor(TAKE_OFF, id)));
        }
        Outcome::Stay
    }
}

/// A player put on the club's watch list (`FUN_00418ab2`): not one of
/// its own; when full, told; when there already, told and charged.
pub(crate) fn watch_player(cx: &mut Cx, id: u16) -> Outcome {
    let notice = |text: u16| Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
    let Some(season) = cx.state.season.as_ref() else { return Outcome::Stay };
    let in_squad = season
        .manager()
        .and_then(|m| m.club)
        .and_then(|k| season.clubs.get(&k))
        .is_some_and(|c| c.status.iter().any(|s| s.id == id));
    if in_squad {
        return notice(IN_SQUAD);
    }
    let Some(index) = own_index(cx) else { return Outcome::Stay };
    let Some(records) = cx.state.records.as_mut() else { return Outcome::Stay };
    let Some(slot) = usize::from(records.teams[index].human()).checked_sub(1) else {
        return Outcome::Stay;
    };
    match transfers::watch(records, slot, id) {
        Watch::Added => Outcome::Stay,
        Watch::Full => notice(WATCH_FULL),
        Watch::Already => {
            let team = &mut records.teams[index];
            let cost = team.observation_cost().wrapping_add(WATCH_COST);
            team.put(OBSERVATION_COST, cost);
            notice(WATCHED_ALREADY)
        }
    }
}

/// How high a club plays: its league for a country with leagues, ten for
/// its amateur clubs, none abroad (`FUN_0063e2df`).
pub(crate) fn league_level(records: &Records, country: u8, number: u16) -> i32 {
    const AMATEURS: i32 = 10;
    let countries = &records.countries;
    let c = usize::from(country);
    match countries.kind(c) {
        1 | 2 if (number as i16) < countries.team_count(c) => {
            records.team(country.into(), number).map_or(AMATEURS, |t| i32::from(t.level()))
        }
        1 | 2 => AMATEURS,
        _ => 0,
    }
}

fn season_date(cx: &Cx) -> chrono::NaiveDate {
    cx.state.season.as_ref().map_or_else(|| a3_core::calendar::season_start(2025), |s| s.date())
}

/// The manager's club's team's index.
fn own_index(cx: &Cx) -> Option<usize> {
    let (slot, number) = cx.state.season.as_ref()?.manager()?.club?;
    let nation = cx.game.world.country(slot)?.nation;
    cx.state.records.as_ref()?.team_index(nation, u16::try_from(number).ok()?)
}

fn own_team<'a>(cx: &'a Cx) -> Option<(&'a Records, &'a a3_core::records::Team)> {
    let records = cx.state.records.as_ref()?;
    Some((records, records.teams.get(own_index(cx)?)?))
}

fn own_manager<'a>(cx: &Cx, records: &'a Records) -> Option<&'a a3_core::records::Manager> {
    let current = cx.state.season.as_ref()?.current;
    records.managers.get(records.human_slot(current)?)
}

fn own_manager_mut<'a>(cx: &'a mut Cx) -> Option<&'a mut a3_core::records::Manager> {
    let current = cx.state.season.as_ref()?.current;
    let records = cx.state.records.as_mut()?;
    let slot = records.human_slot(current)?;
    records.managers.get_mut(slot)
}

/// The world's best players (dialog 462, `CWeltbestenSpieler`): the best
/// of every position, with up to three of his abilities, and of every
/// ability, the keepers' last.
#[derive(Default)]
pub struct WorldsBest;

const BY_POSITION: u16 = 6016;
const BY_ABILITY: u16 = 6017;
const BEST_CAPTION: u16 = 9937;
const POSITIONS: u8 = 10;

pub(crate) fn compose_best(dialogs: &mut HashMap<u16, Dialog>) {
    if let Some(dialog) = dialogs.get_mut(&WORLDS_BEST) {
        dialog.controls.insert(0, super::caption(dialog.frame, BEST_CAPTION, dialog.title.clone()));
    }
}

impl Screen for WorldsBest {
    fn ids(&self) -> &[u16] {
        &[WORLDS_BEST]
    }

    /// Both lists (`FUN_005cbac5`).
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        set(view, dialog, BEST_CAPTION, dialog.title.clone());
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(season) = cx.state.season.as_ref() else { return };
        let pitch = &game.rules.pitch;
        let today = season.date();
        let year = i32::from(records.globals.start_year());
        let stars = records.globals.stars();
        // A best player's figures after his position or ability: name,
        // age, his position (the ability list's), nation, club, value,
        // the contract's end.
        let row = |id: u16, position: bool| -> Option<(String, a3_core::world::Player)> {
            let (player, _, status) = a3_core::load::any_player(records, &game.data, id)?;
            let r = records.player(id)?;
            let years = status.contract.unwrap_or(0).max(0);
            let (value, until) = if years >= 1 {
                let value = super::playerinfo::market_value(game, &player, r, stars, years, today);
                (money(game, value), (year + years).to_string())
            } else {
                ("-".to_owned(), "-".to_owned())
            };
            let [country, number] = r.club();
            let fields = [
                records.known_as(id).unwrap_or_default(),
                player.age(today).to_string(),
                if position { hub::abbreviation(game, player.positions[0]) } else { String::new() },
                game.world
                    .nation_codes
                    .get(usize::from(status.nations[0]))
                    .cloned()
                    .unwrap_or_default(),
                game.club_name(Some(records), country, number.into()),
                value,
                until,
            ];
            Some((fields.join("\t"), player))
        };
        let mut rows = Vec::new();
        let mut icons = Vec::new();
        let mut keys = Vec::new();
        for position in 1..=POSITIONS {
            let id = stars[usize::from(position)];
            if id == 0 {
                continue;
            }
            let Some((text, player)) = row(id, false) else { continue };
            // Position, name, age, nation, club, value, contract.
            let mut fields: Vec<&str> = text.split('\t').collect();
            fields.remove(2);
            let abbreviation = hub::abbreviation(game, position);
            rows.push(format!("{abbreviation}\t{}", fields.join("\t")));
            icons.push(pitch.ability_icons(&player));
            keys.push(usize::from(id));
        }
        fill_list(view, dialog, BY_POSITION, rows, icons, keys);
        let (mut rows, mut icons, mut keys) = (Vec::new(), Vec::new(), Vec::new());
        let field = records.globals.best_by_ability().map(|id| (id, false));
        let keepers = records.globals.best_keepers_by_ability().map(|id| (id, true));
        for (ability, (id, keeper)) in field
            .iter()
            .enumerate()
            .chain(keepers.iter().enumerate())
            .map(|(a, &(id, keeper))| (a, (id, keeper)))
        {
            let Some(id) = u16::try_from(id).ok().filter(|&id| id != 0) else { continue };
            let Some((text, _)) = row(id, true) else { continue };
            rows.push(format!("\t{text}"));
            icons.push(pitch.ability_icon(keeper, true, ability).into_iter().collect());
            keys.push(usize::from(id));
        }
        fill_list(view, dialog, BY_ABILITY, rows, icons, keys);
        mark_inert(view, dialog, |_| true);
    }

    /// A row clicked: negotiations, not built yet; the row does not stay
    /// chosen (`OnSelChange_IDC_WELTBESTER_POSITIONEN_005cc740`).
    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Row { control: c, .. } if view.list(c).is_some() => {
                if let Some(list) = view.list_mut(c) {
                    list.selected = None;
                }
                info!("Verhandlungen: not built yet");
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

/// A list's rows, each with its icons in the list's icon slots.
fn fill_list(
    view: &mut View,
    dialog: &Dialog,
    id: u16,
    rows: Vec<String>,
    icons: Vec<Vec<u16>>,
    keys: Vec<usize>,
) {
    let Some(c) = control(dialog, id) else { return };
    let slots = crate::controls::list::icon_slots(&dialog.controls[c]);
    if let Some(list) = view.list_mut(c) {
        list.inks = vec![None; rows.len()];
        list.rows = rows;
        list.row_icons =
            icons.into_iter().map(|row| slots.iter().copied().zip(row).collect()).collect();
        list.keys = keys;
        list.selected = None;
    }
}

/// The clubs human managers have, as (country, number).
fn human_clubs(records: &Records) -> Vec<(u8, i16)> {
    records.humans().map(|m| (m.country() as u8, m.team() as i16)).collect()
}

/// The club of a player's record, now.
fn present_club(records: &Records, id: u16) -> (u8, i16) {
    records.player(id).map_or((0, 0), |r| {
        let [country, number] = r.club();
        (country, i16::from(number))
    })
}

/// A player's name to nation, as the market's lists write them.
fn short_columns(cx: &Cx, id: u16) -> Option<(Vec<String>, a3_core::world::Player)> {
    let (game, records) = (cx.game, cx.state.records.as_ref()?);
    let (player, fitness, status) = a3_core::load::any_player(records, &game.data, id)?;
    let today = season_date(cx);
    let [main, second, third] = player.positions;
    let sides = match (second, third) {
        (0, _) => " ".to_owned(),
        (a, 0) => hub::abbreviation(game, a),
        (a, b) => format!("{}/{}", hub::abbreviation(game, a), hub::abbreviation(game, b)),
    };
    let picks = records.player(id).map_or(0, |r| r.picks());
    let unknown = matches!((picks & 0xc0) >> 6, 1 | 3);
    let strength = if unknown { "?".to_owned() } else { player.strength.to_string() };
    let pitch = &game.rules.pitch;
    let columns = vec![
        super::duties::short_name(cx, &player, id),
        hub::abbreviation(game, main),
        sides,
        strength,
        a3_core::lineup::form(fitness).to_string(),
        hub::tenths(pitch.strength(&player, fitness, status.doping)),
        player.age(today).to_string(),
        game.world.nation_codes.get(usize::from(status.nations[0])).cloned().unwrap_or_default(),
    ];
    Some((columns, player))
}

/// The row menu with the player's info alone.
const INFO_ONLY: [Command; 1] = [Command::Info];

/// The week's appointments (dialog 25, `CBekannteTermine`;
/// TERMINE-BEOBACHTUNG.md §2): every public negotiation of the week, by
/// the player's main position; red where the club's player is talked
/// about or a human club takes part.
#[derive(Default)]
pub struct Appointments {
    menu_player: Option<u16>,
}

const TALKS_LIST: u16 = 1313;
const TALKS_CAPTION: u16 = 9938;
const RELEASE_FEE_TEXT: u16 = 2609;
const PURCHASE_TEXT: u16 = 3069;
const LOAN_KIND_TEXT: u16 = 3070;
const ALREADY_IN: u16 = 2350;
const GERMANY: u8 = 14;

pub(crate) fn compose_talks(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [APPOINTMENTS, WATCH_LIST, WATCHED_PLAYER] {
        if let Some(dialog) = dialogs.get_mut(&id) {
            let caption = super::caption(dialog.frame, TALKS_CAPTION, dialog.title.clone());
            dialog.controls.insert(0, caption);
            if id != WATCHED_PLAYER {
                dialog.controls.push(hub::menu_control());
            }
        }
    }
}

/// An exchange sort as the appointments' (`FUN_00762ba3`): every later row
/// that goes before swaps places.
pub(crate) fn swap_sort<T>(rows: &mut [T], before: impl Fn(&T, &T) -> bool) {
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            if before(&rows[j], &rows[i]) {
                rows.swap(i, j);
            }
        }
    }
}

impl Screen for Appointments {
    fn ids(&self) -> &[u16] {
        &[APPOINTMENTS]
    }

    /// The rows (`FUN_0047c6f7`); the dialog file's caption.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.menu_player = None;
        set(view, dialog, TALKS_CAPTION, dialog.title.clone());
        let Some(records) = cx.state.records.as_ref() else { return };
        let late = records.globals.day() > LATE;
        let year = i32::from(records.globals.start_year());
        let own = own_team(cx).map(|(_, t)| (t.country(), i16::from(t.number())));
        let humans = human_clubs(records);
        let position = |id: u16| records.player(id).map_or(0, |r| r.positions()[0]);
        let mut talks = transfers::appointments(records);
        swap_sort(&mut talks, |a, b| position(a.player) < position(b.player));
        let mut rows = Vec::new();
        let mut inks = Vec::new();
        let mut keys = Vec::new();
        for talk in talks.iter().filter(|t| t.kind != transfers::POACHING) {
            let Some((player_columns, _)) = short_columns(cx, talk.player) else { continue };
            let (country, number) = present_club(records, talk.player);
            let level = league_level(records, country, number as u16).min(2);
            let r = records.player(talk.player);
            let years = r.map_or(0, |r| i32::from(r.contract() & 0xfff) - year);
            let kind = match talk.kind {
                transfers::RELEASE_FEE_TALK if country == GERMANY && level >= 2 => {
                    game.text(RELEASE_FEE_TEXT).to_owned()
                }
                transfers::RELEASE_FEE_TALK => {
                    money(game, i64::from(r.map_or(0, |r| r.fixed_fee())) * 10_000)
                }
                transfers::PURCHASE if years <= 0 || (years == 1 && late) => {
                    game.text(FREE_TEXT).to_owned()
                }
                transfers::PURCHASE => format!("{} ({})", game.text(PURCHASE_TEXT), year + years),
                transfers::LOAN_TALK => game.text(LOAN_KIND_TEXT).to_owned(),
                _ => "-".to_owned(),
            };
            let day = if talk.day >= 0 {
                game.text(WEEKDAYS + talk.day.rem_euclid(7) as u16).to_owned()
            } else {
                game.text(NO).to_owned()
            };
            // Name to strength, then the league, the age, the club, the
            // kind and the day.
            let mut columns = player_columns[..4].to_vec();
            columns.extend([
                (level + 1).to_string(),
                player_columns[6].clone(),
                game.club_name(Some(records), country, number),
                kind,
                day,
            ]);
            rows.push(columns.join("\t"));
            let ours = own == Some((country, number));
            let human = talk.clubs.iter().any(|c| humans.contains(c));
            inks.push(Some(if ours || human { OWN_INK } else { OTHER_INK }));
            keys.push(usize::from(talk.player));
        }
        if let Some(list) = control(dialog, TALKS_LIST).and_then(|c| view.list_mut(c)) {
            list.rows = rows;
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
            return info_menu(cx, view, target, TALKS_LIST, id);
        }
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Menu(grip @ Grip::Row { control: c, row })
                if dialog.controls[c].id == Some(TALKS_LIST) =>
            {
                self.menu_player = open_info_menu(cx, view, grip, c, row);
                Outcome::Stay
            }
            // The transfer list's request (`OnSelChange_IDC_BEKANNTE_LISTE`):
            // a club already in the talks is told so; joining is not built.
            Target::Row { control: c, row } if dialog.controls[c].id == Some(TALKS_LIST) => {
                let Some(records) = cx.state.records.as_ref() else { return Outcome::Stay };
                let Some(id) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied())
                else {
                    return Outcome::Stay;
                };
                let id = id as u16;
                if !transfers::listings(records).iter().any(|e| e.id == id) {
                    return Outcome::Stay;
                }
                let own = own_team(cx).map(|(_, t)| (t.country(), i16::from(t.number())));
                let taking_part = transfers::appointments(records)
                    .iter()
                    .any(|t| t.player == id && own.is_some_and(|o| t.clubs.contains(&o)));
                if taking_part {
                    return Outcome::Popup(Request::with(NOTICE, Args::Text(ALREADY_IN)));
                }
                info!("Verhandlungen ({id}): not built yet");
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

/// A right click on a list's row: the player's menu with his info.
fn open_info_menu(cx: &Cx, view: &mut View, grip: Grip, list: usize, row: usize) -> Option<u16> {
    let player = view.list(list).and_then(|l| l.keys.get(l.scroll + row).copied())?;
    hub::show_menu(cx, view, grip, &INFO_ONLY)?;
    Some(player as u16)
}

/// The menu's choice: the info, paging through the list's rows backwards.
fn info_menu(cx: &mut Cx, view: &mut View, target: Target, list: u16, id: u16) -> Outcome {
    if hub::close_menu(cx, view, target).and_then(|i| INFO_ONLY.get(i)).is_none() {
        return Outcome::Stay;
    }
    let rows = control(cx.dialog, list).and_then(|l| view.list(l));
    cx.state.paging_ids = rows.map(|l| l.keys.iter().rev().map(|&k| k as u16).collect());
    Outcome::Popup(Request::with(super::playerinfo::CURRENT, Args::Record(id)))
}

/// The watch list (dialog 27, `CBeobachtungslisteHaupt`;
/// TERMINE-BEOBACHTUNG.md §4): every watched player with his marks and
/// form; a click asks what to do with him; five scouts to send.
#[derive(Default)]
pub struct WatchList {
    menu_player: Option<u16>,
    /// The scouts' questions: the cost, then where to.
    asking: Option<(Scout, usize)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scout {
    Cost,
    Sent,
}

const WATCH_ROWS: u16 = 1330;
const WATCH_COUNT: u16 = 1329;
const REGIONS: u16 = 5843;
const NONE_WATCHED: u16 = 2611;
const ONE_WATCHED: u16 = 2612;
/// The marks: substituted, came on, banned, injured; a grade's half.
const MARK_TEXTS: [(u8, u16); 4] = [(0x20, 2171), (0x10, 2172), (0x80, 2174), (0x08, 2173)];
const PLAYED: u8 = 0x40;
const GRADE: u8 = 0x0f;
/// The scouts: busy there, busy elsewhere, the cost and where to.
const SCOUT_THERE: u16 = 4232;
const SCOUT_OUT: u16 = 4627;
const SCOUT_COST: u16 = 4626;
const SCOUT_SENT: u16 = 4133;
const REGION_NAMES: u16 = 4134;
const SCOUT_PRICE: i32 = 50_000;
/// The team's staff costs (the club object's +0x280).
const STAFF_COSTS: usize = 0x4f8;

impl Screen for WatchList {
    fn ids(&self) -> &[u16] {
        &[WATCH_LIST]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.menu_player = None;
        self.asking = None;
        if let Some(season) = cx.state.season.as_ref() {
            set(view, dialog, TALKS_CAPTION, hub::manager_title_of(game, season, &dialog.title));
        }
        if let Some(c) = control(dialog, REGIONS) {
            view.set_chosen(c, crate::controls::options::NONE);
        }
        self.fill(cx, view);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some(id) = self.menu_player.take() {
            return info_menu(cx, view, target, WATCH_ROWS, id);
        }
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Menu(grip @ Grip::Row { control: c, row }) if id(c) == Some(WATCH_ROWS) => {
                self.menu_player = open_info_menu(cx, view, grip, c, row);
                Outcome::Stay
            }
            Target::Row { control: c, row } if id(c) == Some(WATCH_ROWS) => {
                match view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied()) {
                    Some(player) => {
                        Outcome::Popup(Request::with(WATCHED_PLAYER, Args::Record(player as u16)))
                    }
                    None => Outcome::Stay,
                }
            }
            // A scout sent: not where one is, nor while one is out
            // (`0x48067a`).
            Target::Option { control: c, item } if id(c) == Some(REGIONS) => {
                let Some(slot) = own_slot(cx) else { return Outcome::Stay };
                let Some(records) = cx.state.records.as_ref() else { return Outcome::Stay };
                let scouts = transfers::scouts(records, slot);
                let notice = |text| Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
                if scouts.get(item).is_some_and(|&d| d > 0) {
                    return notice(SCOUT_THERE);
                }
                if scouts.iter().any(|&d| d > 0) {
                    return notice(SCOUT_OUT);
                }
                self.asking = Some((Scout::Cost, item));
                let args = Args::TextForMoney(SCOUT_COST, SCOUT_PRICE.into());
                Outcome::Popup(Request::with(QUESTION, args))
            }
            _ => Outcome::Stay,
        }
    }

    /// The scout's questions; back from a watched player, the list anew if
    /// he was dealt with.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        match self.asking.take() {
            Some((Scout::Cost, region)) if reply == Reply::Ok => {
                self.asking = Some((Scout::Sent, region));
                let args = Args::TextForText(SCOUT_SENT, REGION_NAMES + region as u16);
                Outcome::Popup(Request::with(QUESTION, args))
            }
            Some((Scout::Sent, region)) => {
                let today = cx.state.records.as_ref().map_or(0, |r| r.globals.day() as i16);
                let (Some(slot), Some(index)) = (own_slot(cx), own_index(cx)) else {
                    return Outcome::Stay;
                };
                if let Some(records) = cx.state.records.as_mut() {
                    transfers::send_scout(records, slot, region, today);
                    let team = &mut records.teams[index];
                    let cost = team.observation_cost().wrapping_add(SCOUT_PRICE);
                    team.put(OBSERVATION_COST, cost);
                    let staff = team.get::<i32>(STAFF_COSTS).wrapping_add(SCOUT_PRICE);
                    team.put(STAFF_COSTS, staff);
                }
                Outcome::Stay
            }
            Some(_) => Outcome::Stay,
            None => {
                if reply == Reply::Ok {
                    self.fill(cx, view);
                }
                Outcome::Stay
            }
        }
    }
}

impl WatchList {
    /// The rows (`FUN_0047fdee`): every place with a player, in order;
    /// the list at its top; the count of rows below.
    fn fill(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(slot) = own_slot(cx) else { return };
        let year = i32::from(records.globals.start_year());
        let mut rows = Vec::new();
        let mut keys = Vec::new();
        for w in transfers::watch_list(records, slot) {
            let Some((mut columns, _)) = short_columns(cx, w.id) else { continue };
            let Some((_, fitness, status)) = a3_core::load::any_player(records, &game.data, w.id)
            else {
                continue;
            };
            let form = a3_core::lineup::form(fitness);
            let average = if w.matches != 0 {
                (f64::from(w.form_sum) / f64::from(w.matches)) as f32
            } else {
                form as f32
            };
            let marks: String = w
                .marks
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| match MARK_TEXTS.iter().find(|&&(bit, _)| b & bit != 0) {
                    Some(&(_, text)) => game.text(text).to_owned(),
                    None if b & PLAYED != 0 && b & GRADE == 0 => "-".to_owned(),
                    None if b & PLAYED != 0 => ((b & GRADE) / 2).to_string(),
                    None => String::new(),
                })
                .collect();
            let trend = match (form as f32).partial_cmp(&average) {
                Some(std::cmp::Ordering::Greater) => "+",
                Some(std::cmp::Ordering::Less) => "-",
                _ => " ",
            };
            let years = status.contract.unwrap_or(0);
            let until = if years >= 1 { (year + years).to_string() } else { "-".to_owned() };
            columns.extend([
                super::playerinfo::one_decimal(f64::from(average)),
                marks,
                trend.to_owned(),
                until,
            ]);
            rows.push(columns.join("\t"));
            keys.push(usize::from(w.id));
        }
        let count = rows.len();
        if let Some(list) = control(dialog, WATCH_ROWS).and_then(|c| view.list_mut(c)) {
            list.inks = vec![Some(OTHER_INK); count];
            list.rows = rows;
            list.keys = keys;
            list.scroll = 0;
            list.selected = None;
        }
        let line = match count {
            0 => game.text(NONE_WATCHED).to_owned(),
            1 => game.text(ONE_WATCHED).to_owned(),
            n => control(dialog, WATCH_COUNT)
                .map(|c| dialog.controls[c].text.replacen("%d", &n.to_string(), 1))
                .unwrap_or_default(),
        };
        set(view, dialog, WATCH_COUNT, line);
    }
}

/// The club's human slot, which its watch list is kept by.
fn own_slot(cx: &Cx) -> Option<usize> {
    own_team(cx).and_then(|(_, t)| usize::from(t.human()).checked_sub(1))
}

/// A watched player's choices (dialog 28, `CBeobachtungAngeklickt`):
/// negotiations, off the list, nothing.
#[derive(Default)]
pub struct WatchedPlayer {
    id: u16,
}

const CHOICES: u16 = 1333;
const NEGOTIATE: usize = 0;
const UNWATCH: usize = 1;
const QUESTION_FRAMES: std::ops::RangeInclusive<u16> = 1745..=1753;
const BALL: u16 = 1336;
/// The refusals the replica can tell, in the game's order
/// (`FUN_005a5ac2` with the flags 0x0dd00000).
const ON_LIST: u16 = 2360;
const RETIRING_TEXT: u16 = 3135;
const SIGNED_ELSEWHERE: u16 = 3077;
const TALK_THIS_WEEK: u16 = 2512;
const IN_OWN_TEAM: u16 = 4307;
/// A club books three negotiations a week at most.
const TALKS_A_WEEK: usize = 3;
const THREE_TALKS: u16 = 4237;

impl Screen for WatchedPlayer {
    fn ids(&self) -> &[u16] {
        &[WATCHED_PLAYER]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        set(view, dialog, TALKS_CAPTION, dialog.title.clone());
        if let Args::Record(id) = args {
            self.id = id;
        }
        if let Some(c) = control(dialog, CHOICES) {
            view.set_chosen(c, crate::controls::options::NONE);
        }
        question_ball(view, dialog);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let Target::Option { control: c, item } = target else { return Outcome::Stay };
        if cx.dialog.controls[c].id != Some(CHOICES) {
            return Outcome::Stay;
        }
        match item {
            NEGOTIATE => negotiate(cx, self.id),
            UNWATCH => {
                if let Some(slot) = own_slot(cx)
                    && let Some(records) = cx.state.records.as_mut()
                {
                    transfers::unwatch(records, slot, self.id);
                }
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Close(Reply::Cancel),
        }
    }
}

/// A question popup's ball, back and forth.
pub(crate) fn question_ball(view: &mut View, dialog: &Dialog) {
    if let Some(ball) = control(dialog, BALL).and_then(|b| view.anim_mut(b)) {
        ball.frames = QUESTION_FRAMES.chain(QUESTION_FRAMES.rev().skip(1)).collect();
        ball.frames.pop();
        ball.playing = true;
    }
}

/// The checks before a negotiation (`FUN_005a5ac2` with the flags
/// 0x0dd00000), as far as the replica can tell them, for 28, 306 and 155;
/// past them, booking is not built.
pub(crate) fn negotiate(cx: &Cx, id: u16) -> Outcome {
    let notice = |text| Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
    let Some(records) = cx.state.records.as_ref() else { return Outcome::Stay };
    let own = own_team(cx).map(|(_, t)| (t.country(), i16::from(t.number())));
    let talks = transfers::appointments(records);
    if talks.iter().any(|t| t.player == id && own.is_some_and(|o| t.clubs.contains(&o))) {
        return notice(ALREADY_IN);
    }
    let Some(r) = records.player(id) else { return Outcome::Stay };
    if r.retiring() & 0x80 != 0 {
        return notice(RETIRING_TEXT);
    }
    if r.get::<u8>(0x14c) != 0 {
        return notice(SIGNED_ELSEWHERE);
    }
    if talks.iter().any(|t| t.player == id) {
        return notice(TALK_THIS_WEEK);
    }
    if own.is_some_and(|o| present_club(records, id) == o) {
        return notice(IN_OWN_TEAM);
    }
    if transfers::listings(records).iter().any(|e| e.id == id) {
        return notice(ON_LIST);
    }
    let ours = talks.iter().filter(|t| own.is_some_and(|o| t.clubs.contains(&o))).count();
    if ours >= TALKS_A_WEEK {
        return notice(THREE_TALKS);
    }
    info!("Verhandlungstermin ({id}): not built yet");
    Outcome::Stay
}
