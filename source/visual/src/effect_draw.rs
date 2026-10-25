//! What the PC binds when it draws an effect's quads and models: the
//! texture of a TEX0, the GS blend of an ALPHA byte, and the ALPHA a model
//! draw uses. Shared by the effect presenters (spells, Drive and system
//! effects, the world map and cutscenes).
//!
//! The PC port does not emulate GS memory for effects. FUN_1404aecd0 makes
//! one texture per DPD texture (its remastered page when the file has one,
//! [`data::object_effect::Dpd::attach_hd`]), and FUN_1402c2ae0 registers,
//! for each effect it draws, the textures its PDATA header (+0xb0) marks,
//! by buffer pointer (FUN_14012c620, later ones replacing earlier ones at
//! the same pointer, the palette pointer left 0). A draw's TEX0 then finds
//! its texture by buffer pointer alone (FUN_14012c0f0): the CLUT pointer is
//! not part of the match. The resident `effect/texcommon.dpd` textures are
//! registered the same way, all of them (FUN_1402c2640).
//!
//! The PC draws an effect with pixel shader 026: texture × vertex colour
//! (the PS2 bytes, 0x80 = 1.0), the GS alpha test, and a D3D blend into a
//! non-sRGB back buffer, so every blend runs on the stored (gamma) values.
//! [`EffectMaterial`] does the same on a camera that composites in sRGB
//! space ([`scene_compositing`]); in linear light the faint edges of
//! additive sprites brighten and overlapping puffs flatten into sheets.
use bevy::{
    asset::{AssetPath, RenderAssetUsages, embedded_asset, embedded_path},
    camera::CompositingSpace,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin, MeshPipelineKey},
    prelude::*,
    render::{
        render_resource::{
            AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, Extent3d,
            RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureDimension,
            TextureFormat,
        },
        view::ColorGrading,
    },
    shader::{Shader, ShaderRef},
};
use data::object_effect::{Dpd, DpdTexture};
use std::sync::Arc;

/// The texture a draw binds: texture `index` of the DPD at address `dpd`,
/// sampled over the TEX0 page of 2^tw × 2^th texels.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TexKey {
    pub dpd: usize,
    pub index: usize,
    pub tw: u8,
    pub th: u8,
}

impl TexKey {
    /// The texture a draw with `tex0` finds: the last texture of `own`
    /// that the effect's `mask` (PDATA +0xb0) registers at the TEX0's
    /// buffer pointer, else the last of `own` at that pointer, else the
    /// last common texture there.
    pub fn pick(
        own: &Arc<Dpd>,
        mask: &[u8; 16],
        common: Option<&Arc<Dpd>>,
        tex0: u64,
    ) -> Option<Self> {
        let tbp = (tex0 & 0x3fff) as u16;
        let marked = |i: usize| mask.get(i / 8).is_some_and(|b| b >> (i % 8) & 1 != 0);
        let at = |d: &Dpd| -> Vec<usize> {
            d.textures
                .iter()
                .enumerate()
                .filter(|(_, t)| t.dbp == tbp)
                .map(|(i, _)| i)
                .collect()
        };
        let own_at = at(own);
        let (dpd, index) = match own_at.iter().rev().find(|&&i| marked(i)).or(own_at.last()) {
            Some(&i) => (Arc::as_ptr(own) as usize, i),
            None => {
                let c = common?;
                (Arc::as_ptr(c) as usize, *at(c).last()?)
            }
        };
        Some(TexKey {
            dpd,
            index,
            tw: ((tex0 >> 26) & 0xf) as u8,
            th: ((tex0 >> 30) & 0xf) as u8,
        })
    }

    /// The page's size in PS2 texels.
    pub fn size(&self) -> [u32; 2] {
        [1 << self.tw.min(10), 1 << self.th.min(10)]
    }
}

