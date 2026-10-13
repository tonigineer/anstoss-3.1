//! The week as it runs: it starts on the autosave's wait screen, and once
//! every manager has taken a turn it runs on its own screen, day by day,
//! with a panel for every human club. Both stand a while, then move on.

use std::collections::HashMap;

use bevy_math::{Rect, Vec2};

use a3_core::season::Season;
use a3_files::rch::{Align, Control, Dialog, Kind};

pub use a3_core::cycle::dialogs::{WAIT, WEEKS};

use super::{Args, Cx, Outcome, Reply, Screen, control, set};
use crate::controls::bitmap::{Picture, PictureSource};
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

/// The week screen and the panel it shows for each human club. The game
/// draws the frames of its panels into the background, so the week screen
/// is put together under ids of its own, [`WEEKS`], with no panel, one, …
/// four.
pub const WEEK: u16 = 2;
const WEEK_PANEL: u16 = 460;
pub const WEEK_BACKGROUNDS: [u16; 5] = [2835, 2798, 2799, 2800, 2801];
pub const PANEL_LEFT: f32 = 128.0;
pub const PANEL_TOPS: [&[f32]; 5] =
    [&[], &[162.0], &[125.0, 257.0], &[30.0, 162.0, 294.0], &[24.0, 120.0, 216.0, 312.0]];

/// Controls by their ids in the dialog file.
const WAIT_DETAIL: u16 = 6363;
const WEEK_WEATHER: u16 = 5811;
const PANEL_HOME: u16 = 1020;
const PANEL_AWAY: u16 = 1021;
const PANEL_CONFIDENCE: u16 = 5824;
const PANEL_MATCHDAY: u16 = 5825;
const PANEL_MANAGER: u16 = 5826;
const PANEL_PRESALE: u16 = 5827;
const PANEL_PRESALE_COUNT: u16 = 5828;
const PANEL_CONDITION: u16 = 5959;
const PANEL_FRESHNESS: u16 = 5960;
const PANEL_CREST: u16 = 5801;
const PANEL_CHARTS: [u16; 2] = [5802, 5803];
/// The charts: a point a day, this far apart.
const CHART_STEP: u32 = 6;
/// Ids for the tiles the week screen marks its elapsed days with, which the
/// game draws itself.
const DAY_TILE: u16 = 9900;

/// Texts by id.
const WEEK_HOME: u16 = 2843;
const WEEK_AWAY: u16 = 2844;
const SEASON_TICKETS: u16 = 5008;

/// The elapsed days' tiles: the weekday names of `woche/wochD02.bmp`, one
/// above the other every 63 pixels, laid over the grey column of the screen.
const DAY_TILES: u16 = 1385;
pub const DAY_TILE_SIZE: (u32, u32) = (120, 56);
const DAY_TILE_STEP: u32 = 63;
pub const DAY_TILE_TOPS: [f32; 7] = [26.0, 89.0, 152.0, 215.0, 278.0, 341.0, 404.0];
/// The weather, graphics 1513 to 1525; the game's weather map for Germany
/// starts the season fair.
const WEATHER: u16 = 1512;
const FAIR: u16 = 10;
/// The week screen and the line-up's comparison show the next match within
/// this many days (`FUN_0062bfc9`).
pub(super) const FIXTURE_AHEAD: usize = 63;

/// The wait screen stands at least this long, as saving takes the game.
const WAIT_SECS: f32 = 1.2;
/// A day of the week's run takes at least this long on screen.
const DAY_SECS: f32 = 0.3;
pub(crate) const WEEK_DAYS: usize = 7;

/// Whether a screen is the week running.
pub fn is_week(id: u16) -> bool {
    WEEKS.contains(&id)
}

