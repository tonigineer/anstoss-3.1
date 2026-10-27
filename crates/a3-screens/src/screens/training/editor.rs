//! The week templates' editor (dialog 350, `CTrainingswochenErstellung`),
//! from a right click on a template of the short training: the seven lists
//! of units above, the week's 21 sessions below — the one to set in red —,
//! the templates on a combo and the name in an edit. A unit clicked goes
//! into the session in red, which moves on to the next (`FUN_005a4904`);
//! a session clicked is the next to set (`FUN_005a47af`). Another template
//! on the combo keeps the one shown (`OnSelChange…_005a426d`), OK keeps it
//! and ends (`FUN_005a3f33`). The unit's video the original plays beside
//! the lists is not shown.

use std::collections::HashMap;

use a3_core::training::Template;
use a3_files::rch::{Align, Control, Dialog, Kind, PLAIN_FONT};
use bevy_math::Rect;

use super::{FIRST_UNIT, UNIT_LIST, UNIT_ROW, UNIT_SIZE};
use crate::screens::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

pub const TEMPLATE_EDITOR: u16 = 350;
/// The 21 sessions, by day from Sunday and session of the day.
const SESSIONS: [[u16; 3]; 7] = [
    [4477, 4478, 4480],
    [4479, 4481, 4483],
    [4482, 4484, 4486],
    [4485, 4487, 4489],
    [4488, 4490, 4492],
    [4491, 4493, 4495],
    [4494, 4496, 4497],
];
const NAME: u16 = 4499;
const TEMPLATES: u16 = 5651;
const CAPTION_ID: u16 = 9944;
/// The name OK keeps, at most (`FUN_00402e60(0x1d)`).
const NAME_MOST: usize = 0x1d;
const RED: [u8; 3] = [255, 0, 0];

/// Where the editor puts its seven unit lists (`0x88ba64`); the popup has
/// places of its own.
const LISTS_AT: [(f32, f32); 7] = [
    (17.0, 52.0),
    (133.0, 52.0),
    (249.0, 52.0),
    (249.0, 152.0),
    (17.0, 196.0),
    (133.0, 196.0),
    (249.0, 196.0),
];

/// The caption, and the unit lists the dialog makes as it opens
/// (`FUN_005a37d6`): statics of 100×13 down each list.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &a3_core::season::Rules) {
    let Some(dialog) = dialogs.get_mut(&TEMPLATE_EDITOR) else { return };
    let caption = crate::screens::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
    for ((c, category), &(x, y)) in rules.training.categories.iter().enumerate().zip(&LISTS_AT) {
        let top = |i: usize| y + UNIT_ROW * i as f32 + (i / 2) as f32;
        let count = category.units.len();
        for i in 0..count {
            let next = if i + 1 < count { top(i + 1) } else { f32::MAX };
            let bottom = (top(i) + UNIT_SIZE.1).min(next);
            dialog.controls.push(Control {
                kind: Kind::Static,
                rect: Rect::new(x, top(i), x + UNIT_SIZE.0, bottom),
                align: Align::Centre,
                text: String::new(),
                id: Some(FIRST_UNIT + c as u16 * UNIT_LIST + i as u16),
                font: PLAIN_FONT,
                ink: [0; 3],
            });
        }
    }
}

#[derive(Default)]
pub struct TemplateEditor {
    /// The template shown, a copy of it, the session to set and the unit
    /// last clicked.
    index: usize,
    template: Option<Template>,
    at: (usize, usize),
    unit: Option<usize>,
}

impl TemplateEditor {
    fn templates(cx: &Cx) -> Vec<Template> {
        cx.state.season.as_ref().and_then(|s| s.manager()).map_or_else(Vec::new, |m| {
            m.templates.clone()
        })
    }

    /// The shown template with the edit's name, into the manager and his
    /// record (`memcpy` to mgr+0x38a).
    fn keep(&mut self, cx: &mut Cx, view: &View) {
        let Some(mut template) = self.template.clone() else { return };
        if let Some(edit) = control(cx.dialog, NAME) {
            template.name = view.text(edit).chars().take(NAME_MOST).collect();
        }
        let state = &mut *cx.state;
        let Some(season) = state.season.as_mut() else { return };
        let current = season.current;
        let Some(manager) = season.managers.get_mut(current) else { return };
        if let Some(slot) = manager.templates.get_mut(self.index) {
            *slot = template;
        }
        let block = a3_core::training::week_file(&manager.templates);
        if let Some(records) = state.records.as_mut()
            && let Some(slot) = records.human_slot(current)
            && let Some(record) = records.managers.get_mut(slot)
        {
            record.set_templates(&block);
        }
    }

