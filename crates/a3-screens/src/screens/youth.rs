//! The youth and the amateurs: other clubs' youths without a pre-contract
//! (dialog 107, `CJugendspielerAbwerben`), the pre-contract their parents
//! agree to (dialog 325, `CVorvertragAbschliessen`;
//! VERLAENGERUNG-ABWERBEN.md §3–4 in the RE repo), the club's own youth
//! (dialog 428, `CJugendUndAmateure`), its amateurs (dialog 427,
//! `CAmateure`) and a trial with the professionals (dialog 11,
//! `CAmateurspielerProbetraining`; JUGEND-AMATEURE.md).

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::{
    AMATEUR_PLAYERS, NOTICE, PRE_CONTRACT, QUESTION, TRIAL, YOUTH_POACHING, YOUTH_TEAM,
};
use a3_core::records::Team;
use a3_core::youth;
use a3_files::rch::Dialog;
use tracing::info;

use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

const CAPTION_ID: u16 = 9935;
const POACHING_LIST: u16 = 2124;
/// A human club's youths show four weeks after they were entered.
const HUMAN_WAIT: i32 = 28;
/// The main positions, keeper to striker, in the list's order.
const POSITIONS: std::ops::RangeInclusive<u8> = 1..=10;
/// The youth classes' names, C to A; the talents', huge to none.
const CLASSES: [u16; 3] = [1674, 1675, 1676];
const TALENTS: u16 = 4074;
const BLACK: [u8; 3] = [0, 0, 0];
const FULL_TEXT: u16 = 2168;
const PRE_CONTRACT_TEXT: u16 = 4189;
const AGREE: u16 = 4190;
const DISAGREE: u16 = 4191;

/// Their captions, the file's title to start with.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [YOUTH_POACHING, PRE_CONTRACT, YOUTH_TEAM, AMATEUR_PLAYERS, TRIAL] {
        if let Some(dialog) = dialogs.get_mut(&id) {
            let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
            dialog.controls.insert(0, caption);
        }
    }
}

/// Other clubs' youths of the country without a pre-contract, by main
/// position; a click offers one a pre-contract. The list has no row menu.
#[derive(Default)]
pub struct Poaching {
    /// The youth 325 asks about, once it closes.
    asked: Option<u16>,
}

impl Screen for Poaching {
    fn ids(&self) -> &[u16] {
        &[YOUTH_POACHING]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        self.asked = None;
        fill(cx, view);
        mark_inert(view, cx.dialog, |_| true);
    }

    /// `OnSelChange_IDC_JUGENDABWERBEN_LISTE`: the alerts keep the row
    /// selected; 325 refills the list.
    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let notice = |args| Outcome::Popup(Request::with(NOTICE, args));
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Row { control: c, row } if dialog.controls[c].id == Some(POACHING_LIST) => {
                let Some(slot) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied())
                else {
                    return Outcome::Stay;
                };
                let (Some(records), Some(team)) = (cx.state.records.as_ref(), own_team(cx)) else {
                    return Outcome::Stay;
                };
                let Some(prospect) = youth::prospect_in(records, slot) else {
                    return notice(Args::Message(youth::CANNOT_TALK));
                };
                let (youths, more) = youth::youths(records, team.country(), team.number().into());
                if youths.len() + more >= youth::FULL {
                    return notice(Args::Text(FULL_TEXT));
                }
                if records.player(prospect.id).is_none_or(|r| r.club()[0] == 0) {
                    return notice(Args::Message(youth::WONT_MOVE));
                }
                self.asked = Some(prospect.id);
                Outcome::Popup(Request::with(PRE_CONTRACT, Args::Record(prospect.id)))
            }
            _ => Outcome::Stay,
        }
    }

    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        let Some(id) = self.asked.take() else { return Outcome::Stay };
        if reply == Reply::Ok
            && let Some(salary) = pre_contract(cx, id)
            && let Some(club) = own_team(cx).map(|t| (t.country(), u16::from(t.number())))
            && let Some(records) = cx.state.records.as_mut()
        {
            youth::sign(records, id, club, salary);
        }
        fill(cx, view);
        Outcome::Stay
    }
}

