//! The bank behind the balance sheet (BILANZ.md §4 in the RE repo): the
//! deposits (dialog 16, `CAnlagen`) — an amount set by the slider, put with
//! one of five partners, the deposits listed with what they cost and are
//! worth, and dissolved again —, the loans (335, `CWieLangeKredite`) —
//! an amount, a term and whether it may be paid back early, the loans
//! listed — and paying one back (138, `CKreditZurueckzahlen`), or the
//! bank's refusal to lend more (24, `CKeinKreditMehr`).

use std::collections::HashMap;

use a3_core::finances::{self, Balance as Figures, LEAST_DEPOSIT, STEP_UNIT};
use a3_files::rch::Dialog;

use super::finances::{money64, own_index};
use super::{Args, Cx, Outcome, Reply, Screen, control, set};
use crate::pointer::Target;
use crate::view::View;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::NOTICE;
pub use a3_core::cycle::dialogs::{INVESTMENTS, LOANS, NO_CREDIT, REPAYMENT};
use a3_core::finances::{Credit, LEAST_LOAN, LOAN_YEARS, Loan};

use super::playerinfo::money;

/// The amount, the bank's money, what may still be invested, what all
/// deposits cost.
const AMOUNT: u16 = 1267;
const BANK: u16 = 1268;
const INVESTABLE: u16 = 1269;
const TOTAL: u16 = 1275;
const SLIDER: u16 = 1278;
const INVEST: u16 = 1276;
const DISSOLVE: u16 = 1277;
const LIST: u16 = 1266;
const PARTNERS: u16 = 6050;
const OK: u16 = 1;
const CAPTION_ID: u16 = 9934;
/// The partners' kinds of deposit, by partner (`DAT_008791c0`).
const KINDS: [u16; 5] = [2804, 2805, 2806, 2807, 2808];
/// A club the bank trusts less may only put call money; the partners it
/// may not use.
const ONLY_CALL_MONEY: u8 = 0x80;
const RISKY_PARTNERS: [usize; 3] = [2, 3, 4];
/// The amount a deposit starts at, in the slider's steps.
const FIRST_STEP: i64 = LEAST_DEPOSIT / STEP_UNIT;

/// The dialogs' titles in their captions.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [INVESTMENTS, LOANS, REPAYMENT, NO_CREDIT] {
        if let Some(dialog) = dialogs.get_mut(&id) {
            let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
            dialog.controls.insert(0, caption);
        }
    }
}

/// The deposits.
#[derive(Default)]
pub struct Investments {
    /// The amount, the partner chosen, the slot the next deposit takes.
    amount: i64,
    partner: usize,
    slot: Option<usize>,
}

impl Screen for Investments {
    fn ids(&self) -> &[u16] {
        &[INVESTMENTS]
    }

    /// The call money's rate in its partner's name; the risky partners off
    /// for a club the bank trusts less (`FUN_00443c25`).
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        self.partner = 0;
        let Some((records, index)) = own_index(cx.game, cx) else { return };
        let rate = records.globals.deposit_rate();
        let rate = format!("{}.{:02}", rate / 100, rate % 100);
        let trusted = records.teams[index].standing() & ONLY_CALL_MONEY == 0;
        if let Some(options) = control(dialog, PARTNERS).and_then(|c| view.options_mut(c)) {
            if let Some(first) = options.labels.first_mut() {
                *first = first.replacen("%s", &rate, 1).replace("%%", "%");
            }
            if !trusted {
                options.off.resize(options.labels.len(), false);
                RISKY_PARTNERS.iter().for_each(|&p| options.off[p] = true);
            }
        }
        self.refresh(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if id(c) == Some(OK) => return Outcome::Close(Reply::Ok),
            Target::Button(c) if id(c) == Some(INVEST) => self.invest(cx, view),
            Target::Button(c) if id(c) == Some(DISSOLVE) => self.dissolve(cx, view),
            Target::Option { control: c, item } if id(c) == Some(PARTNERS) => self.partner = item,
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. }
                if id(c) == Some(SLIDER) =>
            {
                let position = view.value(c).map_or(0, |v| v.value);
                self.amount = finances::step_amount(position) * STEP_UNIT;
                set(view, dialog, AMOUNT, money64(cx.game, self.amount));
            }
            _ => {}
        }
        Outcome::Stay
    }
}

