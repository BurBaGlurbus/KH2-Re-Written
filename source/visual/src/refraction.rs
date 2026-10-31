//! The frame-buffer effects: pppKeShpDtt (160), pppKeMdlDtt (145) and the
//! refraction ribbon (241 with +8). Each vertex carries the point whose screen
//! place it samples, so the picture behind it shows moved (Quick Run's haze, form
//! and boss effects). They draw after the scene's transparent pass: the frame
//! is copied, then the meshes sample the copy as texture × vertex colour with an
//! alpha blend, no depth write, and the depth test unless the effect turns it
//! off. Texture coordinates are screen places, so they run linear on the screen.
use crate::glow::{Glow, keep_alpha};
use bevy::{
    asset::{AssetPath, embedded_asset, embedded_path},
    core_pipeline::{
        core_3d::CORE_3D_DEPTH_FORMAT,
        schedule::{Core3d, Core3dSystems},
    },
    mesh::VertexBufferLayout,
    prelude::*,
    render::{
        ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems,
        camera::ExtractedCamera,
        render_resource::{
            BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendComponent,
            BlendFactor, BlendOperation, BlendState, BufferInitDescriptor, BufferUsages,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, CompareFunction,
            DepthStencilState, Extent3d, FilterMode, FragmentState, MultisampleState,
            PipelineCache, RenderPassDescriptor, RenderPipelineDescriptor, Sampler,
            SamplerBindingType, SamplerDescriptor, ShaderStages, SpecializedRenderPipeline,
            SpecializedRenderPipelines, StoreOp, TextureDescriptor, TextureDimension,
            TextureFormat, TextureSampleType, TextureUsages, VertexFormat, VertexState,
            VertexStepMode,
            binding_types::{sampler, texture_2d},
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        texture::{CachedTexture, TextureCache},
        view::{ExtractedView, ViewDepthTexture, ViewTarget},
    },
    shader::Shader,
};
use effect::Refraction;
use std::sync::Mutex;

/// A triangle in the scene's space: position, sampled point, colour (1.0 =
/// 0x80), for each corner.
type Corner = ([f32; 3], [f32; 3], [f32; 4]);

#[derive(Clone, Default)]
struct Frame {
    tested: Vec<Corner>,
    untested: Vec<Corner>,
}

impl Frame {
    const fn new() -> Self {
        Self {
            tested: Vec::new(),
            untested: Vec::new(),
        }
    }
}

/// Which effect list a publish replaces: each steps on its own.
#[derive(Clone, Copy)]
pub enum Host {
    Effects,
    Drive,
    Spells,
}

/// Each host's latest triangles. The VM steps on the tick and frames can
/// outnumber ticks, so each frame draws the latest lists.
static LATEST: Mutex<[Frame; 3]> = Mutex::new([const { Frame::new() }; 3]);

/// The room's way from the effect VM's view (x right, y down, z forward) to
/// the scene: through the view's inverse and the exe's world axes (x, -y, -z).
pub fn room_scene(view: &[[f32; 4]; 4]) -> Mat4 {
    Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.)) * Mat4::from_cols_array_2d(view).inverse()
}

/// A host's refractions of this tick, from the instances with every program
/// translated, with the host's matrix from the effect VM's view to the scene.
pub fn publish<'a>(
    host: Host,
    to_scene: Mat4,
    instances: impl Iterator<Item = &'a effect::Instance>,
) {
    let frame = convert(
        to_scene,
        instances
            .filter(|i| i.unsupported.is_empty())
            .flat_map(|i| i.refractions.iter()),
    );
    if let Ok(mut latest) = LATEST.lock() {
        latest[host as usize] = frame;
    }
}

fn convert<'a>(matrix: Mat4, list: impl Iterator<Item = &'a Refraction>) -> Frame {
    let mut frame = Frame::new();
    let to_scene = |p: [f32; 3]| matrix.transform_point3(Vec3::from(p)).to_array();
    for r in list {
        let out = if r.depth_test {
            &mut frame.tested
        } else {
            &mut frame.untested
        };
        let corner = |v: &effect::RefractionVertex| {
            (
                to_scene(v.position),
                to_scene(v.sample),
                v.rgba.map(|c| f32::from(c) / 128.),
            )
        };
        if r.fan {
            for pair in r.vertices[1..].windows(2) {
                out.extend([corner(&r.vertices[0]), corner(&pair[0]), corner(&pair[1])]);
            }
        } else {
            out.extend(r.vertices.iter().map(corner));
        }
    }
    frame
}

