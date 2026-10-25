//! Prints the stored titles of dialogs, and where their statics stand.
//!
//!     ANSTOSS3_DIR=… cargo run -p a3-screens --example titles -- 84 174 95

use std::path::PathBuf;
use std::process::ExitCode;

use a3_screens::game::Game;

fn main() -> ExitCode {
    let Some(dir) = std::env::var_os("ANSTOSS3_DIR") else {
        eprintln!("ANSTOSS3_DIR is not set");
        return ExitCode::from(2);
    };
    let game = match Game::load(&PathBuf::from(dir)) {
        Ok(game) => game,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };
    for arg in std::env::args().skip(1) {
        let Ok(id) = arg.parse::<u16>() else { continue };
        let Some(dialog) = game.dialogs.get(&id) else {
            println!("{id}: not in the dialog file");
            continue;
        };
        println!("{id}: title {:?}, background {}, frame {:?}", dialog.title, dialog.background, dialog.frame);
        for (i, c) in dialog
            .controls
            .iter()
            .enumerate()
            .filter(|(_, c)| c.rect.min.y < 60.0)
        {
            println!(
                "  {i} id {:?} font {:?} at {:?} text {:?}",
                c.id, c.font, c.rect, c.text
            );
        }
    }
    ExitCode::SUCCESS
}
