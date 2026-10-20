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
    /// What the game adds of its own: [`HELP_FLAG`], [`PRINTER_FLAG`].
    pub flags: u32,
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
    /// Which of the game's fonts a static's text is drawn in: [`PLAIN_FONT`],
    /// [`BOLD_FONT`] …; the plain one for the other controls.
    pub font: u8,
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
    /// A list of rows (`CMyListBox`): how high a row is, and its style —
    /// bits 2 and 3 and 7 for its scrollbar, bits 4 to 6 its colours.
    ListBox {
        row_height: u8,
        style: u8,
        columns: Vec<Column>,
        /// Where its rows' icons go, slot by slot, from its left edge.
        icons: Vec<f32>,
    },
    Bitmap,
    /// The game's `AniStatic`: a picture that can play a series of frames.
    Anim,
    /// The tiny up/down arrows next to a number: its colour, and the
    /// game's style bits (0x40 small arrows of their own).
    Spin {
        color: u16,
        style: u16,
    },
    /// A horizontal scrollbar: arrows at both ends, a handle in between, in
    /// one of the game's colours (0 blue, 1 red, 3 green, 4 yellow).
    Slider {
        style: u16,
    },
    /// Column titles of a list, separated by tabs, at their columns (the
    /// game's `CMyTabStatic`).
    Header {
        columns: Vec<Column>,
    },
    Edit {
        max_len: usize,
    },
    /// The game's `HighButton`: a group of options, one of them chosen. With an
    /// `image`, the chosen option is drawn from that strip of buttons;
    /// without, a radio bullet painted into the background marks it.
    Options {
        image: u16,
        items: Vec<Item>,
        /// Bits of the game's own: [`TOGGLE_OPTIONS`].
        style: u16,
        /// Its labels' colour, and the chosen option's (COLORREFs after the
        /// image).
        ink: [u8; 3],
        chosen_ink: [u8; 3],
    },
    /// A box to tick, with its label beside it.
    Check,
    /// A field that shows one of a list of texts, the list dropping down
    /// under it to choose another: the game's `CMyComboBox`. How far the
    /// list drops below the field's window (`+0xfc`), and its style: bits 4
    /// to 6 the colour of its scrollbar's handle (`+0x315`); where its
    /// rows' tab-separated columns are.
    Combo {
        drop: u16,
        style: u8,
        columns: Vec<Column>,
    },
    /// A button that steps through states, a picture each, at every click:
    /// the game's `CMyPushButton` with states (its kind 3); whether its
    /// pictures' black is see-through (style bit 0x8).
    States {
        pictures: Vec<u16>,
        clear: bool,
    },
    /// A button that shows the dialog's picture until it is held, with its
    /// text on it if it has one: the game's `CMyPushButton`, and some of
    /// its bitmap buttons.
    PushButton {
        normal: u16,
        hover: u16,
    },
    /// A menu that opens where a right click falls, which the screens put
    /// in; the game's `CPopupMenue`.
    Menu,
    /// A player's box on the pitch or the bench of the line-up, the game's
    /// `CMannStatic`, which draws itself.
    Player,
    /// A marker on the list view's small pitch, one of the game's bitmap
    /// buttons the screen moves, pictures and labels itself.
    Marker,
    /// The menu bar the game puts along the bottom of its main screens, its
    /// `CLeiste`: a strip of symbols, which draws itself.
    Bar {
        items: Vec<Item>,
    },
    /// A strip the screen paints bars into, the game's `CMyPaint`.
    Paint,
    /// An upright slider, the game's `CMySchieber` (its kind 12 with its
    /// second word 0; lying ones are not read yet): the picture of its
    /// handle.
    Fader {
        handle: u16,
    },
    Other,
}

