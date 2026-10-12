// Sharp bilinear: draws pixel art scaled by any factor, whole or not, with
// every source pixel a solid square. Only the screen pixels on the border
// between two source pixels are blended, so nothing shimmers or doubles.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct SharpMaterial {
    tint: vec4<f32>,
}

@group(1) @binding(0)
var<uniform> material: SharpMaterial;
@group(1) @binding(1)
var pixels: texture_2d<f32>;
@group(1) @binding(2)
var pixels_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(pixels));
    let texel = in.uv * size;
    // Screen pixels per source pixel, from how fast `texel` moves on screen.
    let step = max(abs(vec2<f32>(dpdx(texel.x), dpdy(texel.y))), vec2<f32>(1e-5));
    let scale = max(1.0 / step, vec2<f32>(1.0));
    // Inside `region` of its centre a pixel is sampled at its centre, solid;
    // beyond, the sample slides to the border with the neighbour, reaching it
    // exactly at the pixel's edge, so the blend spans one screen pixel.
    let region = 0.5 - 0.5 / scale;
    let from_centre = fract(texel) - 0.5;
    let slide = (from_centre - clamp(from_centre, -region, region)) * scale + 0.5;
    return textureSample(pixels, pixels_sampler, (floor(texel) + slide) / size) * material.tint;
}
