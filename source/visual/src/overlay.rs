//! GPU drawing of menu 2D layouts (title, camp menu) in the original's
//! layout space: 512x416 PS2 units widened to 682.67x416 for 16:9 with no X
//! origin (title-01 §3.1), so x runs from -85.33 to 597.33.
//!
//! Colour math follows the exe's sprite shaders: texture x vertex colour
//! (vertex bytes / 128), clamped, blended in gamma space (the camera uses
//! `CompositingSpace::Srgb`). SEQD blend 0 is alpha, 1 additive, 2
//! subtractive (destination - source x alpha), as in the repo's gauge
//! canvas.
use bevy::{
    asset::{AssetEventSystems, AssetPath, RenderAssetUsages, embedded_asset, embedded_path},
    camera::{CameraOutputMode, CompositingSpace, ScalingMode, visibility::RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology},
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, Extent3d,
        RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureDescriptor,
        TextureDimension, TextureFormat,
    },
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPlugin},
};
use data::sprite_sequence::Quad;
use std::collections::HashMap;

/// Visible layout width at 16:9 (512 x 4/3).
pub const WIDE_WIDTH: f32 = 512. * 4. / 3.;
pub const HEIGHT: f32 = 416.;
/// Layout x of the visible area's centre.
const CENTRE_X: f32 = 256.;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlendKey {
    blend: u8,
    srgb_source: bool,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(BlendKey)]
pub struct LayoutMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,
    pub blend: u8,
    /// The texture is sRGB (a render target): re-encode to gamma.
    pub srgb_source: bool,
}

impl From<&LayoutMaterial> for BlendKey {
    fn from(m: &LayoutMaterial) -> Self {
        BlendKey {
            blend: m.blend,
            srgb_source: m.srgb_source,
        }
    }
}

impl Material2d for LayoutMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(
            AssetPath::from_path_buf(embedded_path!("overlay.wgsl")).with_source("embedded"),
        )
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let color = match key.bind_group_data.blend {
            1 => BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
            2 => BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::ReverseSubtract,
            },
            _ => BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
        };
        // Coverage for overlay layers (premultiplied "over"); additive and
        // subtractive draws leave it unchanged.
        let alpha = if key.bind_group_data.blend == 0 {
            BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            }
        } else {
            BlendComponent {
                src_factor: BlendFactor::Zero,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            }
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            if key.bind_group_data.srgb_source {
                fragment.shader_defs.push("SRGB_SOURCE".into());
            }
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState { color, alpha });
            }
        }
        Ok(())
    }
}

/// `order` places the 2D camera among the app's cameras; an overlay keeps
/// what earlier cameras drew instead of clearing.
pub struct Ui2dPlugin {
    pub order: isize,
    pub overlay: bool,
}

#[derive(Resource, Clone, Copy)]
struct CameraSettings {
    order: isize,
    overlay: bool,
}

impl Plugin for Ui2dPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "overlay.wgsl");
        app.add_plugins(Material2dPlugin::<LayoutMaterial>::default())
            .init_resource::<Ui2dFrame>()
            .insert_resource(CameraSettings {
                order: self.order,
                overlay: self.overlay,
            })
            .add_systems(Startup, spawn_camera)
            .add_systems(
                PostUpdate,
                (present, rebind_resized.after(AssetEventSystems)),
            );
    }
}

