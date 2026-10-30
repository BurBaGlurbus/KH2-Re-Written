//! An event's depth of field: SeqFocus (0x18, FUN_1402da870) and the pass
//! FUN_14018a9e0 in its plain mode. The frame is shrunk to a quarter
//! (FUN_140189e50) and drawn back as five quads with a depth test at the
//! event's GS depth, so only what is beyond it blurs (FUN_14018a5a0): four
//! shifted 3 pixels at alpha 0x40, the last in place at 0x60.
//!
//! GS depth runs `16777215 / w` here: the exe's projection (near 1, far 4000000,
//! FUN_14019dc50) stores `1 - 1 / w`, and FUN_140142ba0 turns a GS depth into
//! `1 - z / 16777215`.
use crate::glow::{GlowPass, keep_alpha};
use bevy::{
    asset::{AssetPath, embedded_asset, embedded_path},
    core_pipeline::{
        core_3d::CORE_3D_DEPTH_FORMAT,
        schedule::{Core3d, Core3dSystems},
    },
    mesh::VertexBufferLayout,
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        camera::ExtractedCamera,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_resource::{
            BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendComponent,
            BlendFactor, BlendOperation, BlendState, BufferInitDescriptor, BufferUsages,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, CompareFunction,
            DepthStencilState, Extent3d, FilterMode, FragmentState, LoadOp, Operations,
            PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
            RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
            SpecializedRenderPipeline, SpecializedRenderPipelines, StoreOp, TextureDescriptor,
            TextureDimension, TextureFormat, TextureSampleType, TextureUsages, VertexFormat,
            VertexState, VertexStepMode,
            binding_types::{sampler, texture_2d},
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        texture::{CachedTexture, TextureCache},
        view::{ExtractedView, ViewDepthTexture, ViewTarget},
    },
    shader::Shader,
};

/// On an event's camera: the GS depth the blur starts at, while it's on.
#[derive(Component, Clone, Copy, Debug, Default, ExtractComponent)]
pub struct Focus {
    pub depth: Option<f32>,
}

/// FUN_140172100: a depth under 15000 counts as 15000.
pub const NEAREST_DEPTH: i32 = 15000;
const GS_DEPTH: f32 = 16777215.;
/// `(+x, +y)` shifts in pixels and alpha (0x80 = 1) of the five quads.
const QUADS: [(f32, f32, f32); 5] = [
    (0., -3., 0x40 as f32 / 128.),
    (-3., 0., 0x40 as f32 / 128.),
    (0., 3., 0x40 as f32 / 128.),
    (3., 0., 0x40 as f32 / 128.),
    (0., 0., 0x60 as f32 / 128.),
];
const VERTEX_FLOATS: usize = 7;

/// The view depth a GS depth stands for.
fn view_depth(gs: f32) -> f32 {
    GS_DEPTH / gs
}

/// Clip-space corners for `size`-pixel target, each `[x, y, z, u, v, a]` with
/// the depth `z`, in strip order per quad; six vertices a quad.
fn quads(size: UVec2, small: UVec2, z: f32) -> Vec<u8> {
    let (w, h) = (size.x as f32, size.y as f32);
    // The source rectangle in the quarter target: its (0.5, 0.5) to its size
    // + 0.5.
    let (u0, v0) = (0.5 / small.x as f32, 0.5 / small.y as f32);
    let (u1, v1) = (
        (small.x as f32 + 0.5) / small.x as f32,
        (small.y as f32 + 0.5) / small.y as f32,
    );
    let mut out = Vec::with_capacity(QUADS.len() * 6 * VERTEX_FLOATS * 4);
    for (dx, dy, alpha) in QUADS {
        let at = |x: f32, y: f32| [2. * (x + dx) / w - 1., 1. - 2. * (y + dy) / h];
        let corners = [
            (at(0., 0.), [u0, v0]),
            (at(w, 0.), [u1, v0]),
            (at(0., h), [u0, v1]),
            (at(w, 0.), [u1, v0]),
            (at(w, h), [u1, v1]),
            (at(0., h), [u0, v1]),
        ];
        for (p, t) in corners {
            for f in [p[0], p[1], z, 1., t[0], t[1], alpha] {
                out.extend(f.to_le_bytes());
            }
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    format: TextureFormat,
    shrink: bool,
}

#[derive(Resource)]
struct FocusPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    shader: Handle<Shader>,
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, server: Res<AssetServer>) {
    let layout = BindGroupLayoutDescriptor::new(
        "focus layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    let sampler = device.create_sampler(&SamplerDescriptor {
        min_filter: FilterMode::Linear,
        mag_filter: FilterMode::Linear,
        ..default()
    });
    commands.insert_resource(FocusPipeline {
        layout,
        sampler,
        shader: server
            .load(AssetPath::from_path_buf(embedded_path!("focus.wgsl")).with_source("embedded")),
    });
}

impl SpecializedRenderPipeline for FocusPipeline {
    type Key = Key;

    fn specialize(&self, key: Key) -> RenderPipelineDescriptor {
        let mut descriptor = RenderPipelineDescriptor {
            label: Some("focus".into()),
            layout: vec![self.layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some("vertex".into()),
                buffers: vec![VertexBufferLayout::from_vertex_formats(
                    VertexStepMode::Vertex,
                    [
                        VertexFormat::Float32x4,
                        VertexFormat::Float32x2,
                        VertexFormat::Float32,
                    ],
                )],
                ..default()
            },
            depth_stencil: (!key.shrink).then(|| DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                // Reversed z: the quad draws where the scene is behind it.
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: default(),
            }),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend: (!key.shrink).then_some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent::OVER,
                    }),
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        };
        // The scene's alpha holds the glow's stencil until the glow is done.
        if !key.shrink {
            keep_alpha(&mut descriptor);
        }
        descriptor
    }
}

