//! What a club's finances come to (`FUN_00621eac`, the RE repo's
//! FINANZEN-S.md): its cash and share proceeds, what it has booked this
//! season as the finance screen's columns count it, less its loans, plus
//! what its deposits are worth now. The game bands it for the opponent's
//! info and weighs it in offers and the board's verdict.

use crate::records::{Agreed, Extra, Manager, Team};

/// The club object's fields sit this far before the team record's.
pub(crate) const CLUB_TO_TEAM: usize = 0x278;
/// Where the team's and the extra data's records start counting.
const TEAM_BASE: usize = 0x20;
const EXTRA_BASE: usize = 0x20;
/// What the statistics and the computer department cost by level (the
/// tables at 0x89e450 and 0x89e458), and in what units.
const STATISTICS: [i64; 4] = [0, 2, 20, 100];
const COMPUTERS: [i64; 4] = [0, 2, 10, 30];
const STATISTICS_UNIT: i64 = 10_000;
const COMPUTERS_UNIT: i64 = 100_000;
/// Loans and deposits: how many, how long each record is.
const LOANS: usize = 8;
const DEPOSITS: usize = 6;
const ENTRY: usize = 0x18;

/// What the season has booked so far, the transfers given (`FUN_00621f50`,
/// `FUN_0061fc81`), with the game's quirks: one income counted twice, some
/// read unsigned, 32-bit sums wrapping.
fn booked(team: &Team, transfers: i64) -> i64 {
    let at = |c: usize| c + CLUB_TO_TEAM;
    let i32_ = |c: usize| i64::from(team.get::<i32>(at(c)));
    let u32_ = |c: usize| i64::from(team.get::<u32>(at(c)));
    let i64_ = |c: usize| team.get::<i64>(at(c));
    // A sum of 32-bit fields as the original adds them: wrapping in 32 bits.
    let r32 = |parts: &[(usize, i32)]| -> i64 {
        let sum = parts.iter().fold(0i32, |sum, &(c, sign)| {
            sum.wrapping_add(team.get::<i32>(at(c)).wrapping_mul(sign))
        });
        i64::from(sum)
    };
    let plus = |c: usize| (c, 1);
    let level = |c: usize, table: &[i64; 4]| {
        let l = team.get::<i8>(at(c));
        usize::try_from(l).ok().and_then(|l| table.get(l)).copied().unwrap_or(0)
    };

    let mut f: i64 = 0;
    f = f.wrapping_add(r32(&[plus(0x230), plus(0x234), plus(0x238), plus(0x23c)])); // tickets
    f = f.wrapping_add(r32(&[plus(0x240), plus(0x244)])); // TV
    f = f.wrapping_add(r32(&[plus(0x248), plus(0x24c), plus(0x250)])); // prizes
    f = f.wrapping_add(u32_(0x2f4)).wrapping_add(u32_(0x2f8)).wrapping_add(u32_(0x2fc)); // sponsors
    f = f.wrapping_sub(u32_(0x308)).wrapping_add(u32_(0x30c)); // merchandise
    f = f.wrapping_add(transfers);
    f = f.wrapping_sub(i64_(0x2d8).wrapping_add(i64_(0x2e8)).wrapping_add(i32_(0x2f0))); // stadium
    f = f.wrapping_add(u32_(0x2e0));
    f = f.wrapping_sub(i32_(0x280)); // staff
    f = f.wrapping_sub(r32(&[
        plus(0x284),
        plus(0x288),
        (0x28c, -1),
        plus(0x290),
        plus(0x294),
        plus(0x2b0),
    ])); // players
    f = f.wrapping_sub(r32(&[plus(0x29c), plus(0x2a0), plus(0x2a4), plus(0x2a8), plus(0x2ac)]));
    f = f.wrapping_add(i32_(0x298)); // other income, and the same field again
    f = f.wrapping_add(r32(&[plus(0x2c4), plus(0x254), plus(0x2cc), plus(0x314), plus(0x298)]));
    let departments = (level(0x5e1, &STATISTICS) * STATISTICS_UNIT
        + level(0x5e2, &COMPUTERS) * COMPUTERS_UNIT) as i32;
    let costs = [plus(0x2b8), plus(0x2c0), plus(0x2c8), plus(0x310)];
    let costs = r32(&costs) as i32;
    f = f.wrapping_sub(i64::from(
        costs.wrapping_add(departments).wrapping_add(team.get::<i32>(at(0x2b4))),
    )); // other costs
    f
}

