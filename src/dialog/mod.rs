//! Dialog screens, drawn from the game's own dialog definitions in both looks.
//!
//! The controls of a dialog come from `MeineDialoge.rch` (see [`crate::rch`]);
//! what they show and where their buttons lead is decided in [`flow`]. Each
//! control keeps its state in [`View`], input lands on invisible hotspots
//! above both looks, and the classic and remastered renderers both draw from
//! that one state — which is what lets F9 switch looks mid-dialog.

mod backdrop;
mod classic;
mod decor;
mod flow;
mod hiring;
mod hub;
mod layout;
mod remastered;
mod turn;

use std::error::Error;
use std::path::PathBuf;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseWheel;
use bevy::picking::pointer::{PointerAction, PointerInput};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use image::{Rgba, RgbaImage};

use crate::bitmap_font::to_cp1252;
use crate::game::Game;
use crate::look::{CANVAS, Look, SharpMaterial, canvas_rect, spawn_stage};
use crate::rch::{Align, Control, Dialog, Kind};
use crate::{Fonts, Screen};

pub use flow::START;

/// List rows are one line of the dialog font.
const ROW_HEIGHT: f32 = 13.0;
/// The radio bullets painted into the backgrounds are Windows radio buttons,
/// 12 px square, with their top-left corner 2 px left of their list and 1 px
/// below the top of their option.
const BULLET_SIZE: f32 = 12.0;
const BULLET_CENTRE: Vec2 = Vec2::new(4.0, 7.0);
/// Labels of bulleted options start right of the bullet.
const LABEL_INSET: f32 = 9.0;
/// Graphic ids of the OK and cancel buttons' normal images.
const OK_IMAGE: u16 = 1272;
const CANCEL_IMAGE: u16 = 1260;
/// Animations move on to their next frame every 200 ms, as the game sets
/// them up; in between they blend into it, which the game did not.
const FRAME_SECS: f32 = 0.2;
/// Sliders have an arrow at each end, this wide; the handle runs between.
const SLIDER_ARROW: f32 = 20.0;
/// The slider handle, the game's `ScrollerBlau.bmp`, is this wide.
const SLIDER_HANDLE: f32 = 18.0;
/// Held arrows repeat, as the game's scrollbars do: first after 300 ms,
/// then every 80 ms.
const REPEAT_DELAY: f32 = 0.3;
const REPEAT_SECS: f32 = 0.08;
/// Lists longer than they are tall get a scrollbar in the strip this wide
/// that the backgrounds leave free right of the list's frame: an arrow at
/// each end, a handle in between.
const SCROLLBAR_WIDTH: f32 = 20.0;
/// The scrollbar handle, the game's `ScrollerBlau.bmp`.
const SCROLL_HANDLE: Vec2 = Vec2::new(18.0, 20.0);
/// The sound of a button, and of the whistle that ends a turn.
const CLICK: &str = "sounds/click.wav";
const WHISTLE: &str = "sounds/Leiste16.wav";
/// Rows the mouse wheel scrolls per notch, as Windows does.
const WHEEL_ROWS: i32 = 3;
/// In the remastered look a popup dims the screen behind it to this much,
/// and casts a shadow this wide.
const POPUP_DIM: f32 = 0.55;
const POPUP_SHADOW: u32 = 28;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load_sounds)
        .init_resource::<flow::Session>()
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
                remastered::fill,
                remastered::refresh,
                remastered::show_bitmaps,
                remastered::animate_flags,
            )
                .chain()
                .run_if(resource_exists::<View>)
                .run_if(in_dialog),
        )
        .add_systems(
            Update,
            (
                remastered::ease_options,
                remastered::ease_boxes,
                remastered::ease_buttons,
                remastered::ease_pushes,
                remastered::ease_bar,
                switch_hotspots,
            )
                .run_if(resource_exists::<View>)
                .run_if(in_dialog),
        );
    let dialogs = flow::DIALOGS.into_iter().chain(turn::DIALOGS).chain(hiring::DIALOGS);
    for id in dialogs.chain(hub::DIALOGS) {
        app.add_systems(OnEnter(Screen::Dialog(id)), open);
    }
}

