//! The individual training, the menu bar's sixth symbol (dialog 347,
//! `CTrainingIndividuell`; TRAINING-INDIV.md in the RE repo): the squad by
//! position, each player with his abilities, the one he works on and how
//! far he got with it, his strength, form, condition and freshness, how
//! hard he trains, an extra shift and the youth round. A click on his name
//! or on what he works on opens the abilities he lacks (dialog 365); on his
//! intensity or the two flags it changes them. Four buttons give the
//! intensity by the minutes played in the last match, and a click on one
//! reckons everyone's anew.

use std::collections::HashMap;

use bevy_math::Rect;

use a3_core::individual::{self, Entry, NOTHING};
use a3_core::lineup;
use a3_core::records::Records;
use a3_core::season::{ClubState, Rules};
use a3_core::world::GOALKEEPER;
use a3_files::bitmap_font::to_cp1252;
use a3_files::rch::{Align, Control, Dialog, Kind, PLAIN_FONT};

use super::super::duties::short_name;
use super::super::hub::{self, ABILITY_CHOICE, INDIVIDUAL_TRAINING, INDIVIDUAL_TRAINING_SYMBOL};
use super::super::{
    Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, playerinfo, set,
};
use crate::controls::ControlState;
use crate::controls::list::Bar;
use crate::pointer::{Grip, Target};
use crate::view::View;

/// Controls by their ids in the dialog file.
const LIST: u16 = 3298;
/// The four groups' buttons: more than 70 minutes, 30 to 70, fewer, none.
const BUTTONS: [u16; individual::GROUPS] = [4782, 4783, 4784, 4785];
const CAREER: u16 = 6151;
/// The OK, whose place the bar takes.
const OK: u16 = 1;
const CAPTION_ID: u16 = 9929;
const MENU_ID: u16 = 9952;

/// Texts of the game.
const CAPTION: u16 = 3654;
const NOT_PLAYING: u16 = 4957;
const PLAYER_INFO: u16 = 4036;
const MARK: &str = "X";

/// Where the list's icons go, from its left edge (its data in the dialog
/// file): three abilities, what he works on.
const ABILITY_SLOTS: [f32; 3] = [130.0, 146.0, 162.0];
const WORKS_ON_SLOT: f32 = 186.0;
/// Where the progress bar starts.
const BAR_LEFT: i32 = 210;
/// The columns a click acts on: the name, what he works on, the
/// intensity, the extra shift, the youth round.
const NAME_COLUMN: usize = 0;
const WORKS_ON_COLUMN: usize = 2;
const INTENSITY_COLUMN: usize = 8;
const EXTRA_COLUMN: usize = 9;
const YOUTH_COLUMN: usize = 10;
/// The positions, keeper to striker, the list's groups.
const POSITIONS: std::ops::RangeInclusive<u8> = 1..=10;
/// Out this long, a player's intensity steps differently.
const LONG_OUT: i32 = 7;
/// A player suspended by the club (`R+0x26b`).
const SUSPENDED: u8 = 0x10;
/// A manager who plays too.
const PLAYING: i16 = 1;

/// The popup of abilities: its list, OK and cancel, the question.
const ABILITIES: u16 = 4600;
const CHOICE_OK: u16 = 4601;
const CHOICE_CANCEL: u16 = 4602;
const QUESTION: u16 = 4603;
const CHOICE_CAPTION_ID: u16 = 9953;
/// Where its icons and bars sit.
const CHOICE_ICON: f32 = 10.0;
const CHOICE_BAR: i32 = 140;
/// The last row: no ability.
const NONE_ROW: usize = 100;

/// The bar takes the OK's place; the caption, the menu over everything.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    if let Some(dialog) = dialogs.get_mut(&INDIVIDUAL_TRAINING) {
        dialog.controls.retain(|c| c.id != Some(OK));
        dialog.controls.insert(0, super::super::caption(dialog.frame, CAPTION_ID, String::new()));
        hub::add_bar(dialog, rules);
        dialog.controls.push(Control {
            kind: Kind::Menu,
            rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            align: Align::Left,
            text: String::new(),
            id: Some(MENU_ID),
            font: PLAIN_FONT,
            ink: [0; 3],
        });
    }
    if let Some(dialog) = dialogs.get_mut(&ABILITY_CHOICE) {
        let caption = super::super::caption(dialog.frame, CHOICE_CAPTION_ID, dialog.title.clone());
        dialog.controls.insert(0, caption);
    }
}

