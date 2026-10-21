//! The club's money, the menu bar's ninth and tenth symbols: the balance
//! sheet (dialog 349, `CBilanzNeu`; BILANZ.md in the RE repo) and the
//! finances (dialog 417, `CFinanzuebersicht`; FINANZUEBERSICHT.md).

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::finances::{self, Balance as Figures, LEAST_LOAN, Refusal};
use a3_core::forecast::Forecast;
use a3_core::records::{Records, Team};
use a3_core::season::Rules;
use a3_files::rch::Dialog;
use tracing::info;

use super::hub::{self, BALANCE, BALANCE_SYMBOL, FINANCES, FINANCES_SYMBOL};
use super::kalkulation;
use super::playerinfo::money;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::bitmap::Picture;
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;
use a3_core::cycle::dialogs::{INVESTMENTS, LOANS as LOANS_DIALOG, NO_CREDIT, NOTICE};

/// The OK, whose place the bar takes.
const OK: u16 = 1;
const CAPTION_ID: u16 = 9933;
const FINANCES_CAPTION: u16 = 2519;

/// The balance sheet's figures: the bank, the deposits, the loans, the
/// fortune, the credit limit, what may still be invested, the rates, the
/// overdraft, the season's start and result, and the forecasts.
const BANK: u16 = 4463;
const DEPOSITS: u16 = 4464;
const LOANS: u16 = 4465;
const FORTUNE: u16 = 4472;
const CREDIT_LIMIT: u16 = 4473;
const INVESTABLE: u16 = 4474;
const LOAN_RATE: u16 = 4469;
const OVERDRAFT_RATE: u16 = 4470;
const OVERDRAFT: u16 = 4471;
const SEASON_START: u16 = 4466;
const RESULT: u16 = 4467;
const AS_PLANNED: u16 = 4468;
const SO_FAR: u16 = 4475;
/// The legal form: club or AG, its shares, their price and what they are
/// worth.
const LEGAL_FORM: u16 = 5510;
const SHARES: u16 = 5511;
const SHARE_PRICE: u16 = 5512;
const MARKET_VALUE: u16 = 5513;
const AG_TEXT: u16 = 4590;
/// The buttons to the deposits and the loans.
const DEPOSITS_BUTTON: u16 = 4461;
const LOANS_BUTTON: u16 = 4462;
/// The button to the stock market, and why it refuses.
const AG_BUTTON: u16 = 6152;
const LEAGUE_REFUSAL: u16 = 4582;
const AMATEURS_REFUSAL: u16 = 4441;
const FORTUNE_REFUSAL: u16 = 4580;
const LOANS_REFUSAL: u16 = 4581;
const LOSSES_REFUSAL: u16 = 4579;
/// What the figures are written with: money below a thousand, below a
/// million, below a billion, and above; a share's price below a thousand
/// and below a million.
const MONEY_64: [u16; 4] = [2157, 2156, 2155, 4140];
const PRICES: [u16; 2] = [4461, 4460];
/// The overdraft is kept in units of this.
const OVERDRAFT_UNIT: i32 = 100_000;

/// The bar takes the OK's place; the caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    for id in [BALANCE, FINANCES] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        dialog.controls.retain(|c| c.id != Some(OK));
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
        hub::add_bar(dialog, rules);
    }
}

/// What both screens share: the bar, the caption — the finances' is not
/// the dialog file's —, the whistle.
fn open_screen(cx: &mut Cx, view: &mut View, symbol: usize) {
    let (game, dialog) = (cx.game, cx.dialog);
    hub::open_bar(cx, view, symbol);
    let caption =
        if symbol == FINANCES_SYMBOL { game.text(FINANCES_CAPTION) } else { dialog.title.as_str() };
    if let Some(season) = cx.state.season.as_ref() {
        set(view, dialog, CAPTION_ID, hub::manager_title_of(game, season, caption));
    }
    let works = |target: Target| hub::on_bar(dialog, target).is_some_and(hub::bar_works);
    mark_inert(view, dialog, works);
}

/// The balance sheet.
#[derive(Default)]
pub struct Balance {
    /// Whether the whistle asks.
    whistle: bool,
}

