//! The telephone ("Kontakte", dialog 404 `CTelefonManager`; KONTAKTE.md in
//! the RE repo): nine buttons, each calling up a sub-dialog — the
//! co-trainers' and goalkeeper trainers' market, fan service, lawyers,
//! rejecting a referee, a cooperation contract, the aliens' office, a
//! reporter, an enquiry at another club. The button stays pressed while
//! its sub-dialog or alert is up. The sub-dialogs are not built yet.

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::{CONTACTS, NOTICE};
use a3_core::trainers;
use a3_core::random::Random;
use a3_core::season::Rules;
use a3_files::rch::Dialog;

use super::hub::{self, CONTACTS_SYMBOL};
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::options::NONE;
use crate::game::Game;
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
/// The enquiry's button, the alert for a manager who has signed with
/// another club, and where that club stands in his record (+0x130).
const ENQUIRY: usize = 8;
const NEW_JOB: u16 = 2422;
const MOVING_AT: usize = 0x130;
/// Every sub-dialog, which shows its file's title as its caption.
const SUB_DIALOGS: [u16; 16] = [35, 58, 166, 398, 494, 492, 493, 15, 12, 13, 14, 31, 32, 34, 37, 39];
/// The buttons whose sub-dialogs are built, and those.
const BUILT: [(usize, u16); 6] = [
    (2, FAN_DIALOG),
    (3, LAWYERS_DIALOG),
    (4, REFEREES_DIALOG),
    (6, ALIENS_DIALOG),
    (7, REPORTER_DIALOG),
    (8, ENQUIRY_DIALOG),
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
                item <= 1 || BUILT.iter().any(|&(i, _)| i == item) || item == COOPERATION
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
        // A manager who has signed elsewhere enquires nowhere.
        if item == ENQUIRY
            && let Some(manager) = super::kalkulation::own_manager(cx)
            && manager.get::<u8>(MOVING_AT) != 0
        {
            let args = Args::ForClub {
                text: NEW_JOB,
                country: manager.get::<u8>(MOVING_AT).into(),
                number: manager.get::<u16>(MOVING_AT + 2),
                value: 0,
            };
            return Outcome::Popup(Request::with(NOTICE, args));
        }
        if item <= 1 {
            let args = Args::Trainers { keepers: item == 1 };
            return Outcome::Popup(Request::with(TRAINERS_DIALOG, args));
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
/// like him enough, and he may have his release clause made public.
#[derive(Default)]
pub struct Reporter {
    /// Whether the interview stands open; its close makes the clause
    /// public and costs the manager.
    clause_open: bool,
}

const REPORTER_DIALOG: u16 = 493;
const REPORTER_ITEMS: u16 = 6214;
/// Making the clause public; the three articles.
const CLAUSE: usize = 0;
/// The interview that says it (dialog 66 `CFreigabeklauselReporter`) and
/// its animated reporter; the clause and its bits in the manager record:
/// +0x179 bit 0 once public, bit 1 the "already known" test — which
/// nothing ever sets, so item 0 stays ready and each use costs again.
const CLAUSE_DIALOG: u16 = 66;
const CLAUSE_ANI: u16 = 1336;
const CLAUSE_AT: usize = 0x178;
const CLAUSE_FLAG_AT: usize = 0x179;
const CLAUSE_PUBLIC: u8 = 1;
const CLAUSE_KNOWN: u8 = 2;
/// "Ihre Ausstiegsklausel ist bereits bekannt."; what the interview
/// costs: five points with the fans (popularity's second) and ten of the
/// board's confidence.
const CLAUSE_ALERT: u16 = 3207;
const FANS: usize = 1;
const FANS_LOSS: i32 = 5;
const CONFIDENCE_LOSS: i32 = 10;
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

    /// `0x537870`: the clause's item is off while the manager has none;
    /// once made public it stays ready, since nothing sets the "known"
    /// bit its test reads.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        self.clause_open = false;
        let dialog = cx.dialog;
        let has_clause =
            super::kalkulation::own_manager(cx).is_some_and(|m| m.get::<u8>(CLAUSE_AT) != 0);
        if let Some(c) = control(dialog, REPORTER_ITEMS) {
            if let Some(options) = view.options_mut(c) {
                options.dimmed = (0..4).map(|i| i == CLAUSE && !has_clause).collect();
                options.actions = true;
            }
            view.set_chosen(c, NONE);
        }
        let works = |t: Target| has_clause || !matches!(t, Target::Option { item: CLAUSE, .. });
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
                    // Off while he has no clause; a known one only answers
                    // itself; else the interview starts (§7.3).
                    let Some(manager) = super::kalkulation::own_manager(cx) else {
                        return Outcome::Stay;
                    };
                    if manager.get::<u8>(CLAUSE_AT) == 0 {
                        return Outcome::Stay;
                    }
                    if manager.get::<u8>(CLAUSE_FLAG_AT) & CLAUSE_KNOWN != 0 {
                        let args = Args::Text(CLAUSE_ALERT);
                        return Outcome::Popup(Request::with(NOTICE, args));
                    }
                    self.clause_open = true;
                    return Outcome::Popup(Request::new(CLAUSE_DIALOG));
                }
                Outcome::Popup(Request::with(NOTICE, Args::Text(write_article(cx))))
            }
            _ => Outcome::Stay,
        }
    }

    /// The interview's end: the clause stands in the paper now, and the
    /// talk costs five points with the fans and ten of the board's
    /// confidence (`0x41849a`).
    fn answer(&mut self, cx: &mut Cx, _: &mut View, _: Reply) -> Outcome {
        if !std::mem::take(&mut self.clause_open) {
            return Outcome::Stay;
        }
        let current = cx.state.season.as_ref().map(|s| s.current);
        if let Some(records) = cx.state.records.as_mut()
            && let Some(slot) = current.and_then(|c| records.human_slot(c))
            && let Some(manager) = records.managers.get_mut(slot)
        {
            let flags = manager.get::<u8>(CLAUSE_FLAG_AT) | CLAUSE_PUBLIC;
            manager.put(CLAUSE_FLAG_AT, flags);
            let mut popularity = manager.popularity();
            popularity[FANS] = (i32::from(popularity[FANS]) - FANS_LOSS).clamp(0, 100) as u8;
            manager.set_popularity(popularity);
            let confidence = (i32::from(manager.confidence()) - CONFIDENCE_LOSS).clamp(0, 100);
            manager.set_confidence(i16::try_from(confidence).unwrap_or(0));
        }
        Outcome::Stay
    }
}

/// The interview that makes the release clause public (dialog 66
/// `CFreigabeklauselReporter`, KONTAKTE.md §7.3): the sentence stands in
/// its file, the reporter's picture turns, and the one button ends it.
#[derive(Default)]
pub struct ReporterClause;

impl Screen for ReporterClause {
    fn ids(&self) -> &[u16] {
        &[CLAUSE_DIALOG]
    }

