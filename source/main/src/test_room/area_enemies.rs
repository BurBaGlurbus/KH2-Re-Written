//! `KH2RW_AREA_ENEMIES=1`: the current area's ordinary enemies (objentry
//! types 4 and 21 in the spawn sets its programs name) with their AI, each
//! with its spawn record's serial, so the scripts meet the map's ledges and
//! find the partners they look up. Entities of spawn types 0, 1 and 3 stand
//! at their places when the area is ready; the groups of spawn type 2 come
//! as the game brings them, when Sora stands in their point's zone
//! (FUN_1403fe320 case 1 -> FUN_1403fe6f0, `::area::Area::group_spawn`).
//! Spawn arguments 1 and 2 place an entity at a free point around Sora or
//! within its radius (FUN_1403f3fa0, or FUN_1403f4080 for an objentry with
//! +0x48 bit 3). [I] The ring search draws from this module's own engine
//! generator; the room holds 16 actors at most.
use super::*;

/// How long the tour stays in one group point's zone.
const TOUR_ZONE_TICKS: u64 = 450;

#[derive(Resource, Default)]
pub struct AreaEnemies {
    /// The area (world, area) whose standing enemies were placed.
    placed: Option<(u8, u8)>,
    /// DAT_142ae5e60: the serials of the objects made in this area.
    listed: std::collections::HashSet<u16>,
    /// Actors whose AI was already asked for.
    enabled: std::collections::HashSet<u64>,
    /// The group zone the tour put Sora in last, and when.
    tour_zone: usize,
    tour_moved: Option<u64>,
    /// The free-point search's generator and lap counter (DAT_142ae5d78).
    rng: enemy::world::Lcg,
    ring_counter: u32,
}

/// The map as the free-point search sees it (as the script brain's
/// `line_hit_polygon`: mask bit 16 takes the attack ray).
struct MapEnv<'a> {
    room: &'a RoomWorld,
    player: [f32; 3],
}

struct NoClips;
impl enemy::env::Clips for NoClips {
    fn length(&self, _: usize) -> u32 {
        0
    }
    fn loop_start(&self, _: usize) -> u32 {
        0
    }
    fn root_delta(&self, _: usize, _: u32) -> [f32; 3] {
        [0.; 3]
    }
    fn events(&self, _: usize) -> Option<&enemy::motion::MotionEvents> {
        None
    }
}

impl enemy::env::Env for MapEnv<'_> {
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        self.room.floor(x, z, top)
    }
    fn clips(&self, _: u32) -> &dyn enemy::env::Clips {
        &NoClips
    }
    fn player_position(&self) -> [f32; 3] {
        self.player
    }
    fn target_position(&self, _: u32) -> Option<[f32; 3]> {
        None
    }
    fn line_hit_polygon(
        &self,
        from: [f32; 3],
        to: [f32; 3],
        class: u32,
    ) -> Option<([f32; 3], i32)> {
        match self.room {
            RoomWorld::Map(map) if class >> 16 & 1 != 0 => {
                map.attack_hit(from, to).map(|h| (h.point, -1))
            }
            RoomWorld::Map(map) => map
                .line(from, to, class)
                .map(|(p, poly)| (p, i32::from(poly))),
            RoomWorld::Triangles(_) => None,
        }
    }
}

/// FUN_1403fe6f0's place for spawn arguments 1 and 2, in the scene: around
/// Sora (the centre 200 above him, 150 to 450 out; in the air 100 below to
/// 100 above the ring) or within the record's radius of its own place.
fn free_point(
    state: &mut AreaEnemies,
    env: &MapEnv<'_>,
    flying: bool,
    place: ::area::Place,
    own: Vec3,
) -> Vec3 {
    use enemy::traps::random_position::{air_point, ground_point};
    let (center, near, far, low, high, class) = match place {
        ::area::Place::Own => return own,
        ::area::Place::NearPlayer => {
            let p = env.player;
            (
                [p[0], p[1] + 200., p[2], 1.],
                150.,
                450.,
                -100.,
                100.,
                0x1_0000,
            )
        }
        ::area::Place::NearOwn(r) => ([own.x, own.y, own.z, 1.], 0., r, 0., 0., 0),
    };
    let v = if flying {
        air_point(
            env,
            &mut state.rng,
            &mut state.ring_counter,
            center,
            near,
            far,
            low,
            high,
            class | 2,
        )
    } else {
        ground_point(
            env,
            &mut state.rng,
            &mut state.ring_counter,
            center,
            near,
            far,
            class | 1,
        )
    };
    Vec3::new(v[0], v[1], v[2])
}

pub fn enabled() -> bool {
    std::env::var_os("KH2RW_AREA_ENEMIES").is_some()
}

/// The group point's view of the room's objects.
struct Census<'a> {
    gallery: &'a enemy_gallery::EnemyGallery,
    listed: &'a mut std::collections::HashSet<u16>,
    source: &'a AreaSource,
}

impl ::area::Census for Census<'_> {
    fn alive(&self, serial: u16) -> bool {
        self.gallery
            .actors
            .iter()
            .any(|a| a.spawn_serial == serial && a.battle.as_ref().is_none_or(|b| b.brain.alive()))
    }
    fn listed(&self, serial: u16) -> bool {
        self.listed.contains(&serial)
    }
    fn list(&mut self, object: u32, serial: u16) {
        list(self.listed, self.source, object, serial);
    }
    fn forget(&mut self, serial: u16) {
        self.listed.remove(&serial);
    }
    fn spawn_limit(&self, object: u32) -> u8 {
        self.source.object(object).map_or(0, |o| o.spawn_limit)
    }
}