impl Screen for Balance {
    fn ids(&self) -> &[u16] {
        &[BALANCE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        open_screen(cx, view, BALANCE_SYMBOL);
        fill_balance(cx, view);
        let works = |target: Target| match target {
            Target::Button(c) => [AG_BUTTON, DEPOSITS_BUTTON, LOANS_BUTTON]
                .contains(&dialog.controls[c].id.unwrap_or(0)),
            _ => hub::on_bar(dialog, target).is_some_and(hub::bar_works),
        };
        mark_inert(view, dialog, works);
        // A company already: the button is off.
        let listed = own_team(game, cx).is_some_and(|(_, team)| team.legal_form() & 1 != 0);
        if let Some(button) = control(dialog, AG_BUTTON) {
            view.set_disabled(button, listed);
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Some(item) = hub::on_bar(cx.dialog, target) {
            let outcome = hub::bar_click(cx, view, item, BALANCE_SYMBOL);
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(AG_BUTTON) => {
                self.whistle = false;
                go_public(cx)
            }
            Target::Button(c) if cx.dialog.controls[c].id == Some(DEPOSITS_BUTTON) => {
                self.whistle = false;
                Outcome::Popup(Request::new(INVESTMENTS))
            }
            // The loans, while there is more than a little to invest
            // (`FUN_00481aeb`).
            Target::Button(c) if cx.dialog.controls[c].id == Some(LOANS_BUTTON) => {
                self.whistle = false;
                let investable = own_team(cx.game, cx).map_or(0, |(records, team)| {
                    Figures::of(team, records.globals.fund_prices(), &records.agreed()).investable()
                });
                let dialog = if investable > LEAST_LOAN { LOANS_DIALOG } else { NO_CREDIT };
                Outcome::Popup(Request::new(dialog))
            }
            _ => Outcome::Stay,
        }
    }

    /// The whistle's question: a yes ends the turn. Back from the
    /// deposits, the figures anew.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) {
            if reply == Reply::Ok {
                return Outcome::Close(Reply::Ok);
            }
            return Outcome::Stay;
        }
        fill_balance(cx, view);
        Outcome::Stay
    }
}

/// The finances: the season's profit and loss account, row by row, the
/// costs on the left, the income on the right, each beside a column to
/// compare with, and a bar graph of both between them.
#[derive(Default)]
pub struct Finances {
    /// Whether the whistle asks.
    whistle: bool,
    /// The row whose calculation is open (`FUN_004a43cb`).
    calculation: Option<usize>,
    /// How the figures are found ("Ermittlung der Zahlen"), and what they
    /// are compared with ("Darstellung"), as the combos' items have it.
    finding: usize,
    compared: usize,
    comparisons: Vec<usize>,
}

