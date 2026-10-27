//! What the game tells of the next opponent (the line-up's "Spielsysteme"
//! button 7, `FUN_004b1eea`): dialog 83 for a club of a league played —
//! its board and staff, finances, stand in the league, cup and European
//! cups, records at home, away and against your team, stadium, gates and
//! titles, and the last ten seasons' results against it — dialog 82 for a
//! foreign club, its stadium; dialog 81 for anyone else. OK closes them
//! (GEGNERINFO.md in the RE repo).

use std::collections::HashMap;

use a3_core::opponent::{self, CupStand, EuropeStand, Kind, Record};
use a3_core::records::{Records, Team};
use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel, set};
use crate::controls::bitmap::Picture;
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

pub const EMPTY: u16 = 81;
pub const FOREIGN: u16 = 82;
pub const STANDARD: u16 = 83;
const CAPTION_ID: u16 = 9928;

/// The crest and the name, on each dialog.
const CRESTS: [(u16, u16, u16); 3] =
    [(EMPTY, 1816, 1817), (FOREIGN, 1821, 1822), (STANDARD, 1827, 1828)];
const FOREIGN_STADIUM: u16 = 1820;
/// Dialog 83's values.
const BOARD: u16 = 1844;
const MANAGER: u16 = 1845;
const TRAINER: u16 = 1846;
const JOB: u16 = 1847;
const FINANCES: u16 = 1848;
const STAND: u16 = 1829;
const HOME: [u16; 3] = [1832, 1833, 1834];
const AWAY: [u16; 3] = [1835, 1836, 1837];
const CUP: u16 = 1830;
const EUROPE: u16 = 1831;
const THEIR_HOME: [u16; 3] = [1838, 1839, 1840];
const YOUR_HOME: [u16; 3] = [1841, 1842, 1843];
const STADIUM: u16 = 1825;
const GATES: u16 = 1826;
const TITLES: [u16; 5] = [1849, 1850, 1851, 1852, 1853];
const HISTORY_TITLE: u16 = 1770;
const HISTORY: u16 = 5248;
const HISTORY_SEASONS: usize = 10;

/// Texts of the game.
const STADIUM_TEXT: u16 = 1462;
const GATES_TEXT: u16 = 1463;
const PLACE: u16 = 1058;
const POINTS: [u16; 3] = [1464, 1465, 1466];
const GAMES: [u16; 2] = [1468, 1469];
const LATELY: u16 = 1478;
/// Wins, draws and defeats: none, one, many.
const WINS: [u16; 3] = [1459, 1454, 1453];
const DRAWS: [u16; 3] = [1460, 1455, 1456];
const DEFEATS: [u16; 3] = [1461, 1457, 1458];
const CUP_NOT_IN: u16 = 1517;
const CUP_OUT: u16 = 1518;
const CUP_WON: u16 = 1525;
const CUP_LOST_FINAL: u16 = 1526;
const CUP_ROUND: u16 = 1527;
const EUROPE_NOT_IN: u16 = 2488;
/// Out of the Champions League, of the UEFA cup ("ausge. %s, %s", the
/// second without the space).
const EUROPE_OUT: [u16; 2] = [2492, 2494];

/// Puts each dialog's title in its frame.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [EMPTY, FOREIGN, STANDARD] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
        dialog.controls.insert(0, caption);
    }
}

/// The dialog the game shows for a club: the short one where the save
/// tells nothing.
pub fn dialog_for(records: Option<&Records>, team: Option<&Team>) -> u16 {
    match (records, team) {
        (Some(records), Some(team)) => {
            match opponent::kind(records, team.country().into(), team.level()) {
                Kind::Standard => STANDARD,
                Kind::Foreign => FOREIGN,
                Kind::Empty => EMPTY,
            }
        }
        _ => EMPTY,
    }
}

pub struct Opponent;

