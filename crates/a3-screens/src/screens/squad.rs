//! The squad and the transfer market, the menu bar's eleventh symbol: a
//! menu of the squad's screens and the market's (dialog 218; SPIELERKADER.md
//! in the RE repo).

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::CURRENT_SQUAD;
use a3_core::cycle::dialogs::NOTICE;
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
/// The current squad's button.
const CURRENT_SQUAD_ITEM: usize = 0;
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
                refusal(cx, item).is_some()
            }
            Target::Option { control: c, item } if dialog.controls[c].id == Some(SQUAD_SCREENS) => {
                item == CURRENT_SQUAD_ITEM
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
            Target::Option { control: c, item: CURRENT_SQUAD_ITEM }
                if cx.dialog.controls[c].id == Some(SQUAD_SCREENS) =>
            {
                Outcome::Popup(Request::new(CURRENT_SQUAD))
            }
            Target::Option { control: c, item }
                if cx.dialog.controls[c].id == Some(MARKET_SCREENS) =>
            {
                match refusal(cx, item) {
                    Some(text) => Outcome::Popup(Request::with(NOTICE, Args::Text(text))),
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
fn own_team<'a>(cx: &'a Cx) -> Option<&'a Team> {
    let (slot, number) = cx.state.season.as_ref()?.manager()?.club?;
    let nation = cx.game.world.country(slot)?.nation;
    cx.state.records.as_ref()?.team(nation, u16::try_from(number).ok()?)
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
                matches!(MODES.get(item), Some(Mode::Bonus | Mode::Fine))
            }
            Target::Button(c) => dialog.controls[c].id != Some(CRITICAL),
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
                    (Some(mode), Some(player)) => act(cx, mode, player),
                    _ => Outcome::Stay,
                }
            }
            _ => Outcome::Stay,
        }
    }
}

/// A row's player acted on by the mode chosen (`OnSelChange` 0x564c02):
/// the bonus and the fine in their dialogs; the contract, the transfer
/// list and the loan as far as their refusals go.
fn act(cx: &Cx, mode: Mode, player: usize) -> Outcome {
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
        None => {
            info!("{mode:?} ({id}): not built yet");
            Outcome::Stay
        }
    }
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
    let pitch = &game.rules.pitch;
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
        let strength = if unknown { "?".to_owned() } else { player.strength.to_string() };
        let es = hub::tenths(pitch.strength(player, fitness, status.doping));
        let nation = game.world.nation_codes.get(usize::from(status.nations[0])).cloned();
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
        rows.push(
            [
                hub::name(player),
                hub::abbreviation(game, main),
                sides,
                strength,
                lineup::form(fitness).to_string(),
                es,
                player.age(today).to_string(),
                nation.unwrap_or_default(),
                fitness.morale.to_string(),
                lineup::condition(fitness).to_string(),
                lineup::freshness(fitness).to_string(),
                money(game, salary),
                contract,
                star.to_owned(),
                fee,
            ]
            .join("\t"),
        );
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