/// A prepared material keeps the texture view it was built with, and an
/// image whose size or format changes gets a new GPU texture (Bevy only
/// reuses one with the same descriptor). So when a sheet's image is resized
/// (the scene target following the window: a resize, a minimize, alt-tab
/// out of full screen, the Resolution setting), the materials sampling it
/// are prepared again; otherwise they go on drawing the old texture's last
/// picture: a frozen scene under a live HUD.
fn rebind_resized(
    mut events: MessageReader<AssetEvent<Image>>,
    images: Res<Assets<Image>>,
    mut materials: ResMut<Assets<LayoutMaterial>>,
    mut known: Local<HashMap<AssetId<Image>, TextureDescriptor<'static>>>,
) {
    let mut changed = Vec::new();
    for event in events.read() {
        match *event {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                let Some(image) = images.get(id) else {
                    continue;
                };
                let descriptor = image.texture_descriptor.clone();
                if known
                    .insert(id, descriptor.clone())
                    .is_some_and(|d| d != descriptor)
                {
                    changed.push(id);
                }
            }
            AssetEvent::Unused { id } | AssetEvent::Removed { id } => {
                known.remove(&id);
            }
            AssetEvent::LoadedWithDependencies { .. } => {}
        }
    }
    if changed.is_empty() {
        return;
    }
    let stale: Vec<_> = materials
        .iter()
        .filter(|(_, m)| changed.contains(&m.texture.id()))
        .map(|(id, _)| id)
        .collect();
    for id in &stale {
        if let Some(m) = materials.get_mut(*id) {
            m.into_inner();
        }
    }
    info!(
        "2D layer: {} image(s) changed size, {} material(s) rebound",
        changed.len(),
        stale.len()
    );
}

/// A registered sheet: its GPU image, HD size and HD texels per sequence
/// texel.
#[derive(Clone)]
pub struct Sheet {
    pub image: Handle<Image>,
    pub size: [u32; 2],
    pub texel_scale: f32,
    pub srgb_source: bool,
}

/// The sheets and this frame's draw list, in draw order.
#[derive(Resource, Default)]
pub struct Ui2dFrame {
    pub sheets: Vec<Sheet>,
    pub draws: Vec<(usize, Vec<Quad>)>,
    materials: HashMap<(usize, u8), Handle<LayoutMaterial>>,
}

impl Ui2dFrame {
    /// Uploads straight RGBA as a gamma-space (non-sRGB) bilinear sheet.
    pub fn add_sheet(
        &mut self,
        images: &mut Assets<Image>,
        rgba: Vec<u8>,
        size: [u32; 2],
        texel_scale: f32,
    ) -> usize {
        let mut image = Image::new(
            Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            ..default()
        });
        self.sheets.push(Sheet {
            image: images.add(image),
            size,
            texel_scale,
            srgb_source: false,
        });
        self.sheets.len() - 1
    }

    /// An existing image (e.g. a render target) as a sheet; `srgb` when
    /// its format is sRGB, so samples are re-encoded to gamma.
    pub fn add_image(&mut self, image: Handle<Image>, size: [u32; 2], srgb: bool) -> usize {
        self.sheets.push(Sheet {
            image,
            size,
            texel_scale: 1.,
            srgb_source: srgb,
        });
        self.sheets.len() - 1
    }

    /// A whole-window quad of `sheet` (its full texel extent) tinted by
    /// `rgba`.
    pub fn full_screen(&mut self, sheet: usize, rgba: [f32; 4]) {
        let [w, h] = self.sheets[sheet].size;
        let (l, r) = (CENTRE_X - WIDE_WIDTH / 2., CENTRE_X + WIDE_WIDTH / 2.);
        self.draws.push((
            sheet,
            vec![Quad {
                corners: [[l, 0.], [r, 0.], [l, HEIGHT], [r, HEIGHT]],
                uv: [0, 0, w as i32, h as i32],
                colors: [rgba; 4],
                blend: 0,
            }],
        ));
    }

    /// Re-points a sheet slot at another upload (e.g. a device-glyph swap).
    pub fn rebind(&mut self, slot: usize, sheet: Sheet) {
        self.sheets[slot] = sheet;
        self.materials.retain(|(s, _), _| *s != slot);
    }

    /// A full-screen solid rectangle (screen fades); needs a white sheet.
    pub fn fill(&mut self, white: usize, rgba: [f32; 4]) {
        let (l, r) = (CENTRE_X - WIDE_WIDTH, CENTRE_X + WIDE_WIDTH);
        self.draws.push((
            white,
            vec![Quad {
                corners: [
                    [l, -HEIGHT],
                    [r, -HEIGHT],
                    [l, 2. * HEIGHT],
                    [r, 2. * HEIGHT],
                ],
                uv: [0, 0, 1, 1],
                colors: [rgba; 4],
                blend: 0,
            }],
        ));
    }
}

