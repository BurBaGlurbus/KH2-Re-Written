//! One entity and one mesh for each effect batch, kept between ticks. A batch
//! that spawned its own entity and mesh every tick cost a spawn and despawn, a
//! mesh allocated, uploaded and freed, and a pipeline looked up, for what is a
//! new set of vertices. Here only the vertices change (`mesh_upload`); a batch
//! not drawn this tick is hidden, and leaves after a while unused.
use crate::room;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use std::collections::HashMap;
use std::hash::Hash;

/// The smallest vertex room a batch's mesh has.
const MIN_ROOM: usize = 48;
/// Drawn ticks a hidden batch keeps its entity.
const IDLE_TICKS: u32 = 600;

/// Marks a batch's entity.
#[derive(Component)]
pub struct EffectMesh;

/// A batch's triangles for this tick.
pub struct Vertices {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
}

impl Vertices {
    /// The mesh's vertices keep their number while a batch grows and shrinks:
    /// the unused ones repeat the first position, so their triangles have no
    /// area and the bounds stay the batch's.
    fn fill(&mut self, room: usize) {
        let first = self.positions.first().copied().unwrap_or_default();
        self.positions.resize(room, first);
        self.uvs.resize(room, [0.; 2]);
        self.colors.resize(room, [0.; 4]);
    }
}

fn room_for(vertices: usize) -> usize {
    vertices.next_power_of_two().max(MIN_ROOM)
}

struct Slot {
    entity: Entity,
    mesh: Handle<Mesh>,
    room: usize,
    shown: bool,
    idle: u32,
}

/// The batches' entities, by what makes a batch.
pub struct EffectMeshes<K> {
    slots: HashMap<K, Slot>,
    /// Entities whose key went away between ticks.
    left: Vec<Entity>,
    shown: usize,
}

impl<K> Default for EffectMeshes<K> {
    fn default() -> Self {
        EffectMeshes {
            slots: HashMap::new(),
            left: Vec::new(),
            shown: 0,
        }
    }
}

impl<K: Hash + Eq> EffectMeshes<K> {
    /// How many batches this tick has drawn.
    pub fn shown(&self) -> usize {
        self.shown
    }