/// The rows (`FUN_004d6bba`): the list's youths of other clubs of the
/// country, a human club's after four weeks, stable by main position;
/// each row keyed by its slot.
fn fill(cx: &Cx, view: &mut View) {
    let (game, dialog) = (cx.game, cx.dialog);
    let (Some(records), Some(team), Some(season)) =
        (cx.state.records.as_ref(), own_team(cx), cx.state.season.as_ref())
    else {
        return;
    };
    let today = i32::from(records.globals.day());
    let own = (team.country(), i16::from(team.number()));
    let human = |country: u8, number: i16| {
        u16::try_from(number)
            .ok()
            .and_then(|n| records.team(country.into(), n))
            .is_some_and(|t| t.human() != 0)
    };
    let shown: Vec<_> = youth::prospects(records)
        .into_iter()
        .filter(|p| (p.country, p.number) != own && p.country == team.country())
        .filter(|p| !human(p.country, p.number) || i32::from(p.day) <= today - HUMAN_WAIT)
        .filter_map(|p| Some((p, records.player(p.id)?.positions()[0])))
        .collect();
    let (mut rows, mut keys) = (Vec::new(), Vec::new());
    for position in POSITIONS {
        for &(p, _) in shown.iter().filter(|(_, main)| *main == position) {
            let Some(r) = records.player(p.id) else { continue };
            let Some((player, _, status)) = a3_core::load::any_player(records, &game.data, p.id)
            else {
                continue;
            };
            let talent = TALENTS + u16::try_from(r.talent()).unwrap_or(0);
            rows.push(
                [
                    game.text(CLASSES[youth::class(records, p.id)]).to_owned(),
                    super::duties::short_name(cx, &player, p.id),
                    player.age(season.date()).to_string(),
                    super::hub::abbreviation(game, position),
                    r.strength().to_string(),
                    game.text(talent).to_owned(),
                    game.world
                        .nation_codes
                        .get(usize::from(status.nations[0]))
                        .cloned()
                        .unwrap_or_default(),
                    game.club_name(Some(records), p.country, p.number),
                ]
                .join("\t"),
            );
            keys.push(p.slot);
        }
    }
    if let Some(list) = control(dialog, POACHING_LIST).and_then(|c| view.list_mut(c)) {
        list.inks = vec![Some(BLACK); rows.len()];
        list.rows = rows;
        list.keys = keys;
        list.scroll = 0;
        list.selected = None;
    }
}

/// The salary a youth's parents agree to with the manager's club
/// (`FUN_0066b648`), by his age today.
fn pre_contract(cx: &Cx, id: u16) -> Option<i32> {
    let records = cx.state.records.as_ref()?;
    let (player, ..) = a3_core::load::any_player(records, &cx.game.data, id)?;
    let age = player.age(cx.state.season.as_ref()?.date());
    Some(youth::salary(id, i32::try_from(age).ok()?, own_team(cx)?.level()))
}

/// The pre-contract a youth's parents agree to (dialog 325).
pub struct PreContract;

impl Screen for PreContract {
    fn ids(&self) -> &[u16] {
        &[PRE_CONTRACT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        if let Args::Record(id) = args {
            let name = cx.state.records.as_ref().and_then(|r| r.known_as(id)).unwrap_or_default();
            let salary = pre_contract(cx, id).unwrap_or(0);
            let text = control(dialog, PRE_CONTRACT_TEXT).map(|c| dialog.controls[c].text.clone());
            let text = text.unwrap_or_default().replacen("%s", &name, 1).replacen(
                "%s",
                &super::playerinfo::money(game, salary.into()),
                1,
            );
            set(view, dialog, PRE_CONTRACT_TEXT, text);
        }
        super::market::question_ball(view, dialog);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(AGREE) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Button(c) if cx.dialog.controls[c].id == Some(DISAGREE) => {
                Outcome::Close(Reply::Cancel)
            }
            _ => Outcome::Stay,
        }
    }
}