/// The figure, in the game's money (`FUN_00621eac`).
pub fn figure(team: &Team, fund_prices: [i32; 5]) -> i64 {
    let f = booked(team, i64::from(team.get::<i32>(at(0x258)))); // transfers, net
    i64_(team, 0x210)
        .wrapping_add(i64::from(team.get::<i32>(at(0x2d0))))
        .wrapping_add(f)
        .wrapping_sub(loans(team))
        .wrapping_add(deposits(team, fund_prices))
}

/// The season's result so far as the balance sheet has it, the transfers
/// as sales less purchases (`FUN_0061fc81`).
pub fn result(team: &Team) -> i64 {
    booked(team, i64_(team, 0x470).wrapping_sub(i64_(team, 0x478)))
}

/// What the club owes on its loans (`FUN_006488ca`).
pub fn loans(team: &Team) -> i64 {
    (0..LOANS)
        .map(|k| 0x318 + ENTRY * k)
        .filter(|&loan| team.get::<u8>(at(loan + 0x0f)) & 1 != 0)
        .fold(0, |sum: i64, loan| sum.wrapping_add(i64_(team, loan)))
}

/// What its deposits are worth now, by the funds' prices against those
/// they were bought at (`FUN_00648947`).
pub fn deposits(team: &Team, fund_prices: [i32; 5]) -> i64 {
    let mut v: i64 = 0;
    for k in 0..DEPOSITS {
        let deposit = 0x3d8 + ENTRY * k;
        if team.get::<u8>(at(deposit + 0x13)) & 1 != 0 {
            let fund = usize::from(team.get::<u8>(at(deposit + 0x12)));
            let price = i64::from(fund_prices.get(fund).copied().unwrap_or(0));
            let bought = i64::from(team.get::<i16>(at(deposit + 0x10)));
            if bought != 0 {
                v = v.wrapping_add(i64_(team, deposit + 8).wrapping_mul(price) / bought);
            }
        }
    }
    v
}

fn at(c: usize) -> usize {
    c + CLUB_TO_TEAM
}

fn i64_(team: &Team, c: usize) -> i64 {
    team.get::<i64>(at(c))
}

/// What the balance sheet shows of a club (BILANZ.md).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Balance {
    /// The cash at the season's start, and what its shares brought in.
    pub cash: i64,
    pub shares: i64,
    /// The season's result so far.
    pub result: i64,
    pub loans: i64,
    pub deposits: i64,
    /// What the bank lends the club by its league and class, and what the
    /// transfers agreed for the season's end bring in and cost.
    base: i32,
    sold: u32,
    bought: u32,
}

impl Balance {
    pub fn of(team: &Team, fund_prices: [i32; 5], agreed: &[Agreed]) -> Self {
        let club = (team.country(), i16::from(team.number()));
        let fees = |pick: &dyn Fn(&Agreed) -> bool| {
            agreed.iter().filter(|a| pick(a)).fold(0u32, |sum, a| sum.wrapping_add(a.fee))
        };
        Self {
            cash: i64_(team, 0x210),
            shares: i64::from(team.get::<i32>(at(0x2d0))),
            result: result(team),
            loans: loans(team),
            deposits: deposits(team, fund_prices),
            base: credit_base(team.level(), team.credit_class()),
            sold: fees(&|a| a.seller == club),
            bought: fees(&|a| a.seller != club && a.buyer == club && !a.flagged),
        }
    }

    /// The club's fortune: cash, shares and the season's result
    /// (`FUN_0061f1d9`).
    pub fn fortune(&self) -> i64 {
        self.cash.wrapping_add(self.shares).wrapping_add(self.result)
    }

    /// Its money in the bank: the fortune less what lies in deposits, plus
    /// what it borrowed (`FUN_0061fd97`).
    pub fn bank(&self) -> i64 {
        self.fortune().wrapping_sub(self.deposits).wrapping_add(self.loans)
    }

    /// What the bank would lend it (`FUN_00622a87`).
    pub fn credit_limit(&self) -> i32 {
        self.base.wrapping_add(self.sold as i32).wrapping_sub(self.bought as i32).max(0)
    }

    /// What it could still spend: its fortune and the credit limit
    /// (`FUN_00622bd9`).
    pub fn investable(&self) -> i64 {
        self.fortune()
            .wrapping_add(i64::from(self.base))
            .wrapping_add(i64::from(self.sold))
            .wrapping_sub(i64::from(self.bought))
            .max(0)
    }
}