#[derive(Resource, Default)]
struct Current(Frame);

fn extract(mut current: ResMut<Current>) {
    current.0.tested.clear();
    current.0.untested.clear();
    if let Ok(latest) = LATEST.lock() {
        for frame in latest.iter() {
            current.0.tested.extend_from_slice(&frame.tested);
            current.0.untested.extend_from_slice(&frame.untested);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    format: TextureFormat,
    depth_test: bool,
    /// The scene's samples: the settings' anti-aliasing.
    samples: u32,
}

#[derive(Resource)]
struct RefractionPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    shader: Handle<Shader>,
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, server: Res<AssetServer>) {
    let layout = BindGroupLayoutDescriptor::new(
        "refraction layout",
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
    commands.insert_resource(RefractionPipeline {
        layout,
        sampler,
        shader: server.load(
            AssetPath::from_path_buf(embedded_path!("refraction.wgsl")).with_source("embedded"),
        ),
    });
}

impl SpecializedRenderPipeline for RefractionPipeline {
    type Key = Key;

    fn specialize(&self, key: Key) -> RenderPipelineDescriptor {
        let mut descriptor = RenderPipelineDescriptor {
            label: Some("refraction".into()),
            layout: vec![self.layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some("vertex".into()),
                buffers: vec![VertexBufferLayout::from_vertex_formats(
                    VertexStepMode::Vertex,
                    [
                        VertexFormat::Float32x4,
                        VertexFormat::Float32x2,
                        VertexFormat::Float32x4,
                    ],
                )],
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                // Reversed z.
                depth_compare: Some(if key.depth_test {
                    CompareFunction::GreaterEqual
                } else {
                    CompareFunction::Always
                }),
                stencil: default(),
                bias: default(),
            }),
            multisample: MultisampleState {
                count: key.samples,
                ..default()
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    // ALPHA_1 0x44: (Cs - Cd) · As + Cd.
                    blend: Some(BlendState {
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
        // The scene's alpha still holds the glow's stencil.
        keep_alpha(&mut descriptor);
        descriptor
    }
}

#[derive(Component)]
struct RefractionView {
    copy: CachedTexture,
    tested: CachedRenderPipelineId,
    untested: CachedRenderPipelineId,
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ExtractedCamera, &ViewTarget, &Msaa), With<Glow>>,
    current: Res<Current>,
    mut cache: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<RefractionPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<RefractionPipeline>>,
) {
    let active = !current.0.tested.is_empty() || !current.0.untested.is_empty();
    for (entity, camera, target, msaa) in &views {
        let size = camera.physical_target_size.unwrap_or_default();
        if !active || size.x == 0 || size.y == 0 {
            commands.entity(entity).remove::<RefractionView>();
            continue;
        }
        let format = target.main_texture_format();
        let mut id = |depth_test| {
            pipelines.specialize(
                &pipeline_cache,
                &pipeline,
                Key {
                    format,
                    depth_test,
                    samples: msaa.samples(),
                },
            )
        };
        let (tested, untested) = (id(true), id(false));
        let copy = cache.get(
            &device,
            TextureDescriptor {
                label: Some("refraction copy"),
                size: Extent3d {
                    width: size.x,
                    height: size.y,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        commands.entity(entity).insert(RefractionView {
            copy,
            tested,
            untested,
        });
    }
}

/// A corner's vertex data: 10 floats.
const VERTEX_BYTES: usize = 40;

/// A corner on the screen: clip position, the sampled point's place on the
/// screen, colour; `None` behind the camera.
fn on_screen(corner: &Corner, clip_from_world: Mat4) -> Option<[f32; 10]> {
    let (position, sample, colour) = corner;
    let clip = clip_from_world * Vec3::from(*position).extend(1.);
    let at = clip_from_world * Vec3::from(*sample).extend(1.);
    if clip.w <= 0. || at.w <= 0. {
        return None;
    }
    let (u, v) = (at.x / at.w * 0.5 + 0.5, 0.5 - at.y / at.w * 0.5);
    Some([
        clip.x, clip.y, clip.z, clip.w, u, v, colour[0], colour[1], colour[2], colour[3],
    ])
}

/// The triangles of a list as vertex data.
fn vertices(list: &[Corner], clip_from_world: Mat4) -> Vec<u8> {
    let mut out = Vec::with_capacity(list.len() * VERTEX_BYTES);
    for triangle in list.as_chunks::<3>().0 {
        let corners: Vec<_> = triangle
            .iter()
            .filter_map(|c| on_screen(c, clip_from_world))
            .collect();
        if corners.len() == 3 {
            for f in corners.iter().flatten() {
                out.extend(f.to_le_bytes());
            }
        }
    }
    out
}

fn refract(
    view: ViewQuery<(
        &ViewTarget,
        &ViewDepthTexture,
        &ExtractedView,
        &RefractionView,
    )>,
    current: Res<Current>,
    cache: Res<PipelineCache>,
    pipeline: Res<RefractionPipeline>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let (target, depth, extracted, r) = view.into_inner();
    let (Some(tested), Some(untested)) = (
        cache.get_render_pipeline(r.tested),
        cache.get_render_pipeline(r.untested),
    ) else {
        return;
    };
    let clip_from_world =
        extracted.clip_from_view * extracted.world_from_view.to_matrix().inverse();
    let upload = |bytes: Vec<u8>| {
        (!bytes.is_empty()).then(|| {
            let buffer = device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("refraction vertices"),
                contents: &bytes,
                usage: BufferUsages::VERTEX,
            });
            (buffer, (bytes.len() / VERTEX_BYTES) as u32)
        })
    };
    let lists = [
        (tested, upload(vertices(&current.0.tested, clip_from_world))),
        (
            untested,
            upload(vertices(&current.0.untested, clip_from_world)),
        ),
    ];
    ctx.command_encoder().copy_texture_to_texture(
        target.main_texture().as_image_copy(),
        r.copy.texture.as_image_copy(),
        Extent3d {
            width: r.copy.texture.width(),
            height: r.copy.texture.height(),
            depth_or_array_layers: 1,
        },
    );
    let group = device.create_bind_group(
        Some("refraction group"),
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((&r.copy.default_view, &pipeline.sampler)),
    );
    // The scene's own attachment: its depth has the same samples, and with
    // anti-aliasing on the pass resolves back into the main texture.
    let descriptor = RenderPassDescriptor {
        label: Some("refraction"),
        color_attachments: &[Some(target.get_color_attachment())],
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    };
    let mut pass = ctx.begin_tracked_render_pass(descriptor);
    pass.set_bind_group(0, &group, &[]);
    for (pipeline, list) in &lists {
        if let Some((buffer, count)) = list {
            pass.set_render_pipeline(pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
        }
    }
}

pub struct RefractionPlugin;

impl Plugin for RefractionPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "refraction.wgsl");
        let Some(render) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render
            .init_resource::<Current>()
            .init_resource::<SpecializedRenderPipelines<RefractionPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(ExtractSchedule, extract)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_systems(
                Core3d,
                refract
                    .in_set(Core3dSystems::EarlyPostProcess)
                    .before(crate::glow::GlowPass),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fan_becomes_triangles_and_goes_to_the_scene_through_the_view() {
        let frame = fan_frame();
        assert_eq!(frame.tested.len(), 6);
        // View (x, 0, 10) with the view 10 back: world z 20, scene -20.
        assert_eq!(frame.tested[0].0, [0., 0., -20.]);
        assert_eq!(frame.tested[1].1, [2., 0., -20.]);
        assert_eq!(frame.tested[0].2, [1., 0.5, 0., 1.]);
        assert!(frame.untested.is_empty());
    }

    /// `publish`'s conversion on a fan, without the shared lists.
    fn fan_frame() -> Frame {
        let v = |x: f32| effect::RefractionVertex {
            position: [x, 0., 10.],
            sample: [x + 1., 0., 10.],
            rgba: [0x80, 0x40, 0, 0x80],
        };
        let r = Refraction {
            source: effect::FrameSource::Current,
            fan: true,
            vertices: vec![v(0.), v(1.), v(2.), v(3.)],
            depth_test: true,
            order: effect::RefractionOrder::Last,
        };
        let mut view = Mat4::IDENTITY.to_cols_array_2d();
        view[3] = [0., 0., -10., 1.];
        convert(room_scene(&view), [&r].into_iter())
    }

    #[test]
    fn a_corner_samples_the_screen_place_of_its_point() {
        let projection = Mat4::perspective_infinite_reverse_rh(1., 1., 5.);
        // In front of the camera (-z), on its axis: the screen's middle.
        let corner = ([0., 0., -100.], [0., 0., -100.], [1.; 4]);
        let out = on_screen(&corner, projection).unwrap();
        assert_eq!(out[4], 0.5);
        assert_eq!(out[5], 0.5);
        // Behind the camera: not drawn.
        assert!(on_screen(&([0., 0., 100.], [0., 0., 100.], [1.; 4]), projection).is_none());
    }
}
