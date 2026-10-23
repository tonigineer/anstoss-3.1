//! The board ("Vereinsführung", dialog 439 `CDiVereinsfuehrung`;
//! VEREINSFUEHRUNG.md in the RE repo): the board's confidence in the
//! manager with the president's face and its history, the opposition, the
//! state of a contract extension, the club's shares; five things to try
//! with the board and the trophy hall. Three of the actions work: seeking
//! a contract extension, a talk with the board and flattering the
//! opposition. The Freistellung, the resignation and the hall are not
//! built yet.

use std::collections::HashMap;

use a3_core::contract;
use a3_core::cycle::Request;
use a3_core::cycle::dialogs::{BOARD, NOTICE};
use a3_core::records::Manager;
use a3_core::season::Rules;
use a3_files::rch::Dialog;

use super::hub::{self, BOARD_SYMBOL};
use super::planner::PLANNER;
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::options::NONE;
use crate::pointer::Target;
use crate::view::View;

const CAPTION_ID: u16 = 9935;
/// "ANSTOSS 3 - Vereinsführung %s %s (%s)": no date.
const CAPTION: u16 = 4490;
const OK: u16 = 1;
const ACTIONS: u16 = 5387;
const TROPHIES: u16 = 5388;
/// Bei Opposition einschleimen, off without an opposition.
const FLATTER: usize = 2;
const CONFIDENCE: u16 = 2774;
const BOARD_STATE: u16 = 5362;
const FACE: u16 = 5363;
/// The president's faces by confidence: below each threshold the next.
const FACE_THRESHOLDS: [i32; 7] = [50, 55, 60, 70, 80, 90, 999];
const FACES: [u16; 7] = [1922, 1921, 1920, 1919, 1918, 1917, 1916];
/// The board's moods 1040 + v, "Vorstand???" past them.
const BOARD_TEXTS: u16 = 1040;
const BOARD_MOODS: u8 = 6;
const NO_BOARD: &str = "Vorstand???";
const OPPOSITION_LIKES: u16 = 5364;
const OPPOSITION_STATE: u16 = 5365;
/// The opposition's strengths, 0 "Nicht relevant" (4497) to 6 (4491).
const OPPOSITION_TEXTS: u16 = 4497;
const EXTENSION: u16 = 5360;
const CHART: u16 = 5379;
const CHART_TEXT: u16 = 5370;
/// "Sie sind der König von %s.", then the six verdicts 2471–2476.
const KING: u16 = 2470;
const MY_SHARES_TEXT: u16 = 5361;
const NOT_LISTED: u16 = 4498;
const SHARE_ROWS: [u16; 13] =
    [5371, 5375, 5369, 5372, 5373, 5374, 5380, 5381, 5382, 5376, 5377, 5378, 5366];
const SHARE_LABELS: [u16; 2] = [5367, 5368];
/// The team's board mood and opposition (team +0x123, +0x871).
const BOARD_AT: usize = 0x123;
const OPPOSITION_AT: usize = 0x871;
/// The manager record: years of contract, the club he moves to, his
/// popularity with the opposition, the confidence's history, the day
/// talks open again.
const YEARS_AT: usize = 0x172;
const MOVING_AT: usize = 0x130;
const OPPOSITION_POPULARITY_AT: usize = 0x198;
const HISTORY_AT: usize = 0x18a;
const HISTORY: usize = 10;
const BLOCKED_AT: usize = 0x1f88;
/// Talks held with the board this season, and whether one was held today
/// (rec +0x1f94, +0x1f95); the confidence's own field; how the
/// opposition likes him and whether he has crawled to it this season.
const TALKS_AT: usize = 0x1f94;
const TALKED_AT: usize = 0x1f95;
const CONFIDENCE_AT: usize = 0x184;
const CRAWLED_AT: usize = 0x19a;
/// The items built: seeking a contract extension, a talk with the board,
/// flattering the opposition.
const EXTEND: usize = 0;
const TALK: usize = 1;
/// Talked already, no reason to; the first talk's three outcomes by the
/// draw, the second's, the later ones'.
const TALKED: u16 = 3763;
const NO_REASON: u16 = 3764;
const TALK_TEXTS: [u16; 3] = [3765, 3766, 3767];
const TALK_AGAIN: u16 = 3768;
const TALKED_ENOUGH: [u16; 2] = [3769, 3770];
/// What each outcome does to the confidence.
const TALK_GAINS: [i32; 3] = [5, 10, -10];
const LATER_TALK_LOSS: i32 = 5;
/// Flattering: it worked, or it was done already; what it costs with the
/// board and brings with the opposition.
const FLATTERED: u16 = 4499;
const FLATTERED_ALREADY: u16 = 4997;
const FLATTER_COST: i32 = 5;
/// "Sie wechseln zum 30.6. v6."; the extension's states (§2.4).
const MOVING: u16 = 3886;
/// Seeking the extension (§4.1): leaving already, a talk booked this
/// week, a talk booked with this board — the latter also what the line
/// shows then (§2.4).
const LEAVING: u16 = 1802;
const TALKING: u16 = 3142;
const BOOKED_TALK: u16 = 3823;

pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&BOARD) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    hub::add_bar(dialog, rules);
}

/// The screen.
#[derive(Default)]
pub struct Board {
    whistle: bool,
    /// Whether the alert now up is the opposition's, whose changes come
    /// after it closes.
    flattered: bool,
    /// Whether an alert of the extension sought is up; after it the
    /// original fills nothing anew.
    sought: bool,
    /// Whether the planner now up is to book the talk with the board.
    booking: bool,
}

impl Screen for Board {
    fn ids(&self) -> &[u16] {
        &[BOARD]
    }

    /// `FUN_005b4d48`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        hub::open_bar(cx, view, BOARD_SYMBOL);
        let (Some(team), Some(manager)) = (own_team(cx), super::kalkulation::own_manager(cx))
        else {
            return;
        };
        let club = game.club_name(cx.state.records.as_ref(), team.country(), team.number().into());
        let caption = super::finances::printf(game.text(CAPTION), &[&first(cx), &last(cx), &club]);
        set(view, dialog, CAPTION_ID, caption);
        let c = manager.shown_confidence();
        self.show(cx, view, c);
        // The opposition.
        let opposition = team.get::<u8>(OPPOSITION_AT);
        let likes = if opposition >= 1 {
            format!("{}%", manager.get::<i16>(OPPOSITION_POPULARITY_AT))
        } else {
            "-".to_owned()
        };
        set(view, dialog, OPPOSITION_LIKES, likes);
        let state = OPPOSITION_TEXTS.saturating_sub(u16::from(opposition.min(6)));
        set(view, dialog, OPPOSITION_STATE, game.text(state).to_owned());
        set(view, dialog, EXTENSION, extension_text(cx, c));
        // No club of the replica's saves is on the stock market.
        set(view, dialog, MY_SHARES_TEXT, game.text(NOT_LISTED).to_owned());
        for id in SHARE_ROWS.iter().chain(&SHARE_LABELS) {
            if let Some(i) = control(dialog, *id) {
                view.controls[i].hidden = true;
            }
        }
        if let Some(c) = control(dialog, ACTIONS) {
            if opposition == 0
                && let Some(options) = view.options_mut(c)
            {
                options.dimmed = (0..5).map(|i| i == FLATTER).collect();
            }
            view.set_chosen(c, NONE);
        }
        let works = |t: Target| match t {
            Target::Option { control: c, item } if dialog.controls[c].id == Some(ACTIONS) => {
                item == EXTEND || item == TALK || (item == FLATTER && opposition != 0)
            }
            Target::Button(c) if dialog.controls[c].id == Some(TROPHIES) => false,
            _ => hub::on_bar(dialog, t).is_none_or(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Some(item) = hub::on_bar(cx.dialog, target) {
            let outcome = hub::bar_click(cx, view, item, BOARD_SYMBOL);
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        self.whistle = false;
        let Target::Option { control: c, item } = target else { return Outcome::Stay };
        if cx.dialog.controls[c].id != Some(ACTIONS) {
            return Outcome::Stay;
        }
        // The button stays pressed while its alert is up.
        match item {
            EXTEND => self.seek(cx),
            TALK => Outcome::Popup(Request::with(NOTICE, Args::Text(self.talk(cx)))),
            FLATTER => {
                let crawled = manager_flag(cx, CRAWLED_AT);
                self.flattered = !crawled;
                let text = if crawled { FLATTERED_ALREADY } else { FLATTERED };
                Outcome::Popup(Request::with(NOTICE, Args::Text(text)))
            }
            _ => {
                view.set_chosen(c, NONE);
                Outcome::Stay
            }
        }
    }

    /// An alert taken in: the opposition's changes follow it; then the
    /// screen is filled anew, as the original fills it.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        let (flattered, sought, booking) = (
            std::mem::take(&mut self.flattered),
            std::mem::take(&mut self.sought),
            std::mem::take(&mut self.booking),
        );
        if flattered {
            self.crawl(cx);
        }
        // The clicked button shows pressed while its popup is up and is
        // plain again after.
        if let Some(c) = control(cx.dialog, ACTIONS) {
            view.set_chosen(c, NONE);
        }
        if booking {
            // The planner closing changes only the extension's line, and
            // only when the talk was booked (§4.1).
            if reply == Reply::Ok && own_talk_booked(cx) {
                let text = cx.game.text(BOOKED_TALK).to_owned();
                set(view, cx.dialog, EXTENSION, text);
            }
            return Outcome::Stay;
        }
        if sought {
            return Outcome::Stay;
        }
        self.refresh(cx, view);
        Outcome::Stay
    }
}

