// An event's depth of field: a texture quad with its own alpha. See focus.rs.

struct Vertex {
    @location(0) clip: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) alpha: f32,
}

struct Output {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var bilinear: sampler;

@vertex
fn vertex(in: Vertex) -> Output {
    var out: Output;
    out.clip = in.clip;
    out.uv = in.uv;
    out.alpha = in.alpha;
    return out;
}

// The scene's alpha is the glow's stencil, not a texture alpha.
@fragment
fn fragment(in: Output) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(source, bilinear, in.uv).rgb, in.alpha);
}