/// The three lists, A to C, and the age group each shows.
const YOUTH_LISTS: [(u16, usize); 3] = [(5264, 2), (5263, 1), (5262, 0)];
const FOCUS: u16 = 5270;
const FOCUS_TEXTS: std::ops::RangeInclusive<u16> = 3446..=3452;
/// The manager record's youth training focus.
const FOCUS_AT: usize = 0x14a9;
/// The age groups' league tables (430), not built.
const TABLES: u16 = 5271;
const SUPPORT_COLUMN: usize = 7;
/// Strong enough for his age group's national team ("JNS"), C to A.
const NATIONAL_STRENGTH: [u8; 3] = [2, 3, 4];
const MARKED: &str = "X";
const NO_PRE_CONTRACT: u16 = 2616;
/// The plaster of an injured youth, in the Status column.
const PLASTER: u16 = 1281;
const PLASTER_SLOT: usize = 3;
const ABILITY_SLOTS: usize = 3;
/// A professional contract wants a youth of 17; a squad takes 35.
const PRO_AGE: u32 = 17;
const SQUAD_MOST: usize = 35;
const SQUAD_FULL: u16 = 2462;
const TOO_YOUNG: u16 = 2435;
const TALK_BOOKED: u16 = 3115;
const TRIAL_QUESTION: u16 = 4302;

/// The club's youth in three lists by age group (dialog 428): a click in
/// "Förd." moves the group's support, elsewhere it is the youth's way up —
/// a pre-contract first, then a trial with the professionals at 17.
#[derive(Default)]
pub struct Youths {
    /// The youth a popup asks about, and what it asks.
    asked: Option<(u16, Ask)>,
    /// An amateur department sees its professionals' youth, and may not
    /// touch it.
    locked: bool,
}

#[derive(Clone, Copy)]
enum Ask {
    PreContract(i32),
    Trial,
}

impl Screen for Youths {
    fn ids(&self) -> &[u16] {
        &[YOUTH_TEAM]
    }

    /// `FUN_004d7bfa`: the focus, the lists, the caption with the club.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        let Some(team) = own_team(cx) else { return };
        self.locked = team.linked()[0] >= 0;
        set(view, dialog, CAPTION_ID, format!("{} {}", dialog.title, team.name()));
        let focus = own_manager(cx).map_or(0, |m| m.get::<u8>(FOCUS_AT));
        if let Some(combo) = control(dialog, FOCUS).and_then(|c| view.combo_mut(c)) {
            combo.items = FOCUS_TEXTS.map(|t| game.text(t).to_owned()).collect();
            combo.selected = Some(usize::from(focus));
            combo.disabled = self.locked;
        }
        for (id, _) in YOUTH_LISTS {
            if let Some(list) = control(dialog, id).and_then(|c| view.list_mut(c)) {
                list.by_column = true;
            }
        }
        fill_youths(cx, view);
        let tables = |t| matches!(t, Target::Option { control: c, .. } if dialog.controls[c].id == Some(TABLES));
        mark_inert(view, dialog, |t| !tables(t));
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Row { control: c, .. } if id(c) == Some(FOCUS) => {
                let chosen = view.combo(c).and_then(|combo| combo.selected);
                if let (Some(row), Some(m)) = (chosen, own_manager_mut(cx)) {
                    m.put(FOCUS_AT, row as u8);
                }
                Outcome::Stay
            }
            Target::Cell { control: c, row, column } => {
                let Some(&(_, group)) = YOUTH_LISTS.iter().find(|(l, _)| id(c) == Some(*l)) else {
                    return Outcome::Stay;
                };
                // The disabled lists take no click (`FUN_004d7bfa` step 6).
                if self.locked {
                    if let Some(list) = view.list_mut(c) {
                        list.selected = None;
                    }
                    return Outcome::Stay;
                }
                let Some(player) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied())
                else {
                    return Outcome::Stay;
                };
                let player = player as u16;
                if column == SUPPORT_COLUMN {
                    let club = own_team(cx).map(youth_club);
                    if let (Some(club), Some(records)) = (club, cx.state.records.as_mut()) {
                        youth::support(records, club, player, group);
                    }
                    fill_youths(cx, view);
                    return Outcome::Stay;
                }
                self.way_up(cx, player)
            }
            _ => Outcome::Stay,
        }
    }

    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        let Some((player, ask)) = self.asked.take() else { return Outcome::Stay };
        if reply != Reply::Ok {
            return Outcome::Stay;
        }
        match ask {
            Ask::PreContract(salary) => {
                if let Some(records) = cx.state.records.as_mut() {
                    youth::pre_contract(records, player, salary);
                }
            }
            Ask::Trial => move_up(cx, player, youth::Trial::Youth),
        }
        fill_youths(cx, view);
        Outcome::Stay
    }
}

