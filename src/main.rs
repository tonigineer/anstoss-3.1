//! A replica of ANSTOSS 3: the intro video, the "Spielvorbereitung" start
//! menu and the screens behind it, the calculation of the first season, the
//! job offers, and the preseason weeks up to the first matchday — every turn
//! on the line-up with the game's menu bar — in the classic look and a
//! remastered one, switched with F9 (see [`look`]).
//!
//! Some of the game's rules live in tables of its executable; they are read
//! from the installed `anstoss3.exe` (see [`exe`]).
//!
//! Nothing from the game is bundled. Graphics, video, sound, texts and the
//! football data are read at runtime from an installed copy, taken from the
//! first command-line argument, `$ANSTOSS3_DIR`, or the Bottles prefix — in
//! that order.
//!
//! `--export-backgrounds` writes the artwork of every remastered dialog to the
//! cache directory and exits, for `tools/outpaint.py` to extend to 16:9.

mod adpcm;
mod bitmap_font;
mod calendar;
mod cpr;
mod dialog;
mod exe;
mod fps;
mod game;
mod intro;
mod jobs;
mod lineup;
mod look;
mod music;
mod pace;
mod random;
mod rch;
mod sav;
mod season;
mod sks;
mod world;

use std::path::{Path, PathBuf};

use bevy::prelude::*;

const EXPORT_FLAG: &str = "--export-backgrounds";
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
    /// A dialog of the game, by its id in the dialog file.
    Dialog(u16),
}

#[derive(Resource)]
pub struct GameDir(pub PathBuf);

/// Fonts of the remastered look; the classic look draws with a bitmap font.
#[derive(Resource)]
pub struct Fonts {
    pub regular: Handle<Font>,
    pub bold: Handle<Font>,
}

fn main() -> AppExit {
    let dir = game_dir();
    if !dir.join("anstoss3.exe").is_file() {
        eprintln!("No ANSTOSS 3 installation found at {}", dir.display());
        eprintln!("Pass the game directory as the first argument or set ANSTOSS3_DIR.");
        return AppExit::error();
    }
    let mut game = match game::Game::load(&dir) {
        Ok(game) => game,
        Err(err) => {
            eprintln!("Cannot read the game's dialogs and graphics in {}: {err}", dir.display());
            return AppExit::error();
        }
    };
    dialog::compose(&mut game);
    if std::env::args().any(|arg| arg == EXPORT_FLAG) {
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

fn game_dir() -> PathBuf {
    std::env::args_os()
        .skip(1)
        .find(|arg| arg != EXPORT_FLAG)
        .or_else(|| std::env::var_os("ANSTOSS3_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            Path::new(&home).join(DEFAULT_DIR)
        })
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