impl Screen for Opponent {
    fn ids(&self) -> &[u16] {
        &[EMPTY, FOREIGN, STANDARD]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let Args::Club(slot, club) = args else { return };
        let (game, dialog) = (cx.game, cx.dialog);
        let title = dialog.title.clone();
        set(view, dialog, CAPTION_ID, title);
        let name = game.world.country(slot).and_then(|c| c.clubs.get(club)).map(|c| c.name.clone());
        if let Some(&(_, crest, name_id)) = CRESTS.iter().find(|(id, ..)| *id == view.id) {
            if let Some(c) = control(dialog, crest) {
                let crest = game.rules.crest(&game.world, cx.state.records.as_ref(), (slot, club));
                view.set_picture(c, crest.map(Picture::file));
            }
            set(view, dialog, name_id, name.clone().unwrap_or_default());
        }
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(nation) = game.world.country(slot).map(|c| c.nation) else { return };
        let Some(team) = u16::try_from(club).ok().and_then(|n| records.team(nation, n)) else {
            return;
        };
        let stadium = |c: u16| {
            let s = opponent::stadium(records, team.country().into(), team.number().into());
            let text = s.map(|s| {
                game.text(STADIUM_TEXT).replacen("%1", &s.name, 1).replacen(
                    "%2",
                    &s.capacity.to_string(),
                    1,
                )
            });
            (c, text.unwrap_or_default())
        };
        match view.id {
            FOREIGN => {
                let (c, text) = stadium(FOREIGN_STADIUM);
                set(view, dialog, c, text);
            }
            STANDARD => {
                let season = cx.state.season.as_ref();
                let own = season.and_then(|s| s.manager()?.club).and_then(|(s, c)| {
                    records.team(game.world.country(s)?.nation, u16::try_from(c).ok()?)
                });
                standard(game, records, team, own, view, dialog);
                let (c, text) = stadium(STADIUM);
                set(view, dialog, c, text);
            }
            _ => {}
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        if ok_or_cancel(cx.dialog, target).0 {
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}

/// A count as the dialog writes it: "kein Sieg", "ein Sieg", "3 Siege".
fn count(game: &Game, n: u32, texts: [u16; 3]) -> String {
    match n {
        0 => game.text(texts[0]).to_owned(),
        1 => game.text(texts[1]).replacen("%1", "1", 1),
        _ => game.text(texts[2]).replacen("%1", &n.to_string(), 1),
    }
}

fn record_texts(game: &Game, [won, drawn, lost]: Record) -> [String; 3] {
    [count(game, won, WINS), count(game, drawn, DRAWS), count(game, lost, DEFEATS)]
}

/// Dialog 83, in the order the original fills it (`FUN_004b267e`).
fn standard(
    game: &Game,
    records: &Records,
    team: &Team,
    own: Option<&Team>,
    view: &mut View,
    dialog: &Dialog,
) {
    let text = |id: u16| game.text(id).to_owned();
    set(view, dialog, BOARD, opponent::board_text(team.board()).map_or_else(str::to_owned, text));
    let (trainer, manager, job) = opponent::staff(records, team);
    set(view, dialog, TRAINER, trainer.unwrap_or_else(text));
    set(view, dialog, MANAGER, manager.unwrap_or_else(text));
    if let Some(job) = job {
        set(view, dialog, JOB, text(opponent::job_text(job)));
    }
    let figure = a3_core::finances::figure(team, records.globals.fund_prices());
    set(view, dialog, FINANCES, text(opponent::finance_text(figure)));

    let calendar = &game.calendar;
    match opponent::stand(records, team) {
        Some(stand) => {
            let suffix = usize::from(stand.place);
            let suffix = if suffix < 25 { suffix } else { suffix % 10 };
            let suffix =
                if suffix < 10 { 1059 + suffix as u16 } else { 1530 + (suffix - 10) as u16 };
            let place = game.text(PLACE).replacen("%1", &stand.place.to_string(), 1).replacen(
                "%2",
                game.text(suffix),
                1,
            );
            let points = match stand.points {
                0 => text(POINTS[0]),
                1 => text(POINTS[1]),
                n => game.text(POINTS[2]).replacen("%1", &n.to_string(), 1),
            };
            let games = match stand.games {
                1 => text(GAMES[0]),
                n => game.text(GAMES[1]).replacen("%1", &n.to_string(), 1),
            };
            let mut line = format!("{place} ({points},{games})");
            if !stand.form.is_empty() {
                line = format!("{line} ({}: {})", text(LATELY), stand.form);
            }
            set(view, dialog, STAND, line);
            for (ids, home) in [(HOME, true), (AWAY, false)] {
                let record = opponent::season_record(records, calendar, team, home);
                for (id, line) in ids.into_iter().zip(record_texts(game, record)) {
                    set(view, dialog, id, line);
                }
            }
        }
        None => {
            for id in [STAND].into_iter().chain(HOME).chain(AWAY) {
                set(view, dialog, id, String::new());
            }
        }
    }
    let cup = match opponent::cup_stand(records, team) {
        CupStand::NotIn => text(CUP_NOT_IN),
        CupStand::Out => text(CUP_OUT),
        CupStand::Won => text(CUP_WON),
        CupStand::LostFinal => text(CUP_LOST_FINAL),
        CupStand::Round(round, against) => {
            game.text(CUP_ROUND).replacen("%s", game.text(round), 1).replacen("%s", &against, 1)
        }
    };
    set(view, dialog, CUP, cup);
    let names = records.countries.cup_names();
    let europe = match opponent::europe_stand(records, team) {
        EuropeStand::NotIn => text(EUROPE_NOT_IN),
        EuropeStand::In(cup, round, against) => {
            let line = format!("{} {}", names[cup], game.text(round));
            match against {
                Some((country, number)) => {
                    format!("{line} ({})", game.club_name(Some(records), country, number))
                }
                None => line,
            }
        }
        EuropeStand::Out(cup, round) => game
            .text(EUROPE_OUT[cup])
            .replacen("%s", &names[cup], 1)
            .replacen("%s", game.text(round), 1),
    };
    set(view, dialog, EUROPE, europe);

    if let Some(own) = own {
        let theirs = opponent::home_record(records, calendar, team, own);
        for (id, line) in THEIR_HOME.into_iter().zip(record_texts(game, theirs)) {
            set(view, dialog, id, line);
        }
        // Your home games, from their side: your wins their defeats.
        let [won, drawn, lost] = opponent::home_record(records, calendar, own, team);
        for (id, line) in YOUR_HOME.into_iter().zip(record_texts(game, [lost, drawn, won])) {
            set(view, dialog, id, line);
        }
    }
    let games = usize::from(team.home_games());
    let gates = if games == 0 {
        String::new()
    } else {
        let sum: i64 = team.gates().iter().take(games).map(|&g| i64::from(g)).sum();
        game.text(GATES_TEXT).replacen("%1", &(sum / games as i64).to_string(), 1)
    };
    set(view, dialog, GATES, gates);
    let [league, cup, europe, moves] = opponent::titles(records, team);
    for (id, n) in TITLES.into_iter().zip([league, cup, europe, moves, moves]) {
        set(view, dialog, id, n.to_string());
    }
    let heading = control(dialog, HISTORY_TITLE).map_or("", |c| dialog.controls[c].text.as_str());
    set(view, dialog, HISTORY_TITLE, heading.replacen("%s", &team.name(), 1));
    if let Some(own) = own {
        history(records, team, own, view, dialog);
    }
}

/// The last ten seasons' results between the two clubs, newest first: your
/// home games, then theirs, the second of each in a league of four rounds
/// (the list 5248).
fn history(records: &Records, team: &Team, own: &Team, view: &mut View, dialog: &Dialog) {
    let year = i32::from(records.globals.start_year());
    let four = opponent::League::of(records, own.country().into(), own.level())
        .is_some_and(|l| records.globals.played(own.country() as i16) && l.four_rounds());
    let score = |r: Option<(u8, u8)>, none: &str| {
        r.map_or_else(|| none.to_owned(), |(a, b)| format!("{a}:{b}"))
    };
    let rows = (0..HISTORY_SEASONS)
        .map(|k| {
            let k32 = k as i32;
            let label = format!("{:02}/{:02}", (year - k32 + 99) % 100, (year - k32 + 100) % 100);
            let second = |a: &Team, b: &Team| {
                if four {
                    score(opponent::archived_result(records, a, b, k, true), "")
                } else {
                    String::new()
                }
            };
            [
                label.clone(),
                score(opponent::archived_result(records, own, team, k, false), "-"),
                second(own, team),
                label,
                score(opponent::archived_result(records, team, own, k, false), "-"),
                second(team, own),
            ]
            .join("\t")
        })
        .collect();
    if let Some(list) = control(dialog, HISTORY).and_then(|c| view.list_mut(c)) {
        list.rows = rows;
    }
}
