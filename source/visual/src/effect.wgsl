// Effect draws as the PC's pixel shader 026 shades them: texel x vertex
// colour on the stored values (vertex colours are PS2 bytes / 128, so
// 0x80 = 1.0), the effects' GS alpha test (alpha > 0), clamped as a UNORM
// target clamps, then shaped for the pipeline's blend (effect_draw.rs).
#import bevy_pbr::forward_io::VertexOutput

struct Params {
    // x: 0 alpha, 1 add, 2 replace, 3 subtract; y: textured; z: sRGB target.
    v: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: Params;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var page: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var page_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var c = vec4<f32>(1.0);
#ifdef VERTEX_COLORS
    c = in.color;
#endif
    if params.v.y > 0.5 {
#ifdef VERTEX_UVS_A
        c = c * textureSample(page, page_sampler, in.uv);
#endif
    }
    if c.a <= 0.0 {
        discard;
    }
    c = clamp(c, vec4<f32>(0.0), vec4<f32>(1.0));
    var rgb = c.rgb;
    if params.v.z < 0.5 {
        // A linear-light target: hand the blender linear values.
        rgb = pow(rgb, vec3<f32>(2.2));
    }
    let mode = params.v.x;
    if mode < 0.5 {
        // Straight alpha: Cs·As + Cd·(1 − As).
        return vec4<f32>(rgb, c.a);
    }
    if mode < 1.5 {
        // Premultiplied pipeline with alpha 0: Cs·As + Cd.
        return vec4<f32>(rgb * c.a, 0.0);
    }
    if mode < 2.5 {
        // Premultiplied pipeline with alpha 1: Cs·As, the frame dropped.
        return vec4<f32>(rgb * c.a, 1.0);
    }
    // Reverse subtract: Cd − Cs·As.
    return vec4<f32>(rgb * c.a, 0.0);
}
