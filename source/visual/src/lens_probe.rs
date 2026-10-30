//! The sun's visibility test. FUN_1404d7120 draws a small block at the light's
//! screen point against the depth buffer (black everywhere, white where the
//! light is in front) and reads the block back: `share = white / (white +
//! black)`. Here the block's depth is written to a small colour target after
//! the main pass (a depth texture copies only whole) and counted when the GPU
//! has finished with it, so the share reaches the effect a few frames late;
//! the exe's smoothing stays in the program.
use crate::glow::Glow;
use bevy::{
    asset::{AssetPath, embedded_asset, embedded_path},
    core_pipeline::{
        FullscreenShader,
        schedule::{Core3d, Core3dSystems},
    },
    prelude::*,
    render::{
        ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems,
        render_resource::{
            BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, Buffer,
            BufferDescriptor, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId,
            ColorTargetState, ColorWrites, Extent3d, FragmentState, MapMode, Operations,
            PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
            RenderPipelineDescriptor, ShaderStages, TexelCopyBufferInfo, TexelCopyBufferLayout,
            TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
            TextureViewDescriptor,
            binding_types::{texture_depth_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::{ExtractedView, ViewDepthTexture},
    },
};
use effect::lens::{Probe, set_seen, take_probe};

/// A buffer copy's row alignment.
const ROW: u32 = 256;
/// The block's copy of the depth.
const BLOCK: TextureFormat = TextureFormat::R32Float;

/// A probe's block in the render world.
#[derive(Resource, Default)]
struct Asked(Option<Probe>);

/// The copies waiting to be mapped (after the frame's submit).
#[derive(Resource, Default)]
struct Copies(Vec<Copy>);

struct Copy {
    buffer: Buffer,
    width: u32,
    height: u32,
    /// The light's depth: a pixel is behind it when its value is smaller
    /// (reversed z).
    light: f32,
}

fn extract(mut asked: ResMut<Asked>) {
    asked.0 = take_probe();
}

/// The block's pixels and the light's depth, from the view's projection.
fn block(probe: &Probe, clip_from_view: Mat4, size: UVec2) -> Option<(IRect, f32)> {
    let [x, y, z] = probe.at;
    // View space here is x right, y down, z forward.
    let clip = clip_from_view * Vec4::new(x, -y, -z, 1.);
    if clip.w <= 0. {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    let at = Vec2::new(
        (ndc.x + 1.) * 0.5 * size.x as f32,
        (1. - ndc.y) * 0.5 * size.y as f32,
    );
    let half = Vec2::from(probe.half);
    let (low, high) = ((at - half).as_ivec2(), (at + half).as_ivec2());
    let rect = IRect::from_corners(
        low.clamp(IVec2::ZERO, size.as_ivec2()),
        high.clamp(IVec2::ZERO, size.as_ivec2()),
    );
    (rect.width() > 0 && rect.height() > 0).then_some((rect, ndc.z))
}

#[derive(Resource)]
struct ProbePipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedRenderPipelineId,
}

fn init_pipeline(
    mut commands: Commands,
    cache: Res<PipelineCache>,
    fullscreen: Res<FullscreenShader>,
    server: Res<AssetServer>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "lens probe layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (texture_depth_2d(), uniform_buffer::<UVec4>(false)),
        ),
    );
    let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("lens probe".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: server.load(
                AssetPath::from_path_buf(embedded_path!("lens_probe.wgsl")).with_source("embedded"),
            ),
            targets: vec![Some(ColorTargetState {
                format: BLOCK,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(ProbePipeline { layout, id });
}

#[allow(clippy::too_many_arguments)]
fn copy_block(
    view: ViewQuery<(&ViewDepthTexture, &ExtractedView, &Glow)>,
    mut asked: ResMut<Asked>,
    mut copies: ResMut<Copies>,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
    pipeline: Res<ProbePipeline>,
    mut ctx: RenderContext,
) {
    let Some(probe) = asked.0.take() else {
        return;
    };
    let Some(render_pipeline) = cache.get_render_pipeline(pipeline.id) else {
        return;
    };
    let (depth, extracted, _) = view.into_inner();
    let size = UVec2::new(depth.texture.width(), depth.texture.height());
    // Off the screen: FUN_1404d7120 sets the strength and +0xac to 0.
    let Some((rect, light)) = block(&probe, extracted.clip_from_view, size) else {
        set_seen(0.);
        return;
    };
    let (width, height) = (rect.width() as u32, rect.height() as u32);
    let extent = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&TextureDescriptor {
        label: Some("lens probe block"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: BLOCK,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&TextureViewDescriptor::default());
    let origin = [rect.min.x as u32, rect.min.y as u32, 0, 0];
    let uniform = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("lens probe origin"),
        contents: &origin.map(u32::to_le_bytes).concat(),
        usage: BufferUsages::UNIFORM,
    });
    let group = device.create_bind_group(
        Some("lens probe group"),
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((depth.view(), uniform.as_entire_binding())),
    );
    {
        let descriptor = RenderPassDescriptor {
            label: Some("lens probe"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &target_view,
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
        pass.set_render_pipeline(render_pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
    let bytes_per_row = (width * 4).div_ceil(ROW) * ROW;
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("lens probe"),
        size: u64::from(bytes_per_row * height),
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    ctx.command_encoder().copy_texture_to_buffer(
        target.as_image_copy(),
        TexelCopyBufferInfo {
            buffer: &buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        extent,
    );
    copies.0.push(Copy {
        buffer,
        width,
        height,
        light,
    });
}

/// The share of a block's rows behind the light.
fn share(bytes: &[u8], width: u32, height: u32, light: f32) -> f32 {
    let row = ((width * 4).div_ceil(ROW) * ROW) as usize;
    let mut seen = 0u32;
    for y in 0..height as usize {
        for x in 0..width as usize {
            let at = y * row + x * 4;
            if let Some(b) = bytes.get(at..at + 4)
                && f32::from_le_bytes([b[0], b[1], b[2], b[3]]) <= light
            {
                seen += 1;
            }
        }
    }
    seen as f32 / (width * height).max(1) as f32
}

fn map_copies(mut copies: ResMut<Copies>) {
    for copy in copies.0.drain(..) {
        let buffer = copy.buffer.clone();
        copy.buffer.slice(..).map_async(MapMode::Read, move |done| {
            if done.is_err() {
                return;
            }
            let answer = {
                let data = buffer.slice(..).get_mapped_range();
                share(&data, copy.width, copy.height, copy.light)
            };
            buffer.unmap();
            set_seen(answer);
        });
    }
}

pub struct LensProbePlugin;

impl Plugin for LensProbePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "lens_probe.wgsl");
        let Some(render) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render
            .init_resource::<Asked>()
            .init_resource::<Copies>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(ExtractSchedule, extract)
            .add_systems(Render, map_copies.in_set(RenderSystems::Cleanup))
            .add_systems(Core3d, copy_block.in_set(Core3dSystems::EarlyPostProcess));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_block(values: &[f32]) -> Vec<u8> {
        let mut out = vec![0u8; ROW as usize];
        for (i, v) in values.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// Reversed z: 0 is the far clear, larger is nearer.
    #[test]
    fn a_pixel_counts_when_the_scene_there_is_behind_the_light() {
        let bytes = row_block(&[0., 0.0005, 0.002, 0.5]);
        assert_eq!(share(&bytes, 4, 1, 0.001), 0.5);
    }

    #[test]
    fn a_block_off_the_screen_has_no_pixels() {
        let probe = Probe {
            at: [100_000., 0., 10.],
            half: [16., 4.],
        };
        let projection = Mat4::perspective_infinite_reverse_rh(1., 16. / 9., 5.);
        assert!(block(&probe, projection, UVec2::new(1280, 720)).is_none());
    }

    #[test]
    fn the_block_sits_at_the_lights_screen_point() {
        let probe = Probe {
            at: [0., 0., 1000.],
            half: [16., 4.],
        };
        let projection = Mat4::perspective_infinite_reverse_rh(1., 16. / 9., 5.);
        let (rect, light) = block(&probe, projection, UVec2::new(1280, 720)).unwrap();
        assert_eq!(rect, IRect::new(624, 356, 656, 364));
        assert!((light - 5. / 1000.).abs() < 1e-6);
    }
}
