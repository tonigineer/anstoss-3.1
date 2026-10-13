//! The game's dialog definitions, `deb/MeineDialoge.rch` (magic `V10`), and
//! the list of its graphics, `deb/ListeDerGrafik.rh`.
//!
//! The `.rch` file starts with a table of `(offset, dialog id)` pairs. A dialog
//! record holds its size, titles, name and background graphic, then its
//! controls. Every control record starts with its type twice, two padding
//! bytes and a zero, then the rectangle, the window style, the German text, an
//! unused string, the English text, five more bytes, the control id and its
//! name — followed by data that depends on the type. Padding inside the
//! records is filled with `0xCD`, the debug heap pattern of the compiler that
//! wrote them.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use bevy_math::{Rect, Vec2};

use crate::cpr::{invalid, u32_at};
use crate::sav::cp1252;

#[derive(Clone)]
pub struct Dialog {
    /// Its German title, which a popup shows in its caption; some are
    /// templates the game fills in itself.
    pub title: String,
    pub background: u16,
    pub controls: Vec<Control>,
    /// A dialog smaller than the screen is a popup over the screen it was
    /// opened from, in the middle of it; its controls are placed there.
    pub frame: Option<Rect>,
}

#[derive(Clone)]
pub struct Control {
    pub kind: Kind,
    /// In classic 640×480 pixels.
    pub rect: Rect,
    pub align: Align,
    pub text: String,
    pub id: Option<u16>,
    /// Statics drawn in the emphasised dialog font.
    pub bold: bool,
    /// The colour of a static's text, red, green and blue; black for the
    /// other controls.
    pub ink: [u8; 3],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Left,
    Centre,
    Right,
}

#[derive(Clone)]
pub enum Kind {
    Static,
    /// A bitmap button: graphic ids of its normal and pointed-at images.
    Button {
        normal: u16,
        hover: u16,
    },
    ListBox,
    Bitmap,
    /// The game's `AniStatic`: a picture that can play a series of frames.
    Anim,
    /// The tiny up/down arrows next to a number.
    Spin,
    /// A horizontal scrollbar: arrows at both ends, a handle in between.
    Slider,
    /// Column titles of a list, separated by tabs.
    Header,
    Edit {
        max_len: usize,
    },
    /// The game's `HighButton`: a group of options, one of them chosen. With an
    /// `image`, the chosen option is drawn from that strip of buttons;
    /// without, a radio bullet painted into the background marks it.
    Options {
        image: u16,
        items: Vec<Item>,
    },
    /// A box to tick, with its label beside it.
    Check,
    /// A menu that opens where a right click falls, which the screens put
    /// in; the game's `CPopupMenue`.
    Menu,
    /// A player's box on the pitch or the bench of the line-up, the game's
    /// `CMannStatic`, which draws itself.
    Player,
    /// The menu bar the game puts along the bottom of its main screens, its
    /// `CLeiste`: a strip of symbols, which draws itself.
    Bar {
        items: Vec<Item>,
    },
    /// A strip the screen paints bars into, the game's `CMyPaint`.
    Paint,
    Other,
}

#[derive(Clone)]
pub struct Item {
    /// Relative to the control.
    pub rect: Rect,
    pub text: String,
}

/// Placeholder texts the resource editor left in controls that have none.
const PLACEHOLDERS: [&str; 12] = [
    "Button",
    "Edit",
    "ListBox",
    "Liste",
    "HighButton",
    "Combobox",
    "Scrollbar",
    "Bitmap",
    "Static",
    "AniStatic",
    "Control",
    "SC",
];

pub fn load_dialogs(path: &Path) -> io::Result<HashMap<u16, Dialog>> {
    let data = std::fs::read(path)?;
    if !data.starts_with(b"V10\0") {
        return Err(invalid("not a V10 dialog file"));
    }
    let count = u32_at(&data, 4)? as usize;
    let mut table = Vec::with_capacity(count);
    for i in 0..count {
        let entry = 0x10 + 8 * i;
        table.push((u32_at(&data, entry)? as usize, u32_at(&data, entry + 4)? as u16));
    }
    let mut dialogs = HashMap::new();
    for (i, &(offset, id)) in table.iter().enumerate() {
        let end = table.get(i + 1).map_or(data.len(), |next| next.0);
        let record = data.get(offset..end).ok_or_else(|| invalid("dialog out of bounds"))?;
        if let Some(dialog) = parse_dialog(record) {
            dialogs.insert(id, dialog);
        }
    }
    Ok(dialogs)
}

/// Where a dialog record's German title starts.
const TITLE: usize = 0x1e;

