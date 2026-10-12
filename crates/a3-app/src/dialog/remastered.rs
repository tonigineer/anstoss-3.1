//! The remastered look of a dialog: the artwork at the left of the 16:9
//! canvas, cleaned of the interface painted into it (see [`backdrop`]), and
//! the interface redrawn across the space right of it (see [`Layout`]); a
//! dialog with a picture of its own spreads its interface over the whole
//! width. The bars and rules of the original become shapes that run to the
//! edge of the canvas, type is set in Open Sans, lists and panels sit in
//! cards, numbers in steppers, chosen options turn black, players' boxes
//! become rounded cards, the menu bar runs along the whole bottom, and the
//! mascot buttons are smoothed.

use std::collections::HashMap;

use bevy::prelude::*;
use image::RgbaImage;

use a3_core::cycle::dialogs::*;
use a3_files::rch::{Align, Control, Dialog, Kind};
use a3_screens::controls::bitmap::Picture;
use a3_screens::controls::list::{ListState, ROW_HEIGHT, visible_rows};
use a3_screens::controls::options::{BULLET_SIZE, bullet_centre};
use a3_screens::controls::player_box::{PlayerBox, fit_ink};
use a3_screens::controls::slider::{self, SLIDER_ARROW};
use a3_screens::controls::spin::Value;
use a3_screens::controls::{ControlState, bar};
use a3_screens::pointer::{Geometry, Target};
use a3_screens::view::View;

use super::decorations::{Decoration, decorations};
use super::layout::{CARD_SCROLLBAR, Layout, wide_bar};
use super::{Caret, Frames, OpenDialog};
use crate::Fonts;
use crate::Game;
use crate::look::{CANVAS, CLASSIC_SCALE, canvas_rect, scale2x, smooth_image};

/// The dialogs with a remastered look: the start menu and the screens
/// behind it, the job offers and talks, and the season's screens with the
/// week. Any other shows the classic look in both looks.
const DIALOGS: [u16; 25] = [
    START,
    LOAD,
    HIGH_SCORES,
    NET_NAME,
    NET_DETAILS,
    NET_GAMES,
    COUNTRIES,
    PLAYERS,
    NAMES,
    CLUB,
    NATIONAL_TEAM,
    ABILITIES,
    BIRTHDAY,
    SETTINGS,
    OFFERS,
    TALKS,
    FAILED,
    SIGNED,
    NOTICE,
    QUESTION,
    WAIT,
    OFFICE,
    GRAND_OFFICE,
    LINEUP,
    DRESSING_ROOM,
];

/// Whether a dialog has a remastered look.
pub(super) fn offers(id: u16) -> bool {
    DIALOGS.contains(&id) || WEEKS.contains(&id)
}

const INK: Color = Color::srgb_u8(0x1f, 0x1f, 0x24);
const INK_SOFT: Color = Color::srgb_u8(0x55, 0x55, 0x5d);
const HOVER: Color = Color::srgb_u8(0xd3, 0x54, 0x00);
const PALE: Color = Color::srgb_u8(0xff, 0xe3, 0x8a);
const YELLOW: Color = Color::srgb_u8(0xf2, 0xbe, 0x22);
const AMBER: Color = Color::srgb_u8(0xe3, 0x9b, 0x12);
const ORANGE: Color = Color::srgb_u8(0xe8, 0x80, 0x10);
const RUST: Color = Color::srgb_u8(0xb4, 0x50, 0x00);
const RING: Color = Color::srgb_u8(0x5a, 0x5a, 0x62);
const PEARL: Color = Color::srgb_u8(0xec, 0xe9, 0xee);
const PEARL_SHADE: Color = Color::srgb_u8(0xb8, 0xb4, 0xbc);
const CHARCOAL: Color = Color::srgb_u8(0x4a, 0x4a, 0x50);
/// Cards and buttons: frosted white on the artwork.
const FROST: Color = Color::srgba(1.0, 1.0, 1.0, 0.74);
const TRACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.10);

const TEXT_SIZE: f32 = 22.0;
const TITLE_SIZE: f32 = 25.0;
const HEADER_SIZE: f32 = 19.0;
const LIST_SIZE: f32 = 21.0;
const OPTION_SIZE: f32 = 23.0;
const BOX_SIZE: f32 = 19.0;
const STEP_SIZE: f32 = 24.0;
/// The size of the bullets of options.
const BULLET_DIAMETER: f32 = BULLET_SIZE * CLASSIC_SCALE;
/// How fast hover and choice effects settle.
const EASE_RATE: f32 = 14.0;
/// Disabled buttons fade to this opacity.
const DISABLED_ALPHA: f32 = 0.4;
/// A slider: its round handle and the height of its track (canvas pixels).
const KNOB: f32 = 34.0;
const TRACK_HEIGHT: f32 = 10.0;
/// The round buttons of sliders and scrollbars.
const ROUND_BUTTON: f32 = 40.0;
const SCROLL_BUTTON: f32 = 30.0;
/// The slim track and handle of a list's scrollbar.
const SCROLL_TRACK: f32 = 6.0;
const SCROLL_THUMB_MIN: f32 = 28.0;
/// Canvas pixels between a list's rows and the edge of its card.
const CARD_PADDING: f32 = 8.0;
/// A player's box: blue with a player in it, amber without; its text and
/// the size of its icons.
const BOX_TOP: Color = Color::srgb_u8(0x74, 0x8e, 0xc4);
const BOX_BOTTOM: Color = Color::srgb_u8(0x4e, 0x66, 0x9c);
const EMPTY_TOP: Color = Color::srgb_u8(0xf4, 0xc8, 0x4c);
const EMPTY_BOTTOM: Color = Color::srgb_u8(0xdc, 0xa4, 0x22);
const BOX_NAME_SIZE: f32 = 19.0;
const BOX_DETAIL_SIZE: f32 = 17.0;
const BOX_ICON: f32 = 22.0;
/// List icons are as high as a row, nearly.
const LIST_ICON: f32 = 24.0;
/// The menu bar: a dark strip across the bottom, its symbols twice their
/// size, a little bigger when pointed at.
const BAR_TOP: Color = Color::srgb_u8(0x5c, 0x58, 0x66);
const BAR_BOTTOM: Color = Color::srgb_u8(0x2c, 0x2a, 0x32);
const SYMBOL_SIZE: f32 = 64.0;
const SYMBOL_GROWTH: f32 = 0.1;
/// Titles above this line sit in the frame, the way the game draws them.
const FRAME_TITLE: f32 = 21.0;
/// A title never starts closer than this to the left edge.
const TITLE_MARGIN: f32 = 60.0;
/// A section's bar starts this far left of its leftmost control, its stem
/// further left by this much.
const BAR_LEAD: f32 = 34.0;
const STEM_LEAD: f32 = 20.0;

/// A bulleted option: its bullet and label, and how far its hover and choice
/// effects have come.
#[derive(Component)]
pub(super) struct OptionRow {
    target: Target,
    bullet: Entity,
    label: Entity,
    hover: f32,
    active: f32,
}

/// An option drawn as a box, as in the start settings.
#[derive(Component)]
pub(super) struct OptionBox {
    target: Target,
    label: Entity,
    hover: f32,
    active: f32,
}

#[derive(Component)]
pub(super) struct Mascot {
    target: Target,
    normal: Handle<Image>,
    pointed: Handle<Image>,
    /// What shows while the button is held down; the objects of a scene
    /// light up when pointed at and only change their picture then.
    pressed: Option<Handle<Image>>,
    hover: f32,
}

/// A round button — of a stepper, slider or scrollbar — and how far its
/// hover effect has come.
#[derive(Component)]
pub(super) struct Push {
    target: Target,
    hover: f32,
}

/// The parts of a round button's mark, which take its hover colour.
#[derive(Component)]
pub(super) struct PushMark;

/// One visible row of a list, with an entity per column, and its width with
/// and without a scrollbar beside it.
#[derive(Component)]
pub(super) struct Slot {
    control: usize,
    row: usize,
    cells: Vec<Entity>,
    /// The places for icons, by their distance from the list's left edge.
    icons: Vec<(f32, Entity)>,
    full: f32,
    narrow: f32,
}

