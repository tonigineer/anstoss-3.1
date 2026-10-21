//! What a club's season will come to, as the balance sheet forecasts it
//! and the finance screen projects it (BILANZ.md §2.6, FINANZUEBERSICHT.md
//! §3.2 in the RE repo): "wie bisher", the booked figures carried on or the
//! plan's where they are more, and "wie kalkuliert", the booked figures
//! extrapolated to the whole season, rounded to 10.000s.

use std::io;

use a3_files::exe::Exe;

use crate::calendar::Calendar;
use crate::finances::{self, Accounts, SLOTS};
use crate::records::{Manager, Records, Team};
use crate::tickets::{self, Gates, TicketTables};

/// The months left in the season by the month, the current one counted
/// (`DAT_0089e220`), and what perimeter advertising brings by the
/// stadium's level, in 1.000s a season (`DAT_0089e230`).
const MONTHS_LEFT_AT: u32 = 0x89e220;
const MONTHS: usize = 13;
const PERIMETER_AT: u32 = 0x89e230;
const PERIMETER_LEVELS: usize = 5;
const PERIMETER_UNIT: i32 = 1000;
/// The season's twelve months; from July on, a remainder counts too.
const YEAR: i64 = 12;
const JULY: usize = 7;
/// Figures are rounded down to this.
const ROUND: i64 = 10_000;
/// A staff member's yearly pay at their record's +0x30; the second table
/// of persons, by id up to 100.
const PERSON_PAY: usize = 0x30;
const OTHER_PERSONS_AT: u32 = 0xbe17f8;
const OTHER_PERSON_PAY: usize = 0x10;
const OTHER_PERSONS: i16 = 100;
/// A human manager's yearly pay, and his status when he is the club's
/// manager rather than its trainer (`FUN_0062053b`).
const MANAGER_PAY: usize = 0x15c;
const MANAGER_ONLY: u8 = 3;
/// What a groundsman and the like cost a season (`DAT_00a3b380`).
const STAFF_UNIT: i32 = 80_000;
/// The merchandise each human club sells, by its slot: 40 articles of
/// 0x50 bytes, an article's weekly sales at +0x4c, 0 at +0 for none.
const MERCHANDISE_AT: u32 = 0xa32860;
const ARTICLES: usize = 40;
const ARTICLE: usize = 0x50;
const WEEKLY_SALES: usize = 0x4c;
const SEASON_WEEKS: i32 = 52;
/// A stadium's level of perimeter advertising, in its block.
const STADIUM_AT: u32 = 0x1a48138;
const PERIMETER_LEVEL: usize = 0x66;

/// The game's tables of the forecasts.
pub struct ForecastTables {
    months_left: Vec<u8>,
    perimeter: Vec<i16>,
    pub tickets: TicketTables,
}

impl ForecastTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        Ok(Self {
            months_left: exe.bytes(MONTHS_LEFT_AT, MONTHS)?.to_vec(),
            perimeter: exe
                .u16s(PERIMETER_AT, PERIMETER_LEVELS)?
                .iter()
                .map(|&v| v as i16)
                .collect(),
            tickets: TicketTables::load(exe)?,
        })
    }
}

/// A club's forecast: its records, the month and the day, and what its
/// tickets will bring as each way reckons it.
pub struct Forecast<'a> {
    records: &'a Records,
    team: &'a Team,
    index: usize,
    manager: Option<&'a Manager>,
    month: usize,
    months_left: i64,
    today: usize,
    perimeter: i32,
    gates: Gates,
    teams: i32,
    sponsor_bonus: bool,
    booked: Accounts,
    plan: Accounts,
    departments: i32,
}