impl Investments {
    /// The list, the figures, the slider and the buttons as they now
    /// stand (`FUN_00443d73`, `FUN_00444007`).
    fn refresh(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some((records, index)) = own_index(game, cx) else { return };
        let team = &records.teams[index];
        let prices = records.globals.fund_prices();
        let deposits = finances::deposits_of(team, prices);
        if let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) {
            list.rows = deposits
                .iter()
                .map(|d| {
                    let kind = game.text(KINDS[usize::from(d.partner).min(KINDS.len() - 1)]);
                    let (cost, value) = (money64(game, d.cost), money64(game, d.value));
                    format!("{}\t{kind}\t{cost}\t{value}", d.slot + 1)
                })
                .collect();
            list.keys = deposits.iter().map(|d| d.slot).collect();
            list.selected = None;
            list.scroll = 0;
        }
        let total = deposits.iter().fold(0i64, |s, d| s.wrapping_add(d.cost));
        set(view, dialog, TOTAL, money64(game, total));
        if let Some(button) = control(dialog, DISSOLVE) {
            view.set_disabled(button, deposits.is_empty());
        }
        let figures = Figures::of(team, prices, &records.agreed());
        let available = finances::available(team, &figures);
        let slider = control(dialog, SLIDER).and_then(|c| view.value_mut(c));
        if available < LEAST_DEPOSIT {
            self.amount = 0;
            self.slot = None;
            if let Some(slider) = slider {
                slider.range = (0, 1);
                slider.value = 0;
            }
        } else {
            self.amount = LEAST_DEPOSIT;
            self.slot = finances::free_deposit(team);
            if let Some(slider) = slider {
                slider.range = (
                    finances::step_position(FIRST_STEP),
                    finances::step_position(available / STEP_UNIT),
                );
                slider.value = finances::step_position(FIRST_STEP);
            }
        }
        set(view, dialog, AMOUNT, money64(game, self.amount));
        set(view, dialog, BANK, money64(game, figures.bank()));
        set(view, dialog, INVESTABLE, money64(game, figures.investable()));
        if let Some(button) = control(dialog, INVEST) {
            view.set_disabled(button, self.slot.is_none());
        }
        if let Some(c) = control(dialog, PARTNERS) {
            view.set_chosen(c, self.partner);
        }
    }

    /// The amount put with the partner in the free slot
    /// (`OnClicked_IDC_ANLAGEN_ANLEGEN_00444359`).
    fn invest(&mut self, cx: &mut Cx, view: &mut View) {
        let Some(slot) = self.slot else { return };
        let Some((_, index)) = own_index(cx.game, cx) else { return };
        let Some(records) = cx.state.records.as_mut() else { return };
        let prices = records.globals.fund_prices();
        let [day, month, _] = records.globals.date();
        let day = finances::bank_day(day, month);
        if let Some(team) = records.teams.get_mut(index) {
            finances::invest(team, slot, self.amount, self.partner as u8, prices, day);
        }
        self.refresh(cx, view);
    }

    /// The deposit of the row chosen dissolved
    /// (`OnClicked_IDC_ANLAGEN_AUFLOESEN_0044440f`).
    fn dissolve(&mut self, cx: &mut Cx, view: &mut View) {
        let chosen = control(cx.dialog, LIST)
            .and_then(|c| view.list(c))
            .and_then(|l| Some(*l.keys.get(l.selected?)?));
        let Some(slot) = chosen else { return };
        let Some((_, index)) = own_index(cx.game, cx) else { return };
        let Some(records) = cx.state.records.as_mut() else { return };
        let prices = records.globals.fund_prices();
        if let Some(team) = records.teams.get_mut(index) {
            let deposit = finances::deposits_of(team, prices).into_iter().find(|d| d.slot == slot);
            if let Some(deposit) = deposit {
                finances::dissolve(team, &deposit);
            }
        }
        self.refresh(cx, view);
    }
}

