//! The board ("Vereinsführung", dialog 439 `CDiVereinsfuehrung`;
//! VEREINSFUEHRUNG.md in the RE repo): the board's confidence in the
//! manager with the president's face and its history, the opposition, the
//! state of a contract extension, the club's shares; five things to try
//! with the board and the trophy hall. The actions and the hall are not
//! built yet.

use std::collections::HashMap;

use a3_core::cycle::dialogs::BOARD;
use a3_core::records::Manager;
use a3_core::season::Rules;
use a3_files::rch::Dialog;

use super::hub::{self, BOARD_SYMBOL};
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
/// "Sie wechseln zum 30.6. v6."; the extension's states (§2.4).
const MOVING: u16 = 3886;

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
        set(view, dialog, EXTENSION, extension_text(cx, manager, c));
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
            Target::Option { control: c, .. } if dialog.controls[c].id == Some(ACTIONS) => false,
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
        if let Target::Option { control: c, .. } = target {
            view.set_chosen(c, NONE);
        }
        Outcome::Stay
    }

    fn answer(&mut self, _: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
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

/// The state of a contract extension (`FUN_005b526d`, `FUN_00757631`):
/// moving away, or what the board thinks of extending now.
fn extension_text(cx: &Cx, manager: &Manager, c: i32) -> String {
    let game = cx.game;
    let moving = manager.get::<u8>(MOVING_AT);
    if moving != 0 {
        let slot = game.world.slot_of(moving.into()).unwrap_or(0);
        let number = manager.get::<i16>(MOVING_AT + 2).max(0) as usize;
        return game.with_club(game.text(MOVING), (slot, number));
    }
    let season = cx.state.season.as_ref();
    let today = cx.state.records.as_ref().map_or(0, |r| i32::from(r.globals.day()));
    let month = season.map_or(1, |s| chrono::Datelike::month(&s.date()) as i32);
    let years = i32::from(manager.get::<i16>(YEARS_AT)) - i32::from(month < 7);
    let blocked = i32::from(manager.get::<i16>(BLOCKED_AT));
    let text = match () {
        _ if c < 60 => 3709,
        _ if c < 70 => 3710,
        _ if blocked > today + 80 => 3712,
        _ if blocked > today => 3711,
        _ if years == 1 && c < 90 => 3713,
        _ if years >= 2 => 3714,
        _ if years == 0 && c < 80 => 3715,
        _ if (years == 0 && c < 90) || (years == 1 && c == 100) => 3716,
        _ if years == 1 => 3718,
        _ if years == 0 && c < 97 => 3719,
        _ if years == 0 => 3720,
        _ => 3721,
    };
    game.text(text).to_owned()
}