impl Youths {
    /// A youth's way up (`FUN_004d86a3`): a pre-contract, under 17 nothing
    /// more, then a trial — asked on the week's last day, else offered in
    /// 11. The alerts keep the row selected.
    fn way_up(&mut self, cx: &Cx, player: u16) -> Outcome {
        let notice = |args| Outcome::Popup(Request::with(NOTICE, args));
        let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
        else {
            return Outcome::Stay;
        };
        let (Some(r), Some((p, ..))) =
            (records.player(player), a3_core::load::any_player(records, &cx.game.data, player))
        else {
            return Outcome::Stay;
        };
        if r.salary() == 0 {
            let Some(salary) = pre_contract(cx, player) else { return Outcome::Stay };
            self.asked = Some((player, Ask::PreContract(salary)));
            return Outcome::Popup(Request::with(PRE_CONTRACT, Args::Record(player)));
        }
        if p.age(season.date()) < PRO_AGE {
            return notice(Args::Text(TOO_YOUNG));
        }
        let squad = own_club(cx).map_or(0, |c| c.players.len());
        if squad >= SQUAD_MOST {
            return notice(Args::Text(SQUAD_FULL));
        }
        if youth::youth_talk_booked(records, player) {
            return notice(Args::Text(TALK_BOOKED));
        }
        self.asked = Some((player, Ask::Trial));
        if super::hub::last_day(cx) {
            Outcome::Popup(Request::with(QUESTION, Args::TextFor(TRIAL_QUESTION, player)))
        } else {
            Outcome::Popup(Request::with(TRIAL, Args::Trial { player, youth: true }))
        }
    }
}

/// The club whose youth the screen shows: the professionals' for an
/// amateur department.
fn youth_club(team: &Team) -> (u8, u16) {
    let [professionals, _] = team.linked();
    let number = u16::try_from(professionals).unwrap_or(u16::from(team.number()));
    (team.country(), number)
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

/// The manager's club in the season.
fn own_club<'a>(cx: &'a Cx) -> Option<&'a a3_core::season::ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

/// A youth or a pool amateur into the squad on trial.
fn move_up(cx: &mut Cx, player: u16, trial: youth::Trial) {
    let Some(club) = own_team(cx).map(|t| (t.country(), u16::from(t.number()))) else { return };
    let data = &cx.game.data;
    let state = &mut *cx.state;
    let (Some(records), Some(season)) = (state.records.as_mut(), state.season.as_mut()) else {
        return;
    };
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    if let Some(club_state) = season.clubs.get_mut(&key) {
        youth::move_up(records, data, club_state, (key, club), player, trial);
    }
}