/// Why a club may not go to the stock market, as far as the game's checks
/// are known (`OnClicked_IDC_BIL_NEU_BUT_AG_00481c8d`): below the second
/// league, the amateurs of another club, a fortune below the least, more
/// loans than deposits, a loss in one of the last three seasons.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    League,
    Amateurs,
    Fortune(i64),
    Loans,
    Losses,
}

/// The least fortune a club on the stock market has.
const LEAST_FORTUNE: i64 = 2_000_000;
/// The last three seasons' results, after tax and dividends.
const PAST_RESULTS: [usize; 3] = [0x908, 0x910, 0x918];

impl Balance {
    /// The first check a club fails of those for the stock market, if it
    /// fails one; the stadium's and the grounds' value are not checked.
    pub fn refusal(&self, team: &Team) -> Option<Refusal> {
        if team.level() >= 2 {
            return Some(Refusal::League);
        }
        if team.linked()[0] >= 0 {
            return Some(Refusal::Amateurs);
        }
        if self.fortune() < LEAST_FORTUNE {
            return Some(Refusal::Fortune(LEAST_FORTUNE));
        }
        if self.loans > self.deposits {
            return Some(Refusal::Loans);
        }
        if PAST_RESULTS.iter().any(|&at| team.get::<i64>(at) < 0) {
            return Some(Refusal::Losses);
        }
        None
    }
}

/// What the bank lends a club of a league level and a class, before the
/// transfers (`FUN_00622a87`): the more, the higher the league and the
/// lower the class.
fn credit_base(level: u8, class: u8) -> i32 {
    let (first, step) = match level {
        0 => (4_000_000, 1_500_000),
        1 => (2_000_000, 800_000),
        _ => (1_000_000, 500_000),
    };
    (8 - i32::from(class)).wrapping_mul(step).wrapping_add(first)
}

/// A season's accounts as the club keeps them (0xf0 bytes): this season's
/// booked at the club's +0x228, the plan at +0x120, last season's at +0x610,
/// and the copy of this season's the turn started with.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Accounts(Vec<u8>);

/// How long the accounts are, and where they lie: this season's and the
/// plan in the team record, last season's in the human club's extra data,
/// the turn's copy in the manager's record.
const ACCOUNTS: usize = 0xf0;
const BOOKED: usize = 0x228;
const PLAN: usize = 0x120;
const LAST_SEASON: usize = 0x4c8;
const TURN_COPY: usize = 0x14d8;
/// What the season's transfers came to, sales and purchases: the club's,
/// last season's (in the extra data), and the turn's copy of the club's
/// (in the manager's record).
const SALES: usize = 0x470;
const PURCHASES: usize = 0x478;
const LAST_SALES: usize = 0x710;
const LAST_PURCHASES: usize = 0x718;
const TURN_SALES: usize = 0x1720;
const TURN_PURCHASES: usize = 0x1728;
/// Whether the club has a last season (in the extra data).
const HAS_LAST_SEASON: usize = 0x898;

/// The sums the finance screen's columns are made of (FINANZUEBERSICHT.md
/// §3.1).
pub const SLOTS: usize = 18;

/// How a slot's sum is widened: a signed or an unsigned 32-bit one, or one
/// of 64 bits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Width {
    Signed,
    Unsigned,
    Wide,
}

const WIDTHS: [Width; SLOTS] = {
    use Width::*;
    [
        Signed, Signed, Signed, Unsigned, Unsigned, Unsigned, Wide, Wide, Wide, Unsigned, Signed,
        Signed, Signed, Signed, Signed, Signed, Signed, Signed,
    ]
};

