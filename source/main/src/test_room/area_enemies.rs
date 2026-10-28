//! `KH2RW_AREA_ENEMIES=1`: the current area's ordinary enemies (objentry
//! types 4 and 21 in the spawn sets its programs name) stand at their spawn
//! records' places with their AI on, each with its record's serial, so the
//! scripts meet the map's ledges and find the partners they look up.
//! [I] The game pops type-2 entities in through their points' zones
//! (FUN_1403fe320, not traced); here every one is placed once the area is
//! ready, up to the gallery's actor limit.
use super::*;

#[derive(Resource, Default)]
pub struct AreaEnemies {
    /// The area (world, area) whose enemies were placed.
    placed: Option<(u8, u8)>,
    /// The tick the placed actors get their AI.
    enable_at: Option<u64>,
}

pub fn enabled() -> bool {
    std::env::var_os("KH2RW_AREA_ENEMIES").is_some()
}

pub fn tick(
    mut game: ResMut<Game>,
    mut gallery: ResMut<enemy_gallery::EnemyGallery>,
    area: Option<Res<AreaRes>>,
    mut state: ResMut<AreaEnemies>,
) {
    if !enabled() {
        return;
    }
    let Some(area) = area else {
        return;
    };
    let tick = game.tick;
    let here = (area.current.world, area.current.area);
    if state.placed != Some(here) {
        if tick < 30 {
            return;
        }
        state.placed = Some(here);
        let catalog = &gallery.catalog.entries;
        let kinds = |id: u32| {
            catalog
                .iter()
                .find(|e| e.object_id == id)
                .map(|e| e.object_type)
        };
        let mut spawns = area.current.spawns.clone();
        spawns.extend(area.current.state.zone_spawns(kinds));
        let mut placed = Vec::new();
        for s in spawns {
            let Some(entry) = gallery.catalog.entries.iter().position(|e| {
                e.object_id == s.entity.object
                    && enemy_battle::supported(e)
                    && e.object_id != enemy_battle::SEPHIROTH
                    && e.model_available
                    && e.motions_available
            }) else {
                continue;
            };
            if placed.len() >= enemy_gallery::MAX_ACTORS {
                break;
            }
            // Engine (Y down) to scene, and the spawn's yaw as the boss
            // placement turns it (sephiroth.rs).
            let [x, y, z] = s.entity.position;
            let at = Vec3::new(x, -y, -z);
            let yaw = ::area::wrap(std::f32::consts::PI - s.yaw);
            gallery.queue_spawn(entry, at, yaw, s.entity.serial);
            placed.push(
                serde_json::json!({"model":gallery.catalog.entries[entry].model_name,
                "serial":s.entity.serial,"set":format!("{:x}", s.set),
                "spawn_type":s.entity.spawn_type,"position":at.to_array()}),
            );
        }
        state.enable_at = Some(tick + 2);
        game.event(
            serde_json::json!({"event":"area_enemies_placed","tick":tick,
            "world":here.0,"area":here.1,"enemies":placed}),
        );
        return;
    }
    if state.enable_at == Some(tick) {
        state.enable_at = None;
        let ids: Vec<_> = gallery
            .actors
            .iter()
            .filter(|a| a.battle.is_none())
            .map(|a| a.id)
            .collect();
        for id in ids {
            gallery.select(id);
            gallery.queue_battle();
        }
    }
}
