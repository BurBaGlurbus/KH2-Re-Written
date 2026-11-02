//! Skinned vertices reach the GPU without touching the mesh asset's tracking.
//! A mesh marked modified is cloned for the render world, freed and allocated
//! again in its buffer, rebuilt and its pipeline looked up, and the characters'
//! meshes change every tick. Here only its vertex bytes go, written over its
//! place in the buffer: the allocation, the indices and the pipeline stay.
//!
//! `room::set_positions` hands a mesh over; a mesh that changes any other way
//! takes Bevy's own path, as does everything when the plugin is absent.
//!
//! That path also refreshes what a transparent mesh is sorted by: the centre of
//! its bounds, kept on the render world's mesh and on its retained phase item.
//! `recentre` does that here, or effects overlapping one another change order.
use bevy::camera::primitives::{Aabb, MeshAabb};
use bevy::camera::visibility::{NoAutoAabb, NoFrustumCulling, VisibilitySystems};
use bevy::core_pipeline::core_3d::{Transparent3d, TransparentSortingInfo3d};
use bevy::mesh::VertexAttributeValues;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::{
    ExtractSchedule, MainWorld, Render, RenderApp, RenderSystems,
    mesh::{RenderMesh, allocator::MeshAllocator},
    render_asset::RenderAssets,
    render_phase::ViewSortedRenderPhases,
    render_resource::{BufferSize, CommandEncoderDescriptor},
    renderer::{RenderDevice, RenderQueue},
    sync_world::MainEntity,
};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use wgpu::util::StagingBelt;

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

/// A mesh's vertices as they stand in its buffer.
enum Packed {
    /// A position, UV and colour each.
    Triangles(Vec<[f32; 9]>),
    Other {
        vertex_size: u64,
        bytes: Vec<u8>,
    },
}

struct Upload {
    mesh: AssetId<Mesh>,
    packed: Packed,
}

/// This frame's vertex bytes.
#[derive(Resource, Default)]
struct Uploads(Vec<Upload>);

/// This frame's new bounds centres: by mesh in its own space, and by entity in
/// the world (`Moved` until the transforms are final).
#[derive(Resource, Default)]
struct Centres {
    meshes: Vec<(AssetId<Mesh>, Vec3)>,
    moved: Vec<(Entity, Vec3)>,
    world: Vec<(Entity, Vec3)>,
}

/// A fixed place in the transparent sort: lower draws first, whatever the
/// mesh's distance. The map's alpha chunks draw in the executable's batch
/// order, before the objects' alpha (FUN_14019f170's phases 8 and 0xb).
#[derive(Component, Clone, Copy)]
pub struct SortOrder(pub f32);

/// The ordered entities, whole, when they change.
#[derive(Resource, Default)]
struct Orders {
    list: Vec<(Entity, f32)>,
    changed: bool,
}

pub struct MeshUploadPlugin;

impl Plugin for MeshUploadPlugin {
    fn build(&self, app: &mut App) {
        ACTIVE.store(true, Ordering::Relaxed);
        app.init_resource::<Uploads>()
            .init_resource::<Centres>()
            .init_resource::<Orders>()
            .add_systems(
                PostUpdate,
                (
                    collect.in_set(VisibilitySystems::CalculateBounds),
                    to_world.after(TransformSystems::Propagate),
                    list_orders,
                ),
            );
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<Uploads>()
                .init_resource::<Centres>()
                .init_resource::<Orders>()
                .add_systems(ExtractSchedule, take)
                .add_systems(
                    Render,
                    (
                        recentre.in_set(RenderSystems::QueueSweep),
                        write.in_set(RenderSystems::PrepareResources),
                    ),
                );
        }
    }
}

/// The meshes whose bounds follow their vertices, as Bevy's own pass picks them.
type Bounds<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Mesh3d, &'static mut Aabb),
    (Without<NoFrustumCulling>, Without<NoAutoAabb>),
>;