/// The combos: how the figures are found, what they are compared with,
/// and their items' texts.
const FINDING: u16 = 2226;
const COMPARED: u16 = 2227;
const FINDINGS: [u16; 4] = [2366, 2367, 2368, 2369];
const COMPARISONS: [u16; 3] = [2363, 2364, 2365];
/// The findings: carried on, forecast, as booked, this week's.
const CARRIED_ON: usize = 0;
const EXTRAPOLATED: usize = 1;
const CURRENT: usize = 2;
const THIS_WEEK: usize = 3;
/// The comparisons: none, last season, the plan.
const NO_COMPARISON: usize = 0;
const LAST_SEASON: usize = 1;
const PLAN: usize = 2;
/// The figures of the actual column and of the one compared with: the
/// costs of the rows, then their income (0 where a row has none).
const ACTUAL: [u16; 28] = [
    5080, 5081, 5082, 5083, 5084, 0, 5085, 5086, 5087, 5088, 5089, 5090, 5091, 5092, 5093, 5094,
    5095, 5096, 5097, 5098, 5099, 5100, 5101, 5102, 5103, 0, 5104, 5105,
];
const COMPARED_FIGURES: [u16; 28] = [
    5053, 5054, 5055, 5056, 5057, 0, 5059, 5060, 5061, 5062, 5063, 5064, 5065, 5066, 5067, 5068,
    5069, 5070, 5071, 5072, 5073, 5074, 5075, 5076, 5077, 0, 5078, 5079,
];
/// The compared column's headings, and what they read: nothing, "Vorjahr",
/// "Kalk".
const COMPARED_HEADINGS: [u16; 2] = [5780, 5782];
const LAST_SEASON_HEADING: u16 = 2396;
const PLAN_HEADING: u16 = 2395;
/// The result row's label: "Gewinn" or "Verlust".
const RESULT_LABEL: u16 = 1616;
const PROFIT: u16 = 3106;
const LOSS: u16 = 3295;
/// The rows, the result's last, and which slot each shows of its costs
/// and of its income (`FUN_004a2e36`); the other income's row shows two.
const ROWS: usize = 14;
const RESULT_ROW: usize = 13;
const COSTS: [Option<usize>; RESULT_ROW] = [
    None,
    None,
    None,
    None,
    Some(4),
    None,
    Some(6),
    Some(10),
    Some(11),
    Some(12),
    Some(8),
    Some(14),
    Some(17),
];
const INCOME: [Option<usize>; RESULT_ROW] = [
    Some(0),
    Some(1),
    Some(2),
    Some(3),
    Some(5),
    Some(15),
    Some(7),
    None,
    None,
    None,
    Some(9),
    None,
    Some(16),
];
const OTHER_INCOME_ROW: usize = 5;
const OTHER_INCOME_FIRST: usize = 13;
const OTHER_COSTS_ROW: usize = 11;
/// The graph's bars: two a row on each side, the costs' from the left's
/// first, the income's from the right's; a row's costs end at the
/// middle, its income starts there, as long as the largest figure makes
/// them (`FUN_004a3f3b`).
const LEFT_BARS: u16 = 5422;
const RIGHT_BARS: u16 = 5451;
const BAR_LENGTH: f64 = 66.0;
/// The bars' pictures, light for the actual figures, dark for those
/// compared with: costs red, income green — a profit green, a loss red.
const RED: u16 = 2628;
const DARK_RED: u16 = 2629;
const GREEN: u16 = 2625;
const DARK_GREEN: u16 = 2626;
/// The figures, without a currency: "1.234.567" (`FUN_004db67a`).
const AMOUNTS: [u16; 3] = [2500, 2499, 2445];
/// The rows' labels, buttons that open their calculations.
const ROW_BUTTONS: std::ops::RangeInclusive<u16> = 5767..=5779;

