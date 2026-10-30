//! dk::Glow (FUN_140169bd0, DAT_140abd198): the area's bloom. The exe's map
//! and sky opaque chunks write a stencil of 0xff where a triangle has a vertex
//! alpha over 0x80 (FUN_1401ba2c0, FUN_1401b9090) and 0 elsewhere; after the
//! scene the glow copies the frame where the stencil is 0xff, shrinks it to a
//! quarter, blurs it with six horizontal and vertical pairs of half-blended
//! shifted copies and adds the result back at full weight. No threshold and no
//! strength: the only input is an on/off flag (arif flag bit 4 at map load,
//! FUN_1403a5a80; SeqGlow in events).
//!
//! The room has no stencil, so the scene's alpha channel holds it: map and sky
//! opaque chunks (a `UV_1` class per triangle, read by `fog.wgsl`) write their
//! class to alpha, and everything else on a [`Glow`] camera leaves alpha alone
//! ([`keeps_alpha`]). The pass puts alpha back to 1 when it is done.
use bevy::{
    asset::{AssetPath, embedded_asset, embedded_path},
    color::LinearRgba,
    core_pipeline::{
        FullscreenShader,
        schedule::{Core3d, Core3dSystems},
    },
    light::ShadowFilteringMethod,
    mesh::VertexBufferLayout,
    pbr::MeshPipelineKey,
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        camera::ExtractedCamera,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            BlendComponent, BlendFactor, BlendOperation, BlendState, Buffer, BufferInitDescriptor,
            BufferUsages, CachedRenderPipelineId, ColorTargetState, ColorWrites, Extent3d,
            FilterMode, FragmentState, LoadOp, Operations, PipelineCache, PrimitiveState,
            PrimitiveTopology, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
            RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
            SpecializedRenderPipeline, SpecializedRenderPipelines, StoreOp, TextureDescriptor,
            TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
            VertexFormat, VertexState, VertexStepMode,
            binding_types::{sampler, texture_2d},
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        texture::{CachedTexture, TextureCache},
        view::ViewTarget,
    },
    shader::Shader,
};

/// On a scene camera: runs the pass, and keeps alpha for the stencil. A camera
/// without it (the camp's character views) draws as it always did.
#[derive(Component, Clone, Copy, Debug, ExtractComponent)]
pub struct Glow {
    pub on: bool,
    /// The camera's own clear colour; `None` follows the `ClearColor` resource.
    pub clear: Option<Color>,
}

/// The stencil's clear (0): the camera's clear colour with alpha 0.
fn clear_stencil(global: Res<ClearColor>, mut cameras: Query<(&mut Camera, &Glow)>) {
    for (mut camera, glow) in &mut cameras {
        let colour = glow.clear.unwrap_or(global.0).with_alpha(0.);
        if !matches!(camera.clear_color, ClearColorConfig::Custom(c) if c == colour) {
            camera.clear_color = ClearColorConfig::Custom(colour);
        }
    }
}

/// What a [`Glow`] camera wears: the marker below in its pipeline key, and a
/// depth buffer the sun's visibility test can copy from.
pub fn camera(on: bool, clear: Option<Color>) -> (Glow, ShadowFilteringMethod, Camera3d) {
    (
        Glow { on, clear },
        ShadowFilteringMethod::Hardware2x2,
        Camera3d {
            depth_texture_usages: (TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING)
                .into(),
            ..default()
        },
    )
}

/// Pipelines of a [`Glow`] camera. The room draws no shadows, so the filter's
/// key bits are free to mark it (the default is Gaussian).
pub fn keeps_alpha(key: MeshPipelineKey) -> bool {
    key & MeshPipelineKey::SHADOW_FILTER_METHOD_RESERVED_BITS
        == MeshPipelineKey::SHADOW_FILTER_METHOD_HARDWARE_2X2
}

/// Nothing but the map and the sky writes the stencil.
pub fn keep_alpha(descriptor: &mut RenderPipelineDescriptor) {
    if let Some(fragment) = descriptor.fragment.as_mut() {
        for target in fragment.targets.iter_mut().flatten() {
            target.write_mask = ColorWrites::COLOR;
        }
    }
}

