//! The game cycle: the game as a sequential program, on a thread of its
//! own, that asks for a screen whenever it needs the player and waits for
//! the reply — as the game's logic thread asks its main thread for dialogs.
//! What comes next is plain control flow here: the start menu, a new game,
//! the job offers, and the weeks of the season up to its first match.
//!
//! The screens share the [`GameState`] with the cycle; while it waits for a
//! reply, the open screen may read and change the state.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use a3_files::an3::Save;
use chrono::Datelike;
use tracing::{info, warn};

use crate::data::Data;
use crate::load;
use crate::medical;
use crate::newgame::{self, Mode, Setup};
use crate::random::Random;
use crate::records::Records;
use crate::season::Season;
use crate::state::GameState;

/// The dialogs the cycle asks for, by their ids in the dialog file; the
/// week screen under ids of its own for none to four club panels.
pub mod dialogs {
    pub const START: u16 = 9;
    pub const LOAD: u16 = 424;
    pub const HIGH_SCORES: u16 = 95;
    pub const NET_NAME: u16 = 474;
    pub const NET_DETAILS: u16 = 473;
    pub const NET_GAMES: u16 = 416;
    pub const COUNTRIES: u16 = 140;
    pub const PLAYERS: u16 = 209;
    pub const NAMES: u16 = 212;
    pub const CLUB: u16 = 304;
    pub const NATIONAL_TEAM: u16 = 352;
    pub const ABILITIES: u16 = 374;
    pub const BIRTHDAY: u16 = 491;
    pub const SETTINGS: u16 = 256;
    pub const OFFERS: u16 = 99;
    pub const TALKS: u16 = 103;
    pub const FAILED: u16 = 102;
    pub const SIGNED: u16 = 104;
    pub const NOTICE: u16 = 380;
    pub const QUESTION: u16 = 384;
    pub const WAIT: u16 = 381;
    pub const OFFICE: u16 = 483;
    /// The grand office of the grandest clubs.
    pub const GRAND_OFFICE: u16 = 504;
    pub const LINEUP: u16 = 147;
    /// The line-up as a list, for a manager who switched to it.
    pub const LINEUP_LIST: u16 = 429;
    pub const DRESSING_ROOM: u16 = 302;
    /// The comparison with the next opponent, man for man, the bar's
    /// second symbol.
    pub const COMPARISON: u16 = 78;
    /// The club's tactics, the bar's third symbol.
    pub const TACTICS: u16 = 271;
    /// The week's training, the bar's fifth symbol: as text, or as
    /// pictures for a manager who switched to them; the popup of its
    /// units.
    pub const SHORT_TRAINING: u16 = 346;
    pub const TRAINING_PICTURES: u16 = 513;
    pub const UNIT_CHOICE: u16 = 362;
    pub const WEEKS: [u16; 5] = [1000, 1001, 1002, 1003, 1004];
    /// A talk with a player, on the day it was booked for.
    pub const TALK: u16 = 461;
    pub const VISIT: u16 = 210;
}

use self::dialogs::*;

/// The start menu's entries, in order.
const NEW_GAME: usize = 0;
const LOAD_GAME: usize = 1;
const HOST_NEW: usize = 2;
const HOST_SAVED: usize = 3;
const JOIN: usize = 4;
const SCORES: usize = 5;
/// "Niemand hat Sie mehr im Angebot": every offer is gone.
const NO_ONE_LEFT: u16 = 5053;
/// Clubs from this class on have the grand office.
const GRAND_CLASS: i64 = 5;
/// The manager's choice of the office over the menu bar.
const OFFICE_MENU: u8 = 1;
const WEEK_DAYS: usize = 7;
/// A talk booked in the week planner, and the talk itself.
const TALK: u8 = 0x19;
const TALK_DIALOG: u16 = dialogs::TALK;
/// A sick visit booked in the week planner; what the week calls each.
const VISIT: u8 = 0x0b;
const TALK_TEXT: u16 = 4230;
const VISIT_TEXT: u16 = 2221;
/// The visit tells how he took it with the preparation at "normal", or
/// from this day of the season on.
const NORMAL_PREPARATION: u8 = 5;
const TOLD_FROM: usize = 42;