    /// Puts the vertices on `key`'s entity; without one, hands them back for
    /// `spawn`.
    pub fn show(
        &mut self,
        key: &K,
        mut vertices: Vertices,
        meshes: &mut Assets<Mesh>,
        visibility: &mut Query<&mut Visibility, With<EffectMesh>>,
    ) -> Option<Vertices> {
        let Some(slot) = self.slots.get_mut(key) else {
            return Some(vertices);
        };
        let Ok(mut seen) = visibility.get_mut(slot.entity) else {
            // Its scene went away.
            self.slots.remove(key);
            return Some(vertices);
        };
        seen.set_if_neq(Visibility::Inherited);
        slot.shown = true;
        slot.idle = 0;
        self.shown += 1;
        if vertices.positions.len() > slot.room {
            slot.room = room_for(vertices.positions.len());
            vertices.fill(slot.room);
            if let Some(mut mesh) = meshes.get_mut(&slot.mesh) {
                *mesh = room::mesh(vertices.positions, vertices.uvs, vertices.colors);
            }
            return None;
        }
        vertices.fill(slot.room);
        if crate::mesh_upload::active()
            && let Some(mesh) = meshes.get_mut_untracked(&slot.mesh)
        {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_POSITION,
                VertexAttributeValues::Float32x3(vertices.positions),
            );
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_UV_0,
                VertexAttributeValues::Float32x2(vertices.uvs),
            );
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_COLOR,
                VertexAttributeValues::Float32x4(vertices.colors),
            );
            crate::mesh_upload::changed(slot.mesh.id());
        } else if let Some(mut mesh) = meshes.get_mut(&slot.mesh) {
            *mesh = room::mesh(vertices.positions, vertices.uvs, vertices.colors);
        }
        None
    }

    /// `key`'s first batch: its entity and mesh, the material made once.
    pub fn spawn<M: Material>(
        &mut self,
        key: K,
        mut vertices: Vertices,
        material: Handle<M>,
        parent: Option<Entity>,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
    ) {
        let room = room_for(vertices.positions.len());
        vertices.fill(room);
        let mesh = meshes.add(room::mesh(
            vertices.positions,
            vertices.uvs,
            vertices.colors,
        ));
        let mut e = commands.spawn((
            EffectMesh,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::default(),
        ));
        if let Some(parent) = parent {
            e.insert(ChildOf(parent));
        }
        let entity = e.id();
        self.slots.insert(
            key,
            Slot {
                entity,
                mesh,
                room,
                shown: true,
                idle: 0,
            },
        );
        self.shown += 1;
    }

    /// After a tick's batches: hides the ones it did not draw.
    pub fn finish(
        &mut self,
        commands: &mut Commands,
        visibility: &mut Query<&mut Visibility, With<EffectMesh>>,
    ) {
        for entity in self.left.drain(..) {
            if visibility.contains(entity) {
                commands.entity(entity).despawn();
            }
        }
        self.slots.retain(|_, slot| {
            if std::mem::take(&mut slot.shown) {
                return true;
            }
            let Ok(mut seen) = visibility.get_mut(slot.entity) else {
                return false;
            };
            seen.set_if_neq(Visibility::Hidden);
            slot.idle += 1;
            if slot.idle < IDLE_TICKS {
                return true;
            }
            commands.entity(slot.entity).despawn();
            false
        });
    }

    /// A new tick's count starts.
    pub fn begin(&mut self) {
        self.shown = 0;
    }

    /// Lets go of the batches `keep` refuses (their entities leave next tick).
    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) {
        let left = &mut self.left;
        self.slots.retain(|key, slot| {
            let kept = keep(key);
            if !kept {
                left.push(slot.entity);
            }
            kept
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;

    fn triangle(x: f32) -> Vertices {
        Vertices {
            positions: vec![[x, 0., 0.], [x, 1., 0.], [x, 0., 1.]],
            uvs: vec![[0.5; 2]; 3],
            colors: vec![[1.; 4]; 3],
        }
    }

    type Parts<'w, 's> = (
        Commands<'w, 's>,
        Query<'w, 's, &'static mut Visibility, With<EffectMesh>>,
    );

    #[test]
    fn a_batch_keeps_its_entity_and_mesh_and_hides_when_not_drawn() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        let mut pool = EffectMeshes::<u8>::default();
        let material = Handle::<StandardMaterial>::default();
        let mut state = SystemState::<Parts>::new(&mut world);
        let mut meshes = world.remove_resource::<Assets<Mesh>>().unwrap();

        // The first tick spawns it.
        let (mut commands, mut seen) = state.get_mut(&mut world).unwrap();
        pool.begin();
        let v = pool.show(&1, triangle(2.), &mut meshes, &mut seen).unwrap();
        pool.spawn(1, v, material, None, &mut commands, &mut meshes);
        pool.finish(&mut commands, &mut seen);
        state.apply(&mut world);
        assert_eq!(pool.shown(), 1);
        assert_eq!(meshes.len(), 1);
        let entity = pool.slots[&1].entity;

        // The next draws on it, with no new mesh.
        let (mut commands, mut seen) = state.get_mut(&mut world).unwrap();
        pool.begin();
        assert!(
            pool.show(&1, triangle(3.), &mut meshes, &mut seen)
                .is_none()
        );
        pool.finish(&mut commands, &mut seen);
        state.apply(&mut world);
        assert_eq!(meshes.len(), 1);
        assert_eq!(pool.slots[&1].entity, entity);
        assert_eq!(
            *world.get::<Visibility>(entity).unwrap(),
            Visibility::Inherited
        );

        // A tick without it hides it.
        let (mut commands, mut seen) = state.get_mut(&mut world).unwrap();
        pool.begin();
        pool.finish(&mut commands, &mut seen);
        state.apply(&mut world);
        assert_eq!(pool.shown(), 0);
        assert_eq!(
            *world.get::<Visibility>(entity).unwrap(),
            Visibility::Hidden
        );

        // Long unused, it leaves.
        for _ in 0..IDLE_TICKS {
            let (mut commands, mut seen) = state.get_mut(&mut world).unwrap();
            pool.begin();
            pool.finish(&mut commands, &mut seen);
            state.apply(&mut world);
        }
        assert!(pool.slots.is_empty());
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn unused_vertices_have_no_area_and_stay_on_the_batch() {
        let mut v = triangle(2.);
        v.fill(room_for(3));
        assert_eq!(v.positions.len(), MIN_ROOM);
        assert_eq!(v.uvs.len(), MIN_ROOM);
        assert_eq!(v.colors.len(), MIN_ROOM);
        assert!(v.positions[3..].iter().all(|p| *p == [2., 0., 0.]));
    }

    #[test]
    fn room_grows_in_powers_of_two() {
        assert_eq!(room_for(1), MIN_ROOM);
        assert_eq!(room_for(49), 64);
        assert_eq!(room_for(1024), 1024);
    }
}