/// The rows (`FUN_004d849b`, `FUN_004d8074`): the youth team's players in
/// its order, each to the list of his age group; every list at its top,
/// nothing selected.
fn fill_youths(cx: &Cx, view: &mut View) {
    let (game, dialog) = (cx.game, cx.dialog);
    let (Some(records), Some(team), Some(season)) =
        (cx.state.records.as_ref(), own_team(cx), cx.state.season.as_ref())
    else {
        return;
    };
    let (country, number) = youth_club(team);
    let (ids, _) = youth::youths(records, country, number);
    let mark = |on: bool| if on { MARKED } else { " " }.to_owned();
    for (list_id, group) in YOUTH_LISTS {
        let Some(c) = control(dialog, list_id) else { continue };
        let slots = crate::controls::list::icon_slots(&dialog.controls[c]);
        let (mut rows, mut icons, mut keys) = (Vec::new(), Vec::new(), Vec::new());
        for &id in ids.iter().filter(|&&id| youth::class(records, id) == group) {
            let Some(r) = records.player(id) else { continue };
            let Some((player, _, status)) = a3_core::load::any_player(records, &game.data, id)
            else {
                continue;
            };
            let talent = TALENTS + u16::try_from(r.talent()).unwrap_or(0);
            let salary = i64::from(r.salary()) * 1000;
            rows.push(
                [
                    super::duties::short_name(cx, &player, id),
                    player.age(season.date()).to_string(),
                    super::hub::abbreviation(game, player.positions[0]),
                    r.strength().to_string(),
                    game.text(talent).to_owned(),
                    nation(cx, &status),
                    youth::goals(r).to_string(),
                    mark(r.shown() & youth::SUPPORTED != 0),
                    mark(r.strength() >= NATIONAL_STRENGTH[group]),
                    if salary == 0 {
                        game.text(NO_PRE_CONTRACT).to_owned()
                    } else {
                        super::playerinfo::money(game, salary)
                    },
                ]
                .join("\t"),
            );
            let mut row: Vec<(f32, u16)> = slots
                .iter()
                .take(ABILITY_SLOTS)
                .copied()
                .zip(game.rules.pitch.ability_icons(&player))
                .collect();
            if status.injured > 0
                && let Some(&x) = slots.get(PLASTER_SLOT)
            {
                row.push((x, PLASTER));
            }
            icons.push(row);
            keys.push(usize::from(id));
        }
        if let Some(list) = view.list_mut(c) {
            list.inks = vec![Some(BLACK); rows.len()];
            list.rows = rows;
            list.row_icons = icons;
            list.keys = keys;
            list.scroll = 0;
            list.selected = None;
        }
    }
}

fn nation(cx: &Cx, status: &a3_core::season::Status) -> String {
    let codes = &cx.game.world.nation_codes;
    codes.get(usize::from(status.nations[0])).cloned().unwrap_or_default()
}

/// The amateurs' list (dialog 427): the pool's amateurs, or the amateur
/// team's squad, or — for an amateur department — the professionals';
/// a click offers a pool amateur a trial, or marks a player to move up
/// (or be asked for) at the week's start.
#[derive(Default)]
pub struct Amateurs {
    mode: Mode,
    ids: Vec<u16>,
    marks: [u16; MARKS],
    /// Fit players not marked, as the fill counts them in the amateur
    /// team's mode.
    fit: usize,
    asked: Option<u16>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    /// The pool of a club without an amateur team.
    #[default]
    Pool,
    /// The club's own amateur team.
    Team,
    /// The club is an amateur department: its professionals.
    Department,
}

const AMATEUR_LIST: u16 = 5261;
const MARKS: usize = 4;
/// The extra data's marks and the partner club's squad (`FUN_0069a446`,
/// `FUN_006400cb`).
const MARKS_AT: usize = 0x2838;
const PARTNERS_AT: usize = 0x2840;
const PARTNERS_MOST: usize = 35;
const DEPARTMENT_TITLE: u16 = 4283;
const MOVING_UP: u16 = 4153;
const ASKED_FOR: u16 = 4154;
/// An amateur's status, when it is known: (R+0x271 >> 6) = 1, its level
/// in bits 4–5.
const STATUS_TEXTS: [u16; 3] = [2465, 2466, 2467];
const KNOWN_STATUS: u8 = 1;
/// The amateur team's limits: four moved up a season, 16 players and 14
/// fit ones left; no third keeper (`CAmateure` 0x4410fe).
const MOVED_UP_AT: usize = 0x8ec;
const MOVED_UP_MOST: u8 = 4;
const FOUR_MOVED_UP: u16 = 4198;
const LEAST_PLAYERS: usize = 16;
const TOO_FEW_PLAYERS: u16 = 4199;
const LEAST_FIT: usize = 14;
const TOO_FEW_FIT: u16 = 4200;
const ENOUGH_KEEPERS: usize = 2;
const KEEPERS_ENOUGH: u16 = 5118;
const LONG_INJURY: i32 = 7;

