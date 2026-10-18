//! Dialog screens, drawn from the game's own dialog definitions in both looks.
//!
//! The controls of a dialog come from `MeineDialoge.rch` (see [`a3_files::rch`]);
//! what they show and where their buttons lead is decided by the game's
//! screens, which the [`Host`] keeps open. Each control keeps its state in
//! the open dialog's [`View`], input lands on invisible hotspots above both
//! looks, and the classic and remastered renderers both draw from that one
//! state — which is what lets F9 switch looks mid-dialog.

mod backdrop;
mod classic;
mod decor;
mod decorations;
mod layout;
mod remastered;

use std::error::Error;
use std::path::PathBuf;

use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseWheel;
use bevy::picking::pointer::{PointerAction, PointerInput};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use image::{Rgba, RgbaImage};

use a3_files::rch::{Dialog, Kind};
use a3_screens::classic::PLAIN;
use a3_screens::controls::button::{cancel_button, ok_button};
use a3_screens::controls::slider;
use a3_screens::host::{Change, Host};
use a3_screens::pointer::{Grip, Target, targets};
use a3_screens::screens::hub;
use a3_screens::view::View;

use crate::Game;
use crate::look::{CANVAS, Look, SharpMaterial, canvas_rect, smooth_image, spawn_stage};
use crate::{Fonts, Screen};

/// Animations move on to their next frame every 200 ms, as the game sets
/// them up; in between they blend into it, which the game did not.
const FRAME_SECS: f32 = 0.2;
/// Held arrows repeat, as the game's scrollbars do: first after 300 ms,
/// then every 80 ms.
const REPEAT_DELAY: f32 = 0.3;
const REPEAT_SECS: f32 = 0.08;
/// The sound of a button, and of the whistle that ends a turn.
const CLICK: &str = "sounds/click.wav";
const WHISTLE: &str = "sounds/Leiste16.wav";
/// In the remastered look a popup dims the screen behind it to this much,
/// and casts a shadow this wide.
const POPUP_DIM: f32 = 0.55;
const POPUP_SHADOW: u32 = 28;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (load_sounds, start_game))
        .add_systems(Update, receive.run_if(not(in_state(Screen::Intro))))
        .init_resource::<Pending>()
        .init_resource::<Caret>()
        .init_resource::<Frames>()
        .init_resource::<Held>()
        .init_resource::<Resting>()
        .init_resource::<Underlay>()
        .add_systems(
            Update,
            (
                keyboard,
                wheel,
                release,
                repeat,
                tick,
                act,
                rest,
                blink,
                animate,
                classic::redraw,
                remastered::fill.run_if(remastered::drawn),
                remastered::refresh.run_if(remastered::drawn),
                remastered::show_bitmaps.run_if(remastered::drawn),
                remastered::animate_flags.run_if(remastered::drawn),
                remastered::drag_card,
            )
                .chain()
                .run_if(resource_exists::<OpenDialog>)
                .run_if(in_dialog),
        )
        .add_systems(
            Update,
            (
                (
                    remastered::ease_options,
                    remastered::ease_boxes,
                    remastered::ease_buttons,
                    remastered::ease_pushes,
                    remastered::ease_bar,
                )
                    .run_if(remastered::drawn),
                switch_hotspots,
            )
                .run_if(resource_exists::<OpenDialog>)
                .run_if(in_dialog),
        );
    for id in Host::dialogs() {
        app.add_systems(OnEnter(Screen::Dialog(id)), open);
    }
}

/// Adds the dialogs the game puts together from others, like the week screen
/// with a panel per club, and the controls it makes itself, like the menu
/// bar of the main screens.
pub fn compose(Game(game): &mut Game) {
    a3_screens::screens::compose(game);
}

/// Writes the pictures the remastered look draws on, with masks of what to
/// repaint, to `<cache>/sources` for `tools/outpaint.py`.
pub fn export_backgrounds(game: &Game) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    backdrop::export(game)
}

fn in_dialog(screen: Res<State<Screen>>) -> bool {
    matches!(screen.get(), Screen::Dialog(_))
}

/// The game's screens as a resource; once the game cycle has asked for the
/// first, it shows as the view of the screen on top.
#[derive(Resource)]
struct OpenDialog(Host);

impl std::ops::Deref for OpenDialog {
    type Target = View;

    fn deref(&self) -> &View {
        self.0.view()
    }
}