fn parse_dialog(record: &[u8]) -> Option<Dialog> {
    let size = Vec2::new(u32_at(record, 0x10).ok()? as f32, u32_at(record, 0x14).ok()? as f32);
    let title = cstr(record, TITLE).map_or_else(String::new, |(title, _)| title);
    let name = find(record, b"IDD_")?;
    let mut pos = name + record[name..].iter().position(|&b| b == 0)? + 1;
    pos = skip_padding(record, pos);
    let background = u16_at(record, pos)?;

    let starts: Vec<usize> = (pos..record.len().saturating_sub(10))
        .filter(|&p| is_control_start(&record[p..]))
        .collect();
    let mut controls: Vec<Control> = starts
        .iter()
        .enumerate()
        .filter_map(|(i, &start)| {
            let end = starts.get(i + 1).copied().unwrap_or(record.len());
            parse_control(&record[start..end])
        })
        .collect();
    let screen = Vec2::new(640.0, 480.0);
    let frame = (size.x > 0.0 && size.y > 0.0 && size.cmplt(screen).any()).then(|| {
        let at = ((screen - size) / 2.0).floor();
        Rect::from_corners(at, at + size)
    });
    if let Some(frame) = frame {
        for control in &mut controls {
            control.rect =
                Rect::from_corners(control.rect.min + frame.min, control.rect.max + frame.min);
        }
    }
    Some(Dialog { title, background, controls, frame })
}

fn is_control_start(bytes: &[u8]) -> bool {
    let kind = bytes[0];
    (1..32).contains(&kind)
        && bytes[1..4] == [0, kind, 0]
        && bytes[4..6] == [0xcd, 0xcd]
        && bytes[6..10] == [0; 4]
}

fn parse_control(bytes: &[u8]) -> Option<Control> {
    let kind = bytes[0];
    let field = |i: usize| u32_at(bytes, 10 + 4 * i).ok();
    let (x, y, w, h) = (field(0)? as i32, field(1)? as i32, field(2)? as i32, field(3)? as i32);
    if !(0..=640).contains(&x) || !(0..=480).contains(&y) || !(1..=640).contains(&w) || h < 1 {
        return None;
    }
    let style = field(4)?;
    let (german, pos) = cstr(bytes, 32)?;
    let (_, pos) = cstr(bytes, pos)?;
    let (_, pos) = cstr(bytes, pos)?;
    let pos = pos + 5;
    let id = u16_at(bytes, pos)?;
    let (_, pos) = cstr(bytes, pos + 2)?;
    let extra = &bytes[skip_padding(bytes, pos)..];
    let u16s = |i: usize| u16_at(extra, 2 * i);

    let text = if PLACEHOLDERS.contains(&german.as_str()) { String::new() } else { german };
    let kind = match kind {
        1 => Kind::Static,
        2 => Kind::Button { normal: u16s(5)?, hover: u16s(6)? },
        4 => Kind::ListBox,
        6 => Kind::Anim,
        7 => Kind::Bitmap,
        8 if w > h => Kind::Slider,
        8 => Kind::Spin,
        9 => Kind::Header,
        10 => Kind::Edit { max_len: usize::from(u16s(4)?).clamp(1, 64) },
        11 => Kind::Options { image: u16s(1)?, items: parse_items(extra, usize::from(u16s(0)?)) },
        13 => Kind::Check,
        15 => Kind::Paint,
        _ => Kind::Other,
    };
    let align = match style & 3 {
        1 => Align::Centre,
        2 => Align::Right,
        _ => Align::Left,
    };
    // Statics carry their colour (a COLORREF) and a font number after their
    // name; 5 is the emphasised font.
    let is_static = matches!(kind, Kind::Static);
    let bold = is_static && u16s(2) == Some(5);
    let ink = match extra.get(..3) {
        Some(&[r, g, b]) if is_static => [r, g, b],
        _ => [0; 3],
    };
    Some(Control {
        kind,
        rect: Rect::new(x as f32, y as f32, (x + w) as f32, (y + h) as f32),
        align,
        text,
        id: (id != u16::MAX).then_some(id),
        bold,
        ink,
    })
}

/// The options of a `HighButton`. Each is a rectangle (four `u16`), eight
/// bytes of flags and its German and English label; the group's own data in
/// front of them varies, so items are found by their labels and checked by
/// the rectangle in front. Items without labels, like tabs the game names
/// itself, are read in order instead: after a header of 24 bytes, each with
/// two more bytes after its labels, padded to an even place.
fn parse_items(extra: &[u8], count: usize) -> Vec<Item> {
    let labelled = parse_items_by_label(extra, count);
    if labelled.len() < count {
        let sequential = parse_items_in_order(extra, count);
        if sequential.len() == count {
            return sequential;
        }
    }
    labelled
}

fn parse_items_by_label(extra: &[u8], count: usize) -> Vec<Item> {
    let mut items = Vec::with_capacity(count);
    let mut pos = 4;
    while items.len() < count && pos < extra.len() {
        let start = pos;
        let len = extra[start..].iter().position(|&b| b == 0).unwrap_or(0);
        let printable = len >= 1 && extra[start..start + len].iter().all(|&b| b >= 0x20);
        if printable && start >= 16 {
            let header = |i: usize| u16_at(extra, start - 16 + 2 * i).unwrap_or(0);
            if let Some(rect) = item_rect([header(0), header(1), header(2), header(3)]) {
                let text = cp1252(&extra[start..start + len]);
                let (_, after_english) =
                    cstr(extra, start + len + 1).unwrap_or((String::new(), start + len + 1));
                items.push(Item { rect, text });
                pos = after_english;
                continue;
            }
        }
        pos += len.max(1);
    }
    items
}

