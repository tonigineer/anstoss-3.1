//! The squad and the transfer market, the menu bar's eleventh symbol: a
//! menu of the squad's screens and the market's (dialog 218; SPIELERKADER.md
//! in the RE repo).

use std::collections::HashMap;

use chrono::Datelike;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::{NOTICE, QUESTION};
use a3_core::cycle::dialogs::{
    AMATEUR_PLAYERS, APPOINTMENTS, CLUB_SEARCH, CURRENT_SQUAD, DRAW_OPTION, EXTENSIONS, LOANED_OUT,
    LISTING_DEMAND, LOAN_CONDITIONS, MARKET_VALUES, NEXT_SQUAD, NO_DATE, NORACSA_REQUEST,
    NORACSA_SEARCH, RELATIONS, TRANSFER_LIST, WATCH_LIST, WORLDS_BEST, YOUTH_POACHING, YOUTH_TEAM,
};
use a3_core::records::Team;
use a3_core::season::Rules;
use a3_core::{lineup, squadlist};
use a3_files::rch::Dialog;
use tracing::info;

use super::hub::{self, SQUAD_MENU, SQUAD_SYMBOL};
use super::playerinfo::money;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::options::NONE;
use crate::pointer::{Grip, Target};
use crate::view::View;

/// The OK, whose place the bar takes; the caption and its text, not the
/// dialog file's.
const OK: u16 = 1;
const CAPTION_ID: u16 = 9935;
const CAPTION: u16 = 2520;
/// The squad's screens and the market's, buttons in two groups.
const SQUAD_SCREENS: u16 = 5042;
const MARKET_SCREENS: u16 = 6371;
/// The squad screens' buttons: the current squad, next season's, the
/// players out on loan, the market values.
const CURRENT_SQUAD_ITEM: usize = 0;
const NEXT_SQUAD_ITEM: usize = 1;
const YOUTH_ITEM: usize = 2;
const EXTENSIONS_ITEM: usize = 4;
const LOANED_OUT_ITEM: usize = 5;
const MARKET_VALUES_ITEM: usize = 6;
const SQUAD_ITEMS: [(usize, u16); 7] = [
    (CURRENT_SQUAD_ITEM, CURRENT_SQUAD),
    (NEXT_SQUAD_ITEM, NEXT_SQUAD),
    (YOUTH_ITEM, YOUTH_TEAM),
    (AMATEURS, AMATEUR_PLAYERS),
    (EXTENSIONS_ITEM, EXTENSIONS),
    (LOANED_OUT_ITEM, LOANED_OUT),
    (MARKET_VALUES_ITEM, MARKET_VALUES),
];
/// The market screens' buttons: the transfer list, the week's
/// appointments, the watch list, the youth to poach, the world's best.
const TRANSFER_LIST_ITEM: usize = 0;
const MARKET_ITEMS: [(usize, u16); 8] = [
    (TRANSFER_LIST_ITEM, TRANSFER_LIST),
    (1, APPOINTMENTS),
    (2, WATCH_LIST),
    (POACHING, YOUTH_POACHING),
    (4, CLUB_SEARCH),
    (5, WORLDS_BEST),
    (NORACSA[0], NORACSA_SEARCH),
    (NORACSA[1], NORACSA_REQUEST),
];
/// The amateurs' button, which a club's amateur department sees as its
/// professionals' (text 4628); the market's youth poaching, which it may
/// not (alert 4698).
const AMATEURS: usize = 3;
const PROFESSIONALS: u16 = 4628;
const POACHING: usize = 3;
const NO_POACHING: u16 = 4698;
/// NORACSA's two searches, which need the computer department (alert
/// 4443).
const NORACSA: [usize; 2] = [6, 7];
const NO_COMPUTERS: u16 = 4443;
/// The computer department's level, in the team record (the club
/// object's +0x5e2).
const COMPUTERS: usize = 0x85a;

/// The bar takes the OK's place; the caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&SQUAD_MENU) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    hub::add_bar(dialog, rules);
}

/// The menu.
#[derive(Default)]
pub struct SquadMenu {
    /// Whether the whistle asks.
    whistle: bool,
}

impl Screen for SquadMenu {
    fn ids(&self) -> &[u16] {
        &[SQUAD_MENU]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        hub::open_bar(cx, view, SQUAD_SYMBOL);
        if let Some(season) = cx.state.season.as_ref() {
            set(view, dialog, CAPTION_ID, hub::manager_title(game, season, CAPTION));
        }
        for id in [SQUAD_SCREENS, MARKET_SCREENS] {
            if let Some(options) = control(dialog, id).and_then(|c| view.options_mut(c)) {
                options.actions = true;
            }
        }
        if amateurs(cx)
            && let Some(label) = control(dialog, SQUAD_SCREENS)
                .and_then(|c| view.options_mut(c))
                .and_then(|o| o.labels.get_mut(AMATEURS))
        {
            *label = game.text(PROFESSIONALS).to_owned();
        }
        let works = |target: Target| match target {
            Target::Option { control: c, item }
                if dialog.controls[c].id == Some(MARKET_SCREENS) =>
            {
                MARKET_ITEMS.iter().any(|&(i, _)| i == item) || refusal(cx, item).is_some()
            }
            Target::Option { control: c, item } if dialog.controls[c].id == Some(SQUAD_SCREENS) => {
                SQUAD_ITEMS.iter().any(|&(i, _)| i == item)
            }
            _ => hub::on_bar(dialog, target).is_some_and(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Some(item) = hub::on_bar(cx.dialog, target) {
            let outcome = hub::bar_click(cx, view, item, SQUAD_SYMBOL);
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        self.whistle = false;
        match target {
            Target::Option { control: c, item }
                if cx.dialog.controls[c].id == Some(SQUAD_SCREENS) =>
            {
                match SQUAD_ITEMS.iter().find(|&&(i, _)| i == item) {
                    Some(&(_, screen)) => Outcome::Popup(Request::new(screen)),
                    None => Outcome::Stay,
                }
            }
            Target::Option { control: c, item }
                if cx.dialog.controls[c].id == Some(MARKET_SCREENS) =>
            {
                if let Some(text) = refusal(cx, item) {
                    return Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
                }
                match MARKET_ITEMS.iter().find(|&&(i, _)| i == item) {
                    Some(&(_, screen)) => Outcome::Popup(Request::new(screen)),
                    None => Outcome::Stay,
                }
            }
            _ => Outcome::Stay,
        }
    }

    /// The whistle's question: a yes ends the turn.
    fn answer(&mut self, _: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}

/// The club's option drawn on him from the current squad (305's OK,
/// `FUN_0056397e` step 4): terms cheap for him hurt him
/// (`FUN_00709c74`, `FUN_00770800(L, 0x20)`), the option gone, a year
/// more, and no longer retiring.
fn draw_club_option(cx: &mut Cx, id: u16) {
    let cheap = with_reckoning(cx, |club| {
        signee(cx, id).map(|p| cx.game.rules.contract.index(&p, club) < CHEAP)
    });
    if let Some(r) = cx.state.records.as_mut().and_then(|r| r.players.get_mut(usize::from(id))) {
        if cheap.flatten() == Some(true) {
            cx.game.rules.discipline.mood.set_reasons(r, CHEAP_OPTION);
        }
        r.set_terms(r.terms() & !CLUB_OPTION);
        r.set_retiring(r.retiring() & !RETIRING);
    }
    extend(cx, id);
}

/// Whether the club is another club's amateur department
/// (`FUN_0063e793`).
fn amateurs(cx: &Cx) -> bool {
    own_team(cx).is_some_and(|team| team.linked()[0] >= 0)
}

/// What the market's button says instead of opening its screen, if the
/// club may not use it (`FUN_00567248`, `FUN_005672c9`, `FUN_0056734e`).
fn refusal(cx: &Cx, item: usize) -> Option<u16> {
    let team = own_team(cx)?;
    if item == POACHING && team.linked()[0] >= 0 {
        return Some(NO_POACHING);
    }
    (NORACSA.contains(&item) && team.get::<i8>(COMPUTERS) != 1).then_some(NO_COMPUTERS)
}

/// The manager's club's team record.
pub(crate) fn own_team<'a>(cx: &'a Cx) -> Option<&'a Team> {
    let (slot, number) = cx.state.season.as_ref()?.manager()?.club?;
    let nation = cx.game.world.country(slot)?.nation;
    cx.state.records.as_ref()?.team(nation, u16::try_from(number).ok()?)
}

/// The manager's club's team record, to change.
pub(crate) fn own_team_mut<'a>(cx: &'a mut Cx) -> Option<&'a mut a3_core::records::Team> {
    let (country, number) = own_team(cx).map(|t| (t.country(), u16::from(t.number())))?;
    cx.state.records.as_mut()?.team_mut(country.into(), number)
}

/// The current squad (dialog 216, `CSpielerkaderAktuell`; KADER-AKTUELL.md):
/// every player in a row, by main position, the squad's count, strength
/// and wages below; a mode chosen, a click on a row acts on the player.
#[derive(Default)]
pub struct CurrentSquad {
    /// The action a row's click runs, if one is chosen.
    mode: Option<Mode>,
    /// The player whose menu is open.
    menu_player: Option<usize>,
    /// What the popup opened by a row's click leads to.
    pending: Pending,
}

/// The current squad's popups that do something when answered.
#[derive(Default, Clone, Copy)]
enum Pending {
    #[default]
    Nothing,
    /// The club's option on him (305): drawn, or the talk goes on.
    Option(u16),
    /// The week planner booking his extension talk.
    Booking,
    /// Whether to take him off the transfer list (384).
    Unlist(u16),
    /// His demand to go onto the list while under contract (205).
    Demand(u16, i32),
    /// An alert after which the squad is shown anew.
    Refill,
}

/// What a click on a row does, by the mode buttons' items.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Contract,
    TransferList,
    Loan,
    Bonus,
    Fine,
}

