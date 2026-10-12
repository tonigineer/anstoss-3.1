//! Where each look puts a dialog's controls on the 1920×1080 canvas.
//!
//! The classic look keeps the 640×480 dialog in the middle at 2.25×. The
//! remastered look puts the shared artwork at the left and gives the
//! interface the space right of it: the part of the dialog the controls take
//! up is stretched across that panel — wider, not taller, so rows stay where
//! they were. Dialogs with a picture of their own spread over the whole
//! width. Text keeps its size; boxes, lists and bars grow. The buttons stay
//! in the bottom right corner, the title moves into a ribbon along the top.
//!
//! Both looks answer the pointer in their own places, so each gives the
//! targets of its controls through [`Geometry`].

use bevy::prelude::*;
use image::RgbaImage;

use super::{SLIDER_ARROW, Target, View, backdrop, visible_rows};
use crate::look::{CANVAS, CLASSIC, CLASSIC_LEFT, CLASSIC_SCALE};
use crate::rch::{Control, Dialog, Kind};

/// The space the remastered interface spreads over, right of the artwork;
/// for a dialog with a picture of its own, nearly the whole width.
const PANEL: (f32, f32) = (840.0, 1840.0);
const WIDE_PANEL: (f32, f32) = (80.0, 1840.0);
/// The interface grows at most this many times wider; it is 2.25× as tall.
const MAX_STRETCH: f32 = 3.4;
/// The title sits in the top bar of every dialog, above this line.
const TITLE_BOTTOM: f32 = 62.0;
/// Remastered steppers: round buttons this wide either side of the value,
/// with this gap, around a value this wide (canvas pixels).
pub(super) const STEP_BUTTON: f32 = 40.0;
const STEP_GAP: f32 = 8.0;
const STEP_VALUE: f32 = 76.0;
/// Inside a remastered list card, the scrollbar strip on the right.
pub(super) const CARD_SCROLLBAR: f32 = 36.0;

/// Where a look puts things, in canvas pixels.
pub(super) trait Geometry {
    /// A control's rectangle.
    fn rect(&self, control: &Control) -> Rect;
    /// Any classic rectangle in the dialog's interface.
    fn map(&self, r: Rect) -> Rect;
    /// How many canvas pixels one classic pixel across becomes.
    fn zoom(&self) -> f32;
    /// The parts of a spinner that step it up and down.
    fn spin(&self, index: usize, control: &Control) -> (Rect, Rect);
    /// The arrows of a list's scrollbar.
    fn scroll(&self, control: &Control) -> (Rect, Rect);
    /// A symbol of the menu bar.
    fn bar(&self, control: &Control, item: usize) -> Rect;
}

/// A symbol of the menu bar where the classic look has it.
pub(super) fn bar_symbol(control: &Control, item: usize) -> Rect {
    let Kind::Bar { items } = &control.kind else { return control.rect };
    let r = items.get(item).map_or(Rect::default(), |i| i.rect);
    Rect::from_corners(control.rect.min + r.min, control.rect.min + r.max)
}

/// The remastered menu bar runs across the whole canvas, its symbols spread
/// evenly along it.
pub(super) fn wide_bar(layout: &Layout, control: &Control) -> Rect {
    Rect::new(0.0, layout.y(control.rect.min.y), CANVAS.x, CANVAS.y)
}

/// The classic look: the dialog in the middle at 2.25×.
pub(super) struct Classic;

impl Geometry for Classic {
    fn rect(&self, control: &Control) -> Rect {
        self.map(control.rect)
    }

    fn map(&self, r: Rect) -> Rect {
        Rect::from_corners(classic_point(r.min), classic_point(r.max))
    }

    fn zoom(&self) -> f32 {
        CLASSIC_SCALE
    }

    fn spin(&self, _: usize, control: &Control) -> (Rect, Rect) {
        let r = self.rect(control);
        let middle = r.center().y;
        (Rect::new(r.min.x, r.min.y, r.max.x, middle), Rect::new(r.min.x, middle, r.max.x, r.max.y))
    }

    fn scroll(&self, control: &Control) -> (Rect, Rect) {
        let (up, down) = super::scroll_arrows(control);
        (self.map(up), self.map(down))
    }