/// FUN_1403fe590 / FUN_1403fe650: a made object of objentry type 4, 13, 21
/// or 23 (mask 0xa02010) goes on the list (FUN_1403f5be0).
fn list(
    listed: &mut std::collections::HashSet<u16>,
    source: &AreaSource,
    object: u32,
    serial: u16,
) {
    if source
        .object(object)
        .is_some_and(|o| o.object_type < 0x18 && 0xa0_2010u32 >> o.object_type & 1 != 0)
    {
        listed.insert(serial);
    }
}

/// The catalog entry of an ordinary enemy that can fight.
fn fighter(gallery: &enemy_gallery::EnemyGallery, object: u32) -> Option<usize> {
    gallery.catalog.entries.iter().position(|e| {
        e.object_id == object
            && enemy_battle::supported(e)
            && e.object_id != enemy_battle::SEPHIROTH
            && e.model_available
            && e.motions_available
    })
}

/// Summon a spawn record's entity: engine (Y down) to scene, and the yaw as
/// the boss placement turns it (sephiroth.rs).
fn place(
    gallery: &mut enemy_gallery::EnemyGallery,
    s: &::area::Spawn,
    at: Option<Vec3>,
) -> Option<serde_json::Value> {
    let entry = fighter(gallery, s.entity.object)?;
    if gallery.actors.len() >= enemy_gallery::MAX_ACTORS {
        return None;
    }
    let [x, y, z] = s.entity.position;
    let at = at.unwrap_or(Vec3::new(x, -y, -z));
    gallery.queue_spawn(
        entry,
        at,
        ::area::wrap(std::f32::consts::PI - s.yaw),
        s.entity.serial,
    );
    Some(
        serde_json::json!({"model":gallery.catalog.entries[entry].model_name,
        "serial":s.entity.serial,"set":format!("{:x}", s.set),
        "spawn_type":s.entity.spawn_type,"position":at.to_array()}),
    )
}

pub fn tick(
    mut game: ResMut<Game>,
    mut gallery: ResMut<enemy_gallery::EnemyGallery>,
    area: Option<ResMut<AreaRes>>,
    mut state: ResMut<AreaEnemies>,
) {
    if !enabled() {
        return;
    }
    let Some(mut area) = area else {
        return;
    };
    let tick = game.tick;
    let here = (area.current.world, area.current.area);
    if state.placed != Some(here) {
        if tick < 30 {
            return;
        }
        state.placed = Some(here);
        // FUN_1403a2170: the list starts empty with the area.
        state.listed.clear();
        // [Tour setup] Points that start disabled wait for a mission or event
        // signal the room does not send (hb25's 1000 Heartless); the tour
        // opens the group points itself.
        if game.enemy_battle_tour {
            for o in &mut area.current.state.objects {
                if o.point.kind == 2 {
                    o.flags &= !1;
                }
            }
        }
        let spawns = area.current.spawns.clone();
        for s in &spawns {
            list(
                &mut state.listed,
                &area.source,
                s.entity.object,
                s.entity.serial,
            );
        }
        let placed: Vec<_> = spawns
            .iter()
            .filter_map(|s| place(&mut gallery, s, None))
            .collect();
        game.event(
            serde_json::json!({"event":"area_enemies_placed","tick":tick,
            "world":here.0,"area":here.1,"enemies":placed}),
        );
    }
    // [Tour setup] The unattended tour walks no route: Sora is put in the
    // group points' zones, one after another, as if he had walked in.
    if game.enemy_battle_tour
        && tick >= 40
        && state.tour_moved.is_none_or(|t| tick >= t + TOUR_ZONE_TICKS)
    {
        state.tour_moved = Some(tick);
        let zones: Vec<_> = area
            .current
            .state
            .objects
            .iter()
            .filter(|o| o.point.kind == 2)
            .filter_map(|o| o.zones.first().map(|z| z.center))
            .collect();
        let zone = (!zones.is_empty()).then(|| zones[state.tour_zone % zones.len()]);
        state.tour_zone += 1;
        if let Some([x, y, z]) = zone {
            let to = [x, -y, -z];
            game.sora.player.position = to;
            game.event(
                serde_json::json!({"event":"area_enemies_sora_moved","tick":tick,
                "to":to,"authored_debug_placement":true}),
            );
        }
    }
    let groups = std::mem::take(&mut area.groups);
    let area = &mut *area;
    for object in groups {
        let made = {
            let mut census = Census {
                gallery: &gallery,
                listed: &mut state.listed,
                source: &area.source,
            };
            area.current.state.group_spawn(object, &mut census)
        };
        let env = MapEnv {
            room: &game.world,
            player: game.sora.player.position,
        };
        let mut placed = Vec::new();
        for g in &made {
            let [x, y, z] = g.spawn.entity.position;
            let flying = area
                .source
                .object(g.spawn.entity.object)
                .is_some_and(|o| o.flags >> 3 & 1 != 0);
            let at = free_point(&mut state, &env, flying, g.place, Vec3::new(x, -y, -z));
            placed.extend(place(&mut gallery, &g.spawn, Some(at)));
        }
        if !placed.is_empty() {
            let wave = area.current.state.objects[object].wave;
            game.event(serde_json::json!({"event":"area_group_spawn","tick":tick,
                "point":area.current.state.objects[object].point.id,"wave":wave,"enemies":placed}));
        }
    }
    // Every placed enemy gets its AI once its summon has been made.
    let ids: Vec<_> = gallery
        .actors
        .iter()
        .filter(|a| a.spawn_serial != 0 && a.battle.is_none() && !state.enabled.contains(&a.id))
        .map(|a| a.id)
        .collect();
    for id in ids {
        state.enabled.insert(id);
        gallery.select(id);
        gallery.queue_battle();
    }
}
