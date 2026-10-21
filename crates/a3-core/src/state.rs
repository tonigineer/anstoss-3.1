//! What the game keeps from screen to screen. The game cycle and the open
//! screen share it: while the cycle waits for a screen, the screen may read
//! and change it, as the game's dialogs do.

use crate::newgame::{Mode, Player, Setup};
use crate::records::Records;
use crate::season::Season;

#[derive(Default)]
pub struct GameState {
    /// The game being played, once it is calculated.
    pub season: Option<Season>,
    /// How the start menu set out to begin a game.
    pub mode: Mode,
    /// A new game being set up.
    pub setup: Setup,
    /// The player of a network game.
    pub player: Player,
    /// The saved game picked to load: its name in `saves/`.
    pub save: Option<String>,
    /// The records of the game loaded, to be written back when it is saved.
    pub records: Option<Records>,
    /// The squad in the order the player info is to page through it, by
    /// place, when the screen that opens it has its own: the line-up list's
    /// rows. Taken by the player info.
    pub paging: Option<Vec<usize>>,
    /// The same by the players' ids, for screens of players of any club:
    /// the transfer list's rows.
    pub paging_ids: Option<Vec<u16>>,
    /// The players NORACSA's search found, for the list of its result.
    pub found: Vec<crate::noracsa::Hit>,
    /// The stadium the expansion plans, and whether it is played in; the
    /// plan comes back in its place when it is OKed.
    pub expansion: Option<(crate::stadium::Sub, bool)>,
}
