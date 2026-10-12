//! The intro: `videos/Intro.sks` played with its soundtrack. Any key but F9,
//! or any mouse button, skips it, as in the game.
//!
//! Classic shows the 320×240 frames with crisp pixels between pillarboxes.
//! Remastered smooths them and fills the 16:9 sides with a blurred, dimmed
//! copy of the picture, like an ambient light. All scaling happens on the GPU;
//! a frame costs three small uploads.

use bevy::prelude::*;
use image::imageops::{self, FilterType};
use image::{ImageFormat, RgbaImage};

use crate::look::{
    CANVAS, CLASSIC, SharpMaterial, canvas_rect, classic_rect, feather_sides, show_frame as show,
    smooth_image, spawn_classic_layer, spawn_stage,
};
use crate::sks::Sks;
use crate::{GameDir, Screen};

/// Soft edges of the remastered picture, in video pixels.
const FEATHER: u32 = 8;
const AMBIENT_SIZE: (u32, u32) = (16, 9);
const AMBIENT_TINT: Color = Color::srgb(0.55, 0.55, 0.55);

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Screen::Intro), start)
        .add_systems(Update, (show_frame, skip).run_if(in_state(Screen::Intro)))
        .add_systems(OnExit(Screen::Intro), |mut commands: Commands| {
            commands.remove_resource::<Intro>();
        });
}

#[derive(Resource)]
struct Intro {
    video: Sks,
    classic: Handle<Image>,
    material: Handle<SharpMaterial>,
    remastered: Handle<Image>,
    ambient: Handle<Image>,
    shown: usize,
    /// Fallback clock for when there is no audio device to follow.
    elapsed: f64,
}

#[derive(Component)]
struct Soundtrack;

/// The three pictures a frame is shown as, all scaled up on the GPU: as it
/// is for the classic look, with soft edges for the remastered one, and
/// shrunk to a few pixels for the ambient light around it.
struct Frame {
    classic: RgbaImage,
    remastered: RgbaImage,
    ambient: RgbaImage,
}

impl Frame {
    fn new(pixels: RgbaImage) -> Self {
        let mut remastered = pixels.clone();
        feather_sides(&mut remastered, FEATHER);
        let (w, h) = AMBIENT_SIZE;
        let ambient = imageops::resize(&pixels, w, h, FilterType::Triangle);
        Self { classic: pixels, remastered, ambient }
    }
}

fn start(
    mut commands: Commands,
    dir: Res<GameDir>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SharpMaterial>>,
    mut sounds: ResMut<Assets<AudioSource>>,
    mut next: ResMut<NextState<Screen>>,
) {
    let path = dir.0.join("videos/Intro.sks");
    let opened = Sks::open(&path).and_then(|video| {
        let first = decode(&video, 0)?;
        Ok((video, first))
    });
    let (video, first) = match opened {
        Ok(opened) => opened,
        Err(err) => {
            warn!("{}: {err}; skipping the intro", path.display());
            next.set(Screen::Dialog(crate::dialog::START));
            return;
        }
    };

    let frame = Frame::new(first);
    let (classic, remastered, ambient) = (
        images.add(smooth_image(frame.classic)),
        images.add(smooth_image(frame.remastered)),
        images.add(smooth_image(frame.ambient)),
    );
    let stage = spawn_stage(&mut commands, Screen::Intro);
    commands.spawn((
        ImageNode::new(ambient.clone()).with_color(AMBIENT_TINT),
        canvas_rect(0.0, 0.0, CANVAS.x, CANVAS.y),
        ChildOf(stage),
    ));
    commands.spawn((
        ImageNode::new(remastered.clone()),
        classic_rect(0.0, 0.0, CLASSIC.x, CLASSIC.y),
        ChildOf(stage),
    ));
    let material = spawn_classic_layer(&mut commands, stage, &mut materials, classic.clone());

    let soundtrack = sounds.add(AudioSource { bytes: video.soundtrack_wav().into() });
    commands.spawn((
        Soundtrack,
        AudioPlayer::new(soundtrack),
        PlaybackSettings::ONCE,
        DespawnOnExit(Screen::Intro),
    ));
    commands.insert_resource(Intro {
        video,
        classic,
        material,
        remastered,
        ambient,
        shown: 0,
        elapsed: 0.0,
    });
}

/// Shows the frame matching the soundtrack's position, so picture and sound
/// cannot drift apart, and moves on to the menu after the last frame.
fn show_frame(
    time: Res<Time>,
    mut intro: ResMut<Intro>,
    sink: Option<Single<&AudioSink, With<Soundtrack>>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SharpMaterial>>,
    mut next: ResMut<NextState<Screen>>,
) {
    intro.elapsed += time.delta_secs_f64();
    let clock = sink.map_or(intro.elapsed, |sink| sink.position().as_secs_f64());
    let frame = (clock * f64::from(intro.video.fps)) as usize;
    if frame >= intro.video.frame_count() {
        next.set(Screen::Dialog(crate::dialog::START));
        return;
    }
    if frame == intro.shown {
        return;
    }
    intro.shown = frame;

    let pixels = match decode(&intro.video, frame) {
        Ok(pixels) => pixels,
        Err(err) => return warn!("intro frame {frame}: {err}"),
    };
    let Frame { classic, remastered, ambient } = Frame::new(pixels);
    show(&mut images, &mut materials, &intro.classic, &intro.material, classic);
    for (handle, frame) in [(&intro.remastered, remastered), (&intro.ambient, ambient)] {
        if let Some(mut image) = images.get_mut(handle) {
            image.data = Some(frame.into_raw());
        }
    }
}

fn decode(video: &Sks, frame: usize) -> std::io::Result<RgbaImage> {
    let pixels = image::load_from_memory_with_format(video.frame_jpeg(frame), ImageFormat::Jpeg)
        .map_err(std::io::Error::other)?
        .to_rgba8();
    if pixels.dimensions() != (video.width, video.height) {
        return Err(std::io::Error::other(format!(
            "frame is {}×{}, not {}×{}",
            pixels.width(),
            pixels.height(),
            video.width,
            video.height
        )));
    }
    Ok(pixels)
}

fn skip(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut next: ResMut<NextState<Screen>>,
) {
    let key = keys.get_just_pressed().any(|&key| key != KeyCode::F9);
    if key || mouse.get_just_pressed().next().is_some() {
        next.set(Screen::Dialog(crate::dialog::START));
    }
}
