//! A manager without a club looks through the job offers and negotiates a
//! contract: the offer screen with a tab per club, the talks with the club's
//! offer and the manager's demand side by side, and the popups that end
//! them — signed, broken off, or every offer declined.

use a3_core::jobs::{
    self, BONUS, CUP, MAX_YEARS, MONEY, Offer, POINT, Round, SALARY, TITLE, at_position, position,
};
use a3_core::season::{GOALS, Season};
use a3_files::rch::{Dialog, Kind};

use a3_core::cycle::dialogs::QUESTION;
pub use a3_core::cycle::dialogs::{FAILED, OFFERS, SIGNED, TALKS};
use a3_core::state::GameState;

use super::popups::ball;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, indices, set};
use crate::controls::bitmap::Picture;
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

/// The offer screen's controls.
const TABS: u16 = 2069;
const CLUB: u16 = 2042;
const OFFER_MONEY: [u16; MONEY] = [2045, 2046, 2047, 2048, 2050];
const OFFER_GOAL: u16 = 2049;
const OFFER_YEARS: u16 = 2051;
const OFFER_WHEN: u16 = 2052;
const BOARD: u16 = 2053;
const STADIUM: u16 = 2054;
const FANS: u16 = 2055;
const FINANCES: u16 = 2056;
const STANDING: u16 = 2057;
const RESULTS: u16 = 2060;
const CUP_RUN: u16 = 2058;
const EUROPE: u16 = 2059;
const OFFER_ACTIONS: u16 = 2063;
/// Its buttons: talks, the club's squad, and asking one's own club to let
/// one go — which a manager without a club has not.
const NEGOTIATE: usize = 0;
const RELEASE: usize = 2;
/// The talks' controls: the club's offer and the manager's demand.
const CLUB_MONEY: [u16; MONEY] = [2082, 2084, 2086, 2088, 2091];
const DEMAND_MONEY: [u16; MONEY] = [2083, 2085, 2087, 2089, 2092];
const CLUB_GOAL: u16 = 2090;
const DEMAND_GOAL: u16 = 2079;
const CLUB_YEARS: u16 = 2093;
const DEMAND_YEARS: u16 = 2094;
const CLUB_OPTION: u16 = 2095;
const DEMAND_OPTION: u16 = 2096;
const CLUB_EXIT: u16 = 2097;
const DEMAND_EXIT: u16 = 2098;
const CLUB_WHEN: u16 = 2099;
const DEMAND_WHEN: u16 = 2100;
const TALKS_CLUB: u16 = 2102;
const TALKS_LEAGUE: u16 = 2103;
const TALKS_ACTIONS: u16 = 2107;
const SIGN: usize = 1;
/// The demand's spinners: money, goal and years.
const MONEY_SPINS: [u16; MONEY] = [2080, 2110, 2111, 2112, 2081];
const GOAL_SPIN: u16 = 2108;
const YEARS_SPIN: u16 = 2109;
/// Salary and title bonus move in hundreds, the rest in tens, along the
/// money scale.
const MONEY_UNITS: [i64; MONEY] = [100, 10, 10, 100, 10];
/// The most a manager can demand: three times the club's first offer.
const LIMIT_TIMES: i64 = 3;
const SIGNED_TEXT: u16 = 2114;
/// The manager's portrait, which a new manager has not chosen yet.
const PORTRAIT: u16 = 2067;
const NO_PORTRAIT: u16 = 2632;
const TALKS_CREST: u16 = 2101;
/// The club president's face: sixty frames from calm to furious, the one
/// shown picked by the anger; after a round it morphs there a frame every
/// 25 ms, and stops one frame short.
const FACE: u16 = 2104;
const FACE_FRAMES: std::ops::RangeInclusive<u16> = 1620..=1679;
const FACE_BY_ANGER: [usize; 16] = [0, 3, 7, 11, 15, 19, 23, 24, 31, 35, 39, 43, 47, 51, 55, 59];
const MORPH_SECS: f32 = 0.025;