/// Each slot's fields in the accounts, added or taken away; the
/// transfers (slots 6 and 7) lie outside them, the stadium's (slot 8) are
/// two of 64 bits and one of 32.
const TERMS: [&[(usize, i32)]; SLOTS] = [
    &[(0x8, 1), (0xc, 1), (0x10, 1), (0x14, 1)], // tickets
    &[(0x18, 1), (0x1c, 1)],                     // TV
    &[(0x20, 1), (0x24, 1), (0x28, 1)],          // prizes
    &[(0xcc, 1), (0xd0, 1), (0xd4, 1)],          // sponsors
    &[(0xe0, 1)],                                // merchandise bought
    &[(0xe4, 1)],                                // merchandise sold
    &[],                                         // purchases
    &[],                                         // sales
    &[],                                         // stadium
    &[(0xb8, 1)],                                // stadium income
    &[(0x58, 1)],                                // staff
    &[(0x5c, 1), (0x60, 1), (0x64, -1), (0x68, 1), (0x6c, 1), (0x88, 1)], // players
    &[(0x74, 1), (0x78, 1), (0x7c, 1), (0x80, 1), (0x84, 1)], // match and training
    &[(0x70, 1)],                                // other income
    &[(0x90, 1), (0x98, 1), (0xa0, 1), (0xe8, 1), (0x8c, 1)], // other costs
    &[(0x9c, 1), (0x2c, 1), (0xa4, 1), (0xec, 1), (0x70, 1)], // other income, more
    &[(0xa8, 1)],                                // share proceeds
    &[(0xac, 1)],                                // dividends
];
pub(crate) const TV_SLOT: usize = 1;
pub(crate) const PRIZES_SLOT: usize = 2;
const PURCHASES_SLOT: usize = 6;
const SALES_SLOT: usize = 7;
const STADIUM_SLOT: usize = 8;
pub(crate) const PLAYERS_SLOT: usize = 11;
const OTHER_INCOME_FIRST_SLOT: usize = 13;
pub(crate) const OTHER_COSTS_SLOT: usize = 14;
pub(crate) const OTHER_INCOME_SLOT: usize = 15;
pub(crate) const SHARES_SLOT: usize = 16;
pub(crate) const DIVIDENDS_SLOT: usize = 17;

impl Accounts {
    fn at(bytes: &[u8], at: usize) -> Self {
        Self(bytes.get(at..at + ACCOUNTS).map_or_else(|| vec![0; ACCOUNTS], <[u8]>::to_vec))
    }

    /// This season's, as booked so far.
    pub fn booked(team: &Team) -> Self {
        Self::at(team.bytes(), at(BOOKED) - TEAM_BASE)
    }

    /// The season's plan, the "Kalkulation".
    pub fn plan(team: &Team) -> Self {
        Self::at(team.bytes(), at(PLAN) - TEAM_BASE)
    }

    /// Last season's, if the club had one with its manager.
    pub fn last_season(extra: &Extra) -> Option<Self> {
        (extra.get::<i16>(HAS_LAST_SEASON) != 0)
            .then(|| Self::at(extra.bytes(), LAST_SEASON - EXTRA_BASE))
    }

    /// This season's as the turn started.
    pub fn turn_copy(manager: &Manager) -> Self {
        Self::at(manager.bytes(), TURN_COPY)
    }

    fn i32_(&self, at: usize) -> i32 {
        i32::from_le_bytes(self.0[at..at + 4].try_into().unwrap())
    }

    fn i64_(&self, at: usize) -> i64 {
        i64::from_le_bytes(self.0[at..at + 8].try_into().unwrap())
    }

    /// A slot's sum in 32 bits, wrapping, the departments in the other
    /// costs.
    pub(crate) fn sum32(&self, slot: usize, departments: i32) -> i32 {
        let sum = TERMS[slot]
            .iter()
            .fold(0i32, |sum, &(at, sign)| sum.wrapping_add(self.i32_(at).wrapping_mul(sign)));
        if slot == OTHER_COSTS_SLOT { sum.wrapping_add(departments) } else { sum }
    }

    /// The stadium's costs (`FUN_006744e3`).
    pub(crate) fn stadium(&self) -> i64 {
        self.i64_(0xb0).wrapping_add(self.i64_(0xc0)).wrapping_add(i64::from(self.i32_(0xc8)))
    }
}

fn widen(slot: usize, v: i32) -> i64 {
    match WIDTHS[slot] {
        Width::Unsigned => i64::from(v as u32),
        _ => i64::from(v),
    }
}

/// What the statistics and the computer department cost a season, by
/// their levels (part of `FUN_006743db`).
pub(crate) fn departments(team: &Team) -> i32 {
    let level = |c: usize, table: &[i64; 4]| {
        let l = team.get::<i8>(at(c));
        usize::try_from(l).ok().and_then(|l| table.get(l)).copied().unwrap_or(0)
    };
    (level(0x5e1, &STATISTICS) * STATISTICS_UNIT + level(0x5e2, &COMPUTERS) * COMPUTERS_UNIT) as i32
}