/// A screen to open, with what it is to show.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Request {
    pub screen: u16,
    pub args: Args,
}

impl Request {
    pub fn new(screen: u16) -> Self {
        Self { screen, args: Args::None }
    }

    pub fn with(screen: u16, args: Args) -> Self {
        Self { screen, args }
    }
}

/// What a screen is asked to show.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Args {
    None,
    /// The text of a question or notice, by its id.
    Text(u16),
    /// A job offer, by its tab.
    Offer(usize),
    /// A player of the current club, by his place in its squad.
    Player(usize),
    /// A player of the game, by his record's id.
    Record(u16),
    /// A text by its id, with a player's name for its `%s`.
    TextFor(u16, u16),
    /// How a player took a sick visit (`medical::Reaction`).
    Visit {
        player: u16,
        kind: u8,
        draw: u8,
    },
    /// An appointment of a kind to book with a player, by his record's id.
    Book(u8, u16),
    /// A club, by its country's slot and its place there.
    Club(usize, usize),
    /// A text by its id, with the name of one of the current manager's
    /// formations for its `%s`.
    TextForFormation(u16, usize),
    /// The list view's question whether one of the current manager's
    /// formations is to keep the eleven's spots, in its own words.
    KeepFormation(usize),
    /// What changed of a player's contentment with the trainer, told with a
    /// text with his name.
    Change {
        text: u16,
        player: u16,
        before: i16,
        after: i16,
    },
}

/// How a screen was left.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reply {
    /// OK, yes, or done.
    Ok,
    /// Cancel or no.
    Cancel,
    /// An entry chosen: the start menu's, or the job offer to talk about.
    Choice(usize),
    /// On to another of the main screens, by its dialog id.
    Goto(u16),
    /// The game is to end: the program closes.
    Quit,
}

/// The screens' end of the line to the cycle: its requests, and where the
/// replies go. Dropping it ends the cycle at its next request.
pub struct Link {
    pub requests: Receiver<Request>,
    pub replies: Sender<Reply>,
}

/// Starts the game cycle on its thread. `seed` gives each new game's random
/// numbers their start; the app takes them from the clock, as the game
/// seeds its generator from the time.
pub fn spawn(
    data: Arc<Data>,
    state: Arc<Mutex<GameState>>,
    seed: Box<dyn FnMut() -> u64 + Send>,
) -> Link {
    let (requests, from_cycle) = channel();
    let (to_cycle, replies) = channel();
    let mut cycle = Cycle { data, state, seed, requests, replies };
    thread::Builder::new()
        .name("game cycle".into())
        .spawn(move || cycle.run())
        .expect("a thread for the game cycle");
    Link { requests: from_cycle, replies: to_cycle }
}

/// The game is over: quit, or nobody is there to answer the cycle's
/// requests any more.
struct Gone;

struct Cycle {
    data: Arc<Data>,
    state: Arc<Mutex<GameState>>,
    seed: Box<dyn FnMut() -> u64 + Send>,
    requests: Sender<Request>,
    replies: Receiver<Reply>,
}

impl Cycle {
    fn run(&mut self) {
        // Ends when the game is quit or nobody answers any more.
        let _ = self.start_menu();
    }

    /// Asks for a screen and waits for its reply.
    fn ask(&self, request: Request) -> Result<Reply, Gone> {
        self.requests.send(request).map_err(|_| Gone)?;
        self.replies.recv().map_err(|_| Gone)
    }

    /// Asks for a screen that only needs OK; whether it got it.
    fn ok(&self, screen: u16) -> Result<bool, Gone> {
        Ok(self.ask(Request::new(screen))? == Reply::Ok)
    }

    fn state(&self) -> MutexGuard<'_, GameState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Works on the season, if there is one.
    fn season<T>(&self, work: impl FnOnce(&mut Season) -> T) -> Option<T> {
        self.state().season.as_mut().map(work)
    }

