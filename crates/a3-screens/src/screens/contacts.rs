//! The telephone ("Kontakte", dialog 404 `CTelefonManager`; KONTAKTE.md in
//! the RE repo): nine buttons, each calling up a sub-dialog — the
//! co-trainers' and goalkeeper trainers' market, fan service, lawyers,
//! rejecting a referee, a cooperation contract, the aliens' office, a
//! reporter, an enquiry at another club. The button stays pressed while
//! its sub-dialog or alert is up. The sub-dialogs are not built yet.

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::{CONTACTS, NOTICE};
use a3_core::season::Rules;
use a3_files::rch::Dialog;

use super::hub::{self, CONTACTS_SYMBOL};
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::options::NONE;
use crate::pointer::Target;
use crate::view::View;
use a3_core::finances::CLUB_TO_TEAM;

const CAPTION_ID: u16 = 9935;
/// "ANSTOSS 3 - Telefon", with the manager, his club and the date.
const CAPTION: u16 = 2523;
const OK: u16 = 1;
const BUTTONS: u16 = 4975;
/// The fourth symbol the week's last day switches off besides 3 and 5.
const SHORT_TRAINING: usize = 4;
/// The cooperation contract's button and its refusals (§3.1): only in the
/// main country, not for an amateur department, not while one runs
/// ("… mit v1. Dieser läuft noch bis zum %s.").
const COOPERATION: usize = 5;
const NOT_MAIN_COUNTRY: u16 = 4458;
const AMATEURS: u16 = 4967;
const RUNNING: u16 = 4456;
/// The partner's number in the main country and the season the contract
/// ends in (team +0x92a, +0x92c); no partner.
const PARTNER: usize = 0x92a;
const UNTIL: usize = 0x92c;
const NO_PARTNER: u16 = 0xffff;
/// Every sub-dialog, which shows its file's title as its caption.
const SUB_DIALOGS: [u16; 8] = [35, 58, 166, 398, 494, 492, 493, 15];
/// The buttons whose sub-dialogs are built, and those.
const BUILT: [(usize, u16); 5] = [
    (2, FAN_DIALOG),
    (3, LAWYERS_DIALOG),
    (4, REFEREES_DIALOG),
    (6, ALIENS_DIALOG),
    (7, REPORTER_DIALOG),
];

pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&CONTACTS) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    hub::add_bar(dialog, rules);
    for id in SUB_DIALOGS {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        let title = dialog.title.clone();
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, title));
    }
}

/// The menu.
#[derive(Default)]
pub struct Contacts {
    /// Whether the whistle asks.
    whistle: bool,
}

impl Screen for Contacts {
    fn ids(&self) -> &[u16] {
        &[CONTACTS]
    }

    /// `FUN_0058cef6`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        hub::open_bar(cx, view, CONTACTS_SYMBOL);
        if hub::last_day(cx) {
            hub::switch_off(cx, view, SHORT_TRAINING);
        }
        if let Some(season) = cx.state.season.as_ref() {
            set(view, dialog, CAPTION_ID, hub::manager_title(game, season, CAPTION));
        }
        if let Some(c) = control(dialog, BUTTONS) {
            view.set_chosen(c, NONE);
        }
        // The cooperation's refusals and the sub-dialogs built.
        let works = |target: Target| match target {
            Target::Option { control: c, item } if dialog.controls[c].id == Some(BUTTONS) => {
                BUILT.iter().any(|&(i, _)| i == item) || item == COOPERATION
            }
            _ => hub::on_bar(dialog, target).is_some_and(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Some(item) = hub::on_bar(cx.dialog, target) {
            let outcome = hub::bar_click(cx, view, item, CONTACTS_SYMBOL);
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        self.whistle = false;
        let Target::Option { control: c, item } = target else { return Outcome::Stay };
        if cx.dialog.controls[c].id != Some(BUTTONS) {
            return Outcome::Stay;
        }
        if let Some(&(_, screen)) = BUILT.iter().find(|&&(i, _)| i == item) {
            return Outcome::Popup(Request::new(screen));
        }
        // The button stays pressed while what it calls up is shown. The
        // cooperation's check lets an expired contract go.
        if item == COOPERATION {
            expire_cooperation(cx);
        }
        if item == COOPERATION {
            return Outcome::Popup(match cooperation_refused(cx) {
                Some(args) => Request::with(NOTICE, args),
                None => Request::new(COOPERATION_DIALOG),
            });
        }
        view.set_chosen(c, NONE);
        Outcome::Stay
    }

    /// The whistle's question: a yes ends the turn. Anything else that
    /// closes lets the button up.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        if let Some(c) = control(cx.dialog, BUTTONS) {
            view.set_chosen(c, NONE);
        }
        Outcome::Stay
    }
}

/// Why the cooperation contract cannot be made, as its alert's text
/// (`FUN_0058d846`): not in the main country, an amateur department, a
/// contract still running — with its partner and the day it ends.
fn cooperation_refused(cx: &Cx) -> Option<Args> {
    let team = own_team(cx)?;
    let records = cx.state.records.as_ref()?;
    if i16::from(team.country()) != records.globals.main_country() {
        return Some(Args::Text(NOT_MAIN_COUNTRY));
    }
    if team.linked()[0] >= 0 {
        return Some(Args::Text(AMATEURS));
    }
    let seasons = i16::try_from(records.globals.seasons()).unwrap_or(i16::MAX);
    let until = team.get::<i16>(UNTIL);
    let partner = team.get::<u16>(PARTNER);
    if partner == NO_PARTNER || seasons >= until {
        return None;
    }
    Some(Args::Cooperation { partner, year: cx.game.world.start_year + i32::from(until - seasons) })
}

