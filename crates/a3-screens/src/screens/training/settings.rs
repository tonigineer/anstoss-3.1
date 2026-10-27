//! The co-trainer's settings, the bitmap button 4417 in the long-term
//! training (dialog 391, `CTrainingCoTrainer`; TRAINING-LANG.md §6.1 in the
//! RE repo): how many days a week the co-trainer plans and how many units
//! over them, what he pays attention to on those units — nine sliders from
//! none to eight, over all of them no more than the units — and the box
//! that lets him plan by all of this by himself; also how heavily the
//! settings load the team, in percent.  OK takes everything, Abbruch
//! leaves the club as it was; with the box ticked and the settings changed,
//! the co-trainer plans the current week right away.

use std::collections::HashMap;

use a3_core::season::ClubState;
use a3_core::training::{CoTraining, CAMP_WEEK, DAYS};
use a3_files::rch::Dialog;

use super::super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use super::super::discipline::CAPTION_ID;
use crate::pointer::Target;
use crate::view::View;
use super::{change as change_training, change_week};

/// The dialog, which the long-term training opens.
pub(crate) const CO_TRAINER_SETTINGS: u16 = 391;

/// Controls by their ids in the dialog file.
/// The box for the co-trainer's automatic training.
const BOX: u16 = 4925;
/// The days of a week and the units over them, each with its arrows and
/// the number between them.
const DAYS_SPIN: u16 = 4796;
const DAYS_SHOWN: u16 = 4798;
const UNITS_SPIN: u16 = 4797;
const UNITS_SHOWN: u16 = 4799;
/// The nine training Schwerpunkte: their sliders and the numbers of the
/// units they get — condition, regeneration, offside trap, anti-offside,
/// duels, set pieces, dirty tricks, technique and formations.
const SLIDERS: [u16; 9] = [4786, 4787, 4788, 4789, 4790, 4791, 4792, 4793, 4794];
const SHOWN: [u16; 9] = [4800, 4801, 4802, 4803, 4804, 4805, 4806, 4807, 4808];
/// "Aktuelle Trainingsauslastung", in percent.
const LOAD: u16 = 4809;
/// OK and Abbruch.
const OK: u16 = 1;
const END: u16 = 2;

/// The week's days run from five to six; a Schwerunkt takes at most eight
/// units, and over all of them as many as the units of the week.
const FEWEST_DAYS: i32 = 5;
const MOST_DAYS: i32 = 6;
const HEAVIEST: i32 = 8;
/// The co-trainer plans the week himself (bit 0 of his settings).
const PLANS: u16 = 1;

/// What stands in the dialog while it is up: the club's settings, which OK
/// takes, and whether anything of them was touched.
#[derive(Default)]
pub struct CoTrainerSettings {
    plan: CoTraining,
    checked: bool,
    changed: bool,
}