/// Adds the dialogs the game puts together from others, like the week screen
/// with a panel per club, and the controls it makes itself, like the menu
/// bar of the main screens.
pub fn compose(game: &mut Game) {
    turn::compose(&mut game.dialogs);
    hub::compose(&mut game.dialogs, &game.rules);
}

/// Writes the pictures the remastered look draws on, with masks of what to
/// repaint, to `<cache>/sources` for `tools/outpaint.py`.
pub fn export_backgrounds(game: &Game) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    backdrop::export(game)
}

fn in_dialog(screen: Res<State<Screen>>) -> bool {
    matches!(screen.get(), Screen::Dialog(_))
}

/// Something a pointer or key can act on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    Option {
        control: usize,
        item: usize,
    },
    Button(usize),
    /// A line of a list, counted from the first one shown.
    Row {
        control: usize,
        row: usize,
    },
    Edit(usize),
    Spin {
        control: usize,
        up: bool,
    },
    /// A static or animation the flow made clickable.
    Control(usize),
    /// An arrow at the end of a slider.
    SliderStep {
        control: usize,
        up: bool,
    },
    /// A slider's track, between its arrows.
    SliderTrack(usize),
    /// A value a slider's handle is dragged or paged to.
    SliderAt {
        control: usize,
        value: i32,
    },
    /// An arrow of a list's scrollbar.
    Scroll {
        control: usize,
        up: bool,
    },
}

/// The open dialog and everything about it that can change.
#[derive(Resource)]
struct View {
    id: u16,
    /// One per control of the dialog, in the same order.
    controls: Vec<ControlState>,
    /// The text field being typed into.
    focus: Option<usize>,
    hovered: Option<Target>,
    /// The button the pointer is holding down.
    pressed: Option<Target>,
    /// The item whose tooltip shows: control and item.
    tip: Option<(usize, usize)>,
}

#[derive(Default)]
struct ControlState {
    /// What a static shows (placeholders filled in), or a field's contents.
    text: String,
    /// The chosen option.
    chosen: usize,
    /// The options' labels.
    labels: Vec<String>,
    /// List rows, their columns separated by tabs.
    rows: Vec<String>,
    selected: Option<usize>,
    /// The first row a list shows.
    scroll: usize,
    /// Where a list's or header's columns start, relative to the control.
    tabs: Vec<f32>,
    /// An animation's frames as graphic ids, and the one it shows. It stays
    /// on that frame while it is not playing.
    frames: Vec<u16>,
    frame: usize,
    playing: bool,
    /// Statics and animations only answer the pointer when the flow says so.
    clickable: bool,
    /// A slider's or spinner's value, within `range`.
    value: i32,
    range: (i32, i32),
    hidden: bool,
    /// A disabled button ignores the pointer.
    disabled: bool,
    /// What a bitmap shows.
    picture: Option<Picture>,
    /// A group whose options are buttons: none of them stays chosen, the one
    /// pointed at lights up.
    actions: bool,
    /// Options that are switched off: they show nothing and do not answer.
    off: Vec<bool>,
    /// What a player's box shows.
    player: Option<PlayerBox>,
    /// How each of a list's columns sits in its width — centred or to the
    /// right — where it does not start at its tab and run to the next.
    aligns: Vec<Option<(Align, f32)>>,
    /// The colour of each list row, if it has one of its own, and the icons
    /// it shows: where from the list's left edge, and which graphic.
    inks: Vec<Option<[u8; 3]>>,
    row_icons: Vec<Vec<(f32, u16)>>,
    /// What the items say when the pointer rests on them.
    tips: Vec<String>,
}