    fn bar(&self, control: &Control, item: usize) -> Rect {
        self.map(bar_symbol(control, item))
    }
}

fn classic_point(p: Vec2) -> Vec2 {
    Vec2::new(CLASSIC_LEFT + p.x * CLASSIC_SCALE, p.y * CLASSIC_SCALE)
}

/// The remastered look of one dialog.
#[derive(Resource)]
pub(super) struct Layout {
    /// Canvas x = `offset` + classic x × `scale`; canvas y = classic y × 2.25.
    offset: f32,
    scale: f32,
    /// The panel's right edge, where bars run out.
    pub right: f32,
    /// A scene keeps everything where the classic look has it, on its
    /// picture.
    pub scene: bool,
    /// The control that is the dialog's title.
    pub title: Option<usize>,
    /// Statics that head a section, with how far down their section runs
    /// and where it starts on the left (classic pixels).
    pub headers: Vec<Header>,
    /// Spinners shown as steppers, with the static that shows their value.
    pub steppers: Vec<Stepper>,
}

/// A section's heading, how far down the section runs and where its
/// controls start on the left, in classic pixels.
pub(super) struct Header {
    pub control: usize,
    pub bottom: f32,
    pub left: f32,
}

/// A spinner with its number, drawn as a minus button, the value and a plus
/// button (canvas pixels).
pub(super) struct Stepper {
    pub spin: usize,
    pub value: usize,
    pub minus: Rect,
    pub number: Rect,
    pub plus: Rect,
}

impl Layout {
    pub fn new(id: u16, dialog: &Dialog, background: &RgbaImage) -> Self {
        if backdrop::is_scene(id) || dialog.frame.is_some() {
            return Self {
                offset: CLASSIC_LEFT,
                scale: CLASSIC_SCALE,
                right: CANVAS.x - CLASSIC_LEFT,
                scene: true,
                title: None,
                headers: Vec::new(),
                steppers: Vec::new(),
            };
        }
        let (left, right) = if backdrop::has_own_picture(id) { WIDE_PANEL } else { PANEL };
        // The week's panels have their titles up there too.
        let title = dialog
            .controls
            .iter()
            .position(|c| matches!(c.kind, Kind::Static) && c.rect.max.y <= TITLE_BOTTOM)
            .filter(|_| !super::turn::is_week(id));
        let body: Vec<&Control> = dialog
            .controls
            .iter()
            .enumerate()
            .filter(|&(i, c)| Some(i) != title && !matches!(c.kind, Kind::Button { .. }))
            .map(|(_, c)| c)
            .collect();
        let span = body
            .iter()
            .map(|c| c.rect)
            .reduce(|a, b| a.union(b))
            .unwrap_or(Rect::new(0.0, 0.0, CLASSIC.x, CLASSIC.y));
        let scale = ((right - left) / span.width().max(1.0)).min(MAX_STRETCH);
        // The interface in the middle of its panel.
        let start = left + ((right - left) - span.width() * scale) / 2.0;
        let mut layout = Self {
            offset: start - span.min.x * scale,
            scale,
            right,
            scene: false,
            title,
            headers: Vec::new(),
            steppers: Vec::new(),
        };
        layout.headers = headers(dialog, background, title);
        layout.steppers = steppers(dialog)
            .into_iter()
            .map(|(spin, value)| {
                let both = dialog.controls[value].rect.union(dialog.controls[spin].rect);
                let centre = layout.map(both).center();
                // Wide values, like money, get a wider pill.
                let wide = layout.map(dialog.controls[value].rect).width();
                let pill = STEP_VALUE.max(wide.min(STEP_VALUE * 2.0));
                let button = |dx: f32| {
                    Rect::from_center_size(centre + Vec2::new(dx, 0.0), Vec2::splat(STEP_BUTTON))
                };
                let side = pill / 2.0 + STEP_GAP + STEP_BUTTON / 2.0;
                Stepper {
                    spin,
                    value,
                    minus: button(-side),
                    number: Rect::from_center_size(centre, Vec2::new(pill, STEP_BUTTON)),
                    plus: button(side),
                }
            })
            .collect();
        layout
    }

