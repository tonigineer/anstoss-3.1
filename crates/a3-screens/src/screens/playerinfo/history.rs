//! The third page, "Historisch" (dialog 400, filled by `FUN_0055c4c7`):
//! his career — for his national team, his titles, honours and
//! punishments as a club's player, his goals and appearances by league,
//! strips of his goals and his strength over the years, and the clubs he
//! has played for.

use a3_files::rch::{Align, Kind};

use super::{Shown, dialog_text, money};
use crate::controls::paint::PaintState;
use crate::view::View;

const INTERNATIONALS: u16 = 4995;
const INTERNATIONAL_GOALS: u16 = 4996;
const EUROPEAN_CHAMPION: u16 = 4997;
const WORLD_CHAMPION: u16 = 4998;
/// Championships to other punishments, as the record keeps them.
const HONOURS: [u16; 14] =
    [4981, 4982, 4983, 4984, 4985, 4986, 4987, 4988, 4989, 4990, 4991, 4992, 4993, 4994];
/// Goals and appearances in the country's four leagues, then in the more
/// countries' (with their labels), the cup and the European cups.
const LEAGUE_GOALS: [u16; 4] = [4999, 5000, 5001, 5002];
const LEAGUE_APPEARANCES: [u16; 4] = [5009, 5010, 5011, 5012];
const MORE_GOALS: [u16; 4] = [5003, 5004, 5005, 5006];
const MORE_APPEARANCES: [u16; 4] = [5013, 5014, 5015, 5016];
const MORE_GOALS_LABELS: [u16; 4] = [5021, 5022, 5023, 5024];
const MORE_APPEARANCES_LABELS: [u16; 4] = [5025, 5026, 5027, 5028];
const CUP_GOALS: u16 = 5007;
const CUP_APPEARANCES: u16 = 5017;
const EUROPE_GOALS: u16 = 5008;
const EUROPE_APPEARANCES: u16 = 5018;
const STATIONS: u16 = 5019;
const PROFESSIONAL_YEARS: u16 = 5020;
const AT_THE_CLUB: u16 = 5029;
const CAREER: u16 = 4980;
const GOALS_STRIP: u16 = 5491;
const STRENGTH_STRIP: u16 = 5492;

/// Texts of the game.
const GOALS: u16 = 1689;
const APPEARANCES: u16 = 1688;
const YOUTH_TEAM: u16 = 4300;
const AMATEUR_TEAM: u16 = 4301;

/// A station of his career: 0x14 bytes from the record's +0x20.
const CAREER_AT: usize = 0x20;
const STATION: usize = 0x14;
const MOST_STATIONS: usize = 15;
/// Bits of a station's last byte: strength in the low four, a youth
/// team's, an amateur team's (strength not known).
const YOUTH: u8 = 0x40;
const AMATEURS: u8 = 0x80;

/// The career list's columns: club, when, fee, strength, appearances,
/// goals; and its titles'.
const TABS: [f32; 6] = [0.0, 120.0, 220.0, 305.0, 340.0, 380.0];
const ALIGNS: [Option<(Align, f32)>; 6] = [
    None,
    None,
    Some((Align::Right, 75.0)),
    Some((Align::Right, 16.0)),
    Some((Align::Right, 24.0)),
    Some((Align::Right, 24.0)),
];
const TITLE_TABS: [f32; 6] = [0.0, 140.0, 220.0, 300.0, 335.0, 385.0];
const TITLE_ALIGNS: [Option<(Align, f32)>; 6] = [
    None,
    None,
    Some((Align::Centre, 80.0)),
    Some((Align::Centre, 35.0)),
    Some((Align::Centre, 50.0)),
    Some((Align::Centre, 30.0)),
];

/// The strips: goals a season as high as 31 of them make the strip, more
/// in red; strength a half-season as high as 15 makes it.
const MOST_GOALS: i32 = 31;
const MOST_STRENGTH: i32 = 15;
const WHITE: u32 = 0xffffff;
const RED: u32 = 0xff;

/// A station of his career.
struct Station {
    start: (u8, u8, u16),
    end: (u8, u8, u16),
    fee: i32,
    goals: u16,
    appearances: u16,
    country: u8,
    club: u8,
    kind: u8,
}

