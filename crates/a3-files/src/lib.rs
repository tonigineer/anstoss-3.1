//! Reading an installed ANSTOSS 3: its archives, data files, dialog
//! definitions, executable tables, video, sound and bitmap font. No game
//! rules — what the files mean for the game is decided in `a3-core`.

pub mod adpcm;
pub mod bitmap_font;
pub mod cpr;
pub mod exe;
pub mod rch;
pub mod sav;
pub mod sks;