const MODES: [Mode; 5] = [Mode::Contract, Mode::TransferList, Mode::Loan, Mode::Bonus, Mode::Fine];
/// The list, the lines below it, the modes, the critical relations, the OK.
const SQUAD_LIST: u16 = 3298;
const SQUAD_LINE: u16 = 3299;
const WAGES_LINE: u16 = 6046;
const MODE_BUTTONS: u16 = 5657;
const CRITICAL: u16 = 6280;
/// "Derzeit verfügen Sie über %d Spieler …", and over none.
const SQUAD_TEXT: u16 = 1677;
const NO_SQUAD_TEXT: u16 = 1678;
/// "K.-Ende": he ends his career with the season.
const CAREER_END: u16 = 4951;
/// The rows' inks: on the transfer list, on loan with us, the others.
const LISTED_INK: [u8; 3] = [150, 0, 0];
const LOANED_INK: [u8; 3] = [0, 175, 0];
const SQUAD_INK: [u8; 3] = [7, 12, 121];
/// A record's bits: on the transfer list, retiring (+0x26f); on loan
/// with us (+0x15e); his strength unknown, bits 6–7 of +0x271 at 1 or 3.
const LISTED: u8 = 0x01;
const RETIRING: u8 = 0x80;
const ON_LOAN: u16 = 0x100;
const UNKNOWN_STRENGTH: u8 = 0xc0;
/// Salaries are kept in 1000s, release fees in 10.000s; the contract's
/// year in the low twelve bits.
const SALARY_UNIT: i64 = 1000;
const FEE_UNIT: i64 = 10_000;
const CONTRACT_YEAR: u16 = 0xfff;
/// The manager's own record, as a player.
const LAST_REAL_ID: u16 = 17_000;
/// The refusals the contract's mode tells: the manager himself, a player
/// on loan with us, one leaving at the season's end, one retiring.
const OWN_CONTRACT: u16 = 4444;
const BORROWED: u16 = 2362;
const LEAVING: u16 = 2381;
const RETIRES: u16 = 3135;
/// The contract's further refusals (`FUN_0056397e` steps 6, 7, 9).
const NO_MORE_EXTENSION: u16 = 4803;
const CONTENT_WITH_CONTRACT: u16 = 3189;
const NO_MOOD: u16 = 2694;
/// R+0x27f bit 6: no further extension (`FUN_00707834`); R+0x1b0 bit 5
/// and R+0x1e2 bit 9 as the talk's checks read them.
const NO_EXTENSION: u8 = 0x40;
const WANTS_TALK: u32 = 0x20;
const NO_MOOD_FLAG: u16 = 0x200;
const MOOD_FLAGS: usize = 0x1e2;
/// He is content when his terms please him above this (`FUN_00767cb9`).
const CONTENT_ABOVE: i32 = 149;
/// The season's day after which a contract's last year counts as run
/// out (`FUN_0075827a`).
const LATE_DAY: i32 = 216;
const OLD: u32 = 30;
/// The transfer list's further alerts (`FUN_00563f7b` steps 5–8).
const TALK_THIS_WEEK: u16 = 2512;
const WANTS_TO_STAY_LISTED: u16 = 3190;
const NO_VALID_CONTRACT: u16 = 2511;
const TAKE_OFF_LIST: u16 = 4072;
const NO_MORE_MOVES: u16 = 3191;
const FED_UP: u16 = 3846;
const NOW_LISTED: u16 = 2513;
/// R+0x161: how often he may still move this season, as the list reads
/// it (L+0x98).
const MOVES: usize = 0x161;
const REGULAR_PLACE: u16 = a3_core::contract::REGULAR_PLACE;
/// The loan's alerts (`FUN_00564703` steps 5–9) and its limits.
const LIST_CLOSED: u16 = 3192;
const CONTRACT_ENDING: u16 = 4179;
const ALREADY_LISTED: u16 = 2216;
const WONT_GO_ON_LOAN: u16 = 4180;
const TOO_MANY_LOANS: u16 = 2358;
const LOANS_MOST: usize = 5;
const LUCKY: i16 = 6;
/// R+0x255: the league matches he missed, as the loan reads them (L+0x56).
const LEAGUE_LESS: usize = 0x255;
/// 205's choices; R+0x1f0, the demand kept; his answers to being benched
/// and what in his character (traits[0], traits[1]) leads to which.
const AGREE: usize = 0;
const BENCH_HIM: usize = 1;
const DEMAND_KEPT: usize = 0x1f0;
const DONT_CARE: u16 = 2179;
const GOLF: u16 = 2180;
const FIGHTS: u16 = 2181;
const CAREER: u16 = 2182;
const GOES_FREELY: u16 = 2183;
const WAIT_AND_SEE: u16 = 2184;
const CARELESS: u16 = 0x260;
const FIGHTER: u16 = 0x1;
const AMBITIOUS: u16 = 0x10;
const GOES: u16 = 0x100;
const YOUNG: u32 = 22;
const SULK_MORE: i32 = -5;
const SULK_LESS: i32 = -2;
/// The transfer list's: the manager himself; the loan's: the manager, a
/// borrowed player.
const OWN_LISTING: u16 = 3315;
const OWN_LOAN: u16 = 4445;
const BORROWED_LOAN: u16 = 2215;
/// The menu of a row: info, a blank, bonus, fine (`CListBoxSpielerInfo`).
const ROW_MENU: [hub::Command; 4] =
    [hub::Command::Info, hub::Command::Blank, hub::Command::Bonus, hub::Command::Fine];

/// The caption; the menu a right click on a row opens.
pub(crate) fn compose_squad(dialogs: &mut HashMap<u16, Dialog>) {
    if let Some(dialog) = dialogs.get_mut(&CURRENT_SQUAD) {
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
        dialog.controls.push(hub::menu_control());
    }
}

impl Screen for CurrentSquad {
    fn ids(&self) -> &[u16] {
        &[CURRENT_SQUAD]
    }

    /// The rows, no mode (`FUN_00562f14`).
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.mode = None;
        self.menu_player = None;
        if let Some(season) = cx.state.season.as_ref() {
            set(view, dialog, CAPTION_ID, hub::manager_title_of(game, season, &dialog.title));
        }
        fill_squad(cx, view);
        if let Some(c) = control(dialog, MODE_BUTTONS) {
            view.set_chosen(c, NONE);
        }
        let works = |target: Target| match target {
            Target::Option { control: c, item } if dialog.controls[c].id == Some(MODE_BUTTONS) => {
                matches!(
                    MODES.get(item),
                    Some(
                        Mode::Contract
                            | Mode::TransferList
                            | Mode::Loan
                            | Mode::Bonus
                            | Mode::Fine
                    )
                )
            }
            _ => true,
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        // An open menu takes the next click.
        if let Some(player) = self.menu_player.take() {
            return match hub::close_menu(cx, view, target).and_then(|i| ROW_MENU.get(i)) {
                Some(hub::Command::Info) => {
                    // The info pages through the rows backwards (`AddHead`).
                    let rows = control(dialog, SQUAD_LIST).and_then(|l| view.list(l));
                    cx.state.paging = rows.map(|l| l.keys.iter().rev().copied().collect());
                    let info = super::playerinfo::CURRENT;
                    Outcome::Popup(Request::with(info, Args::Player(player)))
                }
                Some(hub::Command::Bonus) => {
                    Outcome::Popup(Request::with(super::discipline::BONUS, Args::Player(player)))
                }
                Some(hub::Command::Fine) => {
                    Outcome::Popup(Request::with(super::discipline::FINE, Args::Player(player)))
                }
                _ => Outcome::Stay,
            };
        }
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            // `0x564e6f`: no refill after.
            Target::Button(c) if id(c) == Some(CRITICAL) => Outcome::Popup(Request::new(RELATIONS)),
            Target::Option { control: c, item } if id(c) == Some(MODE_BUTTONS) => {
                self.mode = MODES.get(item).copied();
                Outcome::Stay
            }
            Target::Menu(grip @ Grip::Row { control: c, row }) if id(c) == Some(SQUAD_LIST) => {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                if let Some(player) = player
                    && hub::show_menu(cx, view, grip, &ROW_MENU).is_some()
                {
                    self.menu_player = Some(player);
                }
                Outcome::Stay
            }
            Target::Row { control: c, row } if id(c) == Some(SQUAD_LIST) => {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                match (self.mode, player) {
                    (Some(mode), Some(player)) => act(cx, mode, player, &mut self.pending),
                    _ => Outcome::Stay,
                }
            }
            _ => Outcome::Stay,
        }
    }

    /// The club's option drawn, refilled; declined, the talk goes on. The
    /// planner's booking refills nothing.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        match std::mem::take(&mut self.pending) {
            Pending::Option(id) if reply == Reply::Ok => {
                draw_club_option(cx, id);
                fill_squad(cx, view);
                Outcome::Stay
            }
            Pending::Option(id) => contract_talk(cx, id, true, &mut self.pending),
            Pending::Unlist(id) if reply == Reply::Ok => {
                if let Some(records) = cx.state.records.as_mut() {
                    a3_core::transfers::take_off(records, id);
                }
                fill_squad(cx, view);
                Outcome::Stay
            }
            Pending::Demand(id, demand) => match reply {
                Reply::Choice(choice) => {
                    let (item, free) = (choice / 2, choice % 2 == 1);
                    let outcome = demanded(cx, id, demand, item, free);
                    fill_squad(cx, view);
                    outcome
                }
                _ => Outcome::Stay,
            },
            Pending::Refill => {
                fill_squad(cx, view);
                Outcome::Stay
            }
            Pending::Booking | Pending::Unlist(_) | Pending::Nothing => Outcome::Stay,
        }
    }
}

