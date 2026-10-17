//! A Keyblade swing that meets the map: the attack volume's environment
//! test (FUN_14040f610 → FUN_1403d0bb0), the spark and clank it plays
//! (FUN_1403d1960) and the recoil Sora takes (the owner callback
//! FUN_1403d63e0 → FUN_1403d4650, `Player::wall_recoil`).
//!
//! Each frame a weapon volume that is not yet spent tests the segment from
//! its first sphere's previous position to its current one against the map's
//! polygons with attribute bit 7 (`collision::World::attack_ray`, the class 4
//! ray of FUN_140171090). It runs only for an ATKP row with flags bit 0 (+0x12)
//! set, never again once the volume has met the map or hit an actor, and only
//! from the volume's second frame (it needs a previous position). A row with a
//! reach (+0x28) that missed tests a second, vertical segment of that length
//! below the sphere.
//!
//! A hit starts the Keyblade's own hit element whose category holds the ATKP
//! hit effect's bit and the surface's: 0x100 for surface nibble 1, else 2,
//! turned along the polygon's normal; plays the weapon's clank (sound 500010,
//! the equipped Keyblade's bank); and recoils Sora.
//!
//! [I] The volume's flag bit 8 (a penetrating volume, set only by the spell
//! volumes' routines) is taken as clear for the Keyblade. Dynamic collision
//! objects (`FUN_140163fc0`'s list) do not exist in the supported rooms.
use super::*;
use std::collections::HashSet;

/// The weapon's clank on the map (`0x7a12a` = 500010, FUN_1403dadf0 swaps the
/// bank for the Keyblade's own, 500 here).
const CLANK_SOUND: u32 = 500_010;

/// What the environment test reads from an ATKP row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Row {
    /// +0x12 bit 0: the volume is stopped by the map.
    env: bool,
    /// +0x28: the downward reach of the second segment.
    reach: u8,
    /// +0x0b: the hit effect number (0 = none).
    effect: u8,
}

/// The test's state: the Keyblades' blade spheres, the ATKP table and what
/// each live volume remembers.
pub struct WallHit {
    /// Each hand's Keyblade attack spheres by collision group.
    pub blades: [character_kit::Blade; 2],
    sstm: actor_physics::Sstm,
    atkp: Vec<u8>,
    rows: HashMap<u16, Option<Row>>,
    /// The first sphere's previous position per volume instance
    /// (volume +0xd0).
    previous: HashMap<u64, Vec3>,
    /// Volumes that already met the map (flags +8 bits 3 and 4) or hit an
    /// actor (bit 1): the test skips them.
    done: HashSet<u64>,
}

/// A contact: where it was, and the effect's turn from the polygon normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// Scene coordinates.
    pub point: Vec3,
    /// FUN_1403b7710: the normal's pitch and yaw in the engine's space.
    pub pitch: f32,
    pub yaw: f32,
    pub material: u32,
}

impl WallHit {
    pub fn new(sstm: actor_physics::Sstm, atkp: Vec<u8>, blade: character_kit::Blade) -> Self {
        WallHit {
            blades: [blade.clone(), blade],
            sstm,
            atkp,
            rows: HashMap::new(),
            previous: HashMap::new(),
            done: HashSet::new(),
        }
    }

    /// The volume hit an actor: it no longer tests the map (flags bit 1).
    pub fn struck(&mut self, instance: u64) {
        self.done.insert(instance);
    }

    fn row(&mut self, id: u16) -> Option<Row> {
        let atkp = &self.atkp;
        *self.rows.entry(id).or_insert_with(|| {
            let a = data::battle_tables::attack(atkp, id).ok()?;
            Some(Row {
                env: a.flags & 1 != 0,
                reach: a.env_reach,
                effect: a.hit_effect,
            })
        })
    }

    /// FUN_1403d0bb0 for one volume this frame: `previous` is its first
    /// sphere's position last frame, `current` this frame's.
    fn test(
        &mut self,
        world: &RoomWorld,
        instance: u64,
        row: Row,
        previous: Option<Vec3>,
        current: Vec3,
    ) -> Option<Contact> {
        let previous = previous?;
        if !row.env || self.done.contains(&instance) {
            return None;
        }
        let hit = world.attack_hit(previous, current).or_else(|| {
            // The second segment: straight down by the row's reach.
            (row.reach != 0)
                .then(|| world.attack_hit(current, current - Vec3::Y * row.reach as f32))
                .flatten()
        })?;
        self.done.insert(instance);
        // FUN_1403b7710: the pitch and yaw of the polygon's normal.
        let n = hit.normal;
        Some(Contact {
            point: Vec3::from_array(hit.point),
            pitch: (-n[1]).atan2(n[0].hypot(n[2])),
            yaw: n[0].atan2(n[2]),
            material: hit.material,
        })
    }
}