/// The loan dialog's amount, the bank's money, what it would still lend,
/// what all loans come to; the slider, its buttons, the list, the rates.
const LOAN_AMOUNT: u16 = 4351;
const LOAN_BANK: u16 = 4352;
const LOAN_FRAME: u16 = 4353;
const LOAN_TOTAL: u16 = 4360;
const LOAN_SLIDER: u16 = 4363;
const BORROW: u16 = 4361;
const PAY_BACK: u16 = 4362;
const LOAN_LIST: u16 = 4299;
const RATES: u16 = 6061;
/// The options of the rates: three terms repayable, three not.
const TERMS: usize = 3;
/// "Ja", "Nein": whether a loan may be paid back early.
const YES: u16 = 1028;
const NO: u16 = 1029;
/// A row's key for a loan that may not be paid back early, and what the
/// bank says to that.
const FIXED: usize = 100;
const FIXED_LOAN: u16 = 3235;

/// The loans.
#[derive(Default)]
pub struct Loans {
    /// The amount, the term and whether it may be paid back early, the
    /// slot the next loan takes, whether the bank lends no more.
    amount: i64,
    term: usize,
    repayable: bool,
    slot: Option<usize>,
    closed: bool,
}

impl Screen for Loans {
    fn ids(&self) -> &[u16] {
        &[LOANS]
    }

    /// The rates in their options, the first chosen (`FUN_005d77a6`).
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        self.amount = 0;
        self.term = 0;
        self.repayable = true;
        let Some((records, _)) = own_index(cx.game, cx) else { return };
        let rates = finances::credit_rates(records.globals.credit_rate());
        if let Some(c) = control(dialog, RATES) {
            if let Some(options) = view.options_mut(c) {
                for (label, rate) in options.labels.iter_mut().zip(rates) {
                    *label = format!("{}.{:02}", rate / 100, rate % 100);
                }
            }
            view.set_chosen(c, 0);
        }
        self.refresh(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if id(c) == Some(OK) => return Outcome::Close(Reply::Ok),
            Target::Button(c) if id(c) == Some(BORROW) => self.borrow(cx, view),
            Target::Button(c) if id(c) == Some(PAY_BACK) => return self.pay_back(cx, view),
            Target::Option { control: c, item } if id(c) == Some(RATES) => {
                self.term = item % TERMS;
                self.repayable = item < TERMS;
            }
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. }
                if id(c) == Some(LOAN_SLIDER) =>
            {
                let position = view.value(c).map_or(0, |v| v.value);
                self.amount = finances::step_amount(position) * STEP_UNIT;
                set(view, dialog, LOAN_AMOUNT, money64(cx.game, self.amount));
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// Back from paying back or from the bank's word: the loans anew.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, _: Reply) -> Outcome {
        self.refresh(cx, view);
        Outcome::Stay
    }
}