impl std::ops::DerefMut for OpenDialog {
    fn deref_mut(&mut self) -> &mut View {
        self.0.view_mut()
    }
}

/// The game cycle starts with the app; the intro plays while it asks for its
/// first screen.
fn start_game(mut commands: Commands, game: Res<Game>) {
    commands.insert_resource(OpenDialog(Host::start(&game, Box::new(clock_seed))));
}

/// The game seeds its generator from the clock as it starts; the game's
/// logic itself never reads the clock.
fn clock_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |t| t.as_nanos() as u64)
}

/// How far playing animations have come towards their next frame.
#[derive(Resource, Default)]
struct Frames {
    timer: f32,
}

impl Frames {
    /// From 0 at a frame to 1 at the next.
    fn phase(&self) -> f32 {
        (self.timer / FRAME_SECS).clamp(0.0, 1.0)
    }
}

/// What the pressed pointer holds on to: an arrow that repeats, a slider
/// handle being dragged — from where it was, at how many canvas pixels to a
/// classic one — or a player's box or a list's row, to be let go of on
/// another.
#[derive(Resource, Default)]
struct Held {
    repeat: Option<Target>,
    timer: f32,
    drag: Option<(usize, f32, f32)>,
    grip: Option<Grip>,
    /// Where the pointer took hold of the box or row, in classic pixels, at
    /// how many canvas pixels to a classic one.
    grab: Option<(Vec2, f32)>,
}

/// How long the pointer has rested on what it points at, for its tooltip.
#[derive(Resource, Default)]
struct Resting {
    target: Option<Target>,
    timer: f32,
}

/// A tooltip shows once the pointer has rested this long on a symbol of the
/// menu bar, as Windows shows them.
const TIP_SECS: f32 = 0.8;

/// Clicks waiting to be acted on, with how many came in a row.
#[derive(Resource, Default)]
struct Pending(Vec<(Target, u8)>);

/// Whether a text field's caret is showing; it blinks twice a second.
#[derive(Resource, Default)]
struct Caret {
    on: bool,
    timer: f32,
}

/// An invisible area that catches the pointer for one of the looks, and how
/// many canvas pixels across a classic pixel is there. It answers only while
/// its dialog is the one open: the next can open a frame before the screen
/// changes.
#[derive(Component)]
struct Hotspot {
    target: Target,
    zoom: f32,
    dialog: u16,
}

/// Which look a hotspot belongs to: the remastered one, or the classic.
#[derive(Component)]
struct ForLook {
    remastered: bool,
}

#[allow(clippy::too_many_arguments)]
fn open(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    mut next: ResMut<NextState<Screen>>,
    game: Res<Game>,
    host: Res<OpenDialog>,
    fonts: Res<Fonts>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SharpMaterial>>,
    mut look: ResMut<Look>,
    under: Option<Res<classic::ClassicDialog>>,
    underlay: Res<Underlay>,
) {
    let Screen::Dialog(id) = *screen.get() else { return };
    let Some(dialog) = game.dialogs.get(&id) else {
        return warn!("dialog {id} is not in the game's dialog file");
    };
    // Another screen may have come up since this one was entered.
    if host.id != id {
        return next.set(Screen::Dialog(host.id));
    }
    let view: &View = &host;
    debug!("dialog {id}");
    // A screen without a remastered look shows the classic one in both.
    let remastered = remastered::offers(id);
    if look.offered() != remastered {
        look.offer(remastered);
    }
    let picture = a3_screens::classic::picture(&game, dialog);
    let wide = remastered.then(|| {
        // The remastered look frames popups its own way.
        let art = a3_screens::classic::art(&game, dialog);
        match dialog.frame {
            Some(frame) => wide_popup(&game, under.as_deref(), &underlay, frame, &art),
            None => backdrop::load(id, &art),
        }
    });
    let background = match dialog.frame {
        Some(frame) => classic_popup(under.as_deref(), &images, frame, &picture),
        None => picture,
    };

    let stage = spawn_stage(&mut commands, Screen::Dialog(id));
    let layout = wide.map(|wide| {
        remastered::spawn(
            &mut commands,
            stage,
            dialog,
            view,
            &background,
            wide,
            &game,
            &fonts,
            &mut images,
        )
    });
    classic::spawn(
        &mut commands,
        stage,
        dialog,
        view,
        background,
        &game,
        &mut images,
        &mut materials,
    );
    let cross = images.add(cross());
    spawn_hotspots(&mut commands, stage, dialog, view, &layout::Classic, false, &look, &cross);
    match layout {
        Some(layout) => {
            spawn_hotspots(&mut commands, stage, dialog, view, &layout, true, &look, &cross);
            commands.insert_resource(layout);
        }
        None => commands.remove_resource::<layout::Layout>(),
    }
}