pub(super) fn show(s: &Shown, view: &mut View) {
    titles(s, view);
    let Some(r) = s.record else { return };
    let records = s.records;
    let n = |v: u8| v.to_string();
    s.put(view, INTERNATIONALS, n(r.kind_12_matches()));
    s.put(view, INTERNATIONAL_GOALS, r.international_goals().to_string());
    s.put(view, EUROPEAN_CHAMPION, n(r.titles() & 0xf));
    s.put(view, WORLD_CHAMPION, n(r.titles() >> 4));
    for (&id, value) in HONOURS.iter().zip(r.honours()) {
        s.put(view, id, n(value));
    }
    let (goals, appearances) = (r.career_goals(), r.career_appearances());
    for i in 0..4 {
        s.put(view, LEAGUE_GOALS[i], goals[i].to_string());
        s.put(view, LEAGUE_APPEARANCES[i], appearances[i].to_string());
    }
    s.put(view, CUP_GOALS, n(r.career_cup_goals()));
    let cup: u32 = r.career_cup_appearances().iter().map(|&a| u32::from(a)).sum();
    s.put(view, CUP_APPEARANCES, cup.to_string());
    s.put(view, EUROPE_GOALS, n(r.career_europe_goals()));
    s.put(view, EUROPE_APPEARANCES, n(r.career_europe_appearances()));
    let (more, countries) = records.map_or((0, [0; 4]), |records| {
        let g = &records.globals;
        (usize::try_from(g.more_count()).unwrap_or(0).min(4), g.more_countries())
    });
    for i in 0..4 {
        if i < more {
            let country = s.game.world.nation(countries[i] as u16).to_owned();
            s.put(view, MORE_GOALS_LABELS[i], format!("{} {country}", s.text(GOALS)));
            s.put(view, MORE_APPEARANCES_LABELS[i], format!("{} {country}", s.text(APPEARANCES)));
            s.put(view, MORE_GOALS[i], goals[4 + i].to_string());
            s.put(view, MORE_APPEARANCES[i], appearances[4 + i].to_string());
        } else {
            for id in [MORE_GOALS_LABELS, MORE_APPEARANCES_LABELS, MORE_GOALS, MORE_APPEARANCES] {
                s.put(view, id[i], String::new());
            }
        }
    }

    // His clubs.
    let count = usize::try_from(r.career_count()).unwrap_or(0).min(MOST_STATIONS);
    let stations: Vec<Station> = (0..count).map(|i| station(r, i)).collect();
    s.put(view, STATIONS, count.to_string());
    let start_year = records.map_or(0, |records| i32::from(records.globals.start_year()));
    // From his first station as a professional; with none, the original
    // reads the one after his last.
    let first = (0..count).find(|&i| station(r, i).kind & (YOUTH | AMATEURS) == 0);
    let years = start_year + 1 - i32::from(station(r, first.unwrap_or(count)).start.2);
    s.put(view, PROFESSIONAL_YEARS, if (0..=128).contains(&years) { years } else { 0 }.to_string());
    let since = at_the_club_since(s, &stations);
    s.put(
        view,
        AT_THE_CLUB,
        since.map_or_else(String::new, |year| {
            dialog_text(s.dialog, AT_THE_CLUB).replacen("%d", &year.to_string(), 1)
        }),
    );
    let rows = stations.iter().map(|station| row(s, station)).collect();
    s.rows(view, CAREER, rows);
    if let Some(list) = super::control(s.dialog, CAREER).and_then(|c| view.list_mut(c)) {
        list.tabs = TABS.to_vec();
        list.aligns = ALIGNS.to_vec();
    }

    strips(s, view, r);
}

/// The career list's column titles, lined up with its columns.
fn titles(s: &Shown, view: &mut View) {
    let headers = super::super::indices(s.dialog, |k| matches!(k, Kind::Header { .. }));
    for c in headers {
        if let Some(header) = view.header_mut(c) {
            header.tabs = TITLE_TABS.to_vec();
            header.aligns = TITLE_ALIGNS.to_vec();
        }
    }
}