impl Screen for Finances {
    fn ids(&self) -> &[u16] {
        &[FINANCES]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        open_screen(cx, view, FINANCES_SYMBOL);
        let game = cx.game;
        let dialog = cx.dialog;
        self.calculation = None;
        let works = |target: Target| match target {
            Target::Button(c) => dialog.controls[c].id.is_some_and(|id| {
                ROW_BUTTONS.contains(&id)
                    && kalkulation::BUILT
                        .contains(&kalkulation::ROW_DIALOGS[usize::from(id - ROW_BUTTONS.start())])
            }),
            _ => hub::on_bar(dialog, target).is_some_and(hub::bar_works),
        };
        mark_inert(view, dialog, works);
        self.finding = CURRENT;
        self.compared = NO_COMPARISON;
        let has_last_season = own_extra(game, cx)
            .is_some_and(|(_, e)| a3_core::finances::Accounts::last_season(e).is_some());
        self.comparisons =
            (0..COMPARISONS.len()).filter(|&c| c != LAST_SEASON || has_last_season).collect();
        if let Some(combo) = control(dialog, FINDING).and_then(|c| view.combo_mut(c)) {
            combo.items = FINDINGS.iter().map(|&t| game.text(t).to_owned()).collect();
            combo.selected = Some(CURRENT);
        }
        if let Some(combo) = control(dialog, COMPARED).and_then(|c| view.combo_mut(c)) {
            combo.items =
                self.comparisons.iter().map(|&c| game.text(COMPARISONS[c]).to_owned()).collect();
            combo.selected = Some(0);
        }
        self.fill(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Some(item) = hub::on_bar(cx.dialog, target) {
            let outcome = hub::bar_click(cx, view, item, FINANCES_SYMBOL);
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        if let Target::Button(c) = target
            && let Some(id) = cx.dialog.controls[c].id.filter(|id| ROW_BUTTONS.contains(id))
        {
            let row = usize::from(id - ROW_BUTTONS.start());
            self.calculation = Some(row);
            return Outcome::Popup(Request::new(kalkulation::row_dialog(cx, row)));
        }
        // A combo's row picked: what it now shows.
        let Target::Row { control: c, .. } = target else { return Outcome::Stay };
        let Some(item) = view.combo(c).and_then(|combo| combo.selected) else {
            return Outcome::Stay;
        };
        match cx.dialog.controls[c].id {
            Some(FINDING) => self.finding = item,
            Some(COMPARED) => {
                self.compared = self.comparisons.get(item).copied().unwrap_or(NO_COMPARISON);
            }
            _ => return Outcome::Stay,
        }
        self.fill(cx, view);
        Outcome::Stay
    }

    /// The whistle's question: a yes ends the turn.
    /// A calculation's arrow bar pages on to the next row's or back to
    /// the last's; its OK or cancel brings the figures anew, as the
    /// calculations may have booked.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if let Some(row) = self.calculation.take() {
            if let Reply::Choice(choice @ (kalkulation::NEXT | kalkulation::PREVIOUS)) = reply
                && let Some(next) = kalkulation::step(cx, row, choice)
            {
                self.calculation = Some(next);
                return Outcome::Popup(Request::new(kalkulation::row_dialog(cx, next)));
            }
            self.fill(cx, view);
            return Outcome::Stay;
        }
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}

impl Finances {
    /// The figures, the headings and the graph (`FUN_004a20a7`,
    /// `FUN_004a1fbe`).
    fn fill(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some((records, team)) = own_team(game, cx) else { return };
        let manager = records.human_slot(cx.state.season.as_ref().map_or(0, |s| s.current));
        let manager = manager.and_then(|slot| records.managers.get(slot));
        let forecast = own_forecast(game, cx);
        let actual = match (self.finding, manager, &forecast) {
            (THIS_WEEK, Some(manager), _) => finances::this_week(team, manager),
            (CARRIED_ON, _, Some(forecast)) => forecast.carried_on(),
            (EXTRAPOLATED, _, Some(forecast)) => forecast.extrapolated(),
            _ => finances::current(team),
        };
        let compared = match self.compared {
            LAST_SEASON => own_extra(game, cx)
                .and_then(|(_, extra)| finances::last_season(team, extra, &actual)),
            PLAN => forecast.as_ref().map(|f| finances::planned(team, f.planned_tickets())),
            _ => None,
        };
        // Four columns of the graph: the compared costs and income, the
        // actual costs and income.
        let mut graph = [[0.0; ROWS]; 4];
        for id in ACTUAL.iter().chain(&COMPARED_FIGURES).filter(|&&id| id != 0) {
            set(view, dialog, *id, String::new());
        }
        column(game, view, dialog, &actual, &ACTUAL, &mut graph[2..4]);
        if let Some(compared) = &compared {
            column(game, view, dialog, compared, &COMPARED_FIGURES, &mut graph[0..2]);
        }
        let t = finances::total(&actual);
        set(view, dialog, RESULT_LABEL, game.text(if t >= 0 { PROFIT } else { LOSS }).to_owned());
        let heading = match self.compared {
            NO_COMPARISON => String::new(),
            LAST_SEASON => game.text(LAST_SEASON_HEADING).to_owned(),
            _ => game.text(PLAN_HEADING).to_owned(),
        };
        for id in COMPARED_HEADINGS {
            set(view, dialog, id, heading.clone());
        }
        bars(view, dialog, &graph);
    }
}

/// A column of figures, its costs and its income, into the texts and the
/// graph's two columns (`FUN_004a2e36`): the result, a profit with the
/// costs, a loss with the income.
fn column(
    game: &Game,
    view: &mut View,
    dialog: &Dialog,
    s: &[i64; finances::SLOTS],
    ids: &[u16; 28],
    graph: &mut [[f64; ROWS]],
) {
    for row in 0..RESULT_ROW {
        if let Some(slot) = COSTS[row] {
            set(view, dialog, ids[row], amount(game, s[slot]));
            graph[0][row] = s[slot] as f64;
        }
        if let Some(slot) = INCOME[row] {
            let shown = if row == OTHER_INCOME_ROW {
                s[OTHER_INCOME_FIRST].wrapping_add(s[slot])
            } else {
                s[slot]
            };
            set(view, dialog, ids[ROWS + row], amount(game, shown));
            graph[1][row] = s[slot] as f64;
        }
    }
    let t = finances::total(s);
    let (cost, income) = if t >= 0 { (t, None) } else { (0, Some(t.wrapping_neg())) };
    set(
        view,
        dialog,
        ids[RESULT_ROW],
        if income.is_none() { amount(game, cost) } else { String::new() },
    );
    set(view, dialog, ids[ROWS + RESULT_ROW], income.map_or_else(String::new, |v| amount(game, v)));
    graph[0][RESULT_ROW] = cost as f64;
    graph[1][RESULT_ROW] = income.unwrap_or(0) as f64;
}

/// The bars, each as long against the largest figure as its own
/// (`FUN_004a3f3b`): the costs' grow leftwards from the middle, the
/// income's rightwards.
fn bars(view: &mut View, dialog: &Dialog, graph: &[[f64; ROWS]; 4]) {
    let most = graph.iter().flatten().fold(0.0f64, |m, &v| m.max(v));
    let length = |v: f64| if most == 0.0 { 0 } else { (v / most * BAR_LENGTH) as i32 };
    let [compared_costs, compared_income, costs, income] = graph;
    for (row, ((&cost, &income), (&compared_cost, &compared_income))) in
        costs.iter().zip(income).zip(compared_costs.iter().zip(compared_income)).enumerate()
    {
        let result = row == RESULT_ROW;
        let (red, dark_red, green, dark_green) = if result {
            (GREEN, DARK_GREEN, RED, DARK_RED)
        } else {
            (RED, DARK_RED, GREEN, DARK_GREEN)
        };
        let row_ = row as u16;
        let bars = [
            (LEFT_BARS + 2 * row_, red, cost, true),
            (LEFT_BARS + 2 * row_ + 1, dark_red, compared_cost, true),
            (RIGHT_BARS + 2 * row_, dark_green, compared_income, false),
            (RIGHT_BARS + 2 * row_ + 1, green, income, false),
        ];
        for (id, picture, v, left) in bars {
            if (left && row == OTHER_INCOME_ROW) || (!left && row == OTHER_COSTS_ROW) {
                continue;
            }
            let Some(c) = control(dialog, id) else { continue };
            let r = dialog.controls[c].rect;
            let width = if left {
                // The left edge moves; the right one stays.
                let gap = if most == 0.0 {
                    BAR_LENGTH as i32
                } else {
                    (BAR_LENGTH - v / most * BAR_LENGTH) as i32
                };
                r.width() as i32 - gap
            } else {
                length(v)
            };
            let Some(bitmap) = view.bitmap_mut(c) else { continue };
            bitmap.left = left.then_some(r.max.x - width as f32);
            bitmap.picture = (width > 0).then(|| Picture {
                part: Some([0, 0, width as u32, r.height() as u32]),
                ..Picture::graphic(picture)
            });
        }
    }
}

/// A figure of the finances (`FUN_004db67a`): "1.234.567", "- 982.535".
pub(crate) fn amount(game: &Game, v: i64) -> String {
    let sign = if v < 0 { '-' } else { ' ' };
    let v = v.unsigned_abs();
    let written = match v {
        0..1_000 => printf(game.text(AMOUNTS[0]), &[&sign, &v]),
        1_000..1_000_000 => printf(game.text(AMOUNTS[1]), &[&sign, &(v / 1000), &(v % 1000)]),
        _ => printf(
            game.text(AMOUNTS[2]),
            &[&sign, &((v / 1_000_000) as u32), &(v % 1_000_000 / 1000), &(v % 1000)],
        ),
    };
    if sign == ' ' { written.chars().skip(2).collect() } else { written }
}

/// The manager's club's extra data, and its slot.
pub(crate) fn own_extra<'a>(
    game: &Game,
    cx: &'a Cx,
) -> Option<(usize, &'a a3_core::records::Extra)> {
    let (records, team) = own_team(game, cx)?;
    let slot = usize::from(team.human()).checked_sub(1)?;
    Some((slot, records.extras.get(slot)?))
}

