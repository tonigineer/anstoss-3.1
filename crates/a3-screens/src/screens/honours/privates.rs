//! The manager's private life (dialog 443 `CDiPrivatesTrainer`;
//! ERFOLGE.md §B.1): the wife and children his records kept, his wealth,
//! and the three rows of what he lives in, drives and does — each one a
//! step better for the price of the next step, paid off his wealth at
//! once. What the household costs every month stands by itself.

use chrono::NaiveDate;

use a3_core::records::Manager;
use a3_files::rch::Dialog;

use crate::controls::bitmap::Picture;
use crate::pointer::Target;
use crate::screens::finances::printf;
use crate::screens::playerinfo::{age, money};
use crate::screens::kalkulation::own_manager;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, caption, control, mark_inert, set};
use crate::view::View;

use super::trainer_erfolge::TrainerErfolge;

pub(crate) const PRIVATES_DIALOG: u16 = 443;
const CAPTION_ID: u16 = 9946;
/// The wife: her name, her age, and her picture at (121,169).
const WIFE_NAME: u16 = 5599;
const WIFE_AGE: u16 = 5607;
const WIFE_PICTURE: u16 = 5592;
/// The five children: their names and their ages.
const CHILD_NAMES: [u16; 5] = [5593, 5594, 5595, 5603, 5604];
const CHILD_AGES: [u16; 5] = [5596, 5597, 5598, 5605, 5606];
/// The wealth, and what the household costs a month.
const WEALTH: u16 = 5282;
const COSTS: u16 = 6487;
/// The three rows: the flat, the car and the hobby, each with the button
/// that buys the next step.
const ROWS: [(u16, u16); 3] = [(5600, 5609), (5601, 5608), (5602, 5610)];
/// What he has, by the level: the flat from one step up, the car from
/// one step up, the hobby from none — the first hobby is what he does
/// for free.
const FLAT_TEXTS: [u16; 7] = [4525, 4526, 4527, 4528, 4529, 4530, 4531];
const CAR_TEXTS: [u16; 7] = [4518, 4519, 4520, 4521, 4522, 4523, 4524];
const HOBBY_TEXTS: [u16; 9] = [4629, 4532, 4533, 4534, 4535, 4536, 4537, 4538, 4539];
/// "Golfausrüstung kaufen" and its like, by the level.
const HOBBY_BUY_TEXTS: [u16; 8] = [4540, 4541, 4542, 4543, 4544, 4545, 4546, 4547];
/// "Preis", and "Alles erreicht!" once the last step is bought.
const PRICE: u16 = 4630;
const REACHED: u16 = 4548;
/// What the next step costs (the tables at `0x886588`, `0x8865b4` and
/// `0x8865f4` as doubles, times the difficulty's factor — at normal play
/// that is 1.0).
const FLAT_PRICES: [i64; 7] = [200_000, 400_000, 600_000, 1_000_000, 1_500_000, 3_000_000, 10_000_000];
const CAR_PRICES: [i64; 7] = [20_000, 40_000, 60_000, 100_000, 250_000, 500_000, 2_000_000];
const HOBBY_PRICES: [i64; 8] =
    [20_000, 200_000, 500_000, 2_000_000, 5_000_000, 10_000_000, 20_000_000, 100_000_000];
/// The household's cost by month (`FUN_006a5fd8`): 4,000 up to level 9,
/// 7,500 from level 10, half again for a wife. The figure shown is never
/// touched by what he buys.
const COSTS_BASE: i64 = 4000;
const COSTS_GOOD: i64 = 7500;
const COSTS_LEVEL: i32 = 10;
/// The wife's pictures (`0x88663e`), by her looks; the single man's one.
const WIFE_PICTURES: u16 = 3122;
const WIFE_ALONE: u16 = 3178;
/// The private block (`SAVEGAME.md`, 40 bytes at +0x1c4): the wife's
/// picture, her first name's number plus one and the pool of a nation's
/// names it comes from, her birthday, and the three levels from +0xc.
const PRIVATE_AT: usize = 0x1c4;
const WIFE_NUMBER_AT: usize = 0x1c6;
const WIFE_POOL_AT: usize = 0x1c7;
const WIFE_BORN_AT: usize = 0x1c8;
const LEVELS_AT: usize = 0x1d0;
/// The wealth, and the manager's own person entry: the sixth last player.
const WEALTH_AT: usize = 0x118;
const MANAGER_PERSON: u16 = 17_000;
/// A person's children: their count, the names from his nation's pool,
/// and the birthdays of up to five, four bytes each.
const CHILD_BIRTH_AT: usize = 0x174;

