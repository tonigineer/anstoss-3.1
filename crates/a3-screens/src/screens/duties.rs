//! Who takes the set pieces and who is captain, the game's
//! `CAufgabenverteilung` (dialog 418), which "Aufgaben im Spiel" of the
//! line-up opens: a tab per duty, the squad ranked for it — the manager's
//! order first ([`a3_core::duties`]) — with each player's position,
//! abilities, strength, injury and, for penalties and free kicks, his
//! attempts and goals this season, and the club's below. A click on a
//! player puts him first. Leaving a tab keeps its order, OK the open tab's;
//! a captain deposed costs the old one some of his contentment with the
//! trainer. A right click on a player offers his info.

use std::collections::HashMap;

use a3_core::duties::{self, Duty};
use a3_core::mood;
use a3_core::season::ClubState;
use a3_core::world::Player;
use a3_files::rch::{Align, Control, Dialog, Kind, PLAIN_FONT};
use bevy_math::Rect;

use super::{Args, Cx, Outcome, Reply, Request, Screen, control, ok_or_cancel, playerinfo};
use crate::controls::ControlState;
use crate::pointer::{Grip, Target};
use crate::view::View;

pub const TAKERS: u16 = 418;
const LIST: u16 = 5339;
const TABS: u16 = 5340;
const CLUB_LINE: u16 = 5341;
/// The caption, at the top: the dialog's title.
const CAPTION_ID: u16 = 9925;
/// The menu a right click on a player opens: "Spielerinfo" (text 4036),
/// the one entry of `CListBoxSpielerInfo`'s.
const MENU_ID: u16 = 9951;
const PLAYER_INFO: u16 = 4036;

/// The duties by the tabs' order.
const TAB_DUTIES: [Duty; 6] = [
    Duty::CrossFreeKick,
    Duty::CornerLeft,
    Duty::Penalty,
    Duty::ShotFreeKick,
    Duty::CornerRight,
    Duty::Captain,
];
/// What a deposed captain loses of his contentment with the trainer.
const DEPOSED: i32 = -30;

/// Where a row shows the player's abilities, between the columns the
/// dialog file sets (rank, name, position — strength, weeks injured,
/// attempts, goals).
const ICON_COLUMNS: [f32; 3] = [180.0, 200.0, 220.0];
const WEEK: i32 = 7;

/// Puts the dialog's caption in its frame, and the menu over everything.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&TAKERS) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
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

#[derive(Default)]
pub struct Takers {
    /// The open tab, and its list as the dialog shows it.
    tab: usize,
    list: Vec<usize>,
    /// Who led the captain's list when the dialog opened.
    captain: Option<usize>,
    /// The player whose menu is open; the page of his info that is open.
    menu_player: Option<usize>,
    popup: Option<u16>,
}

impl Screen for Takers {
    fn ids(&self) -> &[u16] {
        &[TAKERS]
    }

