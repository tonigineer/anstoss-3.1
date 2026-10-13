//! Lists the blocks of an ANSTOSS 3 savegame, compares two, or writes one
//! again with every block stored as it is, none compressed:
//!
//!     cargo run -p a3-files --example an3 -- SAVE
//!     cargo run -p a3-files --example an3 -- SAVE OTHER
//!     cargo run -p a3-files --example an3 -- --stored SAVE OUT

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use a3_files::an3::{self, Form, Save};

fn main() -> ExitCode {
    let mut args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.first().is_some_and(|arg| arg.as_os_str() == "--stored") {
        let [_, save, out] = &args[..] else {
            eprintln!("usage: an3 --stored SAVE OUT");
            return ExitCode::from(2);
        };
        let written = Save::read(save)
            .and_then(|save| save.to_bytes_stored())
            .and_then(|bytes| std::fs::write(out, bytes));
        if let Err(err) = written {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }
    let paths = std::mem::take(&mut args);
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
    let (mut real_time, mut leftovers) = (0, 0);
    for difference in &differences.blocks {
        let name = difference.name.as_str();
        if an3::REAL_TIME.contains(&name) {
            real_time += 1;
            writeln!(out, "{difference} (real time)")?;
        } else if an3::LEFTOVERS.contains(&name) {
            leftovers += 1;
            writeln!(out, "{difference} (left over on the stack)")?;
        } else {
            writeln!(out, "{difference}")?;
        }
    }
    let state = differences.blocks.len() - real_time - leftovers;
    writeln!(
        out,
        "{state} blocks of state differ, besides {real_time} in real time and {leftovers} left over on the stack"
    )
}