/// The page image of `key` from its DPD: the remastered page when there is
/// one, else the PS2 texture alone at its place in its page, through its
/// own palette.
pub fn page_image(dpd: &Dpd, key: TexKey, repeat: bool) -> Image {
    let [w, h] = key.size();
    let (size, pixels) = match dpd.textures.get(key.index) {
        Some(DpdTexture { hd: Some(page), .. }) => ([page.width, page.height], page.rgba.clone()),
        Some(t) => ([w, h], ps2_page(t, [w, h])),
        None => ([w, h], vec![0; (w * h * 4) as usize]),
    };
    let mut image = Image::new(
        Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        // The stored values, as the PC samples them (no sRGB decode).
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    let mode = if repeat {
        ImageAddressMode::Repeat
    } else {
        ImageAddressMode::ClampToEdge
    };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: mode,
        address_mode_v: mode,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// A 1×1 clear page, for a draw whose page is missing (not the white
/// default image).
pub fn transparent_image() -> Image {
    Image::new(
        Extent3d::default(),
        TextureDimension::D2,
        vec![0; 4],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

/// A PS2 texture placed at its buffer position in a `size` page.
fn ps2_page(t: &DpdTexture, size: [u32; 2]) -> Vec<u8> {
    let [w, h] = size;
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    for row in 0..t.height {
        let y = t.y as u32 + row;
        if y >= h {
            break;
        }
        for col in 0..t.width {
            let x = t.x as u32 + col;
            if x >= w {
                break;
            }
            let s = ((row * t.width + col) * 4) as usize;
            let d = ((y * w + x) * 4) as usize;
            pixels[d..d + 4].copy_from_slice(&t.rgba[s..s + 4]);
        }
    }
    pixels
}

/// The PC's blend for a GS ALPHA byte ((A − B)·C + D, FUN_140472110 →
/// a D3D blend state): which colours A, B and D pick decides the state.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Blend {
    /// (Cs − Cd)·As + Cd (0x44).
    Alpha,
    /// Cs·As + Cd (0x48).
    Add,
    /// Cd − Cs·As (0x42, D3D reverse subtract).
    Subtract,
    /// Cs·As, the frame buffer's colour dropped (0x88).
    Replace,
}

impl Blend {
    pub fn of(alpha: u8) -> Self {
        let (a, b, d) = (alpha & 3, alpha >> 2 & 3, alpha >> 6 & 3);
        match (a, b, d) {
            (0, 2, 1) => Blend::Add,
            (2, 0, 1) => Blend::Subtract,
            (_, _, 2) => Blend::Replace,
            _ => Blend::Alpha,
        }
    }

    /// The Bevy pipeline that carries the blend: straight alpha, the
    /// premultiplied pipeline for add (alpha 0) and replace (alpha 1), and
    /// the multiply pipeline, whose blend [`EffectMaterial`] turns into a
    /// reverse subtract.
    fn alpha_mode(self) -> AlphaMode {
        match self {
            Blend::Alpha => AlphaMode::Blend,
            Blend::Add => AlphaMode::Add,
            Blend::Replace => AlphaMode::Premultiplied,
            Blend::Subtract => AlphaMode::Multiply,
        }
    }

    /// The shader's output mode (effect.wgsl).
    fn mode(self) -> f32 {
        match self {
            Blend::Alpha => 0.,
            Blend::Add => 1.,
            Blend::Replace => 2.,
            Blend::Subtract => 3.,
        }
    }
}

/// An effect draw as pixel shader 026 shades it: page texel × vertex colour
/// (both stored values, the colour 0x80 = 1.0), alpha test "alpha > 0" (the
/// effects' default GS TEST 0x7000d, FUN_1404af5e0 → FUN_1404724e0), then
/// the blend of the draw's ALPHA byte. Untextured draws take the vertex
/// colour alone.
#[derive(Asset, AsBindGroup, TypePath, Clone, Debug)]
pub struct EffectMaterial {
    /// x: blend mode (0 alpha, 1 add, 2 replace, 3 subtract), y: 1 when
    /// textured, z: 1 when the camera composites in sRGB space (else the
    /// output is taken to linear light), w unused.
    #[uniform(0)]
    pub params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
    pub blend: Blend,
    /// The draw layer's depth bias.
    pub bias: f32,
}

impl EffectMaterial {
    pub fn new(blend: Blend, texture: Option<Handle<Image>>, bias: f32, srgb: bool) -> Self {
        EffectMaterial {
            params: Vec4::new(
                blend.mode(),
                f32::from(u8::from(texture.is_some())),
                f32::from(u8::from(srgb)),
                0.,
            ),
            texture,
            blend,
            bias,
        }
    }
}

impl Material for EffectMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(
            AssetPath::from_path_buf(embedded_path!("effect.wgsl")).with_source("embedded"),
        )
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.blend.alpha_mode()
    }

    fn depth_bias(&self) -> f32 {
        self.bias
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
        descriptor.primitive.cull_mode = None;
        let mesh_key = key.mesh_key;
        let multiply =
            mesh_key & MeshPipelineKey::BLEND_RESERVED_BITS == MeshPipelineKey::BLEND_MULTIPLY;
        if multiply && let Some(fragment) = descriptor.fragment.as_mut() {
            // Cd − Cs·As (FUN_140472110, D3D BLEND_OP_REV_SUBTRACT); the
            // shader hands Cs·As.
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState {
                    color: BlendComponent {
                        src_factor: BlendFactor::One,
                        dst_factor: BlendFactor::One,
                        operation: BlendOperation::ReverseSubtract,
                    },
                    alpha: BlendComponent {
                        src_factor: BlendFactor::Zero,
                        dst_factor: BlendFactor::One,
                        operation: BlendOperation::Add,
                    },
                });
            }
        }
        Ok(())
    }
}