#[derive(Component)]
struct FocusView {
    small: CachedTexture,
    shrink: CachedRenderPipelineId,
    blur: CachedRenderPipelineId,
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ExtractedCamera, &ViewTarget, &Focus)>,
    mut cache: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<FocusPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<FocusPipeline>>,
) {
    for (entity, camera, target, focus) in &views {
        let size = camera.physical_target_size.unwrap_or_default();
        if focus.depth.is_none() || size.x < 4 || size.y < 4 {
            commands.entity(entity).remove::<FocusView>();
            continue;
        }
        let format = target.main_texture_format();
        let small = cache.get(
            &device,
            TextureDescriptor {
                label: Some("focus quarter"),
                size: Extent3d {
                    width: size.x / 4,
                    height: size.y / 4,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let mut id =
            |shrink| pipelines.specialize(&pipeline_cache, &pipeline, Key { format, shrink });
        let (shrink, blur) = (id(true), id(false));
        commands.entity(entity).insert(FocusView {
            small,
            shrink,
            blur,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn focus(
    view: ViewQuery<(
        &ViewTarget,
        &ViewDepthTexture,
        &ExtractedView,
        &Focus,
        &FocusView,
    )>,
    cache: Res<PipelineCache>,
    pipeline: Res<FocusPipeline>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let (target, depth, extracted, focus, v) = view.into_inner();
    let (Some(gs), Some(shrink), Some(blur)) = (
        focus.depth,
        cache.get_render_pipeline(v.shrink),
        cache.get_render_pipeline(v.blur),
    ) else {
        return;
    };
    let clip = extracted.clip_from_view * Vec4::new(0., 0., -view_depth(gs), 1.);
    let z = clip.z / clip.w;
    let layout = cache.get_bind_group_layout(&pipeline.layout);
    let size = UVec2::new(depth.texture.width(), depth.texture.height());
    let small = UVec2::new(v.small.texture.width(), v.small.texture.height());
    let upload = |bytes: &[u8]| {
        device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("focus vertices"),
            contents: bytes,
            usage: BufferUsages::VERTEX,
        })
    };
    // The whole frame into the quarter target.
    let mut whole = Vec::new();
    for (x, y, u, w) in [
        (-1f32, 1f32, 0f32, 0f32),
        (1., 1., 1., 0.),
        (-1., -1., 0., 1.),
        (1., 1., 1., 0.),
        (1., -1., 1., 1.),
        (-1., -1., 0., 1.),
    ] {
        for f in [x, y, 0., 1., u, w, 1.] {
            whole.extend(f.to_le_bytes());
        }
    }
    let whole = upload(&whole);
    let from_scene = device.create_bind_group(
        Some("focus scene"),
        &layout,
        &BindGroupEntries::sequential((target.main_texture_view(), &pipeline.sampler)),
    );
    {
        let descriptor = RenderPassDescriptor {
            label: Some("focus shrink"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &v.small.default_view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        };
        let mut pass = ctx.begin_tracked_render_pass(descriptor);
        pass.set_render_pipeline(shrink);
        pass.set_bind_group(0, &from_scene, &[]);
        pass.set_vertex_buffer(0, whole.slice(..));
        pass.draw(0..6, 0..1);
    }
    let bytes = quads(size, small, z);
    let quads = upload(&bytes);
    let from_small = device.create_bind_group(
        Some("focus quarter"),
        &layout,
        &BindGroupEntries::sequential((&v.small.default_view, &pipeline.sampler)),
    );
    let descriptor = RenderPassDescriptor {
        label: Some("focus"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: target.main_texture_view(),
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Load,
                store: StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    };
    let mut pass = ctx.begin_tracked_render_pass(descriptor);
    pass.set_render_pipeline(blur);
    pass.set_bind_group(0, &from_small, &[]);
    pass.set_vertex_buffer(0, quads.slice(..));
    pass.draw(0..(bytes.len() / (VERTEX_FLOATS * 4)) as u32, 0..1);
}

pub struct FocusPlugin;

impl Plugin for FocusPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "focus.wgsl");
        app.add_plugins(ExtractComponentPlugin::<Focus>::default());
        let Some(render) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render
            .init_resource::<SpecializedRenderPipelines<FocusPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_systems(
                Core3d,
                focus
                    .in_set(Core3dSystems::EarlyPostProcess)
                    .before(GlowPass),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_events_depth_is_a_view_distance() {
        // Event 601's 20000 is about 839 units from the camera.
        assert!((view_depth(20000.) - 838.86).abs() < 0.01);
    }

    #[test]
    fn four_quads_shift_three_pixels_and_the_last_stays() {
        let bytes = quads(UVec2::new(1000, 500), UVec2::new(250, 125), 0.01);
        assert_eq!(bytes.len(), 5 * 6 * VERTEX_FLOATS * 4);
        let f = |vertex: usize, k: usize| {
            let at = (vertex * VERTEX_FLOATS + k) * 4;
            f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
        };
        // The first quad moves up 3 of 500 pixels (y down on the screen).
        assert!((f(0, 1) - (1. + 2. * 3. / 500.)).abs() < 1e-5);
        assert_eq!(f(0, 6), 0.5);
        // The last stays put at 0x60.
        assert_eq!(f(24, 0), -1.);
        assert_eq!(f(24, 1), 1.);
        assert_eq!(f(24, 6), 0.75);
        assert_eq!(f(24, 2), 0.01);
    }
}