/// The slots of the booked figures (the screen's "Aktuell").
pub fn current(team: &Team) -> [i64; SLOTS] {
    let accounts = Accounts::booked(team);
    let d = departments(team);
    std::array::from_fn(|slot| match slot {
        PURCHASES_SLOT => i64_(team, PURCHASES),
        SALES_SLOT => i64_(team, SALES),
        STADIUM_SLOT => accounts.stadium(),
        _ => widen(slot, accounts.sum32(slot, d)),
    })
}

/// The slots of what was booked since the turn started (the screen's
/// "Wochenüberblick"): each sum less the turn's copy's, in 32 bits; the
/// other income's first part left out.
pub fn this_week(team: &Team, manager: &Manager) -> [i64; SLOTS] {
    let (now, then) = (Accounts::booked(team), Accounts::turn_copy(manager));
    let d = departments(team);
    std::array::from_fn(|slot| match slot {
        PURCHASES_SLOT => i64_(team, PURCHASES).wrapping_sub(manager.get::<i64>(TURN_PURCHASES)),
        SALES_SLOT => i64_(team, SALES).wrapping_sub(manager.get::<i64>(TURN_SALES)),
        STADIUM_SLOT => now.stadium().wrapping_sub(then.stadium()),
        OTHER_INCOME_FIRST_SLOT => 0,
        _ => widen(slot, now.sum32(slot, d).wrapping_sub(then.sum32(slot, d))),
    })
}

/// The slots of last season's figures (the screen's "Vergleich mit
/// Vorjahr"), the departments at their levels now; the first part of the
/// other income is the one already in `slots`, which the game leaves
/// there.
pub fn last_season(team: &Team, extra: &Extra, slots: &[i64; SLOTS]) -> Option<[i64; SLOTS]> {
    let accounts = Accounts::last_season(extra)?;
    let d = departments(team);
    Some(std::array::from_fn(|slot| match slot {
        PURCHASES_SLOT => extra.get::<i64>(LAST_PURCHASES),
        SALES_SLOT => extra.get::<i64>(LAST_SALES),
        STADIUM_SLOT => accounts.stadium(),
        OTHER_INCOME_FIRST_SLOT => slots[slot],
        _ => widen(slot, accounts.sum32(slot, d)),
    }))
}

/// What the slots come to: income less costs (`FUN_004a2e36`).
pub fn total(s: &[i64; SLOTS]) -> i64 {
    let plus = [0, 1, 2, 3, 5, 7, 9, 13, 15, 16];
    let minus = [4, 6, 8, 10, 11, 12, 14, 17];
    let t = plus.iter().fold(0i64, |t, &i| t.wrapping_add(s[i]));
    minus.iter().fold(t, |t, &i| t.wrapping_sub(s[i]))
}

/// The plan's staff posts, and what a groundsman and the like cost
/// (`FUN_006745c4`).
const PLANNED_POSTS: [usize; 9] = [0x34, 0x38, 0x3c, 0x40, 0x44, 0x48, 0x4c, 0x50, 0x54];
const PLANNED_HANDS: usize = 0xd8;
const HAND: i32 = 80_000;
/// The plan's transfer balance.
const PLANNED_TRANSFERS: usize = 0x30;
const STAFF_SLOT: usize = 10;

/// The slots of the plan (the screen's "Vergleich mit Kalkulation"), its
/// tickets as the attendance model has them (`FUN_0061f427`); the
/// transfers a balance on one side, the staff by their posts.
pub fn planned(team: &Team, tickets: i64) -> [i64; SLOTS] {
    let plan = Accounts::plan(team);
    let d = departments(team);
    let transfers = i64::from(plan.i32_(PLANNED_TRANSFERS));
    std::array::from_fn(|slot| match slot {
        0 => tickets,
        PURCHASES_SLOT => {
            if transfers < 0 {
                -transfers
            } else {
                0
            }
        }
        SALES_SLOT => transfers.max(0),
        STADIUM_SLOT => plan.stadium(),
        STAFF_SLOT => i64::from(
            PLANNED_POSTS
                .iter()
                .fold(0i32, |s, &at| s.wrapping_add(plan.i32_(at)))
                .wrapping_add(i32::from(plan.0[PLANNED_HANDS]) * HAND),
        ),
        _ => widen(slot, plan.sum32(slot, d)),
    })
}

/// A deposit of the club: its slot, its partner, what it cost and what it
/// is worth now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Deposit {
    pub slot: usize,
    pub partner: u8,
    pub cost: i64,
    pub value: i64,
}