/// The effect category of a map hit: the hit effect's bit above bit 32 and
/// the surface class (FUN_1403d1960: material 1 is 0x100, any other 2).
pub fn category(effect: u8, material: u32) -> u64 {
    (1u64 << (32 + effect as u64 - 1)) | if material == 1 { 0x100 } else { 2 }
}

/// One frame: Sora's Keyblade volumes test the map, a hit plays and recoils.
pub fn tick(game: &mut Game, tick: u64) {
    if game.player.dead() {
        return;
    }
    let Some(&clip) = game.poses.get(&(game.player.action, game.player.armed)) else {
        return;
    };
    let Some(cp) = game.clip_poses.get(clip) else {
        return;
    };
    let frame = game.player.motion_frame(&game.clips) as usize;
    let at = Vec3::from_array(game.player.position);
    let turn = Quat::from_rotation_y(game.player.yaw);
    let mut found = None;
    let mut seen = Vec::new();
    for (hand, (blade, weapon, hits)) in [
        (&cp.blade, &cp.weapon, &cp.hits),
        (&cp.blade2, &cp.weapon2, &cp.hits2),
    ]
    .into_iter()
    .enumerate()
    {
        if blade.is_empty() || weapon.is_empty() {
            continue;
        }
        let f = frame.min(blade.len() - 1);
        let to_model = weapon[f.min(weapon.len() - 1)] * blade[f];
        for (w, &(start, end, atkp, group)) in hits.iter().enumerate() {
            if (frame as u32) < start || frame as u32 > end {
                continue;
            }
            let Some(&(offset, _)) = game.wall.blades[hand].get(&group).and_then(|s| s.first())
            else {
                continue;
            };
            let current = at + turn * to_model.transform_point3(offset);
            let instance = (game.action_started << 7) | (hand as u64) << 5 | w as u64;
            seen.push(instance);
            let previous = game.wall.previous.insert(instance, current);
            if found.is_some() {
                continue;
            }
            let Some(row) = game.wall.row(atkp) else {
                continue;
            };
            if let Some(contact) = game
                .wall
                .test(&game.world, instance, row, previous, current)
            {
                found = Some((contact, row));
            }
        }
    }
    game.wall.previous.retain(|k, _| seen.contains(k));
    game.wall.done.retain(|k| seen.contains(k));
    let Some((contact, row)) = found else {
        return;
    };
    if row.effect != 0 {
        game.effect_queue.push(EffectRequest::Wall {
            effect: row.effect,
            material: contact.material,
            contact: contact.point,
            pitch: contact.pitch,
            yaw: contact.yaw,
        });
    }
    game.sound_queue.push((CLANK_SOUND, "wall_hit"));
    let recoil = game.player.wall_recoil(&game.wall.sstm, &game.clips);
    if recoil.is_some() {
        game.action_started = tick;
    }
    game.event(serde_json::json!({"event":"wall_hit","tick":tick,
        "position":contact.point.to_array(),"material":contact.material,
        "effect":row.effect,"recoil":recoil.map(|a| format!("{a:?}"))}));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_category_names_the_hit_effect_and_the_surface() {
        // Hit effect 1 on surface 1: bit 32 and 0x100; the Keyblade's
        // element 3 (category 0x300000102) matches effects 1 and 2 on both.
        assert_eq!(category(1, 1), 0x1_0000_0100);
        assert_eq!(category(2, 3), 0x2_0000_0002);
        let element = 0x3_0000_0102u64;
        for (effect, material) in [(1, 1), (1, 2), (2, 1), (2, 7)] {
            let mask = category(effect, material);
            assert_eq!(element & mask, mask);
        }
        assert_ne!(element & category(3, 1), category(3, 1));
    }

    fn walls() -> WallHit {
        WallHit::new(
            actor_physics::Sstm::read(&[0; 0x3c]).unwrap(),
            Vec::new(),
            character_kit::Blade::new(),
        )
    }

    #[test]
    fn a_volume_that_hit_an_actor_stays_out_of_the_test() {
        let mut w = walls();
        w.struck(7);
        assert!(w.done.contains(&7));
        let row = Row {
            env: true,
            reach: 0,
            effect: 1,
        };
        // Without a previous position, or after the volume was spent, the
        // test never reaches the map.
        let world = RoomWorld::Triangles(TriangleWorld::new([], 0.5));
        assert!(w.test(&world, 7, row, Some(Vec3::ZERO), Vec3::X).is_none());
        assert!(w.test(&world, 8, row, None, Vec3::X).is_none());
    }
}