    /// `0x4a9414`: the reporter's pictures `fv1…fv9`, up and down, a new
    /// frame every 200 ms.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        if let Some(ball) = control(cx.dialog, CLAUSE_ANI).and_then(|b| view.anim_mut(b)) {
            ball.frames = (1745u16..=1753).chain((1746..=1752).rev()).collect();
            ball.playing = true;
        }
    }

    /// The one button, drawn with the cancel face; it ends the dialog.
    fn input(&mut self, _: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(_) => Outcome::Close(Reply::Cancel),
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

/// The enquiry at another club (dialog 15 `CAnfrageVereinsauswahl`,
/// KONTAKTE.md §8.2): the leagues with clubs looking for a manager, the
/// candidates for next season and the clubs that would take one at once.
/// A click on a club sends the enquiry there; the run of form it weighs
/// is not built.
#[derive(Default)]
pub struct Enquiry {
    /// The combo's leagues, by country and league there.
    leagues: Vec<(u16, usize)>,
    /// The chosen combo row and the club numbers its two lists show.
    chosen: usize,
    clubs: [Vec<u16>; 2],
    /// The club that answered warmly: when its dialog 14 closes, the week
    /// planner opens to fix the talk's day (`0x442560` then `0x58502a`).
    welcomed: Option<(u8, i16)>,
}

const ENQUIRY_DIALOG: u16 = 15;
const AT_ONCE_LIST: u16 = 1260;
const NEXT_SEASON_LIST: u16 = 1261;
const LEAGUE_COMBO: u16 = 1262;
/// "derzeit kein Verein interessiert".
const NOBODY: u16 = 4965;
/// The vacancies (save block `0x1c46f48`): ten leagues' counts for an
/// immediate move, then for next season, then their clubs' numbers.
const VACANCIES: &str = "0x1c46f48";
const LEAGUES: usize = 10;
const CLUBS_A_LEAGUE: usize = 24;
const AT_ONCE_COUNTS: usize = 0x000;
const NEXT_COUNTS: usize = 0x014;
const AT_ONCE_CLUBS: usize = 0x028;
const NEXT_CLUBS: usize = 0x208;
/// The three answers to an enquiry: the chairman friendly, the unfriendly
/// one where the manager stands no chance, and the one that takes the
/// enquiry kindly. Each shows its picture in control 1256.
const ANSWER_FRIENDLY: u16 = 12;
const ANSWER_UNFRIENDLY: u16 = 13;
const ANSWER_WELCOME: u16 = 14;
const ANSWER_PICTURE: u16 = 1256;
const PICTURE_REFUSED: u16 = 1932;
const PICTURE_RUDE: u16 = 1933;
const PICTURE_KEEN: u16 = 1931;
/// Why the enquiry cannot go out at all: talks with that club already
/// running, its board full this week, and the manager under contract with
/// neither clause nor the president's release.
const ALREADY_TALKING: u16 = 2452;
const BOARD_FULL: u16 = 2453;
const UNDER_CONTRACT: u16 = 2454;
/// The odds of an enquiry, per cent, by the manager's standing 0…15 and
/// the club's class 0…9 (the table at `0x8ab900`, `FUN_006a61e5`).
const CHANCES: [[u8; 10]; 16] = [
    [10, 10, 10, 0, 0, 0, 0, 0, 0, 0],
    [20, 10, 10, 0, 0, 0, 0, 0, 0, 0],
    [30, 20, 10, 0, 0, 0, 0, 0, 0, 0],
    [40, 30, 10, 0, 0, 0, 0, 0, 0, 0],
    [50, 40, 20, 0, 0, 0, 0, 0, 0, 0],
    [10, 50, 30, 30, 0, 0, 0, 0, 0, 0],
    [10, 50, 40, 40, 20, 0, 0, 0, 0, 0],
    [0, 20, 50, 50, 50, 20, 0, 0, 0, 0],
    [0, 0, 10, 40, 60, 50, 0, 0, 0, 0],
    [0, 0, 0, 20, 40, 60, 20, 10, 0, 0],
    [0, 0, 0, 10, 10, 50, 40, 20, 10, 0],
    [0, 0, 0, 10, 10, 40, 70, 30, 20, 10],
    [0, 0, 0, 5, 10, 30, 80, 40, 30, 20],
    [0, 0, 0, 0, 5, 20, 90, 50, 40, 30],
    [0, 0, 0, 0, 0, 10, 100, 60, 50, 40],
    [0, 0, 0, 0, 0, 10, 100, 70, 60, 50],
];

/// A league's clubs looking for a manager: at once, and for next season.
fn vacancies(records: &a3_core::records::Records, g: usize) -> ([Vec<u16>; 2], [i16; 2]) {
    let Some(block) = records.block(VACANCIES) else { return ([Vec::new(), Vec::new()], [0; 2]) };
    let word = |at: usize| block.get(at..at + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]));
    let list = |counts: usize, clubs: usize| {
        let n = word(counts + 2 * g).max(0) as usize;
        (0..n.min(CLUBS_A_LEAGUE))
            .map(|i| word(clubs + 2 * (g * CLUBS_A_LEAGUE + i)) as u16)
            .collect::<Vec<_>>()
    };
    let counts = [word(AT_ONCE_COUNTS + 2 * g), word(NEXT_COUNTS + 2 * g)];
    ([list(AT_ONCE_COUNTS, AT_ONCE_CLUBS), list(NEXT_COUNTS, NEXT_CLUBS)], counts)
}

impl Screen for Enquiry {
    fn ids(&self) -> &[u16] {
        &[ENQUIRY_DIALOG]
    }

    /// `0x4428b8`: the main country's leagues with vacancies, then the
    /// first league of each further country.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.leagues.clear();
        (self.chosen, self.clubs, self.welcomed) = (0, Default::default(), None);
        let Some(records) = cx.state.records.as_ref() else { return };
        let main = records.globals.main_country().max(0) as u16;
        let has = |country: u16, league: usize| {
            let g = records.countries.first_league(country.into()).max(0) as usize + league;
            let (_, counts) = vacancies(records, g);
            counts[0] > 0 || counts[1] > 0
        };
        let main_leagues = records.countries.leagues(main.into()).max(0) as usize;
        for league in (0..main_leagues.min(LEAGUES)).filter(|&l| has(main, l)) {
            self.leagues.push((main, league));
        }
        let more = records.globals.more_count().max(0) as usize;
        for &country in records.globals.more_countries().iter().take(more.min(4)) {
            let country = country.max(0) as u16;
            if country != 0 && has(country, 0) {
                self.leagues.push((country, 0));
            }
        }
        let items: Vec<String> = self
            .leagues
            .iter()
            .map(|&(country, league)| records.countries.league_name(country.into(), league))
            .collect();
        if let Some(c) = control(dialog, LEAGUE_COMBO).and_then(|c| view.combo_mut(c)) {
            c.disabled = items.is_empty();
            c.items = items;
            c.selected = (!self.leagues.is_empty()).then_some(0);
            c.scroll = 0;
        }
        if self.leagues.is_empty() {
            for id in [AT_ONCE_LIST, NEXT_SEASON_LIST] {
                if let Some(list) = control(dialog, id).and_then(|c| view.list_mut(c)) {
                    list.rows = vec![game.text(NOBODY).to_owned()];
                }
            }
        } else {
            self.fill(cx, view, 0);
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Row { control: c, row } if dialog.controls[c].id == Some(LEAGUE_COMBO) => {
                self.fill(cx, view, row);
                Outcome::Stay
            }
            Target::Row { control: c, row }
                if matches!(
                    dialog.controls[c].id,
                    Some(AT_ONCE_LIST) | Some(NEXT_SEASON_LIST)
                ) =>
            {
                let at_once = dialog.controls[c].id == Some(AT_ONCE_LIST);
                match self.clubs[usize::from(!at_once)].get(row).copied() {
                    Some(number) => self.ask(cx, at_once, number),
                    None => Outcome::Stay,
                }
            }
            _ => Outcome::Stay,
        }
    }

    /// Closing the warm answer sends the week planner to that club; its
    /// day then goes into the talks' shared list (`FUN_006a8d67`).
    fn answer(&mut self, _: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        if reply != Reply::Ok {
            self.welcomed = None;
            return Outcome::Stay;
        }
        match self.welcomed.take() {
            Some((country, number)) => {
                let args = Args::WithClub(country, number);
                Outcome::Popup(Request::with(super::planner::PLANNER, args))
            }
            None => Outcome::Stay,
        }
    }
}