impl Screen for CoTrainerSettings {
    fn ids(&self) -> &[u16] {
        &[CO_TRAINER_SETTINGS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        *self = Self::default();
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        let Some(club) = own(cx) else { return };
        self.plan = club.co_training;
        self.checked = club.line_up_flags & PLANS != 0;
        if let Some(boxed) = control(dialog, BOX).and_then(|c| view.check_mut(c)) {
            boxed.checked = self.checked;
        }
        self.sync(cx, view);
        let works = |target: Target| match target {
            Target::Button(c) => matches!(id(dialog, c), OK | END),
            Target::Check(c) => id(dialog, c) == BOX,
            Target::Spin { control: c, .. } => {
                [DAYS_SPIN, UNITS_SPIN].contains(&id(dialog, c))
            }
            Target::SliderStep { control: c, .. }
            | Target::SliderAt { control: c, .. }
            | Target::SliderTrack(c) => SLIDERS.contains(&id(dialog, c)),
            _ => false,
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| id(dialog, c);
        match target {
            // The days, five or six; fewer days cap the units again.
            Target::Spin { control: c, .. } if id(c) == DAYS_SPIN => {
                let days = view.value(c).map_or(FEWEST_DAYS, |v| v.value);
                self.plan.days = i16::try_from(days).unwrap_or(FEWEST_DAYS as i16);
                self.changed = true;
                self.sync(cx, view);
            }
            // The units, none to three a day; they cap the Schwerpunkte.
            Target::Spin { control: c, .. } if id(c) == UNITS_SPIN => {
                let units = view.value(c).map_or(0, |v| v.value);
                self.plan.units = i16::try_from(units).unwrap_or(0);
                self.changed = true;
                self.sync(cx, view);
            }
            // A Schwerunkt, its view already moved: none over the units
            // that the others of the nine left (`FUN_005a1b64`).
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. }
                if SLIDERS.contains(&id(c)) =>
            {
                let i = SLIDERS.iter().position(|&s| s == id(c)).unwrap_or(0);
                let left = self.allowed(i);
                if let Some(value) = view.value_mut(c) {
                    if value.value > left {
                        value.set(left);
                    }
                    self.plan.focus[i] = value.value.max(0) as u8;
                }
                self.changed = true;
                self.shown(cx, view);
            }
            Target::Check(c) if id(c) == BOX => {
                self.checked = view.check(c).is_some_and(|b| b.checked);
                self.changed = true;
            }
            // OK takes the settings and the box; with the box ticked and
            // something changed, the co-trainer plans the current week
            // himself, all of its sessions.  Abbruch takes nothing.
            Target::Button(c) if id(c) == OK => {
                let (plan, checked, changed) = (self.plan, self.checked, self.changed);
                change_co(cx, plan, checked);
                if changed && checked {
                    self.plan_now(cx);
                }
                return Outcome::Close(Reply::Ok);
            }
            Target::Button(c) if id(c) == END => return Outcome::Close(Reply::Cancel),
            _ => {}
        }
        Outcome::Stay
    }
}

impl CoTrainerSettings {
    /// The days, the units and the nine Schwerpunkte as they stand, into
    /// the dialog: the units' ceiling follows the days', each Schwerunkt's
    /// own the units and what their eight leaves, and the load with them.
    fn sync(&mut self, cx: &mut Cx, view: &mut View) {
        let dialog = cx.dialog;
        let days = i32::from(self.plan.days).clamp(FEWEST_DAYS, MOST_DAYS);
        self.plan.days = days as i16;
        let units = i32::from(self.plan.units).clamp(0, days * 3);
        self.plan.units = units as i16;
        if let Some(c) = control(dialog, DAYS_SPIN).and_then(|c| view.value_mut(c)) {
            c.range = (FEWEST_DAYS, MOST_DAYS);
            c.set(days);
        }
        if let Some(c) = control(dialog, UNITS_SPIN).and_then(|c| view.value_mut(c)) {
            c.range = (0, days * 3);
            c.set(units);
        }
        let heaviest = units.min(HEAVIEST);
        let mut left = units;
        for (i, &s) in SLIDERS.iter().enumerate() {
            let value = i32::from(self.plan.focus[i]).min(heaviest).min(left.max(0));
            self.plan.focus[i] = value as u8;
            left -= value;
            if let Some(c) = control(dialog, s).and_then(|c| view.value_mut(c)) {
                c.range = (0, heaviest);
                c.set(value);
            }
        }
        self.shown(cx, view);
    }

    /// The numbers, the days' and the units' and the nine, and the load's
    /// percent: half the units over the days.
    fn shown(&self, cx: &mut Cx, view: &mut View) {
        let (dialog, days, units) = (cx.dialog, i32::from(self.plan.days), i32::from(self.plan.units));
        set(view, dialog, DAYS_SHOWN, days.to_string());
        set(view, dialog, UNITS_SHOWN, units.to_string());
        let mut sum = 0;
        for (i, &s) in SHOWN.iter().enumerate() {
            let value = i32::from(self.plan.focus[i]);
            sum += value;
            set(view, dialog, s, value.to_string());
        }
        let load = if days == 0 { 0 } else { 50 * sum / days };
        set(view, dialog, LOAD, format!("{load}%"));
    }

