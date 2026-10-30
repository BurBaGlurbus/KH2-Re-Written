//! KH3's Sora and Keyblade shaded by their own material shaders: the pixel shaders are translated at load
//! (`data::kh3::charmat`) and run here on the mesh's interpolants. The game shades deferred; here base pass and light run
//! in one forward fragment, as StrawHatPro's `kh-look/shaders/char_common.wgsl` (MIT) does. The BRDF is stock UE 4.17:
//! Lambert + GGX / Smith joint approximation / Schlick, with the stock Subsurface transmission term. The lights are the
//! duel's: KH3's level lighting and its `KHS` post process are not here.

use super::*;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::{Shader, ShaderRef};
use data::kh3::charmat::{self, CharMaterials, Owner, Slot};

pub(crate) const SHADER: Handle<Shader> =
    bevy::asset::uuid_handle!("5ac7e1d2-3b84-4f0e-9a61-7e2c54d0b3a1");

/// Rows of the material constant buffer and of a parameter collection.
const ROWS: usize = 192;
const COLLECTION_ROWS: usize = 32;
const TEXTURES: usize = 14;

#[derive(Clone, Copy, ShaderType, Debug)]
pub(crate) struct Params {
    m: [Vec4; ROWS],
    c0: [Vec4; COLLECTION_ROWS],
    /// Primitive: WorldToLocal rows, object position + radius, box extent, actor, local bounds.
    w2l: [Vec4; 4],
    obj: Vec4,
    bounds: Vec4,
    actor: Vec4,
    bmin: Vec4,
    bmax: Vec4,
    /// x: centimetres per room unit, y: seconds.
    info: Vec4,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            m: [Vec4::ZERO; ROWS],
            c0: [Vec4::ZERO; COLLECTION_ROWS],
            w2l: [Vec4::X, Vec4::Y, Vec4::Z, Vec4::W],
            obj: Vec4::ZERO,
            bounds: Vec4::new(100., 100., 100., 1.),
            actor: Vec4::ZERO,
            bmin: Vec4::new(-100., -100., 0., 0.),
            bmax: Vec4::new(100., 100., 200., 0.),
            info: Vec4::new(1., 0., 0., 0.),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    owner: u16,
    masked: bool,
    translucent: bool,
    subsurface: bool,
    two_sided: bool,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(Key)]
pub(crate) struct Kh3Material {
    #[uniform(0)]
    pub(crate) params: Params,
    #[texture(1)]
    t0: Option<Handle<Image>>,
    #[texture(2)]
    t1: Option<Handle<Image>>,
    #[texture(3)]
    t2: Option<Handle<Image>>,
    #[texture(4)]
    t3: Option<Handle<Image>>,
    #[texture(5)]
    t4: Option<Handle<Image>>,
    #[texture(6)]
    t5: Option<Handle<Image>>,
    #[texture(7)]
    t6: Option<Handle<Image>>,
    #[texture(8)]
    t7: Option<Handle<Image>>,
    #[texture(9)]
    t8: Option<Handle<Image>>,
    #[texture(10)]
    t9: Option<Handle<Image>>,
    #[texture(11)]
    t10: Option<Handle<Image>>,
    #[texture(12)]
    t11: Option<Handle<Image>>,
    #[texture(13)]
    t12: Option<Handle<Image>>,
    #[texture(14)]
    t13: Option<Handle<Image>>,
    #[texture(20)]
    #[sampler(21)]
    wrap: Option<Handle<Image>>,
    #[texture(22)]
    #[sampler(23)]
    clamp: Option<Handle<Image>>,
    key: Key,
}

impl From<&Kh3Material> for Key {
    fn from(m: &Kh3Material) -> Self {
        m.key
    }
}

impl Material for Kh3Material {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(SHADER)
    }

    fn alpha_mode(&self) -> AlphaMode {
        if self.key.translucent {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        }
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let k = key.bind_group_data;
        descriptor.primitive.cull_mode = if k.two_sided { None } else { Some(Face::Back) };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment
                .shader_defs
                .push(format!("KH_C{}", k.owner).as_str().into());
            if k.masked {
                fragment.shader_defs.push("KH_MASKED".into());
            }
            if k.translucent {
                fragment.shader_defs.push("KH_TRANSLUCENT".into());
            }
            if k.subsurface {
                fragment.shader_defs.push("KH_SUBSURFACE".into());
            }
        }
        if visual::glow::keeps_alpha(key.mesh_key) {
            visual::glow::keep_alpha(descriptor);
        }
        Ok(())
    }
}

