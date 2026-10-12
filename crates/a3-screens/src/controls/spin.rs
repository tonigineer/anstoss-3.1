//! Spinners: the tiny up and down arrows next to a number, painted into the
//! background. Their value, which sliders share.

use bevy_math::Rect;

use a3_files::rch::Control;

use crate::pointer::{Geometry, Target};

/// A spinner's or slider's value, within its range.
#[derive(Clone, Copy, Default, Debug)]
pub struct Value {
    pub value: i32,
    pub range: (i32, i32),
}

impl Value {
    pub fn step(&mut self, up: bool) {
        self.set(self.value + if up { 1 } else { -1 });
    }

    pub fn set(&mut self, value: i32) {
        let (min, max) = self.range;
        self.value = value.clamp(min, max.max(min));
    }
}

/// The upper part steps up, the lower part down.
pub(crate) fn targets(
    index: usize,
    control: &Control,
    geometry: &dyn Geometry,
    out: &mut Vec<(Target, Rect)>,
) {
    let (up, down) = geometry.spin(index, control);
    out.push((Target::Spin { control: index, up: true }, up));
    out.push((Target::Spin { control: index, up: false }, down));
}