/// Packs the rewritten meshes' vertices and refreshes their bounds, as a
/// modified mesh's would be.
fn collect(
    meshes: Res<Assets<Mesh>>,
    mut uploads: ResMut<Uploads>,
    mut centres: ResMut<Centres>,
    mut bounds: Bounds,
) {
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
        let (packed, aabb) = match pack_triangles(mesh) {
            Some((vertices, aabb)) => (Packed::Triangles(vertices), Some(aabb)),
            None => (
                Packed::Other {
                    vertex_size: mesh.get_vertex_size(),
                    bytes: mesh.create_packed_vertex_buffer_data(),
                },
                mesh.compute_aabb(),
            ),
        };
        uploads.0.push(Upload { mesh: id, packed });
        if let Some(aabb) = aabb {
            aabbs.insert(id, aabb);
            centres.meshes.push((id, aabb.center.into()));
        }
    }
    for (entity, mesh, mut aabb) in &mut bounds {
        if let Some(new) = aabbs.get(&mesh.id()) {
            centres.moved.push((entity, new.center.into()));
            aabb.set_if_neq(*new);
        }
    }
}

/// The moved centres under the frame's final transforms.
fn to_world(mut centres: ResMut<Centres>, transforms: Query<&GlobalTransform>) {
    let centres = &mut *centres;
    centres.world.extend(
        centres
            .moved
            .drain(..)
            .filter_map(|(e, c)| Some((e, transforms.get(e).ok()?.transform_point(c)))),
    );
}

fn list_orders(
    mut orders: ResMut<Orders>,
    added: Query<(), Added<SortOrder>>,
    mut removed: RemovedComponents<SortOrder>,
    all: Query<(Entity, &SortOrder)>,
) {
    if added.is_empty() && removed.read().count() == 0 {
        return;
    }
    orders.list = all.iter().map(|(e, o)| (e, o.0)).collect();
    orders.changed = true;
}

/// What Bevy's path would have refreshed. Newly queued items read the render
/// mesh; ones kept from earlier frames hold their own copy of the world centre.
fn recentre(
    mut centres: ResMut<Centres>,
    orders: Res<Orders>,
    mut render_meshes: ResMut<RenderAssets<RenderMesh>>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
) {
    for phase in phases.values_mut() {
        for &(entity, place) in &orders.list {
            if let Some(item) = phase
                .items
                .get_mut(&(Entity::PLACEHOLDER, MainEntity::from(entity)))
                && let TransparentSortingInfo3d::Sorted {
                    mesh_center,
                    depth_bias,
                } = &mut item.sorting_info
            {
                *mesh_center = Vec3::ZERO;
                *depth_bias = place;
            }
        }
    }
    for (mesh, centre) in centres.meshes.drain(..) {
        if let Some(mesh) = render_meshes.get_mut(mesh) {
            mesh.aabb_center = centre;
        }
    }
    for (entity, centre) in centres.world.drain(..) {
        for phase in phases.values_mut() {
            if let Some(item) = phase
                .items
                .get_mut(&(Entity::PLACEHOLDER, MainEntity::from(entity)))
                && let TransparentSortingInfo3d::Sorted { mesh_center, .. } = &mut item.sorting_info
            {
                *mesh_center = centre;
            }
        }
    }
}

fn take(
    mut main: ResMut<MainWorld>,
    mut uploads: ResMut<Uploads>,
    mut centres: ResMut<Centres>,
    mut orders: ResMut<Orders>,
) {
    if let Some(mut from) = main.get_resource_mut::<Orders>()
        && from.changed
    {
        from.changed = false;
        orders.list = std::mem::take(&mut from.list);
    }
    if let Some(mut from) = main.get_resource_mut::<Uploads>()
        && !from.0.is_empty()
    {
        uploads.0.append(&mut from.0);
    }
    if let Some(mut from) = main.get_resource_mut::<Centres>() {
        centres.meshes.append(&mut from.meshes);
        centres.world.append(&mut from.world);
    }
}