/// Whether the manager's own club has a talk booked with his board in the
/// shared list (`FUN_006a8ce2`).
fn own_talk_booked(cx: &Cx) -> bool {
    let (Some(team), Some(season), Some(records)) =
        (own_team(cx), cx.state.season.as_ref(), cx.state.records.as_ref())
    else {
        return false;
    };
    records.human_slot(season.current).is_some_and(|index| {
        contract::talk_booked(records, index, (team.country(), team.number().into()))
    })
}

/// A flag byte of the manager's record.
fn manager_flag(cx: &Cx, at: usize) -> bool {
    super::kalkulation::own_manager(cx).is_some_and(|m| m.get::<u8>(at) & 1 != 0)
}

/// The manager's record, to change.
fn manager_mut<'a>(cx: &'a mut Cx) -> Option<&'a mut Manager> {
    let current = cx.state.season.as_ref()?.current;
    let records = cx.state.records.as_mut()?;
    let slot = records.human_slot(current)?;
    records.managers.get_mut(slot)
}

/// The board's confidence moved, held between nothing and all of it.
fn move_confidence(cx: &mut Cx, by: i32) {
    if let Some(manager) = manager_mut(cx) {
        let v = (i32::from(manager.get::<i16>(CONFIDENCE_AT)) + by).clamp(0, 100);
        manager.put(CONFIDENCE_AT, v as i16);
    }
}

fn first(cx: &Cx) -> String {
    cx.state
        .season
        .as_ref()
        .and_then(|s| s.manager())
        .map(|m| m.first_name.clone())
        .unwrap_or_default()
}

fn last(cx: &Cx) -> String {
    cx.state
        .season
        .as_ref()
        .and_then(|s| s.manager())
        .map(|m| m.last_name.clone())
        .unwrap_or_default()
}

impl Board {
    /// A talk with the board (`FUN_005b647c`): a draw is made first,
    /// whatever follows. One talk a season is listened to; the later ones
    /// only cost.
    fn talk(&self, cx: &mut Cx) -> u16 {
        let draw = cx.state.season.as_mut().map_or(0, |s| s.random.below(100));
        let Some(manager) = super::kalkulation::own_manager(cx) else { return TALKED };
        let (talks, talked) = (manager.get::<u8>(TALKS_AT), manager.get::<u8>(TALKED_AT));
        let c = manager.shown_confidence();
        if talked != 0 {
            return TALKED;
        }
        if c > 90 {
            return NO_REASON;
        }
        let (text, by) = match talks {
            0 => {
                let k = match draw {
                    0..70 => 0,
                    70..90 => 1,
                    _ => 2,
                };
                (TALK_TEXTS[k], TALK_GAINS[k])
            }
            1 => (TALK_AGAIN, 0),
            _ => (TALKED_ENOUGH[usize::from(draw >= 50)], -LATER_TALK_LOSS),
        };
        move_confidence(cx, by);
        if let Some(manager) = manager_mut(cx) {
            manager.put(TALKS_AT, talks.wrapping_add(1));
            manager.put(TALKED_AT, 1u8);
        }
        text
    }

