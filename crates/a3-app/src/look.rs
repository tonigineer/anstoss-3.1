//! The two looks, switched with F9 like the special editions of Monkey Island:
//! the classic 640×480 original, pixel for pixel, and a remastered 16:9
//! rendition drawn at full resolution.
//!
//! Every screen is laid out on one 1920×1080 canvas. The classic screen sits in
//! its middle at 2.25× — 1440×1080 — so both looks put each element in the
//! same place. Both are always rendered: the remastered look underneath, the
//! classic one on top with black pillarboxes, and switching crossfades the
//! classic layer in or out. A screen without a remastered look shows the
//! classic one in both, and F9 does nothing there.
//!
//! Classic frames are uploaded at their own resolution and scaled on the GPU
//! by [`SharpMaterial`], so a change costs one small texture upload.

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::UiMaterialPlugin;
use bevy::ui_render::ui_material::{MaterialNode, UiMaterial};
use bevy::window::PrimaryWindow;
use image::imageops;
use image::{DynamicImage, RgbaImage};

use crate::Screen;

pub const CANVAS: Vec2 = Vec2::new(1920.0, 1080.0);
pub const CLASSIC: Vec2 = Vec2::new(640.0, 480.0);
pub const CLASSIC_SCALE: f32 = CANVAS.y / CLASSIC.y;
pub const CLASSIC_LEFT: f32 = (CANVAS.x - CLASSIC.x * CLASSIC_SCALE) / 2.0;

const SWITCH_SECS: f32 = 0.6;

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "sharp.wesl");
    app.add_plugins(UiMaterialPlugin::<SharpMaterial>::default())
        .init_resource::<Look>()
        .add_systems(Update, (fit_to_window, switch, (animate, fade_classic).chain()));
}

#[derive(Resource)]
pub struct Look {
    /// The look the player chose.
    remastered: bool,
    /// Whether the screen on show has a remastered look.
    offered: bool,
    /// 0 is classic, 1 remastered; in between while switching.
    blend: f32,
}

impl Default for Look {
    fn default() -> Self {
        Self { remastered: false, offered: true, blend: 0.0 }
    }
}

impl Look {
    /// Whether the screen on show is in the remastered look.
    pub fn showing_remastered(&self) -> bool {
        self.remastered && self.offered
    }

    pub fn offered(&self) -> bool {
        self.offered
    }

    /// A screen comes up with a remastered look or without one; it shows in
    /// its look at once.
    pub fn offer(&mut self, offered: bool) {
        self.offered = offered;
        self.blend = self.target();
    }

    /// F9: the other look, if the screen on show has both; whether it
    /// switched.
    pub fn switch(&mut self) -> bool {
        if self.offered {
            self.remastered = !self.remastered;
        }
        self.offered
    }

    fn target(&self) -> f32 {
        if self.showing_remastered() { 1.0 } else { 0.0 }
    }
}

/// Pixel art scaled to any size with every source pixel kept a crisp square
/// ("sharp bilinear", see `sharp.wesl`), tinted for the crossfade.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SharpMaterial {
    #[uniform(0)]
    tint: LinearRgba,
    #[texture(1)]
    #[sampler(2)]
    pixels: Handle<Image>,
}

impl UiMaterial for SharpMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://anstoss3_menu/sharp.wesl".into()
    }
}

/// Part of the classic layer; fades out as the remastered look fades in.
#[derive(Component)]
pub struct ClassicLayer;

/// Scales the canvas to the largest size that fits the window.
fn fit_to_window(window: Single<&Window, With<PrimaryWindow>>, mut scale: ResMut<UiScale>) {
    let fit = (window.width() / CANVAS.x).min(window.height() / CANVAS.y);
    if fit > 0.0 && scale.0 != fit {
        scale.0 = fit;
    }
}

fn switch(keys: Res<ButtonInput<KeyCode>>, mut look: ResMut<Look>) {
    if keys.just_pressed(KeyCode::F9) && look.switch() {
        info!("{} look", if look.remastered { "Remastered" } else { "Classic" });
    }
}

fn animate(time: Res<Time>, mut look: ResMut<Look>) {
    let target = look.target();
    if look.blend != target {
        let step = time.delta_secs() / SWITCH_SECS;
        look.blend = if target > look.blend {
            (look.blend + step).min(target)
        } else {
            (look.blend - step).max(target)
        };
    }
}

