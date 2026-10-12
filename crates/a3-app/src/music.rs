//! The game's music: sixteen tracks in `Data/music.cpr`, packed WAV files of
//! IMA ADPCM. They play one after another as the game's music thread plays
//! them: it looks every second whether a track is playing, and seven looks
//! after one has ended starts the next, chosen by the mode in `musik.cfg`:
//!
//! - 0: no music;
//! - 1: any track at random, the versions without singing preferred;
//! - 2: by the mood of the game — away from matches, the seven tracks of its
//!   standard list, shuffled;
//! - 3: the tracks ticked on the stereo, in order.
//!
//! The music starts once the intro is over.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::audio::Volume;
use bevy::prelude::*;
use bevy::tasks::futures::check_ready;
use bevy::tasks::{AsyncComputeTaskPool, Task};

use a3_files::adpcm::{decode, wav};
use a3_files::cpr::{Archive, invalid, u32_at};

use crate::{GameDir, Screen};

const TRACKS: [&str; 16] = [
    "1987", "a2back", "bluesmit", "bluesohn", "ether", "fire", "floyp1", "floyp2", "katzenja",
    "poprocks", "samba", "song2", "theabbys", "unplugge", "west_mit", "westohne",
];
/// The tracks with singing, and the same without.
const SUNG: [(usize, usize); 2] = [(2, 3), (14, 15)];
/// What the game plays by mood when no match or event sets one.
const STANDARD: [usize; 7] = [1, 4, 6, 7, 11, 13, 15];
/// Of the standard list's last track, the version with singing plays one
/// time in this many.
const SUNG_ODDS: u64 = 5;
const POLL_SECS: f32 = 1.0;
const IDLE_POLLS: u32 = 7;
const VOLUME: f32 = 0.6;
/// IMA ADPCM's format tag in a WAV file.
const IMA_ADPCM: u16 = 0x11;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (poll, start).chain().run_if(not(in_state(Screen::Intro))));
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Off,
    Random,
    Mood,
    /// The ticked tracks, one bit each.
    Playlist(u32),
}

#[derive(Resource)]
struct Jukebox {
    archive: PathBuf,
    mode: Mode,
    current: Option<usize>,
    /// Looks since nothing played, and the time to the next look.
    idle: u32,
    timer: f32,
    /// The shuffled standard list, and how far it has played.
    shuffled: Vec<usize>,
    next: usize,
    random: u64,
    loading: Option<Task<io::Result<Vec<u8>>>>,
}

/// The playing track; it despawns when the track ends.
#[derive(Component)]
struct Playing;

fn setup(mut commands: Commands, dir: Res<GameDir>) {
    let mode = settings(&dir.0.join("musik.cfg"));
    info!("music: {mode:?}");
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |t| t.as_nanos() as u64);
    commands.insert_resource(Jukebox {
        archive: dir.0.join("Data/music.cpr"),
        mode,
        current: None,
        // The game's counter starts one short, so the first track follows at once.
        idle: IDLE_POLLS - 1,
        timer: 0.0,
        shuffled: Vec::new(),
        next: 0,
        random: seed | 1,
        loading: None,
    });
}

/// `musik.cfg`: the mode (16 bits), a switch (16 bits) and the ticked tracks
/// (32 bits). Without the file the music follows the mood, as most players
/// have it.
fn settings(path: &Path) -> Mode {
    let Ok(cfg) = std::fs::read(path) else { return Mode::Mood };
    let mode = cfg.get(..2).map_or(2, |b| u16::from_le_bytes([b[0], b[1]]));
    match mode {
        0 => Mode::Off,
        1 => Mode::Random,
        3 => Mode::Playlist(u32_at(&cfg, 4).unwrap_or(u32::MAX)),
        _ => Mode::Mood,
    }
}

fn poll(time: Res<Time>, mut jukebox: ResMut<Jukebox>, playing: Query<(), With<Playing>>) {
    if matches!(jukebox.mode, Mode::Off) || jukebox.loading.is_some() {
        return;
    }
    jukebox.timer += time.delta_secs();
    if jukebox.timer < POLL_SECS {
        return;
    }
    jukebox.timer -= POLL_SECS;
    if !playing.is_empty() {
        jukebox.idle = 0;
        return;
    }
    jukebox.idle += 1;
    if jukebox.idle < IDLE_POLLS {
        return;
    }
    jukebox.idle = 0;
    let Some(track) = jukebox.pick() else { return };
    jukebox.current = Some(track);
    let archive = jukebox.archive.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move { load(&archive, TRACKS[track]) });
    jukebox.loading = Some(task);
}

fn start(
    mut commands: Commands,
    mut jukebox: ResMut<Jukebox>,
    mut sounds: ResMut<Assets<AudioSource>>,
) {
    let Some(task) = jukebox.loading.as_mut() else { return };
    let Some(loaded) = check_ready(task) else { return };
    jukebox.loading = None;
    let name = jukebox.current.map_or("", |t| TRACKS[t]);
    match loaded {
        Ok(wav) => {
            info!("music: {name}");
            commands.spawn((
                Playing,
                AudioPlayer::new(sounds.add(AudioSource { bytes: wav.into() })),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(VOLUME)),
            ));
        }
        Err(err) => {
            warn!("music: {name}: {err}; no music");
            jukebox.mode = Mode::Off;
        }
    }
}

