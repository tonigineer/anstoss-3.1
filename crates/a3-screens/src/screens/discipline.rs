//! What the line-up's menu does to a player besides showing him: a special
//! bonus (dialog 195, `CSondersiegpraemieAusschuetten`), a fine, a
//! suspension and a cut of the team's bonuses (dialog 85,
//! `CGeldstrafeAussprechen`), with what follows: the player's reply
//! (dialogs 86 to 88) or how his contentment changed (dialog 392,
//! `CEreignis1Wert`). The records change as in [`a3_core::discipline`].

use std::collections::HashMap;

use bevy_math::{Rect, Vec2};

use a3_core::cycle::Request;
use a3_core::discipline::{self, Club, Fined, PLAYER_UNIT, TEAM_UNIT};
use a3_core::mood::LAST_REAL_ID;
use a3_files::rch::{Align, Control, Dialog, Kind};

use super::popups::{NOTICE, QUESTION, ball};
use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel, set};
use crate::pointer::Target;
use crate::view::View;

pub const BONUS: u16 = 195;
pub const FINE: u16 = 85;
pub const REFORMED: u16 = 86;
pub const ALREADY: u16 = 87;
pub const INNOCENT: u16 = 88;
pub const CHANGE: u16 = 392;

const BONUS_AMOUNT: u16 = 2941;
const BONUS_SLIDER: u16 = 2942;
const BONUS_PLAYERS: u16 = 2945;
/// Mannschaft or Spieler.
const BONUS_FOR: u16 = 2947;
const BONUS_MOST: i32 = 280;

const FINE_PLAYERS: u16 = 1866;
const FINE_SLIDER: u16 = 1865;
const FINE_AMOUNT: u16 = 1863;
const FINE_IT: u16 = 1867;
const SUSPEND: u16 = 1868;
const CUTS: u16 = 6196;
const FINE_MOST: i32 = 500;

/// Both sliders start here.
const START: i32 = 100;

const REPLY_TEXTS: [(u16, u16); 3] = [(REFORMED, 1869), (ALREADY, 1871), (INNOCENT, 1873)];
const CHANGE_TEXT: u16 = 4949;
const BEFORE: u16 = 4945;
const AFTER: u16 = 4947;
const WHAT: u16 = 4950;
const COLUMN: u16 = 5331;
/// "Trainer": what changed.
const TRAINER: u16 = 4771;

/// Texts of the game.
const SUSPENDED_ALREADY: u16 = 2248;
const SUSPEND_REALLY: u16 = 5322;
const SUSPENDED_ANGRY: u16 = 5323;

/// The caption a popup draws at its top, its title.
const CAPTION_ID: u16 = 9924;
const CAPTION_MARGIN: f32 = 5.0;
const CAPTION_TOP: f32 = 3.0;
const CAPTION_BOTTOM: f32 = 19.0;

/// Puts the titles of the popups this module shows, the small notices' and
/// the week planner's in their captions.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let screen = Rect::new(0.0, 0.0, 640.0, 480.0);
    let ids = [
        BONUS,
        FINE,
        REFORMED,
        ALREADY,
        INNOCENT,
        CHANGE,
        NOTICE,
        QUESTION,
        super::planner::PLANNER,
    ];
    for id in ids {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        let frame = dialog.frame.unwrap_or(screen);
        let caption = Control {
            kind: Kind::Static,
            rect: Rect::from_corners(
                frame.min + Vec2::new(CAPTION_MARGIN, CAPTION_TOP),
                Vec2::new(frame.max.x - CAPTION_MARGIN, frame.min.y + CAPTION_BOTTOM),
            ),
            align: Align::Left,
            text: dialog.title.clone(),
            id: Some(CAPTION_ID),
            bold: true,
            ink: [0; 3],
        };
        dialog.controls.insert(0, caption);
    }
}

