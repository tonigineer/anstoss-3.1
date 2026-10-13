//! A replica of ANSTOSS 3: the intro video, the "Spielvorbereitung" start
//! menu and the screens behind it, the calculation of the first season, the
//! job offers, and the preseason weeks up to the first matchday — every turn
//! on the line-up with the game's menu bar — in the classic look and a
//! remastered one, switched with F9 (see [`look`]).
//!
//! Some of the game's rules live in tables of its executable; they are read
//! from the installed `anstoss3.exe` (see [`a3_files::exe`]).
//!
//! Nothing from the game is bundled. Graphics, video, sound, texts and the
//! football data are read at runtime from an installed copy, taken from the
//! first command-line argument, `$ANSTOSS3_DIR`, or the Bottles prefix — in
//! that order.
//!
//! `--export-backgrounds` writes the artwork of every remastered dialog to the
//! cache directory and exits, for `tools/outpaint.py` to extend to 16:9.
//! `--script <file>` plays a script without a window, prints what came up
//! and exits (see [`a3_screens::script`]); `--original <file>` plays it on
//! the original instead, through the probe in it on `$A3PROBE_PORT` (see
//! [`a3_screens::original`], `tools/oracle/run.sh`), and with `--explore`
//! answers the dialogs the script does not know with OK. `--screens` prints the
//! dialogs the replica's screens run, one id a line.

mod dialog;
mod fps;
mod intro;
mod look;
mod music;
mod pace;

use std::path::{Path, PathBuf};

use bevy::prelude::*;

const EXPORT_FLAG: &str = "--export-backgrounds";
const SCRIPT_FLAG: &str = "--script";
const ORIGINAL_FLAG: &str = "--original";
const EXPLORE_FLAG: &str = "--explore";
const SHOT_FLAG: &str = "--shot";
const DEFAULT_PROBE_PORT: u16 = 47300;
const SCREENS_FLAG: &str = "--screens";
const DEFAULT_DIR: &str = ".local/share/bottles/bottles/ANSTOSS-3/drive_c/GOG Games/ANSTOSS 3";
/// Fonts for the remastered look, in order of preference.
const REGULAR_FONTS: [&str; 2] = [
    "/usr/share/fonts/OpenSans/OpenSans-Regular.ttf",
    "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
];
const BOLD_FONTS: [&str; 2] = [
    "/usr/share/fonts/OpenSans/OpenSans-Bold.ttf",
    "/usr/share/fonts/liberation/LiberationSans-Bold.ttf",
];

#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Screen {
    #[default]
    Intro,
    /// The game's first screen is on its way.
    Starting,
    /// A dialog of the game, by its id in the dialog file.
    Dialog(u16),
}

#[derive(Resource)]
pub struct GameDir(pub PathBuf);

/// The installed game (see [`a3_screens::game`]) as a resource; Bevy's traits
/// can't be put on a type of another crate.
#[derive(Resource, Deref, DerefMut)]
pub struct Game(pub a3_screens::game::Game);

/// Fonts of the remastered look; the classic look draws with a bitmap font.
#[derive(Resource)]
pub struct Fonts {
    pub regular: Handle<Font>,
    pub bold: Handle<Font>,
}