/// A map or sky opaque triangle's class: 1 when a vertex alpha (`c / 128`) is
/// over 0x80 (FUN_1401b9b30), one per corner of the triangle list.
pub fn triangle_classes(alphas: &[f32]) -> Vec<[f32; 2]> {
    let mut out = Vec::with_capacity(alphas.len());
    for t in alphas.chunks(3) {
        let class = if t.iter().any(|a| *a > 1.) { 1. } else { 0. };
        out.extend(std::iter::repeat_n([class, 0.], t.len()));
    }
    out
}

const QUAD_VERTICES: u32 = 4;
/// Masked copy, shrink, 6 x 2 x (copy, half blend), composite.
const QUADS: usize = 2 + 6 * 2 * 2 + 1;
/// The table at 0x1405b1f80: (a, c, d) of each iteration.
const SHIFTS: [[i32; 3]; 6] = [
    [1, 0, -1],
    [-1, 1, 0],
    [2, 0, -2],
    [-2, 2, 0],
    [0, 0, -3],
    [0, 3, 0],
];

/// One quad in the exe's terms: destination and source rectangles in pixels of
/// their targets, with those targets' sizes.
struct Rect {
    dest: [f32; 4],
    uv: [f32; 4],
    dest_size: [f32; 2],
    source_size: [f32; 2],
}

fn rects(w: u32, h: u32) -> Vec<Rect> {
    let (w4, h4) = ((w / 4) as f32, (h / 4) as f32);
    let (w, h) = (w as f32, h as f32);
    let (full, small) = ([w, h], [w4, h4]);
    let mut out = Vec::with_capacity(QUADS);
    // A: the masked 1:1 copy. B: 4:1.
    out.push(Rect {
        dest: [0., 0., w, h],
        uv: [0.5, 0.5, w + 0.5, h + 0.5],
        dest_size: full,
        source_size: full,
    });
    out.push(Rect {
        dest: [0., 0., w4, h4],
        uv: [1.5, 1.5, w - 0.5, h - 0.5],
        dest_size: small,
        source_size: full,
    });
    // C: v = (a, c, d) · 16 · 0.5 in 12.4 units, the destination's shift
    // floored to whole pixels by vertex shader 000.
    for [a, c, d] in SHIFTS {
        let (v0, v2, v3) = (
            (a * 8) as f32 / 16.,
            (c * 8) as f32 / 16.,
            (d * 8) as f32 / 16.,
        );
        let shift = v0.floor();
        for pass in 0..2 {
            let m = if pass == 0 { 0. } else { 1. };
            out.push(Rect {
                dest: [0., 0., w4, h4],
                uv: [0.5 + m, 0.5 + m, w4 + 0.5 - m, h4 + 0.5 - m],
                dest_size: small,
                source_size: small,
            });
            let (dest, uv) = if pass == 0 {
                (
                    [shift, 0., w4 + shift, h4],
                    [0.5 + v2, 0.5, w4 + 0.5 + v3, h4 + 0.5],
                )
            } else {
                (
                    [0., shift, w4, h4 + shift],
                    [0.5, 0.5 + v2, w4 + 0.5, h4 + 0.5 + v3],
                )
            };
            out.push(Rect {
                dest,
                uv,
                dest_size: small,
                source_size: small,
            });
        }
    }
    // D: the composite.
    out.push(Rect {
        dest: [0., 0., w, h],
        uv: [0.5, 0.5, w4, h4],
        dest_size: full,
        source_size: small,
    });
    out
}