/// The stock market: the first check the club fails, told; the rest is
/// not built (BILANZ.md §4.3).
fn go_public(cx: &Cx) -> Outcome {
    let game = cx.game;
    let Some((records, team)) = own_team(game, cx) else { return Outcome::Stay };
    let figures = Figures::of(team, records.globals.fund_prices(), &records.agreed());
    let args = match figures.refusal(team) {
        Some(Refusal::League) => Args::Text(LEAGUE_REFUSAL),
        Some(Refusal::Amateurs) => Args::Text(AMATEURS_REFUSAL),
        Some(Refusal::Fortune(least)) => Args::TextForMoney(FORTUNE_REFUSAL, least),
        Some(Refusal::Loans) => Args::Text(LOANS_REFUSAL),
        Some(Refusal::Losses) => Args::Text(LOSSES_REFUSAL),
        None => {
            info!("Börsengang: not built yet");
            return Outcome::Stay;
        }
    };
    Outcome::Popup(Request::with(NOTICE, args))
}

/// The manager's club's team record.
fn own_team<'a>(game: &Game, cx: &'a Cx) -> Option<(&'a Records, &'a Team)> {
    let (records, index) = own_index(game, cx)?;
    Some((records, records.teams.get(index)?))
}

/// The manager's club's team's index.
pub(crate) fn own_index<'a>(game: &Game, cx: &'a Cx) -> Option<(&'a Records, usize)> {
    let (slot, number) = cx.state.season.as_ref()?.manager()?.club?;
    let records = cx.state.records.as_ref()?;
    let nation = game.world.country(slot)?.nation;
    Some((records, records.team_index(nation, u16::try_from(number).ok()?)?))
}