/// The scene camera's compositing as the PC's: blends on the stored sRGB
/// values (`CompositingSpace::Srgb`, an `Rgba8Unorm` main texture), with
/// the PBR materials' linear output encoded to sRGB in their tone mapping
/// (marked by the grading, see [`EffectPlugin`]).
pub fn scene_compositing() -> (CompositingSpace, ColorGrading) {
    let mut grading = ColorGrading::default();
    for section in [
        &mut grading.shadows,
        &mut grading.midtones,
        &mut grading.highlights,
    ] {
        section.gamma = SRGB_MARK;
    }
    (CompositingSpace::Srgb, grading)
}

/// The colour grading gamma (every section) that marks the sRGB encode.
const SRGB_MARK: f32 = 2.2;

const TONEMAPPING_SHARED: &str =
    "embedded://bevy_core_pipeline/tonemapping/tonemapping_shared.wgsl";
/// Where `tone_mapping` starts grading, after the hue and white balance.
const GRADING_ANCHOR: &str =
    "    // Perform the \"sectional\" color grading: i.e. the color grading that";
const SRGB_ENCODE: &str = "    // KH2RW: the sRGB-compositing scene camera (every section's gamma 2.2):
    // the exact sRGB encode of the linear output, and nothing else.
    if all(color_grading.gamma == vec3(2.2)) {
        return vec4(select(1.055 * pow(color, vec3(1.0 / 2.4)) - 0.055, color * 12.92, color <= vec3(0.0031308)), in.a);
    }
";

/// The effect material and the scene's sRGB encode. Bevy's PBR materials
/// tone-map in their shader (`tone_mapping`, with no colour grading for an
/// LDR camera) and then write linear values; once that shader library
/// loads it gains the marked encode, so the opaque scene keeps its look on
/// the sRGB-compositing camera and every blend runs on sRGB values.
pub struct EffectPlugin;

impl Plugin for EffectPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "effect.wgsl");
        app.add_plugins(MaterialPlugin::<EffectMaterial>::default())
            .add_systems(Update, patch_grading);
    }
}

fn patch_grading(
    mut done: Local<bool>,
    mut handle: Local<Option<Handle<Shader>>>,
    server: Res<AssetServer>,
    mut shaders: ResMut<Assets<Shader>>,
) {
    if *done {
        return;
    }
    let h = handle.get_or_insert_with(|| server.load(TONEMAPPING_SHARED));
    if server.load_state(&*h).is_failed() {
        *done = true;
        error!("effect_draw: {TONEMAPPING_SHARED} not found; the 3D scene is not sRGB-encoded");
        return;
    }
    let Some(shader) = shaders.get(&*h) else {
        return;
    };
    *done = true;
    let source = shader.source.as_str();
    if source.matches(GRADING_ANCHOR).count() != 1 {
        error!("effect_draw: Bevy's tone mapping changed; the 3D scene is not sRGB-encoded");
        return;
    }
    let patched = Shader::from_wgsl(
        source.replace(GRADING_ANCHOR, &format!("{SRGB_ENCODE}{GRADING_ANCHOR}")),
        shader.path.clone(),
    );
    info!("effect_draw: the scene's tone mapping encodes to sRGB");
    if let Some(mut s) = shaders.get_mut(&*h) {
        *s = patched;
    }
}

/// A draw whose texture the effect does not have: logged once per buffer
/// pointer and left undrawn (never a white quad).
pub fn report_missing(seen: &mut Vec<(usize, u16)>, dpd: usize, tex0: u64, what: &str) {
    let tbp = (tex0 & 0x3fff) as u16;
    if !seen.contains(&(dpd, tbp)) {
        seen.push((dpd, tbp));
        error!(
            "effect texture missing: {what} TEX0 {tex0:#x} (buffer {tbp:#x}); the draw is left out"
        );
    }
}