/// A contract whose last season has come is ended (`FUN_0069d8b2`).
fn expire_cooperation(cx: &mut Cx) {
    let seasons = cx.state.records.as_ref().map_or(0, |r| r.globals.seasons());
    let seasons = i16::try_from(seasons).unwrap_or(i16::MAX);
    if let Some(team) = super::squad::own_team_mut(cx)
        && team.get::<u16>(PARTNER) != NO_PARTNER
        && seasons >= team.get::<i16>(UNTIL)
    {
        team.put(PARTNER, NO_PARTNER);
    }
}

/// The running contract's alert: the partner with its article, the last
/// day "30.6." of the year it ends.
pub(crate) fn running_text(cx: &Cx, partner: u16, year: i32) -> String {
    let date =
        hub::date_text(cx.game, chrono::NaiveDate::from_ymd_opt(year, 6, 30).unwrap_or_default());
    let text = cx.game.text(RUNNING).replacen("%s", &date, 1);
    let main = cx.state.records.as_ref().map_or(0, |r| r.globals.main_country());
    let slot = u16::try_from(main).ok().and_then(|n| cx.game.world.slot_of(n)).unwrap_or(0);
    cx.game.with_club(&text, (slot, usize::from(partner)))
}

/// The lawyers (dialog 166 `CRechtsanwaelte`, KONTAKTE.md §6.2): five to
/// choose from, the hearings expected — red cards, others — and what they
/// would cost with the one chosen, against what lawyers cost so far and
/// the plan. OK keeps the lawyer and the estimates.
#[derive(Default)]
pub struct Lawyers {
    lawyer: u8,
    hearings: (u16, u16),
}

const LAWYERS_DIALOG: u16 = 166;
const HEARINGS_SPINS: (u16, u16) = (2795, 2796);
const HEARINGS_FIGURES: (u16, u16) = (2793, 2794);
const MOST_HEARINGS: i32 = 10;
const LAWYER_COSTS: u16 = 2790;
const LAWYERS_SO_FAR: u16 = 2791;
const LAWYERS_PLANNED: u16 = 2792;
/// What lawyers cost so far and the plan's (club +0x2b4, +0x1ac).
const SO_FAR_AT: usize = 0x2b4;
const PLANNED_AT: usize = 0x1ac;
const CANCEL: u16 = 2;

impl Screen for Lawyers {
    fn ids(&self) -> &[u16] {
        &[LAWYERS_DIALOG]
    }

    /// `FUN_00534a12`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        use super::kalkulation::{HEARINGS_AT, LAWYER_AT, own_manager};
        let (game, dialog) = (cx.game, cx.dialog);
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        let Some(team) = own_team(cx) else { return };
        self.lawyer = team.get::<i8>(LAWYER_AT + CLUB_TO_TEAM).max(0) as u8;
        self.hearings = own_manager(cx)
            .map_or((0, 0), |m| (m.get::<u16>(HEARINGS_AT.0), m.get::<u16>(HEARINGS_AT.1)));
        for (spin, value) in
            [(HEARINGS_SPINS.0, self.hearings.0), (HEARINGS_SPINS.1, self.hearings.1)]
        {
            if let Some(v) = control(dialog, spin).and_then(|c| view.value_mut(c)) {
                v.range = (0, MOST_HEARINGS);
                v.set(value.into());
            }
        }
        let so_far = team.get::<i32>(SO_FAR_AT + CLUB_TO_TEAM);
        let planned = team.get::<i32>(PLANNED_AT + CLUB_TO_TEAM);
        set(view, dialog, LAWYERS_SO_FAR, super::playerinfo::money(game, so_far.into()));
        set(view, dialog, LAWYERS_PLANNED, super::playerinfo::money(game, planned.into()));
        self.show(cx, view);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        use super::kalkulation::LAWYERS;
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                self.write(cx);
                return Outcome::Close(Reply::Ok);
            }
            Target::Button(c) if id(c) == CANCEL => return Outcome::Close(Reply::Cancel),
            Target::Option { control: c, .. } => {
                if let Some(k) = LAWYERS.iter().position(|&l| l == id(c)) {
                    self.lawyer = k as u8;
                }
            }
            Target::Spin { control: c, .. } => {
                let value = view.value(c).map_or(0, |v| v.value) as u16;
                if id(c) == HEARINGS_SPINS.0 {
                    self.hearings.0 = value;
                } else if id(c) == HEARINGS_SPINS.1 {
                    self.hearings.1 = value;
                }
            }
            _ => return Outcome::Stay,
        }
        self.show(cx, view);
        Outcome::Stay
    }
}

impl Lawyers {
    /// The lawyer chosen lit, the estimates and their cost
    /// (`FUN_00534c44`, `FUN_006231e0`).
    fn show(&self, cx: &Cx, view: &mut View) {
        use super::kalkulation::{LAWYERS, TARIFFS};
        let (game, dialog) = (cx.game, cx.dialog);
        for (k, &id) in LAWYERS.iter().enumerate() {
            if let Some(c) = control(dialog, id) {
                view.set_chosen(c, if k == usize::from(self.lawyer) { 0 } else { NONE });
            }
        }
        set(view, dialog, HEARINGS_FIGURES.0, self.hearings.0.to_string());
        set(view, dialog, HEARINGS_FIGURES.1, self.hearings.1.to_string());
        let cost = TARIFFS.get(usize::from(self.lawyer)).map_or(0, |&(red, other)| {
            (i32::from(self.hearings.0) * red + i32::from(self.hearings.1) * other) * 1000
        });
        set(view, dialog, LAWYER_COSTS, super::playerinfo::money(game, cost.into()));
    }

