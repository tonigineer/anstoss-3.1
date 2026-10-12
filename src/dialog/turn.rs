//! The season, screen by screen: what the managers see once a new game has
//! been calculated.
//!
//! The game runs in weeks. Each week starts with the autosave's wait screen,
//! then every human manager takes a turn on the main screens — the line-up
//! with the menu bar (see [`hub`]) or, for a manager who chose it, the
//! office; the whistle at the end of the bar, or the jacket on the office
//! door, ends it, after the game has asked about anything left undone. When
//! all are done, the week runs on its own screen, day by day, with a panel
//! for every human club, and the next one begins.

use std::collections::HashMap;

use bevy::prelude::*;

use super::flow::{Action, Session, control, indices};
use super::{Picture, Target, View, hiring, hub};
use crate::game::Game;
use crate::rch::{Align, Control, Dialog, Kind};
use crate::season::{Match, Season};

pub const WAIT: u16 = 381;
const OFFICE: u16 = 483;
/// The grand office of the grandest clubs.
const GRAND_OFFICE: u16 = 504;
pub const QUESTION: u16 = 384;
/// The week screen and the panel it shows for each human club.
const WEEK: u16 = 2;
const WEEK_PANEL: u16 = 460;
/// The week screen put together with no panel, one, … four, under ids of
/// its own; the game draws the frames of its panels into the background.
const WEEKS: [u16; 5] = [1000, 1001, 1002, 1003, 1004];
const WEEK_BACKGROUNDS: [u16; 5] = [2835, 2798, 2799, 2800, 2801];
const PANEL_LEFT: f32 = 128.0;
const PANEL_TOPS: [&[f32]; 5] =
    [&[], &[162.0], &[125.0, 257.0], &[30.0, 162.0, 294.0], &[24.0, 120.0, 216.0, 312.0]];
pub const DIALOGS: [u16; 9] =
    [WAIT, OFFICE, GRAND_OFFICE, QUESTION, WEEKS[0], WEEKS[1], WEEKS[2], WEEKS[3], WEEKS[4]];

/// Clubs from this class on have the grand office.
const GRAND_CLASS: i64 = 5;
/// The manager's choice of the office over the menu bar.
const OFFICE_MENU: u8 = 1;

/// Controls by their ids in the dialog file.
const WAIT_DETAIL: u16 = 6363;
const OFFICE_TITLE: u16 = 6353;
const OFFICE_PAIRING: u16 = 6354;
const OFFICE_DOOR: u16 = 6131;
/// The national-team trophy and the remote control: the only objects a
/// manager without a club can use, besides the door.
const OFFICE_NATIONAL: u16 = 6315;
const OFFICE_REMOTE: u16 = 6121;
/// The long-term and individual training boards, closed on Saturdays.
const OFFICE_SATURDAY_CLOSED: [u16; 2] = [6127, 6129];
const QUESTION_TEXT: u16 = 4738;
const QUESTION_OK: u16 = 4741;
const QUESTION_BALL: u16 = 4740;
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
/// Ids for the tiles the week screen marks its elapsed days with, and the
/// crest on the office pennant, which the game draws itself.
const DAY_TILE: u16 = 9900;
const OFFICE_CREST_ID: u16 = 9910;
const OFFICE_PENNANT: u16 = 6130;
const OFFICE_CREST: Vec2 = Vec2::new(11.0, 27.0);
const GRAND_OFFICE_CREST: Vec2 = Vec2::new(9.0, 22.0);
const CREST_SIZE: f32 = 32.0;

/// Texts by id.
const DATE: u16 = 1997;
const HOME: u16 = 3664;
const AWAY: u16 = 3665;
const HOME_RANKED: u16 = 4242;
const AWAY_RANKED: u16 = 4243;
const NO_TRAINING: u16 = 3238;
const NO_CONTRACT: u16 = 5001;
const WEEK_HOME: u16 = 2843;
const WEEK_AWAY: u16 = 2844;
const SEASON_TICKETS: u16 = 5008;