fn fade_classic(
    look: Res<Look>,
    screens: Query<&MaterialNode<SharpMaterial>, With<ClassicLayer>>,
    mut materials: ResMut<Assets<SharpMaterial>>,
    mut fills: Query<&mut BackgroundColor, With<ClassicLayer>>,
) {
    let t = look.blend;
    let alpha = 1.0 - t * t * (3.0 - 2.0 * t);
    for screen in &screens {
        let unchanged = materials.get(&screen.0).is_none_or(|m| m.tint.alpha == alpha);
        if !unchanged && let Some(mut material) = materials.get_mut(&screen.0) {
            material.tint.alpha = alpha;
        }
    }
    for mut fill in &mut fills {
        if fill.0.alpha() != alpha {
            fill.0.set_alpha(alpha);
        }
    }
}

/// Spawns a canvas centred in the window and letterboxed in black, which lives
/// until `screen` is left. Returns the canvas to put the screen's UI on.
pub fn spawn_stage(commands: &mut Commands, screen: Screen) -> Entity {
    let stage = commands
        .spawn(Node {
            width: Val::Px(CANVAS.x),
            height: Val::Px(CANVAS.y),
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    commands
        .spawn((
            DespawnOnExit(screen),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .add_child(stage);
    stage
}

/// Spawns the classic layer: `screen` filling the 4:3 middle of the canvas,
/// with black pillarboxes left and right. Returns the material showing it,
/// to be [`touch`]ed whenever the screen's pixels change.
pub fn spawn_classic_layer(
    commands: &mut Commands,
    stage: Entity,
    materials: &mut Assets<SharpMaterial>,
    screen: Handle<Image>,
) -> Handle<SharpMaterial> {
    let right = CLASSIC_LEFT + CLASSIC.x * CLASSIC_SCALE;
    for x in [0.0, right] {
        commands.spawn((
            ClassicLayer,
            BackgroundColor(Color::BLACK),
            canvas_rect(x, 0.0, CLASSIC_LEFT, CANVAS.y),
            ZIndex(1),
            Pickable::IGNORE,
            ChildOf(stage),
        ));
    }
    let material = materials.add(SharpMaterial { tint: LinearRgba::WHITE, pixels: screen });
    commands.spawn((
        ClassicLayer,
        MaterialNode(material.clone()),
        classic_rect(0.0, 0.0, CLASSIC.x, CLASSIC.y),
        ZIndex(1),
        Pickable::IGNORE,
        ChildOf(stage),
    ));
    material
}

/// Replaces the pixels of a classic screen with a frame of the same size, and
/// has its material pick the new texture up.
pub fn show_frame(
    images: &mut Assets<Image>,
    materials: &mut Assets<SharpMaterial>,
    screen: &Handle<Image>,
    material: &Handle<SharpMaterial>,
    frame: RgbaImage,
) {
    if let Some(mut image) = images.get_mut(screen) {
        image.data = Some(frame.into_raw());
    }
    // A material is rebuilt when it changes, not when its texture does.
    materials.get_mut(material);
}

/// A node covering a rectangle of the canvas, in canvas pixels.
pub fn canvas_rect(x: f32, y: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(x),
        top: Val::Px(y),
        width: Val::Px(width),
        height: Val::Px(height),
        ..default()
    }
}

/// A node covering a rectangle of the classic 640×480 screen, in its pixels.
pub fn classic_rect(x: f32, y: f32, width: f32, height: f32) -> Node {
    canvas_rect(
        CLASSIC_LEFT + x * CLASSIC_SCALE,
        y * CLASSIC_SCALE,
        width * CLASSIC_SCALE,
        height * CLASSIC_SCALE,
    )
}

/// Fades the outer `width` columns on both sides to transparent, so a picture
/// blends into whatever is drawn behind it.
pub fn feather_sides(pixels: &mut RgbaImage, width: u32) {
    let (w, h) = pixels.dimensions();
    for x in 0..width.min(w / 2) {
        let fade = (x as f32 + 0.5) / width as f32;
        for column in [x, w - 1 - x] {
            for y in 0..h {
                let alpha = &mut pixels.get_pixel_mut(column, y)[3];
                *alpha = (f32::from(*alpha) * fade) as u8;
            }
        }
    }
}

/// Extends a picture of the classic screen, already scaled to the canvas
/// height, to the full canvas width. Next to the seam each side mirrors the
/// picture's edge, quickly going out of focus; further out it settles into
/// the tone of that edge, row by row, so neither figures nor painted frames
/// get reflected into the extension.
pub fn fill_wide(art: &RgbaImage) -> RgbaImage {
    let (width, height) = (CANVAS.x as u32, CANVAS.y as u32);
    let pad = width.saturating_sub(art.width()) / 2;
    let mut out = RgbaImage::new(width, height);
    imageops::overlay(&mut out, art, i64::from(pad), 0);
    if pad == 0 {
        return out;
    }
    for left in [true, false] {
        // Column `x` of the strip is `d` pixels from the seam and mirrors the
        // picture's column `d - 1` from that edge.
        let distance = |x: u32| if left { pad - x } else { x + 1 };
        let source = |x: u32| if left { distance(x) - 1 } else { art.width() - distance(x) };
        let strip = RgbaImage::from_fn(pad, height, |x, y| *art.get_pixel(source(x), y));
        let blurred = imageops::fast_blur(&strip, 24.0);
        let tone = edge_tone(&strip);
        let offset = if left { 0 } else { pad + art.width() };
        for (x, y, sharp) in strip.enumerate_pixels() {
            let out_there = distance(x) as f32 / pad as f32;
            let blur = (out_there / 0.3).min(1.0);
            let settle = 0.8 * smoothstep(0.1, 0.85, out_there);
            let shade = 1.0 - 0.08 * out_there;
            let soft = blurred.get_pixel(x, y);
            let pixel = std::array::from_fn(|i| {
                if i == 3 {
                    return 255;
                }
                let focused = f32::from(sharp[i]) * (1.0 - blur) + f32::from(soft[i]) * blur;
                let settled = focused * (1.0 - settle) + tone[y as usize][i] * settle;
                (settled * shade).round() as u8
            });
            out.put_pixel(offset + x, y, image::Rgba(pixel));
        }
    }
    out
}

/// The average colour of each row of `strip`, smoothed over neighbouring rows
/// so it changes as gently down the side as the picture does.
fn edge_tone(strip: &RgbaImage) -> Vec<[f32; 3]> {
    let (w, h) = strip.dimensions();
    let rows: Vec<[f32; 3]> = (0..h)
        .map(|y| {
            let mut sum = [0.0; 3];
            for x in 0..w {
                let p = strip.get_pixel(x, y);
                for i in 0..3 {
                    sum[i] += f32::from(p[i]);
                }
            }
            sum.map(|s| s / w as f32)
        })
        .collect();
    const REACH: i64 = 60;
    (0..i64::from(h))
        .map(|y| {
            let span = (y - REACH).max(0)..(y + REACH + 1).min(i64::from(h));
            let n = (span.end - span.start) as f32;
            let mut sum = [0.0; 3];
            for row in &rows[span.start as usize..span.end as usize] {
                for i in 0..3 {
                    sum[i] += row[i];
                }
            }
            sum.map(|s| s / n)
        })
        .collect()
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Scale2x (EPX): doubles pixel art and rounds off its staircases, a cleaner
/// start than blurring a sprite straight up.
pub fn scale2x(src: &RgbaImage) -> RgbaImage {
    let (w, h) = src.dimensions();
    let at = |x: i64, y: i64| {
        *src.get_pixel(x.clamp(0, i64::from(w) - 1) as u32, y.clamp(0, i64::from(h) - 1) as u32)
    };
    let mut out = RgbaImage::new(w * 2, h * 2);
    for y in 0..i64::from(h) {
        for x in 0..i64::from(w) {
            let (p, up, right, left, down) =
                (at(x, y), at(x, y - 1), at(x + 1, y), at(x - 1, y), at(x, y + 1));
            let corners = [
                if left == up && left != down && up != right { up } else { p },
                if up == right && up != left && right != down { right } else { p },
                if down == left && down != right && left != up { left } else { p },
                if right == down && right != up && down != left { down } else { p },
            ];
            let (ox, oy) = (2 * x as u32, 2 * y as u32);
            for (i, pixel) in corners.into_iter().enumerate() {
                out.put_pixel(ox + (i as u32 % 2), oy + (i as u32 / 2), pixel);
            }
        }
    }
    out
}

/// An image filtered linearly when scaled.
pub fn smooth_image(pixels: RgbaImage) -> Image {
    let mut image =
        Image::from_dynamic(DynamicImage::ImageRgba8(pixels), true, RenderAssetUsages::default());
    image.sampler = ImageSampler::linear();
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f9_does_nothing_on_a_screen_without_a_remastered_look() {
        let mut look = Look::default();
        assert!(look.switch() && look.showing_remastered());
        // The classic look shows at once, and stays.
        look.offer(false);
        assert!(!look.showing_remastered() && look.blend == 0.0);
        assert!(!look.switch() && look.remastered);
        // The next screen with a remastered look shows it, as chosen.
        look.offer(true);
        assert!(look.showing_remastered() && look.blend == 1.0);
    }
}