const ITEMS_START: usize = 24;
const ITEM_FLAGS: usize = 8;

fn parse_items_in_order(extra: &[u8], count: usize) -> Vec<Item> {
    let mut items = Vec::with_capacity(count);
    let mut pos = ITEMS_START;
    while items.len() < count {
        let field = |i: usize| u16_at(extra, pos + 2 * i);
        let Some(rect) = (|| item_rect([field(0)?, field(1)?, field(2)?, field(3)?]))() else {
            break;
        };
        let Some((text, after)) = cstr(extra, pos + 8 + ITEM_FLAGS) else { break };
        let Some((_, after)) = cstr(extra, after) else { break };
        if text.chars().any(|c| c < ' ') {
            break;
        }
        items.push(Item { rect, text });
        // Two more bytes, and padding to an even place.
        pos = (after + 2).next_multiple_of(2);
    }
    items
}

/// An item's rectangle, if it is one: inside the screen and not empty.
fn item_rect([x, y, w, h]: [u16; 4]) -> Option<Rect> {
    let valid = (1..=640).contains(&w) && (1..=200).contains(&h) && x < 640 && y < 480;
    let (x, y, w, h) = (f32::from(x), f32::from(y), f32::from(w), f32::from(h));
    valid.then(|| Rect::new(x, y, x + w, y + h))
}

/// Graphic ids and their paths in the archives, from the comments of
/// `ListeDerGrafik.rh` (`#define IDB_NAME 1234 //path/file.bmp`).
pub fn load_graphics(path: &Path) -> io::Result<HashMap<u16, String>> {
    let text = std::fs::read(path)?;
    let text: String = text.iter().map(|&b| char::from(b)).collect();
    let mut graphics = HashMap::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        if words.next() != Some("#define") {
            continue;
        }
        let (Some(_), Some(id)) = (words.next(), words.next()) else { continue };
        let (Ok(id), Some(comment)) = (id.parse::<u16>(), line.split("//").nth(1)) else {
            continue;
        };
        graphics.insert(id, comment.trim().replace("\\\\", "/").replace('\\', "/"));
    }
    Ok(graphics)
}

/// The game's texts by id, from the comments of `ListeDerTexte.rh`: each
/// `#define IDS_NAME 1234` is followed by its German text, then the English
/// one and empty slots for more languages, each on a `//` line.
pub fn load_texts(path: &Path) -> io::Result<HashMap<u16, String>> {
    let text = cp1252(&std::fs::read(path)?);
    let mut texts = HashMap::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let mut words = line.split_whitespace();
        let (Some("#define"), Some(_), Some(id)) = (words.next(), words.next(), words.next())
        else {
            continue;
        };
        let Ok(id) = id.parse::<u16>() else { continue };
        if let Some(german) = lines.peek().and_then(|l| l.strip_prefix("//")) {
            texts.insert(id, german.to_owned());
        }
    }
    Ok(texts)
}

/// The texts the game shows, as it reads them at run time: the string table
/// of the dialog file, between its table of dialogs and the first dialog —
/// a u16 id and a NUL-terminated string each, padded to an even place. It
/// has the euros of the update where `ListeDerTexte.rh` still has marks.
pub fn load_string_table(path: &Path) -> io::Result<HashMap<u16, String>> {
    let data = std::fs::read(path)?;
    if !data.starts_with(b"V10\0") {
        return Err(invalid("not a V10 dialog file"));
    }
    let count = u32_at(&data, 4)? as usize;
    let start = 0x10 + 8 * count;
    let end =
        (0..count).filter_map(|i| u32_at(&data, 0x10 + 8 * i).ok()).min().unwrap_or(0) as usize;
    let mut texts = HashMap::new();
    let mut pos = start;
    while pos + 2 < end {
        let Some(id) = u16_at(&data, pos) else { break };
        let Some((text, after)) = cstr(&data, pos + 2) else { break };
        texts.insert(id, text);
        pos = after.next_multiple_of(2);
    }
    Ok(texts)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn skip_padding(bytes: &[u8], mut pos: usize) -> usize {
    while bytes.get(pos) == Some(&0xcd) {
        pos += 1;
    }
    pos
}

fn cstr(bytes: &[u8], pos: usize) -> Option<(String, usize)> {
    let len = bytes.get(pos..)?.iter().position(|&b| b == 0)?;
    Some((cp1252(&bytes[pos..pos + len]), pos + len + 1))
}

fn u16_at(bytes: &[u8], pos: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*bytes.get(pos)?, *bytes.get(pos + 1)?]))
}