/// The human manager's club as the records have it, his slot and the ids of
/// his squad.
fn club(cx: &Cx) -> Option<(Club, usize, Vec<u16>)> {
    let season = cx.state.season.as_ref()?;
    let records = cx.state.records.as_ref()?;
    let (slot, number) = season.manager()?.club?;
    let country = cx.game.world.country(slot)?.nation;
    let club = Club { country, number: u16::try_from(number).ok()? };
    let squad = records.team(club.country, club.number)?.players();
    Some((club, records.human_slot(season.current)?, squad))
}

/// The record id of the player at `p` of the club's squad.
fn record_id(cx: &Cx, p: usize) -> Option<u16> {
    let season = cx.state.season.as_ref()?;
    let club = season.clubs.get(&season.manager()?.club?)?;
    club.status.get(p).map(|s| s.id).filter(|&id| id != 0)
}

/// Fills a combo box with the players' names, the one with `chosen`
/// chosen.
fn players(cx: &Cx, view: &mut View, id: u16, ids: &[u16], chosen: Option<u16>) {
    let records = cx.state.records.as_ref();
    let names = ids.iter().map(|&id| records.and_then(|r| r.known_as(id)).unwrap_or_default());
    if let Some(combo) = control(cx.dialog, id).and_then(|c| view.combo_mut(c)) {
        combo.items = names.collect();
        combo.selected = chosen.and_then(|c| ids.iter().position(|&id| id == c));
    }
}

/// The player a combo box has chosen.
fn chosen(cx: &Cx, view: &View, id: u16, ids: &[u16]) -> Option<u16> {
    let selected = control(cx.dialog, id).and_then(|c| view.combo(c))?.selected?;
    ids.get(selected).copied()
}

fn slider(cx: &Cx, view: &mut View, id: u16, value: i32, most: i32) {
    if let Some(slider) = control(cx.dialog, id).and_then(|c| view.value_mut(c)) {
        slider.range = (0, most);
        slider.set(value);
    }
}

fn slider_at(cx: &Cx, view: &View, id: u16) -> i32 {
    control(cx.dialog, id).and_then(|c| view.value(c)).map_or(0, |v| v.value)
}

fn is(cx: &Cx, target: Target, id: u16) -> bool {
    let c = match target {
        Target::Button(c)
        | Target::Combo(c)
        | Target::SliderTrack(c)
        | Target::Option { control: c, .. }
        | Target::Row { control: c, .. }
        | Target::SliderStep { control: c, .. }
        | Target::SliderAt { control: c, .. } => c,
        _ => return false,
    };
    cx.dialog.controls[c].id == Some(id)
}

/// Shows the line-up the morale the records now have.
fn refresh_morale(cx: &mut Cx) {
    let (Some(season), Some(records)) = (cx.state.season.as_mut(), cx.state.records.as_ref())
    else {
        return;
    };
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get_mut(&key) else { return };
    for (p, status) in club.status.iter().enumerate() {
        if let (Some(fitness), Some(r)) = (club.fitness.get_mut(p), records.player(status.id)) {
            fitness.morale = r.morale().into();
        }
    }
}

/// A special bonus for one player or the whole team.
#[derive(Default)]
pub struct Bonus {
    squad: Vec<u16>,
    for_player: bool,
    /// The team's amount and the player's, in their units.
    amounts: [i32; 2],
    /// A notice is up; the bonus closes after it.
    told: bool,
}

impl Bonus {
    fn show_amount(&self, cx: &Cx, view: &mut View) {
        let money = if self.for_player {
            i64::from(self.amounts[1]) * PLAYER_UNIT
        } else {
            i64::from(self.amounts[0]) * TEAM_UNIT
        };
        set(view, cx.dialog, BONUS_AMOUNT, super::playerinfo::money(cx.game, money));
    }
}