    pub fn x(&self, x: f32) -> f32 {
        self.offset + x * self.scale
    }

    pub fn y(&self, y: f32) -> f32 {
        y * CLASSIC_SCALE
    }
}

impl Geometry for Layout {
    fn rect(&self, control: &Control) -> Rect {
        match control.kind {
            // The buttons keep their size in the bottom right corner.
            Kind::Button { .. } if !self.scene => {
                let r = control.rect;
                let right = |x: f32| CANVAS.x - (CLASSIC.x - x) * CLASSIC_SCALE;
                Rect::new(right(r.min.x), self.y(r.min.y), right(r.max.x), self.y(r.max.y))
            }
            _ => self.map(control.rect),
        }
    }

    fn map(&self, r: Rect) -> Rect {
        Rect::new(self.x(r.min.x), self.y(r.min.y), self.x(r.max.x), self.y(r.max.y))
    }

    fn zoom(&self) -> f32 {
        self.scale
    }

    fn spin(&self, index: usize, control: &Control) -> (Rect, Rect) {
        // A stepper steps up on its plus button, down on its minus button.
        if let Some(stepper) = self.steppers.iter().find(|s| s.spin == index) {
            return (stepper.plus, stepper.minus);
        }
        let r = self.rect(control);
        let middle = r.center().y;
        (Rect::new(r.min.x, r.min.y, r.max.x, middle), Rect::new(r.min.x, middle, r.max.x, r.max.y))
    }

    fn scroll(&self, control: &Control) -> (Rect, Rect) {
        let r = self.rect(control);
        let left = r.max.x - CARD_SCROLLBAR;
        let up = Rect::new(left, r.min.y, r.max.x, r.min.y + CARD_SCROLLBAR);
        let down = Rect::new(left, r.max.y - CARD_SCROLLBAR, r.max.x, r.max.y);
        (up, down)
    }

    fn bar(&self, control: &Control, item: usize) -> Rect {
        if self.scene {
            return self.map(bar_symbol(control, item));
        }
        let Kind::Bar { items } = &control.kind else { return self.rect(control) };
        let bar = wide_bar(self, control);
        let step = bar.width() / items.len().max(1) as f32;
        Rect::new(
            bar.min.x + step * item as f32,
            bar.min.y,
            bar.min.x + step * (item + 1) as f32,
            bar.max.y,
        )
    }
}

/// Statics painted on one of the artwork's coloured bars head a section;
/// the section runs down to the next one, or to its last control.
fn headers(dialog: &Dialog, background: &RgbaImage, title: Option<usize>) -> Vec<Header> {
    let heads: Vec<usize> = (0..dialog.controls.len())
        .filter(|&i| Some(i) != title && matches!(dialog.controls[i].kind, Kind::Static))
        .filter(|&i| on_bar(background, dialog.controls[i].rect))
        .collect();
    heads
        .iter()
        .map(|&head| {
            let top = dialog.controls[head].rect.max.y;
            let next = heads
                .iter()
                .map(|&h| dialog.controls[h].rect.min.y)
                .filter(|&y| y > top)
                .fold(CLASSIC.y, f32::min);
            let x = dialog.controls[head].rect.min.x;
            let section: Vec<Rect> = dialog
                .controls
                .iter()
                .filter(|c| !matches!(c.kind, Kind::Button { .. } | Kind::Bar { .. }))
                .filter(|c| c.rect.min.y >= top && c.rect.min.y < next && c.rect.max.x > x)
                .map(|c| c.rect)
                .collect();
            let bottom = section.iter().map(|r| r.max.y).fold(top, f32::max);
            let left = section.iter().map(|r| r.min.x).fold(x, f32::min);
            Header { control: head, bottom, left }
        })
        .collect()
}