/// A player's box on the pitch or the bench: his name — or, on an empty
/// spot, the position's — the position he plays and his strength there,
/// up to three abilities, and how well he fits the spot, in tenths.
#[derive(Clone, Default, PartialEq, Debug)]
struct PlayerBox {
    name: String,
    position: String,
    strength: String,
    fit: i32,
    icons: Vec<u16>,
    filled: bool,
}

impl ControlState {
    fn item_on(&self, item: usize) -> bool {
        !self.off.get(item).copied().unwrap_or(false)
    }
}

/// A picture a bitmap control shows: one of the game's graphics, a file of
/// the installation or a chart the game draws, whole or a part of it (x, y,
/// width, height), and with black left out or not.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Picture {
    source: PictureSource,
    part: Option<[u32; 4]>,
    black_clear: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum PictureSource {
    Graphic(u16),
    File(String),
    /// A red line through values from 0 to 100, one every few pixels, in a
    /// clear picture of the given size.
    Chart {
        values: Vec<u8>,
        size: (u32, u32),
        step: u32,
    },
}

/// What the remastered look draws behind a screen's controls where the
/// classic one has it painted into the picture: a part of the game's
/// artwork, a card, or the band along a card's top — in classic pixels.
enum Decoration {
    /// A picture, stretched over its place or fitted into it.
    Picture {
        picture: Picture,
        at: Rect,
        stretch: bool,
    },
    Card(Rect),
    Band(Rect),
}

/// The decorations of a screen.
fn decorations(id: u16) -> Vec<Decoration> {
    hub::decorations(id).into_iter().chain(turn::decorations(id)).collect()
}

/// The red the game draws its charts in.
const CHART_INK: Rgba<u8> = Rgba([255, 0, 0, 255]);

impl Picture {
    fn graphic(id: u16) -> Self {
        Self { source: PictureSource::Graphic(id), part: None, black_clear: false }
    }

    fn file(path: String) -> Self {
        Self { source: PictureSource::File(path), part: None, black_clear: true }
    }

    fn load(&self, game: &Game) -> Result<RgbaImage, Box<dyn Error>> {
        let whole = match &self.source {
            PictureSource::Graphic(id) => game.graphic(*id)?,
            PictureSource::File(path) => image::load_from_memory(&game.file(path)?)?.to_rgba8(),
            PictureSource::Chart { values, size, step } => chart(values, *size, *step),
        };
        let part = match self.part {
            Some([x, y, w, h]) => image::imageops::crop_imm(&whole, x, y, w, h).to_image(),
            None => whole,
        };
        Ok(if self.black_clear { crate::game::transparent_black(part) } else { part })
    }
}

/// The game's line chart: from the left edge, a point every `step` pixels,
/// 100 at the top and 0 at the bottom.
fn chart(values: &[u8], (w, h): (u32, u32), step: u32) -> RgbaImage {
    let mut picture = RgbaImage::new(w, h);
    let y = |v: u8| h as i32 - i32::from(v) * h as i32 / 101;
    let points: Vec<(i32, i32)> =
        values.iter().enumerate().map(|(i, &v)| (i as i32 * step as i32, y(v))).collect();
    for pair in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for k in 0..=steps {
            let x = x0 + (x1 - x0) * k / steps;
            let y = y0 + (y1 - y0) * k / steps;
            if (0..w as i32).contains(&x) && (0..h as i32).contains(&y) {
                picture.put_pixel(x as u32, y as u32, CHART_INK);
            }
        }
    }
    picture
}

impl ControlState {
    /// Scrolls a list by `rows`, as far as there are rows.
    fn scroll_by(&mut self, rows: i32, visible: usize) {
        let last = self.rows.len().saturating_sub(visible);
        self.scroll = (self.scroll as i32 + rows).clamp(0, last as i32) as usize;
    }

    /// Scrolls a list just far enough to show its selected row.
    fn reveal(&mut self, visible: usize) {
        if let Some(row) = self.selected {
            if row < self.scroll {
                self.scroll = row;
            } else if visible > 0 && row >= self.scroll + visible {
                self.scroll = row + 1 - visible;
            }
        }
    }

