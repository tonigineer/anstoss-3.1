//! The second page, "Saison" (dialog 402, filled by `FUN_00560d30`): his
//! season so far — goals, cards and suspensions in the league, the cup and
//! the European cups, his injury, his form week by week, and strips of his
//! league matchdays: grades, appearances, goals and cards.
//!
//! Some of it only for a country whose matches the game follows; else a
//! dash.

use a3_core::records::Player as Record;

use super::current::{INJURY, injury_text};
use super::{Shown, dialog_text, one_decimal};
use crate::controls::paint::PaintState;
use crate::view::View;

const GOALS: u16 = 4854;
const ASSISTS: u16 = 4855;
const CUP_GOALS: u16 = 4856;
const EUROPE_GOALS: u16 = 4857;
const PENALTIES: u16 = 4858;
const FREE_KICKS: u16 = 4859;
const JOKER: u16 = 4860;
const OWN_GOALS: u16 = 4861;
const APPEARANCES: u16 = 4862;
const OF_THE_DAY: u16 = 4863;
const INTERNATIONALS: u16 = 4864;
const INTERNATIONAL_GOALS: u16 = 4865;
/// The cards in the league, the cup and the European cups: yellow,
/// yellow-red, red.
const YELLOW: [u16; 3] = [4866, 4867, 4868];
const YELLOW_RED: [u16; 3] = [4869, 4870, 4871];
const RED: [u16; 3] = [4872, 4873, 4874];
const BANS: [u16; 3] = [4875, 4876, 4877];
const FORM_AVERAGE: u16 = 4879;
const BEST_FORM: u16 = 4880;
const WORST_FORM: u16 = 4881;
const GRADE_AVERAGE: u16 = 4882;
/// The strips.
const FORM_STRIP: u16 = 5490;
const GRADES: u16 = 5489;
const GOALS_STRIP: u16 = 5488;
const APPEARANCES_STRIP: u16 = 5487;
const CARDS: u16 = 5486;

/// Texts of the game.
const DOPED: u16 = 3267;
const DOPED_A_DAY: u16 = 2507;
const DOPED_DAYS: u16 = 2506;
const DOPED_WEEKS: u16 = 2505;
const NO_WEEKS: u16 = 4141;
const A_WEEK: u16 = 4142;
const WEEKS: u16 = 4143;
const RED_CARD: u16 = 1858;
const YELLOW_RED_CARD: u16 = 1859;
const YELLOW_CARDS: u16 = 1860;
const SHORT_DATE: u16 = 1777;
/// Marks a yellow card count one more of which suspends him.
const ONE_MORE: &str = "*";

/// The form ring holds this many weeks; the strip shows one less once full.
const RING: usize = 26;
/// The strips' bars: their height, how far apart the form's are and how
/// wide, the matchdays' the same.
const STRIP: i32 = 20;
const FORM_STEP: i32 = 13;
const MATCHDAY_STEP: i32 = 6;
const MATCHDAY_BAR: i32 = 18;
/// The most matchdays a season keeps, and those of a league below the
/// fifth level.
const MATCHDAYS: usize = 46;
const LOW_LEAGUE_MATCHDAYS: usize = 18;
const LOW_LEAGUE: u8 = 6;
/// Colours, as Windows writes them: `0x00bbggrr`.
const WHITE: u32 = 0xffffff;
const RED_INK: u32 = 0xff;
const GREEN: u32 = 0xff00;
const YELLOW_INK: u32 = 0xffff;
const CARD_YELLOW: u32 = 0xf0f0;
const CARD_RED: u32 = 0xf0;

