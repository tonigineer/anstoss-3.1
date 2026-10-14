//! The formations a manager chooses from (dialog 244, the game's
//! `CSpielsystemeEditieren`): his nine on buttons, the chosen one's spots on
//! a pitch, each named by the position it makes, and OK to stand the
//! line-up in it — every player on the spot of his place in the new one.

use std::collections::HashMap;

use bevy_math::{Rect, Vec2};
use tracing::info;

use a3_core::lineup::FIELD;
use a3_core::season::Rules;
use a3_files::rch::{Align, Control, Dialog, Kind};

use super::{Args, Cx, Outcome, Reply, Screen, control, set};
use crate::controls::player_box::PlayerBox;
use crate::pointer::Target;
use crate::view::View;

pub const FORMATIONS: u16 = 244;
const IDOK: u16 = 1;
const IDCANCEL: u16 = 2;
/// The nine formations, and the buttons to restore, save and load them.
const CHOICES: u16 = 3386;
const FILES: u16 = 3387;
/// The caption, in the frame: "ANSTOSS 3 - %s %s %s (%s). %s" with
/// "Spielsysteme", the manager's names and his club.
const CAPTION_ID: u16 = 9921;
const CAPTION_KIND: u16 = 4361;
/// The boxes on the pitch, and where its spots start, in the dialog.
const BOXES: u16 = 9940;
const PITCH: Vec2 = Vec2::new(11.0, 22.0);
const BOX_SIZE: Vec2 = Vec2::new(99.0, 24.0);

/// Puts the dialog together: its caption and a box for every spot.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&FORMATIONS) else { return };
    let origin = dialog.frame.map_or(Vec2::ZERO, |f| f.min);
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    let spots = rules.pitch.formations.first().map_or([(0, 0); FIELD], |f| f.1);
    for (i, (x, y)) in spots.into_iter().enumerate() {
        let at = origin + PITCH + Vec2::new(f32::from(x), f32::from(y));
        dialog.controls.push(Control {
            kind: Kind::Player,
            rect: Rect::from_corners(at, at + BOX_SIZE),
            align: Align::Left,
            text: String::new(),
            id: Some(BOXES + i as u16),
            font: a3_files::rch::PLAIN_FONT,
            ink: [0; 3],
        });
    }
}

/// The formations dialog, on the formation it was opened with.
#[derive(Default)]
pub struct Formations {
    chosen: usize,
}

impl Screen for Formations {
    fn ids(&self) -> &[u16] {
        &[FORMATIONS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let Some(season) = &cx.state.season else { return };
        let Some(manager) = season.manager() else { return };
        set(view, cx.dialog, CAPTION_ID, super::hub::club_title(cx.game, season, CAPTION_KIND));
        self.chosen = manager.club.and_then(|c| season.clubs.get(&c)).map_or(0, |s| s.formation);
        if let Some(choices) = control(cx.dialog, CHOICES).and_then(|c| view.options_mut(c)) {
            for (label, (name, _)) in choices.labels.iter_mut().zip(&manager.formations) {
                label.clone_from(name);
            }
            choices.actions = false;
            choices.chosen = self.chosen;
        }
        if let Some(files) = control(cx.dialog, FILES).and_then(|c| view.options_mut(c)) {
            files.actions = true;
        }
        // Loading, saving and the original formations are not built yet.
        let dialog = cx.dialog;
        super::mark_inert(
            view,
            dialog,
            |t| !matches!(t, Target::Option { control: c, .. } if dialog.controls[c].id == Some(FILES)),
        );
        self.show(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        // Its OK is a mascot of its own, so the buttons go by their ids.
        if let Target::Button(b) = target {
            return match cx.dialog.controls[b].id {
                Some(IDOK) => Outcome::Close(Reply::Choice(self.chosen)),
                Some(IDCANCEL) => Outcome::Close(Reply::Cancel),
                _ => Outcome::Stay,
            };
        }
        let Target::Option { control: c, item } = target else { return Outcome::Stay };
        match cx.dialog.controls[c].id {
            Some(CHOICES) => {
                self.chosen = item;
                self.show(cx, view);
            }
            Some(FILES) => {
                let label = view.options(c).and_then(|o| o.labels.get(item)).cloned();
                info!("{}: not built yet", label.unwrap_or_default());
            }
            _ => {}
        }
        Outcome::Stay
    }
}

impl Formations {
    /// The chosen formation's spots, each box named by the position it
    /// makes.
    fn show(&self, cx: &mut Cx, view: &mut View) {
        let Some(manager) = cx.state.season.as_ref().and_then(|s| s.manager()) else { return };
        let Some((_, spots)) = manager.formations.get(self.chosen) else { return };
        let pitch = &cx.game.rules.pitch;
        let origin = cx.dialog.frame.map_or(Vec2::ZERO, |f| f.min) + PITCH;
        for (i, &(x, y)) in spots.iter().enumerate() {
            let Some(c) = control(cx.dialog, BOXES + i as u16) else { continue };
            let position = pitch.position((x, y));
            let name = pitch.position_names.get(usize::from(position)).copied().unwrap_or(0);
            view.set_player(
                c,
                PlayerBox {
                    name: cx.game.text(name as u16).to_owned(),
                    at: Some(origin + Vec2::new(f32::from(x), f32::from(y))),
                    ..PlayerBox::default()
                },
            );
        }
    }
}
