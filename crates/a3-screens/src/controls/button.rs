//! Bitmap buttons: a picture as they stand and one for when the pointer is
//! on them.

use image::RgbaImage;

use a3_files::rch::{Control, Dialog, Kind};

use crate::classic::{Paint, paste};
use crate::pointer::Target;

/// Graphic ids of the OK and cancel buttons' normal images.
pub const OK_IMAGE: u16 = 1272;
pub const CANCEL_IMAGE: u16 = 1260;

#[derive(Clone, Default, Debug)]
pub struct ButtonState {
    /// A disabled button ignores the pointer.
    pub disabled: bool,
}

impl ButtonState {
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let Kind::Button { normal, hover } = paint.control.kind else { return };
        let target = Target::Button(paint.index);
        let pointed = paint.pointed(target) && !self.disabled;
        // The objects of a scene change only while they are held down.
        let pointed = pointed && (!paint.scene || paint.view.pressed == Some(target));
        if let Some(sprite) = paint.image(if pointed { hover } else { normal }) {
            paste(frame, sprite, paint.control.rect.min);
        }
    }
}

pub(crate) fn graphics(control: &Control) -> (Vec<u16>, bool) {
    match control.kind {
        Kind::Button { normal, hover } => (vec![normal, hover], true),
        _ => (Vec::new(), true),
    }
}

/// The button of a dialog that shows `image`, like OK for Enter and cancel
/// for Escape.
pub fn with_image(dialog: &Dialog, image: u16) -> Option<usize> {
    dialog
        .controls
        .iter()
        .position(|c| matches!(c.kind, Kind::Button { normal, .. } if normal == image))
}