/// The elapsed days' tiles: the weekday names of `woche/wochD02.bmp`, one
/// above the other every 63 pixels, laid over the grey column of the screen.
const DAY_TILES: u16 = 1385;
const DAY_TILE_SIZE: (u32, u32) = (120, 56);
const DAY_TILE_STEP: u32 = 63;
const DAY_TILE_TOPS: [f32; 7] = [26.0, 89.0, 152.0, 215.0, 278.0, 341.0, 404.0];
/// The weather, graphics 1513 to 1525; the game's weather map for Germany
/// starts the season fair.
const WEATHER: u16 = 1512;
const FAIR: u16 = 10;
/// The ball of the question popup, playing its frames forth and back.
const BALL_FRAMES: std::ops::RangeInclusive<u16> = 1745..=1753;
/// The week screen shows the next match within this many days.
const FIXTURE_AHEAD: usize = 63;

/// The wait screen stands at least this long, as saving takes the game.
const WAIT_SECS: f32 = 1.2;
/// A day of the week's run takes at least this long on screen.
const DAY_SECS: f32 = 0.3;
const WEEK_DAYS: usize = 7;
/// What the game takes for Saturday.
const SATURDAY: usize = 6;

/// What the open question asks before going on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Question {
    /// No training for the coming week: end the turn anyway?
    NoTraining,
    /// No contract signed: start the season without a club?
    Decline,
}

/// Puts the week screens together: the week dialog with its background for
/// the number of panels, the panels in their places and the tiles for the
/// days.
pub fn compose(dialogs: &mut HashMap<u16, Dialog>) {
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
    // The office pennant carries the club's crest.
    for (office, at) in [(OFFICE, OFFICE_CREST), (GRAND_OFFICE, GRAND_OFFICE_CREST)] {
        let Some(dialog) = dialogs.get_mut(&office) else { continue };
        let pennant = dialog.controls.iter().find(|c| c.id == Some(OFFICE_PENNANT));
        let Some(origin) = pennant.map(|c| c.rect.min) else { continue };
        dialog.controls.push(bitmap(origin + at, CREST_SIZE, OFFICE_CREST_ID));
    }
}

/// A week panel: the band of its title, the boxes of its charts, of the
/// next match and of the news, from its top left corner.
const PANEL_SIZE: Vec2 = Vec2::new(507.0, 100.0);
const PANEL_BAND: f32 = 18.0;
const PANEL_BOXES: [Rect; 4] = [
    Rect { min: Vec2::new(2.0, 52.0), max: Vec2::new(174.0, 82.0) },
    Rect { min: Vec2::new(178.0, 17.0), max: Vec2::new(222.0, 96.0) },
    Rect { min: Vec2::new(229.0, 17.0), max: Vec2::new(273.0, 96.0) },
    Rect { min: Vec2::new(279.0, 22.0), max: Vec2::new(497.0, 90.0) },
];
/// The week screen's bottom: the strip with the weather, and the news box.
const WEATHER_STRIP: Rect = Rect { min: Vec2::new(106.0, 436.0), max: Vec2::new(402.0, 466.0) };
const NEWS_BOX: Rect = Rect { min: Vec2::new(405.0, 416.0), max: Vec2::new(575.0, 472.0) };

/// Whether a screen is the week running.
pub(super) fn is_week(id: u16) -> bool {
    WEEKS.contains(&id)
}

/// What the remastered week draws behind its controls, which the classic
/// one has painted into its pictures: the days as they stand before they
/// have run, a card for every panel with the band of its title and its
/// boxes, and the strip along the bottom.
pub(super) fn decorations(id: u16) -> Vec<super::Decoration> {
    use super::{Decoration, Picture};
    let Some(count) = WEEKS.iter().position(|&w| w == id) else { return Vec::new() };
    let (w, h) = DAY_TILE_SIZE;
    let mut decorations: Vec<Decoration> = DAY_TILE_TOPS
        .iter()
        .map(|&top| {
            let part = [0, top as u32, w, h];
            let at = Rect::new(0.0, top, w as f32, top + h as f32);
            let picture = Picture { part: Some(part), ..Picture::graphic(WEEK_BACKGROUNDS[0]) };
            Decoration::Picture { picture, at, stretch: false }
        })
        .collect();
    for &top in PANEL_TOPS[count] {
        let origin = Vec2::new(PANEL_LEFT, top);
        decorations.push(Decoration::Card(Rect::from_corners(origin, origin + PANEL_SIZE)));
        let band = Vec2::new(PANEL_SIZE.x, PANEL_BAND);
        decorations.push(Decoration::Band(Rect::from_corners(origin, origin + band)));
        decorations.extend(
            PANEL_BOXES
                .iter()
                .map(|b| Decoration::Card(Rect::from_corners(origin + b.min, origin + b.max))),
        );
    }
    decorations.extend([Decoration::Band(WEATHER_STRIP), Decoration::Card(NEWS_BOX)]);
    decorations
}