impl<'a> Forecast<'a> {
    pub fn new(
        records: &'a Records,
        index: usize,
        tables: &ForecastTables,
        calendar: &Calendar,
    ) -> Option<Self> {
        let team = records.teams.get(index)?;
        let manager =
            usize::from(team.human()).checked_sub(1).and_then(|s| records.managers.get(s));
        let month = usize::from(records.globals.date()[1]);
        let months_left = i64::from(tables.months_left.get(month).copied().unwrap_or(0));
        let mut forecast = Self {
            records,
            team,
            index,
            manager,
            month,
            months_left,
            today: usize::from(records.globals.day()),
            perimeter: 0,
            gates: tickets::gates_of(records, team, &tables.tickets, calendar),
            teams: i32::from(tickets::league_teams(records, team)),
            sponsor_bonus: european_place(records, team, &tables.tickets),
            booked: Accounts::booked(team),
            plan: Accounts::plan(team),
            departments: finances::departments(team),
        };
        forecast.perimeter = forecast.perimeter(tables);
        Some(forecast)
    }

    fn i32_(&self, club: usize) -> i32 {
        self.team.get::<i32>(club + finances::CLUB_TO_TEAM)
    }

    fn u32_(&self, club: usize) -> u32 {
        self.team.get::<u32>(club + finances::CLUB_TO_TEAM)
    }

    fn i64_(&self, club: usize) -> i64 {
        self.team.get::<i64>(club + finances::CLUB_TO_TEAM)
    }

    fn sum(&self, accounts: &Accounts, slot: usize) -> i32 {
        accounts.sum32(slot, self.departments)
    }

    /// The larger of the plan's and the booked sum.
    fn larger(&self, slot: usize) -> i32 {
        self.sum(&self.plan, slot).max(self.sum(&self.booked, slot))
    }

    /// What is still to pay of a year's amount (part of `FUN_00620453`).
    fn rest(&self, x: i32) -> i32 {
        let x = i64::from(x);
        let rest = x / YEAR * self.months_left + if self.month >= JULY { x % YEAR } else { 0 };
        rest as i32
    }

    /// A booked amount extrapolated to the season, rounded
    /// (`FUN_00620d4f`).
    fn to_season(&self, x: i64) -> i64 {
        let r = if self.months_left == YEAR {
            x.wrapping_mul(YEAR)
        } else {
            let q = x / (YEAR - self.months_left);
            q.wrapping_mul(YEAR) + if self.month >= JULY { q % YEAR } else { 0 }
        };
        r / ROUND * ROUND
    }

    /// What perimeter advertising brings a month, by the stadium's level
    /// and the gates so far (`FUN_0061ffe4`).
    fn perimeter(&self, tables: &ForecastTables) -> i32 {
        let level = self
            .records
            .block(&format!("{STADIUM_AT:#x}[{}]", self.index))
            .and_then(|b| b.get(PERIMETER_LEVEL).copied())
            .unwrap_or(0);
        let per = i32::from(tables.perimeter.get(usize::from(level)).copied().unwrap_or(0));
        let season = (per * PERIMETER_UNIT) as f32;
        let games = self.team.home_games();
        let average = if games != 0 {
            let gates: i32 = self
                .team
                .gates()
                .iter()
                .take(usize::from(games))
                .fold(0, |s, &g| s.wrapping_add(g));
            (f64::from(gates) / f64::from(games)) as f32
        } else {
            self.i32_(0x120) as f32
        };
        (f64::from(season) * f64::from(average) / 25000.0 / 12.0) as i32
    }

    fn sponsors(&self) -> i32 {
        let months = self.months_left as i32;
        self.i32_(0x2f4)
            .wrapping_add(self.i32_(0x2f8))
            .wrapping_add(self.i32_(0x2fc))
            .wrapping_add(self.perimeter.wrapping_mul(months))
    }

    /// The sponsors carried on, or the plan's if more (`FUN_006200bb`).
    fn sponsors_carried_on(&self) -> i64 {
        i64::from((self.sponsors() as u32).max(self.u32_(0x1ec)) as i32)
    }