/// The screen.
#[derive(Default)]
pub struct Privates {
    /// The three levels: copies of the block's, which the record gets
    /// with every step bought. The original keeps them in the window
    /// until it closes (`FUN_006a5d9f`), which the player sees no
    /// difference in — the wealth is gone at once either way.
    levels: [u8; 3],
}

/// The manager's record, to change.
fn manager_mut<'a>(cx: &'a mut Cx) -> Option<&'a mut Manager> {
    let current = cx.state.season.as_ref()?.current;
    let records = cx.state.records.as_mut()?;
    let slot = records.human_slot(current)?;
    records.managers.get_mut(slot)
}

pub(crate) fn compose(dialogs: &mut std::collections::HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&PRIVATES_DIALOG) else { return };
    dialog.controls.insert(0, caption(dialog.frame, CAPTION_ID, String::new()));
}

impl Screen for Privates {
    fn ids(&self) -> &[u16] {
        &[PRIVATES_DIALOG]
    }

    /// `FUN_00533064`'s OnInitDialog.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(season) = cx.state.season.as_ref() else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(manager) = own_manager(cx) else { return };
        let words = &game.rules.words;
        let today = season.date();
        set(
            view,
            dialog,
            CAPTION_ID,
            printf(
                &words.private_caption,
                &[&dialog.title, &manager.first_name(), &manager.last_name()],
            ),
        );
        set(view, dialog, WEALTH, money(game, manager.get::<i32>(WEALTH_AT).into()));

        // The wife: her number names her in one of the pools of first
        // names. The original keeps eight of them (`FUN_00674fb9` fills
        // `DAT_00b9f048` from the executable's table of 800 names, and
        // `FUN_0067503a` reads `pool * 100 + number` from it). A pool
        // past the eighth reads past the table into unrelated memory —
        // the "Thomas" an old note saw for pool 14 is just string 46
        // of the original's own table — here the name stays empty.
        // Without a wife the "ledig" and "---" of the file stay.
        let block = manager.get::<[u8; 40]>(PRIVATE_AT);
        self.levels = [block[0xc], block[0xd], block[0xe]];
        let picture = |view: &mut View, id: u16| {
            if let Some(c) = control(dialog, WIFE_PICTURE) {
                view.set_picture(c, Some(Picture::graphic(id)));
            }
        };
        let number = manager.get::<u8>(WIFE_NUMBER_AT);
        if number == 0 {
            picture(view, WIFE_ALONE);
        } else {
            let pool = usize::from(manager.get::<u8>(WIFE_POOL_AT));
            let name = words
                .first_names
                .get(pool * 100 + usize::from(number - 1))
                .cloned()
                .unwrap_or_default();
            set(view, dialog, WIFE_NAME, name);
            let born = NaiveDate::from_ymd_opt(
                i32::from(manager.get::<u16>(WIFE_BORN_AT + 2)),
                u32::from(manager.get::<u8>(WIFE_BORN_AT + 1)),
                u32::from(manager.get::<u8>(WIFE_BORN_AT)),
            );
            set(view, dialog, WIFE_AGE, age(born, today).to_string());
            let looks = i32::from(manager.get::<i16>(PRIVATE_AT)).clamp(0, 6);
            picture(view, WIFE_PICTURES + looks as u16);
        }