    /// OK (`0x534dc1`): the estimates into the manager, the lawyer into the
    /// club; the plan stays and nothing is charged.
    fn write(&self, cx: &mut Cx) {
        use super::kalkulation::{HEARINGS_AT, LAWYER_AT};
        if let Some(team) = super::squad::own_team_mut(cx) {
            team.put(LAWYER_AT + CLUB_TO_TEAM, self.lawyer);
        }
        let Some(current) = cx.state.season.as_ref().map(|s| s.current) else { return };
        let Some(records) = cx.state.records.as_mut() else { return };
        let Some(slot) = records.human_slot(current) else { return };
        if let Some(manager) = records.managers.get_mut(slot) {
            manager.put(HEARINGS_AT.0, self.hearings.0);
            manager.put(HEARINGS_AT.1, self.hearings.1);
        }
    }
}

/// The aliens' office (dialog 492 `CAuslaenderbehoerde`, KONTAKTE.md §7.2):
/// the squad's foreigners; a click on one asks for his naturalisation —
/// the best are too proud, the good and old agree, and then the official's
/// luck of the season decides.
#[derive(Default)]
pub struct AliensOffice {
    /// The rows' players, by their place in the squad.
    players: Vec<usize>,
    /// What follows the alert shown: the office's answer for this player,
    /// or the list filled anew after its yes.
    next: Option<Next>,
}

#[derive(Clone, Copy)]
enum Next {
    Office(usize),
    Refill,
}

const ALIENS_DIALOG: u16 = 492;
const ALIENS_LIST: u16 = 6212;
/// The player says no, says yes, the office says no, says yes.
const PLAYER_REFUSES: u16 = 4452;
const PLAYER_AGREES: u16 = 4453;
const OFFICE_REFUSES: u16 = 4454;
const OFFICE_AGREES: u16 = 4455;
/// Too strong to change his nation; strong enough to be asked, if old
/// enough.
const PROUD: u8 = 10;
const GOOD: u8 = 8;
const OLD_ENOUGH: u32 = 25;
/// The manager's own entry in the squad.
const MANAGER_ID: u16 = 17_000;

impl Screen for AliensOffice {
    fn ids(&self) -> &[u16] {
        &[ALIENS_DIALOG]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        self.next = None;
        self.fill(cx, view);
        mark_inert(view, cx.dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Row { control: c, row } if dialog.controls[c].id == Some(ALIENS_LIST) => {
                let scroll = view.list(c).map_or(0, |l| l.scroll);
                let Some(&p) = self.players.get(scroll + row) else { return Outcome::Stay };
                self.ask(cx, p)
            }
            _ => Outcome::Stay,
        }
    }

    /// After the player's yes the office answers; after its yes he is one
    /// of the country's and leaves the list.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, _: Reply) -> Outcome {
        match self.next.take() {
            Some(Next::Office(p)) => {
                let text = self.office(cx, p);
                Outcome::Popup(Request::with(NOTICE, Args::TextFor(text, player_id(cx, p))))
            }
            Some(Next::Refill) => {
                self.fill(cx, view);
                Outcome::Stay
            }
            None => Outcome::Stay,
        }
    }
}

/// A player of the squad's record id.
fn player_id(cx: &Cx, p: usize) -> u16 {
    own_club(cx).and_then(|c| c.status.get(p)).map_or(0, |s| s.id)
}

fn own_club<'a>(cx: &'a Cx) -> Option<&'a a3_core::season::ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

impl AliensOffice {
    /// The foreigners, in the squad's order (`FUN_0047ac01`): neither
    /// nation the club's country; name, position, strength.
    fn fill(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(club), Some(team), Some(records)) =
            (own_club(cx), own_team(cx), cx.state.records.as_ref())
        else {
            return;
        };
        let country = team.country();
        self.players.clear();
        let mut rows = Vec::new();
        for (p, (player, status)) in club.players.iter().zip(&club.status).enumerate() {
            let Some(r) = records.player(status.id) else { continue };
            if status.id > MANAGER_ID || r.nation() == country || r.second_nation() == country {
                continue;
            }
            let name = records.known_as(status.id).unwrap_or_default();
            let position = hub::abbreviation(game, player.positions[0]);
            let strength =
                if status.strength_hidden { "?".to_owned() } else { player.strength.to_string() };
            rows.push(format!("{name}\t{position}\t{strength}"));
            self.players.push(p);
        }
        if let Some(list) = control(dialog, ALIENS_LIST).and_then(|c| view.list_mut(c)) {
            list.rows = rows;
            list.selected = None;
        }
    }

    /// A player asked (`0x47ad63`): his own answer, and for the willing the
    /// office's after it.
    fn ask(&mut self, cx: &mut Cx, p: usize) -> Outcome {
        let id = player_id(cx, p);
        let Some(player) = own_club(cx).and_then(|c| c.players.get(p)) else {
            return Outcome::Stay;
        };
        let today = cx.state.season.as_ref().map(a3_core::season::Season::date);
        let age = today.map_or(0, |d| player.age(d));
        let notice = |text| Outcome::Popup(Request::with(NOTICE, Args::TextFor(text, id)));
        if player.strength >= PROUD || (player.strength >= GOOD && age < OLD_ENOUGH) {
            return notice(PLAYER_REFUSES);
        }
        if player.strength >= GOOD {
            self.next = Some(Next::Office(p));
            return notice(PLAYER_AGREES);
        }
        let text = self.office(cx, p);
        notice(text)
    }

    /// The office's answer by the season's luck at his id: odd no, even
    /// yes — and then he takes the club's nation. Its yes waits for the
    /// list to be filled anew after it closes.
    fn office(&mut self, cx: &mut Cx, p: usize) -> u16 {
        let id = player_id(cx, p);
        let country = own_team(cx).map_or(0, a3_core::records::Team::country);
        let Some(records) = cx.state.records.as_mut() else { return OFFICE_REFUSES };
        let luck = records.luck.values()[usize::from(id) % 30];
        if luck % 2 != 0 {
            return OFFICE_REFUSES;
        }
        if let Some(r) = records.players.get_mut(usize::from(id)) {
            r.set_nation(country);
        }
        self.next = Some(Next::Refill);
        OFFICE_AGREES
    }
}

