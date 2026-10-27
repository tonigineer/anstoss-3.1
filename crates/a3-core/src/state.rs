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
    /// The stadium the views (249, 510) show.
    pub viewed: Option<crate::stadium::Sub>,
    /// What the stadium screen hands the grounds (466).
    pub grounds_stadiums: Option<GroundsStadiums>,
    /// The grounds as 466 has them, for its sub-dialogs.
    pub grounds_shown: Option<crate::grounds::Grounds>,
    /// What the stereo's OK set last — `musik.cfg`'s mode, sounds and
    /// pieces — until the music takes it over.
    pub music: Option<(u16, u16, u32)>,
    /// The piece the stereo wants heard at once, by its slot.
    pub listen: Option<usize>,
    /// Settings files of the game's folder the settings dialogs changed,
    /// with their new bytes, to be written when the program ends — as the
    /// game writes `musik.cfg` and `Konfig3D.cfg` on its way out
    /// (`FUN_00431e01`).
    pub settings_files: Vec<(&'static str, Vec<u8>)>,
    /// How long and how often the game is played (`zeit.saq`).
    pub play: crate::playtime::PlayTime,
}

impl GameState {
    /// Keeps `bytes` as what `file` is to hold at the end.
    pub fn keep_settings(&mut self, file: &'static str, bytes: Vec<u8>) {
        self.settings_files.retain(|(f, _)| *f != file);
        self.settings_files.push((file, bytes));
    }
}

/// What the stadium screen hands the grounds (`FUN_0057eb54`): its three
/// stadiums' looks, names and capacities, the one played in and a copy of
/// its sub-record.
#[derive(Clone, Debug, Default)]
pub struct GroundsStadiums {
    pub looks: [[u8; 12]; 3],
    pub names: [String; 3],
    pub capacities: [i32; 3],
    pub played: usize,
    pub sub: Option<crate::stadium::Sub>,
}
