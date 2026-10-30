#import bevy_pbr::{
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
}

struct FogExt {
    param: vec4<f32>,
    colour: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> fog: FogExt;

fn to_gamma(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(vertex_output: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(vertex_output, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    if fog.colour.w > 0.5 {
        let w = -(view.view_from_world * vec4<f32>(vertex_output.world_position.xyz, 1.0)).z;
        let f = clamp(fog.param.x * w + fog.param.y, fog.param.z, fog.param.w);
        let c = to_gamma(out.color.rgb) * f + fog.colour.rgb * (1.0 - f);
        out.color = vec4<f32>(to_linear(c), out.color.a);
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#ifdef VERTEX_UVS_B
    // A map or sky opaque triangle's glow class, the exe's stencil (glow.rs).
    out.color.a = vertex_output.uv_b.x;
#endif
    return out;
}