impl Enquiry {
    /// The two lists of a league (`FUN_00442c7f`), without the own club,
    /// its professionals and its amateurs.
    fn fill(&mut self, cx: &Cx, view: &mut View, chosen: usize) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(&(country, league)) = self.leagues.get(chosen) else { return };
        let (Some(records), Some(team)) = (cx.state.records.as_ref(), own_team(cx)) else { return };
        self.chosen = chosen;
        let g = records.countries.first_league(country.into()).max(0) as usize + league;
        let ([at_once, next], _) = vacancies(records, g);
        let own = i16::from(team.number());
        let linked = team.linked();
        let mine = |n: u16| {
            country == u16::from(team.country()) && (n as i16 == own || linked.contains(&(n as i8)))
        };
        for (numbers, (id, clubs)) in
            self.clubs.iter_mut().zip([(AT_ONCE_LIST, at_once), (NEXT_SEASON_LIST, next)])
        {
            *numbers = clubs.into_iter().filter(|&n| !mine(n)).collect();
            if let Some(list) = control(dialog, id).and_then(|c| view.list_mut(c)) {
                list.rows = numbers
                    .iter()
                    .map(|&n| game.club_name(Some(records), country as u8, n as i16))
                    .collect();
                list.selected = None;
                list.scroll = 0;
            }
        }
    }

    /// A click on a club (`0x442f85` and `0x443457`): three reasons the
    /// enquiry cannot go out, then the club weighs the manager's standing
    /// against its own class and its luck. `at_once` is the 1260 list.
    /// The own club's run of form and the league's size are not weighed
    /// here, so it always stands mid-table with a run of nought.
    fn ask(&mut self, cx: &mut Cx, at_once: bool, number: u16) -> Outcome {
        let (Some(records), Some(team)) = (cx.state.records.as_ref(), own_team(cx)) else {
            return Outcome::Stay;
        };
        let Some(&(country, _)) = self.leagues.get(self.chosen) else {
            return Outcome::Stay;
        };
        let index = records.human_slot(0).unwrap_or(0);
        let manager = &records.managers[index];
        let alert = |text| Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
        // Under contract past the season, with neither clause nor the
        // president's release, he asks nowhere.
        let bound = if at_once { 0 } else { 1 };
        if manager.contract_years() > bound
            && manager.get::<u8>(0x178) == 0
            && manager.get::<u8>(0x179) & 1 == 0
        {
            return alert(UNDER_CONTRACT);
        }
        let club = (country as u8, number as i16);
        if a3_core::contract::talk_booked(records, index, club) {
            return alert(ALREADY_TALKING);
        }
        if a3_core::contract::board_busy(records, club) {
            return alert(BOARD_FULL);
        }
        let Some(other) = records.team(country, number) else { return Outcome::Stay };
        let class = usize::from(other.class()).min(9);
        let standing = usize::try_from(manager.standing()).unwrap_or(0).min(15);
        let chance = u32::from(CHANCES[standing][class]);
        // Too big for him: no chance there, while a smaller club has one.
        let too_big = chance == 0 && CHANCES[standing][..class].iter().any(|&v| v != 0);
        let engaged = other.engaged();
        let luck = records.luck.values()[usize::from(team.number()) % 30];
        // p: the run of form would lift it to 10 or sink it to 5 … 7, and
        // the own place in the danger zone to 4; abroad the manager's
        // foreign ability shrinks it (the fifth value of his abilities).
        let mut p = 6;
        if country as i16 != records.globals.main_country() {
            let ability = manager.abilities()[usize::from(manager.level()).min(15)][4];
            p = p * (i32::from(ability) * 4 + 80) / 100;
        }
        let answer = if too_big || (!at_once && engaged != 0) {
            ANSWER_UNFRIENDLY
        } else if cx.state.season.as_mut().map_or(0, |s| s.random.below(100)) >= chance {
            ANSWER_FRIENDLY
        } else if i32::from(luck) >= p {
            ANSWER_FRIENDLY
        } else {
            // Keen: the talk it asks for gets booked in the week
            // planner (412, kind 0x14, `FUN_006a8d67`).
            self.welcomed = Some(club);
            ANSWER_WELCOME
        };
        Outcome::Popup(Request::new(answer))
    }
}

/// The answer to an enquiry (dialogs 12, 13 and 14, `CAnfrageAbgelehntFr`,
/// `CAnfrageAbgelehntUnfr` and `CAnfrageAngenommen`): one picture in
/// control 1256 and the chairman's sentence, which stands in the file.
/// Closing it returns to the enquiry, which stays open throughout.
#[derive(Default)]
pub struct EnquiryAnswer;

impl Screen for EnquiryAnswer {
    fn ids(&self) -> &[u16] {
        &[ANSWER_FRIENDLY, ANSWER_UNFRIENDLY, ANSWER_WELCOME]
    }