impl Screen for Amateurs {
    fn ids(&self) -> &[u16] {
        &[AMATEUR_PLAYERS]
    }

    /// `FUN_0044085b`: the mode, the ids, the marks, the caption.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        let Some(team) = own_team(cx) else { return };
        let [professionals, amateurs] = team.linked();
        self.mode = if professionals >= 0 {
            Mode::Department
        } else if amateurs >= 0 {
            Mode::Team
        } else {
            Mode::Pool
        };
        if let Some(extra) = own_extra(cx) {
            self.marks = extra.get::<[i16; MARKS]>(MARKS_AT).map(|m| m as u16);
        }
        self.load_ids(cx);
        let title = if self.mode == Mode::Department {
            game.text(DEPARTMENT_TITLE).to_owned()
        } else {
            dialog.title.clone()
        };
        set(view, dialog, CAPTION_ID, title);
        self.fill(cx, view);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            // OK writes the marks (`0x441083`).
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                let marks = self.marks.map(|m| m as i16);
                if let Some(extra) = own_extra_mut(cx) {
                    extra.put(MARKS_AT, marks);
                }
                Outcome::Close(Reply::Ok)
            }
            Target::Row { control: c, row } if dialog.controls[c].id == Some(AMATEUR_LIST) => {
                let Some(player) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied())
                else {
                    return Outcome::Stay;
                };
                self.click(cx, view, player as u16)
            }
            _ => Outcome::Stay,
        }
    }

    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if let Some(player) = self.asked.take()
            && reply == Reply::Ok
        {
            move_up(cx, player, youth::Trial::Amateur);
            self.load_ids(cx);
            self.fill(cx, view);
        }
        Outcome::Stay
    }
}

impl Amateurs {
    /// The rows' players (`FUN_004409c2`): the partner club's squad as the
    /// extra data keeps it, or the pool's amateurs.
    fn load_ids(&mut self, cx: &Cx) {
        self.ids = match self.mode {
            Mode::Pool => {
                let Some(team) = own_team(cx) else { return };
                let Some(records) = cx.state.records.as_ref() else { return };
                let [_, amateurs] = youth::pool(records, team.country(), team.number().into());
                amateurs
            }
            Mode::Team | Mode::Department => own_extra(cx).map_or_else(Vec::new, |extra| {
                let count = usize::try_from(extra.get::<i16>(PARTNERS_AT)).unwrap_or(0);
                (0..count.min(PARTNERS_MOST))
                    .map(|k| extra.get::<u16>(PARTNERS_AT + 2 + 2 * k))
                    .collect()
            }),
        };
    }