/// Texts by id.
const MONEY_SMALL: u16 = 2157;
const MONEY_THOUSANDS: u16 = 2156;
const MONEY_MILLIONS: u16 = 2155;
const GOAL_NAMES: u16 = 1069;
const BOARD_NAMES: u16 = 1040;
const BOARD_LEVELS: i64 = 6;
const FAN_NAMES: u16 = 1033;
const FAN_LEVELS: i64 = 7;
const FINANCE_NAMES: u16 = 1479;
const FINANCE_LIMITS: [i64; 5] = [-5_000_000, -2_000_000, 0, 2_000_000, 5_000_000];
const NOW: u16 = 1030;
const YES: u16 = 1028;
const NO: u16 = 1029;
const ONE_YEAR_AUTO: u16 = 4262;
const ONE_YEAR: u16 = 1032;
const YEARS: u16 = 1027;
const PROMOTED: u16 = 3657;
const RELEGATED: u16 = 3658;
const NO_CUP: u16 = 1517;
const NO_EUROPE: u16 = 2488;
const NOW_MANAGER: u16 = 1545;
const NO_CONTRACT: u16 = 5001;
const NO_STANDING: &str = "----";
/// Last season's result in a club's file.
const RESULT_PROMOTED: i64 = 4;
const RESULT_RELEGATED: i64 = 3;

/// The offers: a tab per club, its offer and what is known about it.
#[derive(Default)]
pub struct Offers {
    /// The offer looked at.
    tab: usize,
}