/// Puts the week screens together: the week dialog with its background for
/// the number of panels, the panels in their places and the tiles for the
/// days.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let (Some(week), Some(panel)) = (dialogs.get(&WEEK), dialogs.get(&WEEK_PANEL)) else { return };
    let (week, panel) = (week.clone(), panel.clone());
    for (count, &id) in WEEKS.iter().enumerate() {
        let mut screen = week.clone();
        screen.background = WEEK_BACKGROUNDS[count];
        // The panel is smaller than the screen, so it was read as a popup in
        // the middle; it goes where the week screen puts it instead.
        let origin = panel.frame.map_or(Vec2::ZERO, |frame| frame.min);
        for &top in PANEL_TOPS[count] {
            let shift = Vec2::new(PANEL_LEFT, top) - origin;
            screen.controls.extend(panel.controls.iter().cloned().map(|mut c| {
                c.rect = Rect::from_corners(c.rect.min + shift, c.rect.max + shift);
                // The game paints the charts itself.
                if PANEL_CHARTS.contains(&c.id.unwrap_or(0)) {
                    c.kind = Kind::Bitmap;
                }
                c
            }));
        }
        let (w, h) = (DAY_TILE_SIZE.0 as f32, DAY_TILE_SIZE.1 as f32);
        screen.controls.extend(DAY_TILE_TOPS.iter().enumerate().map(|(day, &top)| Control {
            rect: Rect::new(0.0, top, w, top + h),
            ..bitmap(Vec2::ZERO, 0.0, DAY_TILE + day as u16)
        }));
        dialogs.insert(id, screen);
    }
}

/// A bitmap control of the game's own, which the dialog file does not have.
pub(crate) fn bitmap(at: Vec2, size: f32, id: u16) -> Control {
    Control {
        kind: Kind::Bitmap,
        rect: Rect::from_corners(at, at + Vec2::splat(size)),
        align: Align::Left,
        text: String::new(),
        id: Some(id),
        bold: false,
        ink: [0; 3],
    }
}

/// The autosave's wait screen.
#[derive(Default)]
pub struct Wait {
    timer: f32,
}

impl Screen for Wait {
    fn ids(&self) -> &[u16] {
        &[WAIT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        self.timer = 0.0;
        if cx.state.season.is_some() {
            set(view, cx.dialog, WAIT_DETAIL, String::new());
        }
    }

    fn input(&mut self, _: &mut Cx, _: &mut View, _: Target, _: bool) -> Outcome {
        Outcome::Stay
    }

    fn tick(&mut self, cx: &mut Cx, _: &mut View, seconds: f32) -> Outcome {
        self.timer += seconds;
        if cx.state.season.is_none() || self.timer < WAIT_SECS {
            return Outcome::Stay;
        }
        Outcome::Close(Reply::Ok)
    }
}

/// The week running: asked for again after every day, each of which stands
/// a while.
#[derive(Default)]
pub struct Week {
    /// The day it started on.
    start: usize,
    timer: f32,
}

impl Screen for Week {
    fn ids(&self) -> &[u16] {
        &WEEKS
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        self.timer = 0.0;
        let Some(season) = &cx.state.season else { return };
        self.start = season.day;
        show_week(view, cx.dialog, cx.game, season);
    }

    /// A day has run: its tile lights up, the panels move on.
    fn again(&mut self, cx: &mut Cx, view: &mut View, _: Args) -> bool {
        self.timer = (self.timer - DAY_SECS).max(0.0);
        let Some(season) = &cx.state.season else { return true };
        for day in 0..(season.day - self.start).min(WEEK_DAYS) {
            if let Some(tile) = control(cx.dialog, DAY_TILE + day as u16) {
                view.controls[tile].hidden = false;
            }
        }
        show_panels(view, cx.dialog, cx.game, season);
        true
    }

    fn input(&mut self, _: &mut Cx, _: &mut View, _: Target, _: bool) -> Outcome {
        Outcome::Stay
    }