    /// The rows (`FUN_00440b13`), unsorted; a mark or a known amateur
    /// status in the last column ("Verletzt" is set and dropped at once).
    fn fill(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
        else {
            return;
        };
        let Some(c) = control(dialog, AMATEUR_LIST) else { return };
        let slots = crate::controls::list::icon_slots(&dialog.controls[c]);
        self.fit = 0;
        let (mut rows, mut icons, mut keys) = (Vec::new(), Vec::new(), Vec::new());
        for &id in &self.ids {
            let Some(r) = records.player(id) else { continue };
            let Some((player, _, status)) = a3_core::load::any_player(records, &game.data, id)
            else {
                continue;
            };
            let marked = self.marks.contains(&id);
            let mut state = String::new();
            match self.mode {
                Mode::Team if marked => state = game.text(MOVING_UP).to_owned(),
                Mode::Team if status.injured == 0 => self.fit += 1,
                Mode::Department if marked => state = game.text(ASKED_FOR).to_owned(),
                _ => {}
            }
            let picks = r.picks();
            if picks >> 6 == KNOWN_STATUS {
                let level = usize::from((picks >> 4) & 3);
                state = game.text(*STATUS_TEXTS.get(level).unwrap_or(&STATUS_TEXTS[0])).to_owned();
            }
            let goals = if self.mode == Mode::Pool { youth::goals(r) } else { r.goals()[0] };
            let strength =
                if matches!(picks >> 6, 1 | 3) { "?".to_owned() } else { r.strength().to_string() };
            rows.push(
                [
                    records.known_as(id).unwrap_or_default(),
                    player.age(season.date()).to_string(),
                    super::hub::abbreviation(game, player.positions[0]),
                    String::new(),
                    nation(cx, &status),
                    goals.to_string(),
                    strength,
                    state,
                ]
                .join("\t"),
            );
            icons
                .push(slots.iter().copied().zip(game.rules.pitch.ability_icons(&player)).collect());
            keys.push(usize::from(id));
        }
        if let Some(list) = view.list_mut(c) {
            list.inks = vec![Some(BLACK); rows.len()];
            list.rows = rows;
            list.row_icons = icons;
            list.keys = keys;
            list.scroll = 0;
            list.selected = None;
        }
    }

    /// A click (`0x4410fe`): a marked player unmarked; a pool amateur
    /// offered a trial; an amateur team's player marked to move up past
    /// its checks; a professional marked to be asked for. A fifth mark,
    /// and the alerts, leave the row selected.
    fn click(&mut self, cx: &Cx, view: &mut View, player: u16) -> Outcome {
        let notice = |text| Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
        if let Some(i) = self.marks.iter().position(|&m| m == player) {
            self.marks.copy_within(i + 1.., i);
            self.marks[MARKS - 1] = 0;
            self.fill(cx, view);
            return Outcome::Stay;
        }
        let free = self.marks.iter().position(|&m| m == 0).unwrap_or(MARKS);
        let squad = own_club(cx);
        match self.mode {
            Mode::Pool => {
                if squad.is_some_and(|c| c.players.len() >= SQUAD_MOST) {
                    return notice(SQUAD_FULL);
                }
                self.asked = Some(player);
                return Outcome::Popup(Request::with(TRIAL, Args::Trial { player, youth: false }));
            }
            Mode::Team => {
                if own_team(cx).is_some_and(|t| t.get::<u8>(MOVED_UP_AT) >= MOVED_UP_MOST) {
                    return notice(FOUR_MOVED_UP);
                }
                if self.ids.len().saturating_sub(free) <= LEAST_PLAYERS {
                    return notice(TOO_FEW_PLAYERS);
                }
                let records = cx.state.records.as_ref();
                let status = records
                    .and_then(|r| a3_core::load::any_player(r, &cx.game.data, player))
                    .map(|(p, _, s)| (p, s));
                let Some((p, status)) = status else { return Outcome::Stay };
                if self.fit <= LEAST_FIT && status.injured == 0 {
                    return notice(TOO_FEW_FIT);
                }
                let keepers = squad.map_or(0, |c| {
                    c.players
                        .iter()
                        .zip(&c.status)
                        .filter(|(q, s)| {
                            q.positions[0] == a3_core::world::GOALKEEPER
                                && s.suspended == 0
                                && s.injured < LONG_INJURY
                        })
                        .count()
                });
                if p.positions[0] == a3_core::world::GOALKEEPER && keepers >= ENOUGH_KEEPERS {
                    return notice(KEEPERS_ENOUGH);
                }
            }
            Mode::Department => {}
        }
        if free < MARKS {
            self.marks[free] = player;
            self.fill(cx, view);
        }
        Outcome::Stay
    }
}

/// The manager's club's extra data, and to change.
fn own_extra<'a>(cx: &'a Cx) -> Option<&'a a3_core::records::Extra> {
    let slot = usize::from(own_team(cx)?.human()).checked_sub(1)?;
    cx.state.records.as_ref()?.extras.get(slot)
}