#[derive(Clone)]
pub struct Item {
    /// Relative to the control.
    pub rect: Rect,
    /// Where its label starts, from the rectangle's corner; without a
    /// height the label is centred between top and bottom.
    pub text_at: Vec2,
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

/// The game's fonts by their numbers in the dialog file (the table at
/// 0x8adcb8 in `anstoss3.exe`): the dialog font, its bold face, a larger
/// one, the captions' and an italic one.
pub const PLAIN_FONT: u8 = 0;
pub const LARGE_FONT: u8 = 2;
pub const CAPTION_FONT: u8 = 3;
pub const ITALIC_FONT: u8 = 4;
pub const BOLD_FONT: u8 = 5;
pub const FONTS: u8 = 6;

/// A dialog's flags: a help button in its top right corner, and a
/// printer's left of it.
pub const HELP_FLAG: u32 = 1;
pub const PRINTER_FLAG: u32 = 2;

/// Where a dialog record's German title starts.
const TITLE: usize = 0x1e;
/// A bitmap button's style bit: the dialog's picture shows until it is
/// held (its bit 0x200 draws the text on it, which only such have).
const HELD_STYLE: u16 = 0x100;
/// Where a group of options keeps its style (`CMyHighButton` +0x2e6), in
/// words of its extra data; and its bit that lets a click on the chosen
/// option clear it (`OnLButtonDown_007a6d03`).
const OPTIONS_STYLE: usize = 11;
/// Where a group's label colours are in its data.
const LABEL_INK: usize = 4;
const CHOSEN_INK: usize = 8;

/// A COLORREF in a control's data, as red, green and blue.
fn colour(extra: &[u8], at: usize) -> [u8; 3] {
    let [r, g, b, _] = u32_at(extra, at).unwrap_or(0).to_le_bytes();
    [r, g, b]
}
pub const TOGGLE_OPTIONS: u16 = 0x400;
/// Where a combo box keeps how far its list drops (in words) and its style
/// (in bytes) in its extra data.
const COMBO_DROP: usize = 14;
const COMBO_STYLE: usize = 31;
const DEFAULT_DROP: u16 = 100;

fn parse_dialog(record: &[u8]) -> Option<Dialog> {
    let size = Vec2::new(u32_at(record, 0x10).ok()? as f32, u32_at(record, 0x14).ok()? as f32);
    let title = cstr(record, TITLE).map_or_else(String::new, |(title, _)| title);
    let name = find(record, b"IDD_")?;
    let mut pos = name + record[name..].iter().position(|&b| b == 0)? + 1;
    pos = skip_padding(record, pos);
    let background = u16_at(record, pos)?;
    let flags = u32_at(record, pos + 2).unwrap_or(0);

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
        // The original centres on the middle of the screen, half the size
        // rounded down: an odd height sits a pixel lower.
        let at = screen / 2.0 - (size / 2.0).floor();
        Rect::from_corners(at, at + size)
    });
    if let Some(frame) = frame {
        for control in &mut controls {
            control.rect =
                Rect::from_corners(control.rect.min + frame.min, control.rect.max + frame.min);
        }
    }
    Some(Dialog { title, background, flags, controls, frame })
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
    // Four texts — the German label, the English, two notes, mostly empty
    // —, four bytes, the id and its name.
    let (german, pos) = cstr(bytes, 32)?;
    let (_, pos) = cstr(bytes, pos)?;
    let (_, pos) = cstr(bytes, pos)?;
    let (_, pos) = cstr(bytes, pos)?;
    let pos = pos + 4;
    let id = u16_at(bytes, pos)?;
    let (_, pos) = cstr(bytes, pos + 2)?;
    let extra = &bytes[skip_padding(bytes, pos)..];
    let u16s = |i: usize| u16_at(extra, 2 * i);

    let text = if PLACEHOLDERS.contains(&german.as_str()) { String::new() } else { german };
    let kind = match kind {
        1 => Kind::Static,
        // A bitmap button that shows the dialog's picture until it is held
        // is a push button, with its text on it if it has one
        // (`CMyBitmapButton`'s style bits 0x100 and 0x200).
        2 if u16s(7)? & HELD_STYLE != 0 => Kind::PushButton { normal: u16s(5)?, hover: u16s(6)? },
        2 => Kind::Button { normal: u16s(5)?, hover: u16s(6)? },
        // The list's font, row height and style (`FUN_007aa393`: the
        // control's +0x1e8, +0x1ea and +0x1eb).
        // A few lists hold the file's padding there (0xcdcd): nothing set.
        // The number of states, then a picture, a word and a text each.
        3 => match u16s(8) {
            Some(count @ 1..=MOST_STATES) => Kind::States {
                pictures: (0..usize::from(count)).filter_map(|i| u16s(9 + 3 * i)).collect(),
                clear: u16s(3).unwrap_or(0) & CLEAR_STYLE != 0,
            },
            _ => Kind::Other,
        },
        4 => {
            let columns = columns(extra, LIST_COLUMNS);
            // Where a row's icons go, slot by slot (words 2 to 11).
            let icons = (ICON_SLOTS.0..ICON_SLOTS.1)
                .map_while(|i| u16s(i).filter(|&x| x != 0).map(f32::from))
                .collect();
            match u16s(14).unwrap_or(0).to_le_bytes() {
                [row_height, style] if row_height < 0x40 => {
                    Kind::ListBox { row_height, style, columns, icons }
                }
                _ => Kind::ListBox { row_height: 0, style: 0, columns, icons },
            }
        }
        6 => Kind::Anim,
        7 => Kind::Bitmap,
        8 if w > h => Kind::Slider { style: u16s(0)? },
        8 => Kind::Spin { color: u16s(0).unwrap_or(0), style: u16s(2).unwrap_or(0) },
        9 => Kind::Header { columns: columns(extra, HEADER_COLUMNS) },
        10 => Kind::Edit { max_len: usize::from(u16s(4)?).clamp(1, 64) },
        11 => Kind::Options {
            image: u16s(1)?,
            items: parse_items(extra, usize::from(u16s(0)?)),
            style: u16s(OPTIONS_STYLE).unwrap_or(0),
            ink: colour(extra, LABEL_INK),
            chosen_ink: colour(extra, CHOSEN_INK),
        },
        5 => Kind::Combo {
            drop: u16s(COMBO_DROP).unwrap_or(DEFAULT_DROP),
            style: extra.get(COMBO_STYLE).copied().unwrap_or(0),
            columns: columns(extra, COMBO_COLUMNS),
        },
        13 => Kind::Check,
        14 => Kind::PushButton { normal: u16s(5)?, hover: u16s(6)? },
        15 => Kind::Paint,
        12 if u16s(1) == Some(0) => Kind::Fader { handle: u16s(0)? },
        _ => Kind::Other,
    };
    let align = match style & 3 {
        1 => Align::Centre,
        2 => Align::Right,
        _ => Align::Left,
    };
    // Statics carry their colour (a COLORREF) and a font number after their
    // name; column titles the font (`CMyTabStatic` +0xf0).
    let is_static = matches!(kind, Kind::Static);
    let has_font = is_static || matches!(kind, Kind::Header { .. });
    let font = u16s(2)
        .and_then(|f| u8::try_from(f).ok())
        .filter(|&f| has_font && f < FONTS)
        .unwrap_or(PLAIN_FONT);
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
        font,
        ink,
    })
}

