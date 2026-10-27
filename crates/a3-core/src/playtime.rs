//! How long and how often the game is played (`CNutzungAnstoss2`'s
//! figures, the object at `0xae6e2c`): the program's start, the loaded
//! game's start and the play time it was saved with (`0xae6e34`), and from
//! `zeit.saq` the starts, the seasons, the total and the longest session.
//! The game reads the file as it starts, counting itself in
//! (`FUN_00788676`), and writes it back as it ends (`FUN_0078873a`).

use std::path::Path;
use std::time::Instant;

/// The file of the installation that keeps the counts.
pub const FILE: &str = "zeit.saq";

pub struct PlayTime {
    started: Instant,
    loaded: Instant,
    /// The loaded game's play time when it was saved, in seconds.
    stored: u32,
    pub starts: u32,
    pub seasons: u32,
    total: u32,
    longest: u32,
}

impl Default for PlayTime {
    /// Without the file: the first start, the first season.
    fn default() -> Self {
        let now = Instant::now();
        Self { started: now, loaded: now, stored: 0, starts: 1, seasons: 1, total: 0, longest: 0 }
    }
}

/// The four times the dialog shows, in seconds: this session, the game,
/// everything since the installation, the longest session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Figures {
    pub session: u32,
    pub game: u32,
    pub total: u32,
    pub longest: u32,
}

impl PlayTime {
    /// The counts as the file has them, this start counted in.
    pub fn read(path: &Path) -> Self {
        let mut time = Self::default();
        let Ok(file) = std::fs::read(path) else { return time };
        let word = |i: usize| {
            file.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        if let Some(starts) = word(0) {
            time.starts = starts.wrapping_add(1);
        }
        time.seasons = word(1).unwrap_or(time.seasons);
        time.total = word(2).unwrap_or(time.total);
        time.longest = word(3).unwrap_or(time.longest);
        time
    }

    /// A game loaded with the play time it was saved with; a new one
    /// starts at 0.
    pub fn game_loaded(&mut self, stored: u32) {
        self.loaded = Instant::now();
        self.stored = stored;
    }

    fn session(&self, now: Instant) -> u32 {
        now.saturating_duration_since(self.started).as_secs() as u32
    }

    /// The figures at `now` (`FUN_007884cc`); the longest session grows
    /// with this one (`FUN_00788539`).
    pub fn figures(&mut self, now: Instant) -> Figures {
        let session = self.session(now);
        self.longest = self.longest.max(session);
        let game = now.saturating_duration_since(self.loaded).as_secs() as u32 + self.stored;
        Figures { session, game, total: session + self.total, longest: self.longest }
    }

    /// What the file is to hold as the program ends at `now`.
    pub fn file(&self, now: Instant) -> Vec<u8> {
        let session = self.session(now);
        [self.starts, self.seasons, session + self.total, self.longest.max(session)]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect()
    }
}

/// A time as the dialog writes it (`FUN_0078855a`): the days apart —
/// none, `one` or `many` with "%d" — and "hh:mm:ss".
pub fn split(seconds: u32) -> (u32, String) {
    const DAY: u32 = 86_400;
    let rest = seconds % DAY;
    (seconds / DAY, format!("{:02}:{:02}:{:02}", rest / 3600, rest % 3600 / 60, rest % 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_splits_into_days_and_a_clock() {
        assert_eq!(split(86), (0, "00:01:26".to_owned()));
        assert_eq!(split(5838 + 86), (0, "01:38:44".to_owned()));
        assert_eq!(split(2 * 86_400 + 61), (2, "00:01:01".to_owned()));
    }

    #[test]
    fn the_file_counts_this_start_in() {
        let dir = std::env::temp_dir().join(format!("zeit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE);
        let words: Vec<u8> = [60u32, 1, 5838, 229].iter().flat_map(|w| w.to_le_bytes()).collect();
        std::fs::write(&path, &words).unwrap();
        let mut time = PlayTime::read(&path);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!((time.starts, time.seasons), (61, 1));
        let now = time.started;
        let shown = time.figures(now);
        assert_eq!(shown, Figures { session: 0, game: 0, total: 5838, longest: 229 });
        assert_eq!(time.file(now)[..4], 61u32.to_le_bytes());
    }
}