/// A deposit's fields: the amount paid and invested, the fund's price
/// then, the partner, whether it is in use, the day it was made.
const DEPOSIT_COST: usize = 0;
const DEPOSIT_AMOUNT: usize = 8;
const DEPOSIT_PRICE: usize = 0x10;
const DEPOSIT_PARTNER: usize = 0x12;
const DEPOSIT_IN_USE: usize = 0x13;
const DEPOSIT_DAY: usize = 0x14;
/// Where a dissolved deposit's gain and loss are booked: the other income
/// and the other costs.
const GAINS: usize = 0x2cc;
const LOSSES: usize = 0x2c8;
/// Below this, nothing may be invested; more than that, never
/// (`DAT_00a3b37c`, `DAT_00a3b2b0`).
pub const LEAST_DEPOSIT: i64 = 100_000;
const MOST_DEPOSITS: i64 = 50_000_000;
/// The overdraft is kept in units of this.
const OVERDRAFT_UNIT: i64 = 100_000;

fn deposit_at(slot: usize, field: usize) -> usize {
    at(0x3d8 + ENTRY * slot + field)
}

/// The deposits in use (`FUN_00444007`).
pub fn deposits_of(team: &Team, fund_prices: [i32; 5]) -> Vec<Deposit> {
    (0..DEPOSITS)
        .filter(|&slot| team.get::<u8>(deposit_at(slot, DEPOSIT_IN_USE)) & 1 != 0)
        .map(|slot| {
            let partner = team.get::<u8>(deposit_at(slot, DEPOSIT_PARTNER));
            let price = i64::from(fund_prices.get(usize::from(partner)).copied().unwrap_or(0));
            let bought = i64::from(team.get::<i16>(deposit_at(slot, DEPOSIT_PRICE)));
            let amount = team.get::<i64>(deposit_at(slot, DEPOSIT_AMOUNT));
            Deposit {
                slot,
                partner,
                cost: team.get::<i64>(deposit_at(slot, DEPOSIT_COST)),
                value: if bought != 0 { amount.wrapping_mul(price) / bought } else { 0 },
            }
        })
        .collect()
}

/// The first deposit slot free (`FUN_00648161`).
pub fn free_deposit(team: &Team) -> Option<usize> {
    (0..DEPOSITS).find(|&slot| team.get::<u8>(deposit_at(slot, DEPOSIT_IN_USE)) & 1 == 0)
}

/// What the club may still put into deposits: its money in the bank and
/// the overdraft, none if it is past that, at most the most
/// (`FUN_00444485`).
pub fn available(team: &Team, balance: &Balance) -> i64 {
    let overdraft = i64::from(team.overdraft()) * OVERDRAFT_UNIT;
    let bank = balance.bank();
    let v = if bank < -overdraft { 0 } else { bank + overdraft };
    v.min(MOST_DEPOSITS)
}

/// A deposit made with a partner, at the fund's price of the day
/// (`OnClicked_IDC_ANLAGEN_ANLEGEN_00444359`); the cash is not touched,
/// the bank's money is less by the deposit.
pub fn invest(
    team: &mut Team,
    slot: usize,
    amount: i64,
    partner: u8,
    fund_prices: [i32; 5],
    day: i16,
) {
    team.put(deposit_at(slot, DEPOSIT_AMOUNT), amount);
    team.put(deposit_at(slot, DEPOSIT_COST), amount);
    team.put(deposit_at(slot, DEPOSIT_PARTNER), partner);
    let price = fund_prices.get(usize::from(partner)).copied().unwrap_or(0);
    team.put(deposit_at(slot, DEPOSIT_PRICE), price as i16);
    let in_use = team.get::<u8>(deposit_at(slot, DEPOSIT_IN_USE));
    team.put(deposit_at(slot, DEPOSIT_IN_USE), in_use | 1);
    team.put(deposit_at(slot, DEPOSIT_DAY), day);
}

/// A deposit dissolved: what it made booked as other income, what it lost
/// as other costs, at most 2³¹ − 1 either way (`FUN_00648788`).
pub fn dissolve(team: &mut Team, deposit: &Deposit) {
    let gain = deposit.value.wrapping_sub(deposit.cost);
    if gain > 0 {
        let gain = gain.min(i64::from(i32::MAX)) as i32;
        team.put(at(GAINS), team.get::<i32>(at(GAINS)).wrapping_add(gain));
    } else if gain >= -i64::from(i32::MAX) {
        team.put(at(LOSSES), team.get::<i32>(at(LOSSES)).wrapping_sub(gain as i32));
    } else {
        // A slip of the game's: a loss past 2³¹ comes off instead.
        team.put(at(LOSSES), team.get::<i32>(at(LOSSES)).wrapping_sub(i32::MAX));
    }
    team.put(deposit_at(deposit.slot, DEPOSIT_IN_USE), 0u8);
}