/// Rejecting a referee (dialog 398 `CSchiriAblehnen`, KONTAKTE.md §7.1):
/// none or one of the country's forty, the one refused so far in red. OK
/// keeps the choice; a new one makes a headline — the replica has no
/// newspaper yet, but draws its number all the same.
#[derive(Default)]
pub struct Referees {
    /// The rows' referees, 0 for none.
    rows: Vec<i16>,
}

const REFEREES_DIALOG: u16 = 398;
const REFEREE_COMBO: u16 = 6211;
/// "keinen"; the refused referee's red; a country's referees.
const NO_REFEREE: u16 = 1092;
const REFUSED_INK: [u8; 3] = [255, 0, 0];
const COUNTRY_REFEREES: i16 = 40;
/// The refused referee (team +0x928).
const REFUSED_AT: usize = 0x928;

impl Screen for Referees {
    fn ids(&self) -> &[u16] {
        &[REFEREES_DIALOG]
    }

    /// `0x53ca45`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let refused = team.get::<i16>(REFUSED_AT);
        let base = i16::from(records.countries.slot(team.country().into())) * COUNTRY_REFEREES;
        self.rows = std::iter::once(0).chain((1..=COUNTRY_REFEREES).map(|k| base + k)).collect();
        let items = self
            .rows
            .iter()
            .map(|&g| match g {
                0 => game.text(NO_REFEREE).to_owned(),
                g => records.referee_name(g).unwrap_or_default(),
            })
            .collect();
        let inks =
            self.rows.iter().map(|&g| (g != 0 && g == refused).then_some(REFUSED_INK)).collect();
        let selected = self.rows.iter().position(|&g| g == refused && g != 0).unwrap_or(0);
        if let Some(combo) = control(dialog, REFEREE_COMBO).and_then(|c| view.combo_mut(c)) {
            combo.items = items;
            combo.inks = inks;
            combo.selected = Some(selected);
            combo.scroll = 0;
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                let row = control(dialog, REFEREE_COMBO)
                    .and_then(|c| view.combo(c))
                    .and_then(|c| c.selected);
                if let Some(&chosen) = row.and_then(|r| self.rows.get(r)) {
                    self.refuse(cx, chosen);
                }
                Outcome::Close(Reply::Ok)
            }
            Target::Button(c) if dialog.controls[c].id == Some(CANCEL) => {
                Outcome::Close(Reply::Cancel)
            }
            _ => Outcome::Stay,
        }
    }
}

impl Referees {
    /// OK (`0x53cbdf`): a new referee refused makes one of two headlines;
    /// the choice kept, none included.
    fn refuse(&self, cx: &mut Cx, chosen: i16) {
        let refused = own_team(cx).map_or(0, |t| t.get::<i16>(REFUSED_AT));
        if chosen != refused
            && chosen != 0
            && let Some(season) = cx.state.season.as_mut()
        {
            season.random.below(2);
        }
        if let Some(team) = super::squad::own_team_mut(cx) {
            team.put(REFUSED_AT, chosen);
        }
    }
}

/// Fan service (dialog 58 `CFanbetreuung`, KONTAKTE.md §6.1): the fans'
/// number and kind, their clubs; free tickets for the next home match,
/// tickets for the next away match, recruiting fans, a fan club meeting.
/// OK books what was chosen.
#[derive(Default)]
pub struct FanService {
    /// Free and away tickets chosen.
    free: i32,
    away: i32,
}

const FAN_DIALOG: u16 = 58;
const FAN_AMOUNT: u16 = 1535;
const FAN_KIND: u16 = 1537;
const FAN_CLUBS: u16 = 6208;
const ORGANISED: u16 = 6210;
const FREE_COUNT: u16 = 1520;
const FREE_BAR: u16 = 1525;
const FREE_SO_FAR: u16 = 1521;
const AWAY_TITLE: u16 = 1529;
const AWAY_COUNT: u16 = 1522;
const AWAY_BAR: u16 = 1526;
const AWAY_SO_FAR: u16 = 1523;
const AWAY_COST: u16 = 1524;
const RECRUIT: u16 = 1531;
const RECRUIT_COST: u16 = 6209;
const MEETING: u16 = 1528;
/// The fans' number 1033 + v ("Fans???" past them); their kind 3513 + v.
const AMOUNT_TEXTS: u16 = 1033;
const AMOUNTS: u8 = 7;
const NO_AMOUNT: &str = "Fans???";
const KIND_TEXTS: u16 = 3513;
/// No league match left; the stadium full; none given yet; so many given;
/// "Auswärtszuschuß %s"; the away quota used up; so many away tickets.
const NO_MATCH: u16 = 1755;
const STADIUM_FULL: u16 = 1754;
const NONE_GIVEN: u16 = 2310;
const GIVEN: u16 = 2311;
const AWAY_FOR: u16 = 2165;
const QUOTA_USED: u16 = 2163;
const AWAY_GIVEN: u16 = 2164;
/// The team's fan fields: amount, kind, the recruiting bits and their
/// count, free and away tickets given (team +0x11c, +0x11d, +0x127,
/// +0x128, +0x25c, +0x25e); the special expenses (club +0x2c0); the club
/// flags counting fan clubs (+0x110 bits 0 and 0x1000).
const AMOUNT_AT: usize = 0x11c;
const KIND_AT: usize = 0x11d;
const RECRUITING_AT: usize = 0x127;
const RECRUITED_AT: usize = 0x128;
const FREE_AT: usize = 0x25c;
const AWAY_AT: usize = 0x25e;
const SPECIAL_AT: usize = 0x2c0;
const FLAGS_AT: usize = 0x110;
const PRESOLD_AT: usize = 0x22c;
/// The most free tickets a season, tickets a step, the away share of the
/// opponent's stadium, a hundred away tickets' price, recruiting a month;
/// the half weeks before away support opens.
const MOST_FREE: i32 = 10_000;
const TICKET_STEP: i32 = 10;
const AWAY_SHARE: i32 = 10;
const AWAY_PRICE: i32 = 4000;
const RECRUIT_PRICE: i32 = 10_000;
const AWAY_FROM: u16 = 12;
/// The fans' popularity of the manager (record +0x127) and its bounds.
const FANS_POPULARITY: usize = 0x127;