/// A bitmap control of the game's own, which the dialog file does not have.
fn bitmap(at: Vec2, size: f32, id: u16) -> Control {
    Control {
        kind: Kind::Bitmap,
        rect: Rect::from_corners(at, at + Vec2::splat(size)),
        align: Align::Left,
        text: String::new(),
        id: Some(id),
        bold: false,
    }
}

/// Where a new season's flow begins once it has been calculated: with the
/// job offers for managers without a club. Then, and every week after, the
/// wait screen.
pub fn start(session: &mut Session) -> Action {
    hiring::next(session).unwrap_or_else(|| begin(session))
}

pub fn begin(session: &mut Session) -> Action {
    session.timer = 0.0;
    Action::Open(WAIT)
}

pub fn handles(id: u16) -> bool {
    DIALOGS.contains(&id) || hiring::DIALOGS.contains(&id) || hub::DIALOGS.contains(&id)
}

pub fn setup(view: &mut View, dialog: &Dialog, game: &Game, session: &mut Session) {
    session.timer = 0.0;
    if hiring::DIALOGS.contains(&view.id) {
        return hiring::setup(view, dialog, game, session);
    }
    if hub::DIALOGS.contains(&view.id) {
        return hub::setup(view, dialog, game, session);
    }
    let Some(season) = &session.season else { return };
    match view.id {
        WAIT => set_text(view, dialog, WAIT_DETAIL, String::new()),
        OFFICE | GRAND_OFFICE => show_office(view, dialog, game, season),
        QUESTION => {
            let text = match session.question {
                Some(Question::NoTraining) | None => game.text(NO_TRAINING),
                Some(Question::Decline) => game.text(NO_CONTRACT),
            };
            set_text(view, dialog, QUESTION_TEXT, text.to_owned());
            if let Some(ball) = control(dialog, QUESTION_BALL) {
                let state = &mut view.controls[ball];
                state.frames = BALL_FRAMES.chain(BALL_FRAMES.rev().skip(1)).collect();
                state.frames.pop();
                state.playing = true;
            }
        }
        id if WEEKS.contains(&id) => show_week(view, dialog, game, season),
        _ => {}
    }
}

pub fn act(
    view: &mut View,
    dialog: &Dialog,
    game: &Game,
    session: &mut Session,
    target: Target,
) -> Action {
    if hiring::DIALOGS.contains(&view.id) {
        return hiring::act(view, dialog, game, session, target);
    }
    if hub::DIALOGS.contains(&view.id) {
        return hub::act(view, dialog, session, target);
    }
    let Target::Button(button) = target else { return Action::Stay };
    let id = dialog.controls[button].id;
    match view.id {
        OFFICE | GRAND_OFFICE if id == Some(OFFICE_DOOR) => {
            // Training is never set for the coming week, so the game asks.
            session.question = Some(Question::NoTraining);
            session.hub = view.id;
            Action::Open(QUESTION)
        }
        QUESTION => {
            let question = session.question.take();
            let ok = id == Some(QUESTION_OK);
            match question {
                Some(Question::NoTraining) | None if ok => end_turn(game, session),
                Some(Question::NoTraining) | None => Action::Open(session.hub),
                Some(Question::Decline) if ok => hiring::decline(session),
                Some(Question::Decline) => Action::Open(hiring::OFFERS),
            }
        }
        _ => Action::Stay,
    }
}