fn vertices(rects: &[Rect]) -> Vec<[f32; 4]> {
    let mut out = Vec::with_capacity(rects.len() * QUAD_VERTICES as usize);
    for r in rects {
        let px = |x: f32, y: f32| [2. * x / r.dest_size[0] - 1., 1. - 2. * y / r.dest_size[1]];
        let st = |u: f32, v: f32| [u / r.source_size[0], v / r.source_size[1]];
        let [x0, y0, x1, y1] = r.dest;
        let [u0, v0, u1, v1] = r.uv;
        for (p, t) in [
            (px(x0, y0), st(u0, v0)),
            (px(x1, y0), st(u1, v0)),
            (px(x0, y1), st(u0, v1)),
            (px(x1, y1), st(u1, v1)),
        ] {
            out.push([p[0], p[1], t[0], t[1]]);
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    /// The stencil test: the scene's colour where its alpha is set.
    Masked,
    /// No blend.
    Copy,
    /// ALPHA_1 0x4000000064: 0.5·Cs + 0.5·Cd.
    Half,
    /// ALPHA_1 0x8000000068: Cs + Cd.
    Add,
    /// The scene's alpha back to 1.
    Reset,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    kind: Kind,
    format: TextureFormat,
}

#[derive(Resource)]
struct GlowPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    shader: Handle<Shader>,
    fullscreen: FullscreenShader,
}

fn init_pipeline(
    mut commands: Commands,
    device: Res<RenderDevice>,
    fullscreen: Res<FullscreenShader>,
    server: Res<AssetServer>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "glow bind group layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    // CLAMP / CLAMP (FUN_1401271c0(1,1,0,0)); linear.
    let sampler = device.create_sampler(&SamplerDescriptor {
        min_filter: FilterMode::Linear,
        mag_filter: FilterMode::Linear,
        ..default()
    });
    commands.insert_resource(GlowPipeline {
        layout,
        sampler,
        shader: server
            .load(AssetPath::from_path_buf(embedded_path!("glow.wgsl")).with_source("embedded")),
        fullscreen: fullscreen.clone(),
    });
}

impl SpecializedRenderPipeline for GlowPipeline {
    type Key = Key;

    fn specialize(&self, key: Key) -> RenderPipelineDescriptor {
        let (entry, blend, write_mask) = match key.kind {
            Kind::Masked => ("masked", None, ColorWrites::ALL),
            Kind::Copy => ("copy", None, ColorWrites::ALL),
            Kind::Half => {
                let half = BlendComponent {
                    src_factor: BlendFactor::Constant,
                    dst_factor: BlendFactor::OneMinusConstant,
                    operation: BlendOperation::Add,
                };
                (
                    "copy",
                    Some(BlendState {
                        color: half,
                        alpha: half,
                    }),
                    ColorWrites::ALL,
                )
            }
            Kind::Add => {
                let add = BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                };
                (
                    "copy",
                    Some(BlendState {
                        color: add,
                        alpha: add,
                    }),
                    ColorWrites::COLOR,
                )
            }
            Kind::Reset => ("opaque", None, ColorWrites::ALPHA),
        };
        let vertex = if key.kind == Kind::Reset {
            self.fullscreen.to_vertex_state()
        } else {
            VertexState {
                shader: self.shader.clone(),
                entry_point: Some("vertex".into()),
                buffers: vec![VertexBufferLayout::from_vertex_formats(
                    VertexStepMode::Vertex,
                    [VertexFormat::Float32x2, VertexFormat::Float32x2],
                )],
                ..default()
            }
        };
        RenderPipelineDescriptor {
            label: Some("glow".into()),
            layout: if key.kind == Kind::Reset {
                Vec::new()
            } else {
                vec![self.layout.clone()]
            },
            vertex,
            primitive: PrimitiveState {
                topology: if key.kind == Kind::Reset {
                    PrimitiveTopology::TriangleList
                } else {
                    PrimitiveTopology::TriangleStrip
                },
                ..default()
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                entry_point: Some(entry.into()),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend,
                    write_mask,
                })],
                ..default()
            }),
            ..default()
        }
    }
}

/// The small targets' format: blends on the stored values, as the PC's.
const TARGET: TextureFormat = TextureFormat::Rgba8Unorm;

#[derive(Component)]
struct GlowView {
    /// RT1 at full size, then the two quarter-size targets (RT1 and RT2).
    full: CachedTexture,
    a: CachedTexture,
    b: CachedTexture,
    masked: CachedRenderPipelineId,
    copy: CachedRenderPipelineId,
    half: CachedRenderPipelineId,
    add: CachedRenderPipelineId,
    vertices: Buffer,
}

#[derive(Component)]
struct GlowReset(CachedRenderPipelineId);

