//! The transfer list (dialog 287, `CTransferlisteDialog`; TRANSFERLISTE.md
//! in the RE repo): every player listed for sale or loan, by position and
//! strength, narrowed by league or country, by new or own players and by
//! a least strength; the club's own players to take off the list, any
//! player to watch.

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::{CURRENT_SQUAD, NOTICE, QUESTION, TRANSFER_LIST};
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
                Some(Command::Watch) => self.watch(cx, id),
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
            1..=3 => played && league_level(records, e).min(2) == i32::from(self.leagues - 1),
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

    /// A player put on the club's watch list (`FUN_00418ab2`): not one of
    /// its own; when full, told; when there already, told and charged.
    fn watch(&self, cx: &mut Cx, id: u16) -> Outcome {
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
}

/// How high a listed player's club plays: its league for a country with
/// leagues, ten for its amateur clubs, none abroad (`FUN_0063e2df`).
fn league_level(records: &Records, e: &Listing) -> i32 {
    const AMATEURS: i32 = 10;
    let countries = &records.countries;
    let country = usize::from(e.country);
    match countries.kind(country) {
        1 | 2 if (e.number as i16) < countries.team_count(country) => {
            records.team(e.country.into(), e.number).map_or(AMATEURS, |t| i32::from(t.level()))
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