const HARNESS: &str = r#"
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
}
#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

struct KhParams {
    m: array<vec4<f32>, 192>,
    c0: array<vec4<f32>, 32>,
    w2l: array<vec4<f32>, 4>,
    obj: vec4<f32>,
    bounds: vec4<f32>,
    actor: vec4<f32>,
    bmin: vec4<f32>,
    bmax: vec4<f32>,
    info: vec4<f32>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> kh: KhParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var kh_t0: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var kh_t1: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var kh_t2: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var kh_t3: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var kh_t4: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var kh_t5: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var kh_t6: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var kh_t7: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(9) var kh_t8: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(10) var kh_t9: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(11) var kh_t10: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(12) var kh_t11: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(13) var kh_t12: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(14) var kh_t13: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(20) var kh_wrap_image: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(21) var kh_swrap: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(22) var kh_clamp_image: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(23) var kh_sclamp: sampler;

struct KhIn {
    tc0: vec4<f32>, tc1: vec4<f32>, tc2: vec4<f32>, tc3: vec4<f32>, tc4: vec4<f32>, tc5: vec4<f32>, tc6: vec4<f32>,
    tc7: vec4<f32>, tc8: vec4<f32>, tc9: vec4<f32>, tc10: vec4<f32>, tc11: vec4<f32>, tc12: vec4<f32>,
    col0: vec4<f32>, svpos: vec4<f32>, front: vec4<f32>,
    v0: vec4<f32>, v1: vec4<f32>, v2: vec4<f32>, v3: vec4<f32>, v8: vec4<f32>, v9: vec4<f32>, v10: vec4<f32>,
    v12: vec4<f32>, v13: vec4<f32>, v14: vec4<f32>, v26: vec4<f32>, v28: vec4<f32>, v36: vec4<f32>, v37: vec4<f32>,
    v38: vec4<f32>, v39: vec4<f32>, v53: vec4<f32>, v54: vec4<f32>, v55: vec4<f32>, v58: vec4<f32>, v117: vec4<f32>,
    v118: vec4<f32>, v119: vec4<f32>, v122: vec4<f32>, v123: vec4<f32>, v124: vec4<f32>, v125: vec4<f32>,
    v126: vec4<f32>, v127: vec4<f32>, v128: vec4<f32>, v129: vec4<f32>, v130: vec4<f32>, v137: vec4<f32>,
    v141: vec4<f32>, v142: vec4<f32>, v151: vec4<f32>, v160: vec4<f32>, v161: vec4<f32>, v172: vec4<f32>,
    p0: vec4<f32>, p1: vec4<f32>, p2: vec4<f32>, p3: vec4<f32>, p4: vec4<f32>, p5: vec4<f32>, p6: vec4<f32>,
    p7: vec4<f32>, p8: vec4<f32>, p9: vec4<f32>, p10: vec4<f32>, p11: vec4<f32>, p15: vec4<f32>, p16: vec4<f32>,
    p17: vec4<f32>, p18: vec4<f32>,
    l4: vec4<f32>, l5: vec4<f32>, l8: vec4<f32>, l9: vec4<f32>, l10: vec4<f32>, l11: vec4<f32>, l12: vec4<f32>,
    l13: vec4<f32>, l14: vec4<f32>, l15: vec4<f32>,
}

struct KhOut {
    o0: vec4<f32>, o1: vec4<f32>, o2: vec4<f32>, o3: vec4<f32>, o4: vec4<f32>, o5: vec4<f32>, o6: vec4<f32>,
}

// A quotient that is 0 where the divisor is 0: Direct3D drops the NaN in the next min / max / saturate.
fn kh_div(a: f32, b: f32) -> f32 { return select(a / b, 0.0, b == 0.0); }

@@OWNERS@@

fn to_game(v: vec3<f32>) -> vec3<f32> { return vec3<f32>(v.x, v.z, v.y); }

// TDeferredLightPS for a directional light: stock UE 4.17.
fn kh_light(g: KhOut, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    let pi = 3.141593;
    let metallic = g.o2.x;
    let specular = g.o2.y;
    let roughness = max(g.o2.z, 0.08);
    let base = g.o3.xyz;
    let diffuse_color = base - base * metallic;
    let specular_color = mix(vec3<f32>(0.08 * specular), base, metallic);
    let n_l0 = dot(n, l);
    let n_v0 = dot(n, v);
    let v_l = dot(l, v);
    let inv_len_h = inverseSqrt(2.0 + 2.0 * v_l);
    let n_h = saturate((n_l0 + n_v0) * inv_len_h);
    let v_h = saturate(inv_len_h + inv_len_h * v_l);
    let n_l = saturate(n_l0);
    let n_v = min(abs(n_v0) + 0.00001, 1.0);
    let a = roughness * roughness;
    let vis_v = n_l * (n_v * (1.0 - a) + a);
    let vis_l = n_v * (n_l * (1.0 - a) + a);
    let vis = 0.5 / (vis_v + vis_l);
    let a2 = a * a;
    let d0 = (n_h * a2 - n_h) * n_h + 1.0;
    let d = a2 / (pi * d0 * d0);
    let fc0 = 1.0 - v_h;
    let fc = fc0 * fc0 * fc0 * fc0 * fc0;
    let f = saturate(50.0 * specular_color.y) * fc + (1.0 - fc) * specular_color;
    var result = (diffuse_color * 0.318310 + d * vis * f) * n_l;
#ifdef KH_SUBSURFACE
    let subsurface_color = g.o4.xyz * g.o4.xyz;
    let opacity = g.o4.w;
    let h = normalize(v + l);
    let in_scatter = pow(saturate(dot(l, -v)), 12.0) * (opacity * -2.9 + 3.0);
    let opacity_factor = saturate(dot(n, h) * opacity + 1.0 - opacity);
    let back_scatter = opacity_factor * 0.159155;
    result += subsurface_color * (in_scatter * (1.0 - back_scatter) + back_scatter);
#endif
    return result;
}

// EnvBRDFApprox with the ambient light for a reflection: no capture of a KH2 room exists.
fn kh_reflection(g: KhOut, n: vec3<f32>, v: vec3<f32>, env: vec3<f32>) -> vec3<f32> {
    let specular_color = mix(vec3<f32>(0.08 * g.o2.y), g.o3.xyz, g.o2.x);
    let n_v = saturate(dot(n, v));
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = g.o2.z * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * n_v)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return env * (specular_color * ab.x + vec3<f32>(saturate(50.0 * specular_color.y) * ab.y));
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let k = kh.info.x;
    let camera = to_game(view.world_position) * k;
    let room = in.world_position.xyz;
    let world = to_game(room) * k;
    var i: KhIn;
    let n = normalize(to_game(in.world_normal));
#ifdef VERTEX_TANGENTS
    let t = normalize(to_game(in.world_tangent.xyz));
    let sign = in.world_tangent.w;
#else
    let t = vec3<f32>(1.0, 0.0, 0.0);
    let sign = 1.0;
#endif
    i.svpos = vec4<f32>(in.position.xy, in.position.z, 1.0);
    i.front = vec4<f32>(select(0.0, 1.0, is_front));
    i.tc10 = vec4<f32>(t, 0.0);
    i.tc11 = vec4<f32>(n, sign);
#ifdef VERTEX_UVS_A
    i.tc0 = vec4<f32>(in.uv, 0.0, 0.0);
#endif
#ifdef VERTEX_UVS_B
    i.tc1 = vec4<f32>(in.uv_b, 0.0, 0.0);
    i.tc2 = i.tc1;
    i.tc3 = i.tc1;
#endif
    i.tc7 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    let twp = world - camera;
    i.tc9 = vec4<f32>(twp, 1.0);
    i.col0 = vec4<f32>(1.0);
    let right = to_game(view.world_from_view[0].xyz);
    let up = to_game(view.world_from_view[1].xyz);
    let forward = -to_game(view.world_from_view[2].xyz);
    i.v8 = vec4<f32>(right.x, up.x, forward.x, 0.0);
    i.v9 = vec4<f32>(right.y, up.y, forward.y, 0.0);
    i.v10 = vec4<f32>(right.z, up.z, forward.z, 0.0);
    i.v12 = vec4<f32>(right, 0.0);
    i.v13 = vec4<f32>(up, 0.0);
    i.v14 = vec4<f32>(forward, 0.0);
    // SVPositionToTranslatedWorld: pixel to clip, the view's inverse, room to game, the camera's translation.
    let vp = view.viewport;
    let pixel = mat4x4<f32>(
        vec4<f32>(2.0 / vp.z, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, -2.0 / vp.w, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(-1.0 - 2.0 * vp.x / vp.z, 1.0 + 2.0 * vp.y / vp.w, 0.0, 1.0),
    );
    let game = mat4x4<f32>(
        vec4<f32>(k, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, k, 0.0),
        vec4<f32>(0.0, k, 0.0, 0.0),
        vec4<f32>(-camera, 1.0),
    );
    let to_translated = game * view.world_from_clip * pixel;
    i.v36 = to_translated[0];
    i.v37 = to_translated[1];
    i.v38 = to_translated[2];
    i.v39 = to_translated[3];
    i.v28 = vec4<f32>(view.clip_from_view[0][0], 0.0, 0.0, 0.0);
    i.v53 = vec4<f32>(0.0, 0.0, 1.0, 0.0);
    i.v54 = vec4<f32>(0.5, -0.5, 0.5, 0.5);
    i.v55 = vec4<f32>(camera, 0.0);
    i.v58 = vec4<f32>(-camera, 0.0);
    i.v118 = vec4<f32>(vp.zw, 1.0 / vp.zw);
    i.v119 = i.v118;
    i.v122 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.v123 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.v124 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.v125 = vec4<f32>(0.0, 1.0, 0.0, 0.0);
    i.v127 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.v128 = vec4<f32>(kh.info.y);
    i.v142 = vec4<f32>(1.0);
    i.v160 = vec4<f32>(1.0);
    i.v161 = vec4<f32>(1.0, 1.0, kh.info.y, 0.0);
    i.p4 = kh.w2l[0];
    i.p5 = kh.w2l[1];
    i.p6 = kh.w2l[2];
    i.p7 = kh.w2l[3];
    i.p8 = kh.obj;
    i.p9 = vec4<f32>(kh.bounds.xyz, 1.0);
    i.p10 = kh.actor;
    i.p15 = kh.bmin;
    i.p16 = kh.bmax;
    // The indirect lighting cache's first band: an even ambient light, as constant radiance (sqrt(4 pi) x it). [duel
    // choice: KH2's rooms have no KH3 lighting; KH3's characters are lit by boards that follow the camera, `KHS`.]
    let ambient = vec3<f32>(0.42, 0.42, 0.45);
    i.l5 = vec4<f32>(1.0);
    i.l8 = vec4<f32>(ambient.r * 3.544908, 0.0, 0.0, 0.0);
    i.l9 = vec4<f32>(ambient.g * 3.544908, 0.0, 0.0, 0.0);
    i.l10 = vec4<f32>(ambient.b * 3.544908, 0.0, 0.0, 0.0);
#ifdef KH_MASKED
    // The mask is a dither the game's temporal anti-aliasing averages: its average at four offsets decides.
    var cover = 0u;
    for (var q = 0u; q < 4u; q++) {
        i.v137 = vec4<f32>(f32(q));
        if (kh_mask(i) != 0.0) {
            cover += 1u;
        }
    }
    if (cover < 2u) {
        discard;
    }
    i.v137 = vec4<f32>(0.0);
#endif
    var g = kh_material(i);
    // The G-buffer's targets are UNORM: what the shader writes past 1 is clamped there.
    g.o1 = saturate(g.o1);
    g.o2 = saturate(g.o2);
    g.o3 = saturate(g.o3);
    g.o4 = saturate(g.o4);
    var out: FragmentOutput;
#ifdef KH_TRANSLUCENT
    var color = vec4<f32>(max(g.o0.xyz, vec3<f32>(0.0)), saturate(g.o0.w));
#else
    let normal = normalize(g.o1.xyz * 2.0 - vec3<f32>(1.0));
    let to_camera = normalize(-twp);
    var lit = g.o0.xyz;
    // A key light above and to the side of the camera [duel choice]: pi = a white surface facing it shows 1.
    let dir = normalize(right * 0.35 + up * 0.8 - forward * 0.5);
    lit += kh_light(g, normal, to_camera, dir) * vec3<f32>(2.6, 2.55, 2.45);
    lit += kh_reflection(g, normal, to_camera, ambient * 1.6);
    var color = vec4<f32>(max(lit, vec3<f32>(0.0)), 0.0);
#endif
#ifdef TONEMAP_IN_SHADER
    color = tone_mapping(color, view.color_grading);
#endif
    out.color = color;
    return out;
}
"#;

/// The owners' functions under their shader definitions.
fn owners_wgsl(owners: &[Owner]) -> String {
    let mut out = String::new();
    for (k, o) in owners.iter().enumerate() {
        out.push_str(&format!("#ifdef KH_C{k}\n// {}\n", o.asset));
        out.push_str(&format!(
            "fn kh_material(i: KhIn) -> KhOut {{\n{}}}\n",
            o.base
        ));
        if let Some(mask) = &o.mask {
            out.push_str(&format!(
                "fn kh_mask(i: KhIn) -> f32 {{\n{}}}\n",
                charmat::mask_body(mask)
            ));
        }
        out.push_str("#endif\n");
    }
    out
}

/// The whole shader's text.
pub(crate) fn shader_text(m: &CharMaterials) -> String {
    HARNESS.replace("@@OWNERS@@", &owners_wgsl(&m.owners))
}

fn rows(owner: &Owner, slot: &Slot) -> Params {
    let mut p = Params::default();
    let mut table = vec![[0.; 4]; ROWS];
    charmat::rows_of(owner, slot, 0., &mut table);
    for (out, row) in p.m.iter_mut().zip(&table) {
        *out = Vec4::from_array(*row);
    }
    for (out, row) in
        p.c0.iter_mut()
            .zip(owner.collections.first().into_iter().flatten())
    {
        *out = Vec4::from_array(*row);
    }
    p
}

/// A slot's material; `None` when it was not translated or uses more than the harness has.
pub(crate) fn material_of(
    m: &CharMaterials,
    slot: &Slot,
    image: &mut dyn FnMut(&str) -> Option<Handle<Image>>,
    samplers: &(Handle<Image>, Handle<Image>),
) -> Option<Kh3Material> {
    let at = slot.owner?;
    let owner = m.owners.get(at)?;
    if slot.textures.len() > TEXTURES {
        return None;
    }
    let mut t: [Option<Handle<Image>>; TEXTURES] = Default::default();
    for (out, path) in t.iter_mut().zip(&slot.textures) {
        *out = path.as_deref().and_then(&mut *image);
    }
    let [t0, t1, t2, t3, t4, t5, t6, t7, t8, t9, t10, t11, t12, t13] = t;
    Some(Kh3Material {
        params: rows(owner, slot),
        t0,
        t1,
        t2,
        t3,
        t4,
        t5,
        t6,
        t7,
        t8,
        t9,
        t10,
        t11,
        t12,
        t13,
        wrap: Some(samplers.0.clone()),
        clamp: Some(samplers.1.clone()),
        key: Key {
            owner: at as u16,
            masked: owner.mask.is_some(),
            translucent: owner.blend == "BLEND_Translucent",
            subsurface: owner.subsurface,
            two_sided: owner.two_sided,
        },
    })
}

impl Kh3Material {
    /// This frame's actor (KH3's world) and scale.
    pub(crate) fn place(&mut self, at: glam::Vec3, yaw: f32, cm_per_unit: f32, time: f32) {
        let (s, c) = (yaw.sin(), yaw.cos());
        // World to the actor's frame: its turn undone, then its place.
        let p = &mut self.params;
        p.w2l = [
            Vec4::new(c, -s, 0., 0.),
            Vec4::new(s, c, 0., 0.),
            Vec4::new(0., 0., 1., 0.),
            Vec4::new(-(at.x * c + at.y * s), at.x * s - at.y * c, -at.z, 1.),
        ];
        p.obj = Vec4::new(at.x, at.y, at.z + 90., 100.);
        p.actor = Vec4::new(at.x, at.y, at.z, 0.);
        p.info = Vec4::new(cm_per_unit, time, 0., 0.);
    }
}