    /// `0x442314`, `0x442494` and `0x442614`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        let picture = match view.id {
            ANSWER_UNFRIENDLY => PICTURE_RUDE,
            ANSWER_WELCOME => PICTURE_KEEN,
            _ => PICTURE_REFUSED,
        };
        if let Some(b) = control(dialog, ANSWER_PICTURE).and_then(|c| view.bitmap_mut(c)) {
            b.picture = Some(crate::controls::bitmap::Picture::graphic(picture));
        }
    }

    /// The one button each: 12's and 14's is id 1, 13's 1257.
    fn input(&mut self, _: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(_) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// The trainers' market (dialog 35, `CCoTrainerListe` for the
/// co-trainers, `CToTrainerlisteDialog` for the goalkeepers' coaches;
/// KONTAKTE.md §5): the country's free candidates by competence, and the
/// club's own trainer below. A row opens talks with that candidate.
#[derive(Default)]
pub struct Trainers {
    keepers: bool,
    /// Whether the popup last asked was the offer dialog and not an alert.
    offered: bool,
    /// Whether the last popup was the answer to "Vertrag verlängern", whose
    /// closing only refreshes the line naming the club's own coach.
    extended: bool,
}

const TRAINERS_DIALOG: u16 = 35;
/// The offer dialog (`CCoTrainerAngebotMachen`).
const TRAINERS_OFFER: u16 = 32;
/// A row whose trainer is no longer on the market, or no longer free
/// (`0x486ab9`).
const TRAINER_GONE: u16 = 4312;
const TRAINERS_LIST: u16 = 1359;
const EXTEND: u16 = 1361;
/// The answers to that button (dialogs 36 and 38,
/// `CCoTrainerKeineVerlaengerung` and `CCoTrainerVerlaengerung`) and their
/// texts; the ball they animate stands in control 1366.
const EXTEND_REFUSED: u16 = 36;
const EXTEND_DONE: u16 = 38;
const EXTEND_REFUSED_TEXT: u16 = 1364;
const EXTEND_DONE_TEXT: u16 = 1371;
const EXTEND_BALL: u16 = 1366;
const CURRENT: u16 = 1362;
const TRAINERS_HEADER: u16 = 1375;
/// The goalkeepers' caption and header, and the lines naming the club's
/// own trainer.
const KEEPERS_CAPTION: u16 = 2372;
const KEEPERS_HEADER: u16 = 3865;
const CURRENT_CO: u16 = 2370;
const CURRENT_KEEPERS: u16 = 2371;
/// What a trainer is known for, 3858 to 3864.
const REPUTATIONS: u16 = 3858;
const REPUTATIONS_LAST: u8 = 6;
/// "ein Jahr" … "zehn Jahre", else "%d Jahre".
const YEARS_TEXTS: u16 = 1081;
const YEARS_MANY: u16 = 1587;
/// The pools and the markets of both kinds; a market holds its count and
/// thirty entries of an id and that trainer's competence.
const CO_POOL: &str = "0xbe3920";
const KEEPERS_POOL: &str = "0xbe17f8";
const CO_MARKET: &str = "0x8e27a0";
const KEEPERS_MARKET: &str = "0x8e2490";
const MARKET_ENTRIES: usize = 30;
const MARKET_ENTRY: usize = 4;
/// A trainer's record: his name, competence, what he is known for, his
/// birthday, the years he is still under contract, his pay, the years he
/// asks for, and the managers he has turned down.
const NAMES_AT: usize = 0;
const COMPETENCE_AT: usize = 4;
const REPUTATION_AT: usize = 6;
const BORN_AT: usize = 8;
const UNDER_CONTRACT_AT: usize = 0xc;
const PAY_AT: usize = 0x10;
const YEARS_LEFT_AT: usize = 0xe;
const YEARS_ASKED_AT: usize = 0x15;
const TURNED_DOWN_AT: usize = 0x18;
/// The goalkeepers' header and list columns (`0x88b0a4`, `0x88b0b8`,
/// `0x88b0cc`, `0x88b0e0`).
const KEEPERS_HEAD_TABS: [f32; 5] = [0.0, 120.0, 170.0, 260.0, 360.0];
const KEEPERS_HEAD_ALIGNS: [Option<(a3_files::rch::Align, f32)>; 5] = [
    None,
    Some((a3_files::rch::Align::Right, 40.0)),
    Some((a3_files::rch::Align::Centre, 60.0)),
    Some((a3_files::rch::Align::Centre, 100.0)),
    Some((a3_files::rch::Align::Centre, 60.0)),
];
const KEEPERS_TABS: [f32; 5] = [0.0, 130.0, 190.0, 260.0, 370.0];
const KEEPERS_ALIGNS: [Option<(a3_files::rch::Align, f32)>; 5] = [
    None,
    Some((a3_files::rch::Align::Right, 20.0)),
    Some((a3_files::rch::Align::Right, 20.0)),
    Some((a3_files::rch::Align::Centre, 100.0)),
    Some((a3_files::rch::Align::Right, 20.0)),
];
/// The club's own co-trainer and goalkeepers' coach (team +0xa2, +0xa4).
const CO_AT: usize = 0xa2;
const KEEPERS_AT: usize = 0xa4;

/// A trainer of a pool, as the lists read him.
struct Trainer {
    name: String,
    age: u32,
    competence: i8,
    reputation: u8,
    pay: i32,
    years: u8,
    years_left: u8,
    under_contract: u8,
    turned_down: i16,
}

fn trainer(cx: &Cx, keepers: bool, id: i16) -> Option<Trainer> {
    let records = cx.state.records.as_ref()?;
    let pool = if keepers { KEEPERS_POOL } else { CO_POOL };
    let r = records.block(&format!("{pool}[{}]", usize::try_from(id).ok()?))?;
    let word = |at: usize| r.get(at..at + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]));
    let name = |at: usize| records.name(word(at) as u16).to_owned();
    let born = chrono::NaiveDate::from_ymd_opt(
        i32::from(word(BORN_AT + 2)),
        u32::from(*r.get(BORN_AT + 1)?),
        u32::from(*r.get(BORN_AT)?),
    );
    let today = cx.state.season.as_ref().map(a3_core::season::Season::date);
    Some(Trainer {
        name: format!("{} {}", name(NAMES_AT), name(NAMES_AT + 2)),
        age: born.zip(today).and_then(|(b, t)| t.years_since(b)).unwrap_or(0),
        competence: *r.get(COMPETENCE_AT)? as i8,
        reputation: *r.get(REPUTATION_AT)?,
        pay: i32::from_le_bytes(r.get(PAY_AT..PAY_AT + 4)?.try_into().ok()?),
        years: *r.get(YEARS_ASKED_AT)?,
        years_left: *r.get(YEARS_LEFT_AT)?,
        under_contract: *r.get(UNDER_CONTRACT_AT)?,
        turned_down: word(TURNED_DOWN_AT),
    })
}

/// What the head trainer knows, the yardstick for an extension
/// (`FUN_0040a1ec`): the manager's standing while he trains himself, else
/// the competence of the co-trainer record the club names, else the other
/// one's, else two.
fn head_competence(cx: &Cx, team: &a3_core::records::Team) -> i32 {
    if team.get::<u8>(0xa8) != 0 {
        return super::kalkulation::own_manager(cx).map_or(0, |m| m.standing());
    }
    for at in [0x9c, 0x9e] {
        let id = team.get::<i16>(at);
        if id != 0 {
            return trainer(cx, false, id).map_or(2, |t| i32::from(t.competence));
        }
    }
    2
}

impl Screen for Trainers {
    fn ids(&self) -> &[u16] {
        &[TRAINERS_DIALOG]
    }

