//! The open dialog and everything about it that can change: a state per
//! control, the field typed into, and what the pointer is on. Both looks
//! draw from it, which is what lets them be switched mid-dialog.

use a3_files::rch::{Dialog, Kind};
use bevy_math::Vec2;

use crate::controls::anim::AnimState;
use crate::controls::bar::BarState;
use crate::controls::bitmap::{BitmapState, Picture};
use crate::controls::check::CheckState;
use crate::controls::combo::ComboState;
use crate::controls::header::HeaderState;
use crate::controls::list::{ListState, WHEEL_ROWS, visible_rows};
use crate::controls::options::OptionsState;
use crate::controls::paint::PaintState;
use crate::controls::player_box::PlayerBox;
use crate::controls::spin::Value;
use crate::controls::{ControlState, ControlView};
use crate::pointer::{Grip, Target};

#[derive(Clone)]
pub struct View {
    pub id: u16,
    /// One per control of the dialog, in the same order.
    pub controls: Vec<ControlView>,
    /// The text field being typed into.
    pub focus: Option<usize>,
    pub hovered: Option<Target>,
    /// The button the pointer is holding down.
    pub pressed: Option<Target>,
    /// The item whose tooltip shows: control and item.
    pub tip: Option<(usize, usize)>,
    /// What the screen does nothing with yet; the looks cross it out.
    pub inert: Vec<Target>,
    /// A player's box or row held by the pointer, and where the pointer is.
    pub dragging: Option<Dragging>,
    /// Lines the screen draws on the dialog's picture, under its controls:
    /// from one point to another, in a colour; or over them, where the
    /// original drew them again without its controls drawing themselves
    /// anew.
    pub lines: Vec<([i32; 4], [u8; 3])>,
    pub lines_on_top: bool,
    /// Rectangles the screen darkens on the dialog's picture, under its
    /// controls: x, y, width and height, and by how much of 255.
    pub shades: Vec<([i32; 4], u8)>,
    /// The pictures controls are dragged as, with their text on them.
    pub drag_pictures: Vec<(usize, u16)>,
}

/// What the pointer drags, where it is and where it took hold, in classic
/// pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Dragging {
    pub grip: Grip,
    pub at: Vec2,
    pub from: Vec2,
}

macro_rules! by_index {
    ($($get:ident, $get_mut:ident, $as:ident, $as_mut:ident, $state:ty;)*) => {
        $(
            pub fn $get(&self, control: usize) -> Option<&$state> {
                self.controls.get(control)?.state.$as()
            }

            pub fn $get_mut(&mut self, control: usize) -> Option<&mut $state> {
                self.controls.get_mut(control)?.state.$as_mut()
            }
        )*
    };
}

impl View {
    /// The picture of a control being dragged, its text and where it
    /// stands: where the control would, gone with the pointer, kept on the
    /// screen above the bar.
    pub fn dragged_picture(&self, dialog: &Dialog) -> Option<(u16, usize, Vec2)> {
        let dragging = self.dragging?;
        let Grip::Control(c) = dragging.grip else { return None };
        let &(_, picture) = self.drag_pictures.iter().find(|&&(control, _)| control == c)?;
        Some((picture, c, dialog.controls.get(c)?.rect.min + dragging.at - dragging.from))
    }

    /// The box of the player being dragged, standing under the pointer,
    /// with a control of a box's size to draw it as.
    pub fn dragged_box(&self, dialog: &Dialog) -> Option<(usize, PlayerBox)> {
        let dragging = self.dragging?;
        let shown = match dragging.grip {
            Grip::Control(c) => self.controls.get(c)?.state.as_player()?.clone(),
            Grip::Row { control, row } => {
                let list = self.list(control)?;
                list.row_boxes.get(list.scroll + row)?.clone()?
            }
        };
        if !shown.filled {
            return None;
        }
        let size = dialog.controls.iter().position(|c| matches!(c.kind, Kind::Player))?;
        let half = dialog.controls[size].rect.size() / 2.0;
        // A dragged box shows no duty (0x50d59b).
        Some((size, PlayerBox { at: Some(dragging.at - half), duty: None, ..shown }))
    }

    pub fn new(id: u16, dialog: &Dialog) -> Self {
        let controls = dialog.controls.iter().map(ControlView::new).collect();
        Self {
            id,
            controls,
            focus: None,
            hovered: None,
            pressed: None,
            tip: None,
            inert: Vec::new(),
            dragging: None,
            drag_pictures: Vec::new(),
            lines: Vec::new(),
            lines_on_top: false,
            shades: Vec::new(),
        }
    }