    /// Another template into the dialog: its name and its sessions.
    fn load(&mut self, cx: &Cx, view: &mut View, index: usize) {
        self.index = index;
        self.template = Self::templates(cx).get(index).cloned();
        let name = self.template.as_ref().map(|t| t.name.clone()).unwrap_or_default();
        set(view, cx.dialog, NAME, name);
        self.show_sessions(cx, view);
    }

    fn show_sessions(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(template) = &self.template else { return };
        for (day, ids) in SESSIONS.iter().enumerate() {
            for (session, &id) in ids.iter().enumerate() {
                let Some(c) = control(dialog, id) else { continue };
                let unit = template.week[day * 3 + session];
                let text = game.text(game.rules.training.unit_text(unit)).to_owned();
                if let Some(shown) = view.static_mut(c) {
                    shown.text = text;
                    shown.clickable = true;
                    shown.ink = ((day, session) == self.at).then_some(RED);
                }
            }
        }
    }

    fn mark_unit(&self, cx: &Cx, view: &mut View) {
        for (c, category) in cx.game.rules.training.categories.iter().enumerate() {
            for i in 0..category.units.len() {
                let id = FIRST_UNIT + c as u16 * UNIT_LIST + i as u16;
                let Some(control) = control(cx.dialog, id) else { continue };
                if let Some(shown) = view.static_mut(control) {
                    shown.ink = (self.unit == Some(c * 100 + i)).then_some(RED);
                }
            }
        }
    }
}

impl Screen for TemplateEditor {
    fn ids(&self) -> &[u16] {
        &[TEMPLATE_EDITOR]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let Args::Template(index) = args else { return };
        let (game, dialog) = (cx.game, cx.dialog);
        self.at = (0, 0);
        self.unit = None;
        for (c, category) in game.rules.training.categories.iter().enumerate() {
            for (i, &(text_unit, _)) in category.units.iter().enumerate() {
                let id = FIRST_UNIT + c as u16 * UNIT_LIST + i as u16;
                let Some(control) = control(dialog, id) else { continue };
                view.set_text(control, game.text(game.rules.training.unit_text(text_unit)));
                view.set_clickable(control, true);
            }
        }
        let names: Vec<String> = Self::templates(cx).iter().map(|t| t.name.clone()).collect();
        if let Some(combo) = control(dialog, TEMPLATES).and_then(|c| view.combo_mut(c)) {
            combo.items = names;
            combo.selected = Some(index);
        }
        if let Some(edit) = control(dialog, NAME) {
            view.focus = Some(edit);
        }
        self.load(cx, view, index);
        self.mark_unit(cx, view);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Row { control: c, .. } if id(c) == Some(TEMPLATES) => {
                let chosen = view.combo(c).and_then(|combo| combo.selected);
                if let Some(index) = chosen.filter(|&i| i != self.index) {
                    self.keep(cx, view);
                    self.load(cx, view, index);
                }
            }
            Target::Control(c) => {
                let Some(id) = id(c) else { return Outcome::Stay };
                if let Some(day) = SESSIONS.iter().position(|ids| ids.contains(&id)) {
                    let session = SESSIONS[day].iter().position(|&s| s == id).unwrap_or(0);
                    self.at = (day, session);
                    self.show_sessions(cx, view);
                    return Outcome::Stay;
                }
                let Some(unit_id) = id.checked_sub(FIRST_UNIT) else { return Outcome::Stay };
                let (c, i) = (usize::from(unit_id / UNIT_LIST), usize::from(unit_id % UNIT_LIST));
                let categories = &cx.game.rules.training.categories;
                let Some(&(_, unit)) = categories.get(c).and_then(|c| c.units.get(i)) else {
                    return Outcome::Stay;
                };
                let (day, session) = self.at;
                if let Some(template) = self.template.as_mut() {
                    template.week[day * 3 + session] = unit;
                }
                // On to the next session, Saturday's last round to Sunday.
                self.at = if session < 2 { (day, session + 1) } else { ((day + 1) % 7, 0) };
                self.unit = Some(c * 100 + i);
                self.show_sessions(cx, view);
                self.mark_unit(cx, view);
            }
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                self.keep(cx, view);
                return Outcome::Close(Reply::Ok);
            }
            _ => {}
        }
        Outcome::Stay
    }
}