/// The day of the quarter the bank counts from: thirty days a month
/// (`FUN_00676a74`).
pub fn bank_day(day: u16, month: u16) -> i16 {
    (i32::from(month % 3) * 30 - 30 + i32::from(day.min(30))) as i16
}

/// The bank's steps of money for its sliders, in 10.000s: one step each
/// up to 10, then 2, 5 and 10 (`FUN_005c69c9`, `FUN_005c690b` with the
/// table at 0x88d4b8).
pub const STEP_UNIT: i64 = 10_000;

pub fn step_amount(position: i32) -> i64 {
    let p = i64::from(position);
    match p {
        ..=10 => p,
        11..=30 => 10 + 2 * (p - 10),
        31..=40 => 50 + 5 * (p - 30),
        _ => 100 + 10 * (p - 40),
    }
}

pub fn step_position(units: i64) -> i32 {
    let p = match units {
        ..=10 => units,
        11..=50 => 10 + (units - 10) / 2,
        51..=100 => 30 + (units - 50) / 5,
        _ => 40 + (units - 100) / 10,
    };
    p as i16 as i32
}

/// A loan of the club: its slot, what is still owed, its rates for the
/// first, second and further years, its term, whether it may be paid back
/// early, the bank days it was paid on and taken on, and its end (years ×
/// 1000 + the day it was taken).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Loan {
    pub slot: usize,
    pub principal: i64,
    pub rates: [i16; 3],
    pub term: u8,
    pub repayable: bool,
    pub paid: i16,
    pub taken: i16,
    pub end: i16,
}

/// A loan's fields.
const LOAN_PRINCIPAL: usize = 0;
const LOAN_RATES: usize = 8;
const LOAN_TERM: usize = 0xe;
const LOAN_FLAGS: usize = 0xf;
const LOAN_PAID: usize = 0x10;
const LOAN_TAKEN: usize = 0x12;
const LOAN_END: usize = 0x14;
const IN_USE: u8 = 1;
const REPAYABLE: u8 = 2;
/// The terms' years (`DAT_0088d498`).
pub const LOAN_YEARS: [i16; 3] = [1, 2, 5];
/// The bank's rates by term, less than the first year's, repayable ones
/// first (`FUN_005d77a6`).
const RATE_STEPS: [i16; 6] = [0, 25, 50, 150, 175, 200];
/// The least the bank lends; a year of the bank's, in days.
pub const LEAST_LOAN: i64 = 10_000;
const BANK_YEAR: i32 = 360;

fn loan_at(slot: usize, field: usize) -> usize {
    at(0x318 + ENTRY * slot + field)
}

/// The bank's six rates: repayable and not, for one, two and five years.
pub fn credit_rates(base: i16) -> [i16; 6] {
    RATE_STEPS.map(|step| base.wrapping_sub(step))
}

/// The loans in use (`FUN_005d791d`).
pub fn loans_of(team: &Team) -> Vec<Loan> {
    (0..LOANS)
        .filter(|&slot| team.get::<u8>(loan_at(slot, LOAN_FLAGS)) & IN_USE != 0)
        .map(|slot| {
            let rate = |k: usize| team.get::<i16>(loan_at(slot, LOAN_RATES + 2 * k));
            Loan {
                slot,
                principal: team.get::<i64>(loan_at(slot, LOAN_PRINCIPAL)),
                rates: [rate(0), rate(1), rate(2)],
                term: team.get::<u8>(loan_at(slot, LOAN_TERM)),
                repayable: team.get::<u8>(loan_at(slot, LOAN_FLAGS)) & REPAYABLE != 0,
                paid: team.get::<i16>(loan_at(slot, LOAN_PAID)),
                taken: team.get::<i16>(loan_at(slot, LOAN_TAKEN)),
                end: team.get::<i16>(loan_at(slot, LOAN_END)),
            }
        })
        .collect()
}

