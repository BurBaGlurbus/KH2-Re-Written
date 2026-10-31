//! Skinned vertices reach the GPU without touching the mesh asset's tracking.
//! A mesh marked modified is cloned for the render world, freed and allocated
//! again in its buffer, rebuilt and its pipeline looked up, and the characters'
//! meshes change every tick. Here only its vertex bytes go, written over its
//! place in the buffer: the allocation, the indices and the pipeline stay.
//!
//! `room::set_positions` hands a mesh over; a mesh that changes any other way
//! takes Bevy's own path, as does everything when the plugin is absent.
use bevy::camera::primitives::{Aabb, MeshAabb};
use bevy::camera::visibility::{NoAutoAabb, NoFrustumCulling, VisibilitySystems};
use bevy::prelude::*;
use bevy::render::{
    ExtractSchedule, MainWorld, Render, RenderApp, RenderSystems, mesh::allocator::MeshAllocator,
    renderer::RenderQueue,
};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The plugin is in this process's app.
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// The meshes `room::set_positions` rewrote since the last `collect`. (Global
/// so its many callers need no new parameter; a process has one app.)
static CHANGED: Mutex<Vec<AssetId<Mesh>>> = Mutex::new(Vec::new());

/// Whether meshes are uploaded here.
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// `id`'s positions were rewritten without marking it modified.
pub fn changed(id: AssetId<Mesh>) {
    if let Ok(mut list) = CHANGED.lock() {
        list.push(id);
    }
}

struct Upload {
    mesh: AssetId<Mesh>,
    vertex_size: u64,
    bytes: Vec<u8>,
}

/// This frame's vertex bytes.
#[derive(Resource, Default)]
struct Uploads(Vec<Upload>);

pub struct MeshUploadPlugin;

impl Plugin for MeshUploadPlugin {
    fn build(&self, app: &mut App) {
        ACTIVE.store(true, Ordering::Relaxed);
        app.init_resource::<Uploads>().add_systems(
            PostUpdate,
            collect.in_set(VisibilitySystems::CalculateBounds),
        );
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<Uploads>()
                .add_systems(ExtractSchedule, take)
                .add_systems(Render, write.in_set(RenderSystems::PrepareResources));
        }
    }
}

/// The meshes whose bounds follow their vertices, as Bevy's own pass picks them.
type Bounds<'w, 's> = Query<
    'w,
    's,
    (&'static Mesh3d, &'static mut Aabb),
    (Without<NoFrustumCulling>, Without<NoAutoAabb>),
>;

/// Packs the rewritten meshes' vertices and refreshes their bounds, as a
/// modified mesh's would be.
fn collect(meshes: Res<Assets<Mesh>>, mut uploads: ResMut<Uploads>, mut bounds: Bounds) {
    let changed = CHANGED
        .lock()
        .map(|mut list| std::mem::take(&mut *list))
        .unwrap_or_default();
    if changed.is_empty() {
        return;
    }
    let mut seen = HashSet::with_capacity(changed.len());
    let mut aabbs = HashMap::with_capacity(changed.len());
    for id in changed {
        if !seen.insert(id) {
            continue;
        }
        let Some(mesh) = meshes.get(id) else {
            continue;
        };
        uploads.0.push(Upload {
            mesh: id,
            vertex_size: mesh.get_vertex_size(),
            bytes: mesh.create_packed_vertex_buffer_data(),
        });
        if let Some(aabb) = mesh.compute_aabb() {
            aabbs.insert(id, aabb);
        }
    }
    for (mesh, mut aabb) in &mut bounds {
        if let Some(new) = aabbs.get(&mesh.id()) {
            *aabb = *new;
        }
    }
}

fn take(mut main: ResMut<MainWorld>, mut uploads: ResMut<Uploads>) {
    if let Some(mut from) = main.get_resource_mut::<Uploads>()
        && !from.0.is_empty()
    {
        uploads.0.append(&mut from.0);
    }
}

/// Writes after the allocator has placed this frame's new meshes. A mesh not
/// placed yet has no use for the bytes: its first copy, made at the same
/// extraction, holds them.
fn write(mut uploads: ResMut<Uploads>, allocator: Res<MeshAllocator>, queue: Res<RenderQueue>) {
    for mut upload in uploads.0.drain(..) {
        let Some(slice) = allocator.mesh_vertex_slice(&upload.mesh) else {
            continue;
        };
        // (Buffer writes come in multiples of four bytes.)
        let padded = (upload.bytes.len() as u64).next_multiple_of(4) as usize;
        upload.bytes.resize(padded, 0);
        queue.write_buffer(
            slice.buffer,
            u64::from(slice.range.start) * upload.vertex_size,
            &upload.bytes,
        );
    }
}
