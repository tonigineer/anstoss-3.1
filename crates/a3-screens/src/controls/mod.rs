//! The controls of a dialog, a module per kind: what each keeps while its
//! dialog is open, how the classic look draws it, and where it answers the
//! pointer and the keyboard.

pub mod anim;
pub mod bar;
pub mod bitmap;
pub mod button;
pub mod check;
pub mod combo;
pub mod edit;
pub mod fader;
pub mod header;
pub mod list;
pub mod marker;
pub mod menu;
pub mod options;
pub mod paint;
pub mod player_box;
pub mod slider;
pub mod spin;
pub mod text;

use bevy_math::Rect;
use image::RgbaImage;

use a3_files::rch::{Control, Kind};

use crate::classic::Paint;
use crate::pointer::{Geometry, Target};

use self::anim::AnimState;
use self::bar::BarState;
use self::bitmap::BitmapState;
use self::button::ButtonState;
use self::check::CheckState;
use self::combo::ComboState;
use self::edit::EditState;
use self::header::HeaderState;
use self::list::ListState;
use self::marker::Marker;
use self::menu::MenuState;
use self::options::OptionsState;
use self::paint::PaintState;
use self::player_box::PlayerBox;
use self::spin::Value;
use self::text::StaticState;

/// A control of the open dialog.
#[derive(Clone, Debug)]
pub struct ControlView {
    /// A hidden control shows nothing and does not answer.
    pub hidden: bool,
    pub state: ControlState,
}

/// What a control keeps while its dialog is open, by its kind.
#[derive(Clone, Debug)]
pub enum ControlState {
    Static(StaticState),
    Button(ButtonState),
    List(ListState),
    Header(HeaderState),
    Bitmap(BitmapState),
    Anim(AnimState),
    Spin(Value),
    Slider(Value),
    Fader(Value),
    Edit(EditState),
    Options(OptionsState),
    /// What a player's box shows, once the flow has filled it.
    Player(Option<PlayerBox>),
    /// What a marker of the small pitch shows, once the screen has put it.
    Marker(Option<Marker>),
    Check(CheckState),
    Combo(ComboState),
    Menu(MenuState),
    Bar(BarState),
    Paint(PaintState),
    Other,
}

impl ControlState {
    /// A player's box, if the control is one and shows a player or a spot.
    pub fn as_player(&self) -> Option<&PlayerBox> {
        match self {
            Self::Player(player) => player.as_ref(),
            _ => None,
        }
    }
}

/// Where columns start; a list without any has one.
fn tabs(columns: &[a3_files::rch::Column]) -> Vec<f32> {
    if columns.is_empty() { vec![0.0] } else { columns.iter().map(|c| c.at).collect() }
}

impl ControlView {
    pub fn new(control: &Control) -> Self {
        let text = || control.text.clone();
        let state = match &control.kind {
            Kind::Static => {
                ControlState::Static(StaticState { text: text(), clickable: false, quiet: false })
            }
            Kind::Button { .. } | Kind::States { .. } => {
                ControlState::Button(ButtonState::default())
            }
            // Columns as the dialog file sets them, until the screen sets its own.
            Kind::ListBox { columns, .. } => ControlState::List(ListState {
                tabs: tabs(columns),
                aligns: columns.iter().map(|c| c.align).collect(),
                ..ListState::default()
            }),
            Kind::Header { columns } => ControlState::Header(HeaderState {
                text: text(),
                tabs: tabs(columns),
                aligns: columns.iter().map(|c| c.align).collect(),
            }),
            Kind::Bitmap => ControlState::Bitmap(BitmapState::default()),
            Kind::Anim => ControlState::Anim(AnimState::default()),
            Kind::Spin { .. } => ControlState::Spin(Value::default()),
            Kind::Slider { .. } => ControlState::Slider(Value::default()),
            Kind::Fader { .. } => ControlState::Fader(Value::default()),
            Kind::Edit { .. } => ControlState::Edit(EditState { text: text() }),
            Kind::Options { items, style, .. } => {
                ControlState::Options(OptionsState::new(items, *style))
            }
            Kind::Player => ControlState::Player(None),
            Kind::Marker => ControlState::Marker(None),
            Kind::Check => ControlState::Check(CheckState { checked: false, text: text() }),
            Kind::Combo { .. } => ControlState::Combo(ComboState::default()),
            Kind::PushButton { .. } => ControlState::Button(ButtonState::default()),
            Kind::Menu => ControlState::Menu(MenuState::default()),
            Kind::Bar { .. } => ControlState::Bar(BarState::default()),
            Kind::Paint => ControlState::Paint(PaintState::default()),
            Kind::Other => ControlState::Other,
        };
        Self { hidden: false, state }
    }
}

macro_rules! kinds {
    ($($get:ident, $get_mut:ident, $variant:ident, $state:ty;)*) => {
        impl ControlState {
            $(
                pub fn $get(&self) -> Option<&$state> {
                    match self {
                        Self::$variant(state) => Some(state),
                        _ => None,
                    }
                }

                pub fn $get_mut(&mut self) -> Option<&mut $state> {
                    match self {
                        Self::$variant(state) => Some(state),
                        _ => None,
                    }
                }
            )*
        }
    };
}