/// The vertices of the last size seen.
#[derive(Resource, Default)]
struct GlowGeometry(Option<([u32; 2], Buffer)>);

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ExtractedCamera, &ViewTarget, &Glow)>,
    mut cache: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<GlowPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<GlowPipeline>>,
    mut geometry: ResMut<GlowGeometry>,
) {
    for (entity, camera, target, glow) in &views {
        let format = target.main_texture_format();
        let mut id =
            |kind, format| pipelines.specialize(&pipeline_cache, &pipeline, Key { kind, format });
        let reset = id(Kind::Reset, format);
        commands.entity(entity).insert(GlowReset(reset));
        let size = camera.physical_target_size.unwrap_or_default();
        if !glow.on || size.x < 4 || size.y < 4 {
            commands.entity(entity).remove::<GlowView>();
            continue;
        }
        let (masked, copy, half, add) = (
            id(Kind::Masked, TARGET),
            id(Kind::Copy, TARGET),
            id(Kind::Half, TARGET),
            id(Kind::Add, format),
        );
        let mut texture = |label, width, height| {
            cache.get(
                &device,
                TextureDescriptor {
                    label: Some(label),
                    size: Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TARGET,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
        };
        let (full, a, b) = (
            texture("glow_full", size.x, size.y),
            texture("glow_a", size.x / 4, size.y / 4),
            texture("glow_b", size.x / 4, size.y / 4),
        );
        let key = [size.x, size.y];
        let vertices = match &geometry.0 {
            Some((at, buffer)) if *at == key => buffer.clone(),
            _ => {
                let data: Vec<u8> = vertices(&rects(size.x, size.y))
                    .iter()
                    .flatten()
                    .flat_map(|f| f.to_le_bytes())
                    .collect();
                let buffer = device.create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("glow vertices"),
                    contents: &data,
                    usage: BufferUsages::VERTEX,
                });
                geometry.0 = Some((key, buffer.clone()));
                buffer
            }
        };
        commands.entity(entity).insert(GlowView {
            full,
            a,
            b,
            masked,
            copy,
            half,
            add,
            vertices,
        });
    }
}