impl Jukebox {
    fn pick(&mut self) -> Option<usize> {
        let current = self.current;
        match self.mode {
            Mode::Off => None,
            Mode::Random => loop {
                let mut track = self.below(TRACKS.len() as u64) as usize;
                if let Some(&(_, plain)) = SUNG.iter().find(|&&(sung, _)| sung == track) {
                    track = plain;
                }
                if Some(track) != current {
                    return Some(track);
                }
            },
            Mode::Mood => {
                if self.next >= self.shuffled.len() {
                    self.shuffled = STANDARD.to_vec();
                    for i in (1..self.shuffled.len()).rev() {
                        let j = self.below(i as u64 + 1) as usize;
                        self.shuffled.swap(i, j);
                    }
                    self.next = 0;
                }
                let track = self.shuffled[self.next];
                self.next += 1;
                let (sung, plain) = SUNG[1];
                Some(if track == plain && self.below(SUNG_ODDS) == 0 { sung } else { track })
            }
            Mode::Playlist(ticked) => {
                let ticked = |t: usize| ticked & (1 << t) != 0;
                let after = current.map_or(0, |c| c + 1);
                (after..TRACKS.len()).chain(0..TRACKS.len()).find(|&t| ticked(t))
            }
        }
    }

    /// A random number below `n` (xorshift).
    fn below(&mut self, n: u64) -> u64 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        self.random % n
    }
}

/// A track from the archive, decoded to a PCM WAV file in memory.
fn load(archive: &Path, name: &str) -> io::Result<Vec<u8>> {
    let file = Archive::open(archive)?.read(&format!("{name}.wav"))?;
    let (format, channels, rate, block) = fmt(&file)?;
    if format != IMA_ADPCM || channels != 2 || block == 0 {
        return Err(invalid("not stereo IMA ADPCM"));
    }
    Ok(wav(&decode(chunk(&file, b"data")?, block), rate))
}

/// Format tag, channels, sample rate and block size of a WAV file.
fn fmt(file: &[u8]) -> io::Result<(u16, u16, u32, usize)> {
    let fmt = chunk(file, b"fmt ")?;
    let u16_at = |pos: usize| fmt.get(pos..pos + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let (Some(format), Some(channels), Some(block)) = (u16_at(0), u16_at(2), u16_at(12)) else {
        return Err(invalid("short fmt chunk"));
    };
    Ok((format, channels, u32_at(fmt, 4)?, usize::from(block)))
}

/// The contents of a WAV file's chunk.
fn chunk<'a>(file: &'a [u8], id: &[u8; 4]) -> io::Result<&'a [u8]> {
    if file.get(..4) != Some(b"RIFF") || file.get(8..12) != Some(b"WAVE") {
        return Err(invalid("not a WAV file"));
    }
    let mut pos = 12;
    while pos + 8 <= file.len() {
        let len = u32_at(file, pos + 4)? as usize;
        let body = file.get(pos + 8..pos + 8 + len).ok_or_else(|| invalid("truncated chunk"))?;
        if &file[pos..pos + 4] == id {
            return Ok(body);
        }
        pos += 8 + len + (len & 1);
    }
    Err(invalid("chunk missing"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jukebox(mode: Mode) -> Jukebox {
        Jukebox {
            archive: PathBuf::new(),
            mode,
            current: None,
            idle: 0,
            timer: 0.0,
            shuffled: Vec::new(),
            next: 0,
            random: 0x9e37_79b9_7f4a_7c15,
            loading: None,
        }
    }

    #[test]
    fn the_mood_plays_each_standard_track_once_a_round() {
        let mut jukebox = jukebox(Mode::Mood);
        let mut round: Vec<usize> = (0..STANDARD.len()).filter_map(|_| jukebox.pick()).collect();
        round.iter_mut().for_each(|t| *t = if *t == SUNG[1].0 { SUNG[1].1 } else { *t });
        round.sort_unstable();
        assert_eq!(round, STANDARD);
    }

    #[test]
    fn random_prefers_tracks_without_singing_and_never_repeats() {
        let mut jukebox = jukebox(Mode::Random);
        for _ in 0..200 {
            let track = jukebox.pick().unwrap();
            assert!(SUNG.iter().all(|&(sung, _)| track != sung));
            assert_ne!(Some(track), jukebox.current);
            jukebox.current = Some(track);
        }
    }

    #[test]
    fn the_playlist_goes_round_the_ticked_tracks() {
        let mut jukebox = jukebox(Mode::Playlist(0b1000_0000_0000_0101));
        let order: Vec<usize> = (0..4)
            .map(|_| {
                let track = jukebox.pick().unwrap();
                jukebox.current = Some(track);
                track
            })
            .collect();
        assert_eq!(order, [0, 2, 15, 0]);
    }
}
