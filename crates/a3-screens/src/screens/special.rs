//! The line-up's special variants (dialog 386, the game's
//! `CMannaufSpezial`): line-ups the game puts up by one measure, and the
//! players the manager wants always picked, or never — the lists mark them,
//! a click marks or unmarks one. Choosing a variant takes it back to the
//! line-up, which puts it up; OK goes back without one.

use std::collections::HashMap;

use a3_core::autolineup;
use a3_core::cycle::dialogs::NOTICE;
use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Request, Screen, control, ok_or_cancel, set};
use crate::controls::ControlState;
use crate::pointer::Target;
use crate::view::View;

pub const SPECIAL: u16 = 386;
const VARIANTS: u16 = 5330;
const ALWAYS: u16 = 5336;
const NEVER: u16 = 5337;
/// The automatic line-ups before a match, by the co-trainer: in a free
/// formation, or the old one; one or the other.
const AUTO_FREE: u16 = 5338;
const AUTO_OLD: u16 = 6355;
/// The caption, in the frame: the dialog's title.
const CAPTION_ID: u16 = 9922;
/// No more than eleven are always picked: text 4282.
const ELEVEN_ALWAYS: u16 = 4282;
const MOST_ALWAYS: usize = 11;

/// Puts the dialog's caption in its frame.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&SPECIAL) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct Special;

impl Screen for Special {
    fn ids(&self) -> &[u16] {
        &[SPECIAL]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let title = cx.dialog.title.clone();
        set(view, cx.dialog, CAPTION_ID, title);
        if let Some(variants) = control(cx.dialog, VARIANTS).and_then(|c| view.options_mut(c)) {
            variants.actions = true;
        }
        // The co-trainer's settings, which only a club with one has
        // (`CMannaufSpezial`'s OnInitDialog).
        let (flags, co_trainer) = club(cx).map_or((0, false), |c| (c.line_up_flags, c.co_trainer));
        for (id, bit) in [(AUTO_FREE, autolineup::AUTO_FREE), (AUTO_OLD, autolineup::AUTO_OLD)] {
            let Some(c) = control(cx.dialog, id) else { continue };
            if let Some(check) = view.check_mut(c) {
                check.checked = co_trainer && flags & bit != 0;
            }
            if !co_trainer {
                view.inert.push(Target::Check(c));
            }
        }
        show(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if ok_or_cancel(cx.dialog, target).0 {
            keep_settings(cx, view);
            return Outcome::Close(Reply::Ok);
        }
        match target {
            Target::Option { control: c, item } if cx.dialog.controls[c].id == Some(VARIANTS) => {
                keep_settings(cx, view);
                Outcome::Close(Reply::Choice(item))
            }
            Target::Row { control: c, row } => {
                let always = match cx.dialog.controls[c].id {
                    Some(ALWAYS) => true,
                    Some(NEVER) => false,
                    _ => return Outcome::Stay,
                };
                let scroll = view.list(c).map_or(0, |l| l.scroll);
                let outcome = mark(cx, scroll + row, always);
                show(cx, view);
                outcome
            }
            // Without a co-trainer the boxes do not answer.
            Target::Check(c) if view.inert.contains(&target) => {
                if let Some(check) = view.check_mut(c) {
                    check.checked = false;
                }
                Outcome::Stay
            }
            Target::Check(c) => {
                // Ticking one unticks the other.
                let other = match cx.dialog.controls[c].id {
                    Some(AUTO_FREE) => AUTO_OLD,
                    Some(AUTO_OLD) => AUTO_FREE,
                    _ => return Outcome::Stay,
                };
                let ticked =
                    matches!(&view.controls[c].state, ControlState::Check(check) if check.checked);
                if ticked
                    && let Some(o) = control(cx.dialog, other)
                    && let ControlState::Check(check) = &mut view.controls[o].state
                {
                    check.checked = false;
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

/// The club the dialog lines up.
fn club<'a>(cx: &'a Cx) -> Option<&'a a3_core::season::ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

/// The co-trainer's settings as the boxes have them, kept with the club and
/// in its record (`FUN_0063c846`).
fn keep_settings(cx: &mut Cx, view: &View) {
    let ticked = |id: u16| {
        control(cx.dialog, id).and_then(|c| view.check(c)).is_some_and(|check| check.checked)
    };
    let mut flags = 0;
    if ticked(AUTO_FREE) {
        flags |= autolineup::AUTO_FREE;
    }
    if ticked(AUTO_OLD) {
        flags |= autolineup::AUTO_OLD;
    }
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let current = season.current;
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get_mut(&key) else { return };
    // The other settings the word holds stay as they are.
    club.line_up_flags =
        club.line_up_flags & !(autolineup::AUTO_FREE | autolineup::AUTO_OLD) | flags;
    let word = club.line_up_flags;
    if let Some(records) = state.records.as_mut()
        && let Some(slot) = records.human_slot(current)
        && let Some(extra) = records.extras.get_mut(slot)
    {
        extra.set_line_up_flags(word);
    }
}

/// Marks a player "always" or "never", or unmarks him; marking one clears
/// the other, and no more than eleven are marked "always".
fn mark(cx: &mut Cx, player: usize, always: bool) -> Outcome {
    let Some(season) = cx.state.season.as_mut() else { return Outcome::Stay };
    let Some(club) = season.manager().and_then(|m| m.club) else { return Outcome::Stay };
    let Some(state) = season.clubs.get_mut(&club) else { return Outcome::Stay };
    if state.status.len() < state.players.len() {
        state.status.resize(state.players.len(), Default::default());
    }
    let marked = state.status.iter().filter(|s| s.always).count();
    let Some(status) = state.status.get_mut(player) else { return Outcome::Stay };
    if always {
        if !status.always && marked >= MOST_ALWAYS {
            return Outcome::Popup(Request::with(NOTICE, Args::Text(ELEVEN_ALWAYS)));
        }
        status.always = !status.always;
        if status.always {
            status.never = false;
        }
    } else {
        status.never = !status.never;
        if status.never {
            status.always = false;
        }
    }
    Outcome::Stay
}

/// Both lists: the squad, those marked shown so.
fn show(cx: &mut Cx, view: &mut View) {
    let (game, dialog) = (cx.game, cx.dialog);
    let Some(season) = &cx.state.season else { return };
    let Some(state) = season.manager().and_then(|m| m.club).and_then(|c| season.clubs.get(&c))
    else {
        return;
    };
    let pitch = &game.rules.pitch;
    let abbreviation = |kind: u8| {
        let id = pitch.abbreviations.get(usize::from(kind)).copied().unwrap_or(0);
        game.text(id as u16).to_owned()
    };
    let rows: Vec<String> = state
        .players
        .iter()
        .map(|p| {
            let name = format!("{} {}", p.first_name, p.last_name);
            format!("{name}\t{}\t{}", abbreviation(p.positions[0]), p.strength)
        })
        .collect();
    for (id, always) in [(ALWAYS, true), (NEVER, false)] {
        let Some(list) = control(dialog, id).and_then(|c| view.list_mut(c)) else { continue };
        // In the dialog file's columns.
        list.rows.clone_from(&rows);
        list.selected = None;
        list.marked = (0..rows.len())
            .map(|p| state.status.get(p).is_some_and(|s| if always { s.always } else { s.never }))
            .collect();
    }
}