/// The forecast of the manager's club.
fn own_forecast<'a>(game: &'a Game, cx: &'a Cx) -> Option<Forecast<'a>> {
    let (records, index) = own_index(game, cx)?;
    Forecast::new(records, index, &game.rules.forecast, &game.calendar)
}

/// Every figure of the balance sheet (`FUN_004813ad`).
fn fill_balance(cx: &Cx, view: &mut View) {
    let (game, dialog) = (cx.game, cx.dialog);
    let Some((records, team)) = own_team(game, cx) else { return };
    let figures = Figures::of(team, records.globals.fund_prices(), &records.agreed());
    let money64 = |v: i64| money64(game, v);
    set(view, dialog, LOAN_RATE, percent(records.globals.loan_rate()));
    set(view, dialog, FORTUNE, money64(figures.fortune()));
    set(view, dialog, SEASON_START, money64(figures.cash));
    set(view, dialog, CREDIT_LIMIT, money(game, figures.credit_limit().into()));
    set(view, dialog, LOANS, money64(figures.loans));
    set(view, dialog, RESULT, money64(figures.result));
    let overdraft = i32::from(team.overdraft()).wrapping_mul(OVERDRAFT_UNIT);
    set(view, dialog, OVERDRAFT, format!("({})", money(game, overdraft.into())));
    set(view, dialog, OVERDRAFT_RATE, percent(records.globals.overdraft_rate()));
    set(view, dialog, BANK, money64(figures.bank()));
    set(view, dialog, DEPOSITS, money64(figures.deposits));
    set(view, dialog, INVESTABLE, money64(figures.investable()));
    if let Some(forecast) = own_forecast(game, cx) {
        set(view, dialog, AS_PLANNED, money64(forecast.as_planned()));
        set(view, dialog, SO_FAR, money64(forecast.so_far()));
    }
    let none = || "---".to_owned();
    let listing = (team.legal_form() & 1 != 0)
        .then(|| records.listing(team.country(), team.number()))
        .flatten();
    match listing {
        Some(listing) => {
            set(view, dialog, LEGAL_FORM, game.text(AG_TEXT).to_owned());
            set(view, dialog, SHARES, count(listing.shares));
            let price = f64::from(listing.price);
            set(view, dialog, SHARE_PRICE, share_price(game, (price * 100.0) as i32));
            set(view, dialog, MARKET_VALUE, money64((price * f64::from(listing.shares)) as i64));
        }
        None => {
            for id in [SHARES, SHARE_PRICE, MARKET_VALUE] {
                set(view, dialog, id, none());
            }
        }
    }
}