/// The remastered screen as it was drawn when a popup opened over it.
#[derive(Resource, Default)]
struct Underlay {
    from: u16,
    picture: Option<RgbaImage>,
}

/// The canvas in a picture of the window: where the canvas fits it, scaled
/// to the canvas's size.
fn canvas_of(window: &Image) -> Option<RgbaImage> {
    let window = window.clone().try_into_dynamic().ok()?.to_rgba8();
    let (w, h) = (window.width() as f32, window.height() as f32);
    let scale = (w / CANVAS.x).min(h / CANVAS.y);
    let size = CANVAS * scale;
    let (left, top) = (((w - size.x) / 2.0) as u32, ((h - size.y) / 2.0) as u32);
    let canvas = image::imageops::crop_imm(&window, left, top, size.x as u32, size.y as u32);
    let (cw, ch) = (CANVAS.x as u32, CANVAS.y as u32);
    Some(image::imageops::resize(&canvas.to_image(), cw, ch, image::imageops::FilterType::Triangle))
}

/// A popup's picture in the classic look: the screen it was opened from, as
/// it was last drawn, with the popup's own picture on it in the middle.
fn classic_popup(
    under: Option<&classic::ClassicDialog>,
    images: &Assets<Image>,
    frame: Rect,
    picture: &RgbaImage,
) -> RgbaImage {
    let plain = || RgbaImage::from_pixel(640, 480, PLAIN);
    let mut classic = under.and_then(|c| c.last_frame(images)).unwrap_or_else(plain);
    image::imageops::overlay(&mut classic, picture, frame.min.x as i64, frame.min.y as i64);
    classic
}

/// A popup's picture in the remastered look: the screen it was opened from,
/// dimmed, with a shadow under the popup.
fn wide_popup(
    game: &Game,
    under: Option<&classic::ClassicDialog>,
    underlay: &Underlay,
    frame: Rect,
    picture: &RgbaImage,
) -> RgbaImage {
    let parent = under.map(|c| c.id());
    let captured = underlay.picture.clone().filter(|_| parent == Some(underlay.from));
    // Without a capture — opened in the classic look — the screen's picture.
    let parent = captured.or_else(|| {
        let id = parent?;
        let art = game.graphic(game.dialogs.get(&id)?.background).ok()?;
        Some(backdrop::load(id, &art))
    });
    let mut wide =
        parent.unwrap_or_else(|| backdrop::load(0, &RgbaImage::from_pixel(640, 480, PLAIN)));
    for pixel in wide.pixels_mut() {
        for channel in &mut pixel.0[..3] {
            *channel = (f32::from(*channel) * POPUP_DIM) as u8;
        }
    }
    let scale = crate::look::CLASSIC_SCALE;
    let (w, h) = ((frame.width() * scale) as u32, (frame.height() * scale) as u32);
    let at = Vec2::new(crate::look::CLASSIC_LEFT + frame.min.x * scale, frame.min.y * scale);
    let mut shadow =
        RgbaImage::from_pixel(w + 2 * POPUP_SHADOW, h + 2 * POPUP_SHADOW, Rgba([0; 4]));
    for y in POPUP_SHADOW..POPUP_SHADOW + h {
        for x in POPUP_SHADOW..POPUP_SHADOW + w {
            shadow.put_pixel(x, y, Rgba([0, 0, 0, 150]));
        }
    }
    let shadow = image::imageops::blur(&shadow, POPUP_SHADOW as f32 / 2.5);
    let offset = POPUP_SHADOW as i64;
    image::imageops::overlay(&mut wide, &shadow, at.x as i64 - offset, at.y as i64 - offset + 10);
    let big = image::imageops::resize(picture, w, h, image::imageops::FilterType::Lanczos3);
    image::imageops::overlay(&mut wide, &big, at.x as i64, at.y as i64);
    wide
}

