// dk::Glow's quads: program 4 (pixel shader 026, texture x vertex colour with
// the colour at 1.0) in the room's terms. See glow.rs.

struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct Output {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var bilinear: sampler;

@vertex
fn vertex(in: Vertex) -> Output {
    var out: Output;
    out.clip = vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn copy(in: Output) -> @location(0) vec4<f32> {
    return textureSample(source, bilinear, in.uv);
}

// The stencil test (== 0xff): the mask is this pixel's alpha, however the
// colour's half-texel sample falls.
@fragment
fn masked(in: Output) -> @location(0) vec4<f32> {
    let colour = textureSample(source, bilinear, in.uv);
    let stencil = textureLoad(source, vec2<i32>(in.clip.xy), 0).a;
    if stencil < 0.5 {
        discard;
    }
    return vec4<f32>(colour.rgb, 1.0);
}

@fragment
fn opaque() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