#[derive(Default)]
pub struct IndividualTraining {
    /// The player whose abilities the popup shows, by squad place.
    choosing: Option<usize>,
    /// The player whose menu is open; the popup open over the screen.
    menu_player: Option<usize>,
    popup: Option<u16>,
    /// Whether the whistle asks.
    whistle: bool,
}

impl Screen for IndividualTraining {
    fn ids(&self) -> &[u16] {
        &[INDIVIDUAL_TRAINING]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        hub::open_bar(cx, view, INDIVIDUAL_TRAINING_SYMBOL);
        // On the week's last day the short-term training closes too.
        if hub::last_day(cx) {
            hub::switch_off(cx, view, hub::SHORT_TRAINING_SYMBOL);
        }
        if let Some(season) = cx.state.season.as_ref() {
            set(view, dialog, CAPTION_ID, hub::manager_title(game, season, CAPTION));
        }
        if let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) {
            list.by_column = true;
        }
        // Every player gets his choices, which the store keeps as the
        // screen closes (`FUN_00746c48`, `FUN_00746d33`).
        let entries = entries(cx);
        change(cx, |club| {
            for entry in entries {
                individual::keep(&mut club.individual, entry);
            }
        });
        let states = own(cx).map_or(0, |c| c.intensity_states);
        for (group, id) in BUTTONS.into_iter().enumerate() {
            if let Some(button) =
                control(dialog, id).and_then(|c| view.controls[c].state.as_button_mut())
            {
                button.step = usize::from(individual::state(states, group));
            }
        }
        let playing = cx.state.records.as_ref().and_then(|r| {
            let slot = r.human_slot(cx.state.season.as_ref()?.current)?;
            Some(r.managers.get(slot)?.playing())
        });
        if playing.is_some_and(|p| p != PLAYING)
            && let Some(c) = control(dialog, CAREER)
        {
            view.set_disabled(c, true);
            view.set_text(c, game.text(NOT_PLAYING));
        }
        fill(cx, view);
        let works = |target: Target| match target {
            Target::Button(c) => BUTTONS.contains(&dialog.controls[c].id.unwrap_or(0)),
            _ => hub::on_bar(dialog, target).is_none_or(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        // An open menu takes the next click: on its entry, or anywhere else
        // to close it.
        if let Some(player) = self.menu_player.take() {
            let menu = control(dialog, MENU_ID);
            if let Some(ControlState::Menu(m)) = menu.map(|m| &mut view.controls[m].state) {
                m.at = None;
            }
            if matches!(target, Target::Option { control: c, item: 0 } if Some(c) == menu) {
                // The info pages through the rows backwards, as the list
                // box keeps them.
                let rows = control(dialog, LIST).and_then(|l| view.list(l));
                cx.state.paging = rows.map(|l| l.keys.iter().rev().copied().collect());
                self.popup = Some(playerinfo::CURRENT);
                return Outcome::Popup(Request::with(playerinfo::CURRENT, Args::Player(player)));
            }
            return Outcome::Stay;
        }
        if let Some(item) = hub::on_bar(dialog, target) {
            let outcome = hub::bar_click(cx, view, item, INDIVIDUAL_TRAINING_SYMBOL);
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        match target {
            Target::Cell { control: c, row, column } if id(c) == LIST => {
                let Some(list) = view.list(c) else { return Outcome::Stay };
                let Some(&player) = list.keys.get(list.scroll + row) else { return Outcome::Stay };
                let Some(club) = own(cx) else { return Outcome::Stay };
                let status = club.status.get(player).copied().unwrap_or_default();
                let Some(mut entry) = entry_of(club, status.id) else { return Outcome::Stay };
                match column {
                    NAME_COLUMN | WORKS_ON_COLUMN => {
                        self.choosing = Some(player);
                        self.popup = Some(ABILITY_CHOICE);
                        return Outcome::Popup(Request::with(
                            ABILITY_CHOICE,
                            Args::Record(status.id),
                        ));
                    }
                    INTENSITY_COLUMN => {
                        let long_out = status.injured >= LONG_OUT;
                        entry.intensity = cx.game.rules.individual.step(entry.intensity, long_out);
                    }
                    EXTRA_COLUMN => entry.extra = !entry.extra,
                    YOUTH_COLUMN => entry.youth = !entry.youth,
                    _ => return Outcome::Stay,
                }
                change(cx, |club| individual::keep(&mut club.individual, entry));
                fill(cx, view);
            }
            Target::Menu(Grip::Row { control: c, row }) if id(c) == LIST => {
                self.menu_player = open_menu(cx, view, c, row);
            }
            // A group's button: on with the left button, back with the
            // right; then everyone's intensity anew (`FUN_0074bd20`).
            Target::Button(c) | Target::Menu(Grip::Control(c)) if BUTTONS.contains(&id(c)) => {
                let back = matches!(target, Target::Menu(_));
                if let Some(button) = view.controls[c].state.as_button_mut() {
                    button.step_on(3, back);
                }
                let mut states = 0;
                for (group, id) in BUTTONS.into_iter().enumerate() {
                    let step = control(dialog, id)
                        .and_then(|c| view.controls[c].state.as_button())
                        .map_or(0, |b| b.step);
                    states = individual::with_state(states, group, step as u8);
                }
                reckon(cx, states);
                fill(cx, view);
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// The ability chosen; the player info goes on to another page; the
    /// whistle's question, whose yes ends the turn.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        let whistle = std::mem::take(&mut self.whistle);
        match (self.popup.take(), reply) {
            (Some(ABILITY_CHOICE), Reply::Choice(ability)) => {
                let player = self.choosing.take();
                let club = own(cx);
                let id = player.and_then(|p| club?.status.get(p)).map_or(0, |s| s.id);
                if let Some(mut entry) = club.and_then(|c| entry_of(c, id)) {
                    entry.ability = ability as i16;
                    change(cx, |club| individual::keep(&mut club.individual, entry));
                    fill(cx, view);
                }
                Outcome::Stay
            }
            (Some(page), Reply::Goto(next))
                if playerinfo::PAGES_IN_ORDER.contains(&page)
                    && playerinfo::PAGES_IN_ORDER.contains(&next) =>
            {
                self.popup = Some(next);
                Outcome::Popup(Request::new(next))
            }
            (None, Reply::Ok) if whistle => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// The squad, by position and then in its order: each player's name and
/// position, his abilities and what he works on, his progress, strength,
/// form, condition, freshness and intensity, the two flags
/// (`FUN_00595e86`).
fn fill(cx: &Cx, view: &mut View) {
    let (game, dialog) = (cx.game, cx.dialog);
    let Some(club) = own(cx) else { return };
    let pitch = &game.rules.pitch;
    let tables = &game.rules.individual;
    let order: Vec<usize> = POSITIONS
        .flat_map(|position| {
            (0..club.players.len()).filter(move |&p| club.players[p].positions[0] == position)
        })
        .collect();
    let (mut rows, mut icons, mut bars) = (Vec::new(), Vec::new(), Vec::new());
    for &p in &order {
        let player = &club.players[p];
        let status = club.status.get(p).copied().unwrap_or_default();
        let entry = entry_of(club, status.id).unwrap_or_else(|| default_entry(cx, club, p));
        let keeper = player.positions[0] == GOALKEEPER;
        let fitness = lineup::fitness_of(&club.fitness, p);
        let kind = pitch.abbreviations.get(usize::from(player.positions[0])).copied();
        let strength =
            if status.strength_hidden { "?".to_owned() } else { player.strength.to_string() };
        let mark = |on: bool| if on { MARK } else { "" };
        rows.push(format!(
            "{}\t{}\t\t\t{strength}\t{}\t{}\t{}\t{}\t{}\t{}",
            short_name(cx, player, status.id),
            game.text(kind.unwrap_or(0) as u16),
            lineup::form(fitness),
            lineup::condition(fitness),
            lineup::freshness(fitness),
            game.text(tables.intensity_text(entry.intensity)),
            mark(entry.extra),
            mark(entry.youth),
        ));
        let mut row_icons: Vec<(f32, u16)> =
            ABILITY_SLOTS.into_iter().zip(pitch.ability_icons(player)).collect();
        let works_on = usize::try_from(entry.ability).ok().filter(|_| entry.ability != NOTHING);
        let icon = match works_on {
            Some(ability) => pitch.ability_icon(keeper, true, ability),
            None => pitch.nothing_icon(),
        };
        row_icons.extend(icon.map(|icon| (WORKS_ON_SLOT, icon)));
        icons.push(row_icons);
        bars.push(works_on.map(|ability| Bar {
            left: BAR_LEFT,
            width: individual::bar_width(tables.difficulty(keeper, ability)),
            value: i32::from(entry.progress.get(ability).copied().unwrap_or(0)),
            fill: crate::controls::list::BAR_FILL,
        }));
    }
    let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) else { return };
    list.rows = rows;
    list.row_icons = icons;
    list.bars = bars;
    list.keys = order;
}

/// Every squad player's choices: kept, or as a new player gets them.
fn entries(cx: &Cx) -> Vec<Entry> {
    let Some(club) = own(cx) else { return Vec::new() };
    (0..club.players.len())
        .map(|p| {
            let id = club.status.get(p).map_or(0, |s| s.id);
            entry_of(club, id).unwrap_or_else(|| default_entry(cx, club, p))
        })
        .collect()
}

/// A player's kept choices.
fn entry_of(club: &ClubState, id: u16) -> Option<Entry> {
    club.individual.iter().find(|e| e.id == id && id != 0).copied()
}

/// The choices of a player who has none yet (`FUN_007467de`).
fn default_entry(cx: &Cx, club: &ClubState, p: usize) -> Entry {
    let player = &club.players[p];
    let status = club.status.get(p).copied().unwrap_or_default();
    let record = cx.state.records.as_ref().and_then(|r| r.player(status.id));
    let favourite = record.map_or(0, |r| r.favourite());
    let suspended = record.map_or(status.away_kind, |r| r.away_kind()) & SUSPENDED != 0;
    let intensity = individual::first_intensity(status.injured, suspended);
    let keeper = player.positions[0] == GOALKEEPER;
    Entry::new(status.id, keeper, player.abilities, favourite, intensity)
}

/// Everyone's intensity by the four buttons, kept with the states and in
/// the players' records; the choices of players who left go
/// (`FUN_0074bd20`).
fn reckon(cx: &mut Cx, states: u8) {
    let Some(club) = own(cx) else { return };
    let today = cx.state.season.as_ref().map_or(0, |s| s.day);
    let records = cx.state.records.as_ref();
    let mut entries = entries(cx);
    let mut intensities = Vec::new();
    for entry in &mut entries {
        let Some(p) = club.status.iter().position(|s| s.id == entry.id) else { continue };
        let status = club.status[p];
        let fitness = lineup::fitness_of(&club.fitness, p);
        let minutes = records.and_then(|r| r.player(entry.id)).map_or(0, |r| r.minutes());
        entry.intensity = individual::intensity(
            states,
            status.injured,
            minutes,
            today,
            lineup::condition(fitness),
            lineup::freshness(fitness),
        );
        intensities.push((entry.id, entry.intensity));
    }
    let squad: Vec<u16> = club.status.iter().map(|s| s.id).collect();
    change(cx, |club| {
        club.intensity_states = states;
        individual::purge(&mut club.individual, &squad);
        for entry in entries {
            individual::keep(&mut club.individual, entry);
        }
    });
    if let Some(records) = cx.state.records.as_mut() {
        for (id, intensity) in intensities {
            if let Some(record) = records.players.get_mut(usize::from(id)) {
                record.set_intensity(intensity as u8);
            }
        }
    }
}

/// Opens the menu of the player a right click fell on, there: only
/// "Spielerinfo" (`CListBoxSpielerInfo`); the player.
fn open_menu(cx: &mut Cx, view: &mut View, list: usize, row: usize) -> Option<usize> {
    let scroll = view.list(list).map_or(0, |l| l.scroll);
    let player = *view.list(list)?.keys.get(scroll + row)?;
    let target = Target::Row { control: list, row };
    let (_, rect) = crate::pointer::targets(cx.dialog, view, &crate::pointer::Pixels)
        .into_iter()
        .find(|(t, _)| *t == target)?;
    let menu = control(cx.dialog, MENU_ID)?;
    if let ControlState::Menu(m) = &mut view.controls[menu].state {
        m.at = Some(rect.center());
        m.entries = vec![cx.game.text(PLAYER_INFO).to_owned()];
    }
    Some(player)
}

/// The popup of the abilities a player lacks (dialog 365,
/// `CTrainIndAuswahl`), by name, each with its icon and progress, and a
/// last row for none; OK takes the one chosen.
#[derive(Default)]
pub struct AbilityChoice {
    chosen: Option<usize>,
}

impl Screen for AbilityChoice {
    fn ids(&self) -> &[u16] {
        &[ABILITY_CHOICE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.chosen = None;
        let Args::Record(id) = args else { return };
        let Some(club) = own(cx) else { return };
        let Some(p) = club.status.iter().position(|s| s.id == id) else { return };
        let player = &club.players[p];
        let keeper = player.positions[0] == GOALKEEPER;
        let entry = entry_of(club, id).unwrap_or_else(|| default_entry(cx, club, p));
        let name = cx
            .state
            .records
            .as_ref()
            .and_then(|r: &Records| r.known_as(id))
            .unwrap_or_else(|| format!("{} {}", player.first_name, player.last_name));
        if let Some(c) = control(dialog, QUESTION) {
            view.set_text(c, dialog.controls[c].text.replacen("%s", &name, 1));
        }
        let words = &game.rules.words;
        let names = if keeper { &words.keeper_abilities } else { &words.field_abilities };
        let mut abilities: Vec<(Vec<u8>, usize)> = (0..individual::abilities(keeper))
            .map(|a| {
                let text = game.text(names.get(a).copied().unwrap_or(0));
                (text.chars().map(|c| to_cp1252(c).unwrap_or(b'?')).collect(), a)
            })
            .collect();
        abilities.sort();
        let [good, bad] = player.abilities;
        let pitch = &game.rules.pitch;
        let tables = &game.rules.individual;
        let (mut rows, mut icons, mut bars, mut keys) = (vec![], vec![], vec![], vec![]);
        for (_, a) in abilities.into_iter().filter(|&(_, a)| good & (1 << a) == 0) {
            rows.push(format!("\t{}", game.text(names.get(a).copied().unwrap_or(0))));
            icons.push(
                pitch
                    .ability_icon(keeper, bad & (1 << a) == 0, a)
                    .map(|icon| (CHOICE_ICON, icon))
                    .into_iter()
                    .collect(),
            );
            bars.push(Some(Bar {
                left: CHOICE_BAR,
                width: individual::bar_width(tables.difficulty(keeper, a)),
                value: i32::from(entry.progress[a]),
                fill: crate::controls::list::BAR_FILL,
            }));
            keys.push(a);
        }
        rows.push(String::new());
        icons.push(pitch.nothing_icon().map(|icon| (CHOICE_ICON, icon)).into_iter().collect());
        bars.push(None);
        keys.push(NONE_ROW);
        if let Some(list) = control(dialog, ABILITIES).and_then(|c| view.list_mut(c)) {
            list.rows = rows;
            list.row_icons = icons;
            list.bars = bars;
            list.keys = keys;
            list.selected = None;
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Row { control: c, row } if dialog.controls[c].id == Some(ABILITIES) => {
                let list = view.list(c);
                self.chosen = list.and_then(|l| l.keys.get(l.scroll + row)).copied();
            }
            Target::Button(c) if dialog.controls[c].id == Some(CHOICE_OK) => {
                return Outcome::Close(match self.chosen {
                    Some(NONE_ROW) => Reply::Choice(NOTHING as usize),
                    Some(ability) => Reply::Choice(ability),
                    None => Reply::Cancel,
                });
            }
            Target::Button(c) if dialog.controls[c].id == Some(CHOICE_CANCEL) => {
                return Outcome::Close(Reply::Cancel);
            }
            _ => {}
        }
        Outcome::Stay
    }
}

/// The manager's club, as the season follows it.
fn own<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

/// Changes the club's individual training, and its records with it: the
/// store, the four buttons' states.
fn change(cx: &mut Cx, change: impl FnOnce(&mut ClubState)) {
    let game = cx.game;
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get_mut(&key) else { return };
    change(club);
    let (entries, states) = (club.individual.clone(), club.intensity_states);
    let Some(records) = state.records.as_mut() else { return };
    let Some(nation) = game.world.country(key.0).map(|c| c.nation) else { return };
    let Some(team) = u16::try_from(key.1).ok().and_then(|n| records.team_mut(nation, n)) else {
        return;
    };
    team.set_intensity_states(states);
    let human = usize::from(team.human()).checked_sub(1);
    if let Some(store) = human.and_then(|slot| records.individual.get_mut(slot)) {
        store.set_entries(&entries);
    }
}
