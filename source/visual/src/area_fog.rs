//! The exe's per-area fog (draw-01 §4.5, §8.1) as a `StandardMaterial`
//! extension: `f = clamp((far - w) / (far - near), min/255, max/255)` with
//! `w` the view-space depth, `rgb = c * f + fog * (1 - f)` blended in the
//! exe's gamma space. Bevy's own distance fog has no lower bound on `f`,
//! so the cap (tt08 keeps at most 25.5% fog) needs this shader.
use bevy::{
    asset::{AssetPath, embedded_asset, embedded_path},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{
        ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
        MaterialPlugin,
    },
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};
use std::sync::Mutex;

/// A type-14 environment record's fog fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FogParams {
    pub colour: [u8; 3],
    pub near: f32,
    pub far: f32,
    /// 0..=255 byte scale (FogParam z/w times 255).
    pub min: f32,
    pub max: f32,
}

impl FogParams {
    /// FUN_14016f830: fog is off iff min == 0, max == 255 and the colour is
    /// the neutral grey 0x808080. A record without fog uses
    /// near 1000, far 1,000,000, min 0, max 255 (FUN_14016f690).
    pub fn enabled(&self) -> bool {
        !(self.min == 0. && self.max == 255. && self.colour == [0x80; 3])
    }
}

/// Fog applied to materials created from now on; an area load sets it
/// before its models spawn (every draw of an area shares one record).
static CURRENT: Mutex<Option<FogParams>> = Mutex::new(None);

pub fn set_current(params: Option<FogParams>) {
    *CURRENT.lock().unwrap() = params.filter(FogParams::enabled);
}

fn current() -> Option<FogParams> {
    *CURRENT.lock().unwrap()
}

/// The area's fog colour scaled by a scene tint (0x80 = 1.0) as
/// FUN_1401626f0 rescales the current fog from the area's record, as the
/// shader's colour; `None` without fog.
pub fn tinted_colour(tint: [u8; 3]) -> Option<Vec4> {
    let p = current()?;
    let c = |k: usize| ((u32::from(p.colour[k]) * u32::from(tint[k])) >> 7).min(255) as f32 / 255.;
    Some(Vec4::new(c(0), c(1), c(2), 1.))
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct FogExt {
    /// (-1/(far-near), far/(far-near), min/255, max/255); enabled flag in w
    /// of `colour`.
    #[uniform(100)]
    pub param: Vec4,
    #[uniform(100)]
    pub colour: Vec4,
}

impl MaterialExtension for FogExt {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(
            AssetPath::from_path_buf(embedded_path!("fog.wgsl")).with_source("embedded"),
        )
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    /// On a glow camera only map and sky opaque chunks (their `UV_1` class)
    /// write the alpha channel, the exe's stencil.
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if crate::glow::keeps_alpha(key.mesh_key) && !layout.0.contains(Mesh::ATTRIBUTE_UV_1) {
            crate::glow::keep_alpha(descriptor);
        }
        Ok(())
    }
}

pub type FogMaterial = ExtendedMaterial<StandardMaterial, FogExt>;

impl FogExt {
    pub fn from_params(params: Option<FogParams>) -> Self {
        match params {
            Some(p) => {
                let span = (p.far - p.near).max(1e-3);
                FogExt {
                    param: Vec4::new(-1. / span, p.far / span, p.min / 255., p.max / 255.),
                    colour: Vec4::new(
                        p.colour[0] as f32 / 255.,
                        p.colour[1] as f32 / 255.,
                        p.colour[2] as f32 / 255.,
                        1.,
                    ),
                }
            }
            None => FogExt::default(),
        }
    }
}

pub fn material(base: StandardMaterial, fogged: bool) -> FogMaterial {
    ExtendedMaterial {
        base,
        extension: FogExt::from_params(if fogged { current() } else { None }),
    }
}

/// `Assets<FogMaterial>::add` for a plain base material, fogged with the
/// area's current record.
pub trait AddFogged {
    fn add_fogged(&mut self, base: StandardMaterial) -> Handle<FogMaterial>;
    /// Unfogged (sky layer 0 and screen-space overlays).
    fn add_plain(&mut self, base: StandardMaterial) -> Handle<FogMaterial>;
}

impl AddFogged for Assets<FogMaterial> {
    fn add_fogged(&mut self, base: StandardMaterial) -> Handle<FogMaterial> {
        self.add(material(base, true))
    }

    fn add_plain(&mut self, base: StandardMaterial) -> Handle<FogMaterial> {
        self.add(material(base, false))
    }
}

pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "fog.wgsl");
        app.add_plugins(MaterialPlugin::<FogMaterial>::default());
    }
}