    /// The start menu, again and again, until the game is quit.
    fn start_menu(&mut self) -> Result<(), Gone> {
        loop {
            let Reply::Choice(entry) = self.ask(Request::new(START))? else { continue };
            let mode = match entry {
                NEW_GAME => Mode::Single,
                LOAD_GAME => Mode::Load,
                HOST_NEW => Mode::ServerNew,
                HOST_SAVED => Mode::ServerLoad,
                JOIN => Mode::Client,
                SCORES => {
                    self.ask(Request::new(HIGH_SCORES))?;
                    continue;
                }
                _ => return Ok(()),
            };
            self.state().mode = mode;
            match mode {
                Mode::Single => {
                    self.state().setup = Setup::default();
                    self.new_game()?;
                }
                Mode::Load | Mode::ServerLoad => self.load()?,
                Mode::ServerNew | Mode::Client => self.network()?,
            }
        }
    }

    /// A new game, through the game's own sequence of screens: countries,
    /// number of players, a name per player, then for every player a club
    /// and national team, abilities and birthday, and the start settings.
    /// Cancel on any of them goes back to the start menu.
    fn new_game(&mut self) -> Result<(), Gone> {
        if !self.ok(COUNTRIES)? || !self.ok(PLAYERS)? {
            return Ok(());
        }
        let players = self.state().setup.trainers.len();
        for player in 0..players {
            self.state().setup.current = player;
            if !self.ok(NAMES)? {
                return Ok(());
            }
        }
        if self.state().setup.chooses_club() {
            for player in 0..players {
                self.state().setup.current = player;
                if !self.ok(CLUB)? {
                    return Ok(());
                }
                // Without a club, no national team either.
                let club = self.state().setup.trainer().is_some_and(|t| t.club.is_some());
                if club && !self.ok(NATIONAL_TEAM)? {
                    return Ok(());
                }
            }
        }
        for player in 0..players {
            self.state().setup.current = player;
            if !self.ok(ABILITIES)? || !self.ok(BIRTHDAY)? {
                return Ok(());
            }
        }
        if !self.ok(SETTINGS)? {
            return Ok(());
        }
        let seed = (self.seed)();
        {
            let mut state = self.state();
            newgame::log(&self.data.world, &state.setup);
            state.season = Some(newgame::calculate(&self.data, &state.setup, seed));
        }
        self.job_offers()?;
        self.weeks(true)
    }

