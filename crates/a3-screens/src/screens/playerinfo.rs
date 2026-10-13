//! A player's info, as the game's `CSpielerInfoKomplett` (dialog 214)
//! shows it on its first page, "Aktuell" (dialog 399, `CSpielerinfoAktuell`,
//! filled by `FUN_0055a57e`): who he is, his positions and talent, how fit
//! and in what spirits, his contract and market value, his nations,
//! abilities and injury. The lower arrows go through the squad as the
//! line-up lists it; the other pages are not built yet.
//!
//! What only a save's record holds — his contract, foot, portrait, nations —
//! stays empty for a player of a new game.

use std::collections::HashMap;

use bevy_math::{Rect, Vec2};
use chrono::{Datelike, NaiveDate};
use tracing::info;

use a3_core::lineup;
use a3_core::records::Player as Record;
use a3_core::season::{ClubState, Status};
use a3_core::value::Worth;
use a3_core::world::{GOALKEEPER, Player};
use a3_files::rch::{Align, Control, Dialog, Kind};

use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel, set};
use crate::controls::ControlState;
use crate::controls::bitmap::{Picture, PictureSource};
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

pub const CURRENT: u16 = 399;

const NAME: u16 = 4825;
const AGE: u16 = 4826;
const STRENGTH: u16 = 4827;
const PORTRAIT: u16 = 5481;
const TRAITS: [u16; 4] = [4971, 4972, 4973, 4974];
const POSITIONS: [u16; 3] = [4828, 4829, 4830];
const FOOT: u16 = 4831;
const PRONE: u16 = 4832;
const STANDING: u16 = 4833;
const FORM: u16 = 6032;
const SUSPENDED: u16 = 6207;
const TALENT: u16 = 5033;
const TALENT_TEXT: u16 = 5842;
/// The bars, their values, and the most each counts to.
const BARS: [(u16, u16, i32); 3] = [(5495, 4847, 100), (5494, 4848, 100), (5493, 4849, 120)];
const EUROPE: u16 = 6026;
const CLUB: u16 = 6015;
const SALARY: u16 = 4834;
const APPEARANCE: u16 = 4835;
const GOAL: u16 = 4836;
const EARNED: u16 = 4837;
const OPTION_CLUB: u16 = 4838;
const OPTION_PLAYER: u16 = 4839;
const LEAGUES: u16 = 4840;
const REGULAR: u16 = 4841;
const FEE: u16 = 4842;
const FEE_LABEL: u16 = 4850;
const VALUE: u16 = 4843;
const AUTOMATIC: u16 = 4845;
const UNTIL: u16 = 4844;
const NEXT_SEASON: u16 = 4846;
const FLAGS: [u16; 2] = [5829, 5830];
const NATIONS: [u16; 2] = [6230, 6244];
const HOME_GROWN: u16 = 6249;
const ABILITY_ICONS: [u16; 3] = [5482, 5483, 5484];
const ABILITY_NAMES: [u16; 3] = [6246, 6247, 6248];
const INJURY: u16 = 4878;
const SYRINGES: u16 = 6418;
/// The arrows: through the squad, and through the pages.
const PLAYERS: u16 = 4888;
const PAGES: u16 = 4889;

/// Texts of the game.
const YEARS_OLD: u16 = 4034;
const DATE: u16 = 1997;
const STRENGTH_TEXT: u16 = 1595;
const YES: u16 = 1028;
const NO: u16 = 1029;
const ALL_LEAGUES: u16 = 4086;
const NONE: u16 = 2504;
const PRICE: u16 = 2359;
const RETIRING: u16 = 2487;
const ON_LOAN: u16 = 4183;
const SOME_YEARS: u16 = 1587;
const HOME_GROWN_TEXT: u16 = 2276;
const PRONE_TEXTS: [(u16, u16); 3] = [(0x8, 5037), (0x4, 5038), (0x40, 5039)];
const LOVED: u16 = 4186;
const HATED: u16 = 4187;
const HEALTHY: u16 = 1318;
const KNOCKED: u16 = 1320;
const A_WEEK: u16 = 2913;
const WEEKS: u16 = 1321;
const ILL_A_DAY: u16 = 1875;
const ILL: u16 = 1319;
/// Money: up to a thousand, up to a million, and more.
const MONEY: [u16; 3] = [2157, 2156, 2155];
/// The bars' pictures: red, yellow, green.
const BAR_PICTURES: [(i32, u16); 3] = [(60, 2654), (80, 2653), (i32::MAX, 2655)];
/// The suspended mark, and the syringes.
const SUSPENDED_PICTURE: u16 = 1276;
const SYRINGE_PICTURES: [(u16, u16); 2] = [(0x1000, 1910), (0xc000, 1911)];
/// From this kind an injury is an illness.
const ILLNESSES: u8 = 70;