    // A control's state by its index, if it is of that kind.
    by_index! {
        list, list_mut, as_list, as_list_mut, ListState;
        header, header_mut, as_header, as_header_mut, HeaderState;
        options, options_mut, as_options, as_options_mut, OptionsState;
        bar, bar_mut, as_bar, as_bar_mut, BarState;
        anim, anim_mut, as_anim, as_anim_mut, AnimState;
        value, value_mut, as_value, as_value_mut, Value;
        check, check_mut, as_check, as_check_mut, CheckState;
        paint, paint_mut, as_paint, as_paint_mut, PaintState;
        combo, combo_mut, as_combo, as_combo_mut, ComboState;
        bitmap, bitmap_mut, as_bitmap, as_bitmap_mut, BitmapState;
    }

    /// What a static, field or header says; nothing for other controls.
    pub fn text(&self, control: usize) -> &str {
        self.controls.get(control).and_then(|c| c.state.text()).unwrap_or("")
    }

    pub fn set_text(&mut self, control: usize, text: impl Into<String>) {
        if let Some(shown) = self.controls.get_mut(control).and_then(|c| c.state.text_mut()) {
            *shown = text.into();
        }
    }

    /// Lets a static, an animation or a bitmap answer the pointer, or not.
    pub fn set_clickable(&mut self, control: usize, clickable: bool) {
        match self.controls.get_mut(control).map(|c| &mut c.state) {
            Some(ControlState::Static(s)) => s.clickable = clickable,
            Some(ControlState::Anim(a)) => a.clickable = clickable,
            Some(ControlState::Bitmap(b)) => b.clickable = clickable,
            _ => {}
        }
    }

    pub fn disabled(&self, button: usize) -> bool {
        self.controls.get(button).and_then(|c| c.state.as_button()).is_some_and(|b| b.disabled)
    }

    pub fn set_disabled(&mut self, button: usize, disabled: bool) {
        if let Some(b) = self.controls.get_mut(button).and_then(|c| c.state.as_button_mut()) {
            b.disabled = disabled;
        }
    }

    pub fn set_picture(&mut self, bitmap: usize, picture: Option<Picture>) {
        if let Some(b) = self.controls.get_mut(bitmap).and_then(|c| c.state.as_bitmap_mut()) {
            b.picture = picture;
        }
    }

    pub fn set_player(&mut self, control: usize, player: PlayerBox) {
        if let Some(ControlState::Player(shown)) =
            self.controls.get_mut(control).map(|c| &mut c.state)
        {
            *shown = Some(player);
        }
    }

    /// The chosen option of a group, or the bar's symbol of the open screen.
    pub fn chosen(&self, control: usize) -> usize {
        match self.controls.get(control).map(|c| &c.state) {
            Some(ControlState::Options(o)) => o.chosen,
            Some(ControlState::Bar(b)) => b.chosen,
            _ => 0,
        }
    }

    pub fn set_chosen(&mut self, control: usize, chosen: usize) {
        match self.controls.get_mut(control).map(|c| &mut c.state) {
            Some(ControlState::Options(o)) => o.chosen = chosen,
            Some(ControlState::Bar(b)) => b.chosen = chosen,
            _ => {}
        }
    }