#[derive(Component)]
pub(super) struct Field {
    control: usize,
    text: Entity,
    caret: Entity,
}

/// The text of a static, which the flow may change, and its colour when
/// the pointer is not on it.
#[derive(Component)]
pub(super) struct StaticText {
    control: usize,
    ink: Color,
}

/// A bitmap, the picture it shows, and the place it fits that into.
#[derive(Component)]
pub(super) struct Bitmap {
    control: usize,
    shown: Option<Picture>,
    at: Rect,
}

/// A symbol of the menu bar: its pictures as it stands, pressed, lit and
/// greyed out, and how far its hover effect has come.
#[derive(Component)]
pub(super) struct BarSymbol {
    control: usize,
    item: usize,
    images: [Handle<Image>; 4],
    hover: f32,
}

/// The tooltip of the menu bar.
#[derive(Component)]
pub(super) struct Tip {
    text: Entity,
}

/// The icons of the lists, enlarged, by graphic id.
#[derive(Resource, Default)]
pub(super) struct ListIcons(HashMap<u16, Handle<Image>>);

/// An animation: its frames, enlarged and smoothed, and the image on top
/// that blends in the next frame.
#[derive(Component)]
pub(super) struct Flag {
    control: usize,
    frames: Vec<Handle<Image>>,
    next: Entity,
}

/// A slider's handle.
#[derive(Component)]
pub(super) struct SliderKnob {
    control: usize,
}

/// The coloured part of a slider's track, from `left` up to the handle.
#[derive(Component)]
pub(super) struct SliderFill {
    control: usize,
    left: f32,
}

/// A part of a list's scrollbar, shown while the list scrolls: an arrow, the
/// track, or the handle.
#[derive(Component)]
pub(super) struct ScrollPart {
    control: usize,
    thumb: bool,
}

fn px(value: f32) -> Val {
    Val::Px(value)
}

fn node(r: Rect) -> Node {
    canvas_rect(r.min.x, r.min.y, r.width(), r.height())
}