/// The first loan slot free (`FUN_005d8177`).
pub fn free_loan(team: &Team) -> Option<usize> {
    (0..LOANS).find(|&slot| team.get::<u8>(loan_at(slot, LOAN_FLAGS)) & IN_USE == 0)
}

impl Balance {
    /// What the bank would still lend: what may be invested and half the
    /// deposits, less the loans (`FUN_00622db2`).
    pub fn credit_frame(&self) -> i64 {
        self.investable().wrapping_add(self.deposits / 2).wrapping_sub(self.loans)
    }
}

impl Loan {
    /// The rate it runs at now, in 1/100 %.
    pub fn rate(&self) -> i16 {
        self.rates[usize::from(self.term).min(2)]
    }

    /// The months until it ends, by the season's start year and today's
    /// month and year (`FUN_005d791d`).
    pub fn months_left(&self, start_year: u16, month: u16, year: u16) -> i32 {
        let mut m = i32::from(self.end % 1000 / 30) + 7;
        let mut y = i32::from(start_year) + i32::from(self.end / 1000);
        if m > 12 {
            m -= 12;
            y += 1;
        }
        (y - i32::from(year)) * 12 + m - i32::from(month)
    }

    /// The days the bank counts it as running (`FUN_004fb7b9`).
    fn days(&self, bank_day: i16) -> i16 {
        self.paid.wrapping_add(bank_day).wrapping_sub(self.taken)
    }

    /// What paying back an amount early costs more, or saves, by the rate
    /// of the years it has run against its own (`FUN_004fbb79`).
    pub fn early_cost(&self, amount: i32, bank_day: i16) -> i32 {
        let days = self.days(bank_day);
        let years = match days {
            ..360 => 0,
            360..720 => 1,
            _ => 2,
        };
        let difference = i32::from(self.rates[years]) - i32::from(self.rate());
        amount / 100 * difference / 100 * i32::from(days) / BANK_YEAR
    }
}

/// A loan asked for: how much, for which term, whether it may be paid back
/// early.
#[derive(Clone, Copy, Debug)]
pub struct Credit {
    pub amount: i64,
    pub term: u8,
    pub repayable: bool,
}

/// A loan taken in a free slot: its rates from the bank's six, its end
/// after its years (`OnClicked_IDC_KREDITE_AUFNEHMEN_005d7fb3`). The cash is
/// not touched: the bank's money is more by the loan.
pub fn borrow(
    team: &mut Team,
    slot: usize,
    credit: Credit,
    rates: [i16; 6],
    bank_day: i16,
    today: u16,
) {
    let Credit { amount, term, repayable } = credit;
    team.put(loan_at(slot, LOAN_PRINCIPAL), amount);
    team.put(loan_at(slot, LOAN_TERM), term);
    team.put(loan_at(slot, LOAN_FLAGS), if repayable { IN_USE | REPAYABLE } else { IN_USE });
    let first = if repayable { 0 } else { 3 };
    for k in 0..3 {
        team.put(loan_at(slot, LOAN_RATES + 2 * k), rates[first + k]);
    }
    team.put(loan_at(slot, LOAN_TAKEN), bank_day);
    let years = LOAN_YEARS[usize::from(term).min(2)];
    team.put(loan_at(slot, LOAN_END), years.wrapping_mul(1000).wrapping_add(today as i16));
    team.put(loan_at(slot, LOAN_PAID), 0i16);
}

/// An amount paid back: all of it ends the loan; less is taken off it,
/// the interest since it was taken booked — by a slip of the game's, as
/// the other costs doubled (`FUN_006481ac`, `FUN_0064809f`).
pub fn repay(team: &mut Team, loan: &Loan, amount: i32, bank_day: i16) {
    if loan.principal <= i64::from(amount) {
        team.put(loan_at(loan.slot, LOAN_PRINCIPAL), 0i64);
        team.put(loan_at(loan.slot, LOAN_FLAGS), 0u8);
        return;
    }
    if loan.taken < bank_day {
        let days = i32::from(bank_day) - i32::from(loan.taken);
        let interest = amount / 100 * i32::from(loan.rate()) / 100 * days / BANK_YEAR;
        let costs = team.get::<i32>(at(LOSSES));
        let sum = i64::from(costs) + i64::from(interest);
        let booked = if sum < 0x8000_0000 { costs.wrapping_add(costs) } else { i32::MAX };
        team.put(at(LOSSES), booked);
    }
    team.put(loan_at(loan.slot, LOAN_PRINCIPAL), loan.principal - i64::from(amount));
}
