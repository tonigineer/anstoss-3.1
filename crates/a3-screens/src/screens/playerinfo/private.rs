//! The fourth page, "Privat" (dialog 401, `CSpielerinfoPrivat`, filled by
//! `FUN_0055f1d8`): his partner, children and hobby, the team-mates he gets
//! on with and those he does not, his weaknesses, and how content he is —
//! with what, in bars, and why, in a sentence.

use a3_core::mood;
use a3_core::records::Player as Record;

use super::{LAST_REAL_ID, Shown, bars};
use crate::view::View;

const OSTRACISED: u16 = 5650;
const PARTNER_LABEL: u16 = 5648;
const PARTNER: u16 = 4961;
const PARTNERSHIP: u16 = 4962;
const AFFAIR: u16 = 4963;
const CHILDREN: [u16; 5] = [4956, 4957, 4958, 4959, 4960];
const HOBBY: u16 = 4977;
const FRIENDS: u16 = 4942;
const DISLIKES: u16 = 4943;
const UNDERSTANDS: u16 = 4940;
const CLASHES: u16 = 4941;
const BEST_FRIEND: u16 = 4938;
const HATES: u16 = 4939;
const OVERWEIGHT: u16 = 4936;
const SMOKES: u16 = 4944;
const LANGUAGE: u16 = 4937;
const PRESIDENT: u16 = 4933;
const EXTRA_TRAINER: u16 = 4934;
const LICENCE: u16 = 4935;
const CONTENT: u16 = 6003;
/// Condition, freshness and morale: bar, count, and the most it counts to.
const FITNESS_BARS: [(u16, u16, i32); 3] =
    [(5504, 5507, 100), (5505, 5508, 100), (5506, 5509, 120)];
/// His contentment with the trainer, the team, the club, the press, his
/// private life, his distractions and his offers: bar and count.
const CONTENT_BARS: [(u16, u16); 7] = [
    (5497, 4969),
    (5498, 4964),
    (5499, 4965),
    (5500, 4966),
    (5501, 4967),
    (5502, 4968),
    (5503, 4970),
];
const MOST_CONTENT: i32 = 120;

/// Texts of the game.
const WIFE: u16 = 4031;
const OSTRACISED_TEXT: u16 = 4033;
const AGAINST: u16 = 4999;
const FOR: u16 = 5000;

/// Bits of a record's `shown`.
const SHUNNED: u32 = 0x20000;
const FAT: u32 = 0x20;
const SMOKER: u32 = 0x800;
const TRAINS_TOO: u32 = 0x2000;
const HAS_LICENCE: u32 = 0x80;
/// A manager who plays loses at the president's below this confidence.
const PRESIDENT_TRUSTS: i32 = 65;
/// A manager playing has the trainer's full contentment.
const MIDDLING: i32 = 100;