/// One pass: its quads, each with its pipeline, from one source.
fn run(
    ctx: &mut RenderContext,
    target: &TextureView,
    keep: bool,
    vertices: &Buffer,
    draws: &[(&RenderPipeline, Option<&BindGroup>, u32)],
) {
    let descriptor = RenderPassDescriptor {
        label: Some("glow"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: if keep {
                Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                }
            } else {
                Operations::default()
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    };
    let mut pass = ctx.begin_tracked_render_pass(descriptor);
    pass.set_vertex_buffer(0, vertices.slice(..));
    pass.set_blend_constant(LinearRgba::new(0.5, 0.5, 0.5, 0.5));
    for (pipeline, group, quad) in draws {
        pass.set_render_pipeline(pipeline);
        if let Some(group) = group {
            pass.set_bind_group(0, group, &[]);
        }
        pass.draw(quad * QUAD_VERTICES..(quad + 1) * QUAD_VERTICES, 0..1);
    }
}

/// FUN_140169bd0.
fn glow(
    view: ViewQuery<(&ViewTarget, &GlowReset, Option<&GlowView>)>,
    cache: Res<PipelineCache>,
    pipeline: Res<GlowPipeline>,
    mut ctx: RenderContext,
) {
    let (target, reset, glow) = view.into_inner();
    let scene = target.main_texture_view();
    if let Some(g) = glow {
        let found = (
            cache.get_render_pipeline(g.masked),
            cache.get_render_pipeline(g.copy),
            cache.get_render_pipeline(g.half),
            cache.get_render_pipeline(g.add),
        );
        if let (Some(masked), Some(copy), Some(half), Some(add)) = found {
            let layout = cache.get_bind_group_layout(&pipeline.layout);
            let group = |view: &TextureView| {
                ctx.render_device().create_bind_group(
                    Some("glow bind group"),
                    &layout,
                    &BindGroupEntries::sequential((view, &pipeline.sampler)),
                )
            };
            let (from_scene, from_full, from_a, from_b) = (
                group(scene),
                group(&g.full.default_view),
                group(&g.a.default_view),
                group(&g.b.default_view),
            );
            let v = &g.vertices;
            run(
                &mut ctx,
                &g.full.default_view,
                false,
                v,
                &[(masked, Some(&from_scene), 0)],
            );
            run(
                &mut ctx,
                &g.b.default_view,
                false,
                v,
                &[(copy, Some(&from_full), 1)],
            );
            let mut quad = 2;
            for _ in SHIFTS {
                for (to, from) in [(&g.a, &from_b), (&g.b, &from_a)] {
                    run(
                        &mut ctx,
                        &to.default_view,
                        false,
                        v,
                        &[(copy, Some(from), quad), (half, Some(from), quad + 1)],
                    );
                    quad += 2;
                }
            }
            run(&mut ctx, scene, true, v, &[(add, Some(&from_b), quad)]);
        }
    }
    // The stencil is done with; the frame goes on with an opaque alpha.
    if let Some(pipeline) = cache.get_render_pipeline(reset.0) {
        let descriptor = RenderPassDescriptor {
            label: Some("glow reset"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: scene,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        };
        let mut pass = ctx.begin_tracked_render_pass(descriptor);
        pass.set_render_pipeline(pipeline);
        pass.draw(0..3, 0..1);
    }
}

/// The glow's render system, for what must draw into the frame before it.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct GlowPass;

pub struct GlowPlugin;

impl Plugin for GlowPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "glow.wgsl");
        app.add_plugins((
            ExtractComponentPlugin::<Glow>::default(),
            crate::lens_probe::LensProbePlugin,
            crate::refraction::RefractionPlugin,
        ))
        .add_systems(PostUpdate, clear_stencil);
        let Some(render) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render
            .init_resource::<SpecializedRenderPipelines<GlowPipeline>>()
            .init_resource::<GlowGeometry>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_systems(
                Core3d,
                glow.in_set(Core3dSystems::EarlyPostProcess)
                    .in_set(GlowPass),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_triangle_is_class_one_when_any_vertex_alpha_is_over_0x80() {
        let at = |a: u8| f32::from(a) / 128.;
        let classes = triangle_classes(&[at(0x80), at(0x40), at(0x10), at(0x10), at(0x81), at(0)]);
        assert_eq!(classes[0][0], 0.);
        assert_eq!(classes[2][0], 0.);
        assert_eq!(classes[3][0], 1.);
        assert_eq!(classes[5][0], 1.);
    }

    /// The per-iteration values of the exe's blur at 1920x1080 (480x270).
    #[test]
    fn the_blur_quads_follow_the_shift_table() {
        let r = rects(1920, 1080);
        assert_eq!(r.len(), QUADS);
        assert_eq!(r[0].uv, [0.5, 0.5, 1920.5, 1080.5]);
        assert_eq!(r[1].uv, [1.5, 1.5, 1919.5, 1079.5]);
        assert_eq!(r[1].dest, [0., 0., 480., 270.]);
        // (destination shift, first u, last u) of each iteration's blended
        // horizontal quad.
        let table = [
            (0., 0.5, 480.),
            (-1., 1., 480.5),
            (1., 0.5, 479.5),
            (-1., 1.5, 480.5),
            (0., 0.5, 479.),
            (0., 2., 480.5),
        ];
        for (k, (shift, u0, u1)) in table.into_iter().enumerate() {
            let blended = &r[2 + k * 4 + 1];
            assert_eq!(blended.dest[0], shift, "iteration {k}");
            assert_eq!(blended.uv[0], u0, "iteration {k}");
            assert_eq!(blended.uv[2], u1, "iteration {k}");
            // The vertical pass shrinks its plain copy by a pixel and moves
            // y by the same numbers (h4 = 270).
            let plain = &r[2 + k * 4 + 2];
            assert_eq!(plain.uv, [1.5, 1.5, 479.5, 269.5]);
            let vertical = &r[2 + k * 4 + 3];
            assert_eq!(vertical.dest[1], shift, "iteration {k}");
            assert_eq!(vertical.uv[1], u0, "iteration {k}");
            assert_eq!(vertical.uv[3], u1 - 480. + 270., "iteration {k}");
        }
        let last = r.last().unwrap();
        assert_eq!(last.dest, [0., 0., 1920., 1080.]);
        assert_eq!(last.uv, [0.5, 0.5, 480., 270.]);
    }

    #[test]
    fn a_full_quad_fills_clip_space() {
        let v = vertices(&rects(1280, 720)[..1]);
        assert_eq!(v.len(), QUAD_VERTICES as usize);
        assert_eq!([v[0][0], v[0][1]], [-1., 1.]);
        assert_eq!([v[3][0], v[3][1]], [1., -1.]);
    }
}