impl Screen for Bonus {
    fn ids(&self) -> &[u16] {
        &[BONUS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        set(view, cx.dialog, CAPTION_ID, cx.dialog.title.clone());
        let id = match args {
            Args::Player(p) => record_id(cx, p).filter(|&id| id <= LAST_REAL_ID),
            _ => None,
        };
        let squad = club(cx).map_or_else(Vec::new, |(_, _, squad)| squad);
        players(cx, view, BONUS_PLAYERS, &squad, id);
        let start = cx.game.rules.discipline.bonus_amount(START);
        *self = Self { squad, for_player: true, amounts: [start, start], told: false };
        slider(cx, view, BONUS_SLIDER, START, BONUS_MOST);
        if let Some(c) = control(cx.dialog, BONUS_FOR) {
            view.set_chosen(c, usize::from(id.is_some()));
        }
        self.show_amount(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if cancel {
            return Outcome::Close(Reply::Cancel);
        }
        if ok {
            return self.pay(cx, view);
        }
        let tables = &cx.game.rules.discipline;
        if is(cx, target, BONUS_SLIDER) {
            let position = slider_at(cx, view, BONUS_SLIDER);
            self.amounts[usize::from(self.for_player)] = tables.bonus_amount(position);
            self.show_amount(cx, view);
        } else if let Target::Option { item, .. } = target
            && is(cx, target, BONUS_FOR)
        {
            self.for_player = item == 1;
            let position = tables.bonus_position(self.amounts[item.min(1)]);
            slider(cx, view, BONUS_SLIDER, position, BONUS_MOST);
            self.show_amount(cx, view);
        }
        Outcome::Stay
    }

    fn answer(&mut self, _: &mut Cx, _: &mut View, _: Reply) -> Outcome {
        if self.told { Outcome::Close(Reply::Ok) } else { Outcome::Stay }
    }
}

impl Bonus {
    fn pay(&mut self, cx: &mut Cx, view: &View) -> Outcome {
        let player = chosen(cx, view, BONUS_PLAYERS, &self.squad);
        let Some((club, slot, _)) = club(cx) else { return Outcome::Close(Reply::Ok) };
        let tables = &cx.game.rules.discipline;
        let (Some(records), Some(season)) = (cx.state.records.as_mut(), cx.state.season.as_mut())
        else {
            return Outcome::Close(Reply::Ok);
        };
        let random = &mut season.random;
        let text = if self.for_player {
            let units = self.amounts[1];
            discipline::bonus_for_player(
                tables,
                records,
                random,
                club,
                slot,
                player.unwrap_or(0),
                units,
            )
        } else {
            discipline::bonus_for_team(tables, records, random, club, self.amounts[0])
        };
        refresh_morale(cx);
        match text {
            Some(text) => {
                self.told = true;
                Outcome::Popup(Request::with(NOTICE, Args::Text(text)))
            }
            None => Outcome::Close(Reply::Ok),
        }
    }
}

/// What the fine dialog waits for.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum Waiting {
    #[default]
    Nothing,
    /// The answer whether to suspend him.
    Suspension,
}

/// A fine, a suspension, a cut of the team's bonuses.
#[derive(Default)]
pub struct Fine {
    squad: Vec<u16>,
    /// What to show next; the dialog closes once they are shown.
    queue: Vec<Request>,
    waiting: Waiting,
    closing: bool,
}

impl Fine {
    fn amount(cx: &Cx, view: &View) -> i64 {
        let position = slider_at(cx, view, FINE_SLIDER);
        i64::from(cx.game.rules.discipline.fine_amount(position)) * PLAYER_UNIT
    }

    /// The cut of the team's bonuses as the options have it, and what the
    /// team says to it.
    fn cut(&mut self, cx: &mut Cx, view: &View) {
        let cut = control(cx.dialog, CUTS).map_or(0, |c| view.chosen(c)) as i16;
        let Some((club, _, _)) = club(cx) else { return };
        let Some(records) = cx.state.records.as_mut() else { return };
        if let Some(text) = discipline::cut_bonuses(records, club, cut) {
            self.queue.push(Request::with(NOTICE, Args::Text(text)));
        }
    }

