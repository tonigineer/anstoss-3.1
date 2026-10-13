//! Where a dialog answers the pointer: what can be acted on, and where each
//! look puts it.

use bevy_math::Rect;

use a3_files::rch::{Control, Dialog};

use crate::controls::{bar, list};
use crate::view::View;

/// What a drag takes hold of, or is let go of on: a control — a player's
/// box — or a list's row, counted from the first one shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grip {
    Control(usize),
    Row { control: usize, row: usize },
}

impl Grip {
    /// What the pointer pressing on a target takes hold of, if anything.
    pub fn of(target: Target) -> Option<Self> {
        match target {
            Target::Control(control) => Some(Self::Control(control)),
            Target::Row { control, row } => Some(Self::Row { control, row }),
            _ => None,
        }
    }
}

/// Something a pointer or key can act on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    /// Something dragged and let go of on another place, or on none.
    Drop {
        from: Grip,
        to: Option<Grip>,
    },
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

/// Where a look puts things. By default everything stays where the classic
/// look has it, mapped by [`Geometry::map`].
pub trait Geometry {
    /// Any classic rectangle in the dialog.
    fn map(&self, r: Rect) -> Rect;
    /// How many of the look's pixels one classic pixel across becomes.
    fn zoom(&self) -> f32;

    /// A control's rectangle.
    fn rect(&self, control: &Control) -> Rect {
        self.map(control.rect)
    }

    /// The parts of a spinner that step it up and down.
    fn spin(&self, _index: usize, control: &Control) -> (Rect, Rect) {
        halves(self.rect(control))
    }

    /// The arrows of a list's scrollbar.
    fn scroll(&self, control: &Control) -> (Rect, Rect) {
        let (up, down) = list::scroll_arrows(control);
        (self.map(up), self.map(down))
    }

    /// A symbol of the menu bar.
    fn bar(&self, control: &Control, item: usize) -> Rect {
        self.map(bar::symbol(control, item))
    }
}

/// The classic look in its own pixels.
pub struct Pixels;

impl Geometry for Pixels {
    fn map(&self, r: Rect) -> Rect {
        r
    }

    fn zoom(&self) -> f32 {
        1.0
    }
}

/// The top and bottom half of a rectangle.
pub fn halves(r: Rect) -> (Rect, Rect) {
    let middle = r.center().y;
    (Rect::new(r.min.x, r.min.y, r.max.x, middle), Rect::new(r.min.x, middle, r.max.x, r.max.y))
}

/// Every place a look answers the pointer, with what it acts on.
pub fn targets(dialog: &Dialog, view: &View, geometry: &dyn Geometry) -> Vec<(Target, Rect)> {
    let mut targets = Vec::new();
    for (index, (control, shown)) in dialog.controls.iter().zip(&view.controls).enumerate() {
        if !shown.hidden {
            shown.state.targets(index, control, geometry, &mut targets);
        }
    }
    targets
}