/// Where a column of a list or of its titles starts, relative to the
/// control, and how its text sits: from there, or centred or to the right
/// in the width given.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Column {
    pub at: f32,
    pub align: Option<(Align, f32)>,
}

/// Where a list's and a header's columns are in their data: the word with
/// their count, the first's; each takes four words, its place — 0x4000
/// centred, 0x8000 right-aligned — and its width first.
const LIST_COLUMNS: (usize, usize) = (12, 15);
/// A list's icon slots, in words of its extra data.
const ICON_SLOTS: (usize, usize) = (2, 12);
const HEADER_COLUMNS: (usize, usize) = (3, 4);
const COMBO_COLUMNS: (usize, usize) = (12, 16);
/// A stepping button whose pictures' black is see-through.
const CLEAR_STYLE: u16 = 0x8;
/// The most states a stepping button has.
const MOST_STATES: u16 = 8;
const MOST_COLUMNS: u16 = 50;

fn columns(extra: &[u8], (count, first): (usize, usize)) -> Vec<Column> {
    let word = |i: usize| u16_at(extra, 2 * i);
    let count = word(count).unwrap_or(0).min(MOST_COLUMNS);
    (0..usize::from(count))
        .map_while(|i| {
            let (place, width) = (word(first + 4 * i)?, f32::from(word(first + 4 * i + 1)?));
            let align = if place & 0x8000 != 0 {
                Some((Align::Right, width))
            } else if place & 0x4000 != 0 {
                Some((Align::Centre, width))
            } else {
                None
            };
            Some(Column { at: f32::from(place & 0x3fff), align })
        })
        .collect()
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
                let text_at = text_at(extra, start - ITEM_FLAGS);
                items.push(Item { rect, text_at, text });
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
        items.push(Item { rect, text_at: text_at(extra, pos + 8), text });
        // Two more bytes, and padding to an even place.
        pos = (after + 2).next_multiple_of(2);
    }
    items
}

/// Where an item's label starts, the first two of its flags.
fn text_at(extra: &[u8], flags: usize) -> Vec2 {
    let at = |i: usize| f32::from(u16_at(extra, flags + 2 * i).unwrap_or(0) as i16);
    Vec2::new(at(0), at(1))
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