/// A row's player acted on by the mode chosen (`OnSelChange` 0x564c02):
/// the bonus and the fine in their dialogs; the contract, the transfer
/// list and the loan as far as their refusals go.
fn act(cx: &mut Cx, mode: Mode, player: usize, pending: &mut Pending) -> Outcome {
    let notice = |text: u16| Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
    match mode {
        Mode::Bonus => {
            return Outcome::Popup(Request::with(super::discipline::BONUS, Args::Player(player)));
        }
        Mode::Fine => {
            return Outcome::Popup(Request::with(super::discipline::FINE, Args::Player(player)));
        }
        _ => {}
    }
    let Some((id, record)) = squad_record(cx, player) else { return Outcome::Stay };
    let manager = id > LAST_REAL_ID;
    let borrowed = record.terms() & ON_LOAN != 0;
    let leaving = record.get::<u16>(NEXT_CLUB) != 0;
    let retiring = record.retiring() & RETIRING != 0;
    let refusal = match mode {
        Mode::Contract if manager => Some(OWN_CONTRACT),
        Mode::Contract if borrowed => Some(BORROWED),
        Mode::Contract if leaving => Some(LEAVING),
        Mode::TransferList if manager => Some(OWN_LISTING),
        Mode::TransferList if retiring => Some(RETIRES),
        Mode::TransferList if borrowed => Some(BORROWED),
        Mode::TransferList if leaving => Some(LEAVING),
        Mode::Loan if manager => Some(OWN_LOAN),
        Mode::Loan if retiring => Some(RETIRES),
        Mode::Loan if borrowed => Some(BORROWED_LOAN),
        Mode::Loan if leaving => Some(LEAVING),
        _ => None,
    };
    match refusal {
        Some(text) => notice(text),
        None if mode == Mode::Contract => contract_talk(cx, id, false, pending),
        None if mode == Mode::TransferList => listing(cx, id, pending),
        None if mode == Mode::Loan => loan(cx, id, pending),
        None => {
            info!("{mode:?} ({id}): not built yet");
            Outcome::Stay
        }
    }
}

/// "Vertrag verlängern" past its first refusals (`FUN_0056397e` steps
/// 4–10): the club's option drawn or not (305), then the player's own
/// reasons, and the week planner booking the talk (kind 0x0c). With
/// `optioned` the option was offered and declined.
fn contract_talk(cx: &Cx, id: u16, optioned: bool, pending: &mut Pending) -> Outcome {
    let notice = |args| Outcome::Popup(Request::with(NOTICE, args));
    let no_date = || Outcome::Popup(Request::with(NO_DATE, Args::Record(id)));
    let Some(records) = cx.state.records.as_ref() else { return Outcome::Stay };
    let Some(r) = records.player(id) else { return Outcome::Stay };
    let free = a3_core::contract::talk_free(records, id);
    if !optioned && r.terms() & CLUB_OPTION != 0 {
        if !free {
            return no_date();
        }
        *pending = Pending::Option(id);
        return Outcome::Popup(Request::with(DRAW_OPTION, Args::Record(id)));
    }
    let today = cx.state.season.as_ref().map(|s| s.date());
    let age = shown(cx, id).zip(today).map_or(0, |(s, d)| s.player.age(d));
    if r.retiring() & RETIRING != 0 && age > OLD {
        return notice(Args::Text(RETIRES));
    }
    if r.problems() & NO_EXTENSION != 0 {
        return notice(Args::Text(NO_MORE_EXTENSION));
    }
    if r.flags() & WANTS_TALK == 0 && content(cx, id) {
        return notice(Args::Text(CONTENT_WITH_CONTRACT));
    }
    if !free {
        return no_date();
    }
    if r.get::<u16>(MOOD_FLAGS) & NO_MOOD_FLAG != 0 {
        return notice(Args::TextFor(NO_MOOD, id));
    }
    *pending = Pending::Booking;
    let args = Args::Book(a3_core::contract::EXTENSION_KIND, id);
    Outcome::Popup(Request::with(super::planner::PLANNER, args))
}

/// "Auf Transferliste setzen" past its first refusals (`FUN_00563f7b`
/// steps 5–8): off the list if he is on it and willing, onto it — at once
/// if his contract runs out or he is fed up, else for what he demands
/// (205).
fn listing(cx: &mut Cx, id: u16, pending: &mut Pending) -> Outcome {
    let notice = |text: u16| Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
    let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
    else {
        return Outcome::Stay;
    };
    let Some(r) = records.player(id) else { return Outcome::Stay };
    let years = years_left(r, i32::from(records.globals.start_year()));
    let late = i32::try_from(season.day).unwrap_or(0) > LATE_DAY;
    let running = years > 1 || years == 1 && !late;
    if let Some(terms) = a3_core::transfers::listed_terms(records, id) {
        if a3_core::transfers::pending(records, id) {
            return notice(TALK_THIS_WEEK);
        }
        if terms != a3_core::transfers::FOR_A_FEE {
            return notice(WANTS_TO_STAY_LISTED);
        }
        if !running {
            return notice(NO_VALID_CONTRACT);
        }
        *pending = Pending::Unlist(id);
        return Outcome::Popup(Request::with(QUESTION, Args::TextFor(TAKE_OFF_LIST, id)));
    }
    let moves = r.get::<u8>(MOVES);
    if (moves == 0 || moves == 1 && records.globals.seasons() > 1) && !late {
        return notice(NO_MORE_MOVES);
    }
    if running {
        let content = a3_core::mood::content_level(a3_core::mood::parts(r));
        if content > 1 {
            *pending = Pending::Refill;
            let listed = list(cx, id, false);
            return if listed { notice(FED_UP) } else { Outcome::Stay };
        }
        let month = usize::try_from(season.date().month()).unwrap_or(0);
        let regular = r.terms() & REGULAR_PLACE != 0;
        let demand =
            cx.game.rules.contract.listing_demand(id, r.salary(), years, month, regular);
        *pending = Pending::Demand(id, demand);
        return Outcome::Popup(Request::with(LISTING_DEMAND, Args::Record(id)));
    }
    *pending = Pending::Refill;
    if list(cx, id, false) { notice(NOW_LISTED) } else { Outcome::Stay }
}

/// "Verleihen" past its first refusals (`FUN_00564703` steps 5–10): the
/// list closed late in the season, a contract ending, already listed,
/// unwilling, five out on loan already; else the loan's conditions (292).
fn loan(cx: &Cx, id: u16, pending: &mut Pending) -> Outcome {
    let notice = |args| Outcome::Popup(Request::with(NOTICE, args));
    let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
    else {
        return Outcome::Stay;
    };
    let Some(r) = records.player(id) else { return Outcome::Stay };
    if i32::try_from(season.day).unwrap_or(0) > LATE_DAY {
        return notice(Args::Text(LIST_CLOSED));
    }
    if years_left(r, i32::from(records.globals.start_year())) <= 1 {
        return notice(Args::Text(CONTRACT_ENDING));
    }
    if a3_core::transfers::listed_terms(records, id).is_some() {
        return notice(Args::Text(ALREADY_LISTED));
    }
    if !lends_himself(cx, id) {
        return notice(Args::TextFor(WONT_GO_ON_LOAN, id));
    }
    let out = agreed_lists(cx)[2].len()
        + own_team(cx).map_or(0, |t| {
            a3_core::transfers::loan_listings(records, (t.country(), i16::from(t.number())))
        });
    if out >= LOANS_MOST {
        return notice(Args::TextForNumber(TOO_MANY_LOANS, LOANS_MOST as i32));
    }
    *pending = Pending::Refill;
    Outcome::Popup(Request::with(LOAN_CONDITIONS, Args::Record(id)))
}

/// Whether he lets himself be loaned out (`FUN_00564ef4`): a move left,
/// not played in most of the league's matchdays, under thirty and lucky.
fn lends_himself(cx: &Cx, id: u16) -> bool {
    let Some(records) = cx.state.records.as_ref() else { return false };
    let Some(r) = records.player(id) else { return false };
    if r.get::<u8>(MOVES) == 0 {
        return false;
    }
    let played = own_team(cx).map_or(0, |t| a3_core::league::matchdays(records, t)) as i32;
    let (matches, less) = (i32::from(r.league_matches()), i32::from(r.get::<u8>(LEAGUE_LESS)));
    if played > 0 && (played / 2 < matches || played / 3 < matches && played / 6 < less) {
        return false;
    }
    let today = cx.state.season.as_ref().map(|s| s.date());
    let age = shown(cx, id).zip(today).map_or(0, |(s, d)| s.player.age(d));
    age < 30 && records.luck.of(id) >= LUCKY
}

/// What 205's answer does (`FUN_00563f7b` step 7, [asm 0x5645de]):
/// "Einverstanden" lists him, his demand kept (R+0x1f0, in thousands); "Dann
/// machen Sie kein Spiel mehr" gets his answer by his age and character —
/// some go onto the list, some sulk; "Vergessen Sie es" nothing.
fn demanded(cx: &mut Cx, id: u16, demand: i32, item: usize, free: bool) -> Outcome {
    let notice = |text: u16| Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
    let agreed = |cx: &mut Cx| {
        if list(cx, id, free)
            && let Some(r) = player_mut(cx, id)
        {
            r.put(DEMAND_KEPT, (demand / 1000) as u16);
        }
    };
    match item {
        AGREE => {
            agreed(cx);
            Outcome::Stay
        }
        BENCH_HIM => {
            let Some(r) = cx.state.records.as_ref().and_then(|r| r.player(id)) else {
                return Outcome::Stay;
            };
            let [first, second] = r.traits();
            let today = cx.state.season.as_ref().map(|s| s.date());
            let age = shown(cx, id).zip(today).map_or(0, |(s, d)| s.player.age(d));
            if age > OLD || second & CARELESS != 0 {
                let golf = cx.state.season.as_mut().map_or(0, |s| s.random.below(2)) == 0;
                sulk(cx, id, SULK_MORE);
                notice(if golf { GOLF } else { DONT_CARE })
            } else if second & FIGHTER != 0 || first & FIGHTER != 0 {
                notice(FIGHTS)
            } else if age < YOUNG || second & AMBITIOUS != 0 {
                agreed(cx);
                notice(CAREER)
            } else if second & GOES != 0 {
                agreed(cx);
                notice(GOES_FREELY)
            } else {
                sulk(cx, id, SULK_LESS);
                notice(WAIT_AND_SEE)
            }
        }
        _ => Outcome::Stay,
    }
}

/// Less content with the trainer (`FUN_007007a1`).
fn sulk(cx: &mut Cx, id: u16, by: i32) {
    if let Some(r) = player_mut(cx, id) {
        a3_core::mood::change_trainer(r, by);
    }
}

fn player_mut<'a>(cx: &'a mut Cx, id: u16) -> Option<&'a mut a3_core::records::Player> {
    cx.state.records.as_mut()?.players.get_mut(usize::from(id))
}