    /// `0x48635c` and `0x5928ef`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.keepers = matches!(args, Args::Trainers { keepers: true });
        self.offered = false;
        self.extended = false;
        let caption =
            if self.keepers { game.text(KEEPERS_CAPTION).to_owned() } else { dialog.title.clone() };
        set(view, dialog, CAPTION_ID, caption);
        // The goalkeepers' list has five columns of its own.
        if self.keepers {
            set(view, dialog, TRAINERS_HEADER, game.text(KEEPERS_HEADER).to_owned());
            if let Some(header) = control(dialog, TRAINERS_HEADER).and_then(|c| view.header_mut(c))
            {
                header.tabs = KEEPERS_HEAD_TABS.into();
                header.aligns = KEEPERS_HEAD_ALIGNS.into();
            }
            if let Some(list) = control(dialog, TRAINERS_LIST).and_then(|c| view.list_mut(c)) {
                list.tabs = KEEPERS_TABS.into();
                list.aligns = KEEPERS_ALIGNS.into();
            }
        }
        self.fill(cx, view);
        // "Vertrag verlängern" is a static with SS_NOTIFY: it answers the
        // pointer like a button.
        if let Some(i) = control(dialog, EXTEND) {
            view.set_clickable(i, true);
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Target::Row { control: c, row } = target {
            if dialog.controls[c].id != Some(TRAINERS_LIST) {
                return Outcome::Stay;
            }
            let Some(id) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied()) else {
                return Outcome::Stay;
            };
            let id = id as i16;
            // Talks only while he is on the market and nobody still has him
            // under contract; the co-trainers say when he is not, the
            // goalkeepers' coaches say nothing (§5.5).
            if !self.on_market(cx, id) {
                return if self.keepers {
                    Outcome::Stay
                } else {
                    Outcome::Popup(Request::with(NOTICE, Args::Text(TRAINER_GONE)))
                };
            }
            self.offered = true;
            let args = Args::TrainerOffer { keepers: self.keepers, id };
            return Outcome::Popup(Request::with(TRAINERS_OFFER, args));
        }
        match target {
            // "Vertrag verlängern": his record says whether he extends,
            // and if he does it gains a year (§5.8).
            Target::Control(c) if dialog.controls[c].id == Some(EXTEND) => {
                let Some(team) = own_team(cx) else { return Outcome::Stay };
                let own = team.get::<i16>(if self.keepers { KEEPERS_AT } else { CO_AT });
                let Some(t) = (own != 0).then(|| trainer(cx, self.keepers, own)).flatten() else {
                    return Outcome::Stay;
                };
                let head = head_competence(cx, team);
                let pool = if self.keepers { KEEPERS_POOL } else { CO_POOL };
                if trainers::may_extend(
                    i32::from(t.years_left),
                    t.age as i32,
                    i32::from(t.competence),
                    head,
                    i64::from(t.pay),
                    self.keepers,
                ) {
                    // The year named is the one the season ends in, plus the
                    // new years before him (`DAT_00b5cbca + years`): a season
                    // counted from 2025 ends in 2026.
                    let until =
                        cx.state.season.as_ref().map_or(0, |s| s.year) + i32::from(t.years_left) + 2;
                    if let Some(at) = cx
                        .state
                        .records
                        .as_mut()
                        .and_then(|rs| {
                            rs.block_mut(&format!(
                                "{pool}[{}]",
                                usize::try_from(own).unwrap_or(0)
                            ))
                        })
                        .and_then(|r| r.get_mut(YEARS_LEFT_AT))
                    {
                        *at = at.saturating_add(1);
                    }
                    self.extended = true;
                    let args =
                        Args::TrainerExtend { dialog: EXTEND_DONE, keepers: self.keepers, id: own, until };
                    return Outcome::Popup(Request::with(EXTEND_DONE, args));
                }
                let args =
                    Args::TrainerExtend { dialog: EXTEND_REFUSED, keepers: self.keepers, id: own, until: 0 };
                Outcome::Popup(Request::with(EXTEND_REFUSED, args))
            }
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }

    /// What the offer chain had to say: with a trainer hired the market
    /// closes with him (the list's `EndDialog(1)`); after failed talks its
    /// rows show again, the turned-down one gone (§5.5). The extension's
    /// answer only refreshes the line naming the club's own coach.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.extended) {
            self.fill(cx, view);
            return Outcome::Stay;
        }
        if !std::mem::take(&mut self.offered) {
            return Outcome::Stay;
        }
        match reply {
            Reply::Ok => Outcome::Close(Reply::Ok),
            _ => {
                self.fill(cx, view);
                Outcome::Stay
            }
        }
    }
}

impl Trainers {
    /// The candidates of the country's market by competence, the ids the
    /// rows stand for, and the club's own trainer on the line below.
    fn fill(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let current = cx.state.season.as_ref().map_or(0, |s| s.current);
        let number = records.human_slot(current).map_or(1, |s| s as i16 + 1);
        let market = if self.keepers { KEEPERS_MARKET } else { CO_MARKET };
        let b = usize::from(records.countries.slot(team.country().into()));
        let ids: Vec<i16> = records
            .block(&format!("{market}[{b}]"))
            .map(|m| {
                (0..market_count(m).min(MARKET_ENTRIES))
                    .filter_map(|k| Some(market_entry(m, k)?.0))
                    .filter(|&id| id > 0)
                    .collect()
            })
            .unwrap_or_default();
        let mut found: Vec<(i16, Trainer)> =
            ids.into_iter().filter_map(|id| Some((id, trainer(cx, self.keepers, id)?))).collect();
        // The game's own exchange sort, which does not keep the order of
        // equal competences.
        for i in 0..found.len() {
            for j in i + 1..found.len() {
                if found[i].1.competence < found[j].1.competence {
                    found.swap(i, j);
                }
            }
        }
        let what = |t: &Trainer| {
            game.text(REPUTATIONS + u16::from(t.reputation.min(REPUTATIONS_LAST))).to_owned()
        };
        let kept: Vec<&(i16, Trainer)> = found
            .iter()
            .filter(|(_, t)| t.turned_down & (1 << number) == 0 && t.under_contract == 0)
            .collect();
        let rows: Vec<String> = kept
            .iter()
            .map(|(_, t)| {
                let pay = super::playerinfo::money(game, t.pay.into());
                if self.keepers {
                    format!("{}\t{}\t{}\t{pay}\t{}", t.name, t.age, t.competence, t.years)
                } else {
                    let what = what(t);
                    format!("{}\t{}\t{}\t{what}\t{pay}\t{}", t.name, t.age, t.competence, t.years)
                }
            })
            .collect();
        if let Some(list) = control(dialog, TRAINERS_LIST).and_then(|c| view.list_mut(c)) {
            list.rows = rows;
            list.keys = kept.iter().map(|(id, _)| *id as usize).collect();
            list.selected = None;
        }
        // The club's own trainer, or neither line.
        let own = team.get::<i16>(if self.keepers { KEEPERS_AT } else { CO_AT });
        let line = (own != 0).then(|| trainer(cx, self.keepers, own)).flatten().map(|t| {
            let pay = super::playerinfo::money(game, t.pay.into());
            let years = match t.under_contract {
                y @ 1..=10 => game.text(YEARS_TEXTS + u16::from(y) - 1).to_owned(),
                y => game.text(YEARS_MANY).replacen("%d", &y.to_string(), 1),
            };
            let text = game.text(if self.keepers { CURRENT_KEEPERS } else { CURRENT_CO });
            let known = what(&t);
            let mut parts: Vec<&dyn std::fmt::Display> = vec![&t.name, &pay, &years, &t.competence];
            if !self.keepers {
                parts.push(&known);
            }
            super::finances::printf(text, &parts)
        });
        match line {
            Some(text) => set(view, dialog, CURRENT, text),
            None => {
                for id in [CURRENT, EXTEND] {
                    if let Some(i) = control(dialog, id) {
                        view.controls[i].hidden = true;
                    }
                }
            }
        }
    }

