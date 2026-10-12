//! The open screens: the screen the game cycle asked for, with any popups
//! it opened on top. The host opens each request of the cycle and hands the
//! cycle each reply; whoever shows the screens — the app, or a script
//! without a window — gives it the player's clicks, keys and the time
//! passing.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, RecvError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tracing::warn;

use a3_core::cycle;
use a3_core::state::GameState;

use crate::game::Game;
use crate::pointer::Target;
use crate::screens::{Cx, Outcome, Reply, Request, Screen, registry};
use crate::view::View;

/// What a request, an input or a tick changed for whoever shows the screens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Change {
    /// The same screen is on top; its view may have changed.
    Same,
    /// Another screen is on top.
    Screen,
    /// The game is over.
    Quit,
}

pub struct Host {
    /// Every screen once, and which runs each dialog.
    screens: Vec<Box<dyn Screen>>,
    by_dialog: HashMap<u16, usize>,
    /// The open screens with their views, popups on top.
    open: Vec<(usize, View)>,
    /// Whether the screen the cycle asked for has replied, and stays until
    /// the next request.
    answered: bool,
    state: Arc<Mutex<GameState>>,
    /// The line to the game cycle; its requests come in behind a lock only
    /// so that the host can be shared between threads.
    requests: Mutex<Receiver<Request>>,
    replies: Sender<Reply>,
}

impl Host {
    /// Starts the game cycle on the installed game; its first request opens
    /// the first screen. `seed` gives each new game's random numbers.
    pub fn start(game: &Game, seed: Box<dyn FnMut() -> u64 + Send>) -> Self {
        let screens = registry();
        let by_dialog = screens
            .iter()
            .enumerate()
            .flat_map(|(i, screen)| screen.ids().iter().map(move |&id| (id, i)))
            .collect();
        let state = Arc::new(Mutex::new(GameState::default()));
        let cycle = cycle::spawn(game.data.clone(), state.clone(), seed);
        let (requests, replies) = (Mutex::new(cycle.requests), cycle.replies);
        Self { screens, by_dialog, open: Vec::new(), answered: false, state, requests, replies }
    }

    /// Every dialog a screen runs.
    pub fn dialogs() -> Vec<u16> {
        registry().iter().flat_map(|screen| screen.ids().to_vec()).collect()
    }

    /// The state of the game, which the cycle shares.
    pub fn state(&self) -> MutexGuard<'_, GameState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether a screen is open, from the cycle's first request on.
    pub fn is_open(&self) -> bool {
        !self.open.is_empty()
    }

    /// The view of the screen on top.
    pub fn view(&self) -> &View {
        &self.open.last().expect("a screen is open").1
    }

    pub fn view_mut(&mut self) -> &mut View {
        &mut self.open.last_mut().expect("a screen is open").1
    }

    /// Whether the screen on top waits for the player, rather than for the
    /// cycle's next request.
    pub fn waiting(&self) -> bool {
        self.is_open() && !self.answered
    }

    /// Opens the cycle's next request, if it has come.
    pub fn poll(&mut self, game: &Game) -> Change {
        let requests = self.requests.get_mut().unwrap_or_else(PoisonError::into_inner);
        match requests.try_recv() {
            Ok(request) => self.take(game, request),
            Err(TryRecvError::Empty) => Change::Same,
            Err(TryRecvError::Disconnected) => Change::Quit,
        }
    }

    /// Waits for the cycle's next request and opens it.
    pub fn next(&mut self, game: &Game) -> Change {
        let requests = self.requests.get_mut().unwrap_or_else(PoisonError::into_inner);
        match requests.recv() {
            Ok(request) => self.take(game, request),
            Err(RecvError) => Change::Quit,
        }
    }

    /// A click or key on a target of the screen on top: first the controls
    /// take it in, then the screen.
    pub fn input(&mut self, game: &Game, target: Target, double: bool) -> Change {
        if !self.waiting() {
            return Change::Same;
        }
        let Some(dialog) = game.dialogs.get(&self.view().id) else { return Change::Same };
        if !self.view_mut().press(dialog, target) {
            return Change::Same;
        }
        let outcome =
            self.with_top(game, |screen, cx, view| screen.input(cx, view, target, double));
        self.apply(game, outcome)
    }

    /// Time passing for the screen on top.
    pub fn tick(&mut self, game: &Game, seconds: f32) -> Change {
        if !self.waiting() {
            return Change::Same;
        }
        let outcome = self.with_top(game, |screen, cx, view| screen.tick(cx, view, seconds));
        self.apply(game, outcome)
    }

    /// A request of the cycle: the screen it asks for in place of all that
    /// are open — or, asked for again, the open one shows it if it can.
    fn take(&mut self, game: &Game, request: Request) -> Change {
        if let [(screen, view)] = &mut self.open[..]
            && view.id == request.screen
            && let Some(dialog) = game.dialogs.get(&view.id)
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let mut cx = Cx { game, dialog, state: &mut state };
            if self.screens[*screen].again(&mut cx, view, request.args) {
                self.answered = false;
                return Change::Same;
            }
        }
        match self.opened(game, request) {
            Some(opened) => {
                self.open.clear();
                self.open.push(opened);
                self.answered = false;
                Change::Screen
            }
            // The cycle goes on as if the screen had been cancelled.
            None => self.reply(Reply::Cancel),
        }
    }

    fn apply(&mut self, game: &Game, outcome: Outcome) -> Change {
        match outcome {
            Outcome::Stay => Change::Same,
            Outcome::Popup(request) => match self.opened(game, request) {
                Some(opened) => {
                    self.open.push(opened);
                    Change::Screen
                }
                None => Change::Same,
            },
            Outcome::Close(reply) if self.open.len() > 1 => {
                // A popup replies to the screen below it.
                self.open.pop();
                let outcome =
                    self.with_top(game, |screen, cx, view| screen.answer(cx, view, reply));
                match self.apply(game, outcome) {
                    Change::Quit => Change::Quit,
                    _ => Change::Screen,
                }
            }
            Outcome::Close(reply) => self.reply(reply),
        }
    }

    /// Replies to the cycle; the screen stays until its next request.
    fn reply(&mut self, reply: Reply) -> Change {
        self.answered = true;
        match self.replies.send(reply) {
            Ok(()) => Change::Same,
            Err(_) => Change::Quit,
        }
    }

    /// The screen a request asks for, opened.
    fn opened(&mut self, game: &Game, request: Request) -> Option<(usize, View)> {
        let Some(dialog) = game.dialogs.get(&request.screen) else {
            warn!("dialog {} is not in the game's dialog file", request.screen);
            return None;
        };
        let Some(&screen) = self.by_dialog.get(&request.screen) else {
            warn!("no screen runs dialog {}", request.screen);
            return None;
        };
        let mut view = View::new(request.screen, dialog);
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut cx = Cx { game, dialog, state: &mut state };
        self.screens[screen].open(&mut cx, &mut view, request.args);
        Some((screen, view))
    }

    /// Calls the screen on top with its view.
    fn with_top(
        &mut self,
        game: &Game,
        call: impl FnOnce(&mut dyn Screen, &mut Cx, &mut View) -> Outcome,
    ) -> Outcome {
        let (screen, view) = self.open.last_mut().expect("a screen is open");
        let Some(dialog) = game.dialogs.get(&view.id) else { return Outcome::Stay };
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut cx = Cx { game, dialog, state: &mut state };
        call(self.screens[*screen].as_mut(), &mut cx, view)
    }
}