    /// The sponsors extrapolated, the bonus for the league's top place
    /// with them (`FUN_006211e8`).
    fn sponsors_extrapolated(&self) -> i64 {
        let mut v = self.sponsors();
        if self.team.table_place() == 1 {
            v = v.wrapping_add(self.i32_(0x514));
        }
        if self.sponsor_bonus {
            v = v.wrapping_add(self.i32_(0x518));
        }
        i64::from(v) / ROUND * ROUND
    }

    /// Merchandise sold, carried on by its weekly sales, or the plan's if
    /// more (`FUN_006201c9`); a contractor's takings as booked.
    fn merchandise_carried_on(&self) -> i64 {
        if self.team.get::<u8>(0x5a9 + finances::CLUB_TO_TEAM) != 0 {
            return i64::from(self.i32_(0x30c));
        }
        let weekly = usize::from(self.team.human())
            .checked_sub(1)
            .and_then(|slot| self.records.block(&format!("{MERCHANDISE_AT:#x}[{slot}]")))
            .map_or(0, |block| {
                (0..ARTICLES)
                    .filter_map(|k| block.get(k * ARTICLE..(k + 1) * ARTICLE))
                    .filter(|a| a[0] != 0)
                    .fold(0i32, |s, a| {
                        s.wrapping_add(i32::from_le_bytes(
                            a[WEEKLY_SALES..WEEKLY_SALES + 4].try_into().unwrap(),
                        ))
                    })
            });
        let weeks = SEASON_WEEKS - (self.today / 7) as i32;
        let v = weekly.wrapping_mul(weeks).wrapping_add(self.i32_(0x30c)) as u32;
        i64::from(v.max(self.u32_(0x204)) as i32)
    }

    /// Merchandise sold, its booked takings added to themselves
    /// extrapolated (`FUN_0062130f`).
    fn merchandise_extrapolated(&self) -> i64 {
        let v = i64::from(self.i32_(0x30c));
        if self.team.get::<u8>(0x5a9 + finances::CLUB_TO_TEAM) != 0 {
            return v;
        }
        let v = if self.months_left == YEAR {
            v
        } else {
            let q = v / (YEAR - self.months_left);
            v + q * YEAR + if self.month >= JULY { q % YEAR } else { 0 }
        };
        v / ROUND * ROUND
    }

    /// A person's yearly pay: from the persons, or the second table.
    fn person_pay(&self, id: i16) -> i32 {
        if id <= 0 {
            return 0;
        }
        self.records.persons.get(id as usize).map_or(0, |p| p.get::<i32>(PERSON_PAY))
    }