pub(super) fn show(s: &Shown, view: &mut View) {
    s.put(view, INJURY, injury_text(s.game, s.record, &s.status));
    let Some(r) = s.record else { return };
    let followed = s.followed();
    let dash = || "-".to_owned();
    let n = |v: u8| v.to_string();
    let goals = r.goals();
    s.put(view, GOALS, if followed { n(goals[0]) } else { dash() });
    s.put(view, ASSISTS, if followed { n(r.assists()) } else { dash() });
    s.put(view, CUP_GOALS, if followed { n(goals[1]) } else { dash() });
    s.put(view, EUROPE_GOALS, n(goals[2]));
    let of = |[taken, scored]: [u8; 2]| {
        if taken == 0 { dash() } else { format!("{scored} von {taken}") }
    };
    s.put(view, PENALTIES, of(r.penalties()));
    s.put(view, FREE_KICKS, of(r.free_kicks()));
    if followed {
        s.put(view, JOKER, format!("{}/{}", r.joker_goals(), r.league_less()[0]));
        s.put(view, OWN_GOALS, n(r.own_goals()));
        s.put(view, APPEARANCES, n(r.league_matches()));
        s.put(view, OF_THE_DAY, n(r.of_the_day()));
    } else {
        // The jokers keep what the page showed.
        s.put(view, JOKER, dialog_text(s.dialog, JOKER));
        for id in [OWN_GOALS, APPEARANCES, OF_THE_DAY] {
            s.put(view, id, dash());
        }
    }
    s.put(view, INTERNATIONALS, n(r.kind_12_matches()));
    s.put(view, INTERNATIONAL_GOALS, r.international_goals().to_string());

    // Cards and suspensions.
    let cards = [r.league_cards(), r.cup_cards(), r.europe_cards()];
    let warned = one_more_suspends(s, r);
    for (i, &[yellow, red, yellow_red]) in cards.iter().enumerate() {
        let star = if warned[i] { ONE_MORE } else { "" };
        s.put(view, YELLOW[i], format!("{yellow}{star}"));
        s.put(view, YELLOW_RED[i], if followed { n(yellow_red) } else { dash() });
        s.put(view, RED[i], if followed { n(red) } else { dash() });
    }
    s.put(view, BANS[0], league_ban(s, r));
    let cup = r.cup_ban();
    s.put(view, BANS[1], if cup == 0 { dash() } else { card_text(s, cup) });
    let europe = r.europe_ban() & 0x1f;
    s.put(view, BANS[2], if europe == 0 { dash() } else { weeks(s, europe) });

    form(s, view, r);
    matchdays(s, view, r, followed);
}

/// Whether one more yellow card suspends him, in the league, the cup and
/// the European cups, by the rules of his country (`FUN_0074612e`,
/// `FUN_007464a4`, `FUN_00746611`).
fn one_more_suspends(s: &Shown, r: &Record) -> [bool; 3] {
    let Some(records) = s.records else { return [false; 3] };
    let country = r.club()[0];
    let [league_rule, cup_rule] = records.countries.card_rules(usize::from(country));
    let league = r.league_cards()[0];
    // The original asks the level of the country's club numbered as the
    // country.
    let level = records.team(country.into(), country.into()).map_or(0, |t| t.level());
    let league_warned = match league_rule {
        0 => league % 5 == 4,
        1 => league & 3 == 3,
        2 => league == 3 || league >= 6 && league.is_multiple_of(3),
        3 => league == 3 || league == 6 || league >= 8,
        4 => level < 2 && league % 5 == 4,
        5 => league % 7 == 6,
        6 => league == 4 || league >= 6 && league.is_multiple_of(4),
        _ => league % 5 == 4,
    };
    // A cup counts the league's cards in the country 16.
    let cup = if country == 0x10 { league } else { r.cup_cards()[0] };
    let cup_warned = match cup_rule {
        1 => cup & 3 == 3,
        2 => cup == 3 || cup >= 6 && cup % 3 == 0,
        3 => cup == 3 || cup == 6 || cup >= 8,
        5 => cup != 0,
        _ => cup % 5 == 4,
    };
    let europe = r.europe_cards()[0];
    [league_warned, cup_warned, europe == 2 || europe >= 4 && europe.is_multiple_of(2)]
}

