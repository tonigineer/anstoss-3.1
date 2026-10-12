//! The game's text data files, `Data.a3/*.sav`: Windows-1252 lines ending in
//! CRLF — a checksum, then sections opened by `%SECT%NAME` and closed by
//! `%ENDSECT%NAME`, which nest. Every other line is one value; the meaning of
//! a value is its position within its section.

use std::io;
use std::path::Path;

use crate::bitmap_font::CP1252_HIGH;
use crate::cpr::invalid;

const OPEN: &str = "%SECT%";
const CLOSE: &str = "%ENDSECT%";

#[derive(Debug, Default)]
pub struct Section {
    pub name: String,
    /// The section's own lines, in order, without those of nested sections.
    pub values: Vec<String>,
    pub children: Vec<Section>,
}

impl Section {
    pub fn read(path: &Path) -> io::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::parse(&bytes).map_err(|err| invalid(&format!("{}: {err}", path.display())))
    }

    /// The file as one unnamed section holding the checksum and the sections.
    ///
    /// The files are not always well formed — the editor left a stray
    /// `%SECT%LAND` inside a `PROMI` of `LandPort.sav` — and the game reads
    /// past that, so a close also closes whatever is still open inside the
    /// section it names, a close without its section is skipped, and the end
    /// of the file closes everything.
    pub fn parse(bytes: &[u8]) -> io::Result<Self> {
        let text = cp1252(bytes);
        let mut open = vec![Section::default()];
        for line in text.split('\n') {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if let Some(name) = line.strip_prefix(OPEN) {
                open.push(Section { name: name.to_owned(), ..Section::default() });
            } else if let Some(name) = line.strip_prefix(CLOSE) {
                if let Some(depth) = open.iter().skip(1).rposition(|s| s.name == name) {
                    close_to(&mut open, depth + 1);
                }
            } else {
                open.last_mut().expect("the file itself stays open").values.push(line.to_owned());
            }
        }
        close_to(&mut open, 1);
        let mut file = open.pop().ok_or_else(|| invalid("empty file"))?;
        // The last line break leaves an empty line behind.
        if file.values.last().is_some_and(String::is_empty) {
            file.values.pop();
        }
        Ok(file)
    }

    /// The first nested section called `name`.
    pub fn child(&self, name: &str) -> Option<&Section> {
        self.children.iter().find(|s| s.name == name)
    }

    /// Every nested section called `name`, in order.
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Section> {
        self.children.iter().filter(move |s| s.name == name)
    }

    pub fn text(&self, index: usize) -> Option<&str> {
        self.values.get(index).map(String::as_str)
    }

    pub fn number(&self, index: usize) -> Option<i64> {
        self.text(index)?.trim().parse().ok()
    }
}

/// Closes the open sections from `depth` on, each into the one around it.
fn close_to(open: &mut Vec<Section>, depth: usize) {
    while open.len() > depth.max(1) {
        let section = open.pop().expect("more than one is open");
        open.last_mut().expect("more than one was open").children.push(section);
    }
}

/// Windows-1252 bytes as text.
pub fn cp1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0x80..0xa0 => CP1252_HIGH[usize::from(b - 0x80)],
            _ => char::from(b),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_nest_and_keep_their_own_values() {
        let file = b"123\r\n%SECT%A\r\nx\r\n%SECT%B\r\n1\r\n%ENDSECT%B\r\ny\r\n\r\n%ENDSECT%A\r\n";
        let file = Section::parse(file).unwrap();
        assert_eq!(file.values, ["123"]);
        let a = file.child("A").unwrap();
        assert_eq!(a.values, ["x", "y", ""]);
        assert_eq!(a.child("B").unwrap().number(0), Some(1));
    }

    #[test]
    fn broken_nesting_is_read_past_like_the_game_does() {
        let file = b"%SECT%A\r\n%SECT%B\r\n1\r\n%ENDSECT%A\r\n%ENDSECT%C\r\n%SECT%D\r\n";
        let file = Section::parse(file).unwrap();
        let names: Vec<&str> = file.children.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["A", "D"]);
        assert_eq!(file.child("A").unwrap().child("B").unwrap().number(0), Some(1));
    }

    #[test]
    fn windows_1252_differs_from_latin_1_only_in_0x80_to_0x9f() {
        assert_eq!(cp1252(&[0x80, 0x8a, 0xd6, 0x9e]), "€ŠÖž");
    }
}