    /// Flattering the opposition (`FUN_005b62aa`), once a season: the
    /// board thinks the less of him, the opposition the more.
    fn crawl(&self, cx: &mut Cx) {
        move_confidence(cx, -FLATTER_COST);
        if let Some(manager) = manager_mut(cx) {
            manager.put(CRAWLED_AT, 1u8);
            let v = (i32::from(manager.get::<i16>(OPPOSITION_POPULARITY_AT)) + FLATTER_COST)
                .clamp(0, 100);
            manager.put(OPPOSITION_POPULARITY_AT, v as i16);
        }
    }

    /// The screen filled anew after an action (`FUN_005b4ff2` and the
    /// three that follow it).
    fn refresh(&self, cx: &mut Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(manager) = super::kalkulation::own_manager(cx) else { return };
        let c = manager.shown_confidence();
        let likes = i32::from(manager.get::<i16>(OPPOSITION_POPULARITY_AT));
        self.show(cx, view, c);
        if own_team(cx).is_some_and(|t| t.get::<u8>(OPPOSITION_AT) >= 1) {
            set(view, dialog, OPPOSITION_LIKES, format!("{likes}%"));
        }
        set(view, dialog, EXTENSION, extension_text(cx, c));
        let _ = game;
    }

    /// Seeking a contract extension (`FUN_005b5f3b`): he is leaving the
    /// club, has a talk booked this week, or the board has its mind on
    /// one; only when it is ready to talk does the week planner ask for
    /// the day.
    fn seek(&mut self, cx: &mut Cx) -> Outcome {
        let (Some(team), Some(manager)) = (own_team(cx), super::kalkulation::own_manager(cx))
        else {
            return Outcome::Stay;
        };
        let (country, number) = (team.country(), team.number().into());
        let (moving, leaving) =
            (manager.get::<u8>(MOVING_AT), manager.get::<i16>(MOVING_AT + 2));
        let c = manager.shown_confidence();
        if moving != 0 {
            self.sought = true;
            return Outcome::Popup(Request::with(
                NOTICE,
                Args::TextForClub(LEAVING, moving, leaving),
            ));
        }
        let booked = cx.state.records.as_ref().zip(cx.state.season.as_ref()).is_some_and(
            |(r, s)| r.human_slot(s.current).is_some_and(|i| contract::talk_in_week(r, i)),
        );
        if booked {
            self.sought = true;
            return Outcome::Popup(Request::with(NOTICE, Args::Text(TALKING)));
        }
        let (text, r) = ladder(cx, c);
        if r <= 0 {
            self.sought = true;
            return Outcome::Popup(Request::with(NOTICE, Args::Text(text)));
        }
        self.booking = true;
        Outcome::Popup(Request::with(PLANNER, Args::WithClub(country, number)))
    }