/// Why and how long he is suspended in the league: a doping ban, a ban of
/// weeks, a red card's or yellow cards' matches.
fn league_ban(s: &Shown, r: &Record) -> String {
    if s.status.long > 0 {
        return if s.status.long_unknown {
            s.text(DOPED)
        } else if s.status.long == 1 {
            s.text(DOPED_A_DAY)
        } else if s.status.long < 15 {
            s.text(DOPED_DAYS).replacen("%d", &s.status.long.to_string(), 1)
        } else {
            s.text(DOPED_WEEKS).replacen("%d", &((s.status.long + 6) / 7).to_string(), 1)
        };
    }
    if r.banned_weeks() > 0 {
        return weeks(s, r.banned_weeks());
    }
    // A red card's ban once its day has passed is over.
    let [matches, kind] = r.league_ban();
    let today = s.records.map_or(0, |records| records.globals.day() as i16);
    let day = r.sent_off_on();
    let over = (day == 0 || day < today) && kind & 0x80 != 0;
    let (matches, kind) = if over { (0, 0) } else { (matches, kind) };
    if matches & 0x1f == 0 {
        "-".to_owned()
    } else if kind & 0x80 != 0 {
        let when = if matches & 0x80 != 0 {
            " (?)".to_owned()
        } else {
            let date = s.records.and_then(|records| records.globals.date_of(day));
            format!(" ({})", date.map_or_else(String::new, |d| short_date(s, d)))
        };
        card_text(s, kind) + &when
    } else if kind & 0x60 != 0 {
        weeks(s, matches & 0x1f)
    } else {
        "-".to_owned()
    }
}

/// "eine Woche", "%d Wochen" (`FUN_00578e2f`).
fn weeks(s: &Shown, n: u8) -> String {
    match n {
        0 => s.text(NO_WEEKS),
        1 => s.text(A_WEEK),
        _ => s.text(WEEKS).replacen("%d", &n.to_string(), 1),
    }
}

/// The card a suspension is for (`FUN_00732313`).
fn card_text(s: &Shown, kind: u8) -> String {
    s.text(if kind & 0x80 != 0 {
        RED_CARD
    } else if kind & 0x40 != 0 {
        YELLOW_RED_CARD
    } else {
        YELLOW_CARDS
    })
}

fn short_date(s: &Shown, d: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    s.text(SHORT_DATE)
        .replace("%1", &d.day().to_string())
        .replace("%2", &d.month().to_string())
        .replace("%3", &d.year().to_string())
}

/// His form week by week: its average, best and worst, and a white bar a
/// week, as high as it was.
fn form(s: &Shown, view: &mut View, r: &Record) {
    let weeks = usize::from(r.form_weeks());
    let ring = r.form_ring();
    let kept = &ring[..weeks.min(RING)];
    let (best, worst) = (kept.iter().max(), kept.iter().min());
    let sum: u32 = kept.iter().map(|&f| u32::from(f)).sum();
    let average = if kept.is_empty() {
        String::new()
    } else {
        one_decimal(f64::from(sum) / kept.len() as f64)
    };
    s.put(view, FORM_AVERAGE, average);
    s.put(view, BEST_FORM, best.map_or_else(String::new, |b| b.to_string()));
    s.put(view, WORST_FORM, worst.map_or_else(String::new, |w| w.to_string()));
    let mut strip = PaintState::default();
    let bar = |strip: &mut PaintState, x: i32, v: u8, w: i32| {
        let v = i32::from(v);
        if v != 0 {
            strip.bar(x, STRIP - v, w, v, WHITE);
        }
    };
    if weeks < RING {
        for (i, &v) in ring[..weeks].iter().enumerate() {
            bar(&mut strip, FORM_STEP * i as i32, v, 9);
        }
    } else {
        // Once full, from the oldest on, leaving out the newest.
        let oldest = weeks % RING;
        for j in 0..RING - 1 {
            bar(&mut strip, FORM_STEP * j as i32, ring[(oldest + j) % RING], 10);
        }
    }
    paint(s, view, FORM_STRIP, strip);
}

