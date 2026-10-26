//! The manager's stations and his titles (dialog 405 `CStationenErfolge`;
//! ERFOLGE.md §A.1): one full screen of what his career record at
//! `0xa4de58[slot]` kept — the titles he won, the stations he went
//! through, clubs and national teams merged by their start, his games and
//! their wins, and his level's history as a line. Nothing here writes.

use chrono::{Datelike, NaiveDate};

use a3_files::rch::Dialog;

use crate::pointer::Target;
use crate::screens::playerinfo::age;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, caption, control, mark_inert, set};
use crate::view::View;

pub(crate) const TRAINER_DIALOG: u16 = 405;
const CAPTION_ID: u16 = 9945;
/// The manager's both names, his age and birthday, his level.
const NAME: u16 = 5928;
const AGE: u16 = 5929;
const LEVEL: u16 = 5930;
/// "The level's development" graph, a paint of 228×75.
const GRAPH: u16 = 5991;
/// Ligaspiele, Pokalspiele, Europapokalspiele, Länderspiele.
const GAMES: [u16; 4] = [5955, 5956, 5957, 5958];
/// The sixteen title counts, and the car offsets they stand at.
const TITLE_COUNTS: [u16; 16] = [
    5931, 5932, 5933, 5934, 5935, 5936, 5937, 5938, 5939, 5940, 5941, 5942, 5943, 5944, 5945,
    5946,
];
const TITLE_AT: [usize; 16] = [
    0x0, 0x2, 0x4, 0x6, 0x8, 0xa, 0xc, 0xe, 0x12, 0x14, 0x1a, 0x1c, 0x1e, 0x20, 0x22, 0x24,
];
/// "%s-Siege", the Champions League's name in the file text's place.
const CL_SIEGE: u16 = 6300;
const TITLE_LIST: u16 = 6002;
const STATION_LIST: u16 = 5948;
/// Anzahl Stationen, Entlassen, Selbst gegangen, Spiele gewonnen.
const STATIONS: u16 = 5949;
const FIRED: u16 = 5950;
const QUIT: u16 = 5951;
const WON: u16 = 5952;
/// Trainerjahre and Im Verein seit.
const YEARS: u16 = 5953;
const SINCE: u16 = 5954;

/// "How old is he (%s)" and "The date".
const YEARS_OLD: u16 = 4034;
const DATE: u16 = 1997;
/// A national team's row.
const NATIONAL_ROW: u16 = 4699;
/// A station's end: 0 and 3 the sack, then the contract's end, a way out,
/// a resignation, a takeover, retirement, an interim and its end.
const WAYS: [u16; 9] = [1098, 1099, 1106, 1098, 1100, 1101, 4700, 4227, 4228];
/// A title's kind by its number: the Champions League and the Europa
/// League stand for their names, taken from the game's words.
const TITLE_TEXTS: [Option<u16>; 13] = [
    None, // none
    Some(4157), // Meister
    Some(4162), // Aufstieg
    None, // none
    Some(4158), // Pokalsieger
    Some(4165), // Supercup
    None, // the Champions League
    None, // the Europa League, twice
    None,
    Some(4163), // Europäischer Supercup
    Some(4164), // Klub-WM
    Some(4161), // Weltmeister
    Some(4160), // Europameister
];

// The manager's record (ERFOLGE.md §A.1): his games and wins at +0x21c,
// league and European cup four bytes each, cup and national team two;
// the level's history, a count and up to thirty points of six bytes at
// +0x230; the years he has been a trainer, two bytes at +0x124; and the
// bonus marks
// of the level at +0x19c9 (the object's +0x17d9).
const MARKS_AT: usize = 0x21c;
const POINTS_AT: usize = 0x232;
const POINTS_COUNT_AT: usize = 0x230;
const TRAINER_YEARS_AT: usize = 0x124;
const LEVEL_BONUS_AT: usize = 0x19c9;