pub(super) fn show(s: &Shown, view: &mut View) {
    let (condition, freshness) = bars(s.fitness);
    let fitness = [condition, freshness, s.fitness.morale];
    for (&(bar, count, most), value) in FITNESS_BARS.iter().zip(fitness) {
        s.put(view, count, value.to_string());
        s.bar(view, bar, value, most);
    }
    let Some(r) = s.record else { return };
    let words = &s.game.rules.words;
    let dash = || "-".to_owned();
    s.put(
        view,
        OSTRACISED,
        if r.shown() & SHUNNED != 0 { s.text(OSTRACISED_TEXT) } else { String::new() },
    );

    // His family, named from the first names of a nation.
    let name = |index: u8| {
        let at = usize::from(r.names_nation()) * 100 + usize::from(index);
        words.first_names.get(at).cloned().unwrap_or_default()
    };
    match usize::from(r.partner() & 3) {
        0 => {
            s.put(view, PARTNER_LABEL, s.text(WIFE));
            s.put(view, PARTNER, dash());
            s.put(view, PARTNERSHIP, dash());
        }
        kind => {
            s.put(view, PARTNER_LABEL, s.text(words.partners[kind]));
            s.put(view, PARTNER, name(r.partner_name()));
            let standing = words.partnerships.get(usize::from(r.partner() >> 2 & 7));
            s.put(view, PARTNERSHIP, standing.map_or_else(String::new, |&t| s.text(t)));
        }
    }
    let affair = r.affair_name();
    s.put(view, AFFAIR, if affair == 0 { dash() } else { name(affair) });
    let children = usize::try_from(r.children()).unwrap_or(0);
    for (i, &id) in CHILDREN.iter().enumerate() {
        let child = match i {
            _ if i < children => name(r.children_names()[i]),
            0 => dash(),
            _ => String::new(),
        };
        s.put(view, id, child);
    }
    let manager = r.id() > LAST_REAL_ID;
    let hobby = match words.hobbies.get(usize::from(r.hobby())) {
        Some(&t) if t != 0 && !manager => s.text(t),
        _ => String::new(),
    };
    s.put(view, HOBBY, hobby);

    // The team-mates.
    let names = |ids: &[u16]| ids.iter().filter_map(|&id| s.known_as(id)).collect();
    s.rows(view, FRIENDS, names(&r.friends()));
    s.rows(view, DISLIKES, names(&r.dislikes()));
    s.rows(view, UNDERSTANDS, names(&r.understands()));
    s.rows(view, CLASHES, names(&r.clashes()));
    s.put(view, BEST_FRIEND, s.known_as(r.best_friend()).unwrap_or_default());
    s.put(view, HATES, s.known_as(r.hates()).unwrap_or_default());

    // What else is remarkable.
    let shown = r.shown();
    s.tick(view, OVERWEIGHT, shown & FAT != 0);
    s.tick(view, SMOKES, shown & SMOKER != 0);
    s.tick(view, LANGUAGE, r.problems() >> 2 & 3 != 0);
    if manager {
        let confidence = s.records.and_then(|records| {
            let slot = usize::from(r.id() - LAST_REAL_ID).checked_sub(1)?;
            Some(records.managers.get(slot)?.shown_confidence())
        });
        s.tick(view, PRESIDENT, confidence.is_some_and(|c| c < PRESIDENT_TRUSTS));
        s.tick(view, EXTRA_TRAINER, false);
        s.tick(view, LICENCE, true);
    } else {
        s.tick(view, PRESIDENT, r.problems() & 3 != 0);
        s.tick(view, EXTRA_TRAINER, shown & TRAINS_TOO != 0);
        s.tick(view, LICENCE, shown & HAS_LICENCE != 0);
    }

    // How content he is.
    let mut parts = mood::parts(r);
    if manager {
        parts[0] = MIDDLING;
    }
    for (&(bar, count), value) in CONTENT_BARS.iter().zip(parts) {
        s.put(view, count, value.to_string());
        s.bar(view, bar, value, MOST_CONTENT);
    }
    let sentence = if manager { String::new() } else { content_sentence(s, r, parts) };
    s.put(view, CONTENT, sentence);
}

/// How content he is in a sentence, with the reason that counts most
/// (`FUN_00770877`, `FUN_0055df7b`): happy to wanting to leave by the sum of
/// his contentment; the reason a bit of his flags, first among those that
/// matter to what he is least content with, grievances (the low sixteen)
/// before what pleases him.
fn content_sentence(s: &Shown, r: &Record, parts: [i32; 7]) -> String {
    let words = &s.game.rules.words;
    let [trainer, team, club, press, private, distraction, offers] = parts;
    let work = trainer + team + club - 200;
    let all = work + press + private + distraction + offers - 300;
    let level = match () {
        _ if work < 70 => 3,
        _ if all < 170 => 2,
        _ if all < 201 => 1,
        _ => 0,
    };
    let mut sentence = s.text(words.contentment[level]);
    let mut weakest = (trainer, 0);
    for (i, value) in [(1, team), (2, club)] {
        if value < weakest.0 {
            weakest = (value, i);
        }
    }
    let mask = words.reason_masks[weakest.1];
    let flags = r.flags();
    let has = |b: &u32| flags & (1 << b) != 0;
    let reason = (0..16)
        .find(|b| mask & (1 << b) != 0 && has(b))
        .or_else(|| (0..16).find(has))
        .or_else(|| (16..32).find(|b| mask & (1 << b) != 0 && has(b)))
        .or_else(|| (0..32).find(has));
    if let Some(b) = reason {
        let why = words.reasons.get(b as usize).map_or_else(String::new, |&t| s.text(t));
        if why.chars().count() > 1 {
            let side = s.text(if b < 16 { AGAINST } else { FOR });
            sentence = format!("{sentence} {side}{why}");
        }
    }
    sentence
}