/// Onto the list for sale (`FUN_0075a129(L, free, 1)`): whether it took
/// him.
fn list(cx: &mut Cx, id: u16, free: bool) -> bool {
    cx.state.records.as_mut().is_some_and(|r| a3_core::transfers::list_for_sale(r, id, free))
}

/// Whether he is happy with his contract as it runs (`FUN_00767cb9`):
/// years left — more than one late in the season — and his terms
/// pleasing him above 149.
fn content(cx: &Cx, id: u16) -> bool {
    let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
    else {
        return false;
    };
    let Some(r) = records.player(id) else { return false };
    let years = years_left(r, i32::from(records.globals.start_year()));
    let late = i32::try_from(season.day).unwrap_or(0) > LATE_DAY;
    if years <= 0 || late && years <= 1 {
        return false;
    }
    let Some(p) = signee(cx, id) else { return false };
    let rules = &cx.game.rules.contract;
    let pleased = with_reckoning(cx, |club| rules.satisfaction(&p, club, &p.terms));
    pleased.is_some_and(|v| v > CONTENT_ABOVE)
}

/// The club he signed with for next season, if any (R+0x14c).
const NEXT_CLUB: usize = 0x14c;

/// A squad place's player: his id and record.
fn squad_record<'a>(cx: &'a Cx, player: usize) -> Option<(u16, &'a a3_core::records::Player)> {
    let season = cx.state.season.as_ref()?;
    let club = season.clubs.get(&season.manager()?.club?)?;
    let id = club.status.get(player)?.id;
    Some((id, cx.state.records.as_ref()?.player(id)?))
}

/// The eleven columns every squad row shows, here and in the opponent's
/// squad (`FUN_00563147`, `FUN_005652cb`): name, position and its sides,
/// strength and form, ES, age, nation, morale, will and freshness.
pub(crate) fn squad_row(
    cx: &Cx,
    club: &a3_core::season::ClubState,
    p: usize,
    today: chrono::NaiveDate,
) -> [String; 11] {
    let (game, records) = (cx.game, cx.state.records.as_ref());
    let pitch = &game.rules.pitch;
    let player = &club.players[p];
    let status = club.status.get(p).copied().unwrap_or_default();
    let fitness = lineup::fitness_of(&club.fitness, p);
    let record = records.and_then(|r| r.player(status.id));
    let get = |f: &dyn Fn(&a3_core::records::Player) -> u16| record.map_or(0, f);
    let [main, second, third] = player.positions;
    let sides = match (second, third) {
        (0, _) => " ".to_owned(),
        (s, 0) => hub::abbreviation(game, s),
        (s, t) => format!("{}/{}", hub::abbreviation(game, s), hub::abbreviation(game, t)),
    };
    let unknown = matches!((get(&|r| r.picks().into()) as u8 & UNKNOWN_STRENGTH) >> 6, 1 | 3);
    let nation = game.world.nation_codes.get(usize::from(status.nations[0])).cloned();
    [
        super::duties::short_name(cx, player, status.id),
        hub::abbreviation(game, main),
        sides,
        if unknown { "?".to_owned() } else { player.strength.to_string() },
        lineup::form(fitness).to_string(),
        hub::tenths(pitch.strength(player, fitness, status.doping)),
        player.age(today).to_string(),
        nation.unwrap_or_default(),
        fitness.morale.to_string(),
        lineup::condition(fitness).to_string(),
        lineup::freshness(fitness).to_string(),
    ]
}

/// The rows and the lines below them (`FUN_00563147`): the squad in its
/// order, sorted by main position as the game sorts — not stably —, the
/// scroll kept, no row selected.
fn fill_squad(cx: &Cx, view: &mut View) {
    let (game, dialog) = (cx.game, cx.dialog);
    let Some(season) = cx.state.season.as_ref() else { return };
    let Some(club) = season.manager().and_then(|m| m.club).and_then(|k| season.clubs.get(&k))
    else {
        return;
    };
    let records = cx.state.records.as_ref();
    let players = &club.players;
    let order = squadlist::exchange_sort((0..players.len()).collect(), |p, q| {
        players[p].positions[0] < players[q].positions[0]
    });
    let year = records.map_or(0, |r| i32::from(r.globals.start_year()));
    let today = season.date();
    let mut rows = Vec::new();
    let mut inks = Vec::new();
    let (mut salaries, mut strengths) = (0i32, 0i16);
    for &p in &order {
        let player = &players[p];
        let status = club.status.get(p).copied().unwrap_or_default();
        let record = records.and_then(|r| r.player(status.id));
        let get = |f: &dyn Fn(&a3_core::records::Player) -> u16| record.map_or(0, f);
        let salary = i64::from(get(&|r| r.salary())) * SALARY_UNIT;
        let retiring = get(&|r| r.retiring().into()) as u8 & RETIRING != 0;
        let years = ((i32::from(get(&|r| r.contract()) & CONTRACT_YEAR) - year) as i8).max(0);
        let (contract, star) = if retiring {
            (game.text(CAREER_END).to_owned(), "")
        } else if years >= 1 {
            ((year + i32::from(years)).to_string(), if years == 1 { "*" } else { "" })
        } else {
            ("-".to_owned(), "")
        };
        let fee = get(&|r| r.fixed_fee());
        let fee = if fee == 0 {
            "-".to_owned()
        } else {
            super::finances::amount(game, i64::from(fee) * FEE_UNIT)
        };
        let mut row = squad_row(cx, club, p, today).to_vec();
        row.extend([money(game, salary), contract, star.to_owned(), fee]);
        rows.push(row.join("\t"));
        let ink = if get(&|r| r.retiring().into()) as u8 & LISTED != 0 {
            LISTED_INK
        } else if get(&|r| r.terms()) & ON_LOAN != 0 {
            LOANED_INK
        } else {
            SQUAD_INK
        };
        inks.push(Some(ink));
        salaries = salaries.wrapping_add(salary as i32);
        strengths = strengths.wrapping_add(i16::from(player.strength));
    }
    if let Some(list) = control(dialog, SQUAD_LIST).and_then(|c| view.list_mut(c)) {
        let visible = control(dialog, SQUAD_LIST)
            .map_or(0, |l| crate::controls::list::visible_rows(&dialog.controls[l]));
        list.scroll = list.scroll.min(rows.len().saturating_sub(visible));
        list.selected = None;
        list.rows = rows;
        list.inks = inks;
        list.keys = order;
    }
    let count = players.len();
    let line = if count == 0 {
        game.text(NO_SQUAD_TEXT).to_owned()
    } else {
        let average = f64::from(strengths) / count as f64;
        game.text(SQUAD_TEXT).replacen("%d", &count.to_string(), 1).replacen(
            "%.1f",
            &super::playerinfo::one_decimal(average),
            1,
        )
    };
    set(view, dialog, SQUAD_LINE, line);
    if let Some(format) = control(dialog, WAGES_LINE).map(|c| dialog.controls[c].text.clone()) {
        set(view, dialog, WAGES_LINE, format.replacen("%s", &money(game, salaries.into()), 1));
    }
}

/// The market values (dialog 217, `CSpielerkaderMarktwerte`): the squad by
/// what each player is worth, with his wage, bonuses, release fee and the
/// contract's end.
#[derive(Default)]
pub struct MarketValues {
    /// The player whose menu is open.
    menu_player: Option<usize>,
}

const VALUES_LIST: u16 = 3307;
/// "Karriereende" for a player retiring; the bonuses are kept in 100s.
const CAREER_ENDS: u16 = 2487;
const BONUS_UNIT: i64 = 100;
/// The day the contracts end on, the 30th of June.
const CONTRACT_END: (u32, u32) = (30, 6);
/// The list's menu: the player's info only (`CListBoxSpielerInfo`).
const INFO_MENU: [hub::Command; 1] = [hub::Command::Info];

pub(crate) fn compose_values(dialogs: &mut HashMap<u16, Dialog>) {
    if let Some(dialog) = dialogs.get_mut(&MARKET_VALUES) {
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
        dialog.controls.push(hub::menu_control());
    }
}