// The career record `0xa4de58[slot]`: the title counts at +0 and the
// titles from +0x2a, eight bytes each — the kind, the season's offset,
// the country and the club or 200 for the national team. The club
// stations' count at +0x1c2 and their record from +0x1c4, the national
// teams' at +0x1c3 and +0x624, each 0x1c bytes: country, number, from,
// to, why, role, running totals and the four marks. How often he was
// sacked at +0x1ba and left himself at +0x1bc.
const CAR_TITLES_COUNT: usize = 0x26;
const CAR_TITLES_AT: usize = 0x2a;
const CAR_TITLE_ENTRY: usize = 8;
const CAR_CLUBS_COUNT: usize = 0x1c2;
const CAR_NATIONALS_COUNT: usize = 0x1c3;
const CAR_CLUBS_AT: usize = 0x1c4;
const CAR_NATIONALS_AT: usize = 0x624;
const CAR_STATION_ENTRY: usize = 0x1c;
const CAR_FIRED: usize = 0x1ba;
const CAR_QUIT: usize = 0x1bc;

/// The caption bar across the top of the full screen.
pub(crate) fn compose(dialogs: &mut std::collections::HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&TRAINER_DIALOG) else { return };
    dialog.controls.insert(0, caption(dialog.frame, CAPTION_ID, String::new()));
}

/// One row of the stations list.
struct Station {
    country: u8,
    number: i16,
    from: (u8, u8, u16),
    to: (u8, u8, u16),
    why: u8,
    national: bool,
    marks: [u8; 4],
}

impl Station {
    /// Its start as the merge compares it (`FUN_00437850`): year, month,
    /// then day.
    fn start(&self) -> (u16, u8, u8) {
        (self.from.2, self.from.1, self.from.0)
    }
}

/// The screen.
pub struct TrainerErfolge;

impl TrainerErfolge {
    /// The level as it is shown (`FUN_006a61e5`): what the record says,
    /// with a bonus per mark until twelve, only the −1 above it.
    pub(super) fn shown_level(manager: &a3_core::records::Manager) -> i32 {
        let marks = manager.get::<u8>(LEVEL_BONUS_AT);
        let bonus = |bit: u8, n: i32| i32::from(marks & bit != 0) * n;
        let mut level = i32::from(manager.level());
        if level < 12 {
            level += bonus(0x40, 1) + bonus(0x80, 2) + bonus(0x20, 2) + bonus(0x10, 1)
                + bonus(0x08, 3) + bonus(0x04, 3) - bonus(0x02, 1);
            level = level.min(12);
        } else {
            level -= bonus(0x02, 1);
        }
        level.clamp(0, 15)
    }

    /// A date the way the rows write it (`FUN_00676569`): "7.7.25".
    fn short_date(game: &crate::game::Game, (day, month, year): (u8, u8, u16)) -> String {
        game.text(DATE)
            .replace("%1", &day.to_string())
            .replace("%2", &month.to_string())
            .replace("%3", &format!("{:02}", year % 100))
    }

    /// One station's record at `at` in the career record.
    fn station(car: &[u8], at: usize) -> Option<Station> {
        let e = car.get(at..at + CAR_STATION_ENTRY)?;
        Some(Station {
            country: e[0],
            number: u16::from_le_bytes(e[2..4].try_into().ok()?) as i16,
            from: (e[4], e[5], u16::from_le_bytes(e[6..8].try_into().ok()?)),
            to: (e[8], e[9], u16::from_le_bytes(e[0xa..0xc].try_into().ok()?)),
            why: e[0xc],
            national: at >= CAR_NATIONALS_AT,
            marks: [e[0x18], e[0x19], e[0x1a], e[0x1b]],
        })
    }

    /// Both lists merged by their start (`FUN_00437850`): while both
    /// remain, the club goes first when the national team's start is
    /// later, else the national team; then the rest of either.
    fn stations(car: &[u8]) -> Vec<Station> {
        let club = |i: usize| Self::station(car, CAR_CLUBS_AT + i * CAR_STATION_ENTRY);
        let team = |j: usize| Self::station(car, CAR_NATIONALS_AT + j * CAR_STATION_ENTRY);
        let clubs = usize::from(car.get(CAR_CLUBS_COUNT).copied().unwrap_or_default());
        let teams = usize::from(car.get(CAR_NATIONALS_COUNT).copied().unwrap_or_default());
        let mut rows = Vec::new();
        let (mut i, mut j) = (0, 0);
        while i < clubs && j < teams {
            match (club(i), team(j)) {
                (Some(c), Some(t)) if t.start() <= c.start() => {
                    rows.push(t);
                    j += 1;
                }
                (Some(c), _) => {
                    rows.push(c);
                    i += 1;
                }
                _ => break,
            }
        }
        rows.extend((i..clubs).filter_map(club));
        rows.extend((j..teams).filter_map(team));
        rows
    }
}

