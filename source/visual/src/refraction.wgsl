// Frame-buffer effects: the copied frame x vertex colour, sampled at a screen
// place that runs linear on the screen (the exe's unnormalised UVs). See
// refraction.rs.

struct Vertex {
    @location(0) clip: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) colour: vec4<f32>,
}

struct Output {
    @builtin(position) clip: vec4<f32>,
    @location(0) @interpolate(linear) uv: vec2<f32>,
    @location(1) colour: vec4<f32>,
}

@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var bilinear: sampler;

@vertex
fn vertex(in: Vertex) -> Output {
    var out: Output;
    out.clip = in.clip;
    out.uv = in.uv;
    out.colour = in.colour;
    return out;
}

// The copy's alpha is the glow's stencil, not a texture alpha: the blend uses
// the vertex's alone.
@fragment
fn fragment(in: Output) -> @location(0) vec4<f32> {
    let c = textureSample(frame, bilinear, in.uv);
    return vec4<f32>(c.rgb * in.colour.rgb, in.colour.a);
}