/// A C format filled in: `%c`, `%s`, `%d` with a width and zeros, `%%`.
pub(crate) fn printf(format: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut args = args.iter();
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut spec = String::new();
        while let Some(&d) = chars.peek().filter(|d| d.is_ascii_digit()) {
            spec.push(d);
            chars.next();
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some(_) => {
                let arg = args.next().map_or(String::new(), |a| a.to_string());
                let width = spec.parse::<usize>().unwrap_or(0);
                if spec.starts_with('0') {
                    out.push_str(&format!("{arg:0>width$}"));
                } else {
                    out.push_str(&format!("{arg:>width$}"));
                }
            }
            None => {}
        }
    }
    out
}

/// Money in 64 bits as the game writes it (`FUN_004db27e`): "5.92 Mio €",
/// "- 13.05 Mio €", " 0 €", digits cut off rather than rounded.
pub(crate) fn money64(game: &Game, v: i64) -> String {
    let sign = if v < 0 { '-' } else { ' ' };
    let v = v.unsigned_abs();
    let text = |i: usize| game.text(MONEY_64[i]);
    let written = match v {
        0..1_000 => printf(text(0), &[&sign, &v]),
        1_000..1_000_000 => printf(text(1), &[&sign, &(v / 1000), &(v % 1000)]),
        1_000_000..1_000_000_000 => {
            printf(text(2), &[&sign, &(v / 1_000_000), &(v % 1_000_000 / 10_000)])
        }
        _ => printf(text(3), &[&sign, &(v / 1_000_000_000), &(v % 1_000_000_000 / 1_000_000)]),
    };
    // Without a sign the text starts after it.
    if sign == ' ' { written.chars().skip(2).collect() } else { written }
}

/// A rate in hundredths of a percent: "13%", "10.50%" (`FUN_00481a30`).
fn percent(v: i16) -> String {
    let (whole, part) = (v / 100, v % 100);
    if part != 0 { format!("{whole}.{part:02}%") } else { format!("{whole}%") }
}

/// A count with points between the thousands: "7.974.620" (`FUN_004dacb9`).
pub(crate) fn count(n: i32) -> String {
    let sign = if n < 0 { '-' } else { ' ' };
    let mut n = n.unsigned_abs();
    let mut rest = String::new();
    while n >= 1000 {
        rest = format!(".{:03}{rest}", n % 1000);
        n /= 1000;
    }
    let written = format!("{sign} {n}{rest}");
    if sign == ' ' { written[2..].to_owned() } else { written }
}

/// A share's price from its cents: " 261,94 €", " 358,-- €"
/// (`FUN_004db9fb`).
fn share_price(game: &Game, cents: i32) -> String {
    let fraction = match cents % 100 {
        0 => "--".to_owned(),
        c => format!("{c:02}"),
    };
    let euros = cents / 100;
    match euros {
        ..1_000 => printf(game.text(PRICES[0]), &[&(euros % 1000), &fraction]),
        1_000..1_000_000 => {
            printf(game.text(PRICES[1]), &[&(euros % 1_000_000 / 1000), &(euros % 1000), &fraction])
        }
        _ => format!(
            "{}.{:03}.{:03}{fraction}",
            euros / 1_000_000,
            euros % 1_000_000 / 1000,
            euros % 1000
        ),
    }
}