impl Screen for Offers {
    fn ids(&self) -> &[u16] {
        &[OFFERS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        self.tab = if let Args::Offer(tab) = args { tab } else { 0 };
        ball(view, cx);
        let Some(season) = &cx.state.season else { return };
        show_offers(view, cx.dialog, cx.game, season, self.tab);
        if let Some(i) = control(cx.dialog, PORTRAIT) {
            view.set_picture(i, Some(Picture::graphic(NO_PORTRAIT)));
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let id_of = |control: usize| cx.dialog.controls[control].id;
        match target {
            Target::Option { control, item } if id_of(control) == Some(TABS) => {
                self.tab = item;
                if let Some(season) = &cx.state.season {
                    show_offers(view, cx.dialog, cx.game, season, self.tab);
                }
                Outcome::Stay
            }
            // Talks about the offer looked at.
            Target::Option { control, item } if id_of(control) == Some(OFFER_ACTIONS) => {
                if item == NEGOTIATE {
                    Outcome::Close(Reply::Choice(self.tab))
                } else {
                    Outcome::Stay
                }
            }
            // Going without a club needs saying twice.
            Target::Button(_) => Outcome::Popup(Request::with(QUESTION, Args::Text(NO_CONTRACT))),
            _ => Outcome::Stay,
        }
    }

    /// Yes to going without a club.
    fn answer(&mut self, _: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        match reply {
            Reply::Ok => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }
}

/// The talks: the club's offer and the manager's demand, and the club
/// president's face. They end signed — OK — or with cancel, back to the
/// offers or broken off by the club.
#[derive(Default)]
pub struct Talks {
    /// The offer talked about.
    tab: usize,
    /// Whether the talks began with their first round.
    fresh: bool,
    /// The face on its way to a frame, and how long since it last moved.
    morph: Option<(usize, usize)>,
    timer: f32,
}

impl Screen for Talks {
    fn ids(&self) -> &[u16] {
        &[TALKS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        self.tab = if let Args::Offer(tab) = args { tab } else { 0 };
        self.timer = 0.0;
        self.morph = None;
        ball(view, cx);
        let Some(season) = &mut cx.state.season else { return };
        let manager = &mut season.managers[season.current];
        let Some(offer) = manager.offers.get_mut(self.tab) else { return };
        // The demand can go up to three times what the club first offers.
        offer.limit.get_or_insert(offer.offer.money.map(|m| m * LIMIT_TIMES));
        offer.demand.years = offer.demand.years.max(1);
        self.fresh = offer.rounds == 0;
        if let Some(face) = control(cx.dialog, FACE).and_then(|f| view.anim_mut(f)) {
            face.frames = FACE_FRAMES.collect();
            face.frame = face_frame(offer.anger);
        }
        if let Some(i) = control(cx.dialog, TALKS_CREST) {
            view.set_picture(i, cx.game.rules.crest(&cx.game.world, offer.club).map(Picture::file));
        }
        show_talks(view, cx.dialog, cx.game, offer);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let id_of = |control: usize| cx.dialog.controls[control].id;
        match target {
            Target::Spin { control, up } => {
                if let Some(offer) = offer(cx.state, self.tab) {
                    step_demand(offer, cx.game, id_of(control).unwrap_or(0), up);
                    show_talks(view, cx.dialog, cx.game, offer);
                }
                Outcome::Stay
            }
            Target::Control(control) => {
                if let Some(offer) = offer(cx.state, self.tab) {
                    match id_of(control) {
                        Some(DEMAND_OPTION) => {
                            offer.demand.option = i32::from(offer.demand.option == 0)
                        }
                        Some(DEMAND_EXIT) => offer.demand.exit = !offer.demand.exit,
                        _ => {}
                    }
                    show_talks(view, cx.dialog, cx.game, offer);
                }
                Outcome::Stay
            }
            Target::Option { control, item } if id_of(control) == Some(TALKS_ACTIONS) => {
                if item == SIGN { Outcome::Close(Reply::Ok) } else { self.talk(cx, view) }
            }
            Target::Button(_) => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }

    /// The face morphs a frame closer to where the anger puts it, and stops
    /// one short, as the game's morph does.
    fn tick(&mut self, _: &mut Cx, view: &mut View, seconds: f32) -> Outcome {
        self.timer += seconds;
        while let Some((face, target)) = self.morph
            && self.timer >= MORPH_SECS
        {
            self.timer -= MORPH_SECS;
            let Some(state) = view.anim_mut(face) else { break };
            let next =
                if target > state.frame { state.frame + 1 } else { state.frame.saturating_sub(1) };
            if next == target || state.frame == target {
                self.morph = None;
            } else {
                state.frame = next;
            }
        }
        Outcome::Stay
    }
}

impl Talks {
    /// A round of talks.
    fn talk(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let Some(season) = &mut cx.state.season else { return Outcome::Stay };
        let current = season.current;
        let skill = 3 * season.managers[current].negotiating;
        let Some(offer) = season.managers[current].offers.get_mut(self.tab) else {
            return Outcome::Stay;
        };
        let board = board_of(cx.game, offer);
        let rules = &cx.game.rules;
        let round = jobs::negotiate(offer, rules, skill, board, self.fresh, &mut season.random);
        if let Some(face) = control(cx.dialog, FACE) {
            self.morph = Some((face, face_frame(offer.anger)));
            self.timer = 0.0;
        }
        show_talks(view, cx.dialog, cx.game, offer);
        match round {
            Round::Going => Outcome::Stay,
            Round::BrokenOff => Outcome::Close(Reply::Cancel),
        }
    }
}

/// The talks broke off.
pub struct Failed;

impl Screen for Failed {
    fn ids(&self) -> &[u16] {
        &[FAILED]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        ball(view, cx);
    }

    fn input(&mut self, _: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(_) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// The contract is signed.
pub struct Signed;

impl Screen for Signed {
    fn ids(&self) -> &[u16] {
        &[SIGNED]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        ball(view, cx);
        let club = cx.state.season.as_ref().and_then(|s| s.managers[s.current].club);
        if let (Some(i), Some(club)) = (control(cx.dialog, SIGNED_TEXT), club) {
            let manager = cx.game.with_club(cx.game.text(NOW_MANAGER), club);
            view.set_text(i, cx.dialog.controls[i].text.replacen("%s", &manager, 1));
        }
    }

    fn input(&mut self, _: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(_) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// The current manager's offer on `tab`.
fn offer(state: &mut GameState, tab: usize) -> Option<&mut Offer> {
    let season = state.season.as_mut()?;
    let current = season.current;
    season.managers[current].offers.get_mut(tab)
}

fn face_frame(anger: i32) -> usize {
    FACE_BY_ANGER[anger.clamp(0, FACE_BY_ANGER.len() as i32 - 1) as usize]
}

fn board_of(game: &Game, offer: &Offer) -> i64 {
    let (slot, club) = offer.club;
    game.world.country(slot).and_then(|c| c.clubs.get(club)).map_or(0, |c| c.board)
}

/// One click on a spinner of the demand.
fn step_demand(offer: &mut Offer, game: &Game, spin: u16, up: bool) {
    let step = if up { 1 } else { -1 };
    if let Some(i) = MONEY_SPINS.iter().position(|&s| s == spin) {
        let unit = MONEY_UNITS[i];
        let limit = offer.limit.map_or(i64::MAX, |l| l[i]);
        let top = position(limit / unit);
        let at = (position(offer.demand.money[i] / unit) + step).clamp(0, top);
        offer.demand.money[i] = at_position(at) * unit;
    } else if spin == GOAL_SPIN {
        // Up asks for a harder goal, down for an easier one.
        let table = if up { &game.rules.harder } else { &game.rules.easier };
        offer.demand.goal = usize::from(table[offer.demand.goal.min(GOALS - 1)]);
    } else if spin == YEARS_SPIN {
        offer.demand.years = (offer.demand.years + step as i32).clamp(0, MAX_YEARS);
    }
}

/// The offer screen: a tab per club, its offer and what is known about it.
fn show_offers(view: &mut View, dialog: &Dialog, game: &Game, season: &Season, tab: usize) {
    let manager = &season.managers[season.current];
    if let Some(state) = control(dialog, TABS).and_then(|t| view.options_mut(t)) {
        state.chosen = tab;
        let count = state.labels.len();
        state.off = (0..count).map(|i| !manager.offers.get(i).is_some_and(|o| o.open)).collect();
        state.labels = (0..count)
            .map(|i| match manager.offers.get(i).filter(|o| o.open) {
                Some(offer) => club_name(game, offer).to_owned(),
                None => String::new(),
            })
            .collect();
    }
    if let Some(state) = control(dialog, OFFER_ACTIONS).and_then(|a| view.options_mut(a)) {
        state.actions = true;
        state.off = (0..state.labels.len()).map(|i| i == RELEASE).collect();
        if let Some(label) = state.labels.get_mut(RELEASE) {
            label.clear();
        }
    }
    let Some(offer) = manager.offers.get(tab) else { return };
    let (slot, index) = offer.club;
    let Some(club) = game.world.country(slot).and_then(|c| c.clubs.get(index)) else { return };
    let league = game.world.leagues_in_play(slot, season.main).get(offer.league);
    let league = league.map_or("", |l| l.name.as_str());
    set(view, dialog, CLUB, format!("{} ({league})", club.name));
    for (i, &id) in OFFER_MONEY.iter().enumerate() {
        set(view, dialog, id, money(game, offer.offer.money[i]));
    }
    set(view, dialog, OFFER_GOAL, goal(game, offer.offer.goal));
    set(view, dialog, OFFER_YEARS, offer.offer.years.to_string());
    set(view, dialog, OFFER_WHEN, game.text(NOW).to_owned());
    let named = |first: u16, levels: i64, value: i64, fallback: &str| {
        if (0..levels).contains(&value) {
            game.text(first + value as u16).to_owned()
        } else {
            fallback.to_owned()
        }
    };
    set(view, dialog, BOARD, named(BOARD_NAMES, BOARD_LEVELS, club.board, "Vorstand???"));
    set(view, dialog, STADIUM, club.capacity.to_string());
    set(view, dialog, FANS, named(FAN_NAMES, FAN_LEVELS, club.fans, "Fans???"));
    let finances = FINANCE_LIMITS.iter().position(|&limit| club.money < limit);
    let finances = finances.unwrap_or(FINANCE_LIMITS.len()) as u16;
    set(view, dialog, FINANCES, game.text(FINANCE_NAMES + finances).to_owned());
    // Before the first matchday: how last season ended.
    let standing = match club.last_season & 7 {
        RESULT_PROMOTED => game.text(PROMOTED).to_owned(),
        RESULT_RELEGATED => game.text(RELEGATED).to_owned(),
        _ => NO_STANDING.to_owned(),
    };
    set(view, dialog, STANDING, standing);
    set(view, dialog, RESULTS, String::new());
    set(view, dialog, CUP_RUN, game.text(NO_CUP).to_owned());
    set(view, dialog, EUROPE, game.text(NO_EUROPE).to_owned());
}

/// The talks: the club's offer and the demand, item by item.
fn show_talks(view: &mut View, dialog: &Dialog, game: &Game, offer: &Offer) {
    let (slot, index) = offer.club;
    if let Some(club) = game.world.country(slot).and_then(|c| c.clubs.get(index)) {
        set(view, dialog, TALKS_CLUB, club.name.clone());
    }
    let league = game.world.leagues_in_play(slot, slot).get(offer.league);
    set(view, dialog, TALKS_LEAGUE, league.map_or(String::new(), |l| l.name.clone()));
    for i in [SALARY, POINT, CUP, TITLE, BONUS] {
        set(view, dialog, CLUB_MONEY[i], money(game, offer.offer.money[i]));
        set(view, dialog, DEMAND_MONEY[i], money(game, offer.demand.money[i]));
    }
    set(view, dialog, CLUB_GOAL, goal(game, offer.offer.goal));
    set(view, dialog, DEMAND_GOAL, goal(game, offer.demand.goal));
    set(view, dialog, CLUB_YEARS, years(game, offer.offer.years));
    set(view, dialog, DEMAND_YEARS, years(game, offer.demand.years));
    let yes_no = |yes: bool| game.text(if yes { YES } else { NO }).to_owned();
    set(view, dialog, CLUB_OPTION, yes_no(offer.offer.option != 0));
    set(view, dialog, DEMAND_OPTION, yes_no(offer.demand.option != 0));
    set(view, dialog, CLUB_EXIT, yes_no(offer.offer.exit));
    set(view, dialog, DEMAND_EXIT, yes_no(offer.demand.exit));
    set(view, dialog, CLUB_WHEN, game.text(NOW).to_owned());
    set(view, dialog, DEMAND_WHEN, game.text(NOW).to_owned());
    for id in [DEMAND_OPTION, DEMAND_EXIT] {
        if let Some(i) = control(dialog, id) {
            view.set_clickable(i, true);
        }
    }
    if let Some(actions) = control(dialog, TALKS_ACTIONS).and_then(|a| view.options_mut(a)) {
        actions.actions = true;
    }
    // Spinners have no range of their own here; the demand sets the limits.
    for spin in indices(dialog, |k| matches!(k, Kind::Spin)) {
        if let Some(spin) = view.value_mut(spin) {
            spin.range = (i32::MIN, i32::MAX);
        }
    }
}

fn club_name<'a>(game: &'a Game, offer: &Offer) -> &'a str {
    let (slot, index) = offer.club;
    game.world.country(slot).and_then(|c| c.clubs.get(index)).map_or("", |c| c.name.as_str())
}

/// Money as the game writes it: "850 DM", "350.000 DM", "1.25 Mio DM".
fn money(game: &Game, value: i64) -> String {
    let value = value / 5 * 5;
    let (format, parts) = match value.abs() {
        v if v < 1000 => (MONEY_SMALL, vec![v]),
        v if v < 1_000_000 => (MONEY_THOUSANDS, vec![v / 1000, v % 1000]),
        v => (MONEY_MILLIONS, vec![v / 1_000_000, v % 1_000_000 / 10_000]),
    };
    let format = game.text(format);
    // A sign in front, which positive amounts leave out.
    let format =
        if value < 0 { format.replacen("%c", "-", 1) } else { format.replacen("%c ", "", 1) };
    let mut text = String::new();
    let mut parts = parts.into_iter();
    let mut rest = format.as_str();
    while let Some(at) = rest.find('%') {
        text.push_str(&rest[..at]);
        let spec = &rest[at..];
        let end = spec.find('d').map_or(spec.len(), |d| d + 1);
        let width = spec[1..end - 1].trim_start_matches('0').parse::<usize>().unwrap_or(0);
        let value = parts.next().unwrap_or(0);
        text.push_str(&format!("{value:0width$}"));
        rest = &spec[end..];
    }
    text.push_str(rest);
    text
}

fn goal(game: &Game, goal: usize) -> String {
    game.text(GOAL_NAMES + goal.min(GOALS - 1) as u16).to_owned()
}

fn years(game: &Game, years: i32) -> String {
    match years {
        0 => game.text(ONE_YEAR_AUTO).to_owned(),
        1 => game.text(ONE_YEAR).to_owned(),
        n => game.text(YEARS).replace("%1", &n.to_string()),
    }
}