impl Screen for TrainerErfolge {
    fn ids(&self) -> &[u16] {
        &[TRAINER_DIALOG]
    }

    /// `FUN_00581ad0`'s OnInitDialog.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        let Some(season) = cx.state.season.as_ref() else { return };
        let Some(manager) = crate::screens::kalkulation::own_manager(cx) else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let today = season.date();
        // YEAR, the season's first year, and FIRST, the game's first.
        let year = i32::from(records.globals.start_year());
        let first = i32::from(records.globals.year());

        set(
            view,
            dialog,
            NAME,
            format!("{} {}", manager.first_name(), manager.last_name()),
        );
        let born = NaiveDate::from_ymd_opt(
            i32::from(manager.birth_year()),
            u32::from(manager.birth_month()),
            u32::from(manager.birth_day()),
        );
        set(
            view,
            dialog,
            AGE,
            game
                .text(YEARS_OLD)
                .replacen("%d", &age(born, today).to_string(), 1)
                .replacen(
                    "%s",
                    &born.map_or_else(String::new, |d| crate::screens::hub::date_text(game, d)),
                    1,
                ),
        );
        if let Some(i) = control(dialog, LEVEL) {
            set(
                view,
                dialog,
                LEVEL,
                dialog.controls[i]
                    .text
                    .replace("%d", &Self::shown_level(manager).to_string()),
            );
        }

        // His games, and what his wins of them make in percent. The
        // league and the European cup count four bytes each, the cup and
        // the national team two.
        let games: [u32; 4] = [
            manager.get::<i32>(MARKS_AT) as u32,
            u32::from(manager.get::<u16>(MARKS_AT + 8)),
            u32::from(manager.get::<u16>(MARKS_AT + 12)),
            u32::from(manager.get::<u16>(MARKS_AT + 16)),
        ];
        let won: [u32; 4] = [
            manager.get::<i32>(MARKS_AT + 4) as u32,
            u32::from(manager.get::<u16>(MARKS_AT + 0xa)),
            u32::from(manager.get::<u16>(MARKS_AT + 0xe)),
            u32::from(manager.get::<u16>(MARKS_AT + 0x12)),
        ];
        for (id, &count) in GAMES.iter().zip(&games) {
            set(view, dialog, *id, count.to_string());
        }
        set(
            view,
            dialog,
            WON,
            match games.iter().sum::<u32>() {
                0 => "-".to_owned(),
                all => format!("{}%", won.iter().sum::<u32>() * 100 / all),
            },
        );

        // The career record's counts and lists.
        let Some(slot) = records.human_slot(season.current) else { return };
        let Some(car) = records.career(slot) else { return };
        let count = |at: usize| -> i32 {
            i32::from(i16::from_le_bytes(
                car.get(at..at + 2)
                    .unwrap_or_default()
                    .try_into()
                    .unwrap_or_default(),
            ))
        };
        for (id, at) in TITLE_COUNTS.iter().zip(TITLE_AT) {
            set(view, dialog, *id, count(at).to_string());
        }
        let cups = records.countries.cup_names();
        if let Some(i) = control(dialog, CL_SIEGE) {
            set(
                view,
                dialog,
                CL_SIEGE,
                dialog.controls[i].text.replace("%s", &cups[0]),
            );
        }
        let title_rows = (0..count(CAR_TITLES_COUNT))
            .filter_map(|k| {
                let e = car.get(CAR_TITLES_AT + usize::try_from(k).ok()? * CAR_TITLE_ENTRY..)?;
                let title = match usize::from(e[0]) {
                    6 => cups[0].clone(),
                    7 | 8 => cups[1].clone(),
                    kind => game.text(TITLE_TEXTS.get(kind).copied().flatten()?).to_owned(),
                };
                let off = i32::from(i16::from_le_bytes(e[2..4].try_into().ok()?));
                Some(format!(
                    "{:02}/{:02}\t{}\t{}",
                    (first + off - 1).rem_euclid(100),
                    (first + off).rem_euclid(100),
                    title,
                    game.club_name(
                        Some(records),
                        e[4],
                        i16::from_le_bytes(e[6..8].try_into().ok()?),
                    )
                ))
            })
            .collect();
        if let Some(list) = control(dialog, TITLE_LIST).and_then(|c| view.list_mut(c)) {
            list.rows = title_rows;
            list.selected = None;
        }