        // The children of his own person entry: months old until their
        // nineteenth month, whole years from there.
        if let Some(slot) = records.human_slot(season.current)
            && let Some(person) = records.player(MANAGER_PERSON + u16::try_from(slot + 1).unwrap_or(0))
        {
            let count = usize::from(person.children().max(0) as u8).min(CHILD_NAMES.len());
            let [_, month, year] = records.globals.date();
            let nation = usize::from(person.names_nation());
            let names = person.children_names();
            for (k, (name_id, age_id)) in CHILD_NAMES.iter().zip(CHILD_AGES).enumerate() {
                if k >= count {
                    set(view, dialog, *name_id, String::new());
                    set(view, dialog, age_id, String::new());
                    continue;
                }
                let name = words
                    .first_names
                    .get(nation * 100 + usize::from(names[k]))
                    .cloned()
                    .unwrap_or_default();
                let birth = person.get::<[u8; 4]>(CHILD_BIRTH_AT + 4 * k);
                let months = i32::from(year) - i32::from(u16::from_le_bytes([birth[2], birth[3]]));
                let months = months * 12 + i32::from(month) - i32::from(birth[1]) + 1;
                let shown = if months < 2 {
                    words.month_one.clone()
                } else if months < 19 {
                    words.month_many.replace("%d", &months.to_string())
                } else {
                    let born = NaiveDate::from_ymd_opt(
                        i32::from(u16::from_le_bytes([birth[2], birth[3]])),
                        u32::from(birth[1]),
                        u32::from(birth[0]),
                    );
                    age(born, today).to_string()
                };
                set(view, dialog, *name_id, name);
                set(view, dialog, age_id, shown);
            }
        }

        // What the household costs: half again for a wife, whatever he
        // has or has bought.
        let mut costs = if TrainerErfolge::shown_level(manager) >= COSTS_LEVEL {
            COSTS_GOOD
        } else {
            COSTS_BASE
        };
        if number != 0 {
            costs += costs / 2;
        }
        set(view, dialog, COSTS, money(game, costs));

        for row in 0..ROWS.len() {
            self.show_row(view, cx, row);
        }
        // Everything here works: at the last step the button still
        // presses, the original just does nothing.
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Button(c) => {
                let row = match dialog.controls[c].id {
                    Some(5609) => 0,
                    Some(5608) => 1,
                    Some(5610) => 2,
                    _ => return Outcome::Stay,
                };
                self.buy(cx, view, row);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

impl Privates {
    /// One row: what he has, and what the next step costs (`FUN_00533a94`).
    fn show_row(&self, view: &mut View, cx: &Cx, row: usize) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (thing, button) = ROWS[row];
        let level = usize::from(self.levels[row].min(8));
        let texts: &[u16] = match row {
            0 => &FLAT_TEXTS,
            1 => &CAR_TEXTS,
            _ => &HOBBY_TEXTS,
        };
        let prices: &[i64] = match row {
            0 => &FLAT_PRICES,
            1 => &CAR_PRICES,
            _ => &HOBBY_PRICES,
        };
        // What he owns: the hobby text always stands, its first being
        // the free one he does anyway; the flat and the car keep the
        // file's "Miete" and "Rostlaube" until he buys, and show the
        // step of their level from there.
        if row == 2 {
            set(view, dialog, thing, game.text(texts[level]).to_owned());
        } else if level > 0 {
            set(view, dialog, thing, game.text(texts[level - 1]).to_owned());
        }
        // The button names the step he could buy next and its price;
        // past the last step it only says so.
        let label = match prices.get(level) {
            Some(&price) if row == 2 => printf(
                &game.rules.words.purchase_hobby,
                &[&game.text(HOBBY_BUY_TEXTS[level]), &money(game, price)],
            ),
            Some(&price) => printf(
                &game.rules.words.purchase,
                &[
                    &game.text(texts[level]),
                    &game.text(PRICE),
                    &money(game, price),
                ],
            ),
            None => game.text(REACHED).to_owned(),
        };
        set(view, dialog, button, label);
    }

    /// A step up for its price (`FUN_005333ca`, `0x5334aa`, `0x53358a`):
    /// without the money for it, nothing at all changes.
    fn buy(&mut self, cx: &mut Cx, view: &mut View, row: usize) {
        let prices: &[i64] = match row {
            0 => &FLAT_PRICES,
            1 => &CAR_PRICES,
            _ => &HOBBY_PRICES,
        };
        let Some(&price) = prices.get(usize::from(self.levels[row].min(8))) else {
            return;
        };
        let paid = {
            let Some(manager) = manager_mut(cx) else { return };
            let wealth = manager.get::<i32>(WEALTH_AT);
            if i64::from(wealth) < price {
                return;
            }
            manager.put(WEALTH_AT, (i64::from(wealth) - price) as i32);
            manager.put(LEVELS_AT + row, self.levels[row] + 1);
            manager.get::<i32>(WEALTH_AT)
        };
        self.levels[row] += 1;
        set(view, cx.dialog, WEALTH, money(cx.game, paid.into()));
        self.show_row(view, cx, row);
    }
}