    /// Opened from the line-up on the first tab, any tab to choose.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let title = cx.dialog.title.clone();
        super::set(view, cx.dialog, CAPTION_ID, title);
        *self = Self::default();
        self.captain = club(cx).and_then(|c| c.duties[Duty::Captain.stored()].first().copied());
        self.load(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        // An open menu takes the next click: on its entry, or anywhere else
        // to close it.
        if let Some(player) = self.menu_player.take() {
            let menu = control(cx.dialog, MENU_ID);
            if let Some(ControlState::Menu(m)) = menu.map(|m| &mut view.controls[m].state) {
                m.at = None;
            }
            if matches!(target, Target::Option { control: c, item: 0 } if Some(c) == menu) {
                self.popup = Some(playerinfo::CURRENT);
                return Outcome::Popup(Request::with(playerinfo::CURRENT, Args::Player(player)));
            }
            return Outcome::Stay;
        }
        if ok_or_cancel(cx.dialog, target).0 {
            self.keep(cx);
            self.depose(cx);
            return Outcome::Close(Reply::Ok);
        }
        match target {
            Target::Option { control: c, item } if cx.dialog.controls[c].id == Some(TABS) => {
                if item != self.tab && item < TAB_DUTIES.len() {
                    self.keep(cx);
                    self.tab = item;
                    self.load(cx, view);
                }
                Outcome::Stay
            }
            // The player clicked goes first; those above him one down.
            Target::Row { control: c, row } if cx.dialog.controls[c].id == Some(LIST) => {
                let scroll = view.list(c).map_or(0, |l| l.scroll);
                if scroll + row < self.list.len() {
                    let player = self.list.remove(scroll + row);
                    self.list.insert(0, player);
                }
                self.show(cx, view);
                Outcome::Stay
            }
            Target::Menu(grip) => {
                self.menu_player = self.open_menu(cx, view, grip);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }

    /// The player info goes on to another of its pages.
    fn answer(&mut self, _: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        match (self.popup.take(), reply) {
            (Some(page), Reply::Goto(next))
                if playerinfo::PAGES_IN_ORDER.contains(&page)
                    && playerinfo::PAGES_IN_ORDER.contains(&next) =>
            {
                self.popup = Some(next);
                Outcome::Popup(Request::new(next))
            }
            _ => Outcome::Stay,
        }
    }
}

impl Takers {
    fn duty(&self) -> Duty {
        TAB_DUTIES[self.tab]
    }

    /// The open tab's list, put in order as the dialog does: the captain's
    /// by its own rule, which may clear everyone's flag against being
    /// captain (`FUN_00477f9b`).
    fn load(&mut self, cx: &mut Cx, view: &mut View) {
        if let Some(tabs) = control(cx.dialog, TABS).and_then(|c| view.options_mut(c)) {
            tabs.chosen = self.tab;
        }
        let duty = self.duty();
        let Some(club) = club(cx) else { return };
        let stored = &club.duties[duty.stored()];
        self.list = if duty == Duty::Captain {
            let (list, clear) = duties::normalise_captain(&club.players, &club.status, stored);
            if clear {
                clear_no_captain(cx);
            }
            list
        } else {
            duties::normalise(&club.players, stored)
        };
        self.show(cx, view);
    }

    /// The open tab's list kept with the club and in its record
    /// (`FUN_00477ebd`).
    fn keep(&self, cx: &mut Cx) {
        let duty = self.duty();
        let state = &mut *cx.state;
        let Some(season) = state.season.as_mut() else { return };
        let Some(key) = season.manager().and_then(|m| m.club) else { return };
        let Some(club) = season.clubs.get_mut(&key) else { return };
        club.duties[duty.stored()].clone_from(&self.list);
        let ids: Vec<u16> = club.status.iter().map(|s| s.id).collect();
        let Some(records) = state.records.as_mut() else { return };
        let Some(nation) = cx.game.world.country(key.0).map(|c| c.nation) else { return };
        let Some(team) = u16::try_from(key.1).ok().and_then(|n| records.team(nation, n)) else {
            return;
        };
        let Some(slot) = usize::from(team.human()).checked_sub(1) else { return };
        if let Some(extra) = records.extras.get_mut(slot) {
            let mut lists = extra.duty_lists();
            lists[duty.stored()] = duties::to_ids(&ids, &self.list);
            extra.set_duty_lists(lists);
        }
    }

    /// A new captain: the one who led the list before, still in the squad,
    /// takes it ill — from the season's first day on, and if he may be
    /// captain at all (the dialog's OK).
    fn depose(&self, cx: &mut Cx) {
        let Some(old) = self.captain else { return };
        let Some(club) = club(cx) else { return };
        if club.duties[Duty::Captain.stored()].first() == Some(&old) {
            return;
        }
        let Some(status) = club.status.get(old).copied() else { return };
        let Some(records) = cx.state.records.as_mut() else { return };
        if records.globals.day() == 0 || status.no_captain {
            return;
        }
        if let Some(r) = records.players.get_mut(usize::from(status.id)).filter(|_| status.id != 0)
        {
            mood::change_trainer(r, DEPOSED);
        }
        super::discipline::refresh_morale(cx);
    }

    /// The rows: rank, name, position, abilities, strength, weeks injured,
    /// and attempts and goals of penalties or free kicks; the club's line.
    fn show(&self, cx: &mut Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(season) = cx.state.season.as_ref() else { return };
        let Some(key) = season.manager().and_then(|m| m.club) else { return };
        let Some(club) = season.clubs.get(&key) else { return };
        let records = cx.state.records.as_ref();
        let pitch = &game.rules.pitch;
        let duty = self.duty();
        let counted = |p: usize| -> Option<[u8; 2]> {
            let r = records?.player(club.status.get(p)?.id)?;
            match duty {
                Duty::Penalty => Some(r.penalties()),
                Duty::CrossFreeKick | Duty::ShotFreeKick => Some(r.free_kicks()),
                _ => None,
            }
        };
        let mut rows = Vec::new();
        let mut icons = Vec::new();
        for (rank, &p) in self.list.iter().enumerate() {
            let player = &club.players[p];
            let status = club.status.get(p).copied().unwrap_or_default();
            let kind = pitch.abbreviations.get(usize::from(player.positions[0])).copied();
            let strength = if status.strength_unknown {
                "?".to_owned()
            } else {
                let fitness = a3_core::lineup::fitness_of(&club.fitness, p);
                tenths(pitch.strength(player, fitness, status.doping))
            };
            let out = if status.injured >= WEEK {
                (status.injured / WEEK).to_string()
            } else {
                String::new()
            };
            let [attempts, goals] = match duty {
                Duty::Penalty | Duty::CrossFreeKick | Duty::ShotFreeKick => {
                    counted(p).unwrap_or_default().map(|n| n.to_string())
                }
                _ => ["-".to_owned(), "-".to_owned()],
            };
            let name = short_name(cx, player, status.id);
            let position = game.text(kind.unwrap_or(0) as u16);
            rows.push(
                [(rank + 1).to_string(), name, position.to_owned(), strength, out, attempts, goals]
                    .join("\t"),
            );
            icons.push(ICON_COLUMNS.into_iter().zip(pitch.ability_icons(player)).collect());
        }
        if let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) {
            list.inks = vec![None; rows.len()];
            list.rows = rows;
            list.row_icons = icons;
            list.selected = None;
        }

        // The club's: penalties awarded and scored, or direct free kicks
        // taken and scored, missed, and the share scored.
        let name = game.world.country(key.0).and_then(|c| c.clubs.get(key.1)).map(|c| &c.name);
        let name = name.cloned().unwrap_or_default();
        let team = records.and_then(|r| {
            let nation = game.world.country(key.0)?.nation;
            r.team(nation, u16::try_from(key.1).ok()?)
        });
        let [awarded, scored] = match (duty, team) {
            (Duty::Penalty, Some(team)) => team.penalties().map(u32::from),
            (Duty::CrossFreeKick | Duty::ShotFreeKick, Some(team)) => {
                team.free_kicks().map(u32::from)
            }
            _ => [0, 0],
        };
        let line = if awarded == 0 {
            format!("{name}\\t-\\t-\\t-\\t-")
        } else {
            let share = f64::from(scored) * 100.0 / f64::from(awarded);
            let missed = i64::from(awarded) - i64::from(scored);
            let share = playerinfo::one_decimal(share);
            format!("{name}\\t{awarded}\\t{scored}\\t{missed}\\t{share}%")
        };
        super::set(view, dialog, CLUB_LINE, line);
    }

    /// Opens the menu of the player a right click fell on, there; the player.
    fn open_menu(&self, cx: &mut Cx, view: &mut View, grip: Grip) -> Option<usize> {
        let Grip::Row { control: c, row } = grip else { return None };
        (cx.dialog.controls.get(c)?.id? == LIST).then_some(())?;
        let scroll = view.list(c).map_or(0, |l| l.scroll);
        let player = *self.list.get(scroll + row)?;
        let target = Target::Row { control: c, row };
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
}

/// The current manager's club as the season follows it.
fn club<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

/// Nobody is barred from being captain any more (`FUN_007075ac(0)` on
/// the whole squad).
fn clear_no_captain(cx: &mut Cx) {
    const NO_CAPTAIN: u8 = 0x40;
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get_mut(&key) else { return };
    for status in &mut club.status {
        status.no_captain = false;
        if let Some(r) =
            state.records.as_mut().and_then(|r| r.players.get_mut(usize::from(status.id)))
            && status.id != 0
        {
            r.set_manner(r.manner() & !NO_CAPTAIN);
        }
    }
}

/// "Name, F.", or the nickname he goes by (`FUN_006fec36`).
pub(crate) fn short_name(cx: &Cx, player: &Player, id: u16) -> String {
    const NICKNAME: u32 = 0x8000_0000;
    let records = cx.state.records.as_ref();
    if let Some(r) = records.and_then(|records| records.player(id))
        && r.shown() & NICKNAME != 0
    {
        return records.map_or_else(String::new, |records| records.name(r.nickname()).to_owned());
    }
    match player.first_name.chars().next() {
        Some(initial) => format!("{}, {initial}.", player.last_name),
        None => player.last_name.clone(),
    }
}

/// Tenths as the game prints them: "7.3".
fn tenths(value: i32) -> String {
    format!("{:.1}", value as f32 / 10.0)
}
