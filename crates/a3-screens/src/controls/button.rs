//! Bitmap buttons: a picture as they stand and one for when the pointer is
//! on them. Push buttons have their text in the middle, on the dialog's
//! picture, their own picture showing them held down.

use image::RgbaImage;

use a3_files::rch::{Control, Dialog, Kind};

use crate::classic::{Paint, TEXT, TEXT_HOVER, centred, paste};
use crate::pointer::Target;

/// Graphic ids of the OK and cancel buttons' normal images.
pub const OK_IMAGE: u16 = 1272;
pub const CANCEL_IMAGE: u16 = 1260;

#[derive(Clone, Default, Debug)]
pub struct ButtonState {
    /// A disabled button ignores the pointer.
    pub disabled: bool,
    /// Other pictures than the dialog file's, normal and pointed at, and
    /// those to have ready for them.
    pub images: Option<(u16, u16)>,
    pub ready: Vec<u16>,
}

impl ButtonState {
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let (normal, hover, labelled) = match paint.control.kind {
            Kind::Button { normal, hover } => (normal, hover, false),
            Kind::PushButton { normal, hover } => (normal, hover, true),
            _ => return,
        };
        let (normal, hover) = self.images.unwrap_or((normal, hover));
        let target = Target::Button(paint.index);
        let pointed = paint.pointed(target) && !self.disabled;
        // The objects of a scene change only while they are held down.
        let pointed = pointed && (!paint.scene || paint.view.pressed == Some(target));
        let r = paint.control.rect;
        // A push button is the dialog's picture; its own shows it held down.
        let held = paint.view.pressed == Some(target);
        if (!labelled || held)
            && let Some(sprite) = paint.image(if pointed { hover } else { normal })
        {
            paste(frame, sprite, r.min);
        }
        if let (true, Some(font)) = (labelled, paint.font()) {
            let text = &paint.control.text;
            let left = centred(r.min.x as i32, r.width() as i32, font.width(text, false) as i32);
            let top = r.min.y as i32 + (r.height() as i32 - font.height() as i32) / 2;
            font.draw(frame, left, top, text, if pointed { TEXT_HOVER } else { TEXT }, false);
        }
    }
}

pub(crate) fn graphics(state: &ButtonState, control: &Control) -> (Vec<u16>, bool) {
    let (mut ids, sprite) = file_graphics(control);
    ids.extend(&state.ready);
    (ids, sprite)
}

fn file_graphics(control: &Control) -> (Vec<u16>, bool) {
    match control.kind {
        Kind::Button { normal, hover } => (vec![normal, hover], true),
        Kind::PushButton { normal, hover } => (vec![normal, hover], false),
        _ => (Vec::new(), true),
    }
}

/// The button of a dialog that shows `image`.
pub fn with_image(dialog: &Dialog, image: u16) -> Option<usize> {
    dialog
        .controls
        .iter()
        .position(|c| matches!(c.kind, Kind::Button { normal, .. } if normal == image))
}

/// The ids Windows gives OK and cancel, which the game's buttons keep in
/// every look of theirs — the seasons' among them (`OkHerb1.bmp` …).
const IDOK: u16 = 1;
const IDCANCEL: u16 = 2;

/// Whether a control is an OK button: by its picture or its id.
pub fn is_ok(control: &Control) -> bool {
    is_button(control, OK_IMAGE, IDOK)
}

pub fn is_cancel(control: &Control) -> bool {
    is_button(control, CANCEL_IMAGE, IDCANCEL)
}

fn is_button(control: &Control, image: u16, id: u16) -> bool {
    match control.kind {
        Kind::Button { normal, .. } => normal == image || control.id == Some(id),
        _ => false,
    }
}

/// A dialog's OK button, for Enter; its cancel button, for Escape.
pub fn ok_button(dialog: &Dialog) -> Option<usize> {
    dialog.controls.iter().position(is_ok)
}

pub fn cancel_button(dialog: &Dialog) -> Option<usize> {
    dialog.controls.iter().position(is_cancel)
}