/// A mesh with a position, UV and colour per vertex and nothing else, in one
/// pass: its vertices as `create_packed_vertex_buffer_data` lays them out, and
/// their bounds.
fn pack_triangles(mesh: &Mesh) -> Option<(Vec<[f32; 9]>, Aabb)> {
    let (
        3,
        Some(VertexAttributeValues::Float32x3(positions)),
        Some(VertexAttributeValues::Float32x2(uvs)),
        Some(VertexAttributeValues::Float32x4(colors)),
    ) = (
        mesh.attributes().count(),
        mesh.attribute(Mesh::ATTRIBUTE_POSITION),
        mesh.attribute(Mesh::ATTRIBUTE_UV_0),
        mesh.attribute(Mesh::ATTRIBUTE_COLOR),
    )
    else {
        return None;
    };
    if positions.is_empty() || uvs.len() != positions.len() || colors.len() != positions.len() {
        return None;
    }
    let mut min = positions[0];
    let mut max = positions[0];
    let mut out = Vec::with_capacity(positions.len());
    for ((p, u), c) in positions.iter().zip(uvs).zip(colors) {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
        out.push([p[0], p[1], p[2], u[0], u[1], c[0], c[1], c[2], c[3]]);
    }
    let aabb = Aabb::from_min_max(Vec3::from_array(min), Vec3::from_array(max));
    Some((out, aabb))
}

/// A frame's vertex bytes go in one copy from a staging buffer kept between
/// frames: `queue.write_buffer` makes a new staging buffer per mesh part, and
/// creating and destroying those cost more than the copies.
const STAGING_CHUNK: u64 = 1 << 20;

/// Writes after the allocator has placed this frame's new meshes. A mesh not
/// placed yet has no use for the bytes: its first copy, made at the same
/// extraction, holds them.
fn write(
    mut uploads: ResMut<Uploads>,
    allocator: Res<MeshAllocator>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut belt: Local<Option<StagingBelt>>,
) {
    if uploads.0.is_empty() {
        return;
    }
    let belt =
        belt.get_or_insert_with(|| StagingBelt::new(device.wgpu_device().clone(), STAGING_CHUNK));
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("mesh_upload"),
    });
    for upload in uploads.0.drain(..) {
        let Some(slice) = allocator.mesh_vertex_slice(&upload.mesh) else {
            continue;
        };
        let (bytes, vertex_size): (&[u8], u64) = match &upload.packed {
            Packed::Triangles(vertices) => (bytemuck::cast_slice(vertices), 36),
            Packed::Other { vertex_size, bytes } => (bytes, *vertex_size),
        };
        // (Buffer writes come in multiples of four bytes.)
        let padded = (bytes.len() as u64).next_multiple_of(4);
        let Some(size) = BufferSize::new(padded) else {
            continue;
        };
        let offset = u64::from(slice.range.start) * vertex_size;
        let mut view = belt.write_buffer(&mut encoder, slice.buffer, offset, size);
        view.slice(..bytes.len()).copy_from_slice(bytes);
        view.slice(bytes.len()..)
            .copy_from_slice(&[0; 4][..padded as usize - bytes.len()]);
    }
    belt.finish();
    queue.submit([encoder.finish()]);
    belt.recall();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangles_pack_as_the_engine_packs_them() {
        let mesh = crate::room::mesh(
            vec![[1., -2., 3.], [-4., 5., 6.], [7., 8., -9.]],
            vec![[0.25, 0.5], [0.75, 1.], [0., 0.125]],
            vec![[0.1, 0.2, 0.3, 0.4], [0.5, 0.6, 0.7, 0.8], [1., 0., 1., 0.]],
        );
        let (vertices, aabb) = pack_triangles(&mesh).unwrap();
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);
        assert_eq!(bytes, mesh.create_packed_vertex_buffer_data().as_slice());
        assert_eq!(mesh.get_vertex_size(), 36);
        assert_eq!(aabb, mesh.compute_aabb().unwrap());
    }

    #[test]
    fn a_mesh_with_other_attributes_takes_the_engine_s_path() {
        let mesh = crate::room::mesh(vec![[0.; 3]; 3], vec![[0.; 2]; 3], vec![[0.; 4]; 3])
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; 3]);
        assert!(pack_triangles(&mesh).is_none());
    }
}