impl Screen for MarketValues {
    fn ids(&self) -> &[u16] {
        &[MARKET_VALUES]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.menu_player = None;
        if let Some(season) = cx.state.season.as_ref() {
            set(view, dialog, CAPTION_ID, hub::manager_title_of(game, season, &dialog.title));
        }
        fill_values(cx, view);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some(player) = self.menu_player.take() {
            if hub::close_menu(cx, view, target).and_then(|i| INFO_MENU.get(i)).is_some() {
                let rows = control(dialog, VALUES_LIST).and_then(|l| view.list(l));
                cx.state.paging = rows.map(|l| l.keys.iter().rev().copied().collect());
                let info = super::playerinfo::CURRENT;
                return Outcome::Popup(Request::with(info, Args::Player(player)));
            }
            return Outcome::Stay;
        }
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Menu(grip @ Grip::Row { control: c, row })
                if dialog.controls[c].id == Some(VALUES_LIST) =>
            {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                if let Some(player) = player
                    && hub::show_menu(cx, view, grip, &INFO_MENU).is_some()
                {
                    self.menu_player = Some(player);
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

/// The rows (`FUN_00565dfc`): by market value, the highest first — the
/// manager and players retiring count none —, sorted by insertion.
fn fill_values(cx: &Cx, view: &mut View) {
    let (game, dialog) = (cx.game, cx.dialog);
    let Some(season) = cx.state.season.as_ref() else { return };
    let Some(club) = season.manager().and_then(|m| m.club).and_then(|k| season.clubs.get(&k))
    else {
        return;
    };
    let Some(records) = cx.state.records.as_ref() else { return };
    let today = season.date();
    let year = i32::from(records.globals.start_year());
    let stars = records.globals.stars();
    let players = &club.players;
    let record = |p: usize| club.status.get(p).and_then(|s| records.player(s.id));
    let years = |p: usize| {
        record(p).map_or(0, |r| ((i32::from(r.contract() & CONTRACT_YEAR) - year) as i8).max(0))
    };
    let real = |p: usize| club.status.get(p).is_some_and(|s| s.id <= LAST_REAL_ID);
    let retiring = |p: usize| record(p).is_some_and(|r| r.retiring() & RETIRING != 0);
    let values: Vec<i64> = (0..players.len())
        .map(|p| match record(p) {
            Some(r) if real(p) && !retiring(p) => {
                super::playerinfo::market_value(game, &players[p], r, stars, years(p).into(), today)
            }
            _ => 0,
        })
        .collect();
    let mut order: Vec<usize> = Vec::with_capacity(players.len());
    for p in 0..players.len() {
        let at = order.iter().position(|&q| values[q] < values[p]).unwrap_or(order.len());
        order.insert(at, p);
    }
    let dash = || "-".to_owned();
    let contract_end = |years: i8| {
        let date = chrono::NaiveDate::from_ymd_opt(
            year + i32::from(years),
            CONTRACT_END.1,
            CONTRACT_END.0,
        );
        date.map_or_else(dash, |d| hub::date_text(game, d))
    };
    let rows: Vec<String> = order
        .iter()
        .map(|&p| {
            let player = &players[p];
            let r = record(p);
            let get = |f: &dyn Fn(&a3_core::records::Player) -> u16| r.map_or(0, f);
            let unknown =
                matches!((get(&|r| r.picks().into()) as u8 & UNKNOWN_STRENGTH) >> 6, 1 | 3);
            let strength = if unknown { "?".to_owned() } else { player.strength.to_string() };
            let years = years(p);
            let (value, contract, fee) = if !real(p) {
                (dash(), dash(), String::new())
            } else if retiring(p) {
                let contract = if years >= 1 { contract_end(years) } else { dash() };
                (game.text(CAREER_ENDS).to_owned(), contract, String::new())
            } else {
                let contract = if years >= 1 { contract_end(years) } else { dash() };
                let fee = match get(&|r| r.fixed_fee()) {
                    0 => dash(),
                    fee => money(game, i64::from(fee) * FEE_UNIT),
                };
                (money(game, values[p]), contract, fee)
            };
            let id = club.status.get(p).map_or(0, |s| s.id);
            [
                super::duties::short_name(cx, player, id),
                hub::abbreviation(game, player.positions[0]),
                strength,
                value,
                money(game, i64::from(get(&|r| r.salary())) * SALARY_UNIT),
                money(game, i64::from(get(&|r| r.appearance_bonus())) * BONUS_UNIT),
                money(game, i64::from(get(&|r| r.goal_bonus())) * BONUS_UNIT),
                fee,
                contract,
            ]
            .join("\t")
        })
        .collect();
    if let Some(list) = control(dialog, VALUES_LIST).and_then(|c| view.list_mut(c)) {
        list.rows = rows;
        list.keys = order;
        list.selected = None;
    }
}

/// The transfers agreed with the club in them: those leaving, those
/// bought, and its players on loan coming home (`FUN_0070ffef`).
fn agreed_lists(cx: &Cx) -> [Vec<a3_core::records::Agreed>; 3] {
    let Some((slot, number)) = cx.state.season.as_ref().and_then(|s| s.manager()?.club) else {
        return Default::default();
    };
    let Some(records) = cx.state.records.as_ref() else { return Default::default() };
    let Some(nation) = cx.game.world.country(slot).map(|c| c.nation) else {
        return Default::default();
    };
    let club = (nation as u8, number as i16);
    let mut lists: [Vec<_>; 3] = Default::default();
    for a in records.agreed() {
        if a.seller == club {
            lists[0].push(a);
        } else if a.buyer == club {
            lists[if a.flagged { 2 } else { 1 }].push(a);
        }
    }
    lists
}

/// A player as a list of the squad shows him, by his record.
struct Shown {
    id: u16,
    player: a3_core::world::Player,
    fitness: a3_core::season::Fitness,
    status: a3_core::season::Status,
}

fn shown(cx: &Cx, id: u16) -> Option<Shown> {
    let records = cx.state.records.as_ref()?;
    let (player, fitness, status) = a3_core::load::any_player(records, &cx.game.data, id)?;
    Some(Shown { id, player, fitness, status })
}

/// The figures a squad's row shows of a player: name, positions,
/// strength, form, ES, age, nation, morale, condition, freshness, as 216
/// and 219 write them.
fn player_columns(cx: &Cx, s: &Shown) -> Vec<String> {
    let game = cx.game;
    let records = cx.state.records.as_ref();
    let [main, second, third] = s.player.positions;
    let sides = match (second, third) {
        (0, _) => " ".to_owned(),
        (a, 0) => hub::abbreviation(game, a),
        (a, b) => format!("{}/{}", hub::abbreviation(game, a), hub::abbreviation(game, b)),
    };
    let picks = records.and_then(|r| r.player(s.id)).map_or(0, |r| r.picks());
    let unknown = matches!((picks & UNKNOWN_STRENGTH) >> 6, 1 | 3);
    let strength = if unknown { "?".to_owned() } else { s.player.strength.to_string() };
    let pitch = &game.rules.pitch;
    let today = cx
        .state
        .season
        .as_ref()
        .map_or_else(|| a3_core::calendar::season_start(2025), |x| x.date());
    vec![
        super::duties::short_name(cx, &s.player, s.id),
        hub::abbreviation(game, main),
        sides,
        strength,
        lineup::form(s.fitness).to_string(),
        hub::tenths(pitch.strength(&s.player, s.fitness, s.status.doping)),
        s.player.age(today).to_string(),
        game.world.nation_codes.get(usize::from(s.status.nations[0])).cloned().unwrap_or_default(),
        s.fitness.morale.to_string(),
        lineup::condition(s.fitness).to_string(),
        lineup::freshness(s.fitness).to_string(),
    ]
}

/// Next season's squad (dialog 219, `CSpielerKaderNaechste`;
/// KADER-NAECHSTE-VERLIEHEN.md §2): who stays and who comes, by main
/// position; a click marks a player, up to five.
#[derive(Default)]
pub struct NextSquad {
    marks: [u16; MARKS],
    menu_player: Option<u16>,
}

const MARKS: usize = 5;
const NEXT_LIST: u16 = 3318;
const NEXT_LINE: u16 = 3317;
const NEXT_FEES: u16 = 3322;
const NEXT_TITLE: u16 = 1686;
const NEW_INK: [u8; 3] = [255, 0, 0];
const BLACK: [u8; 3] = [0, 0, 0];
/// The contracts end on the 30th of June.
const JUNE: u32 = 6;
const LAST_JUNE_DAY: u32 = 30;
/// The manager shows in the list while he has more than a year left.
const MANAGER_YEARS: i16 = 1;
/// R+0x14c: the club he signed with for next season.
const NEXT_CLUB_COUNTRY: usize = 0x14c;
const INFO_ONLY: [hub::Command; 1] = [hub::Command::Info];

pub(crate) fn compose_next(dialogs: &mut HashMap<u16, Dialog>) {
    let ids = [
        NEXT_SQUAD,
        LOANED_OUT,
        EXTENSIONS,
        DRAW_OPTION,
        RELATIONS,
        NO_DATE,
        LISTING_DEMAND,
        LOAN_CONDITIONS,
    ];
    for id in ids {
        if let Some(dialog) = dialogs.get_mut(&id) {
            let kept =
                matches!(id, DRAW_OPTION | RELATIONS | NO_DATE | LISTING_DEMAND | LOAN_CONDITIONS);
            let title = if kept { dialog.title.clone() } else { String::new() };
            dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, title));
            if matches!(id, NEXT_SQUAD | EXTENSIONS) {
                dialog.controls.push(hub::menu_control());
            }
        }
    }
}

impl Screen for NextSquad {
    fn ids(&self) -> &[u16] {
        &[NEXT_SQUAD]
    }

    /// `FUN_00567920`: the rows, no count line, the fees of the players
    /// bought, the caption without a date.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.marks = [0; MARKS];
        self.menu_player = None;
        if let Some(season) = cx.state.season.as_ref() {
            set(view, dialog, CAPTION_ID, hub::club_title(game, season, NEXT_TITLE));
        }
        set(view, dialog, NEXT_LINE, String::new());
        let [_, bought, _] = agreed_lists(cx);
        let fees = bought.iter().fold(0i32, |s, a| s.wrapping_add(a.fee as i32));
        set(view, dialog, NEXT_FEES, money(game, fees.into()));
        self.fill(cx, view);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some(id) = self.menu_player.take() {
            if hub::close_menu(cx, view, target).and_then(|i| INFO_ONLY.get(i)).is_some() {
                let rows = control(dialog, NEXT_LIST).and_then(|l| view.list(l));
                cx.state.paging_ids =
                    rows.map(|l| l.keys.iter().rev().map(|&k| k as u16).collect());
                return Outcome::Popup(Request::with(super::playerinfo::CURRENT, Args::Record(id)));
            }
            return Outcome::Stay;
        }
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Menu(grip @ Grip::Row { control: c, row }) if id(c) == Some(NEXT_LIST) => {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                if let Some(player) = player
                    && hub::show_menu(cx, view, grip, &INFO_ONLY).is_some()
                {
                    self.menu_player = Some(player as u16);
                }
                Outcome::Stay
            }
            // A click marks the player or unmarks him; while a mark
            // changes the list is filled anew, from its top
            // (`OnSelChange_IDC_KADER_NAECHSTE_LISTE`).
            Target::Row { control: c, row } if id(c) == Some(NEXT_LIST) => {
                let Some(player) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied())
                else {
                    return Outcome::Stay;
                };
                let player = player as u16;
                if let Some(mark) = self.marks.iter_mut().find(|m| **m == player) {
                    *mark = 0;
                } else if let Some(mark) = self.marks.iter_mut().find(|m| **m == 0) {
                    *mark = player;
                } else {
                    return Outcome::Stay;
                }
                self.fill(cx, view);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

impl NextSquad {
    /// The rows (`FUN_00568118`): the squad that stays, then the players
    /// bought and those coming back, sorted by main position as the game
    /// sorts; the list at its top, nothing selected.
    fn fill(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(season) = cx.state.season.as_ref() else { return };
        let Some(club) = season.manager().and_then(|m| m.club).and_then(|k| season.clubs.get(&k))
        else {
            return;
        };
        let year = i32::from(records.globals.start_year());
        let manager_shown = records
            .human_slot(season.current)
            .and_then(|slot| records.managers.get(slot))
            .is_some_and(|m| m.contract_years() > MANAGER_YEARS);
        // Each row: the player, his contract's salary, release fee and
        // years, whether he comes to the club.
        let mut rows: Vec<(Shown, u16, u16, i8, bool)> = Vec::new();
        for status in &club.status {
            let Some(r) = records.player(status.id) else { continue };
            let years = status.contract.unwrap_or(0).clamp(i8::MIN.into(), i8::MAX.into()) as i8;
            let leaving = r.get::<u8>(NEXT_CLUB_COUNTRY) != 0;
            let skip = leaving
                || r.retiring() & RETIRING != 0
                || years <= 1
                || r.terms() & ON_LOAN != 0
                || (status.id > LAST_REAL_ID && !manager_shown);
            if skip {
                continue;
            }
            if let Some(s) = shown(cx, status.id) {
                rows.push((s, r.salary(), r.fixed_fee(), years, false));
            }
        }
        let [_, bought, returning] = agreed_lists(cx);
        for a in bought.iter().chain(&returning) {
            if let Some(s) = shown(cx, a.player) {
                rows.push((s, a.salary, a.fixed_fee, a.years, true));
            }
        }
        let order = squadlist::exchange_sort((0..rows.len()).collect(), |p, q| {
            rows[p].0.player.positions[0] < rows[q].0.player.positions[0]
        });
        let mut texts = Vec::new();
        let mut inks = Vec::new();
        let mut keys = Vec::new();
        for &i in &order {
            let (s, salary, fee, years, incoming) = &rows[i];
            let mut columns = player_columns(cx, s);
            columns.push(money(game, i64::from(*salary) * SALARY_UNIT));
            let until = i32::from(*years) + i32::from(*incoming);
            columns.push(if until >= 1 {
                chrono::NaiveDate::from_ymd_opt(year + i32::from(*years), JUNE, LAST_JUNE_DAY)
                    .map_or_else(|| "-".to_owned(), |d| hub::date_text(game, d))
            } else {
                "-".to_owned()
            });
            columns.push(if *fee == 0 {
                "-".to_owned()
            } else {
                super::finances::amount(game, i64::from(*fee) * FEE_UNIT)
            });
            texts.push(columns.join("\t"));
            inks.push(Some(if *incoming { NEW_INK } else { BLACK }));
            keys.push(usize::from(s.id));
        }
        if let Some(list) = control(dialog, NEXT_LIST).and_then(|c| view.list_mut(c)) {
            list.rows = texts;
            list.inks = inks;
            list.keys = keys;
            list.scroll = 0;
            list.selected = None;
        }
    }
}

/// The players out on loan (dialog 300, `CUebersichtVerliehene`;
/// KADER-NAECHSTE-VERLIEHEN.md §3): who, his contract with the club, a
/// purchase option, where he plays, how content he is. The list takes no
/// click (its style lacks 0x100).
#[derive(Default)]
pub struct LoanedOut;

const LOANED_LIST: u16 = 4029;
/// A borrower's option to buy (R+0x15e bit 0x200).
const PURCHASE_OPTION: u16 = 0x200;
const YES: u16 = 1028;
const NO: u16 = 1029;
/// The contentment icons by level, where the list puts them.
const CONTENT_ICONS: [u16; 4] = [1807, 1808, 1806, 1809];
const LOANED_PLAYER_COLUMNS: usize = 8;

impl Screen for LoanedOut {
    fn ids(&self) -> &[u16] {
        &[LOANED_OUT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        if let Some(season) = cx.state.season.as_ref() {
            set(view, dialog, CAPTION_ID, hub::manager_title_of(game, season, &dialog.title));
        }
        let Some(records) = cx.state.records.as_ref() else { return };
        let year = i32::from(records.globals.start_year());
        let [_, _, returning] = agreed_lists(cx);
        let slots = control(dialog, LOANED_LIST)
            .map(|c| crate::controls::list::icon_slots(&dialog.controls[c]))
            .unwrap_or_default();
        let mut texts = Vec::new();
        let mut icons = Vec::new();
        for a in &returning {
            let (Some(s), Some(r)) = (shown(cx, a.player), records.player(a.player)) else {
                continue;
            };
            // Name to nation, then the contract's year, the option, his club.
            let mut columns = player_columns(cx, &s);
            columns.truncate(LOANED_PLAYER_COLUMNS);
            columns.push((year + i32::from(a.years)).to_string());
            let option = r.terms() & PURCHASE_OPTION != 0;
            columns.push(game.text(if option { YES } else { NO }).to_owned());
            let [country, number] = r.club();
            columns.push(game.club_name(Some(records), country, number.into()));
            texts.push(columns.join("\t"));
            let level = a3_core::mood::content_level(a3_core::mood::parts(r));
            icons.push(slots.iter().copied().zip(CONTENT_ICONS.get(level).copied()).collect());
        }
        if let Some(list) = control(dialog, LOANED_LIST).and_then(|c| view.list_mut(c)) {
            list.inks = vec![Some(BLACK); texts.len()];
            list.rows = texts;
            list.row_icons = icons;
            list.keyed = CONTENT_ICONS.to_vec();
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            // The list takes no click: nothing stays chosen.
            Target::Row { control: c, .. } => {
                if let Some(list) = view.list_mut(c) {
                    list.selected = None;
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

/// The semi-automatic contract extension (dialog 92,
/// `CHalbautomatischeAuswahl`; VERLAENGERUNG-ABWERBEN.md §1–2): first the
/// players in their contract's last year, a click offering each a year
/// more on his terms; then those the club holds an option on, a click
/// drawing it.
#[derive(Default)]
pub struct Extensions {
    options: bool,
    /// What the popup open asks of the screen once it closes.
    after: After,
    menu_player: Option<u16>,
}

/// What follows a popup of the extensions.
#[derive(Clone, Copy, Default)]
enum After {
    #[default]
    Nothing,
    Refill,
    /// Back to the first mode: too late in the season for options.
    FirstMode,
    /// The option on the player drawn, if the question says so.
    Draw(u16),
}

const EXTENSION_LIST: u16 = 1883;
const EXTENSION_MODES: u16 = 1884;
const FIRST_MODE: usize = 0;
const OPTIONS_MODE: usize = 1;
/// Options are drawn up to the 30th of April (alert 4299).
const NO_OPTIONS: std::ops::RangeInclusive<u32> = 5..=6;
const TOO_LATE: u16 = 4299;
const EXTENDS: u16 = 4178;
const DECLINES: u16 = 3110;
/// R+0x26f bit 6: he declined the semi-automatic extension.
const DECLINED: u8 = 0x40;
/// The strength class that keeps a player out (`FUN_00706a7e`).
const UNLISTED_CLASS: u8 = 1;
/// The second mode's players: those the club holds an option on.
const CLUB_OPTION: u16 = a3_core::contract::CLUB_OPTION;
/// The first mode's: those with less than two years left.
const LAST_YEARS: i8 = 2;
/// The contract index below and above which a row shows green and red;
/// a decline shows blue (`FUN_00708ca5`).
const CHEAP: i32 = 80;
const DEAR: i32 = 120;
const CHEAP_INK: [u8; 3] = [0, 160, 0];
const DEAR_INK: [u8; 3] = [200, 0, 0];
const DECLINED_INK: [u8; 3] = [0, 0, 160];
/// What hurts a player whose option is drawn on cheap terms (the club's
/// reason 0x20).
const CHEAP_OPTION: u32 = 0x20;
/// The columns of 219 but the morale.
const MORALE_COLUMN: usize = 8;
const QUESTION_TEXT: u16 = 4056;
const DRAW: u16 = 4057;
const DONT_DRAW: u16 = 4058;

impl Screen for Extensions {
    fn ids(&self) -> &[u16] {
        &[EXTENSIONS]
    }

    /// `FUN_004b6e56`: the first mode chosen and filled, the caption.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        if let Some(season) = cx.state.season.as_ref() {
            set(view, dialog, CAPTION_ID, hub::manager_title_of(game, season, &dialog.title));
        }
        if let Some(c) = control(dialog, EXTENSION_MODES) {
            view.set_chosen(c, FIRST_MODE);
        }
        if let Some(list) = control(dialog, EXTENSION_LIST).and_then(|c| view.list_mut(c)) {
            list.keyed = CONTENT_ICONS.to_vec();
        }
        self.fill(cx, view);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let notice = |text| Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
        if let Some(id) = self.menu_player.take() {
            if hub::close_menu(cx, view, target).and_then(|i| INFO_ONLY.get(i)).is_some() {
                let rows = control(dialog, EXTENSION_LIST).and_then(|l| view.list(l));
                cx.state.paging_ids =
                    rows.map(|l| l.keys.iter().rev().map(|&k| k as u16).collect());
                return Outcome::Popup(Request::with(super::playerinfo::CURRENT, Args::Record(id)));
            }
            return Outcome::Stay;
        }
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Option { control: c, item } if id(c) == Some(EXTENSION_MODES) => {
                let month = cx.state.season.as_ref().map_or(0, |s| s.date().month());
                if item == OPTIONS_MODE && NO_OPTIONS.contains(&month) {
                    self.after = After::FirstMode;
                    return notice(TOO_LATE);
                }
                self.options = item == OPTIONS_MODE;
                self.fill(cx, view);
                Outcome::Stay
            }
            Target::Menu(grip @ Grip::Row { control: c, row }) if id(c) == Some(EXTENSION_LIST) => {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                if let Some(player) = player
                    && hub::show_menu(cx, view, grip, &INFO_ONLY).is_some()
                {
                    self.menu_player = Some(player as u16);
                }
                Outcome::Stay
            }
            // `OnSelChange_IDC_HALB_AUSWAHL_LISTE`: the row stays selected
            // under the popup; the refill after it drops it.
            Target::Row { control: c, row } if id(c) == Some(EXTENSION_LIST) => {
                let Some(player) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied())
                else {
                    return Outcome::Stay;
                };
                let player = player as u16;
                if self.options {
                    let held = cx
                        .state
                        .records
                        .as_ref()
                        .and_then(|r| r.player(player))
                        .is_some_and(|r| r.terms() & CLUB_OPTION != 0);
                    if !held {
                        return Outcome::Stay;
                    }
                    self.after = After::Draw(player);
                    return Outcome::Popup(Request::with(DRAW_OPTION, Args::Record(player)));
                }
                let Some(extends) = offer(cx, player) else { return Outcome::Stay };
                self.after = After::Refill;
                if extends {
                    extend(cx, player);
                    Outcome::Popup(Request::with(NOTICE, Args::TextFor(EXTENDS, player)))
                } else {
                    if let Some(r) = cx
                        .state
                        .records
                        .as_mut()
                        .and_then(|r| r.players.get_mut(usize::from(player)))
                    {
                        r.set_retiring(r.retiring() | DECLINED);
                    }
                    notice(DECLINES)
                }
            }
            _ => Outcome::Stay,
        }
    }

    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        match std::mem::take(&mut self.after) {
            After::Nothing => return Outcome::Stay,
            After::Refill => {}
            After::FirstMode => {
                if let Some(c) = control(cx.dialog, EXTENSION_MODES) {
                    view.set_chosen(c, FIRST_MODE);
                }
                self.options = false;
            }
            // `FUN_00709c74`: an option drawn on cheap terms hurts him.
            After::Draw(id) if reply == Reply::Ok => {
                let cheap = with_reckoning(cx, |club| {
                    signee(cx, id).map(|p| cx.game.rules.contract.index(&p, club) < CHEAP)
                });
                if let Some(r) =
                    cx.state.records.as_mut().and_then(|r| r.players.get_mut(usize::from(id)))
                {
                    if cheap.flatten() == Some(true) {
                        cx.game.rules.discipline.mood.set_reasons(r, CHEAP_OPTION);
                    }
                    r.set_terms(r.terms() & !CLUB_OPTION);
                }
                extend(cx, id);
            }
            After::Draw(_) => {}
        }
        self.fill(cx, view);
        Outcome::Stay
    }
}

impl Extensions {
    /// The rows (`FUN_004b6f95`): the squad in its order, but those on loan
    /// with the club, ending their career, of strength class 1, signed
    /// elsewhere or with a talk pending; the list at its top, nothing
    /// selected.
    fn fill(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(club) = own_club(cx) else { return };
        let year = i32::from(records.globals.start_year());
        let slots = control(dialog, EXTENSION_LIST)
            .map(|c| crate::controls::list::icon_slots(&dialog.controls[c]))
            .unwrap_or_default();
        let (mut texts, mut inks, mut keys, mut icons) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        with_reckoning(cx, |reckoning| {
            for status in &club.status {
                let Some(r) = records.player(status.id) else { continue };
                let years = years_left(r, year);
                let wanted = if self.options {
                    r.terms() & CLUB_OPTION != 0
                } else {
                    years < LAST_YEARS && r.retiring() & DECLINED == 0
                };
                let skip = r.terms() & ON_LOAN != 0
                    || r.retiring() & RETIRING != 0
                    || (r.picks() & UNKNOWN_STRENGTH) >> 6 == UNLISTED_CLASS
                    || r.get::<u8>(NEXT_CLUB) != 0
                    || !wanted
                    || a3_core::transfers::pending(records, status.id);
                let (false, Some(s), Some(p)) = (skip, shown(cx, status.id), signee(cx, status.id))
                else {
                    continue;
                };
                let mut columns = player_columns(cx, &s);
                columns.remove(MORALE_COLUMN);
                columns.push(money(game, i64::from(r.salary()) * SALARY_UNIT));
                columns.push(if years >= 1 {
                    chrono::NaiveDate::from_ymd_opt(year + i32::from(years), JUNE, LAST_JUNE_DAY)
                        .map_or_else(|| "-".to_owned(), |d| hub::date_text(game, d))
                } else {
                    "-".to_owned()
                });
                let option = r.terms() & CLUB_OPTION != 0;
                columns.push(game.text(if option { YES } else { NO }).to_owned());
                texts.push(columns.join("\t"));
                let index = game.rules.contract.index(&p, reckoning);
                inks.push(Some(if index < CHEAP {
                    CHEAP_INK
                } else if index > DEAR {
                    DEAR_INK
                } else if r.retiring() & DECLINED != 0 {
                    DECLINED_INK
                } else {
                    BLACK
                }));
                icons.push(
                    slots
                        .iter()
                        .copied()
                        .zip(CONTENT_ICONS.get(p.content_level).copied())
                        .collect(),
                );
                keys.push(usize::from(status.id));
            }
        });
        if let Some(list) = control(dialog, EXTENSION_LIST).and_then(|c| view.list_mut(c)) {
            list.rows = texts;
            list.inks = inks;
            list.keys = keys;
            list.row_icons = icons;
            list.scroll = 0;
            list.selected = None;
        }
    }
}

/// The manager's club's state.
fn own_club<'a>(cx: &'a Cx) -> Option<&'a a3_core::season::ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

/// A contract's years left, this season's counted, none below 0.
fn years_left(r: &a3_core::records::Player, year: i32) -> i8 {
    ((i32::from(r.contract() & CONTRACT_YEAR) - year) as i8).max(0)
}

/// What the manager's club is to a contract's reckoning: its squad, its
/// league and the matchdays played, the month.
fn with_reckoning<T>(cx: &Cx, f: impl FnOnce(&a3_core::contract::Club) -> T) -> Option<T> {
    let records = cx.state.records.as_ref()?;
    let season = cx.state.season.as_ref()?;
    let club = own_club(cx)?;
    let team = own_team(cx)?;
    let squad: Vec<_> = club
        .status
        .iter()
        .zip(&club.players)
        .map(|(s, p)| (s.id, p.positions[0], p.strength))
        .collect();
    let (matchdays, played) = a3_core::tickets::league_days(records, team);
    let month = usize::try_from(season.date().month()).unwrap_or(0);
    Some(f(&a3_core::contract::Club {
        level: team.level(),
        squad: &squad,
        matchdays,
        played,
        month,
    }))
}

/// A player as his contract's reckoning sees him, by his record.
fn signee(cx: &Cx, id: u16) -> Option<a3_core::contract::Signee> {
    let records = cx.state.records.as_ref()?;
    let s = shown(cx, id)?;
    let today = cx.state.season.as_ref()?.date();
    let star = records.globals.stars().get(usize::from(s.player.positions[0])) == Some(&id);
    Some(a3_core::contract::Signee::of(
        records.player(id)?,
        s.player.age(today) as i32,
        lineup::form(s.fitness),
        &cx.game.rules.value,
        star,
        records.globals.start_year(),
    ))
}

/// Whether he takes a year more on his terms (`FUN_004b784c`): by luck,
/// one draw below a hundred.
fn offer(cx: &mut Cx, id: u16) -> Option<bool> {
    let p = signee(cx, id)?;
    let answer = with_reckoning(cx, |club| cx.game.rules.contract.extension(&p, club))?;
    Some(match answer {
        a3_core::contract::Extension::Yes => true,
        a3_core::contract::Extension::No => false,
        a3_core::contract::Extension::ByLuck(chance) => {
            cx.state.season.as_mut()?.random.below(100) < chance
        }
    })
}

/// His contract a year longer on its terms.
fn extend(cx: &mut Cx, id: u16) {
    let Some(records) = cx.state.records.as_mut() else { return };
    let year = i32::from(records.globals.start_year());
    let Some(r) = records.players.get_mut(usize::from(id)) else { return };
    let years = years_left(r, year) + 1;
    let until = (year + i32::from(years)) as u16 & CONTRACT_YEAR;
    r.set_contract(r.contract() & !CONTRACT_YEAR | until);
    let Some(season) = cx.state.season.as_mut() else { return };
    let Some(club) = season.manager().and_then(|m| m.club) else { return };
    if let Some(status) =
        season.clubs.get_mut(&club).and_then(|c| c.status.iter_mut().find(|s| s.id == id))
    {
        status.contract = Some(years.into());
    }
}

/// A player under contract asked onto the transfer list (dialog 205,
/// `CSpielerMitVertragAufTL`): his contract's end and what he wants for
/// it; the choice and the "ablösefrei" box go back as the choice × 2 +
/// the box.
#[derive(Default)]
pub struct ListingDemand {
    demand: i32,
}

const UNTIL_TEXT: u16 = 3020;
const DEMAND_TEXT: u16 = 3021;
const DEMAND_CHOICES: u16 = 3022;
const FREE_BOX: u16 = 6047;
/// "%1.%2.%3", a date; contracts run to the 30th of June.
const DATE_TEXT: u16 = 1997;

impl Screen for ListingDemand {
    fn ids(&self) -> &[u16] {
        &[LISTING_DEMAND]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Args::Record(id) = args else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(r) = records.player(id) else { return };
        let year = i32::from(records.globals.start_year());
        let years = years_left(r, year);
        let month = cx.state.season.as_ref().map_or(0, |s| s.date().month() as usize);
        let regular = r.terms() & REGULAR_PLACE != 0;
        self.demand = game.rules.contract.listing_demand(id, r.salary(), years, month, regular);
        let name = records.known_as(id).unwrap_or_default();
        let until = game
            .text(DATE_TEXT)
            .replacen("%1", "30", 1)
            .replacen("%2", "6", 1)
            .replacen("%3", &(year + i32::from(years)).to_string(), 1);
        let text = |c: u16| control(dialog, c).map(|i| dialog.controls[i].text.clone());
        let first = text(UNTIL_TEXT).unwrap_or_default().replacen("%s", &name, 1);
        set(view, dialog, UNTIL_TEXT, first.replacen("%s", &until, 1));
        let demand = money(game, i64::from(self.demand));
        let second = text(DEMAND_TEXT).unwrap_or_default().replacen("%s", &demand, 1);
        set(view, dialog, DEMAND_TEXT, second);
        if let Some(c) = control(dialog, DEMAND_CHOICES) {
            view.set_chosen(c, NONE);
        }
        super::market::question_ball(view, dialog);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Option { control: c, item }
                if dialog.controls[c].id == Some(DEMAND_CHOICES) =>
            {
                let free_box = control(dialog, FREE_BOX).and_then(|b| view.check(b));
                let free = free_box.is_some_and(|b| b.checked);
                Outcome::Close(Reply::Choice(item * 2 + usize::from(free)))
            }
            _ => Outcome::Stay,
        }
    }
}

/// The loan's conditions (dialog 292, `CTransferlisteSetzenLeihe`): with
/// an option to buy, the fee and the option's price; without one, the
/// fee — each beside its fair price (`FUN_00622f7d`), set on a spinner
/// from 85 to 115 per cent of it in the steps of `0x88be90`. OK lists him
/// for the loan chosen (`FUN_0075b0da`).
#[derive(Default)]
pub struct LoanConditions {
    player: u16,
    without_option: bool,
}

const LOAN_TITLE: u16 = 3966;
/// Each amount's fair text, shown amount and spinner: the fee with an
/// option, the option, the fee without one.
const LOAN_AMOUNTS: [(u16, u16, u16); 3] =
    [(3962, 3963, 3971), (3964, 3967, 3972), (3965, 3968, 3973)];
const WITH_OPTION: u16 = 3974;
const WITHOUT_OPTION: u16 = 3975;
const LOAN_OK: u16 = 3969;
const LOAN_CANCEL: u16 = 3970;
/// The spinners' steps in thousands: up to 50 by one, … beyond 10,000 by
/// 200 (`0x88be90`).
const LOAN_SCALE: [(i32, i32); 8] =
    [(50, 1), (100, 2), (300, 5), (1000, 10), (3000, 20), (6000, 50), (10_000, 100), (-1, 200)];
const THOUSAND: i32 = 1000;

impl LoanConditions {
    fn fair(cx: &Cx, id: u16) -> Option<[i32; 3]> {
        let records = cx.state.records.as_ref()?;
        let today = cx.state.season.as_ref()?.date();
        let (player, _, status) = a3_core::load::any_player(records, &cx.game.data, id)?;
        let r = records.player(id)?;
        let worth = super::playerinfo::worth(&player, r, records.globals.stars(), today);
        let years = status.contract.unwrap_or(0);
        let p = cx.game.rules.value.loan_prices(&worth, years, today.month());
        Some([p.with_option, p.option, p.fee])
    }

    fn amount(view: &View, dialog: &Dialog, spin: u16) -> i32 {
        let position = control(dialog, spin).and_then(|c| view.value(c)).map_or(0, |v| v.value);
        a3_core::discipline::amount(&LOAN_SCALE, position) * THOUSAND
    }

    fn show(&self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        for (_, shown, spin) in LOAN_AMOUNTS {
            let amount = Self::amount(view, dialog, spin);
            set(view, dialog, shown, money(cx.game, i64::from(amount)));
        }
        let chosen = [(WITH_OPTION, !self.without_option), (WITHOUT_OPTION, self.without_option)];
        for (id, on) in chosen {
            if let Some(c) = control(dialog, id) {
                view.set_chosen(c, if on { 0 } else { NONE });
            }
        }
    }
}

impl Screen for LoanConditions {
    fn ids(&self) -> &[u16] {
        &[LOAN_CONDITIONS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Args::Record(id) = args else { return };
        (self.player, self.without_option) = (id, false);
        let name = cx.state.records.as_ref().and_then(|r| r.known_as(id)).unwrap_or_default();
        let text = |c: u16| control(dialog, c).map(|i| dialog.controls[i].text.clone());
        let title = text(LOAN_TITLE).unwrap_or_default().replacen("%s", &name, 1);
        set(view, dialog, LOAN_TITLE, title);
        let fair = Self::fair(cx, id).unwrap_or_default();
        let place = |v: i32| a3_core::discipline::position(&LOAN_SCALE, v / THOUSAND);
        for ((fair_text, _, spin), fair) in LOAN_AMOUNTS.into_iter().zip(fair) {
            let line = text(fair_text).unwrap_or_default();
            set(view, dialog, fair_text, line.replacen("%s", &money(game, i64::from(fair)), 1));
            if let Some(value) = control(dialog, spin).and_then(|c| view.value_mut(c)) {
                let unit = (fair as u32 / 100) as i32;
                value.range = (place(unit * 85), place(unit * 115));
                value.value = place(fair);
            }
        }
        self.show(cx, view);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Spin { .. } => self.show(cx, view),
            Target::Option { control: c, .. } if id(c) == Some(WITH_OPTION) => {
                self.without_option = false;
                self.show(cx, view);
            }
            Target::Option { control: c, .. } if id(c) == Some(WITHOUT_OPTION) => {
                self.without_option = true;
                self.show(cx, view);
            }
            Target::Button(b) if id(b) == Some(LOAN_CANCEL) => {
                return Outcome::Close(Reply::Cancel);
            }
            Target::Button(b) if id(b) == Some(LOAN_OK) => {
                let [with_fee, option, fee] =
                    LOAN_AMOUNTS.map(|(_, _, spin)| Self::amount(view, dialog, spin));
                if let Some(records) = cx.state.records.as_mut() {
                    let (fee, option) =
                        if self.without_option { (fee, 0) } else { (with_fee, option) };
                    a3_core::transfers::list_for_loan(
                        records,
                        self.player,
                        !self.without_option,
                        fee,
                        option,
                    );
                }
                return Outcome::Close(Reply::Ok);
            }
            _ => {}
        }
        Outcome::Stay
    }
}

/// "Kein Termin mehr frei" (dialog 262, `CSummeKeinTermin`): the player
/// has no appointment free this week; OK closes it.
pub struct NoDate;

const NO_DATE_TEXT: u16 = 3704;
const NO_DATE_OK: u16 = 3705;

impl Screen for NoDate {
    fn ids(&self) -> &[u16] {
        &[NO_DATE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        if let Args::Record(id) = args {
            let name = cx.state.records.as_ref().and_then(|r| r.known_as(id)).unwrap_or_default();
            let text = control(dialog, NO_DATE_TEXT).map(|c| dialog.controls[c].text.clone());
            set(view, dialog, NO_DATE_TEXT, text.unwrap_or_default().replacen("%s", &name, 1));
        }
        super::popups::ball(view, cx);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(NO_DATE_OK) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}

/// Whether the club draws its option on a player (dialog 305,
/// `CVereinZiehtOption`).
pub struct DrawOption;

impl Screen for DrawOption {
    fn ids(&self) -> &[u16] {
        &[DRAW_OPTION]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        if let Args::Record(id) = args {
            let name = cx.state.records.as_ref().and_then(|r| r.known_as(id)).unwrap_or_default();
            let text = control(dialog, QUESTION_TEXT).map(|c| dialog.controls[c].text.clone());
            set(view, dialog, QUESTION_TEXT, text.unwrap_or_default().replacen("%s", &name, 1));
        }
        super::market::question_ball(view, dialog);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(DRAW) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Button(c) if cx.dialog.controls[c].id == Some(DONT_DRAW) => {
                Outcome::Close(Reply::Cancel)
            }
            _ => Outcome::Stay,
        }
    }
}

/// The squad's friendships and enmities (dialog 500,
/// `CSpielerkaderKritisch`): each player's group, then his best friend and
/// his friends, or the one he hates and those he dislikes — as far as they
/// play for the club. The list takes no click (its style lacks 0x100).
#[derive(Default)]
pub struct Relations;

const RELATIONS_LIST: u16 = 3298;
const RELATIONS_TITLES: u16 = 6281;
const RELATION_MODES: u16 = 1884;
const FRIENDSHIPS: usize = 0;
const FRIENDSHIP_TITLES: u16 = 4426;
const ENMITY_TITLES: u16 = 4427;
/// R+0x280's low two bits: the player's group, 0 none; shown from "B".
const GROUP_AT: usize = 0x280;
const GROUPS: u8 = 3;
const NO_ONE: &str = "-";

impl Screen for Relations {
    fn ids(&self) -> &[u16] {
        &[RELATIONS]
    }

    /// `FUN_005656ff`: the friendships first.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        if let Some(c) = control(cx.dialog, RELATION_MODES) {
            view.set_chosen(c, FRIENDSHIPS);
        }
        fill_relations(cx, view, true);
        mark_inert(view, cx.dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Option { control: c, item } if id(c) == Some(RELATION_MODES) => {
                fill_relations(cx, view, item == FRIENDSHIPS);
                Outcome::Stay
            }
            Target::Row { control: c, .. } => {
                if let Some(list) = view.list_mut(c) {
                    list.selected = None;
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

/// The rows (`FUN_0056572c`), in the squad's order, and the titles of
/// the mode; the list at its top, nothing selected.
fn fill_relations(cx: &Cx, view: &mut View, friends: bool) {
    let (game, dialog) = (cx.game, cx.dialog);
    let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
    else {
        return;
    };
    let Some(club) = season.manager().and_then(|m| m.club).and_then(|k| season.clubs.get(&k))
    else {
        return;
    };
    let titles = if friends { FRIENDSHIP_TITLES } else { ENMITY_TITLES };
    set(view, dialog, RELATIONS_TITLES, game.text(titles).to_owned());
    let in_club = |id: u16| id != 0 && club.status.iter().any(|s| s.id == id);
    let name = |id: u16| records.known_as(id).unwrap_or_default();
    let (mut rows, mut keys) = (Vec::new(), Vec::new());
    for status in &club.status {
        let Some(r) = records.player(status.id) else { continue };
        let group = r.get::<u8>(GROUP_AT) & GROUPS;
        let group =
            if group == 0 { NO_ONE.to_owned() } else { char::from(b'A' + group).to_string() };
        let (one, three) =
            if friends { (r.best_friend(), r.friends()) } else { (r.hates(), r.dislikes()) };
        let one = if in_club(one) { name(one) } else { NO_ONE.to_owned() };
        let three: Vec<String> = three.into_iter().filter(|&id| in_club(id)).map(name).collect();
        rows.push(format!("{}\t{group}\t{one}\t{}", name(status.id), three.join(",")));
        keys.push(usize::from(status.id));
    }
    if let Some(list) = control(dialog, RELATIONS_LIST).and_then(|c| view.list_mut(c)) {
        list.inks = vec![Some(BLACK); rows.len()];
        list.rows = rows;
        list.keys = keys;
        list.scroll = 0;
        list.selected = None;
    }
}