    /// Whether talks can still be opened with him: he stands in the
    /// country's market block, and his record keeps no year left for him
    /// with anyone (record +0xe).
    fn on_market(&self, cx: &Cx, id: i16) -> bool {
        let (Some(records), Some(team)) = (cx.state.records.as_ref(), own_team(cx)) else {
            return false;
        };
        let b = usize::from(records.countries.slot(team.country().into()));
        let (market, pool) =
            if self.keepers { (KEEPERS_MARKET, KEEPERS_POOL) } else { (CO_MARKET, CO_POOL) };
        let listed = records.block(&format!("{market}[{b}]")).is_some_and(|m| {
            (0..market_count(m).min(MARKET_ENTRIES))
                .any(|k| market_entry(m, k).is_some_and(|(entry, _)| entry == id))
        });
        let free = records
            .block(&format!("{pool}[{}]", usize::try_from(id).unwrap_or(0)))
            .and_then(|r| r.get(YEARS_LEFT_AT))
            == Some(&0);
        listed && free
    }
}

/// A market block: how many candidates it holds.
fn market_count(m: &[u8]) -> usize {
    m.get(..2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]).max(0) as usize)
}

/// Its entry of a trainer's id and his competence.
fn market_entry(m: &[u8], k: usize) -> Option<(i16, i16)> {
    let at = 2 + k * MARKET_ENTRY;
    let e = m.get(at..at + MARKET_ENTRY)?;
    Some((i16::from_le_bytes([e[0], e[1]]), i16::from_le_bytes([e[2], e[3]])))
}

/// The market block after one of its trainers was hired: he is taken out
/// of it, and the coach sent away goes back in (`FUN_0040ae73` and
/// `FUN_0040b142`).
fn market_leave(m: Option<&mut [u8]>, gone: i16, back: i16, back_competence: i16) {
    let Some(m) = m else { return };
    let mut count = market_count(m);
    let hit = (0..count.min(MARKET_ENTRIES)).find(|&k| {
        market_entry(m, k).is_some_and(|(id, _)| id == gone)
    });
    if let Some(k) = hit {
        for j in k..count - 1 {
            let (id, competence) = market_entry(m, j + 1).unwrap_or((0, 0));
            let Some(at) = m.get_mut(2 + j * MARKET_ENTRY..2 + j * MARKET_ENTRY + MARKET_ENTRY)
            else {
                break;
            };
            at[0..2].copy_from_slice(&id.to_le_bytes());
            at[2..4].copy_from_slice(&competence.to_le_bytes());
        }
        count -= 1;
    }
    let listed = (0..count).any(|k| market_entry(m, k).is_some_and(|(id, _)| id == back));
    if back != 0 && !listed && count < MARKET_ENTRIES {
        if let Some(at) = m.get_mut(2 + count * MARKET_ENTRY..2 + count * MARKET_ENTRY + MARKET_ENTRY)
        {
            at[0..2].copy_from_slice(&back.to_le_bytes());
            at[2..4].copy_from_slice(&back_competence.to_le_bytes());
        }
        count += 1;
    }
    if let Some(at) = m.get_mut(..2) {
        at.copy_from_slice(&(count as i16).to_le_bytes());
    }
}

/// One year as the offer dialog says it, else so many years
/// (`FUN_00730c8d`).
fn years_short(game: &Game, years: i32) -> String {
    if years < 2 {
        game.text(ONE_YEAR).to_owned()
    } else {
        game.text(MANY_YEARS).replacen("%d", &years.to_string(), 1)
    }
}

/// One to ten years as the compromise's words say them, and more than
/// those (`FUN_00730bdc`).
fn years_word(game: &Game, years: i32) -> String {
    match u32::try_from(years) {
        Ok(y @ 1..=10) => game.text(YEARS_TEXTS + u16::try_from(y - 1).unwrap_or(0)).to_owned(),
        _ => game.text(YEARS_MANY).replacen("%d", &years.to_string(), 1),
    }
}

/// The offer dialog (32, `CCoTrainerAngebotMachen`; KONTAKTE.md §5.6): a
/// yearly salary on the slider and years on the spin against what the
/// candidate's record says he asks. He takes it, makes one last compromise,
/// or turns it down for good; the news dialogs of that outcome are led from
/// here.
#[derive(Default)]
pub struct TrainerOffer {
    keepers: bool,
    id: i16,
    /// What his record says he asks: a yearly salary and years.
    demand: i64,
    asked_years: i32,
    /// What slider and spin stand on.
    salary: i64,
    years: i32,
    /// The contract about to be made, and what sending the old coach away
    /// costs.
    pending: (i64, i32),
    severance: i64,
    stage: Stage,
}

/// How far the talks got.
#[derive(Default, PartialEq)]
enum Stage {
    #[default]
    Idle,
    /// Turned down; its news dialog has only to close.
    Failed,
    /// His last compromise; to take it or to leave it.
    Compromise,
    /// Taken, but the old coach wants his severance first.
    Severance,
    /// Hired; its news dialog closes the market.
    Done,
}

/// The goalkeepers' captions for the offer and for the news dialogs; the
/// severance notice keeps its own title.
const OFFER_CAPTION: u16 = 1449;
const NEWS_CAPTION: u16 = 1450;
const OFFER_HEAD: u16 = 1353;
/// The money and years he asks, and the ones offered.
const OFFER_DEMAND: u16 = 1346;
const OFFER_ASKED_YEARS: u16 = 1348;
const OFFER_OFFER: u16 = 1342;
const OFFER_YEARS: u16 = 1344;
const OFFER_SPIN: u16 = 1349;
const OFFER_SLIDER: u16 = 1350;
const OFFER_OK: u16 = 1351;
const OFFER_BREAK: u16 = 1352;
/// "1 Jahr" and "%d Jahre", the offer dialog's way of counting.
const ONE_YEAR: u16 = 1007;
const MANY_YEARS: u16 = 4956;
/// The news dialogs and their texts; 37 has the one refusal button.
const NEWS_DIALOGS: [u16; 4] = [SEVER_NEWS, FAILED_NEWS, PROMISE_NEWS, DONE_NEWS];
const SEVER_NEWS: u16 = 31;
const FAILED_NEWS: u16 = 34;
const PROMISE_NEWS: u16 = 37;
const DONE_NEWS: u16 = 39;
const SEVER_TEXT: u16 = 1338;
const FAILED_TEXT: u16 = 1357;
const PROMISE_TEXT: u16 = 1367;
const PROMISE_BREAK: u16 = 1369;
const DONE_TEXT: u16 = 1373;
/// The balls, and the frames they loop through; the one of failed talks
/// comes from further along the film.
const BALLS: [(u16, std::ops::RangeInclusive<u16>); 4] = [
    (1340, 1762..=1773),
    (1358, 1778..=1783),
    (1370, 1762..=1773),
    (1375, 1762..=1773),
];
/// The staff accounts the severance is booked on: a co-trainer two of them,
/// a goalkeepers' coach one (club +0x280/+0x264 and +0x268).
const SEVERANCE_ACCOUNTS: &[usize] = &[0x4f8, 0x4dc];
const SEVERANCE_ACCOUNT_KEEPERS: &[usize] = &[0x4e0];