/// The ALPHA byte a model draw uses (FUN_1404b6860): the draw's own mode
/// word when it is not zero, else the model header's (+0x16).
pub fn model_alpha(header: u8, mode: Option<u16>) -> u8 {
    match mode {
        Some(m) if m != 0 => m as u8,
        _ => header,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::object_effect::Dpd;
    use std::collections::HashMap;

    fn dpd(buffers: &[u16]) -> Arc<Dpd> {
        let texture = |dbp: u16| DpdTexture {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
            indices: vec![0],
            palette: vec![0; 1024],
            dbp,
            cbp: 0,
            dbw: 1,
            x: 0,
            y: 0,
            tex0: 0,
            data_offset: 0,
            hd: None,
        };
        Arc::new(Dpd {
            bytes: Vec::new(),
            pdata: Vec::new(),
            shapes: HashMap::new(),
            model_vertices: HashMap::new(),
            models: HashMap::new(),
            vsf: HashMap::new(),
            textures: buffers.iter().map(|&b| texture(b)).collect(),
        })
    }

    #[test]
    fn a_draw_binds_the_last_registered_texture_at_its_buffer() {
        let own = dpd(&[0x3000, 0x3020, 0x3000, 0x3000]);
        let common = dpd(&[0x2a00, 0x2a00]);
        let tex0 = |tbp: u64| tbp | 7 << 26 | 6 << 30;
        // Textures 0 and 2 registered: 2 replaced 0.
        let mut mask = [0u8; 16];
        mask[0] = 0b0101;
        let k = TexKey::pick(&own, &mask, Some(&common), tex0(0x3000)).unwrap();
        assert_eq!((k.index, k.size()), (2, [128, 64]));
        // Nothing registered there: the last at that buffer.
        let k = TexKey::pick(&own, &[0; 16], Some(&common), tex0(0x3000)).unwrap();
        assert_eq!(k.index, 3);
        // A common buffer: the common DPD's last texture there.
        let k = TexKey::pick(&own, &mask, Some(&common), tex0(0x2a00)).unwrap();
        assert_eq!((k.dpd, k.index), (Arc::as_ptr(&common) as usize, 1));
        assert!(TexKey::pick(&own, &mask, None, tex0(0x2c00)).is_none());
    }

    #[test]
    fn blend_bytes_map_to_their_d3d_states() {
        assert_eq!(Blend::of(0x48), Blend::Add);
        assert_eq!(Blend::of(0x44), Blend::Alpha);
        assert_eq!(Blend::of(0x42), Blend::Subtract);
        assert_eq!(Blend::of(0x88), Blend::Replace);
        assert_eq!(model_alpha(0x48, Some(0x42)), 0x42);
        assert_eq!(model_alpha(0x44, Some(0)), 0x44);
        assert_eq!(model_alpha(0x44, None), 0x44);
    }

    #[test]
    fn every_blend_has_a_pipeline_and_a_shader_mode() {
        // Straight alpha; add and replace on the premultiplied pipeline
        // (alpha 0 / 1 from the shader); subtract on the multiply pipeline
        // that `specialize` turns into a reverse subtract.
        let cases = [
            (Blend::Alpha, AlphaMode::Blend, 0.),
            (Blend::Add, AlphaMode::Add, 1.),
            (Blend::Replace, AlphaMode::Premultiplied, 2.),
            (Blend::Subtract, AlphaMode::Multiply, 3.),
        ];
        for (blend, mode, shader) in cases {
            let m = EffectMaterial::new(blend, None, 500., true);
            assert_eq!(m.alpha_mode(), mode);
            assert_eq!(m.params, Vec4::new(shader, 0., 1., 0.));
            assert_eq!(m.depth_bias(), 500.);
        }
        let textured = EffectMaterial::new(Blend::Add, Some(Handle::default()), 0., false);
        assert_eq!((textured.params.y, textured.params.z), (1., 0.));
    }

    #[test]
    fn the_scene_camera_composites_in_srgb_with_the_marked_grading() {
        let (space, grading) = scene_compositing();
        assert_eq!(space, CompositingSpace::Srgb);
        for section in [grading.shadows, grading.midtones, grading.highlights] {
            assert_eq!(section.gamma, SRGB_MARK);
        }
        // The shader's test reads the same constant.
        assert!(SRGB_ENCODE.contains(&format!("gamma == vec3({SRGB_MARK:?})")));
    }
}
