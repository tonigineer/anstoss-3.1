//! The game's data as every new game starts from it: the football world of
//! the data files, and the calendar and rules the executable keeps in
//! tables of its own.

use std::io;
use std::path::{Path, PathBuf};

use a3_files::exe::Exe;

use crate::calendar::Calendar;
use crate::season::Rules;
use crate::world::World;

pub struct Data {
    /// The installation, with its saves in `saves/`.
    pub dir: PathBuf,
    pub world: World,
    pub calendar: Calendar,
    /// The rules the game keeps in tables of its own.
    pub rules: Rules,
}

impl Data {
    /// Reads the data of the installation in `dir`.
    pub fn load(dir: &Path) -> io::Result<Self> {
        let world = World::load(&dir.join("Data.a3"))?;
        let exe = Exe::open(&dir.join("anstoss3.exe"))?;
        Ok(Self {
            dir: dir.to_owned(),
            calendar: Calendar::load(&exe, &world)?,
            rules: Rules::load(&exe)?,
            world,
        })
    }
}