    fn tick(&mut self, cx: &mut Cx, _: &mut View, seconds: f32) -> Outcome {
        self.timer += seconds;
        if cx.state.season.is_none() || self.timer < DAY_SECS {
            return Outcome::Stay;
        }
        Outcome::Close(Reply::Ok)
    }
}

/// The week as it starts: a panel for every human club with its next match,
/// its manager and the board's confidence, fair weather and no day done yet.
fn show_week(view: &mut View, dialog: &Dialog, game: &Game, season: &Season) {
    if let Some(weather) = control(dialog, WEEK_WEATHER) {
        view.set_picture(weather, Some(Picture::graphic(WEATHER + FAIR)));
    }
    for day in 0..WEEK_DAYS {
        if let Some(tile) = control(dialog, DAY_TILE + day as u16) {
            let (w, h) = DAY_TILE_SIZE;
            let picture = Picture {
                source: PictureSource::Graphic(DAY_TILES),
                part: Some([0, DAY_TILE_STEP * day as u32, w, h]),
                black_clear: false,
            };
            view.set_picture(tile, Some(picture));
            view.controls[tile].hidden = true;
        }
    }
    show_panels(view, dialog, game, season);
}

/// Every human club's panel, as the week has gone so far.
fn show_panels(view: &mut View, dialog: &Dialog, game: &Game, season: &Season) {
    let clubs = season.managers.iter().filter_map(|m| Some((m, m.club?)));
    for (panel, (manager, own)) in clubs.enumerate() {
        let set = |view: &mut View, id: u16, text: String| {
            if let Some(i) = nth_control(dialog, id, panel) {
                view.set_text(i, text);
            }
        };
        let name = |(slot, club): (usize, usize)| {
            game.world.country(slot).and_then(|c| c.clubs.get(club)).map_or("", |c| &c.name)
        };
        // The own club always on the left, marked home or away.
        let next = season
            .next_match(&game.world, &game.calendar, own)
            .filter(|m| m.day < season.day + FIXTURE_AHEAD);
        let (home, away) = match next {
            Some(m) => {
                let at_home = m.home == own;
                let opponent = if at_home { m.away } else { m.home };
                let mark = |home: bool, club| {
                    game.text(if home { WEEK_HOME } else { WEEK_AWAY }).replacen(
                        "%s",
                        name(club),
                        1,
                    )
                };
                (mark(at_home, own), mark(!at_home, opponent))
            }
            None => (name(own).to_owned(), String::new()),
        };
        set(view, PANEL_HOME, home);
        set(view, PANEL_AWAY, away);
        set(view, PANEL_CONFIDENCE, format!("{}%", manager.confidence));
        set(view, PANEL_MATCHDAY, String::new());
        set(
            view,
            PANEL_MANAGER,
            format!("{} {} ({})", manager.first_name, manager.last_name, name(own)),
        );
        set(view, PANEL_PRESALE, game.text(SEASON_TICKETS).to_owned());
        let state = season.clubs.get(&own);
        set(view, PANEL_PRESALE_COUNT, thousands(state.map_or(0, |s| s.tickets)));
        let week = state.map_or(&[][..], |s| &s.week[..]);
        // The labels show the latest value, the charts the whole week.
        for (k, (id, chart)) in
            [PANEL_CONDITION, PANEL_FRESHNESS].into_iter().zip(PANEL_CHARTS).enumerate()
        {
            let values: Vec<u8> =
                week.iter().map(|&(c, f)| if k == 0 { c } else { f }.clamp(0, 100) as u8).collect();
            if let Some(i) = nth_control(dialog, id, panel) {
                let label = &dialog.controls[i].text;
                let text = match values.iter().rev().find(|&&v| v != 0) {
                    Some(v) => format!("{label} {v}"),
                    None => label.clone(),
                };
                view.set_text(i, text);
            }
            if let Some(i) = nth_control(dialog, chart, panel) {
                let size = dialog.controls[i].rect.size();
                let picture = Picture {
                    source: PictureSource::Chart {
                        values,
                        size: (size.x as u32, size.y as u32),
                        step: CHART_STEP,
                    },
                    part: None,
                    black_clear: false,
                };
                view.set_picture(i, Some(picture));
            }
        }
        if let Some(i) = nth_control(dialog, PANEL_CREST, panel) {
            view.set_picture(i, game.rules.crest(&game.world, own).map(Picture::file));
        }
    }
}

/// A number with dots between the thousands: "12.345".
fn thousands(value: i64) -> String {
    let digits = value.abs().to_string();
    let mut text = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            text.push('.');
        }
        text.push(c);
    }
    if value < 0 { format!("- {text}") } else { text }
}

/// The control with `id` of the `n`th panel: panels repeat their ids.
fn nth_control(dialog: &Dialog, id: u16, n: usize) -> Option<usize> {
    dialog.controls.iter().enumerate().filter(|(_, c)| c.id == Some(id)).nth(n).map(|(i, _)| i)
}