#[derive(Component)]
struct Slot;

/// The layer only the 2D layout camera renders, so other 2D cameras (the
/// test room's HUD) do not draw these meshes at their own scale.
const LAYER: usize = 7;

fn spawn_camera(mut commands: Commands, settings: Res<CameraSettings>) {
    commands.spawn((
        Camera2d,
        if settings.overlay {
            // A gamma-space layer cleared transparent, laid over the
            // earlier cameras as premultiplied colour [approximation: the
            // exe blends 2D straight onto the gamma framebuffer].
            Camera {
                order: settings.order,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                output_mode: CameraOutputMode::Write {
                    blend_state: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    clear_color: ClearColorConfig::None,
                },
                ..default()
            }
        } else {
            Camera {
                order: settings.order,
                ..default()
            }
        },
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        CompositingSpace::Srgb,
        RenderLayers::layer(LAYER),
        Projection::Orthographic(OrthographicProjection {
            // Layout units are not square: 682.67 x 416 fills the 16:9
            // window (title-01 §3.1).
            scaling_mode: ScalingMode::Fixed {
                width: WIDE_WIDTH,
                height: HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

fn quad_mesh(quads: &[&Quad], sheet: &Sheet) -> Mesh {
    let mut positions = Vec::with_capacity(quads.len() * 4);
    let mut uvs = Vec::with_capacity(quads.len() * 4);
    let mut colors = Vec::with_capacity(quads.len() * 4);
    let mut indices = Vec::with_capacity(quads.len() * 6);
    let [w, h] = sheet.size.map(|v| v as f32);
    for q in quads {
        let base = positions.len() as u32;
        let [u0, v0, u1, v1] = q.uv.map(|v| v as f32 * sheet.texel_scale);
        for (k, [x, y]) in q.corners.into_iter().enumerate() {
            positions.push([x - CENTRE_X, HEIGHT / 2. - y, 0.]);
            let (u, v) = (
                if k % 2 == 0 { u0 } else { u1 },
                if k < 2 { v0 } else { v1 },
            );
            uvs.push([u / w, v / h]);
            colors.push(q.colors[k]);
        }
        indices.extend([0, 1, 2, 1, 3, 2].map(|i| base + i));
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// What a slot's mesh was built from: the sheet (its image, size and texel
/// scale) and the quads.
#[derive(PartialEq)]
struct SlotSource {
    image: AssetId<Image>,
    size: [u32; 2],
    texel_scale: u32,
    quads: Vec<Quad>,
}

type SlotParts = (
    Entity,
    &'static Mesh2d,
    &'static mut MeshMaterial2d<LayoutMaterial>,
    &'static mut Transform,
    &'static mut Visibility,
);

/// Draws this frame's list: one mesh per run of quads sharing a sheet and a
/// blend, stacked in list order; then clears the list. A slot whose run is
/// the same as last frame's keeps its mesh (no rebuild, no upload).
fn present(
    mut commands: Commands,
    mut frame: ResMut<Ui2dFrame>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LayoutMaterial>>,
    mut slots: Query<SlotParts, With<Slot>>,
    mut built: Local<HashMap<Entity, SlotSource>>,
) {
    let frame = &mut *frame;
    let mut runs: Vec<(usize, u8, Vec<&Quad>)> = Vec::new();
    for (sheet, quads) in &frame.draws {
        for q in quads {
            let blend = q.blend.clamp(0, 2) as u8;
            match runs.last_mut() {
                Some((s, b, list)) if *s == *sheet && *b == blend => list.push(q),
                _ => runs.push((*sheet, blend, vec![q])),
            }
        }
    }
    let mut existing = slots.iter_mut();
    for (order, (sheet, blend, quads)) in runs.iter().enumerate() {
        let sheet_data = &frame.sheets[*sheet];
        let source = || SlotSource {
            image: sheet_data.image.id(),
            size: sheet_data.size,
            texel_scale: sheet_data.texel_scale.to_bits(),
            quads: quads.iter().map(|q| **q).collect(),
        };
        let material = frame
            .materials
            .entry((*sheet, *blend))
            .or_insert_with(|| {
                materials.add(LayoutMaterial {
                    texture: frame.sheets[*sheet].image.clone(),
                    blend: *blend,
                    srgb_source: frame.sheets[*sheet].srgb_source,
                })
            })
            .clone();
        let z = order as f32 * 0.01;
        match existing.next() {
            Some((entity, handle, mut m, mut t, mut v)) => {
                let same = built.get(&entity).is_some_and(|b| {
                    b.image == sheet_data.image.id()
                        && b.size == sheet_data.size
                        && b.texel_scale == sheet_data.texel_scale.to_bits()
                        && b.quads.len() == quads.len()
                        && b.quads.iter().zip(quads).all(|(a, b)| a == *b)
                });
                if !same && let Some(mut slot) = meshes.get_mut(&handle.0) {
                    *slot = quad_mesh(quads, sheet_data);
                    built.insert(entity, source());
                }
                if m.0 != material {
                    m.0 = material;
                }
                if t.translation.z != z {
                    t.translation.z = z;
                }
                v.set_if_neq(Visibility::Visible);
            }
            None => {
                let mesh = quad_mesh(quads, sheet_data);
                let entity = commands.spawn((
                    Slot,
                    Mesh2d(meshes.add(mesh)),
                    MeshMaterial2d(material),
                    Transform::from_xyz(0., 0., z),
                    Visibility::Visible,
                    RenderLayers::layer(LAYER),
                ));
                built.insert(entity.id(), source());
            }
        }
    }
    for (_, _, _, _, mut v) in existing {
        v.set_if_neq(Visibility::Hidden);
    }
    frame.draws.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Rebound(usize);

    #[test]
    fn a_resized_image_rebinds_its_materials_and_a_rewrite_does_not() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<LayoutMaterial>()
            .init_resource::<Rebound>()
            .add_systems(PostUpdate, rebind_resized.after(AssetEventSystems))
            .add_systems(
                Last,
                |mut events: MessageReader<AssetEvent<LayoutMaterial>>,
                 mut rebound: ResMut<Rebound>| {
                    rebound.0 += events
                        .read()
                        .filter(|e| matches!(e, AssetEvent::Modified { .. }))
                        .count();
                },
            );
        let target =
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .add(Image::new_target_texture(
                    64,
                    36,
                    TextureFormat::Rgba8UnormSrgb,
                    None,
                ));
        let other = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new_target_texture(
                8,
                8,
                TextureFormat::Rgba8UnormSrgb,
                None,
            ));
        let _materials = [target.clone(), other].map(|texture| {
            app.world_mut()
                .resource_mut::<Assets<LayoutMaterial>>()
                .add(LayoutMaterial {
                    texture,
                    blend: 0,
                    srgb_source: true,
                })
        });
        let settle = |app: &mut App| {
            for _ in 0..3 {
                app.update();
            }
        };
        settle(&mut app);
        app.world_mut().resource_mut::<Rebound>().0 = 0;
        // Same descriptor (a pixel rewrite): Bevy keeps the texture.
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&target)
            .unwrap()
            .into_inner();
        settle(&mut app);
        assert_eq!(app.world().resource::<Rebound>().0, 0);
        // A new size: only the material sampling it is prepared again.
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&target)
            .unwrap()
            .resize(Extent3d {
                width: 128,
                height: 72,
                depth_or_array_layers: 1,
            });
        settle(&mut app);
        assert_eq!(app.world().resource::<Rebound>().0, 1);
    }
}