/// Spawns the remastered look of a dialog and returns where it put things.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn(
    commands: &mut Commands,
    stage: Entity,
    dialog: &Dialog,
    view: &View,
    background: &RgbaImage,
    wide: RgbaImage,
    game: &Game,
    fonts: &Fonts,
    images: &mut Assets<Image>,
) -> Layout {
    let layout = Layout::new(view.id, dialog, background);
    let root = commands
        .spawn((
            canvas_rect(0.0, 0.0, CANVAS.x, CANVAS.y),
            ZIndex(0),
            Pickable::IGNORE,
            ChildOf(stage),
        ))
        .id();
    commands.spawn((
        ImageNode::new(images.add(smooth_image(wide))),
        canvas_rect(0.0, 0.0, CANVAS.x, CANVAS.y),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    if !layout.scene {
        chrome(commands, root, &layout, dialog);
        for decoration in decorations(view.id) {
            decorate(commands, root, &layout, decoration, game, images);
        }
    }

    let enlarge = |images: &mut Assets<Image>, sprite: RgbaImage| {
        images.add(smooth_image(scale2x(&scale2x(&sprite))))
    };
    for (index, (control, shown)) in dialog.controls.iter().zip(&view.controls).enumerate() {
        let at = layout.rect(control);
        // Bitmaps come and go with what they show.
        if matches!(control.kind, Kind::Bitmap) {
            commands.spawn((
                Bitmap { control: index, shown: None, at },
                ImageNode::default(),
                node(at),
                Visibility::Hidden,
                Pickable::IGNORE,
                ChildOf(root),
            ));
            continue;
        }
        if shown.hidden {
            continue;
        }
        let state = &shown.state;
        let r = control.rect;
        match &control.kind {
            Kind::Static if layout.steppers.iter().any(|s| s.value == index) => {}
            Kind::Static => {
                let header = layout.headers.iter().any(|h| h.control == index);
                let style = if layout.title == Some(index) {
                    TextStyle::Title
                } else if header {
                    TextStyle::Header
                } else if r.height() >= 26.0 {
                    TextStyle::Note
                } else {
                    TextStyle::Plain
                };
                // A title at the left stays clear of the canvas edge.
                let at = if style == TextStyle::Title && at.min.x < TITLE_MARGIN {
                    Rect::new(TITLE_MARGIN, at.min.y, TITLE_MARGIN + at.width(), at.max.y)
                } else {
                    at
                };
                spawn_text(commands, root, index, at, control, view.text(index), style, fonts);
            }
            Kind::Anim => {
                let mut size = Vec2::ZERO;
                let frames = state
                    .as_anim()
                    .map_or(&[][..], |a| &a.frames)
                    .iter()
                    .map(|&id| match game.sprite(id) {
                        Ok(sprite) => {
                            size = Vec2::new(sprite.width() as f32, sprite.height() as f32);
                            enlarge(images, sprite)
                        }
                        Err(err) => {
                            warn!("graphic {id}: {err}");
                            Handle::default()
                        }
                    })
                    .collect::<Vec<_>>();
                let first = frames.first().cloned().unwrap_or_default();
                let next = commands
                    .spawn((
                        ImageNode::new(first.clone()).with_color(Color::NONE),
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .id();
                let size = size * CLASSIC_SCALE;
                commands
                    .spawn((
                        Flag { control: index, frames, next },
                        ImageNode::new(first),
                        canvas_rect(at.min.x, at.min.y, size.x, size.y),
                        Pickable::IGNORE,
                        ChildOf(root),
                    ))
                    .add_child(next);
            }
            Kind::Slider => {
                let value = state.as_value().copied().unwrap_or_default();
                spawn_slider(commands, images, root, &layout, index, control, &value)
            }
            Kind::Header => {
                let Some(header) = state.as_header() else { continue };
                for (column, title) in header.text.split("\\t").enumerate() {
                    let Some(&tab) = header.tabs.get(column) else { break };
                    commands.spawn((
                        Text::new(title),
                        font(&fonts.bold, HEADER_SIZE),
                        TextColor(INK_SOFT),
                        TextLayout::no_wrap(),
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(layout.x(r.min.x + tab) + 12.0),
                            top: px(at.min.y),
                            ..default()
                        },
                        Pickable::IGNORE,
                        ChildOf(root),
                    ));
                }
            }
            Kind::ListBox => {
                let Some(list) = state.as_list() else { continue };
                spawn_list(commands, images, root, &layout, index, control, list, fonts);
                let mut icons = ListIcons::default();
                for &(_, id) in list.row_icons.iter().flatten() {
                    icons.0.entry(id).or_insert_with(|| {
                        game.graphic(id).map_or_else(
                            |err| {
                                warn!("graphic {id}: {err}");
                                Handle::default()
                            },
                            |icon| images.add(smooth_image(scale2x(&scale2x(&icon)))),
                        )
                    });
                }
                commands.insert_resource(icons);
            }
            Kind::Player => {
                if let ControlState::Player(Some(player)) = state {
                    spawn_player(commands, images, game, root, at, player, fonts);
                }
            }
            Kind::Bar { items } => {
                spawn_bar(commands, images, game, root, &layout, index, control, items.len(), fonts)
            }
            Kind::Edit { .. } => spawn_field(commands, root, index, at, fonts),
            Kind::Options { image: 0, items } => {
                let Some(options) = state.as_options() else { continue };
                for (item, it) in items.iter().enumerate() {
                    let target = Target::Option { control: index, item };
                    let active = if options.chosen == item { 1.0 } else { 0.0 };
                    let bullet = commands
                        .spawn((
                            Node {
                                width: px(BULLET_DIAMETER),
                                height: px(BULLET_DIAMETER),
                                border: UiRect::all(px(2.0)),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            bullet_style(0.0, active),
                            Pickable::IGNORE,
                        ))
                        .id();
                    let label = options.labels.get(item).map_or(it.text.trim(), |l| l.trim());
                    let label = commands
                        .spawn((
                            Text::new(label),
                            font(&fonts.regular, OPTION_SIZE),
                            TextColor(INK),
                            TextLayout::no_wrap(),
                            Pickable::IGNORE,
                        ))
                        .id();
                    let centre = bullet_centre(control, item);
                    commands
                        .spawn((
                            OptionRow { target, bullet, label, hover: 0.0, active },
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(layout.x(centre.x) - BULLET_DIAMETER / 2.0),
                                top: px(layout.y(centre.y - 8.0)),
                                height: px(layout.y(16.0)),
                                align_items: AlignItems::Center,
                                column_gap: px(14.0),
                                ..default()
                            },
                            Pickable::IGNORE,
                            ChildOf(root),
                        ))
                        .add_children(&[bullet, label]);
                }
            }
            Kind::Options { items, .. } => {
                let Some(options) = state.as_options() else { continue };
                for (item, it) in items.iter().enumerate() {
                    let target = Target::Option { control: index, item };
                    let active = if options.chosen == item { 1.0 } else { 0.0 };
                    let (fill, border, ink) = box_style(0.0, active);
                    let label = commands
                        .spawn((
                            Text::new(options.labels.get(item).unwrap_or(&it.text).as_str()),
                            font(&fonts.regular, BOX_SIZE),
                            TextColor(ink),
                            TextLayout::justify(Justify::Center).with_no_wrap(),
                            Pickable::IGNORE,
                        ))
                        .id();
                    let rect = Rect::from_corners(r.min + it.rect.min, r.min + it.rect.max);
                    commands
                        .spawn((
                            OptionBox { target, label, hover: 0.0, active },
                            Node {
                                border: UiRect::all(px(2.0)),
                                border_radius: BorderRadius::all(px(9.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..node(layout.map(rect).inflate(-3.0))
                            },
                            BackgroundColor(fill),
                            BorderColor::all(border),
                            Pickable::IGNORE,
                            ChildOf(root),
                        ))
                        .add_child(label);
                }
            }
            Kind::Spin => match layout.steppers.iter().find(|s| s.spin == index) {
                Some(stepper) => {
                    let minus = Target::Spin { control: index, up: false };
                    let plus = Target::Spin { control: index, up: true };
                    push_button(commands, images, root, minus, stepper.minus, Mark::Minus);
                    push_button(commands, images, root, plus, stepper.plus, Mark::Plus);
                    let pill = commands
                        .spawn((
                            Node {
                                border: UiRect::all(px(2.0)),
                                border_radius: BorderRadius::all(px(10.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..node(stepper.number)
                            },
                            BackgroundColor(Color::WHITE.with_alpha(0.9)),
                            BorderColor::all(RING.with_alpha(0.35)),
                            Pickable::IGNORE,
                            ChildOf(root),
                        ))
                        .id();
                    commands.spawn((
                        StaticText { control: stepper.value, ink: INK },
                        Text::new(view.text(stepper.value)),
                        font(&fonts.bold, STEP_SIZE),
                        TextColor(INK),
                        TextLayout::no_wrap(),
                        Pickable::IGNORE,
                        ChildOf(pill),
                    ));
                }
                None => {
                    let (up, down) = layout.spin(index, control);
                    let size = Vec2::splat(up.height().min(SCROLL_BUTTON));
                    let up_target = Target::Spin { control: index, up: true };
                    let down_target = Target::Spin { control: index, up: false };
                    let up_rect = Rect::from_center_size(up.center(), size);
                    let down_rect = Rect::from_center_size(down.center(), size);
                    push_button(commands, images, root, up_target, up_rect, Mark::Up);
                    push_button(commands, images, root, down_target, down_rect, Mark::Down);
                }
            },
            Kind::Button { normal, hover } => {
                let sprite =
                    |id: u16| game.sprite(id).inspect_err(|err| warn!("graphic {id}: {err}")).ok();
                let mut smooth = |sprite: Option<RgbaImage>| {
                    sprite.map_or_else(Handle::default, |sprite| {
                        images.add(smooth_image(scale2x(&scale2x(&sprite))))
                    })
                };
                let (pointed, pressed) = if layout.scene {
                    (smooth(sprite(*normal).map(|s| lighten(&s))), Some(smooth(sprite(*hover))))
                } else {
                    (smooth(sprite(*hover)), None)
                };
                let normal = smooth(sprite(*normal));
                commands.spawn((
                    Mascot {
                        target: Target::Button(index),
                        normal: normal.clone(),
                        pointed,
                        pressed,
                        hover: 0.0,
                    },
                    ImageNode::new(normal),
                    node(at),
                    UiTransform::default(),
                    Pickable::IGNORE,
                    ChildOf(root),
                ));
            }
            _ => {}
        }
    }
    layout
}

/// The bars and rules of the original, as shapes that run to the edge: the
/// orange rule along the top, the title ribbon, a yellow bar and stem for
/// every section, and the stripes behind the buttons.
fn chrome(commands: &mut Commands, root: Entity, layout: &Layout, dialog: &Dialog) {
    let edge = CANVAS.x;
    shape(
        commands,
        root,
        to_right(&[(RUST, 0.0), (ORANGE, 50.0), (RUST, 100.0)]),
        canvas_rect(0.0, layout.y(21.0), edge, layout.y(2.0)),
    );
    shape(
        commands,
        root,
        to_right(&[(PALE.with_alpha(0.7), 0.0), (PALE.with_alpha(0.7), 100.0)]),
        canvas_rect(0.0, layout.y(23.0), edge, layout.y(1.0)),
    );

    // A title in the frame, above the rule, needs no ribbon.
    if let Some(title) = layout.title.filter(|&t| dialog.controls[t].rect.min.y >= FRAME_TITLE) {
        let text = layout.rect(&dialog.controls[title]);
        let left = (text.min.x - 260.0).max(0.0);
        let (top, height) = (layout.y(36.0), layout.y(24.0));
        let solid = left + 0.3 * (edge - left);
        commands.spawn((
            BoxShadow::new(Color::BLACK.with_alpha(0.2), px(0.0), px(5.0), px(0.0), px(11.0)),
            canvas_rect(solid, top, edge - solid, height),
            Pickable::IGNORE,
            ChildOf(root),
        ));
        shape(
            commands,
            root,
            to_right(&[(PALE.with_alpha(0.0), 0.0), (PALE, 17.0), (YELLOW, 40.0), (AMBER, 100.0)]),
            canvas_rect(left, top, edge - left, height),
        );
        let twin = left + 0.72 * (edge - left);
        shape(
            commands,
            root,
            to_right(&[(YELLOW, 0.0), (AMBER, 100.0)]),
            Node {
                border_radius: BorderRadius::px(7.0, 0.0, 0.0, 7.0),
                ..canvas_rect(twin, layout.y(31.0), edge - twin, layout.y(5.0))
            },
        );
    }

    for header in &layout.headers {
        let text = layout.rect(&dialog.controls[header.control]);
        let left = layout.x(header.left) - BAR_LEAD;
        let bottom = header.bottom;
        let (top, height) = (text.min.y - 5.0, text.height() + 8.0);
        let right = (layout.right + 80.0).min(edge);
        // The rule along the top of the bar, its shadow, the bar and the stem.
        shape(
            commands,
            root,
            to_right(&[(AMBER, 0.0), (YELLOW, 55.0), (YELLOW.with_alpha(0.0), 100.0)]),
            canvas_rect(left - 40.0, top - 4.0, right - left + 40.0, 4.0),
        );
        shape(
            commands,
            root,
            to_right(&[(Color::BLACK.with_alpha(0.2), 0.0), (Color::BLACK.with_alpha(0.0), 100.0)]),
            canvas_rect(left, top + height, (right - left) * 0.6, 6.0),
        );
        shape(
            commands,
            root,
            to_right(&[(YELLOW, 0.0), (PALE, 45.0), (PALE.with_alpha(0.0), 100.0)]),
            canvas_rect(left, top, right - left, height),
        );
        let stem_bottom = layout.y(bottom) + 10.0;
        if stem_bottom > top + height + 20.0 {
            shape(
                commands,
                root,
                LinearGradient::to_bottom(stops(&[
                    (AMBER, 0.0),
                    (YELLOW, 25.0),
                    (YELLOW.with_alpha(0.0), 100.0),
                ]))
                .into(),
                canvas_rect(left - STEM_LEAD, top - 4.0, 7.0, stem_bottom - top + 4.0),
            );
        }
    }

    // The stripes behind the buttons, trailing off to the right edge.
    let buttons = dialog
        .controls
        .iter()
        .filter(|c| matches!(c.kind, Kind::Button { .. }))
        .map(|c| layout.rect(c).min.x)
        .reduce(f32::min);
    if let Some(left) = buttons {
        let left = left - 140.0;
        for stripe in 0..6 {
            let top = layout.y(448.0 + 5.0 * stripe as f32);
            shape(
                commands,
                root,
                to_right(&[
                    (ORANGE.with_alpha(0.0), 0.0),
                    (ORANGE.with_alpha(0.85), 30.0),
                    (AMBER, 100.0),
                ]),
                Node {
                    border_radius: BorderRadius::MAX,
                    ..canvas_rect(left, top, edge - left, layout.y(2.6))
                },
            );
        }
    }
}

/// A gradient-filled rectangle.
fn shape(commands: &mut Commands, root: Entity, fill: BackgroundGradient, node: Node) {
    commands.spawn((fill, node, Pickable::IGNORE, ChildOf(root)));
}

#[derive(Clone, Copy, PartialEq)]
enum TextStyle {
    Title,
    Header,
    Note,
    Plain,
}

/// A static: aligned within its place as in the dialog, wrapped if it is a
/// note with room for more lines. Text justification only lines up lines
/// against each other, so the alignment within the place comes from its flex
/// box.
#[allow(clippy::too_many_arguments)]
fn spawn_text(
    commands: &mut Commands,
    root: Entity,
    index: usize,
    at: Rect,
    control: &Control,
    text: &str,
    style: TextStyle,
    fonts: &Fonts,
) {
    let (justify, place) = match control.align {
        Align::Left => (Justify::Left, JustifyContent::FlexStart),
        Align::Centre => (Justify::Center, JustifyContent::Center),
        Align::Right => (Justify::Right, JustifyContent::FlexEnd),
    };
    let note = style == TextStyle::Note;
    let layout = if note {
        TextLayout::justify(justify)
    } else {
        TextLayout::justify(justify).with_no_wrap()
    };
    let face = |bold: bool| if bold { &fonts.bold } else { &fonts.regular };
    let (face, size, ink) = match style {
        TextStyle::Title => (&fonts.bold, TITLE_SIZE, INK),
        TextStyle::Header => (&fonts.bold, TEXT_SIZE, INK),
        TextStyle::Note => (face(control.bold), TEXT_SIZE - 1.0, INK_SOFT),
        TextStyle::Plain => (face(control.bold), TEXT_SIZE, INK),
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(at.min.x),
                top: px(at.min.y),
                width: px(at.width()),
                min_height: px(at.height()),
                justify_content: place,
                align_items: if note { AlignItems::FlexStart } else { AlignItems::Center },
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .with_child((
            StaticText { control: index, ink },
            Text::new(text),
            font(face, size),
            TextColor(ink),
            layout,
            Pickable::IGNORE,
        ));
}

/// A text field: white, its border turning orange while typed into.
fn spawn_field(commands: &mut Commands, root: Entity, index: usize, at: Rect, fonts: &Fonts) {
    let field = commands
        .spawn((
            Node {
                border: UiRect::all(px(2.0)),
                border_radius: BorderRadius::all(px(9.0)),
                padding: UiRect::horizontal(px(10.0)),
                align_items: AlignItems::Center,
                ..node(at.inflate(3.0))
            },
            BackgroundColor(Color::WHITE.with_alpha(0.88)),
            BorderColor::all(RING.with_alpha(0.5)),
            BoxShadow::new(Color::BLACK.with_alpha(0.08), px(0.0), px(2.0), px(0.0), px(8.0)),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let text = commands
        .spawn((
            Text::default(),
            font(&fonts.regular, TEXT_SIZE),
            TextColor(INK),
            TextLayout::no_wrap(),
            Pickable::IGNORE,
            ChildOf(field),
        ))
        .id();
    let caret = commands
        .spawn((
            Node {
                width: px(2.0),
                height: Val::Percent(60.0),
                margin: UiRect::left(px(1.0)),
                ..default()
            },
            BackgroundColor(INK),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(field),
        ))
        .id();
    commands.entity(field).insert(Field { control: index, text, caret });
}

/// A slider: round arrow buttons at its ends, a track that fills with colour
/// up to a round handle.
fn spawn_slider(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    root: Entity,
    layout: &Layout,
    index: usize,
    control: &Control,
    value: &Value,
) {
    let r = layout.rect(control);
    let arrow = SLIDER_ARROW * layout.zoom();
    let middle = r.center().y;
    let button = |x: f32| Rect::from_center_size(Vec2::new(x, middle), Vec2::splat(ROUND_BUTTON));
    let down = Target::SliderStep { control: index, up: false };
    let up = Target::SliderStep { control: index, up: true };
    push_button(commands, images, root, down, button(r.min.x + arrow / 2.0), Mark::Left);
    push_button(commands, images, root, up, button(r.max.x - arrow / 2.0), Mark::Right);

    let (left, right) = (r.min.x + arrow + 4.0, r.max.x - arrow - 4.0);
    let top = middle - TRACK_HEIGHT / 2.0;
    commands.spawn((
        Node {
            border_radius: BorderRadius::MAX,
            ..canvas_rect(left, top, right - left, TRACK_HEIGHT)
        },
        BackgroundColor(TRACK),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    let centre = layout.map(slider::handle(control, value)).center().x;
    commands.spawn((
        SliderFill { control: index, left },
        Node {
            border_radius: BorderRadius::MAX,
            ..canvas_rect(left, top, (centre - left).max(0.0), TRACK_HEIGHT)
        },
        to_right(&[(YELLOW, 0.0), (AMBER, 100.0)]),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        SliderKnob { control: index },
        Node {
            border: UiRect::all(px(3.0)),
            border_radius: BorderRadius::MAX,
            ..canvas_rect(centre - KNOB / 2.0, middle - KNOB / 2.0, KNOB, KNOB)
        },
        BackgroundColor(INK),
        BorderColor::all(Color::WHITE),
        BoxShadow::new(Color::BLACK.with_alpha(0.35), px(0.0), px(2.0), px(0.0), px(6.0)),
        Pickable::IGNORE,
        ChildOf(root),
    ));
}

/// A list in a card: its rows, and a scrollbar while it has more rows than
/// it shows.
#[allow(clippy::too_many_arguments)]
fn spawn_list(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    root: Entity,
    layout: &Layout,
    index: usize,
    control: &Control,
    state: &ListState,
    fonts: &Fonts,
) {
    let r = layout.rect(control);
    commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            border_radius: BorderRadius::all(px(14.0)),
            ..node(r.inflate(CARD_PADDING))
        },
        BackgroundColor(FROST),
        BorderColor::all(RING.with_alpha(0.22)),
        BoxShadow::new(Color::BLACK.with_alpha(0.13), px(0.0), px(4.0), px(0.0), px(16.0)),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    let (full, narrow) = (r.width(), r.width() - CARD_SCROLLBAR - 6.0);
    let height = layout.y(ROW_HEIGHT);
    for row in 0..visible_rows(control) {
        let top = layout.y(control.rect.min.y + 1.0 + row as f32 * ROW_HEIGHT);
        let width = if state.scrolls(control) { narrow } else { full };
        let slot = commands
            .spawn((
                Node {
                    border_radius: BorderRadius::all(px(8.0)),
                    overflow: Overflow::clip(),
                    ..canvas_rect(r.min.x, top, width, height)
                },
                BackgroundColor(Color::NONE),
                Pickable::IGNORE,
                ChildOf(root),
            ))
            .id();
        let text = || {
            (
                Text::default(),
                font(&fonts.regular, LIST_SIZE),
                TextColor(INK),
                TextLayout::no_wrap(),
                Pickable::IGNORE,
            )
        };
        let cells = state
            .tabs
            .iter()
            .enumerate()
            .map(|(column, &tab)| {
                let left = tab * layout.zoom() + 12.0;
                match state.aligns.get(column).copied().flatten() {
                    // Centred or to the right in a width of its own.
                    Some((align, width)) => {
                        let place = commands
                            .spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(left),
                                    width: px(width * layout.zoom()),
                                    height: Val::Percent(100.0),
                                    justify_content: if align == Align::Right {
                                        JustifyContent::FlexEnd
                                    } else {
                                        JustifyContent::Center
                                    },
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                Pickable::IGNORE,
                                ChildOf(slot),
                            ))
                            .id();
                        commands.spawn((text(), ChildOf(place))).id()
                    }
                    None => commands
                        .spawn((
                            text(),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(left),
                                top: px((height - LIST_SIZE * 1.3) / 2.0),
                                ..default()
                            },
                            ChildOf(slot),
                        ))
                        .id(),
                }
            })
            .collect();
        let mut columns: Vec<f32> = state.row_icons.iter().flatten().map(|&(x, _)| x).collect();
        columns.sort_by(f32::total_cmp);
        columns.dedup();
        let icons = columns
            .into_iter()
            .map(|x| {
                let icon = commands
                    .spawn((
                        ImageNode::default(),
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(x * layout.zoom() + 12.0),
                            top: px((height - LIST_ICON) / 2.0),
                            width: px(LIST_ICON),
                            height: px(LIST_ICON),
                            ..default()
                        },
                        Visibility::Hidden,
                        Pickable::IGNORE,
                        ChildOf(slot),
                    ))
                    .id();
                (x, icon)
            })
            .collect();
        commands.entity(slot).insert(Slot { control: index, row, cells, icons, full, narrow });
    }

    let visibility =
        if state.scrolls(control) { Visibility::Inherited } else { Visibility::Hidden };
    let (up, down) = layout.scroll(control);
    let track = Rect::new(
        up.center().x - SCROLL_TRACK / 2.0,
        up.max.y + 4.0,
        up.center().x + SCROLL_TRACK / 2.0,
        down.min.y - 4.0,
    );
    commands.spawn((
        ScrollPart { control: index, thumb: false },
        Node { border_radius: BorderRadius::MAX, ..node(track) },
        BackgroundColor(TRACK),
        visibility,
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        ScrollPart { control: index, thumb: true },
        Node { border_radius: BorderRadius::MAX, ..node(card_thumb(layout, control, state)) },
        BackgroundColor(INK_SOFT),
        visibility,
        Pickable::IGNORE,
        ChildOf(root),
    ));
    for (rect, up, mark) in [(up, true, Mark::Up), (down, false, Mark::Down)] {
        let rect = Rect::from_center_size(rect.center(), Vec2::splat(SCROLL_BUTTON));
        let target = Target::Scroll { control: index, up };
        let button = push_button(commands, images, root, target, rect, mark);
        commands.entity(button).insert((ScrollPart { control: index, thumb: false }, visibility));
    }
}

/// The handle of a list card's scrollbar: as long as the share of rows
/// shown, as far down as the list is scrolled.
fn card_thumb(layout: &Layout, control: &Control, state: &ListState) -> Rect {
    let (up, down) = layout.scroll(control);
    let (top, bottom) = (up.max.y + 4.0, down.min.y - 4.0);
    let total = state.rows.len().max(1) as f32;
    let visible = visible_rows(control) as f32;
    let room = bottom - top;
    let length = (room * visible / total).clamp(SCROLL_THUMB_MIN.min(room), room);
    let offset = (room - length) * state.scroll as f32 / (total - visible).max(1.0);
    let x = up.center().x;
    Rect::new(x - SCROLL_TRACK / 2.0, top + offset, x + SCROLL_TRACK / 2.0, top + offset + length)
}

#[derive(Clone, Copy)]
enum Mark {
    Minus,
    Plus,
    Up,
    Down,
    Left,
    Right,
}

/// A round frosted button with a mark: a minus, a plus, or a chevron.
fn push_button(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    root: Entity,
    target: Target,
    at: Rect,
    mark: Mark,
) -> Entity {
    let size = at.width().min(at.height());
    let button = commands
        .spawn((
            Node {
                border: UiRect::all(px(2.0)),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..node(Rect::from_center_size(at.center(), Vec2::splat(size)))
            },
            BackgroundColor(FROST),
            BorderColor::all(RING.with_alpha(0.4)),
            BoxShadow::new(Color::BLACK.with_alpha(0.12), px(0.0), px(2.0), px(0.0), px(6.0)),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let stroke = (size / 14.0).max(2.0);
    match mark {
        Mark::Minus | Mark::Plus => {
            let bar = size * 0.42;
            let bars: &[(f32, f32)] = if matches!(mark, Mark::Plus) {
                &[(bar, stroke), (stroke, bar)]
            } else {
                &[(bar, stroke)]
            };
            for &(width, height) in bars {
                commands.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(width),
                        height: px(height),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(INK),
                    PushMark,
                    Pickable::IGNORE,
                    ChildOf(button),
                ));
            }
        }
        Mark::Up | Mark::Down | Mark::Left | Mark::Right => {
            let side = size * 0.42;
            commands.spawn((
                ImageNode::new(images.add(chevron(mark))).with_color(INK),
                Node { width: px(side), height: px(side), ..default() },
                PushMark,
                Pickable::IGNORE,
                ChildOf(button),
            ));
        }
    }
    commands.entity(button).insert(Push { target, hover: 0.0 });
    button
}

/// A chevron pointing the way of `mark`, white on transparent so it can be
/// tinted, its stroke smoothed.
fn chevron(mark: Mark) -> Image {
    const SIZE: u32 = 64;
    const STROKE: f32 = 7.0;
    // Pointing right: the tip, and the two ends of its arms.
    let (tip, ends) = (Vec2::new(44.0, 32.0), [Vec2::new(22.0, 10.0), Vec2::new(22.0, 54.0)]);
    let turn = |p: Vec2| {
        let c = Vec2::splat(SIZE as f32 / 2.0);
        let d = p - c;
        c + match mark {
            Mark::Left => -d,
            Mark::Up => Vec2::new(d.y, -d.x),
            Mark::Down => Vec2::new(-d.y, d.x),
            _ => d,
        }
    };
    let (tip, ends) = (turn(tip), ends.map(turn));
    let pixels = RgbaImage::from_fn(SIZE, SIZE, |x, y| {
        let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
        let distance = ends
            .iter()
            .map(|&end| {
                let (a, b) = (end, tip);
                let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
                p.distance(a + (b - a) * t)
            })
            .fold(f32::MAX, f32::min);
        let alpha = (STROKE / 2.0 - distance + 0.5).clamp(0.0, 1.0);
        image::Rgba([255, 255, 255, (alpha * 255.0) as u8])
    });
    smooth_image(pixels)
}

/// Whether the remastered look on the canvas is the open dialog's: after an
/// input the next screen is open a frame or two before it is drawn.
pub(super) fn drawn(view: Res<OpenDialog>, layout: Option<Res<Layout>>) -> bool {
    layout.is_some_and(|layout| layout.id == view.id)
}

/// List rows and field contents follow the dialog's state.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn fill(
    view: Res<OpenDialog>,
    caret: Res<Caret>,
    icons: Option<Res<ListIcons>>,
    mut slots: Query<(&Slot, &mut BackgroundColor)>,
    mut fields: Query<(&Field, &mut BorderColor)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
    mut carets: Query<&mut Visibility, Without<ImageNode>>,
    mut pictures: Query<(&mut ImageNode, &mut Visibility)>,
) {
    if !(view.is_changed() || caret.is_changed()) {
        return;
    }
    for (slot, mut fill) in &mut slots {
        let Some(state) = view.list(slot.control) else { continue };
        let at = state.scroll + slot.row;
        let line = state.rows.get(at);
        let selected = line.is_some() && state.selected == Some(at);
        let pointed = line.is_some()
            && view.hovered == Some(Target::Row { control: slot.control, row: slot.row });
        let own = state.inks.get(at).copied().flatten().map(|[r, g, b]| Color::srgb_u8(r, g, b));
        let (back, ink) = match (selected, pointed) {
            (true, _) => (INK, Color::WHITE),
            (false, true) => (PALE.with_alpha(0.55), HOVER),
            _ => (Color::NONE, own.unwrap_or(INK)),
        };
        let row_icons = state.row_icons.get(at).map_or(&[][..], Vec::as_slice);
        for &(x, entity) in &slot.icons {
            let Ok((mut image, mut visibility)) = pictures.get_mut(entity) else { continue };
            let icon = row_icons
                .iter()
                .find(|&&(ix, _)| ix == x)
                .and_then(|&(_, id)| icons.as_ref()?.0.get(&id).cloned());
            *visibility = if icon.is_some() { Visibility::Inherited } else { Visibility::Hidden };
            if let Some(icon) = icon
                && image.image != icon
            {
                image.image = icon;
            }
        }
        fill.0 = back;
        let mut cells = line.map(|l| l.split('\t')).into_iter().flatten();
        for &cell in &slot.cells {
            if let Ok((mut text, mut color)) = texts.get_mut(cell) {
                text.0 = cells.next().unwrap_or_default().to_owned();
                color.0 = ink;
            }
        }
    }
    for (field, mut border) in &mut fields {
        let focused = view.focus == Some(field.control);
        border.set_all(if focused { ORANGE } else { RING.with_alpha(0.5) });
        if let Ok((mut text, _)) = texts.get_mut(field.text) {
            text.0 = view.text(field.control).to_owned();
        }
        if let Ok(mut visibility) = carets.get_mut(field.caret) {
            *visibility =
                if focused && caret.on { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

/// Everything else the flow may change: texts, option labels, slider
/// handles, scrollbars and disabled buttons.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    view: Res<OpenDialog>,
    layout: Option<Res<Layout>>,
    game: Res<Game>,
    mut statics: Query<(&StaticText, &mut Text, &mut TextColor)>,
    rows: Query<&OptionRow>,
    boxes: Query<&OptionBox>,
    mut labels: Query<&mut Text, Without<StaticText>>,
    mut mascots: Query<(&Mascot, &mut ImageNode)>,
    knobs: Query<(Entity, &SliderKnob)>,
    fills: Query<(Entity, &SliderFill)>,
    scrollbars: Query<(Entity, &ScrollPart)>,
    slots: Query<(Entity, &Slot)>,
    mut moving: Query<
        (&mut Node, Option<&mut Visibility>),
        Or<(With<SliderKnob>, With<SliderFill>, With<ScrollPart>, With<Slot>)>,
    >,
) {
    let Some(layout) = layout else { return };
    if !view.is_changed() {
        return;
    }
    let layout: &Layout = &layout;
    let Some(dialog) = game.dialogs.get(&view.id) else { return };
    for (shown, mut text, mut color) in &mut statics {
        let wanted = view.text(shown.control);
        if text.0 != wanted {
            text.0 = wanted.to_owned();
        }
        let pointed = view.hovered == Some(Target::Control(shown.control));
        let ink = if pointed { HOVER } else { shown.ink };
        if color.0 != ink {
            color.0 = ink;
        }
    }
    let options =
        rows.iter().map(|r| (r.target, r.label)).chain(boxes.iter().map(|b| (b.target, b.label)));
    for (target, label) in options {
        let Target::Option { control, item } = target else { continue };
        let Some(wanted) = view.options(control).and_then(|o| o.labels.get(item)) else {
            continue;
        };
        let wanted = wanted.trim();
        if let Ok(mut text) = labels.get_mut(label)
            && text.0 != wanted
        {
            text.0 = wanted.to_owned();
        }
    }
    for (mascot, mut image) in &mut mascots {
        let Target::Button(button) = mascot.target else { continue };
        let alpha = if view.disabled(button) { DISABLED_ALPHA } else { 1.0 };
        if image.color.alpha() != alpha {
            image.color = Color::WHITE.with_alpha(alpha);
        }
    }
    let handle = |control: usize| {
        let value = view.value(control).copied().unwrap_or_default();
        layout.map(slider::handle(&dialog.controls[control], &value)).center().x
    };
    for (entity, knob) in &knobs {
        if let Ok((mut node, _)) = moving.get_mut(entity) {
            node.left = px(handle(knob.control) - KNOB / 2.0);
        }
    }
    for (entity, fill) in &fills {
        if let Ok((mut node, _)) = moving.get_mut(entity) {
            node.width = px((handle(fill.control) - fill.left).max(0.0));
        }
    }
    // Rows make room for a scrollbar while there is one.
    for (entity, slot) in &slots {
        let control = &dialog.controls[slot.control];
        let scrolls = view.list(slot.control).is_some_and(|l| l.scrolls(control));
        let wanted = px(if scrolls { slot.narrow } else { slot.full });
        if let Ok((mut node, _)) = moving.get_mut(entity)
            && node.width != wanted
        {
            node.width = wanted;
        }
    }
    for (entity, part) in &scrollbars {
        let control = &dialog.controls[part.control];
        let Some(state) = view.list(part.control) else { continue };
        let Ok((mut node, visibility)) = moving.get_mut(entity) else { continue };
        let wanted =
            if state.scrolls(control) { Visibility::Inherited } else { Visibility::Hidden };
        if let Some(mut visibility) = visibility
            && *visibility != wanted
        {
            *visibility = wanted;
        }
        if part.thumb {
            let thumb = card_thumb(layout, control, state);
            node.top = px(thumb.min.y);
            node.height = px(thumb.height());
        }
    }
}

/// Animations show their frame with the next one blending in on top.
pub(super) fn animate_flags(
    view: Res<OpenDialog>,
    frames: Res<Frames>,
    mut flags: Query<(&Flag, &mut ImageNode)>,
    mut nexts: Query<&mut ImageNode, Without<Flag>>,
) {
    if !(view.is_changed() || frames.is_changed()) {
        return;
    }
    for (flag, mut image) in &mut flags {
        let Some(anim) = view.anim(flag.control) else { continue };
        let (shown, next, blend) = anim.showing(frames.phase());
        if let Some(frame) = flag.frames.get(shown)
            && image.image != *frame
        {
            image.image = frame.clone();
        }
        if let (Ok(mut over), Some(frame)) = (nexts.get_mut(flag.next), flag.frames.get(next)) {
            if over.image != *frame {
                over.image = frame.clone();
            }
            over.color = Color::WHITE.with_alpha(blend);
        }
    }
}

pub(super) fn ease_options(
    time: Res<Time>,
    view: Res<OpenDialog>,
    mut rows: Query<&mut OptionRow>,
    mut labels: Query<&mut TextColor>,
    mut bullets: Query<(&mut BackgroundGradient, &mut BorderColor, &mut BoxShadow)>,
) {
    let ease = 1.0 - (-EASE_RATE * time.delta_secs()).exp();
    for mut row in &mut rows {
        let (hover, active) = targets(&view, row.target);
        let (hover, active) =
            (approach(row.hover, hover, ease), approach(row.active, active, ease));
        if (hover, active) == (row.hover, row.active) {
            continue;
        }
        (row.hover, row.active) = (hover, active);
        if let Ok(mut color) = labels.get_mut(row.label) {
            color.0 = mix(INK, HOVER, hover);
        }
        if let Ok((mut fill, mut border, mut shadow)) = bullets.get_mut(row.bullet) {
            (*fill, *border, *shadow) = bullet_style(hover, active);
        }
    }
}

pub(super) fn ease_boxes(
    time: Res<Time>,
    view: Res<OpenDialog>,
    mut boxes: Query<(&mut OptionBox, &mut BackgroundColor, &mut BorderColor)>,
    mut labels: Query<&mut TextColor>,
) {
    let ease = 1.0 - (-EASE_RATE * time.delta_secs()).exp();
    for (mut option, mut fill, mut border) in &mut boxes {
        let (hover, active) = targets(&view, option.target);
        let (hover, active) =
            (approach(option.hover, hover, ease), approach(option.active, active, ease));
        if (hover, active) == (option.hover, option.active) {
            continue;
        }
        (option.hover, option.active) = (hover, active);
        let (back, edge, ink) = box_style(hover, active);
        fill.0 = back;
        border.set_all(edge);
        if let Ok(mut color) = labels.get_mut(option.label) {
            color.0 = ink;
        }
    }
}

/// Round buttons warm up under the pointer: their face turns pale, their
/// edge and mark orange.
#[allow(clippy::type_complexity)]
pub(super) fn ease_pushes(
    time: Res<Time>,
    view: Res<OpenDialog>,
    mut pushes: Query<
        (&mut Push, &mut BackgroundColor, &mut BorderColor, &Children),
        Without<PushMark>,
    >,
    mut marks: Query<(Option<&mut BackgroundColor>, Option<&mut ImageNode>), With<PushMark>>,
) {
    let ease = 1.0 - (-EASE_RATE * time.delta_secs()).exp();
    for (mut push, mut face, mut edge, children) in &mut pushes {
        let hover = approach(push.hover, targets(&view, push.target).0, ease);
        if hover == push.hover {
            continue;
        }
        push.hover = hover;
        face.0 = mix(FROST, PALE, hover);
        edge.set_all(mix(RING.with_alpha(0.4), HOVER, hover));
        let ink = mix(INK, HOVER, hover);
        for &child in children {
            // A chevron is an image to tint; the bars of minus and plus
            // are filled. Every node has a fill, so the image decides.
            match marks.get_mut(child) {
                Ok((_, Some(mut image))) => image.color = ink,
                Ok((Some(mut fill), None)) => fill.0 = ink,
                _ => {}
            }
        }
    }
}

pub(super) fn ease_buttons(
    time: Res<Time>,
    view: Res<OpenDialog>,
    mut buttons: Query<(&mut Mascot, &mut ImageNode, &mut UiTransform)>,
) {
    let ease = 1.0 - (-EASE_RATE * time.delta_secs()).exp();
    for (mut mascot, mut image, mut transform) in &mut buttons {
        let disabled = match mascot.target {
            Target::Button(button) => view.disabled(button),
            _ => false,
        };
        let wanted = if disabled { 0.0 } else { targets(&view, mascot.target).0 };
        let hover = approach(mascot.hover, wanted, ease);
        let held = view.pressed == Some(mascot.target) && hover > 0.5;
        let shown = match &mascot.pressed {
            Some(pressed) if held => pressed,
            _ if hover > 0.5 => &mascot.pointed,
            _ => &mascot.normal,
        };
        if image.image != *shown {
            image.image = shown.clone();
        }
        if hover == mascot.hover {
            continue;
        }
        mascot.hover = hover;
        // Objects of a scene stay on their picture.
        if mascot.pressed.is_none() {
            transform.scale = Vec2::splat(1.0 + 0.06 * hover);
        }
    }
}

/// Bitmaps show what the view says, and only when it says so.
pub(super) fn show_bitmaps(
    view: Res<OpenDialog>,
    game: Res<Game>,
    mut images: ResMut<Assets<Image>>,
    mut bitmaps: Query<(&mut Bitmap, &mut ImageNode, &mut Node, &mut Visibility)>,
) {
    if !view.is_changed() {
        return;
    }
    for (mut bitmap, mut image, mut node, mut visibility) in &mut bitmaps {
        let shown = &view.controls[bitmap.control];
        let picture = shown.state.as_bitmap().and_then(|b| b.picture.as_ref());
        if bitmap.shown.as_ref() != picture {
            bitmap.shown = picture.cloned();
            let loaded =
                picture.and_then(|p| p.load(&game).inspect_err(|err| warn!("{p:?}: {err}")).ok());
            if let Some(picture) = &loaded {
                // Wider than the classic look, the place keeps the picture's shape.
                let fitted =
                    fit(bitmap.at, Vec2::new(picture.width() as f32, picture.height() as f32));
                *node = canvas_rect(fitted.min.x, fitted.min.y, fitted.width(), fitted.height());
            }
            image.image = loaded.map_or_else(Handle::default, |picture| {
                images.add(smooth_image(scale2x(&scale2x(&picture))))
            });
        }
        let wanted = if shown.hidden || picture.is_none() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// The biggest rectangle of a shape that fits into a place, in its middle.
fn fit(at: Rect, size: Vec2) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return at;
    }
    let scale = (at.width() / size.x).min(at.height() / size.y);
    Rect::from_center_size(at.center(), size * scale)
}

/// Draws what a screen has painted into its classic picture.
fn decorate(
    commands: &mut Commands,
    root: Entity,
    layout: &Layout,
    decoration: Decoration,
    game: &Game,
    images: &mut Assets<Image>,
) {
    match decoration {
        Decoration::Picture { picture, at, stretch } => {
            let Ok(pixels) = picture.load(game).inspect_err(|err| warn!("{picture:?}: {err}"))
            else {
                return;
            };
            let place = layout.map(at);
            let size = Vec2::new(pixels.width() as f32, pixels.height() as f32);
            let (place, mode) = if stretch {
                (place, NodeImageMode::Stretch)
            } else {
                (fit(place, size), NodeImageMode::Auto)
            };
            commands.spawn((
                ImageNode {
                    image_mode: mode,
                    ..ImageNode::new(images.add(smooth_image(scale2x(&scale2x(&pixels)))))
                },
                node(place),
                BoxShadow::new(Color::BLACK.with_alpha(0.18), px(0.0), px(4.0), px(0.0), px(14.0)),
                Pickable::IGNORE,
                ChildOf(root),
            ));
        }
        Decoration::Card(at) => {
            commands.spawn((
                Node {
                    border: UiRect::all(px(1.0)),
                    border_radius: BorderRadius::all(px(14.0)),
                    ..node(layout.map(at))
                },
                BackgroundColor(FROST),
                BorderColor::all(RING.with_alpha(0.22)),
                BoxShadow::new(Color::BLACK.with_alpha(0.13), px(0.0), px(4.0), px(0.0), px(16.0)),
                Pickable::IGNORE,
                ChildOf(root),
            ));
        }
        Decoration::Band(at) => shape(
            commands,
            root,
            to_right(&[(AMBER, 0.0), (YELLOW, 40.0), (PALE.with_alpha(0.6), 100.0)]),
            Node { border_radius: BorderRadius::all(px(10.0)), ..node(layout.map(at)) },
        ),
    }
}

/// A player's box: a rounded card, blue with his name and below it his
/// abilities, position and strength, amber with the position's name when
/// the spot is empty.
fn spawn_player(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    game: &Game,
    root: Entity,
    at: Rect,
    player: &PlayerBox,
    fonts: &Fonts,
) {
    let (top, bottom) =
        if player.filled { (BOX_TOP, BOX_BOTTOM) } else { (EMPTY_TOP, EMPTY_BOTTOM) };
    let card = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(px(8.0)),
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::all(px(9.0)),
                ..node(at)
            },
            BackgroundGradient::from(LinearGradient::to_bottom(stops(&[
                (top, 0.0),
                (bottom, 100.0),
            ]))),
            BorderColor::all(Color::WHITE.with_alpha(0.5)),
            BoxShadow::new(Color::BLACK.with_alpha(0.3), px(0.0), px(3.0), px(0.0), px(8.0)),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    if !player.filled {
        commands.spawn((
            Text::new(player.name.as_str()),
            font(&fonts.bold, BOX_NAME_SIZE),
            TextColor(INK),
            TextLayout::justify(Justify::Center),
            Pickable::IGNORE,
            ChildOf(card),
        ));
        return;
    }
    let [red, green, blue] = fit_ink(player.fit);
    commands.spawn((
        Text::new(player.name.as_str()),
        font(&fonts.bold, BOX_NAME_SIZE),
        TextColor(Color::srgb_u8(red, green, blue)),
        TextLayout::no_wrap(),
        TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::BLACK.with_alpha(0.45) },
        Pickable::IGNORE,
        ChildOf(card),
    ));
    let line = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(card),
        ))
        .id();
    let abilities = commands
        .spawn((Node { column_gap: px(3.0), ..default() }, Pickable::IGNORE, ChildOf(line)))
        .id();
    for &id in &player.icons {
        let Ok(icon) = game.graphic(id).inspect_err(|err| warn!("graphic {id}: {err}")) else {
            continue;
        };
        commands.spawn((
            ImageNode::new(images.add(smooth_image(scale2x(&scale2x(&icon))))),
            Node {
                width: px(BOX_ICON),
                height: px(BOX_ICON),
                border_radius: BorderRadius::all(px(4.0)),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(abilities),
        ));
    }
    commands.spawn((
        Text::new(format!("{}  {}", player.position, player.strength)),
        font(&fonts.bold, BOX_DETAIL_SIZE),
        TextColor(Color::WHITE),
        TextLayout::no_wrap(),
        TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::BLACK.with_alpha(0.45) },
        Pickable::IGNORE,
        ChildOf(line),
    ));
}

/// The menu bar: a dark strip across the bottom of the canvas with the
/// game's symbols spread along it, twice their size.
#[allow(clippy::too_many_arguments)]
fn spawn_bar(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    game: &Game,
    root: Entity,
    layout: &Layout,
    index: usize,
    control: &Control,
    count: usize,
    fonts: &Fonts,
) {
    let bar = wide_bar(layout, control);
    commands.spawn((
        Node { border: UiRect::top(px(2.0)), ..node(bar) },
        BackgroundGradient::from(LinearGradient::to_bottom(stops(&[
            (BAR_TOP, 0.0),
            (BAR_BOTTOM, 100.0),
        ]))),
        BorderColor::all(AMBER),
        BoxShadow::new(Color::BLACK.with_alpha(0.35), px(0.0), px(-4.0), px(0.0), px(16.0)),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    let strips: Vec<Option<RgbaImage>> = bar::IMAGES
        .iter()
        .map(|&id| game.graphic(id).inspect_err(|err| warn!("graphic {id}: {err}")).ok())
        .collect();
    let strip =
        |id: u16| bar::IMAGES.iter().position(|&i| i == id).and_then(|i| strips[i].as_ref());
    for item in 0..count {
        let symbol = bar::symbol(control, item).translate(-control.rect.min);
        let mut crop = |id: u16, grow: f32| {
            let Some(strip) = strip(id) else { return Handle::default() };
            let left = (symbol.min.x - grow).max(0.0) as u32;
            let width = (symbol.width() + 2.0 * grow) as u32;
            let width = width.min(strip.width().saturating_sub(left));
            let piece = image::imageops::crop_imm(strip, left, 0, width, strip.height()).to_image();
            images.add(smooth_image(scale2x(&piece)))
        };
        let images = [
            crop(bar::NORMAL, 0.0),
            crop(bar::PRESSED, 0.0),
            crop(bar::LIT[item % 2], 0.0),
            crop(bar::OFF, 0.0),
        ];
        let slot = layout.bar(control, item);
        let at = Rect::from_center_size(slot.center(), Vec2::splat(SYMBOL_SIZE));
        commands.spawn((
            BarSymbol { control: index, item, images: images.clone(), hover: 0.0 },
            ImageNode::new(images[0].clone()),
            node(at),
            UiTransform::default(),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    // The tooltip, shown above the symbol pointed at for a while.
    let text = commands
        .spawn((
            Text::default(),
            font(&fonts.regular, TEXT_SIZE - 2.0),
            TextColor(Color::WHITE),
            TextLayout::no_wrap(),
            Pickable::IGNORE,
        ))
        .id();
    commands
        .spawn((
            Tip { text },
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::axes(px(14.0), px(6.0)),
                border_radius: BorderRadius::all(px(10.0)),
                ..default()
            },
            BackgroundColor(INK.with_alpha(0.88)),
            BoxShadow::new(Color::BLACK.with_alpha(0.3), px(0.0), px(3.0), px(0.0), px(10.0)),
            Visibility::Hidden,
            ZIndex(5),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .add_child(text);
}

/// The bar's symbols show greyed out, pressed for the open screen, lit and
/// a little bigger when pointed at; the tooltip of the one pointed at.
#[allow(clippy::type_complexity)]
pub(super) fn ease_bar(
    time: Res<Time>,
    view: Res<OpenDialog>,
    layout: Option<Res<Layout>>,
    game: Res<Game>,
    mut symbols: Query<(&mut BarSymbol, &mut ImageNode, &mut UiTransform)>,
    mut tips: Query<(&Tip, &mut Node, &mut Visibility)>,
    mut texts: Query<&mut Text>,
) {
    let ease = 1.0 - (-EASE_RATE * time.delta_secs()).exp();
    for (mut symbol, mut image, mut transform) in &mut symbols {
        let Some(state) = view.bar(symbol.control) else { continue };
        let target = Target::Option { control: symbol.control, item: symbol.item };
        let (on, chosen) = (state.item_on(symbol.item), state.chosen == symbol.item);
        let wanted = f32::from(on && !chosen && view.hovered == Some(target));
        symbol.hover = approach(symbol.hover, wanted, ease);
        let shown = if !on {
            &symbol.images[3]
        } else if chosen {
            &symbol.images[1]
        } else if symbol.hover > 0.5 {
            &symbol.images[2]
        } else {
            &symbol.images[0]
        };
        if image.image != *shown {
            image.image = shown.clone();
        }
        let tint = Color::WHITE.with_alpha(if on { 1.0 } else { DISABLED_ALPHA + 0.1 });
        if image.color != tint {
            image.color = tint;
        }
        let scale = Vec2::splat(1.0 + SYMBOL_GROWTH * symbol.hover);
        if transform.scale != scale {
            transform.scale = scale;
        }
    }
    let (Some(layout), Some(dialog)) = (layout, game.dialogs.get(&view.id)) else { return };
    for (tip, mut node, mut visibility) in &mut tips {
        let shown = view.tip.and_then(|(control, item)| {
            let text = view.bar(control)?.tips.get(item)?;
            Some((layout.bar(&dialog.controls[control], item), text))
        });
        let wanted = if shown.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if let Some((symbol, text)) = shown {
            node.left = px((symbol.center().x - 80.0).clamp(10.0, CANVAS.x - 420.0));
            node.top = px(symbol.min.y - 52.0);
            if let Ok(mut shown) = texts.get_mut(tip.text)
                && shown.0 != *text
            {
                shown.0.clone_from(text);
            }
        }
    }
}

/// An object of a scene as it looks pointed at: a little brighter.
fn lighten(sprite: &RgbaImage) -> RgbaImage {
    let mut lit = sprite.clone();
    for pixel in lit.pixels_mut() {
        for channel in &mut pixel.0[..3] {
            *channel = (f32::from(*channel) * 1.18 + 14.0).min(255.0) as u8;
        }
    }
    lit
}

/// Where hover and choice effects of `target` are heading: 1 or 0.
fn targets(view: &View, target: Target) -> (f32, f32) {
    let hover = f32::from(view.hovered == Some(target));
    let active = match target {
        Target::Option { control, item } => {
            f32::from(view.options(control).is_some_and(|o| !o.actions && o.chosen == item))
        }
        _ => 0.0,
    };
    (hover, active)
}

fn approach(current: f32, target: f32, ease: f32) -> f32 {
    let next = current + (target - current) * ease;
    if (target - next).abs() < 0.002 { target } else { next }
}

/// A pearl bullet that turns into a black one as it is chosen.
fn bullet_style(hover: f32, active: f32) -> (BackgroundGradient, BorderColor, BoxShadow) {
    let fill = RadialGradient::new(
        UiPosition::CENTER,
        RadialGradientShape::FarthestCorner,
        stops(&[
            (mix(Color::WHITE, CHARCOAL, active), 0.0),
            (mix(PEARL, INK, active), 55.0),
            (mix(PEARL_SHADE, Color::BLACK, active), 100.0),
        ]),
    );
    let ring = mix(mix(RING, HOVER, hover), Color::BLACK, active);
    let shadow =
        BoxShadow::new(Color::BLACK.with_alpha(0.35 * active), px(0.0), px(2.0), px(0.0), px(6.0));
    (fill.into(), BorderColor::all(ring), shadow)
}

/// Fill, border and text colour of an option box: frosted and outlined when
/// open, black when chosen, warming up under the pointer.
fn box_style(hover: f32, active: f32) -> (Color, Color, Color) {
    let fill = mix(mix(FROST, PALE, hover * 0.6), INK, active);
    let edge = mix(mix(RING.with_alpha(0.35), HOVER, hover), INK, active);
    let ink = mix(mix(INK, HOVER, hover), Color::WHITE, active);
    (fill, edge, ink)
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    Color::srgba(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    )
}

fn stops(stops: &[(Color, f32)]) -> Vec<ColorStop> {
    stops.iter().map(|&(color, percent)| ColorStop::percent(color, percent)).collect()
}

fn to_right(colors: &[(Color, f32)]) -> BackgroundGradient {
    LinearGradient::to_right(stops(colors)).into()
}

fn font(face: &Handle<Font>, size: f32) -> TextFont {
    TextFont { font: face.into(), font_size: FontSize::Px(size), ..default() }
}