    /// How many units the one Schwerunkt may hold: the heaviest, and the
    /// units the other eight stand on not already spent.
    fn allowed(&self, i: usize) -> i32 {
        let units = i32::from(self.plan.units);
        let others: i32 = self
            .plan
            .focus
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != i)
            .map(|(_, &f)| i32::from(f))
            .sum();
        units.min(HEAVIEST).min(units - others).max(0)
    }

    /// The current week, with no template of the manager's or a camp's
    /// own: the co-trainer plans it now, all of its sessions, by his
    /// kind's odds (`FUN_0074e0fb(club, 0)`).
    fn plan_now(&self, cx: &mut Cx) {
        let game = cx.game;
        let Some(club) = own(cx) else { return };
        if !club.co_trainer {
            return;
        }
        let Some(season) = cx.state.season.as_ref() else { return };
        let current = season.day / DAYS;
        let free = week_entry(cx, current)
            .is_none_or(|w| w.template <= 0 || w.template > CAMP_WEEK);
        if !free {
            return;
        }
        let records = cx.state.records.as_ref();
        let kind = own_key(cx)
            .and_then(|key| team_of(cx, key))
            .and_then(|t| records?.persons.get(usize::try_from(t.co_trainer()).ok()?))
            .map_or(0, |p| p.training_kind());
        let tables = &game.rules.training;
        let Some(draw) = cx.state.season.as_mut().map(|s| s.random.below(tables.co_draw())) else {
            return;
        };
        let template = tables.co_template(kind, draw);
        change_week(cx, current, |w| w.template = template as i16 + 1);
        let sessions = cx
            .state
            .season
            .as_ref()
            .and_then(|s| s.manager())
            .and_then(|m| m.templates.get(template))
            .map(|t| t.week);
        if let Some(sessions) = sessions {
            change_training(cx, |t| t.apply(0, &sessions, 0));
        }
    }
}

/// The window title, over the dialog's own.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&CO_TRAINER_SETTINGS) else { return };
    let text = dialog.title.clone();
    let caption = super::super::caption(dialog.frame, CAPTION_ID, text);
    dialog.controls.insert(0, caption);
}

fn id(dialog: &Dialog, c: usize) -> u16 {
    dialog.controls[c].id.unwrap_or(0)
}

/// The manager's club, as the season follows it, and its key.
fn own<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

fn own_key(cx: &Cx) -> Option<(usize, usize)> {
    cx.state.season.as_ref()?.manager()?.club
}

fn team_of<'a>(cx: &'a Cx, (slot, number): (usize, usize)) -> Option<&'a a3_core::records::Team> {
    let nation = cx.game.world.country(slot)?.nation;
    cx.state.records.as_ref()?.team(nation, u16::try_from(number).ok()?)
}

/// What the manager planned for a week of the season.
fn week_entry(cx: &Cx, week: usize) -> Option<a3_core::training::Week> {
    cx.state.season.as_ref()?.manager()?.weeks.get(week).cloned()
}

/// The settings and the box into the club, and his extra data with them
/// (`FUN_005a1a8e`, the OK's store).
fn change_co(cx: &mut Cx, plan: CoTraining, checked: bool) {
    let game = cx.game;
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get_mut(&key) else { return };
    club.co_training = plan;
    club.line_up_flags = club.line_up_flags & !PLANS | u16::from(checked);
    let flags = club.line_up_flags;
    let Some(records) = state.records.as_mut() else { return };
    let Some(nation) = game.world.country(key.0).map(|c| c.nation) else { return };
    let Some(team) = u16::try_from(key.1).ok().and_then(|n| records.team_mut(nation, n)) else {
        return;
    };
    let human = usize::from(team.human()).checked_sub(1);
    if let Some(extra) = human.and_then(|slot| records.extras.get_mut(slot)) {
        extra.set_line_up_flags(flags);
        extra.set_co_training_days(plan.days);
        extra.set_co_training_units(plan.units);
        extra.set_co_training_focus(plan.focus);
    }
}