fn station(r: &a3_core::records::Player, i: usize) -> Station {
    let at = CAREER_AT + i * STATION;
    let b: [u8; STATION] = r.get(at);
    let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    Station {
        start: (b[0], b[1], u16_at(2)),
        end: (b[4], b[5], u16_at(6)),
        fee: i32::from_le_bytes([b[8], b[9], b[10], b[11]]),
        goals: u16_at(0xc),
        appearances: u16_at(0xe),
        country: b[0x10],
        club: b[0x11],
        kind: b[0x13],
    }
}

/// A station's row: club, from and to, fee, strength, appearances, goals.
fn row(s: &Shown, station: &Station) -> String {
    let date = |(day, month, year): (u8, u8, u16)| format!("{day}.{month}.{year}");
    let start = if station.start.0 == 0 { String::new() } else { date(station.start) };
    let end = if station.end.0 == 0 { String::new() } else { format!("-{}", date(station.end)) };
    let followed =
        s.records.is_none_or(|records| records.globals.played(i16::from(station.country)));
    let (appearances, goals) = if followed {
        (station.appearances.to_string(), station.goals.to_string())
    } else {
        ("-".to_owned(), "-".to_owned())
    };
    let strength = if station.kind & AMATEURS != 0 {
        "?".to_owned()
    } else {
        (station.kind & 0xf).to_string()
    };
    let name = s.game.club_name(s.records, station.country, station.club.into());
    let club = if station.kind & YOUTH != 0 {
        s.text(YOUTH_TEAM).replacen("%s", &name, 1)
    } else if station.kind & AMATEURS != 0 {
        s.text(AMATEUR_TEAM).replacen("%s", &name, 1)
    } else {
        name
    };
    let fee = money(s.game, i64::from(station.fee) * 1000);
    format!("{club}\t{start}{end}\t{fee}\t{strength}\t{appearances}\t{goals}")
}

/// The year he came to his club, if he is at one: back over the stations
/// at it and the club it is one with (`FUN_0055c48e`).
fn at_the_club_since(s: &Shown, stations: &[Station]) -> Option<u16> {
    let last = stations.last().filter(|station| station.end.0 == 0)?;
    let (country, club) = (last.country, last.club);
    let linked = s
        .records
        .and_then(|records| records.team(country.into(), club.into()))
        .map_or([0, 0], |t| t.linked());
    let pair = if linked[0] < 0 { [club as i8, linked[1]] } else { [linked[0], club as i8] };
    let mut year = last.start.2;
    for station in stations.iter().rev() {
        if station.country != country || !pair.contains(&(station.club as i8)) {
            break;
        }
        year = station.start.2;
    }
    Some(year)
}

/// His goals season by season, and his strength half-season by
/// half-season, a white bar each.
fn strips(s: &Shown, view: &mut View, r: &a3_core::records::Player) {
    let height = |id: u16| {
        super::control(s.dialog, id).map_or(0, |c| s.dialog.controls[c].rect.height() as i32)
    };
    let mut goals = PaintState::default();
    let h_most = height(GOALS_STRIP);
    let mut x = 1;
    for b in r.season_goals().into_iter().take_while(|b| b & 0x80 != 0) {
        let v = i32::from(b & 0x7f);
        let (v, ink) = if v > MOST_GOALS { (MOST_GOALS, RED) } else { (v, WHITE) };
        let h = v * h_most / MOST_GOALS;
        let h = if v > 0 { h.max(1) } else { h };
        goals.bar(x, h_most - h, 8, h, ink);
        x += 11;
    }
    let mut strength = PaintState::default();
    let h_most = height(STRENGTH_STRIP) - 2;
    let mut x = 1;
    'halves: for b in r.development() {
        for half in [b & 0xf, b >> 4] {
            if half == 0 {
                break 'halves;
            }
            let h = i32::from(half) * h_most / MOST_STRENGTH;
            strength.bar(x, h_most - h + 2, 4, h, WHITE);
            x += 5;
        }
    }
    for (id, strip) in [(GOALS_STRIP, goals), (STRENGTH_STRIP, strength)] {
        if let Some(p) = super::control(s.dialog, id).and_then(|c| view.paint_mut(c)) {
            *p = strip;
        }
    }
}