/// His league matchdays, a bar each: his grade (green, yellow at his best,
/// red at his worst), that he played (green, white coming on, red going
/// off), his goals, and his cards — or, with none, his assists in the
/// appearance's colour, as the original draws them.
fn matchdays(s: &Shown, view: &mut View, r: &Record, followed: bool) {
    let count = if followed { matchday_count(s, r).min(MATCHDAYS) } else { 0 };
    let (played, goals) = (r.matchdays(), r.matchday_goals());
    let mut strips: [PaintState; 4] = Default::default();
    let [grades, appearances, scored, cards] = &mut strips;
    let height = |v: i32, of: i32| {
        let h = v * MATCHDAY_BAR / of;
        if v != 0 && h == 0 { 1 } else { h }
    };
    let (mut sum, mut games) = (0, 0);
    for i in 0..count {
        let x = MATCHDAY_STEP * i as i32;
        let w = MATCHDAY_STEP - 1;
        let (m, t) = (played[i], goals[i]);
        if m & 0xc0 == 0 {
            continue;
        }
        let grade = i32::from(m >> 2 & 0xf);
        sum += grade;
        games += 1;
        let ink = match grade {
            ..3 => YELLOW_INK,
            12.. => RED_INK,
            _ => GREEN,
        };
        let h = height(grade, 12);
        grades.bar(x, STRIP - h, w, h, ink);
        let ink = match m & 0xc0 {
            0x80 => WHITE,
            0xc0 => RED_INK,
            _ => GREEN,
        };
        appearances.bar(x, STRIP - MATCHDAY_BAR, w, MATCHDAY_BAR, ink);
        let h = height(i32::from(t >> 4).min(5), 5);
        scored.bar(x, STRIP - h, w, h, GREEN);
        match m & 3 {
            2 => {
                let h = height(2, 3);
                cards.bar(x, STRIP - h, w, h / 2, CARD_RED);
                cards.bar(x, STRIP - h / 2, w, h / 2, CARD_YELLOW);
            }
            card => {
                let (k, ink) = match card {
                    1 => (2, CARD_YELLOW),
                    3 => (2, RED_INK),
                    _ => (i32::from(t & 0xf).min(5), ink),
                };
                let h = height(k, 3);
                cards.bar(x, STRIP - h, w, h, ink);
            }
        }
    }
    s.put(
        view,
        GRADE_AVERAGE,
        if games == 0 {
            "-".to_owned()
        } else {
            one_decimal(f64::from(sum) / f64::from(games) / 2.0)
        },
    );
    let [grades, appearances, scored, cards] = strips;
    paint(s, view, GRADES, grades);
    paint(s, view, APPEARANCES_STRIP, appearances);
    paint(s, view, GOALS_STRIP, scored);
    paint(s, view, CARDS, cards);
}

/// The matchdays of his club's league (`FUN_006927b3`).
fn matchday_count(s: &Shown, r: &Record) -> usize {
    let [country, number] = r.club();
    let level = s.records.and_then(|records| records.team(country.into(), number.into()));
    if level.is_some_and(|t| t.level() >= LOW_LEAGUE) {
        return LOW_LEAGUE_MATCHDAYS;
    }
    let (slot, club) = s.club;
    s.game
        .world
        .leagues_in_play(slot, s.season.main)
        .iter()
        .find(|league| league.clubs.contains(&club))
        .map_or(0, |league| league.fixtures.len())
}

fn paint(s: &Shown, view: &mut View, id: u16, strip: PaintState) {
    if let Some(p) = super::control(s.dialog, id).and_then(|c| view.paint_mut(c)) {
        *p = strip;
    }
}