impl Loans {
    /// The list, the figures, the slider and the buttons as they now stand
    /// (`FUN_005d791d`, `FUN_005d81c9`); the amount stays, at most what the
    /// bank would lend.
    fn refresh(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some((records, index)) = own_index(game, cx) else { return };
        let team = &records.teams[index];
        let loans = finances::loans_of(team);
        let [_, month, year] = records.globals.date();
        let start_year = records.globals.start_year();
        let mut repayable = 0;
        let mut keys = Vec::new();
        let rows = loans
            .iter()
            .map(|loan| {
                let number = repayable + 1;
                if loan.repayable {
                    repayable += 1;
                    keys.push(loan.slot + 1);
                } else {
                    keys.push(FIXED);
                }
                let rate = f64::from(loan.rate()) / 100.0;
                let early = game.text(if loan.repayable { YES } else { NO });
                let years = LOAN_YEARS[usize::from(loan.term).min(2)];
                let left = loan.months_left(start_year, month, year);
                let principal = money64(game, loan.principal);
                format!("{number}\t{principal}\t{rate:.2}\t{early}\t{years}\t{left}")
            })
            .collect();
        if let Some(list) = control(dialog, LOAN_LIST).and_then(|c| view.list_mut(c)) {
            list.rows = rows;
            list.keys = keys;
            list.selected = None;
            list.scroll = 0;
        }
        let total = loans.iter().fold(0i64, |s, l| s.wrapping_add(l.principal));
        set(view, dialog, LOAN_TOTAL, money64(game, total));
        if let Some(button) = control(dialog, PAY_BACK) {
            view.set_disabled(button, repayable == 0);
        }
        let figures = Figures::of(team, records.globals.fund_prices(), &records.agreed());
        let frame = figures.credit_frame();
        set(view, dialog, LOAN_BANK, money64(game, figures.bank()));
        set(view, dialog, LOAN_FRAME, money64(game, frame));
        self.closed = frame < LEAST_LOAN;
        if self.closed {
            self.amount = 0;
            set(view, dialog, LOAN_AMOUNT, money(game, 0));
        } else {
            self.amount = self.amount.min(frame);
            set(view, dialog, LOAN_AMOUNT, money64(game, self.amount));
            let position = |amount: i64| {
                finances::step_position(i64::from((amount as i32) / STEP_UNIT as i32))
            };
            if let Some(slider) = control(dialog, LOAN_SLIDER).and_then(|c| view.value_mut(c)) {
                slider.range = (1, position(frame));
                slider.set(position(self.amount));
            }
        }
        self.slot = finances::free_loan(team);
        if let Some(button) = control(dialog, BORROW) {
            view.set_disabled(button, self.slot.is_none() || self.closed);
        }
    }

    /// The amount borrowed for the term chosen
    /// (`OnClicked_IDC_KREDITE_AUFNEHMEN_005d7fb3`).
    fn borrow(&mut self, cx: &mut Cx, view: &mut View) {
        let Some(slot) = self.slot.filter(|_| self.amount != 0) else { return };
        let Some((_, index)) = own_index(cx.game, cx) else { return };
        let Some(records) = cx.state.records.as_mut() else { return };
        let rates = finances::credit_rates(records.globals.credit_rate());
        let [day, month, _] = records.globals.date();
        let (bank_day, today) = (finances::bank_day(day, month), records.globals.day());
        if let Some(team) = records.teams.get_mut(index) {
            let credit =
                Credit { amount: self.amount, term: self.term as u8, repayable: self.repayable };
            finances::borrow(team, slot, credit, rates, bank_day, today);
        }
        self.refresh(cx, view);
    }

    /// The loan of the row chosen paid back, if it may be
    /// (`OnClicked_IDC_KREDITE_ZURUECKZAHLEN_005d8284`).
    fn pay_back(&mut self, cx: &Cx, view: &View) -> Outcome {
        let chosen = control(cx.dialog, LOAN_LIST)
            .and_then(|c| view.list(c))
            .and_then(|l| Some(*l.keys.get(l.selected?)?));
        match chosen {
            None => Outcome::Stay,
            Some(key) if !(1..=8).contains(&key) => {
                Outcome::Popup(Request::with(NOTICE, Args::Text(FIXED_LOAN)))
            }
            Some(key) => Outcome::Popup(Request::with(REPAYMENT, Args::Loan(key - 1))),
        }
    }
}

/// The repayment's amount, what is owed, the term, the rate, what paying
/// early costs more and its label, the bank's money, what may still be
/// invested; the slider and the button.
const REPAY_AMOUNT: u16 = 2447;
const OWED: u16 = 2448;
const TERM: u16 = 2449;
const RATE: u16 = 2450;
const EXTRA_LABEL: u16 = 2451;
const EXTRA: u16 = 2452;
const REPAY_BANK: u16 = 2453;
const REPAY_INVESTABLE: u16 = 2454;
const REPAY_SLIDER: u16 = 2455;
const REPAY: u16 = 2457;
/// "Einsparung": paying back early saves.
const SAVING: u16 = 1093;

