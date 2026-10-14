//! What a club's finances come to (`FUN_00621eac`, the RE repo's
//! FINANZEN-S.md): its cash and share proceeds, what it has booked this
//! season as the finance screen's columns count it, less its loans, plus
//! what its deposits are worth now. The game bands it for the opponent's
//! info and weighs it in offers and the board's verdict.

use crate::records::Team;

/// The club object's fields sit this far before the team record's.
const CLUB_TO_TEAM: usize = 0x278;
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

/// The figure, in the game's money (`FUN_00621eac`), with its quirks: one
/// income counted twice, some read unsigned, 32-bit sums wrapping.
pub fn figure(team: &Team, fund_prices: [i32; 5]) -> i64 {
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
    f = f.wrapping_add(i32_(0x258)); // transfers, net
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

    let mut v = i64_(0x210).wrapping_add(i32_(0x2d0)).wrapping_add(f);
    for k in 0..LOANS {
        let loan = 0x318 + ENTRY * k;
        if team.get::<u8>(at(loan + 0x0f)) & 1 != 0 {
            v = v.wrapping_sub(i64_(loan));
        }
    }
    for k in 0..DEPOSITS {
        let deposit = 0x3d8 + ENTRY * k;
        if team.get::<u8>(at(deposit + 0x13)) & 1 != 0 {
            let fund = usize::from(team.get::<u8>(at(deposit + 0x12)));
            let price = i64::from(fund_prices.get(fund).copied().unwrap_or(0));
            let bought = i64::from(team.get::<i16>(at(deposit + 0x10)));
            if bought != 0 {
                v = v.wrapping_add(i64_(deposit + 8).wrapping_mul(price) / bought);
            }
        }
    }
    v
}