    /// The next popup to show, or closes.
    fn next(&mut self) -> Outcome {
        if self.queue.is_empty() {
            return if self.closing { Outcome::Close(Reply::Ok) } else { Outcome::Stay };
        }
        Outcome::Popup(self.queue.remove(0))
    }

    fn fine(&mut self, cx: &mut Cx, view: &View, player: u16) {
        let amount = Self::amount(cx, view);
        let Some((club, _, _)) = club(cx) else { return };
        let tables = &cx.game.rules.discipline;
        let (Some(records), Some(season)) = (cx.state.records.as_mut(), cx.state.season.as_mut())
        else {
            return;
        };
        let fined = discipline::fine(tables, records, &mut season.random, club, player, amount);
        let request = match fined {
            Fined::Innocent => Some(Request::with(INNOCENT, Args::Record(player))),
            Fined::Already => Some(Request::with(ALREADY, Args::Record(player))),
            Fined::Reformed => Some(Request::with(REFORMED, Args::Record(player))),
            Fined::Nothing => None,
            Fined::Accepts { before, after } if before != after => Some(Request::with(
                CHANGE,
                Args::Change {
                    text: discipline::ACCEPTS,
                    player,
                    before: before as i16,
                    after: after as i16,
                },
            )),
            Fined::Accepts { .. } => {
                Some(Request::with(NOTICE, Args::TextFor(discipline::ACCEPTS, player)))
            }
        };
        self.queue.extend(request);
        refresh_morale(cx);
    }
}

impl Screen for Fine {
    fn ids(&self) -> &[u16] {
        &[FINE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        set(view, cx.dialog, CAPTION_ID, cx.dialog.title.clone());
        // The managers who play cannot be fined.
        let squad: Vec<u16> = club(cx)
            .map_or_else(Vec::new, |(_, _, squad)| squad)
            .into_iter()
            .filter(|&id| id <= LAST_REAL_ID)
            .collect();
        let id = match args {
            Args::Player(p) => record_id(cx, p),
            _ => None,
        };
        let id = id.filter(|id| squad.contains(id)).or(squad.first().copied());
        players(cx, view, FINE_PLAYERS, &squad, id);
        slider(cx, view, FINE_SLIDER, START, FINE_MOST);
        let amount = Self::amount(cx, view);
        set(view, cx.dialog, FINE_AMOUNT, super::playerinfo::money(cx.game, amount));
        let cut = match (club(cx), cx.state.records.as_ref()) {
            (Some((club, _, _)), Some(records)) => discipline::bonus_cut(records, club),
            _ => 0,
        };
        if let Some(c) = control(cx.dialog, CUTS) {
            view.set_chosen(c, usize::try_from(cut).unwrap_or(0).min(2));
        }
        *self = Self { squad, ..Self::default() };
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if cancel {
            return Outcome::Close(Reply::Cancel);
        }
        if is(cx, target, FINE_SLIDER) {
            let amount = Self::amount(cx, view);
            set(view, cx.dialog, FINE_AMOUNT, super::playerinfo::money(cx.game, amount));
            return Outcome::Stay;
        }
        let player = chosen(cx, view, FINE_PLAYERS, &self.squad);
        if ok {
            self.cut(cx, view);
        } else if is(cx, target, FINE_IT) {
            self.cut(cx, view);
            if let Some(player) = player {
                self.fine(cx, view, player);
            }
        } else if is(cx, target, SUSPEND) {
            self.cut(cx, view);
            let Some(player) = player else { return Outcome::Stay };
            let records = cx.state.records.as_ref();
            if records.and_then(|r| r.player(player)).is_some_and(discipline::suspended) {
                self.queue.push(Request::with(NOTICE, Args::Text(SUSPENDED_ALREADY)));
                if let Some(records) = cx.state.records.as_mut() {
                    discipline::mark_dealt_with(records, player);
                }
            } else {
                self.queue.push(Request::with(QUESTION, Args::Text(SUSPEND_REALLY)));
                self.waiting = Waiting::Suspension;
            }
        } else {
            return Outcome::Stay;
        }
        self.closing = true;
        self.next()
    }

    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if self.waiting == Waiting::Suspension {
            self.waiting = Waiting::Nothing;
            let player = chosen(cx, view, FINE_PLAYERS, &self.squad);
            if let (Some(player), Some(records)) = (player, cx.state.records.as_mut()) {
                if reply == Reply::Ok {
                    discipline::suspend(records, player);
                    self.queue.push(Request::with(NOTICE, Args::Text(SUSPENDED_ANGRY)));
                }
                discipline::mark_dealt_with(records, player);
            }
            refresh_morale(cx);
        }
        self.next()
    }
}

/// The player's reply to a fine: he takes it to heart, he has had one this
/// week, he did nothing.
pub struct FineReply;

impl Screen for FineReply {
    fn ids(&self) -> &[u16] {
        &[REFORMED, ALREADY, INNOCENT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        set(view, cx.dialog, CAPTION_ID, cx.dialog.title.clone());
        ball(view, cx);
        let Args::Record(player) = args else { return };
        let name = cx.state.records.as_ref().and_then(|r| r.known_as(player)).unwrap_or_default();
        let Some(&(_, id)) = REPLY_TEXTS.iter().find(|(dialog, _)| *dialog == view.id) else {
            return;
        };
        let template =
            control(cx.dialog, id).map_or_else(String::new, |c| cx.dialog.controls[c].text.clone());
        set(view, cx.dialog, id, template.replacen("%s", &name, 1));
    }

    fn input(&mut self, _: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(_) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// How an event changed a player's contentment with the trainer: a text, the
/// value before and after, and a bar of the new one.
pub struct Change;

/// The bar's pictures: red below 80, yellow below 100, green.
const COLUMN_PICTURES: [(i32, u16); 3] = [(80, 2654), (100, 2653), (i32::MAX, 2655)];
const COLUMN_MOST: i32 = 120;

impl Screen for Change {
    fn ids(&self) -> &[u16] {
        &[CHANGE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let Args::Change { text, player, before, after } = args else { return };
        let records = cx.state.records.as_ref();
        let season = cx.state.season.as_ref();
        let manager = season.and_then(|s| s.manager());
        let who = manager.map_or_else(String::new, |m| format!("{} {}", m.first_name, m.last_name));
        let club = manager
            .and_then(|m| m.club)
            .and_then(|(slot, c)| cx.game.world.country(slot)?.clubs.get(c))
            .map_or_else(String::new, |c| c.name.clone());
        set(view, cx.dialog, CAPTION_ID, format!("{} {who} ({club})", cx.dialog.title));
        let name = records.and_then(|r| r.known_as(player)).unwrap_or_default();
        set(view, cx.dialog, CHANGE_TEXT, cx.game.text(text).replacen("%s", &name, 1));
        set(view, cx.dialog, BEFORE, before.to_string());
        set(view, cx.dialog, AFTER, after.to_string());
        set(view, cx.dialog, WHAT, cx.game.text(TRAINER).to_owned());
        let Some(c) = control(cx.dialog, COLUMN) else { return };
        let rect = cx.dialog.controls[c].rect;
        let shown = i32::from(after).clamp(0, COLUMN_MOST);
        let height = ((rect.height() as i32 - 1) * shown / COLUMN_MOST).max(0) as u32;
        let graphic =
            COLUMN_PICTURES.iter().find(|(below, _)| shown < *below).map_or(0, |&(_, g)| g);
        if let crate::controls::ControlState::Bitmap(b) = &mut view.controls[c].state {
            b.picture = (height > 0).then(|| crate::controls::bitmap::Picture {
                source: crate::controls::bitmap::PictureSource::Graphic(graphic),
                part: Some([0, 0, rect.width() as u32, height]),
                black_clear: false,
            });
            b.bottom = true;
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        if ok_or_cancel(cx.dialog, target).0 {
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}
