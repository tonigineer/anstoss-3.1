//! The manager's own page ("Persönliche Erfolge", dialog 175
//! `CScoreKohle`; ERFOLGE.md in the RE repo): his contract and what it
//! pays, his national team, nerves, marks, age and image, the board's
//! confidence, his private wealth and his shares, with two sliders that
//! move money into the club's books and into the black till. The nine
//! pages. Seven of the nine are built — his own career with its titles
//! and stations, his private life, his abilities, his pay, how popular
//! he is, his season records and the highscores —; the player-manager's
//! page and the comparison with other managers are not.

use std::collections::HashMap;

use a3_core::cycle::dialogs::HONOURS;
use a3_core::season::Rules;
use a3_files::rch::Dialog;

use super::hub::{self, HONOURS_SYMBOL};
use super::playerinfo::money;
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::options::NONE;
use crate::pointer::Target;
use crate::view::View;

pub mod abilities;
pub mod beliebtheit;
pub mod pay;
pub mod privates;
pub mod scores;
pub mod season_records;
pub mod trainer_erfolge;

const CAPTION_ID: u16 = 9935;
/// "ANSTOSS 3 - Persönliche Erfolge", with the manager, his club and the
/// date.
const CAPTION: u16 = 2526;
const OK: u16 = 1;
/// The contract's figures, the season's goal, how long it runs and the
/// two "Ja/Nein" lines.
const SALARY: u16 = 5559;
const A_POINT: u16 = 5560;
const A_CUP_ROUND: u16 = 5561;
const A_TITLE: u16 = 5562;
const GOAL: u16 = 5563;
const GOAL_BONUS: u16 = 5564;
const EXIT: u16 = 5565;
const RUNS: u16 = 5576;
const OPTION: u16 = 5577;
/// The national team's nation, salary and bonus.
const NATION: u16 = 5566;
const NATION_SALARY: u16 = 5567;
const NATION_BONUS: u16 = 5578;
/// Nerves, the marks' average, the age, the image, the confidence, the
/// wealth and next season's club.
const NERVES: u16 = 5568;
const AVERAGE: u16 = 5569;
const AGE: u16 = 5570;
const IMAGE: u16 = 5579;
const CONFIDENCE: u16 = 2774;
const WEALTH: u16 = 5282;
const NEXT_CLUB: u16 = 5573;
/// The shares, the two sliders with their amounts and OKs, and the
/// groups.
const SHARES: u16 = 5261;
const TO_BOOKS: (u16, u16, u16) = (5533, 5535, 5555);
const TO_TILL: (u16, u16, u16) = (5574, 5575, 5588);
const PAGES: u16 = 5643;
const CONFIDENCE_PAGE: u16 = 5571;
const MARKETS: u16 = 5572;
/// Item 3 of the pages, whose label changes for a player-manager.
const PLAYER_MANAGER: usize = 3;
/// The dialog each page button opens, by the item (ERFOLGE.md §5): the
/// career, the privates, the abilities, the player-manager's page, the
/// pay, the popularity, the comparison, the season records and the
/// highscores. Those not built are `None`.
const PAGE_DIALOGS: [Option<u16>; 9] = [
    Some(trainer_erfolge::TRAINER_DIALOG),
    Some(privates::PRIVATES_DIALOG),
    Some(abilities::ABILITIES_DIALOG),
    None,
    Some(pay::PAY_DIALOG),
    Some(beliebtheit::BELIEBT_DIALOG),
    None,
    Some(season_records::SEASON_RECORDS_DIALOG),
    Some(scores::SCORES_DIALOG),
];
const STEP_DOWN: u16 = 4246;
/// "Ja" / "Nein"; "(bis 30.06.%4d)"; the season's goals 1069 to 1080.
const YES_NO: [u16; 2] = [1028, 1029];
const UNTIL: u16 = 1596;
const GOALS: u16 = 1069;
const GOAL_KINDS: u8 = 12;
/// The manager's images (`0x887af8`), by his record's image.
const IMAGES: [u16; 18] = [
    3755, 3740, 3741, 3742, 3743, 3744, 3745, 3746, 3747, 3748, 3749, 3754, 3750, 3751, 3752, 3753,
    3739, 4002,
];
/// The manager's record: his club, nerves, the marks' sum and count,
/// whether his age shows, his birthday, next season's club, the
/// contract's five figures, its years, the club's option, the exit
/// clause, his wealth, the national team with its pay,
/// his image, and whether he plays.
const CLUB_AT: usize = 0x1e;
const NERVES_AT: usize = 0x11e;
const MARKS_AT: usize = 0x120;
const AGE_SHOWN_AT: usize = 0x12a;
const AGE_SHOWN: i16 = 9;
const BORN_AT: usize = 0x2e;
const NEXT_CLUB_AT: usize = 0x130;
const SALARY_AT: usize = 0x15c;
const YEARS_AT: usize = 0x172;
const OPTION_AT: usize = 0x174;
const EXIT_AT: usize = 0x178;
const WEALTH_AT: usize = 0x118;
const NATION_AT: usize = 0x1f72;
const NATION_PAY_AT: usize = 0x1f7c;
const IMAGE_AT: usize = 0x1f90;
const PLAYS_AT: usize = 0x2e8;
/// The club's season goal (team +0x136).
const GOAL_AT: usize = 0x136;
/// The sliders' scale (`0x887aa8`): a hundred a step to 10,000, a
/// thousand to 100,000, then ten thousand.
const SCALE: [(i32, i32); 3] = [(10_000, 100), (100_000, 1000), (i32::MAX, 10_000)];
const NOTHING: &str = "-";