fn main() -> AppExit {
    let args = Args::read();
    if args.screens {
        a3_screens::host::Host::dialogs().iter().for_each(|id| println!("{id}"));
        return AppExit::Success;
    }
    let dir = args.game_dir();
    if !dir.join("anstoss3.exe").is_file() {
        eprintln!("No ANSTOSS 3 installation found at {}", dir.display());
        eprintln!("Pass the game directory as the first argument or set ANSTOSS3_DIR.");
        return AppExit::error();
    }
    let mut game = match a3_screens::game::Game::load(&dir) {
        Ok(game) => Game(game),
        Err(err) => {
            eprintln!("Cannot read the game's dialogs and graphics in {}: {err}", dir.display());
            return AppExit::error();
        }
    };
    // The dialogs as the screens show them — as the original does, with the
    // menu bar of its main screens, which the dialog file leaves out.
    dialog::compose(&mut game);
    if let Some(script) = &args.original {
        return play(&game, script, Some(args.explore), None);
    }
    if let Some(script) = &args.script {
        return play(&game, script, None, args.shot.as_deref());
    }
    if args.export {
        return match dialog::export_backgrounds(&game) {
            Ok(written) => {
                written.iter().for_each(|path| println!("{}", path.display()));
                AppExit::Success
            }
            Err(err) => {
                eprintln!("Cannot export the backgrounds: {err}");
                AppExit::error()
            }
        };
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "ANSTOSS 3".into(),
                resolution: (1600, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(GameDir(dir))
        .insert_resource(game)
        .init_state::<Screen>()
        .add_systems(Startup, (spawn_camera, load_fonts))
        .add_plugins((look::plugin, intro::plugin, dialog::plugin, fps::plugin, pace::plugin))
        .add_plugins(music::plugin)
        .run()
}

/// The command line: the game directory, if it names one, and what to do
/// instead of playing in a window.
#[derive(Default)]
struct Args {
    dir: Option<PathBuf>,
    export: bool,
    script: Option<PathBuf>,
    original: Option<PathBuf>,
    explore: bool,
    /// Where `--script` saves the screen it ends on.
    shot: Option<PathBuf>,
    screens: bool,
}

impl Args {
    fn read() -> Self {
        let mut args = Self::default();
        let mut words = std::env::args_os().skip(1);
        while let Some(word) = words.next() {
            if word == EXPORT_FLAG {
                args.export = true;
            } else if word == SCREENS_FLAG {
                args.screens = true;
            } else if word == SCRIPT_FLAG {
                args.script = words.next().map(PathBuf::from);
            } else if word == ORIGINAL_FLAG {
                args.original = words.next().map(PathBuf::from);
            } else if word == EXPLORE_FLAG {
                args.explore = true;
            } else if word == SHOT_FLAG {
                args.shot = words.next().map(PathBuf::from);
            } else if args.dir.is_none() {
                args.dir = Some(PathBuf::from(word));
            }
        }
        args
    }

    fn game_dir(&self) -> PathBuf {
        self.dir
            .clone()
            .or_else(|| std::env::var_os("ANSTOSS3_DIR").map(PathBuf::from))
            .unwrap_or_else(|| {
                let home = std::env::var_os("HOME").unwrap_or_default();
                Path::new(&home).join(DEFAULT_DIR)
            })
    }
}

/// Plays a script without a window, on the replica or — with whether to
/// explore — the original, and prints every screen that came up and every
/// action. On the replica, `shot` saves the screen it ends on, drawn in the
/// classic look.
fn play(game: &Game, path: &Path, original: Option<bool>, shot: Option<&Path>) -> AppExit {
    let script = match std::fs::read_to_string(path) {
        Ok(text) => a3_screens::script::Script::parse(&text),
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            return AppExit::error();
        }
    };
    let script = match script {
        Ok(script) => script,
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            return AppExit::error();
        }
    };
    let (log, result) = if let Some(explore) = original {
        let port = std::env::var("A3PROBE_PORT").ok().and_then(|p| p.parse().ok());
        match a3_screens::original::Probe::connect(port.unwrap_or(DEFAULT_PROBE_PORT)) {
            Ok(probe) => {
                let (original, result) =
                    a3_screens::original::Original::play(game, probe, &script, explore);
                (original.log, result)
            }
            Err(err) => (Vec::new(), Err(err)),
        }
    } else {
        let (runner, result) = a3_screens::script::Runner::play(game, &script);
        if let Some(shot) = shot {
            let view = runner.host.view();
            let Some(dialog) = game.dialogs.get(&view.id) else {
                eprintln!("no dialog {} to draw", view.id);
                return AppExit::error();
            };
            let background = a3_screens::classic::picture(game, dialog);
            let classic = a3_screens::classic::Classic::new(game, dialog, view, background);
            if let Err(err) = classic.draw(dialog, view, false, 0.0).save(shot) {
                eprintln!("{}: {err}", shot.display());
                return AppExit::error();
            }
        }
        (runner.log, result)
    };
    log.iter().for_each(|line| println!("{line}"));
    match result {
        Ok(()) => AppExit::Success,
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            AppExit::error()
        }
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn load_fonts(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let mut load =
        |candidates: &[&str]| match candidates.iter().find_map(|path| std::fs::read(path).ok()) {
            Some(bytes) => fonts.add(Font::from_bytes(bytes)),
            None => {
                warn!("none of {candidates:?} found; falling back to Bevy's built-in font");
                Handle::default()
            }
        };
    let regular = load(&REGULAR_FONTS);
    let bold = load(&BOLD_FONTS);
    commands.insert_resource(Fonts { regular, bold });
}