        set(view, dialog, STATIONS, car[CAR_CLUBS_COUNT].to_string());
        let times = |at: usize| {
            u16::from_le_bytes(
                car.get(at..at + 2)
                    .unwrap_or_default()
                    .try_into()
                    .unwrap_or_default(),
            )
        };
        set(view, dialog, FIRED, times(CAR_FIRED).to_string());
        set(view, dialog, QUIT, times(CAR_QUIT).to_string());
        // Two bytes, though the save keeps another value riding next to
        // them, which a four-byte read would pick up.
        set(
            view,
            dialog,
            YEARS,
            manager.get::<u16>(TRAINER_YEARS_AT).to_string(),
        );
        // The seasons at this club, this one counted.
        let joined = i32::from(manager.signed_year()) - i32::from(manager.signed_month() < 7);
        set(view, dialog, SINCE, (year - joined + 1).to_string());
        let station_rows = Self::stations(car)
            .iter()
            .map(|s| {
                let ongoing = s.to.0 == 0;
                let span = format!(
                    "{}-{}",
                    Self::short_date(game, s.from),
                    if ongoing {
                        String::new()
                    } else {
                        Self::short_date(game, s.to)
                    }
                );
                let why = if ongoing {
                    String::new()
                } else {
                    WAYS.get(usize::from(s.why))
                        .map_or_else(String::new, |id| game.text(*id).to_owned())
                };
                if s.national {
                    game.text(NATIONAL_ROW)
                        .replacen("%s", &game.club_name(Some(records), s.country, 200), 1)
                        .replacen("%s", &span, 1)
                        .replacen("%s", &why, 1)
                        .replacen("%d", &s.marks[2].to_string(), 1)
                        .replacen("%d", &s.marks[0].to_string(), 1)
                } else {
                    format!(
                        "{}\t{}\t{}\t{}\t{}\t{}",
                        game.club_name(Some(records), s.country, s.number),
                        span,
                        why,
                        s.marks[0],
                        i32::from(s.marks[1]) + i32::from(s.marks[2]),
                        s.marks[3]
                    )
                }
            })
            .collect();
        if let Some(list) = control(dialog, STATION_LIST).and_then(|c| view.list_mut(c)) {
            list.rows = station_rows;
            list.selected = None;
        }

        // The level's history (`CMyPaintStatic`'s paint): W and H one
        // less than the static, the day numbers as x, the level as y.
        // The saves keep a point's year as one byte, so its high byte is
        // 0 here; only differences of the day number matter.
        let points = (0..manager.get::<i16>(POINTS_COUNT_AT).clamp(0, 30))
            .filter_map(|k| {
                let e = manager.get::<[u8; 6]>(POINTS_AT + 6 * usize::try_from(k).ok()?);
                let year = u16::from_le_bytes(e[2..4].try_into().ok()?);
                let day =
                    NaiveDate::from_ymd_opt(i32::from(year), u32::from(e[1]), u32::from(e[0]))?;
                Some((i64::from(day.num_days_from_ce()), i32::from(e[4])))
            })
            .collect::<Vec<_>>();
        view.lines.clear();
        if let Some(rect) = control(dialog, GRAPH).map(|c| dialog.controls[c].rect) {
            let (w, h) = ((rect.width() - 1.0) as i32, (rect.height() - 1.0) as i32);
            let (x0, y0) = (rect.min.x as i32, rect.min.y as i32);
            let y = |level: i32| y0 + h - h * level / 15;
            if points.len() <= 1 {
                let level = points.first().map_or(0, |&(_, l)| l);
                view.lines.push(([x0, y(level), x0 + w, y(level)], [0, 0, 255]));
            } else {
                let first_day = points[0].0;
                let span = points[points.len() - 1].0 - first_day + 1;
                let mut at = (x0, y(points[0].1));
                for pair in points.windows(2) {
                    let (from, to) = (pair[0], pair[1]);
                    let x = ((to.0 - first_day) * i64::from(w) / span.max(1)) as i32;
                    // A drop draws red, a stay or a rise blue.
                    let colour = if from.1 > to.1 { [255, 0, 0] } else { [0, 0, 255] };
                    view.lines.push(([at.0, at.1, x0 + x, y(to.1)], colour));
                    at = (x0 + x, y(to.1));
                }
            }
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&cx.dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}