    /// Every manager without a club looks at the offers and talks, until a
    /// contract is signed or every offer is gone.
    fn job_offers(&mut self) -> Result<(), Gone> {
        while self.next_with_offers() {
            let mut tab = 0;
            loop {
                let Reply::Choice(talk) = self.ask(Request::with(OFFERS, Args::Offer(tab)))? else {
                    // Going without a club.
                    self.decline();
                    break;
                };
                tab = talk;
                if self.ask(Request::with(TALKS, Args::Offer(tab)))? == Reply::Ok {
                    self.sign(tab);
                    self.ask(Request::new(SIGNED))?;
                    break;
                }
                if self.offer(tab, |o| o.open) {
                    continue;
                }
                // The club broke the talks off: on to the first offer still
                // open, if there is one.
                self.ask(Request::new(FAILED))?;
                match self.first_open_offer() {
                    Some(first) => tab = first,
                    None => {
                        self.ask(Request::with(NOTICE, Args::Text(NO_ONE_LEFT)))?;
                        self.decline();
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    /// Makes the next manager with offers the current one; whether there is
    /// one.
    fn next_with_offers(&self) -> bool {
        self.season(|season| {
            let manager = season.managers.iter().position(|m| !m.offers.is_empty());
            if let Some(manager) = manager {
                season.current = manager;
            }
            manager.is_some()
        })
        .unwrap_or(false)
    }

    /// Something about the current manager's offer on `tab`.
    fn offer(&self, tab: usize, look: impl FnOnce(&crate::jobs::Offer) -> bool) -> bool {
        self.season(|s| s.managers[s.current].offers.get(tab).is_some_and(look)).unwrap_or(false)
    }

    fn first_open_offer(&self) -> Option<usize> {
        self.season(|s| s.managers[s.current].offers.iter().position(|o| o.open)).flatten()
    }

    /// The current manager goes without a club, for now.
    fn decline(&self) {
        self.season(|s| {
            let current = s.current;
            s.managers[current].offers.clear();
        });
    }

    /// The current manager signs the offer on `tab` as it stands.
    fn sign(&self, tab: usize) {
        let data = &self.data;
        self.season(|s| {
            let current = s.current;
            if let Some(club) = s.managers[current].offers.get(tab).map(|o| o.club) {
                s.take_club(&data.world, &data.rules, current, club);
            }
        });
    }

    /// The season week by week: the autosave's wait screen, every human
    /// manager's turn, and the week running day by day — until the week of
    /// the first match, which is not played yet. A loaded game starts with
    /// the turns, as the original's does (its cycle's case 2).
    fn weeks(&mut self, mut wait: bool) -> Result<(), Gone> {
        loop {
            if wait {
                self.ask(Request::new(WAIT))?;
            }
            wait = true;
            let managers = self.season(|s| s.managers.len()).unwrap_or(0);
            for manager in 0..managers {
                self.season(|s| s.current = manager);
                self.turn()?;
            }
            let Some(week) = self.season(|s| self.start_week(s)).flatten() else {
                return Ok(());
            };
            self.ask(Request::new(week))?;
            for _ in 0..WEEK_DAYS {
                self.appointments()?;
                self.season(|s| {
                    s.pass_day();
                    s.day += 1;
                });
                self.ask(Request::new(week))?;
            }
        }
    }

    /// What the managers booked for today in their planners with a player
    /// still in their squads, manager by manager (`FUN_0072fd53`): a talk
    /// opens in its manager's turn (`FUN_0048ddbe`); a sick visit happens
    /// (`FUN_00557e1f`) and tells how he took it — unless the game runs its
    /// preparation fast and the season is younger than day 42. Each
    /// manager's step clears the week's message, so the last one's stays.
    fn appointments(&self) -> Result<(), Gone> {
        let managers = self.season(|s| s.managers.len()).unwrap_or(0);
        for n in 0..managers {
            let booked = {
                let mut state = self.state();
                let state = &mut *state;
                let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_mut())
                else {
                    return Ok(());
                };
                season.current = n;
                season.planner_message = None;
                let manager = &season.managers[n];
                let last_name = manager.last_name.clone();
                let club = manager.club;
                let booked = records
                    .human_slot(n)
                    .and_then(|slot| records.planners.get(slot))
                    .map(|p| p.day(season.day))
                    .filter(|b| b.player != 0);
                let status = booked.and_then(|b| {
                    let club = season.clubs.get(&club?)?;
                    club.status.iter().position(|s| s.id == b.player).map(|p| (b, p))
                });
                match status {
                    Some((b, _)) if b.kind == TALK => {
                        season.planner_message = Some((TALK_TEXT, last_name));
                        Some((TALK_DIALOG, Args::Record(b.player)))
                    }
                    Some((b, p)) if b.kind == VISIT => {
                        let Some(key) = club else { continue };
                        let days_out = season.clubs[&key].status[p].injured;
                        let Some(r) = records.players.get_mut(usize::from(b.player)) else {
                            continue;
                        };
                        let reaction = medical::visit(r, &mut season.random, days_out);
                        let morale = i32::from(r.morale());
                        if let Some(fitness) =
                            season.clubs.get_mut(&key).and_then(|c| c.fitness.get_mut(p))
                        {
                            fitness.morale = morale;
                        }
                        season.planner_message = Some((VISIT_TEXT, last_name));
                        let told = records.globals.preparation() == NORMAL_PREPARATION
                            || season.day >= TOLD_FROM;
                        told.then_some((
                            dialogs::VISIT,
                            Args::Visit {
                                player: b.player,
                                kind: reaction.kind,
                                draw: reaction.draw,
                            },
                        ))
                    }
                    _ => None,
                }
            };
            if let Some((dialog, args)) = booked {
                self.ask(Request::with(dialog, args))?;
            }
        }
        self.season(|s| s.current = 0);
        Ok(())
    }

    /// The week after the turns: its screen, or none in the week of the
    /// first match, when the season stops.
    fn start_week(&self, season: &mut Season) -> Option<u16> {
        season.current = 0;
        let first_match = season
            .managers
            .iter()
            .filter_map(|m| season.next_match(&self.data.world, &self.data.calendar, m.club?))
            .min_by_key(|m| m.day);
        if let Some(m) = first_match.filter(|m| m.day < season.day + WEEK_DAYS) {
            let date = crate::calendar::date(season.year, m.day);
            info!(
                "{}, {}. Spieltag am {}.{}.{}: here the match would be played; back to the \
                 start menu",
                m.league,
                m.matchday,
                date.day(),
                date.month(),
                date.year()
            );
            return None;
        }
        season.start_week();
        let clubs = season.managers.iter().filter(|m| m.club.is_some()).count();
        Some(WEEKS[clubs.min(WEEKS.len() - 1)])
    }

    /// The current manager's turn, on the main screens with the menu bar or
    /// in the office, until it is ended.
    fn turn(&self) -> Result<(), Gone> {
        let Some(mut screen) = self.season(|s| self.turn_screen(s)) else { return Ok(()) };
        loop {
            match self.ask(Request::new(screen))? {
                Reply::Goto(next) => {
                    screen = self.season(|s| line_up_view(s, next)).unwrap_or(next);
                }
                // "Spiel beenden": the cycle ends, and with it the program.
                Reply::Quit => return Err(Gone),
                _ => return Ok(()),
            }
        }
    }

    /// Where a manager's turn opens: the office for one who chose it — the
    /// grand one for the grandest clubs — else the line-up, or the dressing
    /// room for a manager without a club.
    fn turn_screen(&self, season: &Season) -> u16 {
        let Some(manager) = season.manager() else { return DRESSING_ROOM };
        if manager.menu == OFFICE_MENU {
            let club =
                manager.club.and_then(|(slot, c)| self.data.world.country(slot)?.clubs.get(c));
            return if club.is_some_and(|c| c.class >= GRAND_CLASS) {
                GRAND_OFFICE
            } else {
                OFFICE
            };
        }
        if manager.club.is_some() { line_up_view(season, LINEUP) } else { DRESSING_ROOM }
    }

    /// A saved game: read, and on with its weeks; for a network game, where
    /// it would be hosted.
    fn load(&mut self) -> Result<(), Gone> {
        if !self.ok(LOAD)? {
            return Ok(());
        }
        if self.state().mode == Mode::ServerLoad {
            return self.network_games();
        }
        let name = self.state().save.clone().unwrap_or_default();
        let path = self.data.dir.join("saves").join(format!("{name}.an3"));
        let records = match Save::read(&path).and_then(Records::from_save) {
            Ok(records) => records,
            Err(err) => {
                warn!("{}: {err}", path.display());
                return Ok(());
            }
        };
        let season = load::season(&records, &self.data, Random::new((self.seed)()));
        info!("Spielstand \"{name}\" geladen: Tag {}, {}", season.day, season.date());
        {
            let mut state = self.state();
            state.season = Some(season);
            state.records = Some(records);
        }
        self.weeks(false)
    }

    /// A network game: the player's name, the details for hosting one, and
    /// the list of games.
    fn network(&mut self) -> Result<(), Gone> {
        if !self.ok(NET_NAME)? {
            return Ok(());
        }
        if self.state().mode != Mode::Client && !self.ok(NET_DETAILS)? {
            return Ok(());
        }
        self.network_games()
    }

    fn network_games(&mut self) -> Result<(), Gone> {
        if self.ok(NET_GAMES)? {
            would_start("Netzwerkspiel");
        }
        Ok(())
    }
}

/// The line-up in the view the current manager has it in: on the pitch or
/// as a list (`FUN_006a5ba7`, every turn's hub); any other screen as it is.
fn line_up_view(season: &Season, screen: u16) -> u16 {
    let manager = season.manager();
    match screen {
        LINEUP if manager.is_some_and(|m| m.list_view) => LINEUP_LIST,
        SHORT_TRAINING if manager.is_some_and(|m| m.training_pictures) => TRAINING_PICTURES,
        _ => screen,
    }
}

/// Where the game itself would begin, which is not built yet; back to the
/// start menu.
fn would_start(what: &str) {
    info!("{what}: here the game itself would begin; back to the start menu");
}