    /// The confidence, its face, the board's mood, the chart and its
    /// verdict (`FUN_005b4ff2`, `FUN_005b5ad2`).
    fn show(&self, cx: &Cx, view: &mut View, c: i32) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(team) = own_team(cx) else { return };
        let i = FACE_THRESHOLDS.iter().filter(|&&t| t <= c).count().min(FACES.len() - 1);
        // The face is cut to its control, which is smaller than the
        // picture.
        if let Some(face) = control(dialog, FACE) {
            let r = dialog.controls[face].rect;
            let mut picture = crate::controls::bitmap::Picture::graphic(FACES[i]);
            picture.part = Some([0, 0, r.width() as u32, r.height() as u32]);
            view.set_picture(face, Some(picture));
        }
        set(view, dialog, CONFIDENCE, format!("{c}%"));
        let mood = team.get::<u8>(BOARD_AT);
        let mood_text = if mood < BOARD_MOODS {
            game.text(BOARD_TEXTS + u16::from(mood)).to_owned()
        } else {
            NO_BOARD.to_owned()
        };
        set(view, dialog, BOARD_STATE, mood_text);
        let verdict = match c {
            100 => {
                let city = cx
                    .state
                    .records
                    .as_ref()
                    .and_then(|r| a3_core::stadium::subs(r, team.country(), team.number().into()))
                    .and_then(|s| s.first().map(a3_core::stadium::Sub::city))
                    .unwrap_or_default();
                game.text(KING).replacen("%s", &city, 1)
            }
            90.. => game.text(KING + 1).to_owned(),
            80.. => game.text(KING + 2).to_owned(),
            70.. => game.text(KING + 3).to_owned(),
            60.. => game.text(KING + 4).to_owned(),
            ..55 => game.text(KING + 6).to_owned(),
            _ => game.text(KING + 5).to_owned(),
        };
        set(view, dialog, CHART_TEXT, verdict);
        // The history and today's confidence as a black line from the
        // strip's left edge, a tenth of its width apart.
        view.lines.clear();
        let Some(manager) = super::kalkulation::own_manager(cx) else { return };
        let Some(strip) = control(dialog, CHART).map(|i| dialog.controls[i].rect) else { return };
        let (w, h) = (strip.width() as i32, strip.height() as i32);
        let mut points: Vec<i32> =
            (0..HISTORY).map(|k| i32::from(manager.get::<u8>(HISTORY_AT + k))).collect();
        points.push(c);
        let Some(f) = points[..HISTORY].iter().position(|&v| v != 0) else { return };
        let y = |v: i32| h - (v as f32 / 100.0 * h as f32) as i32;
        let (x0, y0) = (strip.min.x as i32, strip.min.y as i32);
        let mut at = (x0, y0 + y(points[f]));
        for (i, &v) in points.iter().enumerate().skip(f) {
            let to = (x0 + w * (i - f) as i32 / 10, y0 + y(v));
            view.lines.push(([at.0, at.1, to.0, to.1], [0, 0, 0]));
            at = to;
        }
        // The last point is drawn too.
        view.lines.push(([at.0, at.1, at.0, at.1], [0, 0, 0]));
    }
}

/// What the board thinks of extending now (`FUN_00757631`): the text and
/// its return. When the talks are open again — the confidence high enough
/// and the block day past — the block day is cleared (rec +0x1f88 := 0).
fn ladder(cx: &mut Cx, c: i32) -> (u16, i32) {
    let (years_at, blocked_at) = super::kalkulation::own_manager(cx)
        .map_or((0, 0), |m| {
            (i32::from(m.get::<i16>(YEARS_AT)), i32::from(m.get::<i16>(BLOCKED_AT)))
        });
    let today = cx.state.records.as_ref().map_or(0, |r| i32::from(r.globals.day()));
    let month = cx.state.season.as_ref().map_or(1, |s| chrono::Datelike::month(&s.date()) as i32);
    let years = years_at - i32::from(month < 7);
    let (text, r) = match () {
        _ if c < 60 => (3709, 0),
        _ if c < 70 => (3710, 0),
        _ if blocked_at > today + 80 => (3712, 0),
        _ if blocked_at > today => (3711, 0),
        _ if years == 1 && c < 90 => (3713, 0),
        _ if years >= 2 => (3714, 0),
        _ if years == 0 && c < 80 => (3715, 1),
        _ if (years == 0 && c < 90) || (years == 1 && c == 100) => (3716, 2),
        _ if years == 1 => (3718, 0),
        _ if years == 0 && c < 97 => (3719, 1),
        _ if years == 0 => (3720, 4),
        _ => (3721, 0),
    };
    if c >= 70 && blocked_at <= today && let Some(m) = manager_mut(cx) {
        m.put(BLOCKED_AT, 0i16);
    }
    (text, r)
}

/// The state of a contract extension (`FUN_005b526d`): a talk booked with
/// this board, moving away, or what the board thinks of extending now.
fn extension_text(cx: &mut Cx, c: i32) -> String {
    if own_talk_booked(cx) {
        return cx.game.text(BOOKED_TALK).to_owned();
    }
    let (moving, number) = super::kalkulation::own_manager(cx)
        .map_or((0, 0), |m| (m.get::<u8>(MOVING_AT), m.get::<i16>(MOVING_AT + 2)));
    if moving != 0 {
        let game = cx.game;
        let slot = game.world.slot_of(moving.into()).unwrap_or(0);
        return game.with_club(game.text(MOVING), (slot, number.max(0) as usize));
    }
    let game = cx.game;
    let (text, _) = ladder(cx, c);
    game.text(text).to_owned()
}