kinds! {
    as_static, as_static_mut, Static, StaticState;
    as_button, as_button_mut, Button, ButtonState;
    as_list, as_list_mut, List, ListState;
    as_header, as_header_mut, Header, HeaderState;
    as_bitmap, as_bitmap_mut, Bitmap, BitmapState;
    as_anim, as_anim_mut, Anim, AnimState;
    as_edit, as_edit_mut, Edit, EditState;
    as_options, as_options_mut, Options, OptionsState;
    as_bar, as_bar_mut, Bar, BarState;
    as_check, as_check_mut, Check, CheckState;
    as_paint, as_paint_mut, Paint, PaintState;
    as_combo, as_combo_mut, Combo, ComboState;
}

impl ControlState {
    /// A spinner's or slider's value.
    pub fn as_value(&self) -> Option<&Value> {
        match self {
            Self::Spin(value) | Self::Slider(value) | Self::Fader(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_value_mut(&mut self) -> Option<&mut Value> {
        match self {
            Self::Spin(value) | Self::Slider(value) | Self::Fader(value) => Some(value),
            _ => None,
        }
    }

    /// The text of a static, a field or a header.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Static(s) => Some(&s.text),
            Self::Edit(e) => Some(&e.text),
            Self::Header(h) => Some(&h.text),
            _ => None,
        }
    }

    pub fn text_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::Static(s) => Some(&mut s.text),
            Self::Edit(e) => Some(&mut e.text),
            Self::Header(h) => Some(&mut h.text),
            _ => None,
        }
    }

    /// The graphics the classic look draws the control with, by id, and
    /// whether their black is clear.
    pub(crate) fn graphics(&self, control: &Control) -> (Vec<u16>, bool) {
        match self {
            Self::Button(b) => button::graphics(b, control),
            Self::Options(o) => options::graphics(o, control),
            Self::Anim(anim) => (anim.frames.clone(), true),
            Self::Bar(_) => (bar::IMAGES.to_vec(), false),
            Self::Player(player) => player_box::graphics(player.as_ref()),
            Self::Marker(marker) => (marker.iter().map(|m| m.image).collect(), true),
            Self::List(list) => list.graphics(),
            Self::Fader(_) => fader::graphics(control),
            _ => (Vec::new(), false),
        }
    }

    /// Draws the control in the classic look.
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        match self {
            Self::Static(s) => s.draw(frame, paint),
            Self::Button(b) => b.draw(frame, paint),
            Self::List(l) => l.draw(frame, paint),
            Self::Header(h) => h.draw(frame, paint),
            Self::Bitmap(b) => b.draw(frame, paint),
            Self::Anim(a) => a.draw(frame, paint),
            Self::Slider(v) => slider::draw(v, frame, paint),
            Self::Fader(v) => fader::draw(v, frame, paint),
            Self::Spin(_) => spin::draw(frame, paint),
            Self::Edit(e) => e.draw(frame, paint),
            Self::Options(o) => o.draw(frame, paint),
            Self::Player(Some(p)) => p.draw(frame, paint),
            Self::Marker(Some(m)) => m.draw(frame, paint),
            Self::Check(c) => c.draw(frame, paint),
            Self::Menu(m) => m.draw(frame, paint),
            Self::Bar(b) => b.draw(frame, paint),
            Self::Paint(p) => p.draw(frame, paint),
            Self::Combo(c) => c.draw(frame, paint),
            Self::Player(None) | Self::Marker(None) | Self::Other => {}
        }
    }

    /// Where the control answers the pointer, with what it acts on.
    pub(crate) fn targets(
        &self,
        index: usize,
        control: &Control,
        geometry: &dyn Geometry,
        out: &mut Vec<(Target, Rect)>,
    ) {
        match self {
            Self::Static(s) => s.targets(index, control, geometry, out),
            Self::Button(_) => out.push((Target::Button(index), geometry.rect(control))),
            Self::List(l) => list::targets(l, index, control, geometry, out),
            Self::Anim(a) => a.targets(index, control, geometry, out),
            Self::Spin(_) => spin::targets(index, control, geometry, out),
            Self::Slider(_) => slider::targets(index, control, geometry, out),
            Self::Fader(v) => fader::targets(v, index, control, geometry, out),
            Self::Edit(_) => out.push((Target::Edit(index), geometry.rect(control))),
            Self::Options(o) => o.targets(index, control, geometry, out),
            Self::Bar(b) => b.targets(index, control, geometry, out),
            Self::Check(c) => c.targets(index, control, geometry, out),
            Self::Combo(c) => c.targets(index, control, geometry, out),
            Self::Menu(m) => m.targets(index, control, geometry, out),
            Self::Player(Some(player)) => player.targets(index, control, geometry, out),
            // The markers have no handler: a click on one does nothing.
            Self::Header(_)
            | Self::Bitmap(_)
            | Self::Player(None)
            | Self::Marker(_)
            | Self::Paint(_)
            | Self::Other => {}
        }
    }
}