impl Screen for FanService {
    fn ids(&self) -> &[u16] {
        &[FAN_DIALOG]
    }

    /// `FUN_0049e3d0`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        (self.free, self.away) = (0, 0);
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let today = usize::from(records.globals.day());
        let own = (team.country(), i16::from(team.number()));
        let fixtures = a3_core::opponent::fixture_days(records, &game.calendar, team);
        let capacity = |country: u8, number: i16| {
            a3_core::opponent::stadium(records, country.into(), number as u16)
                .map_or(0, |s| s.capacity)
        };
        let bar = |view: &mut View, id: u16, most: i32| {
            if let Some(v) = control(dialog, id).and_then(|c| view.value_mut(c)) {
                v.range = (0, most);
                v.set(0);
            }
        };
        // Free tickets for the next home match.
        let given = i32::from(team.get::<i16>(FREE_AT));
        match fixtures.iter().find(|f| f.day > today && f.home) {
            None => {
                bar(view, FREE_BAR, 0);
                set(view, dialog, FREE_SO_FAR, game.text(NO_MATCH).to_owned());
            }
            Some(f) => {
                let sale = a3_core::tickets::sale(records, own, (own.0, f.opponent), f.day as i16);
                let sold = sale.map_or(0, |s| s.sold) + team.get::<i32>(PRESOLD_AT + CLUB_TO_TEAM);
                let room = capacity(own.0, own.1);
                if sold >= room {
                    bar(view, FREE_BAR, 0);
                    set(view, dialog, FREE_SO_FAR, game.text(STADIUM_FULL).to_owned());
                } else {
                    let free = (room - sold).min(MOST_FREE).min(MOST_FREE - given).max(0);
                    bar(view, FREE_BAR, free / TICKET_STEP);
                    let text = if given == 0 {
                        game.text(NONE_GIVEN).to_owned()
                    } else {
                        game.text(GIVEN).replacen("%d", &given.to_string(), 1)
                    };
                    set(view, dialog, FREE_SO_FAR, text);
                }
            }
        }
        set(view, dialog, FREE_COUNT, "0".to_owned());
        // Tickets for the next away match.
        set(view, dialog, AWAY_COUNT, "0".to_owned());
        let away_given = i32::from(team.get::<i16>(AWAY_AT));
        let money = |v: i32| super::playerinfo::money(game, v.into());
        if records.globals.half_weeks() < AWAY_FROM {
            bar(view, AWAY_BAR, 0);
            set(view, dialog, AWAY_SO_FAR, String::new());
            set(view, dialog, AWAY_COST, money(0));
        } else if let Some(f) = fixtures.iter().find(|f| f.day >= today && !f.home) {
            let name = game.club_name(Some(records), own.0, f.opponent);
            set(view, dialog, AWAY_TITLE, game.text(AWAY_FOR).replacen("%s", &name, 1));
            let quota = capacity(own.0, f.opponent) / AWAY_SHARE;
            if away_given >= quota {
                bar(view, AWAY_BAR, 0);
                set(view, dialog, AWAY_SO_FAR, game.text(QUOTA_USED).to_owned());
                set(view, dialog, AWAY_COST, money(0));
            } else {
                if away_given != 0 {
                    let text = game.text(AWAY_GIVEN).replacen("%d", &away_given.to_string(), 1);
                    set(view, dialog, AWAY_SO_FAR, text);
                }
                bar(view, AWAY_BAR, (quota - away_given) / TICKET_STEP);
                set(view, dialog, AWAY_COST, money(away_cost(away_given)));
            }
        } else {
            bar(view, AWAY_BAR, 0);
            set(view, dialog, AWAY_SO_FAR, game.text(NO_MATCH).to_owned());
        }
        // Recruiting, as the club last chose it.
        let bits = team.get::<u8>(RECRUITING_AT);
        let recruiting = if bits & 2 != 0 { 2 } else { usize::from(bits & 1) };
        if let Some(c) = control(dialog, RECRUIT) {
            view.set_chosen(c, recruiting);
        }
        set(view, dialog, RECRUIT_COST, money(if recruiting >= 1 { RECRUIT_PRICE } else { 0 }));
        let amount = team.get::<u8>(AMOUNT_AT);
        let amount_text = if amount < AMOUNTS {
            game.text(AMOUNT_TEXTS + u16::from(amount)).to_owned()
        } else {
            NO_AMOUNT.to_owned()
        };
        set(view, dialog, FAN_AMOUNT, amount_text);
        set(
            view,
            dialog,
            FAN_KIND,
            game.text(KIND_TEXTS + u16::from(team.get::<u8>(KIND_AT))).to_owned(),
        );
        let (clubs, organised) = fan_clubs(records, team, today);
        set(view, dialog, FAN_CLUBS, clubs.to_string());
        set(view, dialog, ORGANISED, organised.to_string());
        let works =
            |t: Target| !matches!(t, Target::Button(c) if dialog.controls[c].id == Some(MEETING));
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (game, dialog) = (cx.game, cx.dialog);
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                let recruiting = control(dialog, RECRUIT).map_or(0, |c| view.chosen(c));
                self.write(cx, recruiting);
                Outcome::Close(Reply::Ok)
            }
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. } => {
                let value = view.value(c).map_or(0, |v| v.value) * TICKET_STEP;
                if id(c) == FREE_BAR {
                    self.free = value;
                    set(view, dialog, FREE_COUNT, value.to_string());
                } else if id(c) == AWAY_BAR {
                    self.away = value;
                    set(view, dialog, AWAY_COUNT, value.to_string());
                    let given = own_team(cx).map_or(0, |t| i32::from(t.get::<i16>(AWAY_AT)));
                    let cost = super::playerinfo::money(game, away_cost(given + value).into());
                    set(view, dialog, AWAY_COST, cost);
                }
                Outcome::Stay
            }
            Target::Option { control: c, item } if id(c) == RECRUIT => {
                let cost = if item >= 1 { RECRUIT_PRICE } else { 0 };
                set(view, dialog, RECRUIT_COST, super::playerinfo::money(game, cost.into()));
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

/// Away tickets' price: 4,000 € a hundred begun.
fn away_cost(tickets: i32) -> i32 {
    (tickets + 99) / 100 * AWAY_PRICE
}

/// The fan clubs and the organised fans (`FUN_0049e2d9`): from the fans'
/// amount and kind, the club's luck of the season, its titles and two of
/// its flags.
fn fan_clubs(
    records: &a3_core::records::Records,
    team: &a3_core::records::Team,
    today: usize,
) -> (i32, i32) {
    let amount = i32::from(team.get::<u8>(AMOUNT_AT));
    let flags = team.get::<u16>(FLAGS_AT);
    let k = i32::from(flags & 1 != 0) + i32::from(flags & 0x1000 != 0);
    // Spoilt fans' further step depends on a check not traced; taken as
    // the others'.
    let v = match team.get::<u8>(KIND_AT) {
        6 => (amount + 2).min(7),
        7 => amount,
        _ => amount + 1,
    };
    let q = 8 - v;
    let luck = records.luck.values();
    let number = usize::from(team.number());
    let titles = a3_core::opponent::titles(records, team)[0];
    let clubs = 2 * q * q * q + i32::from(luck[number % 30]) / 10 + 1 + titles + 5 * k;
    let organised = clubs * ((number % 3) as i32 + 2 * q + 1) + i32::from(luck[today % 30]);
    (clubs, organised)
}

impl FanService {
    /// OK (`0x49f2a7`): the free tickets counted, recruiting kept and its
    /// first month booked with a point of the fans' favour, the away
    /// tickets paid by the hundred begun, the fans' favour by their share.
    fn write(&self, cx: &mut Cx, recruiting: usize) {
        let mut favour = 0;
        let Some(team) = super::squad::own_team_mut(cx) else { return };
        team.put(FREE_AT, team.get::<i16>(FREE_AT).wrapping_add(self.free as i16));
        let mut bits = team.get::<u8>(RECRUITING_AT);
        let special = SPECIAL_AT + CLUB_TO_TEAM;
        if recruiting >= 1 {
            if recruiting == 2 {
                bits |= 2;
            } else {
                bits &= !2;
            }
            if bits & 1 == 0 {
                bits |= 1;
                team.put(special, team.get::<i32>(special).wrapping_add(RECRUIT_PRICE));
                if bits & 4 == 0 {
                    team.put(RECRUITED_AT, team.get::<u8>(RECRUITED_AT).wrapping_add(1));
                    favour += 1;
                }
            }
        } else {
            bits &= !3;
        }
        team.put(RECRUITING_AT, bits);
        let given = i32::from(team.get::<i16>(AWAY_AT));
        let share = |n: i32| (n * 3 / 500).min(6);
        let now = given + self.away;
        let cost = team.get::<i32>(special) - away_cost(given) + away_cost(now);
        team.put(special, cost);
        team.put(AWAY_AT, now as i16);
        favour += share(now) - share(given);
        let Some(current) = cx.state.season.as_ref().map(|s| s.current) else { return };
        let Some(records) = cx.state.records.as_mut() else { return };
        let Some(slot) = records.human_slot(current) else { return };
        if let Some(manager) = records.managers.get_mut(slot) {
            let v = (i32::from(manager.get::<u8>(FANS_POPULARITY)) + favour).clamp(0, 100);
            manager.put(FANS_POPULARITY, v as u8);
        }
    }
}

/// A text with a club's phrases and a number for its "%d" (`FUN_00733084`).
pub(crate) fn club_text(cx: &Cx, text: u16, country: u16, number: u16, value: i32) -> String {
    let text = cx.game.text(text).replacen("%d", &value.to_string(), 1);
    let slot = cx.game.world.slot_of(country).unwrap_or(0);
    cx.game.with_club(&text, (slot, usize::from(number)))
}

/// The cooperation contract (dialog 494 `CKooperationsvertrag`,
/// KONTAKTE.md §8.1): a smaller or a bigger club of the main country to
/// cooperate with, from the season's table of free clubs. A club that
/// agrees offers three to five years; a yes binds both clubs.
#[derive(Default)]
pub struct Cooperation {
    /// The combos' clubs below their first row, by number: smaller,
    /// bigger.
    smaller: Vec<u16>,
    bigger: Vec<u16>,
    /// The club and the years offered, while the question is asked.
    offer: Option<(u16, i16)>,
}

const COOPERATION_DIALOG: u16 = 494;
const SMALLER_COMBO: u16 = 6296;
const BIGGER_COMBO: u16 = 6297;
const SMALLER_CHECK: u16 = 6298;
const BIGGER_CHECK: u16 = 6299;
/// "Bitte auswählen"; a club not interested; its offer of %d years.
const CHOOSE: u16 = 4945;
const NOT_INTERESTED: u16 = 4703;
const OFFER: u16 = 4704;
/// The table of the main country's clubs free to cooperate (save block
/// `0xb9f6a8`): free, class, squad rating a club.
const COOPERATION_TABLE: &str = "0xb9f6a8";
const TABLE_ENTRY: usize = 4;
/// How much weaker a smaller club must be; the years offered, from three.
const WEAKER: i16 = 250;
const LEAST_YEARS: i16 = 3;

/// A club's entry in the table: free, class, rating.
fn table_entry(records: &a3_core::records::Records, n: usize) -> Option<(bool, u8, i16)> {
    let block = records.block(COOPERATION_TABLE)?;
    let e = block.get(n * TABLE_ENTRY..(n + 1) * TABLE_ENTRY)?;
    Some((e[0] != 0, e[1], i16::from_le_bytes([e[2], e[3]])))
}

/// A club's class in the main country (`FUN_006755b8`): its league,
/// capped at the third (the fourth in country 16).
fn class_of(country: u8, level: u8) -> u8 {
    level.min(if country == 16 { 3 } else { 2 })
}

impl Screen for Cooperation {
    fn ids(&self) -> &[u16] {
        &[COOPERATION_DIALOG]
    }

    /// `0x4fa21d`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.offer = None;
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let main = records.globals.main_country();
        let own = usize::from(team.number());
        let class = class_of(team.country(), team.level());
        // The club's own rating as the table has it; the original takes
        // the live one (`FUN_007779e4`, not traced).
        let rating = table_entry(records, own).map_or(0, |e| e.2);
        let humans: Vec<u16> = records
            .humans()
            .filter(|m| i16::try_from(m.country()).ok() == Some(main))
            .map(a3_core::records::Manager::team)
            .collect();
        let partner = team.linked()[1];
        let count = records.countries.team_count(main.max(0) as usize);
        let (mut smaller, mut bigger) = (Vec::new(), Vec::new());
        for n in 0..count.max(0) as u16 {
            let Some((free, c, r)) = table_entry(records, usize::from(n)) else { continue };
            if !free || humans.contains(&n) || i16::from(partner) == n as i16 {
                continue;
            }
            if c < class {
                bigger.push(n);
            } else if r < rating - WEAKER && c > class {
                smaller.push(n);
            }
        }
        let name = |&n: &u16| game.club_name(Some(records), main as u8, n as i16);
        let sort = |list: &mut Vec<u16>| {
            list.sort_by_cached_key(|n| name(n));
        };
        sort(&mut smaller);
        sort(&mut bigger);
        for (combo, list) in [(SMALLER_COMBO, &smaller), (BIGGER_COMBO, &bigger)] {
            let items = std::iter::once(game.text(CHOOSE).to_owned()).chain(list.iter().map(name));
            if let Some(c) = control(dialog, combo).and_then(|c| view.combo_mut(c)) {
                c.items = items.collect();
                c.selected = Some(0);
                c.scroll = 0;
                c.disabled = list.is_empty();
            }
        }
        let ticks = (!smaller.is_empty(), smaller.is_empty() && !bigger.is_empty());
        for (check, ticked, empty) in [
            (SMALLER_CHECK, ticks.0, smaller.is_empty()),
            (BIGGER_CHECK, ticks.1, bigger.is_empty()),
        ] {
            if let Some(c) = control(dialog, check).and_then(|c| view.check_mut(c)) {
                c.checked = ticked;
                c.disabled = empty;
            }
        }
        (self.smaller, self.bigger) = (smaller, bigger);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                self.ask(cx, view)
            }
            Target::Button(c) if id(c) == CANCEL => Outcome::Close(Reply::Cancel),
            // A box ticked unticks the other.
            Target::Check(c) => {
                let other = if id(c) == SMALLER_CHECK { BIGGER_CHECK } else { SMALLER_CHECK };
                if view.check(c).is_some_and(|b| b.checked)
                    && let Some(o) = control(dialog, other).and_then(|o| view.check_mut(o))
                {
                    o.checked = false;
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }

    /// The offer answered: a yes binds both clubs for the years offered.
    fn answer(&mut self, cx: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        let Some((n, years)) = self.offer.take() else { return Outcome::Stay };
        if reply != Reply::Ok {
            return Outcome::Stay;
        }
        self.sign(cx, n, years);
        Outcome::Close(Reply::Ok)
    }
}

impl Cooperation {
    /// OK (`0x4faa0b`): the ticked box's club, if one is chosen; a club
    /// free to — or a bigger one not in debt — offers its years, else says
    /// no.
    fn ask(&mut self, cx: &Cx, view: &View) -> Outcome {
        let dialog = cx.dialog;
        let ticked =
            |id: u16| control(dialog, id).and_then(|c| view.check(c)).is_some_and(|b| b.checked);
        let (combo, list, bigger) = if ticked(SMALLER_CHECK) {
            (SMALLER_COMBO, &self.smaller, false)
        } else if ticked(BIGGER_CHECK) {
            (BIGGER_COMBO, &self.bigger, true)
        } else {
            return Outcome::Stay;
        };
        let row = control(dialog, combo).and_then(|c| view.combo(c)).and_then(|c| c.selected);
        let Some(&n) = row.and_then(|r| r.checked_sub(1)).and_then(|r| list.get(r)) else {
            return Outcome::Stay;
        };
        let Some(records) = cx.state.records.as_ref() else { return Outcome::Stay };
        let main = records.globals.main_country() as u16;
        let years = records.luck.values()[usize::from(n) % 30] % 3 + LEAST_YEARS;
        let free = table_entry(records, usize::from(n)).is_some_and(|e| e.0);
        let solvent = || {
            records
                .team(main, n)
                .is_some_and(|t| a3_core::finances::figure(t, records.globals.fund_prices()) >= 0)
        };
        if !(free || bigger && solvent()) {
            let args = Args::ForClub { text: NOT_INTERESTED, country: main, number: n, value: 0 };
            return Outcome::Popup(Request::with(NOTICE, args));
        }
        self.offer = Some((n, years));
        let args = Args::ForClub { text: OFFER, country: main, number: n, value: years.into() };
        Outcome::Popup(Request::with(a3_core::cycle::dialogs::QUESTION, args))
    }

    /// Both clubs bound until the seasons plus the years; neither free in
    /// the table any more.
    fn sign(&self, cx: &mut Cx, n: u16, years: i16) {
        let Some(team) = own_team(cx) else { return };
        let own = u16::from(team.number());
        let Some(records) = cx.state.records.as_mut() else { return };
        let main = records.globals.main_country() as u16;
        let until = records.globals.seasons() as i16 + years;
        for (club, partner) in [(n, own), (own, n)] {
            if let Some(t) = records.team_mut(main, club) {
                t.put(PARTNER, partner);
                t.put(UNTIL, until);
            }
        }
        if let Some(block) = records.block_mut(COOPERATION_TABLE) {
            for club in [own, n] {
                if let Some(free) = block.get_mut(usize::from(club) * TABLE_ENTRY) {
                    *free = 0;
                }
            }
        }
    }
}

/// The reporter (dialog 493 `CReporterAnfrage`, KONTAKTE.md §7.3): a
/// friend at the paper writes for the manager once a season, if the media
/// like him enough. Making his release clause public (item 0) is not
/// built.
#[derive(Default)]
pub struct Reporter;

const REPORTER_DIALOG: u16 = 493;
const REPORTER_ITEMS: u16 = 6214;
/// Making the clause public; the three articles.
const CLAUSE: usize = 0;
/// The press was used this season; the media do not like him enough; the
/// article was written.
const PRESS_USED: u16 = 4288;
const NOT_LIKED: u16 = 4952;
const WRITTEN: u16 = 4953;
/// The least the media must like him.
const LIKED_ENOUGH: i32 = 80;
/// The manager's record: his level, the season's flags with the press's
/// bit. How the media like him is the third of his popularity.
const LEVEL_AT: usize = 0x23;
const SEASON_FLAGS_AT: usize = 0x1f3c;
const PRESS_BIT: u32 = 0x800;

impl Screen for Reporter {
    fn ids(&self) -> &[u16] {
        &[REPORTER_DIALOG]
    }

    /// `0x537870`: the clause's item is off while the manager has none or
    /// it is public already.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        if let Some(c) = control(dialog, REPORTER_ITEMS) {
            if let Some(options) = view.options_mut(c) {
                options.actions = true;
                options.dimmed = (0..4).map(|i| i == CLAUSE).collect();
            }
            view.set_chosen(c, NONE);
        }
        let works = |t: Target| !matches!(t, Target::Option { item: CLAUSE, .. });
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Option { control: c, item }
                if dialog.controls[c].id == Some(REPORTER_ITEMS) =>
            {
                view.set_chosen(c, NONE);
                if item == CLAUSE {
                    return Outcome::Stay;
                }
                Outcome::Popup(Request::with(NOTICE, Args::Text(write_article(cx))))
            }
            _ => Outcome::Stay,
        }
    }
}

/// An article asked for (`0x5378d6`, items 1 to 3): once a season, and
/// only for a manager the media like. The headline itself needs the
/// newspaper, which is not built.
fn write_article(cx: &mut Cx) -> u16 {
    let Some(manager) = super::kalkulation::own_manager(cx) else { return PRESS_USED };
    if manager.get::<u32>(SEASON_FLAGS_AT) & PRESS_BIT != 0 {
        return PRESS_USED;
    }
    let level = usize::from(manager.get::<u8>(LEVEL_AT)).min(15);
    let ability = i32::from(manager.abilities()[level][0]);
    let media = i32::from(manager.popularity()[2]);
    if (media * (ability * 4 + 80) / 100).clamp(0, 100) < LIKED_ENOUGH {
        return NOT_LIKED;
    }
    let current = cx.state.season.as_ref().map(|s| s.current);
    if let Some(records) = cx.state.records.as_mut()
        && let Some(slot) = current.and_then(|c| records.human_slot(c))
        && let Some(manager) = records.managers.get_mut(slot)
    {
        let flags = manager.get::<u32>(SEASON_FLAGS_AT) | PRESS_BIT;
        manager.put(SEASON_FLAGS_AT, flags);
    }
    WRITTEN
}
