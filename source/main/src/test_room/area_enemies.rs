//! `KH2RW_AREA_ENEMIES=1`: the current area's ordinary enemies (objentry
//! types 4 and 21 in the spawn sets its programs name) with their AI, each
//! with its spawn record's serial, so the scripts meet the map's ledges and
//! find the partners they look up. Entities of spawn types 0, 1 and 3 stand
//! at their places when the area is ready; the groups of spawn type 2 come
//! as the game brings them, when Sora stands in their point's zone
//! (FUN_1403fe320 case 1 -> FUN_1403fe6f0, `::area::Area::group_spawn`).
//! [I] Places around the player or within a radius (spawn arguments 1 and
//! 2) are taken as the record's own; the room holds 16 actors at most.
use super::*;

#[derive(Resource, Default)]
pub struct AreaEnemies {
    /// The area (world, area) whose standing enemies were placed.
    placed: Option<(u8, u8)>,
    /// DAT_142ae5e60: serials whose objects were defeated.
    defeated: std::collections::HashSet<u16>,
    /// Actors whose AI was already asked for.
    enabled: std::collections::HashSet<u64>,
    /// The tour moved Sora into the first group's zone.
    tour_moved: bool,
}

pub fn enabled() -> bool {
    std::env::var_os("KH2RW_AREA_ENEMIES").is_some()
}

/// The group point's view of the room's objects.
struct Census<'a> {
    gallery: &'a enemy_gallery::EnemyGallery,
    defeated: &'a mut std::collections::HashSet<u16>,
    source: &'a AreaSource,
}

impl ::area::Census for Census<'_> {
    fn alive(&self, serial: u16) -> bool {
        self.gallery
            .actors
            .iter()
            .any(|a| a.spawn_serial == serial && a.battle.as_ref().is_none_or(|b| b.brain.alive()))
    }
    fn defeated(&self, serial: u16) -> bool {
        self.defeated.contains(&serial)
    }
    fn forget(&mut self, serial: u16) {
        self.defeated.remove(&serial);
    }
    fn spawn_limit(&self, object: u32) -> u8 {
        self.source.object(object).map_or(0, |o| o.spawn_limit)
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
) -> Option<serde_json::Value> {
    let entry = fighter(gallery, s.entity.object)?;
    if gallery.actors.len() >= enemy_gallery::MAX_ACTORS {
        return None;
    }
    let [x, y, z] = s.entity.position;
    let at = Vec3::new(x, -y, -z);
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
        let spawns = area.current.spawns.clone();
        let placed: Vec<_> = spawns
            .iter()
            .filter_map(|s| place(&mut gallery, s))
            .collect();
        game.event(
            serde_json::json!({"event":"area_enemies_placed","tick":tick,
            "world":here.0,"area":here.1,"enemies":placed}),
        );
    }
    // [Tour setup] The unattended tour walks no route: Sora is put in the
    // first group point's zone, as if he had walked in.
    if game.enemy_battle_tour && !state.tour_moved && tick >= 40 {
        state.tour_moved = true;
        let zone = area
            .current
            .state
            .objects
            .iter()
            .filter(|o| o.point.kind == 2)
            .find_map(|o| o.zones.first().map(|z| z.center));
        if let Some([x, y, z]) = zone {
            let to = [x, -y, -z];
            game.sora.player.position = to;
            game.event(
                serde_json::json!({"event":"area_enemies_sora_moved","tick":tick,
                "to":to,"authored_debug_placement":true}),
            );
        }
    }
    // The defeated go on the list (the serials a group does not make again).
    for a in &gallery.actors {
        if a.spawn_serial != 0 && a.battle.as_ref().is_some_and(|b| !b.brain.alive()) {
            state.defeated.insert(a.spawn_serial);
        }
    }
    let groups = std::mem::take(&mut area.groups);
    let area = &mut *area;
    for object in groups {
        let made = {
            let mut census = Census {
                gallery: &gallery,
                defeated: &mut state.defeated,
                source: &area.source,
            };
            area.current.state.group_spawn(object, &mut census)
        };
        let placed: Vec<_> = made
            .iter()
            .filter_map(|g| place(&mut gallery, &g.spawn))
            .collect();
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
