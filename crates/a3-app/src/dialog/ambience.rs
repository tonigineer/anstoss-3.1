//! The sounds a screen loops while it is open — the grounds' ambience —,
//! each at the volume the screen asks for.

use std::collections::HashMap;

use bevy::audio::Volume;
use bevy::prelude::*;

use super::OpenDialog;
use crate::Game;

/// A looping sound, by its file.
#[derive(Component)]
pub(super) struct Ambience(&'static str);

/// Starts the sounds the open screen asks for, sets their volumes, stops
/// those it no longer does.
pub(super) fn play(
    mut commands: Commands,
    view: Res<OpenDialog>,
    game: Res<Game>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut playing: Query<(Entity, &Ambience, Option<&mut AudioSink>)>,
    mut loaded: Local<HashMap<&'static str, Option<Handle<AudioSource>>>>,
) {
    let volume = |v: u8| Volume::Linear(f32::from(v) / 100.0);
    let mut started = Vec::new();
    for (entity, sound, sink) in &mut playing {
        match view.ambience.iter().find(|(file, _)| *file == sound.0) {
            Some(&(_, v)) => {
                if let Some(mut sink) = sink {
                    sink.set_volume(volume(v));
                }
                started.push(sound.0);
            }
            None => commands.entity(entity).despawn(),
        }
    }
    for &(file, v) in view.ambience.iter().filter(|(file, _)| !started.contains(file)) {
        let handle = loaded.entry(file).or_insert_with(|| match game.file(file) {
            Ok(bytes) => Some(audio.add(AudioSource { bytes: bytes.into() })),
            Err(err) => {
                warn!("{file}: {err}");
                None
            }
        });
        if let Some(handle) = handle {
            let settings = PlaybackSettings::LOOP.with_volume(volume(v));
            commands.spawn((Ambience(file), AudioPlayer::new(handle.clone()), settings));
        }
    }
}