impl Screen for TrainerOffer {
    fn ids(&self) -> &[u16] {
        &[TRAINERS_OFFER]
    }

    /// `FUN_00485059`: what he asks stands in his record; the offer starts
    /// at half of it, snapped onto the money scale, and with the years he
    /// asks for.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Args::TrainerOffer { keepers, id } = args else { return };
        *self = Self { keepers, id, ..Self::default() };
        let caption =
            if self.keepers { game.text(OFFER_CAPTION).to_owned() } else { dialog.title.clone() };
        set(view, dialog, CAPTION_ID, caption);
        let Some(t) = trainer(cx, self.keepers, self.id) else { return };
        self.demand = i64::from(t.pay);
        self.asked_years = i32::from(t.years);
        self.salary = trainers::at_position(trainers::position(self.demand / 2 / 1000)) * 1000;
        self.years = self.asked_years.clamp(1, 5);
        // The original puts only the trainer's name on the yellow band,
        // though the file's text reads "Verhandlung mit %s".
        set(view, dialog, OFFER_HEAD, t.name.clone());
        let demand = super::playerinfo::money(game, self.demand);
        let asked = years_short(game, self.asked_years);
        let offer = super::playerinfo::money(game, self.salary);
        let years = years_short(game, self.years);
        set(view, dialog, OFFER_DEMAND, demand);
        set(view, dialog, OFFER_ASKED_YEARS, asked);
        set(view, dialog, OFFER_OFFER, offer);
        set(view, dialog, OFFER_YEARS, years);
        if let Some(slider) = control(dialog, OFFER_SLIDER).and_then(|c| view.value_mut(c)) {
            slider.range = (0, trainers::position(trainers::OFFER_MOST / 1000) as i32);
            slider.set(trainers::position(self.salary / 1000) as i32);
        }
        if let Some(spin) = control(dialog, OFFER_SPIN).and_then(|c| view.value_mut(c)) {
            spin.range = (1, 5);
            spin.set(self.years);
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            // View has moved slider and spin along; the money snaps onto
            // the scale and the dialog says what it became (`FUN_004853fa`,
            // `FUN_00485459`).
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. }
                if id(c) == OFFER_SLIDER =>
            {
                let at = view.value(c).map_or(0, |v| v.value);
                self.salary = trainers::at_position(at.into()) * 1000;
                let money = super::playerinfo::money(cx.game, self.salary);
                set(view, dialog, OFFER_OFFER, money);
                Outcome::Stay
            }
            Target::Spin { control: c, .. } if id(c) == OFFER_SPIN => {
                self.years = view.value(c).map_or(1, |v| v.value);
                let word = years_short(cx.game, self.years);
                set(view, dialog, OFFER_YEARS, word);
                Outcome::Stay
            }
            Target::Button(c) if id(c) == OFFER_BREAK => Outcome::Close(Reply::Cancel),
            Target::Button(c) if id(c) == OFFER_OK => self.judge(cx),
            _ => Outcome::Stay,
        }
    }

    /// The chain of the talks' outcome (`0x486b84` onwards in
    /// `CCoTrainerListe`): failed talks end back at the list, a compromise
    /// is taken or left, the severance is shown and then the contract.
    fn answer(&mut self, cx: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        match self.stage {
            Stage::Idle => Outcome::Stay,
            Stage::Failed => {
                self.stage = Stage::Idle;
                Outcome::Close(Reply::Cancel)
            }
            Stage::Compromise => match reply {
                Reply::Ok => {
                    let (money, years) = self.pending;
                    self.stage = Stage::Idle;
                    self.sign(cx, money, years)
                }
                _ => {
                    self.turn_down(cx);
                    self.stage = Stage::Failed;
                    news(FAILED_NEWS, self.keepers, self.id, 0, 0)
                }
            },
            Stage::Severance => {
                let (money, years) = self.pending;
                self.hire(cx, money, years);
                self.stage = Stage::Done;
                news(DONE_NEWS, self.keepers, self.id, 0, 0)
            }
            Stage::Done => Outcome::Close(Reply::Ok),
        }
    }
}

impl TrainerOffer {
    /// The offer taken as it stands: first the old coach's severance when
    /// there is one, then the new contract, then the news of it.
    fn sign(&mut self, cx: &mut Cx, money: i64, years: i32) -> Outcome {
        use chrono::Datelike;
        self.pending = (money, years);
        let old =
            own_team(cx).map_or(0, |t| t.get::<i16>(if self.keepers { KEEPERS_AT } else { CO_AT }));
        if old != 0 {
            let month = cx.state.season.as_ref().map_or(12, |s| s.date().month());
            let (pay, left) = trainer(cx, self.keepers, old)
                .map_or((0, 0), |t| (i64::from(t.pay), i32::from(t.years_left)));
            self.severance = trainers::severance(pay, left, month);
            self.stage = Stage::Severance;
            return news(SEVER_NEWS, self.keepers, old, self.severance, 0);
        }
        self.hire(cx, money, years);
        self.stage = Stage::Done;
        news(DONE_NEWS, self.keepers, self.id, 0, 0)
    }

    /// The trainer's say over the offer (`FUN_004854c3` at the offer's OK
    /// button), and the chain it opens.
    fn judge(&mut self, cx: &mut Cx) -> Outcome {
        let (salary, years, demand, asked) =
            (self.salary, self.years, self.demand, self.asked_years);
        let verdict = match cx.state.season.as_mut() {
            Some(season) => trainers::negotiate(salary, years, demand, asked, &mut season.random),
            None => trainers::negotiate(salary, years, demand, asked, &mut Random::new(0)),
        };
        match verdict {
            -1 => self.sign(cx, salary, years),
            1 => {
                // The last compromise is computed at once, before its
                // dialog shows, drawing after the judgement (`FUN_00485f3a`).
                let promise = match cx.state.season.as_mut() {
                    Some(season) => {
                        trainers::compromise(demand, salary, years, asked, &mut season.random)
                    }
                    None => {
                        let mut random = Random::new(0);
                        trainers::compromise(demand, salary, years, asked, &mut random)
                    }
                };
                self.pending = promise;
                self.stage = Stage::Compromise;
                news(PROMISE_NEWS, self.keepers, self.id, promise.0, promise.1)
            }
            _ => {
                self.turn_down(cx);
                self.stage = Stage::Failed;
                news(FAILED_NEWS, self.keepers, self.id, 0, 0)
            }
        }
    }