/// The caption, at the top: the page's title.
const CAPTION_ID: u16 = 9923;
const CAPTION_RECT: Rect = Rect { min: Vec2::new(5.0, 3.0), max: Vec2::new(635.0, 19.0) };

/// Puts the page's title in its caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&CURRENT) else { return };
    let caption = Control {
        kind: Kind::Static,
        rect: CAPTION_RECT,
        align: Align::Left,
        text: dialog.title.clone(),
        id: Some(CAPTION_ID),
        bold: true,
        ink: [0; 3],
    };
    dialog.controls.insert(0, caption);
}

/// The player info, on a player of the current club's squad.
#[derive(Default)]
pub struct PlayerInfo {
    /// The squad as the line-up lists it, and the player shown.
    order: Vec<usize>,
    at: usize,
}

impl Screen for PlayerInfo {
    fn ids(&self) -> &[u16] {
        &[CURRENT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let Some(club) = club_state(cx) else { return };
        self.order = super::hub::list_order(&club.lineup, club.players.len());
        let player = if let Args::Player(p) = args { p } else { 0 };
        self.at = self.order.iter().position(|&p| p == player).unwrap_or(0);
        for id in [PLAYERS, PAGES] {
            if let Some(arrows) = control(cx.dialog, id).and_then(|c| view.options_mut(c)) {
                arrows.actions = true;
            }
        }
        self.show(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if ok_or_cancel(cx.dialog, target).0 {
            return Outcome::Close(Reply::Ok);
        }
        let Target::Option { control: c, item } = target else { return Outcome::Stay };
        match cx.dialog.controls[c].id {
            Some(PLAYERS) if !self.order.is_empty() => {
                let n = self.order.len();
                self.at = if item == 0 { (self.at + n - 1) % n } else { (self.at + 1) % n };
                self.show(cx, view);
            }
            Some(PAGES) => info!("the other pages of the player info: not built yet"),
            _ => {}
        }
        Outcome::Stay
    }
}

fn club_state<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

impl PlayerInfo {
    fn show(&self, cx: &mut Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(season) = cx.state.season.as_ref() else { return };
        let Some((slot, index)) = season.manager().and_then(|m| m.club) else { return };
        let Some(club) = season.clubs.get(&(slot, index)) else { return };
        let Some(&p) = self.order.get(self.at) else { return };
        let player = &club.players[p];
        let fitness = lineup::fitness_of(&club.fitness, p);
        let status = club.status.get(p).copied().unwrap_or_default();
        let records = cx.state.records.as_ref();
        let record = records.and_then(|r| r.player(status.id));
        let today = season.date();
        let words = &game.rules.words;
        let text = |id: u16| game.text(id).to_owned();
        let mut put = |id: u16, s: String| set(view, dialog, id, s);

        // Who he is.
        let nickname = record
            .filter(|r| r.shown() & NICKNAME != 0)
            .and_then(|r| records.map(|records| records.name(r.nickname()).to_owned()));
        put(
            NAME,
            match nickname {
                Some(nick) => format!("{} \"{nick}\" {}", player.first_name, player.last_name),
                None => format!("{} {}", player.first_name, player.last_name),
            },
        );
        let born = player.birthday.map_or_else(String::new, |d| date(game, d));
        put(
            AGE,
            text(YEARS_OLD)
                .replacen("%d", &age(player.birthday, today).to_string(), 1)
                .replacen("%s", &born, 1),
        );
        put(STRENGTH, text(STRENGTH_TEXT).replacen("%s", &player.strength.to_string(), 1));
        // His character, the first four of its bits.
        let character = player.traits.map(|t| (t >> 1) as u16);
        let traits: Vec<String> = (0..2)
            .flat_map(|w| {
                (0..16).filter(move |b| character[w] & (1 << b) != 0).map(move |b| (w, b))
            })
            .map(|(w, b)| text(words.traits[w][b]))
            .take(TRAITS.len())
            .collect();
        for (i, &id) in TRAITS.iter().enumerate() {
            put(id, traits.get(i).cloned().unwrap_or_default());
        }
        for (i, &id) in POSITIONS.iter().enumerate() {
            let position = usize::from(player.positions[i]);
            let name = if i > 0 && position == 0 {
                String::new()
            } else {
                text(words.positions[position.min(10)])
            };
            put(id, name);
        }
        put(FORM, lineup::form(fitness).to_string());
        let europe = record.is_some_and(|r| r.terms() & NOT_IN_EUROPE != 0);
        put(EUROPE, if europe { dialog_text(cx.dialog, EUROPE) } else { String::new() });
        put(
            CLUB,
            game.world
                .country(slot)
                .and_then(|c| c.clubs.get(index))
                .map_or_else(String::new, |c| c.name.clone()),
        );
        let (condition, freshness) = bars(fitness);
        let values = [condition, freshness, fitness.morale];
        for ((_, id, _), value) in BARS.iter().zip(values) {
            put(*id, value.to_string());
        }
        let injury = injury_text(game, record, &status);
        put(INJURY, injury);

        // What only a save tells.
        let r = record;
        let foot = r.map(|r| usize::from((r.manner() >> 2) & 3).clamp(1, 3));
        put(FOOT, foot.map_or_else(String::new, |f| text(words.feet[f])));
        let prone = r.and_then(|r| PRONE_TEXTS.iter().find(|(bit, _)| r.knocks() & bit != 0));
        put(PRONE, prone.map_or_else(String::new, |&(_, t)| text(t)));
        let standing = r.and_then(|r| match (r.flags(), r.manner() & 3) {
            (f, _) if f & FANS_LOVE != 0 => Some(LOVED),
            (f, _) if f & FANS_HATE != 0 => Some(HATED),
            (_, 2) => Some(LOVED),
            (_, 3) => Some(HATED),
            _ => None,
        });
        put(STANDING, standing.map_or_else(String::new, text));
        let talent = r.map(|r| usize::try_from(r.talent()).unwrap_or(0).min(4));
        put(TALENT_TEXT, talent.map_or_else(String::new, |t| text(words.talent_texts[t])));
        let contract = r.filter(|r| r.id() < LAST_REAL_ID);
        let years = status.contract.unwrap_or(0).max(0);
        let money = |v: i64| money(game, v);
        let yes = |on: bool| text(if on { YES } else { NO });
        let dash = || "-".to_owned();
        put(SALARY, contract.map_or_else(dash, |r| money(i64::from(r.salary()) * 1000)));
        put(
            APPEARANCE,
            contract.map_or_else(dash, |r| money(i64::from(r.appearance_bonus()) * 100)),
        );
        put(GOAL, contract.map_or_else(dash, |r| money(i64::from(r.goal_bonus()) * 100)));
        put(EARNED, contract.map_or_else(dash, |r| money(i64::from(r.earned()))));
        put(OPTION_CLUB, contract.map_or_else(dash, |r| yes(r.terms() & OPTION_FOR_CLUB != 0)));
        put(OPTION_PLAYER, contract.map_or_else(dash, |r| yes(r.terms() & OPTION_FOR_PLAYER != 0)));
        put(REGULAR, contract.map_or_else(dash, |r| yes(r.terms() & REGULAR_PLACE != 0)));
        put(LEAGUES, contract.map_or_else(dash, |r| text(ALL_LEAGUES + (r.terms() & 3))));
        put(
            FEE,
            contract.map_or_else(dash, |r| match r.fixed_fee() {
                0 => text(NONE),
                fee => money(i64::from(fee) * 10_000),
            }),
        );
        let loan = contract.is_some_and(|r| r.terms() & ON_LOAN_TERM != 0);
        put(FEE_LABEL, if loan { text(PRICE) } else { dialog_text(dialog, FEE_LABEL) });
        put(
            VALUE,
            contract.map_or_else(dash, |r| {
                if r.retiring() & 0x80 != 0 {
                    return text(RETIRING);
                }
                let stars = records.map(|records| records.globals.stars()).unwrap_or_default();
                let worth = Worth {
                    strength: player.strength,
                    position: player.positions[0],
                    talent: r.talent(),
                    age: age(player.birthday, today),
                    good: r.good(),
                    character: r.traits(),
                    qualities: r.qualities(),
                    star: stars.get(usize::from(player.positions[0])).is_some_and(|&s| s == r.id()),
                    kind_12_matches: r.kind_12_matches(),
                };
                money(game.rules.value.market_value(&worth, years, today.month()))
            }),
        );
        let automatic = contract.is_some_and(|r| r.terms() & EXTENDS != 0);
        put(AUTOMATIC, if automatic { dialog_text(dialog, AUTOMATIC) } else { String::new() });
        put(
            UNTIL,
            match contract {
                None => String::new(),
                Some(_) if loan => text(ON_LOAN),
                Some(_) => {
                    let start = records.map_or(today.year(), |r| i32::from(r.globals.start_year()));
                    let until = NaiveDate::from_ymd_opt(start + years, 6, 30)
                        .map_or_else(String::new, |d| date(game, d));
                    let left = match usize::try_from(years)
                        .ok()
                        .and_then(|y| words.years.get(y).filter(|_| (1..=10).contains(&y)))
                    {
                        Some(&t) => text(t),
                        None => text(SOME_YEARS).replacen("%d", &years.to_string(), 1),
                    };
                    dialog_text(dialog, UNTIL).replacen("%s", &until, 1).replacen("%s", &left, 1)
                }
            },
        );
        put(NEXT_SEASON, String::new());
        let nations: Vec<u8> = r.map(|r| vec![r.nation(), r.second_nation()]).unwrap_or_default();
        let name = |n: u8| game.world.nation(u16::from(n)).to_owned();
        let home_grown = r.is_some_and(|r| r.manner() & HOME_GROWN_BIT != 0);
        let inland = if home_grown { text(HOME_GROWN_TEXT) } else { String::new() };
        match nations[..] {
            [first, second] if second != 0 => {
                put(NATIONS[0], name(first));
                put(NATIONS[1], name(second));
                put(HOME_GROWN, inland);
            }
            [first, _] => {
                put(NATIONS[0], name(first));
                put(NATIONS[1], inland);
                put(HOME_GROWN, String::new());
            }
            _ => {
                for id in NATIONS.into_iter().chain([HOME_GROWN]) {
                    put(id, String::new());
                }
            }
        }

        // The pictures.
        let mut picture = |id: u16, picture: Option<Picture>| {
            if let Some(c) = control(dialog, id) {
                view.set_picture(c, picture);
            }
        };
        let portrait = r.and_then(|r| words.portrait(r.face()));
        picture(PORTRAIT, portrait.map(Picture::graphic));
        picture(TALENT, talent.map(|t| Picture::graphic(words.talent_pictures[t])));
        let suspended = status.away_kind & SUSPENDED_BIT != 0;
        picture(SUSPENDED, suspended.then(|| Picture::graphic(SUSPENDED_PICTURE)));
        let syringe =
            r.and_then(|r| SYRINGE_PICTURES.iter().find(|(bit, _)| r.knocks() & bit != 0));
        picture(SYRINGES, syringe.map(|&(_, g)| Picture::graphic(g)));
        for (i, &id) in FLAGS.iter().enumerate() {
            let nation = nations.get(i).copied().filter(|&n| n != 0);
            picture(id, nation.map(flag));
        }
        let keeper = player.positions[0] == GOALKEEPER;
        let icons = game.rules.pitch.ability_icons(player);
        let names = ability_names(game, player, keeper);
        for (i, &id) in ABILITY_ICONS.iter().enumerate() {
            picture(id, icons.get(i).map(|&g| Picture::graphic(g)));
        }
        for (i, &id) in ABILITY_NAMES.iter().enumerate() {
            set(view, dialog, id, names.get(i).cloned().unwrap_or_default());
        }
        for ((bar, _, most), value) in BARS.iter().zip(values) {
            let Some(c) = control(dialog, *bar) else { continue };
            let rect = dialog.controls[c].rect;
            let shown = value.clamp(0, *most);
            let height = (rect.height() as i32 * shown / most).max(0) as u32;
            let graphic =
                BAR_PICTURES.iter().find(|(below, _)| shown < *below).map_or(0, |&(_, g)| g);
            let picture = Picture {
                source: PictureSource::Graphic(graphic),
                part: Some([0, 0, rect.width() as u32, height.max(1)]),
                black_clear: false,
            };
            if let ControlState::Bitmap(b) = &mut view.controls[c].state {
                b.picture = (height > 0).then_some(picture);
                b.bottom = true;
            }
        }
    }
}

/// Bits of a record that the page reads.
const NICKNAME: u32 = 0x8000_0000;
const NOT_IN_EUROPE: u16 = 0x10;
const OPTION_FOR_CLUB: u16 = 0x2000;
const OPTION_FOR_PLAYER: u16 = 0x1000;
const REGULAR_PLACE: u16 = 0x4000;
const ON_LOAN_TERM: u16 = 0x100;
const EXTENDS: u16 = 0x80;
const FANS_LOVE: u32 = 0x10000;
const FANS_HATE: u32 = 0x400;
const HOME_GROWN_BIT: u8 = 0x10;
const SUSPENDED_BIT: u8 = 0x10;
/// The game's players; the ids above are the managers who play.
const LAST_REAL_ID: u16 = 17_000;

/// The text a control has in the dialog file, as a template.
fn dialog_text(dialog: &Dialog, id: u16) -> String {
    control(dialog, id).map_or_else(String::new, |c| dialog.controls[c].text.clone())
}

/// A date as the game writes it: "18.1.2006".
fn date(game: &Game, d: NaiveDate) -> String {
    game.text(DATE)
        .replace("%1", &d.day().to_string())
        .replace("%2", &d.month().to_string())
        .replace("%3", &d.year().to_string())
}

/// How old he is, as the game counts it (`FUN_0067582a`).
fn age(birthday: Option<NaiveDate>, today: NaiveDate) -> i32 {
    let Some(born) = birthday else { return 0 };
    let had =
        born.month() < today.month() || born.month() == today.month() && born.day() <= today.day();
    (today.year() - born.year() - 1 + i32::from(had)).max(0)
}

/// Condition and freshness as the bars show them (`FUN_00457e00`,
/// `FUN_006ff649`).
fn bars(f: a3_core::season::Fitness) -> (i32, i32) {
    let mut condition = f.condition / 100;
    if f.worn {
        condition = (condition - 5).max(0);
    }
    let fresh: i32 = f.freshness.iter().sum();
    let freshness = if fresh < 0 {
        0
    } else if fresh <= 10_000 {
        fresh / 100
    } else {
        100
    };
    (condition, freshness)
}

/// Money as the game writes it (`FUN_004daf8f`): "144.000 €", "1.36 Mio €".
fn money(game: &Game, value: i64) -> String {
    let sign = if value < 0 { "- " } else { "" };
    let v = value.abs();
    let template = |i: usize| {
        game.text(MONEY[i]).trim_start().trim_start_matches("%c").trim_start().to_owned()
    };
    let body = if v < 1000 {
        template(0).replacen("%d", &v.to_string(), 1)
    } else if v < 1_000_000 {
        template(1).replacen("%d", &(v / 1000).to_string(), 1).replacen(
            "%03d",
            &format!("{:03}", v % 1000),
            1,
        )
    } else {
        template(2).replacen("%d", &(v / 1_000_000).to_string(), 1).replacen(
            "%02d",
            &format!("{:02}", v % 1_000_000 / 10_000),
            1,
        )
    };
    format!("{sign}{body}")
}

/// A flag's picture (`FUN_00696030`).
fn flag(nation: u8) -> Picture {
    let n = nation.clamp(1, 0x74);
    Picture {
        source: PictureSource::Archive(format!("klein/Flag{n:02}.bmp")),
        part: None,
        black_clear: false,
    }
}

/// The names of the abilities his icons show, three at most.
fn ability_names(game: &Game, player: &Player, keeper: bool) -> Vec<String> {
    let words = &game.rules.words;
    let (names, bits) =
        if keeper { (&words.keeper_abilities, 9) } else { (&words.field_abilities, 16) };
    let [good, bad] = player.abilities;
    [good, bad]
        .into_iter()
        .flat_map(|word| (0..bits).filter(move |b| word & (1 << b) != 0))
        .take(3)
        .map(|b| game.text(names[b]).to_owned())
        .collect()
}

/// What his injury is, and how long it keeps him out (`FUN_00731e54`).
fn injury_text(game: &Game, record: Option<&Record>, status: &Status) -> String {
    let text = |id: u16| game.text(id).to_owned();
    if status.injured == 0 {
        return text(if status.knocked { KNOCKED } else { HEALTHY });
    }
    let kind = record.map_or(0, |r| r.injury_kind());
    let name =
        game.rules.words.injuries.get(usize::from(kind)).map_or_else(String::new, |&t| text(t));
    let fill = |template: String, second: String| {
        template.replacen("%1", &name, 1).replacen("%s", &name, 1).replacen("%2", &second, 1)
    };
    if kind < ILLNESSES {
        let weeks = (status.injured + 6) / 7;
        if weeks < 2 {
            fill(text(A_WEEK), String::new())
        } else {
            fill(text(WEEKS), weeks.to_string())
        }
    } else if status.injured == 1 {
        fill(text(ILL_A_DAY), String::new())
    } else {
        fill(text(ILL), status.injured.to_string())
    }
}