fn own_extra_mut<'a>(cx: &'a mut Cx) -> Option<&'a mut a3_core::records::Extra> {
    let slot = usize::from(own_team(cx)?.human()).checked_sub(1)?;
    cx.state.records.as_mut()?.extras.get_mut(slot)
}

/// What to do with a youth or an amateur (dialog 11): four weeks with the
/// professionals, a professional contract (needs the week planner's
/// booking, not built), nothing.
#[derive(Default)]
pub struct TrialChoice {
    player: u16,
    youth: bool,
}

const TRIAL_TEXT: u16 = 1252;
const TRIAL_CHOICES: u16 = 1254;
const TRY: usize = 0;
const CONTRACT: usize = 1;
const NOTHING: usize = 2;
const YOUTH_TITLE: u16 = 4289;
const INJURED_TOO_LONG: u16 = 2920;
const ON_TRIAL: u16 = 2461;
const TRIAL_MASK: u8 = 0x0f;
/// The amateurs with a known status a squad may hold (dialog 10, not
/// built).
const AMATEURS_MOST: usize = 4;

impl Screen for TrialChoice {
    fn ids(&self) -> &[u16] {
        &[TRIAL]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        if let Args::Trial { player, youth } = args {
            (self.player, self.youth) = (player, youth);
        }
        let title =
            if self.youth { game.text(YOUTH_TITLE).to_owned() } else { dialog.title.clone() };
        set(view, dialog, CAPTION_ID, title);
        let name = cx.state.records.as_ref().and_then(|r| r.known_as(self.player));
        let text = control(dialog, TRIAL_TEXT).map(|c| dialog.controls[c].text.clone());
        set(
            view,
            dialog,
            TRIAL_TEXT,
            text.unwrap_or_default().replacen("%s", &name.unwrap_or_default(), 1),
        );
        if let Some(c) = control(dialog, TRIAL_CHOICES) {
            view.set_chosen(c, crate::controls::options::NONE);
        }
        super::market::question_ball(view, dialog);
        let contract = |t| matches!(t, Target::Option { control: c, item: CONTRACT } if dialog.controls[c].id == Some(TRIAL_CHOICES));
        mark_inert(view, dialog, |t| !contract(t));
    }

    /// `CAmateurspielerProbetraining`: the trial's checks, then a shirt
    /// number for him; the alerts leave the dialog open.
    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let Target::Option { control: c, item } = target else { return Outcome::Stay };
        if cx.dialog.controls[c].id != Some(TRIAL_CHOICES) {
            return Outcome::Stay;
        }
        match item {
            TRY => self.try_out(cx),
            NOTHING => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }
}

impl TrialChoice {
    fn try_out(&self, cx: &mut Cx) -> Outcome {
        let notice = |text| Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
        let Some(records) = cx.state.records.as_ref() else { return Outcome::Stay };
        let Some((_, _, status)) = a3_core::load::any_player(records, &cx.game.data, self.player)
        else {
            return Outcome::Stay;
        };
        if status.injured >= LONG_INJURY {
            return notice(INJURED_TOO_LONG);
        }
        if records.player(self.player).is_some_and(|r| r.picks() & TRIAL_MASK != 0) {
            return notice(ON_TRIAL);
        }
        let squad: Vec<u16> =
            own_club(cx).map_or_else(Vec::new, |c| c.status.iter().map(|s| s.id).collect());
        let known = squad
            .iter()
            .filter_map(|&id| records.player(id))
            .filter(|r| r.picks() >> 6 == KNOWN_STATUS)
            .count();
        if !self.youth && known >= AMATEURS_MOST {
            info!("Amateurspielergrenze erreicht (10): not built yet");
            return Outcome::Stay;
        }
        if let Some(records) = cx.state.records.as_mut() {
            youth::give_shirt(records, &squad, self.player);
        }
        Outcome::Close(Reply::Ok)
    }
}