/// Where an amount sits on a slider (`FUN_005c690b`).
fn place(v: i32) -> i32 {
    let (mut place, mut from) = (0, 0);
    for (to, step) in SCALE {
        if v <= to {
            return place + (v - from) / step;
        }
        place += (to - from) / step;
        from = to;
    }
    place
}

/// What a place on a slider stands for (`FUN_005c69c9`).
fn amount(p: i32) -> i32 {
    let (mut place, mut from) = (0, 0);
    for (to, step) in SCALE {
        let steps = (to - from) / step;
        if p <= place + steps {
            return from + (p - place) * step;
        }
        place += steps;
        from = to;
    }
    from
}

pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&HONOURS) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    hub::add_bar(dialog, rules);
}

/// The screen.
#[derive(Default)]
pub struct Honours {
    whistle: bool,
}

impl Screen for Honours {
    fn ids(&self) -> &[u16] {
        &[HONOURS]
    }

    /// `0x53d5cf`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        hub::open_bar(cx, view, HONOURS_SYMBOL);
        if hub::last_day(cx) {
            hub::switch_off(cx, view, 4);
        }
        if let Some(season) = cx.state.season.as_ref() {
            set(view, dialog, CAPTION_ID, hub::manager_title(game, season, CAPTION));
        }
        let Some(manager) = super::kalkulation::own_manager(cx) else { return };
        let m = |v: i32| money(game, v.into());
        let yes_no = |on: bool| game.text(YES_NO[usize::from(!on)]).to_owned();
        if manager.get::<u16>(CLUB_AT) != 0 {
            for (k, id) in [SALARY, A_POINT, A_CUP_ROUND, A_TITLE, GOAL_BONUS].iter().enumerate() {
                set(view, dialog, *id, m(manager.get::<i32>(SALARY_AT + 4 * k)));
            }
            let goal = own_team(cx).map_or(0, |t| t.get::<u8>(GOAL_AT));
            let text = if goal < GOAL_KINDS {
                game.text(GOALS + u16::from(goal)).to_owned()
            } else {
                goal.to_string()
            };
            set(view, dialog, GOAL, text);
            let year = cx.game.world.start_year + i32::from(manager.get::<i16>(YEARS_AT));
            set(view, dialog, RUNS, game.text(UNTIL).replacen("%4d", &year.to_string(), 1));
            set(view, dialog, OPTION, yes_no(manager.get::<i16>(OPTION_AT) != 0));
            set(view, dialog, EXIT, yes_no(manager.get::<u8>(EXIT_AT) != 0));
            set(view, dialog, CONFIDENCE, format!("{} %", manager.shown_confidence()));
        } else {
            for id in [
                SALARY,
                A_POINT,
                A_CUP_ROUND,
                A_TITLE,
                GOAL_BONUS,
                GOAL,
                RUNS,
                OPTION,
                EXIT,
                CONFIDENCE,
            ] {
                set(view, dialog, id, NOTHING.to_owned());
            }
        }
        // Nerves and the marks' average; without marks the average keeps
        // the nerves' own text, as the original leaves its buffer.
        let nerves = manager.get::<i16>(NERVES_AT).to_string();
        let (sum, count) = (manager.get::<i16>(MARKS_AT), manager.get::<i16>(MARKS_AT + 2));
        let average = if count != 0 { (sum / count).to_string() } else { nerves.clone() };
        set(view, dialog, NERVES, nerves);
        set(view, dialog, AVERAGE, average);
        let age = if manager.get::<i16>(AGE_SHOWN_AT) == AGE_SHOWN {
            let born = chrono::NaiveDate::from_ymd_opt(
                i32::from(manager.get::<u16>(BORN_AT + 2)),
                u32::from(manager.get::<u8>(BORN_AT + 1)),
                u32::from(manager.get::<u8>(BORN_AT)),
            );
            let today = cx.state.season.as_ref().map(a3_core::season::Season::date);
            born.zip(today).and_then(|(b, t)| t.years_since(b)).map(|y| y.to_string())
        } else {
            None
        };
        set(view, dialog, AGE, age.unwrap_or_else(|| NOTHING.to_owned()));
        let image = usize::from(manager.get::<u16>(IMAGE_AT)).min(IMAGES.len() - 1);
        set(view, dialog, IMAGE, game.text(IMAGES[image]).to_owned());
        set(view, dialog, WEALTH, m(manager.get::<i32>(WEALTH_AT)));
        if manager.get::<u8>(NEXT_CLUB_AT) != 0 {
            let country = manager.get::<u8>(NEXT_CLUB_AT);
            let number = manager.get::<i16>(NEXT_CLUB_AT + 2);
            let name = game.club_name(cx.state.records.as_ref(), country, number);
            set(view, dialog, NEXT_CLUB, name);
        }
        let nation = manager.get::<i16>(NATION_AT);
        if nation != 0 {
            set(view, dialog, NATION, game.world.nation(nation.max(0) as u16).to_owned());
            set(view, dialog, NATION_SALARY, m(manager.get::<i32>(NATION_PAY_AT)));
            set(view, dialog, NATION_BONUS, m(manager.get::<i32>(NATION_PAY_AT + 4)));
        } else {
            for id in [NATION, NATION_SALARY, NATION_BONUS] {
                set(view, dialog, id, NOTHING.to_owned());
            }
        }
        // The sliders start at nothing, and reach as far as the money
        // they move.
        let wealth = manager.get::<i32>(WEALTH_AT);
        for (bar, amount_id, ok) in [TO_BOOKS, TO_TILL] {
            if let Some(v) = control(dialog, bar).and_then(|c| view.value_mut(c)) {
                v.range = (0, place(wealth));
                v.set(0);
            }
            set(view, dialog, amount_id, m(0));
            if let Some(i) = control(dialog, ok) {
                view.controls[i].hidden = true;
            }
        }
        if let Some(list) = control(dialog, SHARES).and_then(|c| view.list_mut(c)) {
            list.rows = Vec::new();
            list.selected = None;
        }
        if manager.get::<i16>(PLAYS_AT) != 0
            && let Some(options) = control(dialog, PAGES).and_then(|c| view.options_mut(c))
            && let Some(label) = options.labels.get_mut(PLAYER_MANAGER)
        {
            *label = game.text(STEP_DOWN).to_owned();
        }
        for id in [PAGES, CONFIDENCE_PAGE, MARKETS] {
            if let Some(c) = control(dialog, id) {
                view.set_chosen(c, NONE);
            }
        }
        // The two transfers and the built pages are open.
        let works = |t: Target| match t {
            Target::Option { control: c, item } if dialog.controls[c].id == Some(PAGES) => {
                PAGE_DIALOGS.get(item).is_some_and(Option::is_some)
            }
            Target::Option { control: c, .. }
                if matches!(dialog.controls[c].id, Some(CONFIDENCE_PAGE | MARKETS)) =>
            {
                false
            }
            _ => hub::on_bar(dialog, t).is_none_or(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (game, dialog) = (cx.game, cx.dialog);
        if let Some(item) = hub::on_bar(dialog, target) {
            let outcome = hub::bar_click(cx, view, item, HONOURS_SYMBOL);
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        self.whistle = false;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. } => {
                let (_, amount_id, ok) = if id(c) == TO_BOOKS.0 { TO_BOOKS } else { TO_TILL };
                let v = view.value(c).map_or(0, |v| v.value);
                let moved = amount(v);
                set(view, dialog, amount_id, money(game, moved.into()));
                if let Some(i) = control(dialog, ok) {
                    view.controls[i].hidden = moved <= 0;
                }
                Outcome::Stay
            }
            // The page's button stays pressed while it is open.
            Target::Option { control: c, item }
                if dialog.controls[c].id == Some(PAGES) =>
            {
                match PAGE_DIALOGS.get(item).copied().flatten() {
                    Some(id) => Outcome::Popup(a3_core::cycle::Request::new(id)),
                    None => {
                        view.set_chosen(c, NONE);
                        Outcome::Stay
                    }
                }
            }
            Target::Option { control: c, .. } => {
                view.set_chosen(c, NONE);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }

    /// A page taken in: its button comes up again, and the wealth and
    /// the confidence show what the page may have changed (`FUN_0053e4d4`
    /// runs anew — the privates spend their wealth right here).
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        if let Some(c) = control(cx.dialog, PAGES) {
            view.set_chosen(c, NONE);
        }
        if let Some(manager) = super::kalkulation::own_manager(cx) {
            set(view, cx.dialog, WEALTH, money(cx.game, manager.get::<i32>(WEALTH_AT).into()));
            let confidence = if manager.get::<u16>(CLUB_AT) != 0 {
                format!("{} %", manager.shown_confidence())
            } else {
                NOTHING.to_owned()
            };
            set(view, cx.dialog, CONFIDENCE, confidence);
        }
        Outcome::Stay
    }
}