/// The current manager's turn: on the main screens with the menu bar, or
/// in the office for a manager who goes about it from there.
pub fn turn(game: &Game, session: &mut Session) -> Action {
    let manager = session.season.as_ref().and_then(Season::manager);
    if manager.is_none_or(|m| m.menu != OFFICE_MENU) {
        return hub::open(session);
    }
    let grand = manager
        .and_then(|m| m.club)
        .and_then(|(slot, club)| game.world.country(slot)?.clubs.get(club))
        .is_some_and(|club| club.class >= GRAND_CLASS);
    Action::Open(if grand { GRAND_OFFICE } else { OFFICE })
}

/// The next manager's turn, or the week runs.
fn end_turn(game: &Game, session: &mut Session) -> Action {
    let Some(season) = &mut session.season else { return Action::Open(super::START) };
    if season.current + 1 < season.managers.len() {
        season.current += 1;
        return turn(game, session);
    }
    season.current = 0;
    // Matches are not played yet: the season stops before the first one.
    let first_match = season
        .managers
        .iter()
        .filter_map(|m| season.next_match(&game.world, &game.calendar, m.club?))
        .min_by_key(|m| m.day);
    if let Some(m) = first_match.filter(|m| m.day < season.day + WEEK_DAYS) {
        let date = crate::calendar::date(season.year, m.day);
        info!(
            "{}, {}. Spieltag am {}: here the match would be played; back to the start menu",
            m.league,
            m.matchday,
            date.format("%-d.%-m.%Y")
        );
        return Action::Open(super::START);
    }
    season.start_week();
    session.week_start = season.day;
    session.timer = 0.0;
    let clubs = season.managers.iter().filter(|m| m.club.is_some()).count();
    Action::Open(WEEKS[clubs.min(WEEKS.len() - 1)])
}

/// Moves timed screens on: the wait screen to the turns, the week through
/// its days.
pub fn tick(view: &mut View, game: &Game, session: &mut Session, seconds: f32) -> Action {
    session.timer += seconds;
    if view.id == hiring::TALKS {
        while session.morph.is_some() && session.timer >= hiring::MORPH_SECS {
            session.timer -= hiring::MORPH_SECS;
            hiring::morph(view, session);
        }
        return Action::Stay;
    }
    let Some(season) = &mut session.season else { return Action::Stay };
    match view.id {
        WAIT if session.timer >= WAIT_SECS => {
            season.current = 0;
            turn(game, session)
        }
        id if WEEKS.contains(&id) && session.timer >= DAY_SECS => {
            session.timer -= DAY_SECS;
            let done = season.day - session.week_start;
            if done == WEEK_DAYS {
                return begin(session);
            }
            // A day has run: its tile lights up, the panels move on.
            season.pass_day();
            season.day += 1;
            let dialog = &game.dialogs[&view.id];
            if let Some(tile) = control(dialog, DAY_TILE + done as u16) {
                view.controls[tile].hidden = false;
            }
            show_panels(view, dialog, game, season);
            Action::Stay
        }
        _ => Action::Stay,
    }
}

/// The office: who sits in it and when, the next match, and the objects a
/// manager can use today.
fn show_office(view: &mut View, dialog: &Dialog, game: &Game, season: &Season) {
    let Some(manager) = season.manager() else { return };
    let date = season.date();
    let date = game
        .text(DATE)
        .replace("%1", &date.format("%-d").to_string())
        .replace("%2", &date.format("%-m").to_string())
        .replace("%3", &date.format("%Y").to_string());
    let club = manager.club.and_then(|(slot, club)| game.world.country(slot)?.clubs.get(club));
    // The game leaves an empty word in front of the name.
    let title = match club {
        Some(club) => {
            format!(" {} {} ({}), {date}", manager.first_name, manager.last_name, club.name)
        }
        None => format!(" {} {}, {date}", manager.first_name, manager.last_name),
    };
    set_text(view, dialog, OFFICE_TITLE, title);
    let pairing = manager
        .club
        .and_then(|club| season.next_match(&game.world, &game.calendar, club).map(|m| (club, m)))
        .map_or_else(
            || club.map(|c| c.name.clone()).unwrap_or_default(),
            |(club, m)| pairing(game, club, &m),
        );
    set_text(view, dialog, OFFICE_PAIRING, pairing);
    if let Some(crest) = control(dialog, OFFICE_CREST_ID) {
        view.controls[crest].picture =
            manager.club.and_then(|c| game.rules.crest(&game.world, c)).map(Picture::file);
    }

    let saturday = season.day % WEEK_DAYS == SATURDAY;
    for button in indices(dialog, |k| matches!(k, Kind::Button { .. })) {
        let id = dialog.controls[button].id.unwrap_or(0);
        view.controls[button].disabled = match id {
            OFFICE_DOOR | OFFICE_REMOTE => false,
            OFFICE_NATIONAL => manager.national_team.is_none(),
            _ if manager.club.is_none() => true,
            _ => saturday && OFFICE_SATURDAY_CLOSED.contains(&id),
        };
    }
}