/// A loan paid back.
#[derive(Default)]
pub struct Repayment {
    loan: Option<Loan>,
    amount: i32,
}

impl Screen for Repayment {
    fn ids(&self) -> &[u16] {
        &[REPAYMENT]
    }

    /// All of it to pay at first (`FUN_004fb7b9`).
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        let Args::Loan(slot) = args else { return };
        let Some((records, index)) = own_index(game, cx) else { return };
        let team = &records.teams[index];
        let Some(loan) = finances::loans_of(team).into_iter().find(|l| l.slot == slot) else {
            return;
        };
        self.loan = Some(loan);
        self.amount = loan.principal as i32;
        set(view, dialog, REPAY_AMOUNT, money(game, self.amount.into()));
        set(view, dialog, OWED, money64(game, loan.principal));
        let figures = Figures::of(team, records.globals.fund_prices(), &records.agreed());
        set(view, dialog, REPAY_BANK, money64(game, figures.bank()));
        set(view, dialog, REPAY_INVESTABLE, money64(game, figures.investable()));
        set(view, dialog, TERM, LOAN_YEARS[usize::from(loan.term).min(2)].to_string());
        let rate = loan.rate();
        set(view, dialog, RATE, format!("{}.{:02}%", rate / 100, rate % 100));
        if self.extra(cx, view) < 0 {
            set(view, dialog, EXTRA_LABEL, game.text(SAVING).to_owned());
        }
        if let Some(slider) = control(dialog, REPAY_SLIDER).and_then(|c| view.value_mut(c)) {
            let steps = self.amount / STEP_UNIT as i32;
            slider.range = (0, steps);
            slider.set(steps);
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if id(c) == Some(OK) => return Outcome::Close(Reply::Ok),
            Target::Button(c) if id(c) == Some(REPAY) => {
                self.repay(cx);
                return Outcome::Close(Reply::Ok);
            }
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. }
                if id(c) == Some(REPAY_SLIDER) =>
            {
                let position = view.value(c).map_or(0, |v| v.value);
                self.amount = position.wrapping_mul(STEP_UNIT as i32);
                set(view, dialog, REPAY_AMOUNT, money(cx.game, self.amount.into()));
                self.extra(cx, view);
            }
            _ => {}
        }
        Outcome::Stay
    }
}

impl Repayment {
    fn bank_day(cx: &Cx) -> i16 {
        cx.state.records.as_ref().map_or(0, |r| {
            let [day, month, _] = r.globals.date();
            finances::bank_day(day, month)
        })
    }

    /// What paying back early costs more, or saves (`FUN_004fbb79`).
    fn extra(&self, cx: &Cx, view: &mut View) -> i32 {
        let Some(loan) = self.loan else { return 0 };
        let extra = loan.early_cost(self.amount, Self::bank_day(cx));
        set(view, cx.dialog, EXTRA, money(cx.game, i64::from(extra).abs()));
        extra
    }

    /// The amount paid back (`OnClicked_IDC_KREDITZURUECK_ZAHLEN_004fbc2c`).
    fn repay(&self, cx: &mut Cx) {
        let Some(loan) = self.loan else { return };
        let bank_day = Self::bank_day(cx);
        let Some((_, index)) = own_index(cx.game, cx) else { return };
        if let Some(team) = cx.state.records.as_mut().and_then(|r| r.teams.get_mut(index)) {
            finances::repay(team, &loan, self.amount, bank_day);
        }
    }
}

/// The bank's refusal to lend more.
#[derive(Default)]
pub struct NoCredit;

impl Screen for NoCredit {
    fn ids(&self) -> &[u16] {
        &[NO_CREDIT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        set(view, cx.dialog, CAPTION_ID, cx.dialog.title.clone());
        super::popups::ball(view, cx);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(OK) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}
