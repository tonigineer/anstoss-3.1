//! The frame rate the application is rendering at, in the window's corner.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

const REFRESH_SECS: f32 = 0.25;

pub fn plugin(app: &mut App) {
    app.add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_systems(Startup, spawn)
        .add_systems(Update, update);
}

/// The frame rate's text, which screenshots of the screen leave out.
#[derive(Component)]
pub struct FpsText;

fn spawn(mut commands: Commands) {
    commands.spawn((
        FpsText,
        Text::new("– fps"),
        TextFont { font_size: FontSize::Px(18.0), ..default() },
        TextColor(Color::WHITE),
        BackgroundColor(Color::BLACK.with_alpha(0.55)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        GlobalZIndex(100),
        Pickable::IGNORE,
    ));
}

/// Shows the smoothed frame rate, a few times a second so it stays readable.
fn update(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut since: Local<f32>,
    mut text: Single<&mut Text, With<FpsText>>,
) {
    *since += time.delta_secs();
    if *since < REFRESH_SECS {
        return;
    }
    *since = 0.0;
    if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed())
    {
        text.0 = format!("{fps:.0} fps");
    }
}
