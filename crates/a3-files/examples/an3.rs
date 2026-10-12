//! Lists the blocks of an ANSTOSS 3 savegame, or compares two:
//!
//!     cargo run -p a3-files --example an3 -- SAVE
//!     cargo run -p a3-files --example an3 -- SAVE OTHER

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use a3_files::an3::{self, Form, Save};

fn main() -> ExitCode {
    let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let saves: Result<Vec<Save>, _> = paths.iter().map(|path| Save::read(path)).collect();
    let saves = match (paths.len(), saves) {
        (1 | 2, Ok(saves)) => saves,
        (1 | 2, Err(err)) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
        _ => {
            eprintln!("usage: an3 SAVE [OTHER]");
            return ExitCode::from(2);
        }
    };
    // Stops quietly when the reader goes away (`| head`).
    let _ = report(&saves);
    ExitCode::SUCCESS
}

fn report(saves: &[Save]) -> io::Result<()> {
    let mut out = io::stdout().lock();
    if let [save] = saves {
        let h = &save.header;
        writeln!(
            out,
            "version {}, {}.{}.{}, {} blocks",
            h.version,
            h.date.0,
            h.date.1,
            h.date.2,
            save.blocks.len()
        )?;
        for block in &save.blocks {
            let form = match block.form {
                Form::Empty => "empty".to_owned(),
                Form::Stored => "stored".to_owned(),
                Form::Packed(len) => format!("packed {len}"),
                Form::Raw => "raw".to_owned(),
            };
            writeln!(
                out,
                "{:#09x} {:>8} {:<14} {}",
                block.offset,
                block.data.len(),
                form,
                block.name
            )?;
        }
        return Ok(());
    }
    let differences = an3::compare(&saves[0], &saves[1]);
    if differences.header {
        writeln!(out, "the headers differ: {:?} against {:?}", saves[0].header, saves[1].header)?;
    }
    for difference in &differences.blocks {
        writeln!(out, "{difference}")?;
    }
    writeln!(out, "{} blocks differ", differences.blocks.len())
}
