//! The classic look on the canvas: the dialog's frame as the classic
//! renderer draws it (see [`a3_screens::classic`]), shown at 2.25× and drawn
//! anew whenever the view, the caret or a blending animation changes.

use bevy::prelude::*;
use image::RgbaImage;

use a3_files::rch::Dialog;
use a3_screens::classic::Classic;

use super::{Caret, Frames, OpenDialog, View};
use crate::Game;
use crate::look::{SharpMaterial, show_frame, smooth_image, spawn_classic_layer};

/// Blending animations is drawn in this many steps per frame — 40 a second —
/// which is smooth enough while composing the screen stays cheap.
const BLEND_STEPS: f32 = 8.0;

#[derive(Resource)]
pub(super) struct ClassicDialog {
    id: u16,
    classic: Classic,
    screen: Handle<Image>,
    material: Handle<SharpMaterial>,
    /// The blending step last drawn.
    step: u32,
}

impl ClassicDialog {
    pub(super) fn id(&self) -> u16 {
        self.id
    }

    /// The screen as it was last drawn.
    pub(super) fn last_frame(&self, images: &Assets<Image>) -> Option<RgbaImage> {
        let image = images.get(&self.screen)?;
        let size = image.size();
        RgbaImage::from_raw(size.x, size.y, image.data.clone()?)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn(
    commands: &mut Commands,
    stage: Entity,
    dialog: &Dialog,
    view: &View,
    background: RgbaImage,
    game: &Game,
    images: &mut Assets<Image>,
    materials: &mut Assets<SharpMaterial>,
) {
    let classic = Classic::new(game, dialog, view, background);
    let screen = images.add(smooth_image(classic.draw(dialog, view, false, 0.0)));
    let material = spawn_classic_layer(commands, stage, materials, screen.clone());
    commands.insert_resource(ClassicDialog { id: view.id, classic, screen, material, step: 0 });
}

pub(super) fn redraw(
    classic: Option<ResMut<ClassicDialog>>,
    view: Res<OpenDialog>,
    caret: Res<Caret>,
    frames: Res<Frames>,
    game: Res<Game>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SharpMaterial>>,
) {
    let Some(mut classic) = classic.filter(|c| c.id == view.id) else { return };
    if view.is_changed()
        && let Some(dialog) = game.dialogs.get(&view.id)
    {
        classic.classic.update_with(&game, dialog, &view);
    }
    let step = (frames.phase() * BLEND_STEPS) as u32;
    let blended = frames.is_changed() && step != classic.step;
    if !(view.is_changed() || caret.is_changed() || blended) {
        return;
    }
    classic.step = step;
    let Some(dialog) = game.dialogs.get(&view.id) else { return };
    let frame = classic.classic.draw(dialog, &view, caret.on, step as f32 / BLEND_STEPS);
    show_frame(&mut images, &mut materials, &classic.screen, &classic.material, frame);
}