/// The week as it starts: a panel for every human club with its next match,
/// its manager and the board's confidence, fair weather and no day done yet.
fn show_week(view: &mut View, dialog: &Dialog, game: &Game, season: &Season) {
    if let Some(weather) = control(dialog, WEEK_WEATHER) {
        view.controls[weather].picture = Some(Picture::graphic(WEATHER + FAIR));
    }
    for day in 0..WEEK_DAYS {
        if let Some(tile) = control(dialog, DAY_TILE + day as u16) {
            let (w, h) = DAY_TILE_SIZE;
            view.controls[tile].picture = Some(Picture {
                source: super::PictureSource::Graphic(DAY_TILES),
                part: Some([0, DAY_TILE_STEP * day as u32, w, h]),
                black_clear: false,
            });
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
                view.controls[i].text = text;
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
                view.controls[i].text = match values.iter().rev().find(|&&v| v != 0) {
                    Some(v) => format!("{label} {v}"),
                    None => label.clone(),
                };
            }
            if let Some(i) = nth_control(dialog, chart, panel) {
                let size = dialog.controls[i].rect.size();
                view.controls[i].picture = Some(Picture {
                    source: super::PictureSource::Chart {
                        values,
                        size: (size.x as u32, size.y as u32),
                        step: CHART_STEP,
                    },
                    part: None,
                    black_clear: false,
                });
            }
        }
        if let Some(i) = nth_control(dialog, PANEL_CREST, panel) {
            view.controls[i].picture = game.rules.crest(&game.world, own).map(Picture::file);
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

/// "Own club (H) - opponent (A)": the manager's club first, each side marked
/// home or away, with its table position once the league has begun.
fn pairing(game: &Game, own: (usize, usize), m: &Match) -> String {
    let name = |(slot, club): (usize, usize)| {
        game.world.country(slot).and_then(|c| c.clubs.get(club)).map_or("", |c| c.name.as_str())
    };
    let home = m.home == own;
    let opponent = if home { m.away } else { m.home };
    // There is no table before the first matchday, and so no positions.
    format!("{} - {}", side(game, name(own), home, None), side(game, name(opponent), !home, None))
}

/// One side of a pairing: "Name (H)", or "Name (H, 3.)" with its position.
fn side(game: &Game, name: &str, home: bool, position: Option<usize>) -> String {
    match position {
        Some(position) => game
            .text(if home { HOME_RANKED } else { AWAY_RANKED })
            .replacen("%s", name, 1)
            .replacen("%d", &position.to_string(), 1),
        None => game.text(if home { HOME } else { AWAY }).replacen("%s", name, 1),
    }
}

fn set_text(view: &mut View, dialog: &Dialog, id: u16, text: String) {
    if let Some(i) = control(dialog, id) {
        view.controls[i].text = text;
    }
}

/// The control with `id` of the `n`th panel: panels repeat their ids.
fn nth_control(dialog: &Dialog, id: u16, n: usize) -> Option<usize> {
    dialog.controls.iter().enumerate().filter(|(_, c)| c.id == Some(id)).nth(n).map(|(i, _)| i)
}