    fn other_person_pay(&self, id: i16) -> i32 {
        if !(1..=OTHER_PERSONS).contains(&id) {
            return 0;
        }
        self.records
            .block(&format!("{OTHER_PERSONS_AT:#x}[{id}]"))
            .and_then(|b| b.get(OTHER_PERSON_PAY..OTHER_PERSON_PAY + 4))
            .map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))
    }

    /// What the staff are still to be paid: the trainer, the manager, the
    /// co-trainer and one more (`FUN_00620453`, `FUN_0062053b`,
    /// `FUN_0062068a`, `FUN_00620702`).
    fn wages(&self) -> i32 {
        let [human_trainer, human_manager] = self.team.humans();
        let [manager, trainer, other_manager] = self.team.staff().map(|id| id as i16);
        let pay = self.manager.map_or(0, |m| m.get::<i32>(MANAGER_PAY));
        let trainer = if human_trainer > 0 { pay } else { self.person_pay(trainer) };
        let manager = if human_manager > 0 {
            if self.manager.is_some_and(|m| m.status() == MANAGER_ONLY) { 0 } else { pay }
        } else if manager != 0 {
            self.person_pay(manager)
        } else {
            self.person_pay(other_manager)
        };
        let co_trainer = self.person_pay(self.team.co_trainer());
        let other = self.other_person_pay(self.team.get::<i16>(0xa4));
        [trainer, manager, co_trainer, other].iter().fold(0, |s, &p| s.wrapping_add(self.rest(p)))
    }

    /// The staff carried on: the plan's year and the wages still to pay,
    /// or the plan's posts if more (`FUN_006207aa`).
    fn staff_carried_on(&self) -> i64 {
        let planned = self.i32_(0x178).wrapping_add(self.wages());
        let posts = [0x154, 0x158, 0x15c, 0x160, 0x174, 0x164, 0x168, 0x16c, 0x170]
            .iter()
            .fold(0i32, |s, &c| s.wrapping_add(self.i32_(c)))
            .wrapping_add(
                i32::from(self.team.get::<u8>(0x1f8 + finances::CLUB_TO_TEAM)) * STAFF_UNIT,
            );
        i64::from(planned.max(posts))
    }

    /// The staff extrapolated (`FUN_006214ba`).
    fn staff_extrapolated(&self) -> i64 {
        let v = if self.months_left == YEAR {
            i64::from(self.i32_(0x178))
        } else {
            let q = i64::from(self.i32_(0x280)) / (YEAR - self.months_left);
            q * YEAR + if self.month >= JULY { q % YEAR } else { 0 }
        };
        v / ROUND * ROUND
    }

    /// The players' pay booked: wages and bonuses, less what came back.
    fn players_booked(&self) -> i32 {
        self.i32_(0x284).wrapping_sub(self.i32_(0x28c)).wrapping_add(self.i32_(0x288)).max(0)
    }

    /// The players carried on: their pay and bonuses for the months left,
    /// or the plan's if more (`FUN_006202c1`, `FUN_0062023c`,
    /// `FUN_0061f222`).
    fn players_carried_on(&self) -> i64 {
        let bill = finances::wage_bill(self.records, self.team, self.teams);
        let v = self
            .players_booked()
            .wrapping_add(self.rest(bill))
            .wrapping_add(self.i32_(0x290))
            .wrapping_add(self.i32_(0x294));
        let planned = self.sum(&self.plan, finances::PLAYERS_SLOT);
        i64::from(if v <= planned { planned } else { v })
    }

    /// The players extrapolated: only their pay for the months left
    /// (`FUN_00621548`).
    fn players_extrapolated(&self) -> i64 {
        i64::from(
            self.rest(self.players_booked())
                .wrapping_add(self.i32_(0x290))
                .wrapping_add(self.i32_(0x294)),
        )
    }

    /// Match and training costs: booked, and the plan's for the months
    /// left (`FUN_0062039b`).
    fn operations(&self) -> i32 {
        let planned =
            [0x194, 0x198, 0x19c, 0x1a0].iter().fold(0i32, |s, &c| s.wrapping_add(self.i32_(c)));
        self.i32_(0x29c)
            .wrapping_add(self.i32_(0x2a4))
            .wrapping_add(self.i32_(0x2a8))
            .wrapping_add(self.rest(planned))
    }

    /// Match and training costs, at least the plan's (`FUN_006215d5`).
    fn operations_extrapolated(&self) -> i64 {
        let o = self.operations();
        let least = self.i32_(0x194).wrapping_add(self.i32_(0x19c)).wrapping_add(self.i32_(0x1a0));
        i64::from(if least < o { o } else { least }.wrapping_add(self.i32_(0x2ac)))
    }

    /// The plan's tickets: its league gates, cup, European and other
    /// (`FUN_0061f427`).
    pub fn planned_tickets(&self) -> i64 {
        i64::from(self.gates.planned)
    }

    /// The finance screen's "Fortführung": the season carried on.
    pub fn carried_on(&self) -> [i64; SLOTS] {
        let (p, b) = (&self.plan, &self.booked);
        let stadium = p.stadium().max(b.stadium());
        let stadium_income = self.i32_(0x1d8).max(self.i32_(0x2e0));
        [
            i64::from(self.gates.carried_on),
            i64::from(self.larger(finances::TV_SLOT)),
            i64::from(self.larger(finances::PRIZES_SLOT)),
            self.sponsors_carried_on(),
            i64::from(self.u32_(0x308)),
            self.merchandise_carried_on(),
            self.i64_(0x478),
            self.i64_(0x470),
            stadium,
            i64::from(stadium_income),
            self.staff_carried_on(),
            self.players_carried_on(),
            i64::from(self.operations().wrapping_add(self.i32_(0x2ac))),
            0,
            i64::from(self.larger(finances::OTHER_COSTS_SLOT)),
            i64::from(self.larger(finances::OTHER_INCOME_SLOT)),
            i64::from(self.i32_(0x1c8)),
            i64::from(self.i32_(0x1cc)),
        ]
    }

    /// The finance screen's "Prognose": the season extrapolated.
    pub fn extrapolated(&self) -> [i64; SLOTS] {
        let b = &self.booked;
        let round = |v: i32| i64::from(v) / ROUND * ROUND;
        [
            i64::from(self.gates.extrapolated),
            round(self.larger(finances::TV_SLOT)),
            round(self.larger(finances::PRIZES_SLOT)),
            self.sponsors_extrapolated(),
            self.to_season(i64::from(self.u32_(0x308))),
            self.merchandise_extrapolated(),
            self.i64_(0x478),
            self.i64_(0x470),
            b.stadium(),
            self.to_season(i64::from(self.u32_(0x2e0))),
            self.staff_extrapolated(),
            self.players_extrapolated(),
            self.operations_extrapolated(),
            0,
            self.to_season(i64::from(self.sum(b, finances::OTHER_COSTS_SLOT))),
            self.to_season(i64::from(self.sum(b, finances::OTHER_INCOME_SLOT))),
            i64::from(self.i32_(0x2d0)),
            i64::from(self.i32_(0x2d4)),
        ]
    }

    /// The balance sheet's "wie bisher": the season carried on, without
    /// the shares (`FUN_0062218e`).
    pub fn so_far(&self) -> i64 {
        let s = self.carried_on();
        finances::total(&s)
            .wrapping_sub(s[finances::SHARES_SLOT])
            .wrapping_add(s[finances::DIVIDENDS_SLOT])
    }

    /// The balance sheet's "wie kalkuliert": the season extrapolated, its
    /// first four incomes summed in 32 bits, the merchandise's takings
    /// extrapolated alone, without the shares (`FUN_00622326`).
    pub fn as_planned(&self) -> i64 {
        let s = self.extrapolated();
        let first = (0..4).fold(0i32, |sum, i| sum.wrapping_add(s[i] as i32));
        let merchandise = self.to_season(i64::from(self.u32_(0x30c)));
        let mut t = i64::from(first);
        t = t.wrapping_sub(s[4]).wrapping_add(merchandise);
        for (slot, sign) in
            [(6, -1), (7, 1), (8, -1), (9, 1), (10, -1), (11, -1), (12, -1), (14, -1), (15, 1)]
        {
            t = t.wrapping_add(s[slot].wrapping_mul(sign));
        }
        t
    }
}

/// Whether a top-league club's place earns the sponsors' bonus for a
/// European place: second to as far as the country's ranking reaches,
/// unless it won the cup (`FUN_006211e8`). The game looks the country up
/// by a slip, under the key of the country in its slot.
fn european_place(records: &Records, team: &Team, tables: &TicketTables) -> bool {
    let country = team.country();
    let key = records.countries.ranking_key(usize::from(country));
    let ranking = records.globals.ranking();
    let row = (0..RANKED)
        .find(|&i| i16::from_le_bytes([ranking[i * RANK_ROW], ranking[i * RANK_ROW + 1]]) == key)
        .unwrap_or(RANKED);
    let places = match row {
        0..3 => 4,
        3..9 => 3,
        _ => 2,
    };
    let winner = tickets::cup_winner(records, country, tables);
    let place = team.table_place();
    winner != Some(u16::from(team.number()))
        && team.level() == 0
        && (2..=places + 1).contains(&place)
}

/// The ranking's countries, and how long a row is.
const RANKED: usize = 15;
const RANK_ROW: usize = 0x24;