/// Whether a good part of the artwork just around a rectangle is the strong
/// yellow, blue or red of its bars.
fn on_bar(background: &RgbaImage, r: Rect) -> bool {
    let (x0, x1) = (r.min.x.max(0.0) as u32, (r.max.x as u32).min(background.width()));
    let (y0, y1) = ((r.min.y - 6.0).max(0.0) as u32, (r.max.y as u32).min(background.height()));
    let (mut yellow, mut all) = (0, 0);
    for y in y0..y1 {
        for x in x0..x1 {
            let [r, g, b, _] = background.get_pixel(x, y).0;
            all += 1;
            let yellowish = r > 180 && g > 140 && b < 110 && r.saturating_sub(b) > 100;
            let blueish = b > 150 && r < 130 && b.saturating_sub(r) > 70;
            let reddish = r > 180 && g < 120 && b < 100;
            if yellowish || blueish || reddish {
                yellow += 1;
            }
        }
    }
    all > 0 && yellow * 6 > all
}

/// Spinners with a number just left of them, on the same line.
fn steppers(dialog: &Dialog) -> Vec<(usize, usize)> {
    let controls = &dialog.controls;
    (0..controls.len())
        .filter(|&i| matches!(controls[i].kind, Kind::Spin))
        .filter_map(|spin| {
            let s = controls[spin].rect;
            let value = (0..controls.len()).find(|&i| {
                let v = controls[i].rect;
                matches!(controls[i].kind, Kind::Static)
                    && v.max.x <= s.min.x + 2.0
                    && v.max.x >= s.min.x - 12.0
                    && v.min.y < s.max.y
                    && v.max.y > s.min.y
            })?;
            Some((spin, value))
        })
        .collect()
}

/// Every place a look answers the pointer, with what it acts on.
pub(super) fn targets(
    dialog: &Dialog,
    view: &View,
    geometry: &dyn Geometry,
) -> Vec<(Target, Rect)> {
    let mut targets = Vec::new();
    for (control, (c, state)) in dialog.controls.iter().zip(&view.controls).enumerate() {
        if state.hidden {
            continue;
        }
        let r = c.rect;
        match &c.kind {
            Kind::Options { image, items } => {
                // Bulleted options also answer on their bullet.
                let lead = if *image == 0 { 2.0 } else { 0.0 };
                for (item, it) in items.iter().enumerate().filter(|&(i, _)| state.item_on(i)) {
                    let min = r.min + it.rect.min - Vec2::new(lead, 0.0);
                    let rect = Rect::from_corners(min, r.min + it.rect.max);
                    targets.push((Target::Option { control, item }, geometry.map(rect)));
                }
            }
            Kind::Button { .. } => targets.push((Target::Button(control), geometry.rect(c))),
            Kind::ListBox => {
                let row_height = super::ROW_HEIGHT;
                for row in 0..visible_rows(c) {
                    let y = r.min.y + 1.0 + row as f32 * row_height;
                    let rect = Rect::new(r.min.x, y, r.max.x, y + row_height);
                    targets.push((Target::Row { control, row }, geometry.map(rect)));
                }
                let (up, down) = geometry.scroll(c);
                targets.push((Target::Scroll { control, up: true }, up));
                targets.push((Target::Scroll { control, up: false }, down));
            }
            Kind::Edit { .. } => targets.push((Target::Edit(control), geometry.rect(c))),
            Kind::Spin => {
                let (up, down) = geometry.spin(control, c);
                targets.push((Target::Spin { control, up: true }, up));
                targets.push((Target::Spin { control, up: false }, down));
            }
            Kind::Slider => {
                let (y0, y1) = (r.min.y, r.max.y);
                let left = Rect::new(r.min.x, y0, r.min.x + SLIDER_ARROW, y1);
                let track = Rect::new(r.min.x + SLIDER_ARROW, y0, r.max.x - SLIDER_ARROW, y1);
                let right = Rect::new(r.max.x - SLIDER_ARROW, y0, r.max.x, y1);
                targets.push((Target::SliderStep { control, up: false }, geometry.map(left)));
                targets.push((Target::SliderTrack(control), geometry.map(track)));
                targets.push((Target::SliderStep { control, up: true }, geometry.map(right)));
            }
            Kind::Bar { items } => {
                for item in (0..items.len()).filter(|&i| state.item_on(i)) {
                    targets.push((Target::Option { control, item }, geometry.bar(c, item)));
                }
            }
            Kind::Static | Kind::Anim if state.clickable => {
                targets.push((Target::Control(control), geometry.rect(c)))
            }
            _ => {}
        }
    }
    targets
}