    /// The new trainer's contract, the old coach sent away and the market
    /// the two leave and come back to (`FUN_004870e1`). The goalkeepers'
    /// coaches go without the market.
    fn hire(&mut self, cx: &mut Cx, money: i64, years: i32) {
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let (country, number) = (team.country(), team.number());
        let old = team.get::<i16>(if self.keepers { KEEPERS_AT } else { CO_AT });
        let b = usize::from(records.countries.slot(country.into()));
        let old_competence =
            if self.keepers || old == 0 { 0 } else { trainer(cx, false, old).map_or(0, |t| t.competence as i16) };
        let (market, pool) =
            if self.keepers { (KEEPERS_MARKET, KEEPERS_POOL) } else { (CO_MARKET, CO_POOL) };
        let key = |pool: &str, id: i16| format!("{pool}[{}]", usize::try_from(id).unwrap_or(0));
        let Some(records) = cx.state.records.as_mut() else { return };
        if let Some(r) = records.block_mut(&key(pool, self.id)) {
            let pay = i32::try_from(money).unwrap_or(i32::MAX);
            if let Some(at) = r.get_mut(PAY_AT..PAY_AT + 4) {
                at.copy_from_slice(&pay.to_le_bytes());
            }
            if let Some(at) = r.get_mut(YEARS_LEFT_AT) {
                *at = years as u8;
            }
        }
        if old != 0 {
            if let Some(at) =
                records.block_mut(&key(pool, old)).and_then(|r| r.get_mut(YEARS_LEFT_AT))
            {
                *at = 0;
            }
        }
        if let Some(team) = records.team_mut(country.into(), number.into()) {
            let accounts = if self.keepers { SEVERANCE_ACCOUNT_KEEPERS } else { SEVERANCE_ACCOUNTS };
            for &at in accounts {
                team.put::<i32>(at, team.get::<i32>(at).wrapping_add(self.severance as i32));
            }
            team.put::<i16>(if self.keepers { KEEPERS_AT } else { CO_AT }, self.id);
        }
        if !self.keepers {
            market_leave(records.block_mut(&format!("{market}[{b}]")), self.id, old, old_competence);
        }
    }

    /// A trainer turned down stays turned down with this manager, and the
    /// list hides him (§5.5).
    fn turn_down(&mut self, cx: &mut Cx) {
        let current = cx.state.season.as_ref().map_or(0, |s| s.current);
        let number = cx
            .state
            .records
            .as_ref()
            .and_then(|r| r.human_slot(current))
            .map_or(1, |s| s as u32 + 1);
        let pool = if self.keepers { KEEPERS_POOL } else { CO_POOL };
        let Some(r) = cx
            .state
            .records
            .as_mut()
            .and_then(|rs| rs.block_mut(&format!("{pool}[{}]", usize::try_from(self.id).unwrap_or(0))))
        else {
            return;
        };
        if let Some(at) = r.get_mut(TURNED_DOWN_AT..TURNED_DOWN_AT + 2) {
            let turned_down = u16::from_le_bytes([at[0], at[1]]) | (1 << (number & 31));
            at.copy_from_slice(&turned_down.to_le_bytes());
        }
    }
}

/// One of the talks' outcome dialogs, opened from the offer screen.
fn news(dialog: u16, keepers: bool, id: i16, amount: i64, years: i32) -> Outcome {
    let args = Args::TrainerNews { dialog, keepers, id, amount, years };
    Outcome::Popup(Request::with(dialog, args))
}

/// What the talks brought (dialogs 31, 34, 37 and 39, `CCoTrainerAbfindung`,
/// `CCoTrainerGescheitert`, `CCoTrainerLetztesAngebot` and
/// `CCoTrainerVerpflichtung`): what sending the old coach away costs, that
/// talks came to nothing, his last compromise, and at last that he is here.
/// The offer dialog leads them; the market's own rows wait below.
#[derive(Default)]
pub struct TrainerNews;

impl Screen for TrainerNews {
    fn ids(&self) -> &[u16] {
        &NEWS_DIALOGS
    }

    /// Each names the trainer from his record, and the money; 37 asks
    /// whether to take the last compromise.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Args::TrainerNews { dialog: which, keepers, id, amount, years } = args else { return };
        // The goalkeepers' coaches have their own captions, save the
        // severance notice, which keeps its title.
        let caption = if keepers && which != SEVER_NEWS {
            game.text(NEWS_CAPTION).to_owned()
        } else {
            dialog.title.clone()
        };
        set(view, dialog, CAPTION_ID, caption);
        let name = trainer(cx, keepers, id).map_or_else(String::new, |t| t.name);
        let money = super::playerinfo::money(game, amount);
        let word = years_word(game, years);
        let (text_id, parts): (u16, &[&dyn std::fmt::Display]) = match which {
            SEVER_NEWS => (SEVER_TEXT, &[&name, &money]),
            PROMISE_NEWS => (PROMISE_TEXT, &[&name, &money, &word]),
            FAILED_NEWS => (FAILED_TEXT, &[&name]),
            _ => (DONE_TEXT, &[&name]),
        };
        if let Some(c) = control(dialog, text_id) {
            let text = super::finances::printf(&dialog.controls[c].text, parts);
            set(view, dialog, text_id, text);
        }
        for (ball, frames) in BALLS {
            if let Some(ball) = control(dialog, ball).and_then(|b| view.anim_mut(b)) {
                ball.frames = frames.collect();
                ball.playing = true;
            }
        }
    }

    /// 37 asks; the others only tell.
    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if id_is(cx, c, PROMISE_BREAK) => Outcome::Close(Reply::Cancel),
            Target::Button(c) if crate::controls::button::is_ok(&cx.dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}

/// The answer to "Vertrag verlängern" (dialogs 36 and 38,
/// `CCoTrainerKeineVerlaengerung` and `CCoTrainerVerlaengerung`): he won't,
/// or he agrees and his contract runs a year longer. The market opens them
/// and refreshes its line when they close; the salary stays (§5.8).
#[derive(Default)]
pub struct TrainerExtend;

impl Screen for TrainerExtend {
    fn ids(&self) -> &[u16] {
        &[EXTEND_REFUSED, EXTEND_DONE]
    }

    /// `0x485ac2` and `0x487682`: his name, and of the one that agrees the
    /// year his contract now runs to.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        let Args::TrainerExtend { dialog: which, keepers, id, until } = args else { return };
        let name = trainer(cx, keepers, id).map_or_else(String::new, |t| t.name);
        let (text_id, parts): (u16, Vec<&dyn std::fmt::Display>) = if which == EXTEND_DONE {
            (EXTEND_DONE_TEXT, vec![&name, &until])
        } else {
            (EXTEND_REFUSED_TEXT, vec![&name])
        };
        if let Some(c) = control(dialog, text_id) {
            let filled = super::finances::printf(&dialog.controls[c].text, &parts);
            set(view, dialog, text_id, filled);
        }
        if let Some(ball) = control(dialog, EXTEND_BALL).and_then(|b| view.anim_mut(b)) {
            ball.frames = (1762u16..=1773).collect();
            ball.playing = true;
        }
    }

    /// Only to close again; the market stays open. Each has one button —
    /// the agreeing one's drawn with the cancel face, but its handler
    /// (`0x4877d9`) just ends the dialog like the other's OK.
    fn input(&mut self, _: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(_) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// The control's id, or 0.
fn id_is(cx: &Cx, c: usize, want: u16) -> bool {
    cx.dialog.controls[c].id == Some(want)
}