    fn step_value(&mut self, up: bool) {
        self.set_value(self.value + if up { 1 } else { -1 });
    }

    fn set_value(&mut self, value: i32) {
        let (min, max) = self.range;
        self.value = value.clamp(min, max.max(min));
    }

    /// The frame an animation shows, the one after it, and how far it has
    /// blended into that one, given how far the frame time has run.
    fn showing(&self, phase: f32) -> (usize, usize, f32) {
        let count = self.frames.len().max(1);
        let blend = if self.playing && count > 1 { phase } else { 0.0 };
        (self.frame % count, (self.frame + 1) % count, blend)
    }
}

impl View {
    fn new(id: u16, dialog: &Dialog) -> Self {
        let controls = dialog
            .controls
            .iter()
            .map(|c| ControlState {
                text: c.text.clone(),
                // The dialog file breaks a label into lines with "\n".
                labels: match &c.kind {
                    Kind::Options { items, .. } => {
                        items.iter().map(|i| i.text.replace("\\n", "\n")).collect()
                    }
                    _ => Vec::new(),
                },
                tabs: vec![0.0],
                ..default()
            })
            .collect();
        Self { id, controls, focus: None, hovered: None, pressed: None, tip: None }
    }
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

/// What the pressed pointer holds on to: an arrow that repeats, or a slider
/// handle being dragged — from where it was, at how many canvas pixels to a
/// classic one.
#[derive(Resource, Default)]
struct Held {
    repeat: Option<Target>,
    timer: f32,
    drag: Option<(usize, f32, f32)>,
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
/// many canvas pixels across a classic pixel is there.
#[derive(Component)]
struct Hotspot {
    target: Target,
    zoom: f32,
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
    game: Res<Game>,
    mut session: ResMut<flow::Session>,
    fonts: Res<Fonts>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SharpMaterial>>,
    look: Res<Look>,
    under: Option<Res<classic::ClassicDialog>>,
    underlay: Res<Underlay>,
) {
    let Screen::Dialog(id) = *screen.get() else { return };
    let Some(dialog) = game.dialogs.get(&id) else {
        return warn!("dialog {id} is not in the game's dialog file");
    };
    let mut view = View::new(id, dialog);
    flow::setup(&mut view, dialog, &game, &mut session);
    let background = game.graphic(dialog.background).unwrap_or_else(|err| {
        warn!("background of dialog {id}: {err}");
        RgbaImage::from_pixel(640, 480, Rgba([224, 222, 226, 255]))
    });
    let (background, wide) = match dialog.frame {
        Some(frame) => {
            popup_backgrounds(&game, under.as_deref(), &underlay, &images, frame, &background)
        }
        None => {
            let wide = backdrop::load(id, &background);
            (background, wide)
        }
    };

    let stage = spawn_stage(&mut commands, Screen::Dialog(id));
    let layout = remastered::spawn(
        &mut commands,
        stage,
        dialog,
        &view,
        &background,
        wide,
        &game,
        &fonts,
        &mut images,
    );
    classic::spawn(
        &mut commands,
        stage,
        dialog,
        &view,
        background,
        &game,
        &mut images,
        &mut materials,
    );
    spawn_hotspots(&mut commands, stage, dialog, &view, &layout::Classic, false, &look);
    spawn_hotspots(&mut commands, stage, dialog, &view, &layout, true, &look);
    commands.insert_resource(layout);
    commands.insert_resource(view);
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

/// A popup's pictures in both looks: the screen it was opened from, as it was
/// last drawn, with the popup's own picture on it in the middle; in the
/// remastered look that screen dimmed, with a shadow under the popup.
fn popup_backgrounds(
    game: &Game,
    under: Option<&classic::ClassicDialog>,
    underlay: &Underlay,
    images: &Assets<Image>,
    frame: Rect,
    picture: &RgbaImage,
) -> (RgbaImage, RgbaImage) {
    let plain = || RgbaImage::from_pixel(640, 480, Rgba([224, 222, 226, 255]));
    let mut classic = under.and_then(|c| c.last_frame(images)).unwrap_or_else(plain);
    image::imageops::overlay(&mut classic, picture, frame.min.x as i64, frame.min.y as i64);

    let parent = under.map(|c| c.id());
    let captured = underlay.picture.clone().filter(|_| parent == Some(underlay.from));
    // Without a capture — opened in the classic look — the screen's picture.
    let parent = captured.or_else(|| {
        let id = parent?;
        let art = game.graphic(game.dialogs.get(&id)?.background).ok()?;
        Some(backdrop::load(id, &art))
    });
    let mut wide = parent.unwrap_or_else(|| backdrop::load(0, &plain()));
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
    (classic, wide)
}

/// The hotspots of one look; only the current look's answer the pointer.
fn spawn_hotspots(
    commands: &mut Commands,
    stage: Entity,
    dialog: &Dialog,
    view: &View,
    geometry: &dyn layout::Geometry,
    remastered: bool,
    look: &Look,
) {
    let display = if look.remastered == remastered { Display::Flex } else { Display::None };
    for (target, at) in layout::targets(dialog, view, geometry) {
        // Scrollbar arrows lie on top of the rows they scroll.
        let z = if matches!(target, Target::Scroll { .. }) { 3 } else { 2 };
        commands
            .spawn((
                Hotspot { target, zoom: geometry.zoom() },
                ForLook { remastered },
                Node { display, ..canvas_rect(at.min.x, at.min.y, at.width(), at.height()) },
                ZIndex(z),
                ChildOf(stage),
            ))
            .observe(point_at)
            .observe(point_away)
            .observe(press)
            .observe(click)
            .observe(drag);
    }
}

/// Switching looks switches which hotspots answer the pointer.
fn switch_hotspots(
    look: Res<Look>,
    mut hotspots: Query<(&ForLook, &mut Node)>,
    mut view: ResMut<View>,
) {
    if !look.is_changed() {
        return;
    }
    for (owner, mut node) in &mut hotspots {
        let display =
            if owner.remastered == look.remastered { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    view.hovered = None;
}

fn visible_rows(control: &Control) -> usize {
    ((control.rect.height() - 2.0) / ROW_HEIGHT).floor().max(0.0) as usize
}

/// Whether a list has more rows than it shows, and so a scrollbar.
fn scrolls(control: &Control, state: &ControlState) -> bool {
    state.rows.len() > visible_rows(control)
}

/// Where a list's scrollbar puts its up and down arrows, in classic pixels.
fn scroll_arrows(control: &Control) -> (Rect, Rect) {
    let r = control.rect;
    let left = r.max.x - SCROLLBAR_WIDTH;
    let up = Rect::new(left, r.min.y, r.max.x, r.min.y + SCROLLBAR_WIDTH);
    let down = Rect::new(left, r.max.y - SCROLLBAR_WIDTH, r.max.x, r.max.y);
    (up, down)
}

/// Where a list's scrollbar handle sits, in classic pixels: between the
/// arrows, as far down as the list is scrolled.
fn scroll_thumb(control: &Control, state: &ControlState) -> Rect {
    let (up, down) = scroll_arrows(control);
    let room = (down.min.y - up.max.y - SCROLL_HANDLE.y).max(0.0);
    let last = state.rows.len().saturating_sub(visible_rows(control)).max(1) as f32;
    let top = up.max.y + (room * state.scroll as f32 / last).round();
    let left = up.min.x + ((SCROLLBAR_WIDTH - SCROLL_HANDLE.x) / 2.0).floor();
    Rect::new(left, top, left + SCROLL_HANDLE.x, top + SCROLL_HANDLE.y)
}

/// Where a slider's handle sits, in classic pixels.
fn slider_handle(control: &Control, state: &ControlState) -> Rect {
    let r = control.rect;
    let (min, max) = state.range;
    let track = r.width() - 2.0 * SLIDER_ARROW - SLIDER_HANDLE;
    let share = if max > min { (state.value - min) as f32 / (max - min) as f32 } else { 0.0 };
    let left = r.min.x + SLIDER_ARROW + (track * share).round();
    Rect::new(left, r.min.y, left + SLIDER_HANDLE, r.max.y)
}

/// The value whose handle position is nearest to a handle at `left`.
fn slider_value(control: &Control, state: &ControlState, left: f32) -> i32 {
    let (min, max) = state.range;
    let track = control.rect.width() - 2.0 * SLIDER_ARROW - SLIDER_HANDLE;
    let share = (left - control.rect.min.x - SLIDER_ARROW) / track.max(1.0);
    (min + (share * (max - min) as f32).round() as i32).clamp(min, max.max(min))
}

/// Centre of the painted bullet of an option, in classic pixels.
fn bullet_centre(control: &Control, item: usize) -> Vec2 {
    let Kind::Options { items, .. } = &control.kind else { return Vec2::ZERO };
    control.rect.min + items[item].rect.min + BULLET_CENTRE
}

fn point_at(over: On<PointerOver>, hotspots: Query<&Hotspot>, mut view: ResMut<View>) {
    if let Ok(hotspot) = hotspots.get(over.entity) {
        view.hovered = Some(hotspot.target);
    }
}

fn point_away(out: On<PointerOut>, hotspots: Query<&Hotspot>, mut view: ResMut<View>) {
    // Moving between two hotspots may report the new one first.
    if let Ok(hotspot) = hotspots.get(out.entity)
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
    mut view: ResMut<View>,
    mut held: ResMut<Held>,
    mut pending: ResMut<Pending>,
) {
    let Ok(&Hotspot { target, zoom }) = hotspots.get(press.entity) else { return };
    if press.button != PointerButton::Primary {
        return;
    }
    match target {
        Target::Button(button) => {
            if !view.controls[button].disabled {
                view.pressed = Some(target);
            }
        }
        Target::SliderTrack(control) => {
            let Some(c) = game.dialogs.get(&view.id).map(|d| &d.controls[control]) else {
                return;
            };
            // The hit is relative to the track, from -0.5 to 0.5. The handle
            // jumps under the pointer and stays held there.
            let along = press.hit.position.map_or(0.5, |p| p.x + 0.5);
            let x = c.rect.min.x + SLIDER_ARROW + along * (c.rect.width() - 2.0 * SLIDER_ARROW);
            let left = x - SLIDER_HANDLE / 2.0;
            let value = slider_value(c, &view.controls[control], left);
            pending.0.push((Target::SliderAt { control, value }, 1));
            held.drag = Some((control, left, zoom));
        }
        Target::Option { control, item } if is_bar(&game, view.id, control) => {
            if view.controls[control].item_on(item) {
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
                *held = Held { repeat: Some(target), timer: REPEAT_DELAY, drag: None };
            }
        }
    }
}

/// Whether a control of a dialog is the menu bar.
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
    view: Res<View>,
    sounds: Res<Sounds>,
    mut commands: Commands,
    mut pending: ResMut<Pending>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(&Hotspot { target: target @ Target::Button(button), .. }) = hotspots.get(click.entity)
    else {
        return;
    };
    if view.controls[button].disabled {
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

/// A slider handle follows the pointer that took hold of it.
fn drag(
    drag: On<PointerDrag>,
    game: Res<Game>,
    view: Res<View>,
    held: Res<Held>,
    scale: Res<UiScale>,
    mut pending: ResMut<Pending>,
) {
    let Some((control, start, zoom)) = held.drag else { return };
    let Some(c) = game.dialogs.get(&view.id).map(|d| &d.controls[control]) else { return };
    let state = &view.controls[control];
    // Window pixels per classic pixel.
    let value = slider_value(c, state, start + drag.distance.x / (scale.0 * zoom));
    if value != state.value {
        pending.0.push((Target::SliderAt { control, value }, 1));
    }
}

/// Letting go of the pointer ends repeating, dragging and holding a button
/// down.
fn release(
    mut inputs: MessageReader<PointerInput>,
    mut held: ResMut<Held>,
    mut view: ResMut<View>,
) {
    let released = inputs
        .read()
        .any(|input| matches!(input.action, PointerAction::Release(PointerButton::Primary)));
    if released && (held.repeat.is_some() || held.drag.is_some()) {
        *held = Held::default();
    }
    if released && view.pressed.is_some() {
        view.pressed = None;
    }
}

/// A held arrow acts again and again — while the pointer stays on it.
fn repeat(time: Res<Time>, view: Res<View>, mut held: ResMut<Held>, mut pending: ResMut<Pending>) {
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
    mut view: ResMut<View>,
    mut pending: ResMut<Pending>,
) {
    let Some(dialog) = game.dialogs.get(&view.id) else { return };
    let edits: Vec<usize> = (0..dialog.controls.len())
        .filter(|&i| matches!(dialog.controls[i].kind, Kind::Edit { .. }))
        .collect();
    for event in typed.read() {
        let Some(focus) = view.focus.filter(|_| event.state == ButtonState::Pressed) else {
            continue;
        };
        let Kind::Edit { max_len } = dialog.controls[focus].kind else { continue };
        match &event.logical_key {
            Key::Backspace => {
                view.controls[focus].text.pop();
            }
            Key::Tab => {
                let next =
                    edits.iter().position(|&i| i == focus).map_or(0, |p| (p + 1) % edits.len());
                view.focus = edits.get(next).copied();
            }
            _ => {
                let text = &mut view.controls[focus].text;
                for c in event.text.iter().flat_map(|t| t.chars()) {
                    if !c.is_control() && to_cp1252(c).is_some() && text.chars().count() < max_len {
                        text.push(c);
                    }
                }
            }
        }
    }

    let step = i32::from(keys.just_pressed(KeyCode::ArrowDown))
        - i32::from(keys.just_pressed(KeyCode::ArrowUp));
    if step != 0 {
        flow::step(&mut view, dialog, step);
    }
    let button = |image: u16| {
        dialog
            .controls
            .iter()
            .position(|c| matches!(c.kind, Kind::Button { normal, .. } if normal == image))
    };
    if keys.just_pressed(KeyCode::Enter)
        && let Some(ok) = button(OK_IMAGE)
    {
        pending.0.push((Target::Button(ok), 1));
    }
    if keys.just_pressed(KeyCode::Escape)
        && let Some(cancel) = button(CANCEL_IMAGE)
    {
        pending.0.push((Target::Button(cancel), 1));
    }
}

/// The wheel scrolls the list under the pointer.
fn wheel(mut wheel: MessageReader<MouseWheel>, game: Res<Game>, mut view: ResMut<View>) {
    let notches: f32 = wheel.read().map(|w| w.y.signum()).sum();
    let list = match view.hovered {
        Some(Target::Row { control, .. } | Target::Scroll { control, .. }) => control,
        _ => return,
    };
    let Some(control) = game.dialogs.get(&view.id).map(|d| &d.controls[list]) else { return };
    if notches != 0.0 {
        let visible = visible_rows(control);
        view.controls[list].scroll_by(-notches as i32 * WHEEL_ROWS, visible);
    }
}

#[allow(clippy::too_many_arguments)]
fn act(
    mut pending: ResMut<Pending>,
    game: Res<Game>,
    look: Res<Look>,
    mut commands: Commands,
    mut fps: Query<&mut Visibility, With<crate::fps::FpsText>>,
    mut view: ResMut<View>,
    mut session: ResMut<flow::Session>,
    mut next: ResMut<NextState<Screen>>,
    mut exit: MessageWriter<AppExit>,
) {
    if pending.0.is_empty() {
        return;
    }
    let Some(dialog) = game.dialogs.get(&view.id) else { return };
    for (target, clicks) in std::mem::take(&mut pending.0) {
        match target {
            Target::Button(control) if view.controls[control].disabled => continue,
            Target::Option { control, item } => {
                let state = &mut view.controls[control];
                if !state.item_on(item) {
                    continue;
                }
                if !state.actions {
                    state.chosen = item;
                }
            }
            Target::Row { control, row } => {
                let state = &mut view.controls[control];
                if state.scroll + row >= state.rows.len() {
                    continue;
                }
                state.selected = Some(state.scroll + row);
            }
            Target::Edit(control) => view.focus = Some(control),
            Target::Scroll { control, up } => {
                let visible = visible_rows(&dialog.controls[control]);
                view.controls[control].scroll_by(if up { -1 } else { 1 }, visible);
                continue;
            }
            Target::Spin { control, up } | Target::SliderStep { control, up } => {
                view.controls[control].step_value(up)
            }
            Target::SliderAt { control, value } => view.controls[control].set_value(value),
            Target::SliderTrack(_) => continue,
            Target::Button(_) | Target::Control(_) => {}
        }
        match flow::act(&mut view, dialog, &game, &mut session, target, clicks >= 2) {
            flow::Action::Stay => {}
            flow::Action::Open(id) => {
                let popup = game.dialogs.get(&id).is_some_and(|d| d.frame.is_some());
                if popup && look.remastered {
                    // The remastered popup lies over the screen as it was
                    // drawn, which the next frame captures — without the
                    // frame rate.
                    fps.iter_mut().for_each(|mut v| *v = Visibility::Hidden);
                    let from = view.id;
                    commands.spawn(Screenshot::primary_window()).observe(
                        move |captured: On<ScreenshotCaptured>,
                              mut underlay: ResMut<Underlay>,
                              mut fps: Query<&mut Visibility, With<crate::fps::FpsText>>,
                              mut next: ResMut<NextState<Screen>>| {
                            *underlay = Underlay { from, picture: canvas_of(&captured.image) };
                            fps.iter_mut().for_each(|mut v| *v = Visibility::Inherited);
                            next.set(Screen::Dialog(id));
                        },
                    );
                } else {
                    next.set(Screen::Dialog(id));
                }
                return;
            }
            flow::Action::Quit => {
                exit.write(AppExit::Success);
                return;
            }
        }
    }
}

/// Moves on the screens that move on by themselves.
fn tick(
    time: Res<Time>,
    game: Res<Game>,
    mut view: ResMut<View>,
    mut session: ResMut<flow::Session>,
    mut next: ResMut<NextState<Screen>>,
) {
    if let flow::Action::Open(id) = turn::tick(&mut view, &game, &mut session, time.delta_secs()) {
        next.set(Screen::Dialog(id));
    }
}

/// Shows the tooltip of the bar's symbol the pointer rests on, and hides it
/// as the pointer moves on.
fn rest(time: Res<Time>, game: Res<Game>, mut view: ResMut<View>, mut resting: ResMut<Resting>) {
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
fn animate(time: Res<Time>, mut view: ResMut<View>, mut frames: ResMut<Frames>) {
    if !view.controls.iter().any(|c| c.playing && c.frames.len() > 1) {
        return;
    }
    frames.timer += time.delta_secs();
    while frames.timer >= FRAME_SECS {
        frames.timer -= FRAME_SECS;
        for state in view.controls.iter_mut().filter(|c| c.playing && !c.frames.is_empty()) {
            state.frame = (state.frame + 1) % state.frames.len();
        }
    }
}

fn blink(time: Res<Time>, view: Res<View>, mut caret: ResMut<Caret>) {
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
