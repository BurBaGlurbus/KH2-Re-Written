//! Partial texture updates for CPU-drawn images (the field HUD's canvas):
//! only the rectangles that changed are written into the image's GPU
//! texture. Marking the whole image modified instead copies all of it for
//! the render world and uploads all of it again.
//!
//! The image asset itself is left as it was created; writes are handed to
//! the render world at extraction (so a frame shows exactly that frame's
//! writes) and applied in order once the image's texture exists.
use bevy::prelude::*;
use bevy::render::{
    ExtractSchedule, MainWorld, Render, RenderApp, RenderSystems,
    render_asset::RenderAssets,
    render_resource::{
        Extent3d, Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect,
    },
    renderer::RenderQueue,
    texture::GpuImage,
};

/// RGBA8 texels for the rectangle `[x, y, width, height]` of `image`, rows
/// top to bottom.
pub struct TexelWrite {
    pub image: AssetId<Image>,
    pub rect: [u32; 4],
    pub rgba: Vec<u8>,
}

/// This frame's writes, in order.
#[derive(Resource, Default)]
pub struct TexelWrites(pub Vec<TexelWrite>);

/// Writes taken at extraction and not yet applied.
#[derive(Resource, Default)]
struct Pending(Vec<TexelWrite>);

pub struct TexelUploadPlugin;

impl Plugin for TexelUploadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TexelWrites>();
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<Pending>()
                .add_systems(ExtractSchedule, take)
                .add_systems(Render, write.in_set(RenderSystems::PrepareResources));
        }
    }
}

fn take(mut main: ResMut<MainWorld>, mut pending: ResMut<Pending>) {
    if let Some(mut writes) = main.get_resource_mut::<TexelWrites>()
        && !writes.0.is_empty()
    {
        pending.0.append(&mut writes.0);
    }
}

fn write(
    mut pending: ResMut<Pending>,
    images: Res<RenderAssets<GpuImage>>,
    queue: Res<RenderQueue>,
) {
    // An image without a texture yet keeps its writes, in order, for a
    // later frame.
    let mut waiting: Vec<AssetId<Image>> = Vec::new();
    let mut kept = Vec::new();
    for w in std::mem::take(&mut pending.0) {
        let gpu = images.get(w.image).filter(|_| !waiting.contains(&w.image));
        let Some(gpu) = gpu else {
            waiting.push(w.image);
            kept.push(w);
            continue;
        };
        let [x, y, width, height] = w.rect;
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &gpu.texture,
                mip_level: 0,
                origin: Origin3d { x, y, z: 0 },
                aspect: TextureAspect::All,
            },
            &w.rgba,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
    pending.0 = kept;
}