/// A red cross over what does nothing yet, stretched to its size.
fn cross() -> Image {
    const SIZE: u32 = 64;
    const HALF_WIDTH: f32 = 2.5;
    let mut cross = image::RgbaImage::new(SIZE, SIZE);
    for (x, y, pixel) in cross.enumerate_pixels_mut() {
        let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
        let off = (x - y).abs().min((x + y - SIZE as f32).abs()) / std::f32::consts::SQRT_2;
        if off <= HALF_WIDTH {
            *pixel = image::Rgba([220, 20, 20, 200]);
        }
    }
    smooth_image(cross)
}

/// The hotspots of one look; only the current look's answer the pointer.
/// What does nothing yet is crossed out.
#[allow(clippy::too_many_arguments)]
fn spawn_hotspots(
    commands: &mut Commands,
    stage: Entity,
    dialog: &Dialog,
    view: &View,
    geometry: &dyn a3_screens::pointer::Geometry,
    remastered: bool,
    look: &Look,
    cross: &Handle<Image>,
) {
    let display =
        if look.showing_remastered() == remastered { Display::Flex } else { Display::None };
    for (target, at) in targets(dialog, view, geometry) {
        // Scrollbar arrows lie on top of the rows they scroll.
        let z = if matches!(target, Target::Scroll { .. }) { 3 } else { 2 };
        commands
            .spawn((
                Hotspot { target, zoom: geometry.zoom(), dialog: view.id },
                ForLook { remastered },
                Node { display, ..canvas_rect(at.min.x, at.min.y, at.width(), at.height()) },
                ZIndex(z),
                ChildOf(stage),
            ))
            .observe(point_at)
            .observe(point_away)
            .observe(press)
            .observe(click)
            .observe(drag)
            .with_children(|spot| {
                // The classic look draws its own crosses.
                if remastered && view.inert.contains(&target) {
                    spot.spawn((
                        ImageNode::new(cross.clone()),
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                }
            });
    }
}

/// Switching looks switches which hotspots answer the pointer.
fn switch_hotspots(
    look: Res<Look>,
    mut hotspots: Query<(&ForLook, &mut Node)>,
    mut view: ResMut<OpenDialog>,
) {
    if !look.is_changed() {
        return;
    }
    for (owner, mut node) in &mut hotspots {
        let display = if owner.remastered == look.showing_remastered() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    view.hovered = None;
}

fn point_at(over: On<PointerOver>, hotspots: Query<&Hotspot>, mut view: ResMut<OpenDialog>) {
    if let Ok(hotspot) = hotspots.get(over.entity)
        && hotspot.dialog == view.id
    {
        view.hovered = Some(hotspot.target);
    }
}

fn point_away(out: On<PointerOut>, hotspots: Query<&Hotspot>, mut view: ResMut<OpenDialog>) {
    // Moving between two hotspots may report the new one first.
    if let Ok(hotspot) = hotspots.get(out.entity)
        && hotspot.dialog == view.id
        && view.hovered == Some(hotspot.target)
    {
        view.hovered = None;
    }
}

/// Like the game's controls, everything but the buttons acts as soon as it
/// is pressed. Arrows keep repeating while held; a press on a slider's track
/// moves the handle there and takes hold of it.
#[allow(clippy::too_many_arguments)]
fn press(
    press: On<PointerPress>,
    hotspots: Query<&Hotspot>,
    game: Res<Game>,
    sounds: Res<Sounds>,
    mut commands: Commands,
    mut view: ResMut<OpenDialog>,
    mut held: ResMut<Held>,
    mut pending: ResMut<Pending>,
) {
    let Ok(&Hotspot { target, zoom, dialog }) = hotspots.get(press.entity) else { return };
    // The right button opens the menu of what it is on; it steps a
    // stepping button back.
    if press.button == PointerButton::Secondary && dialog == view.id {
        let grip = Grip::of(target).or(match target {
            Target::Button(button) if is_stepping(&game, view.id, button) => {
                Some(Grip::Control(button))
            }
            _ => None,
        });
        if let Some(grip) = grip {
            pending.0.push((Target::Menu(grip), press.count));
        }
        return;
    }
    if press.button != PointerButton::Primary || dialog != view.id {
        return;
    }
    match target {
        Target::Button(button) => {
            if !view.disabled(button) {
                view.pressed = Some(target);
            }
        }
        Target::SliderTrack(control) => {
            let Some(c) = game.dialogs.get(&view.id).map(|d| &d.controls[control]) else {
                return;
            };
            let Some(&shown) = view.value(control) else { return };
            // The hit is relative to the track, from -0.5 to 0.5. The handle
            // jumps under the pointer and stays held there.
            let along = press.hit.position.map_or(0.5, |p| p.x + 0.5);
            let left = slider::handle_under(c, along);
            let value = slider::value_at(c, &shown, left);
            pending.0.push((Target::SliderAt { control, value }, 1));
            held.drag = Some((control, left, zoom));
        }
        Target::Option { control, item } if is_bar(&game, view.id, control) => {
            if view.bar(control).is_some_and(|b| b.item_on(item)) {
                let sound = if item == hub::WHISTLE { &sounds.whistle } else { &sounds.click };
                if let Some(sound) = sound {
                    commands.spawn((AudioPlayer::new(sound.clone()), PlaybackSettings::DESPAWN));
                }
            }
            pending.0.push((target, press.count));
        }
        target => {
            pending.0.push((target, press.count));
            if matches!(
                target,
                Target::Spin { .. } | Target::SliderStep { .. } | Target::Scroll { .. }
            ) {
                *held = Held { repeat: Some(target), timer: REPEAT_DELAY, ..default() };
            }
            held.grip = Grip::of(target);
            if held.grip.is_some() {
                let dialog = game.dialogs.get(&view.id);
                let rect = dialog.and_then(|d| {
                    targets(d, &view, &a3_screens::pointer::Pixels)
                        .into_iter()
                        .find_map(|(t, r)| (t == target).then_some(r))
                });
                let hit =
                    press.hit.position.map_or(Vec2::splat(0.5), |p| Vec2::new(p.x, p.y) + 0.5);
                held.grab = rect.map(|r| (r.min + hit * r.size(), zoom));
            }
        }
    }
}

/// Whether a control of a dialog is the menu bar.
fn is_stepping(game: &Game, id: u16, control: usize) -> bool {
    game.dialogs
        .get(&id)
        .and_then(|d| d.controls.get(control))
        .is_some_and(|c| matches!(c.kind, Kind::States { .. }))
}

fn is_bar(game: &Game, id: u16, control: usize) -> bool {
    game.dialogs
        .get(&id)
        .and_then(|d| d.controls.get(control))
        .is_some_and(|c| matches!(c.kind, Kind::Bar { .. }))
}

/// Buttons act when the pointer lets go of them, with the game's click.
fn click(
    click: On<PointerClick>,
    hotspots: Query<&Hotspot>,
    view: Res<OpenDialog>,
    sounds: Res<Sounds>,
    mut commands: Commands,
    mut pending: ResMut<Pending>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(&Hotspot { target: target @ Target::Button(button), dialog, .. }) =
        hotspots.get(click.entity)
    else {
        return;
    };
    if dialog != view.id || view.disabled(button) {
        return;
    }
    if let Some(sound) = &sounds.click {
        commands.spawn((AudioPlayer::new(sound.clone()), PlaybackSettings::DESPAWN));
    }
    pending.0.push((target, click.count));
}

/// The click the game's bitmap buttons make when they are let go of, and
/// the whistle of the bar's last symbol.
#[derive(Resource, Default)]
struct Sounds {
    click: Option<Handle<AudioSource>>,
    whistle: Option<Handle<AudioSource>>,
}

fn load_sounds(mut commands: Commands, game: Res<Game>, mut audio: ResMut<Assets<AudioSource>>) {
    let mut load = |path: &str| match game.file(path) {
        Ok(bytes) => Some(audio.add(AudioSource { bytes: bytes.into() })),
        Err(err) => {
            warn!("{path}: {err}");
            None
        }
    };
    commands.insert_resource(Sounds { click: load(CLICK), whistle: load(WHISTLE) });
}

/// A slider handle follows the pointer that took hold of it, and so does a
/// player's box or row being dragged.
fn drag(
    drag: On<PointerDrag>,
    game: Res<Game>,
    mut view: ResMut<OpenDialog>,
    held: Res<Held>,
    scale: Res<UiScale>,
    mut pending: ResMut<Pending>,
) {
    if let (Some(grip), Some((grab, zoom))) = (held.grip, held.grab) {
        let at = grab + Vec2::new(drag.distance.x, drag.distance.y) / (scale.0 * zoom);
        let dragging = Some(a3_screens::view::Dragging { grip, at });
        if view.dragging != dragging {
            view.dragging = dragging;
        }
        return;
    }
    let Some((control, start, zoom)) = held.drag else { return };
    let Some(c) = game.dialogs.get(&view.id).map(|d| &d.controls[control]) else { return };
    let Some(shown) = view.value(control) else { return };
    // Window pixels per classic pixel.
    let value = slider::value_at(c, shown, start + drag.distance.x / (scale.0 * zoom));
    if value != shown.value {
        pending.0.push((Target::SliderAt { control, value }, 1));
    }
}

/// Letting go of the pointer ends repeating, dragging and holding a button
/// down; what it held of a box or a row it lets go of on what it points at,
/// if that is something else.
fn release(
    mut inputs: MessageReader<PointerInput>,
    mut held: ResMut<Held>,
    mut view: ResMut<OpenDialog>,
    mut pending: ResMut<Pending>,
) {
    let released = inputs
        .read()
        .any(|input| matches!(input.action, PointerAction::Release(PointerButton::Primary)));
    if released && let Some(from) = held.grip {
        let to = view.hovered.and_then(Grip::of);
        // Where the pointer took hold and how far it went, in whole
        // classic pixels.
        let whole = |v: Vec2| (v.x.round() as i16, v.y.round() as i16);
        let at = held.grab.map_or((0, 0), |(grab, _)| whole(grab));
        let moved = match (view.dragging, held.grab) {
            (Some(dragging), Some((grab, _))) => whole(dragging.at - grab),
            _ => (0, 0),
        };
        if to != Some(from) {
            pending.0.push((Target::Drop { from, to, at, moved }, 1));
        }
    }
    if released && (held.repeat.is_some() || held.drag.is_some() || held.grip.is_some()) {
        *held = Held::default();
    }
    if released && view.pressed.is_some() {
        view.pressed = None;
    }
    if released && view.dragging.is_some() {
        view.dragging = None;
    }
}

/// A held arrow acts again and again — while the pointer stays on it.
fn repeat(
    time: Res<Time>,
    view: Res<OpenDialog>,
    mut held: ResMut<Held>,
    mut pending: ResMut<Pending>,
) {
    let Some(target) = held.repeat else { return };
    if view.hovered != Some(target) {
        return;
    }
    held.timer -= time.delta_secs();
    if held.timer <= 0.0 {
        held.timer += REPEAT_SECS;
        pending.0.push((target, 1));
    }
}

/// Typing into the focused field, arrows through the options or rows, Enter
/// for OK and Escape for cancel.
fn keyboard(
    mut typed: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<Game>,
    mut view: ResMut<OpenDialog>,
    mut pending: ResMut<Pending>,
) {
    let Some(dialog) = game.dialogs.get(&view.id) else { return };
    for event in typed.read() {
        if view.focus.is_none() || event.state != ButtonState::Pressed {
            continue;
        }
        match &event.logical_key {
            Key::Backspace => view.backspace(),
            Key::Tab => view.next_field(dialog),
            _ => {
                if let Some(text) = &event.text {
                    view.type_text(dialog, text);
                }
            }
        }
    }

    let step = i32::from(keys.just_pressed(KeyCode::ArrowDown))
        - i32::from(keys.just_pressed(KeyCode::ArrowUp));
    if step != 0 {
        view.step(dialog, step);
    }
    if keys.just_pressed(KeyCode::Enter)
        && let Some(ok) = ok_button(dialog)
    {
        pending.0.push((Target::Button(ok), 1));
    }
    if keys.just_pressed(KeyCode::Escape)
        && let Some(cancel) = cancel_button(dialog)
    {
        pending.0.push((Target::Button(cancel), 1));
    }
}

/// The wheel scrolls the list under the pointer.
fn wheel(mut wheel: MessageReader<MouseWheel>, game: Res<Game>, mut view: ResMut<OpenDialog>) {
    let notches: f32 = wheel.read().map(|w| w.y.signum()).sum();
    let Some(dialog) = game.dialogs.get(&view.id) else { return };
    if notches != 0.0 {
        view.wheel(dialog, notches as i32);
    }
}

fn act(
    mut pending: ResMut<Pending>,
    game: Res<Game>,
    mut host: ResMut<OpenDialog>,
    mut shows: Shows,
) {
    for (target, clicks) in std::mem::take(&mut pending.0) {
        let from = host.id;
        // Windows takes every second click of a quick run for a double
        // click, the third for a single one again.
        let change = host.0.input(&game, target, clicks >= 2 && clicks % 2 == 0);
        if shows.change(&game, change, from, host.id) {
            return;
        }
    }
}

/// Moves on the screens that move on by themselves.
fn tick(time: Res<Time>, game: Res<Game>, mut host: ResMut<OpenDialog>, mut shows: Shows) {
    let from = host.id;
    let change = host.0.tick(&game, time.delta_secs());
    shows.change(&game, change, from, host.id);
}

/// Opens the game cycle's next request once it has come.
fn receive(game: Res<Game>, mut host: ResMut<OpenDialog>, mut shows: Shows) {
    let from = if host.0.is_open() { host.id } else { 0 };
    let change = host.0.poll(&game);
    if host.0.is_open() || change == Change::Quit {
        let to = if host.0.is_open() { host.id } else { 0 };
        shows.change(&game, change, from, to);
    }
}

/// What it takes to show the screen that comes up, or to end the game.
#[derive(SystemParam)]
struct Shows<'w, 's> {
    look: Res<'w, Look>,
    commands: Commands<'w, 's>,
    fps: Query<'w, 's, &'static mut Visibility, With<crate::fps::FpsText>>,
    next: ResMut<'w, NextState<Screen>>,
    exit: MessageWriter<'w, AppExit>,
}

impl Shows<'_, '_> {
    /// Shows dialog `to` in place of dialog `from` if another screen came up,
    /// or ends the game; whether either happened.
    fn change(&mut self, game: &Game, change: Change, from: u16, to: u16) -> bool {
        match change {
            Change::Same => return false,
            Change::Quit => _ = self.exit.write(AppExit::Success),
            Change::Screen => {
                // A popup in the remastered look over a screen shown in it.
                let popup = game.dialogs.get(&to).is_some_and(|d| d.frame.is_some());
                if popup && remastered::offers(to) && self.look.showing_remastered() {
                    // The remastered popup lies over the screen as it was
                    // drawn, which the next frame captures — without the
                    // frame rate.
                    self.fps.iter_mut().for_each(|mut v| *v = Visibility::Hidden);
                    self.commands.spawn(Screenshot::primary_window()).observe(
                        move |captured: On<ScreenshotCaptured>,
                              mut underlay: ResMut<Underlay>,
                              mut fps: Query<&mut Visibility, With<crate::fps::FpsText>>,
                              mut next: ResMut<NextState<Screen>>| {
                            *underlay = Underlay { from, picture: canvas_of(&captured.image) };
                            fps.iter_mut().for_each(|mut v| *v = Visibility::Inherited);
                            next.set(Screen::Dialog(to));
                        },
                    );
                } else {
                    self.next.set(Screen::Dialog(to));
                }
            }
        }
        true
    }
}

/// Shows the tooltip of the bar's symbol the pointer rests on, and hides it
/// as the pointer moves on.
fn rest(
    time: Res<Time>,
    game: Res<Game>,
    mut view: ResMut<OpenDialog>,
    mut resting: ResMut<Resting>,
) {
    if resting.target != view.hovered {
        *resting = Resting { target: view.hovered, timer: 0.0 };
        if view.tip.is_some() {
            view.tip = None;
        }
        return;
    }
    resting.timer += time.delta_secs();
    let Some(Target::Option { control, item }) = view.hovered else { return };
    if resting.timer >= TIP_SECS && view.tip.is_none() && is_bar(&game, view.id, control) {
        view.tip = Some((control, item));
    }
}

/// Runs the frame time of playing animations, and moves them on to their
/// next frame when it is up.
fn animate(time: Res<Time>, mut view: ResMut<OpenDialog>, mut frames: ResMut<Frames>) {
    if !view.moving() {
        return;
    }
    frames.timer += time.delta_secs();
    while frames.timer >= FRAME_SECS {
        frames.timer -= FRAME_SECS;
        view.advance();
    }
}

fn blink(time: Res<Time>, view: Res<OpenDialog>, mut caret: ResMut<Caret>) {
    if view.focus.is_none() {
        if caret.on {
            caret.on = false;
        }
        return;
    }
    // Only the toggle counts as a change; the timer ticks unseen.
    let elapsed = {
        let caret = caret.bypass_change_detection();
        caret.timer += time.delta_secs();
        caret.timer
    };
    if elapsed >= 0.5 {
        caret.timer = 0.0;
        caret.on = !caret.on;
    }
}