    /// What a click or key on `target` does to the controls themselves:
    /// choosing an option, selecting a row, focusing a field, scrolling a
    /// list, stepping a spinner or slider. Whether the flow gets to hear of
    /// it: not for a disabled button, an option that is off, a row with
    /// nothing in it, a scrollbar or a slider's track.
    pub fn press(&mut self, dialog: &Dialog, target: Target) -> bool {
        // A dropped list takes the click: a row chooses, an arrow scrolls,
        // anything else puts it up.
        if let Some(open) =
            self.controls.iter().position(|c| c.state.as_combo().is_some_and(|c| c.open))
        {
            let control = &dialog.controls[open];
            let combo = self.combo_mut(open).expect("a combo box");
            return match target {
                Target::Row { control: c, row } if c == open => combo.press(row),
                Target::Scroll { control: c, up } if c == open => {
                    combo.scroll_by(if up { -1 } else { 1 }, control);
                    false
                }
                _ => {
                    combo.open = false;
                    false
                }
            };
        }
        match target {
            Target::Button(button) => !self.disabled(button),
            Target::Option { control, item } => match self.controls.get_mut(control) {
                Some(ControlView { state: ControlState::Options(o), .. }) => o.press(item),
                Some(ControlView { state: ControlState::Bar(b), .. }) => b.item_on(item),
                Some(ControlView { state: ControlState::Menu(m), .. }) => m.at.is_some(),
                _ => false,
            },
            Target::Cell { control, row, .. } => {
                self.list_mut(control).is_some_and(|l| l.press(row))
            }
            Target::Row { control, row } => match self.combo_mut(control) {
                // A script chooses a combo box's row without dropping it.
                Some(combo) => {
                    let row = row.saturating_sub(combo.scroll);
                    combo.press(row)
                }
                None => self.list_mut(control).is_some_and(|l| l.press(row)),
            },
            Target::Combo(control) => {
                if let Some(combo) = self.combo_mut(control) {
                    combo.toggle(&dialog.controls[control]);
                }
                false
            }
            Target::Edit(control) => {
                let disabled = self
                    .controls
                    .get(control)
                    .and_then(|c| c.state.as_edit())
                    .is_some_and(|e| e.disabled);
                if !disabled {
                    self.focus = Some(control);
                }
                !disabled
            }
            Target::Scroll { control, up } => {
                let visible = visible_rows(&dialog.controls[control]);
                if let Some(list) = self.list_mut(control) {
                    list.scroll_by(if up { -1 } else { 1 }, visible);
                }
                false
            }
            Target::Spin { control, up } | Target::SliderStep { control, up } => {
                if let Some(value) = self.value_mut(control) {
                    value.step(up);
                }
                true
            }
            Target::SliderAt { control, value } => {
                if let Some(shown) = self.value_mut(control) {
                    shown.set(value);
                }
                true
            }
            Target::SliderTrack(_) => false,
            Target::Control(_) | Target::Drop { .. } | Target::Menu(_) => true,
            Target::Check(control) => match self.controls.get_mut(control) {
                Some(ControlView { state: ControlState::Check(check), .. }) if !check.disabled => {
                    check.checked = !check.checked;
                    true
                }
                _ => false,
            },
        }
    }

    /// Types into the focused field.
    pub fn type_text(&mut self, dialog: &Dialog, typed: &str) {
        let Some(focus) = self.focus else { return };
        let Kind::Edit { max_len } = dialog.controls[focus].kind else { return };
        if let Some(edit) = self.controls[focus].state.as_edit_mut() {
            edit.type_text(typed, max_len);
        }
    }

    pub fn backspace(&mut self) {
        let edit = self.focus.and_then(|f| self.controls[f].state.as_edit_mut());
        if let Some(edit) = edit {
            edit.backspace();
        }
    }

    /// Tab: on to the next field, round the end.
    pub fn next_field(&mut self, dialog: &Dialog) {
        let Some(focus) = self.focus else { return };
        let edits: Vec<usize> = (0..dialog.controls.len())
            .filter(|&i| matches!(dialog.controls[i].kind, Kind::Edit { .. }))
            .collect();
        let next = edits.iter().position(|&i| i == focus).map_or(0, |p| (p + 1) % edits.len());
        self.focus = edits.get(next).copied();
    }

    /// Arrow keys: through the bulleted options if the dialog has some, else
    /// through the rows of its first list.
    pub fn step(&mut self, dialog: &Dialog, step: i32) {
        let first = |kind: fn(&Kind) -> bool| dialog.controls.iter().position(|c| kind(&c.kind));
        if let Some(options) = first(|k| matches!(k, Kind::Options { image: 0, .. })) {
            let Kind::Options { items, .. } = &dialog.controls[options].kind else { return };
            if let Some(state) = self.options_mut(options) {
                state.step(step, items.len());
            }
        } else if let Some(list) = first(|k| matches!(k, Kind::ListBox { .. })) {
            let visible = visible_rows(&dialog.controls[list]);
            if let Some(state) = self.list_mut(list) {
                state.step(step, visible);
            }
        }
    }

    /// The wheel scrolls the list under the pointer, by notches up.
    pub fn wheel(&mut self, dialog: &Dialog, notches: i32) {
        let list = match self.hovered {
            Some(Target::Row { control, .. } | Target::Scroll { control, .. }) => control,
            _ => return,
        };
        let Some(control) = dialog.controls.get(list) else { return };
        let visible = visible_rows(control);
        if let Some(state) = self.list_mut(list) {
            state.scroll_by(-notches * WHEEL_ROWS, visible);
        }
    }

    /// Whether an animation is playing.
    pub fn moving(&self) -> bool {
        self.controls.iter().filter_map(|c| c.state.as_anim()).any(AnimState::moving)
    }

    /// Playing animations move on to their next frame.
    pub fn advance(&mut self) {
        self.controls.iter_mut().filter_map(|c| c.state.as_anim_mut()).for_each(AnimState::advance);
    }
}
