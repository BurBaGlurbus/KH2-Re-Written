//! The field's treasure chests: FUN_140402200 reads the opened bit when the
//! chest is made, the reaction search offers command 32 (reaction_command.rs),
//! and FUN_1403a8b10 case 0x20 → FUN_1404025c0 opens it: the chest plays its
//! opening motion, Sora the capture motion from the chest's slot 252, and on
//! that tick FUN_1403a1b60 gives the item and sets the bit. The small chest's
//! window shows 60 ticks later (dk::TREASURE_BOX_INFO).
use super::*;
use crate::chests::{Chest, ChestState};
use save::state;

/// Sora's captured motion in the chest's set (FUN_1404025c0).
const CAPTURE_SLOT: i32 = 252;
/// FUN_1404025c0 (DAT_140623bac): a small chest's and its capture's blend.
const SMALL_BLEND: f32 = 8.;
/// AZ10's range 22: the turn toward the target over the first 12 ticks, at 15
/// degrees a tick [I: the parameter's unit].
const TURN_TICKS: u32 = 12;
const TURN_RATE: f32 = 15. * std::f32::consts::PI / 180.;
/// The big chest's event: its frames run at half the tick rate (90 frames over
/// the 180-frame clip), and op 0x3b at frame 75 opens the window.
const EVENT_WINDOW_TICK: u32 = 150;
/// The effect owner of the chests' effects.
const CHEST_FX: u8 = 0xf5;

/// The unattended `--chest-tour`: Sora stands before the area's first closed
/// chest at tick 20 and presses Triangle from tick 60.
pub const TOUR_PRESS: u64 = 60;
pub const CAPTURES: [u64; 6] = [40, 90, 120, 165, 200, 260];
pub const END: u64 = 330;

fn opened(game: &Game, c: &Chest) -> bool {
    c.bit != 0 && game.save.base.u8_at(state::CHESTS + c.bit as usize / 8) >> (c.bit % 8) & 1 != 0
}

/// The turn toward the chest, as FUN_1403d4b90 steps a yaw.
fn turned(yaw: f32, target: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let d = (target - yaw + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI;
    yaw + d.clamp(-TURN_RATE, TURN_RATE)
}

/// FUN_1403a1520 and FUN_1403a1590: the kills and the reaction commands of
/// this tick count once story flag 0x2d0 is set.
/// The stock and levels the flag tail reads (`Progress::holdings`), the items a
/// flag took, and each area entered (FUN_1403baf20, FUN_1403a5330). A gain runs
/// its check at once, as FUN_1403c3fe0 and the level-ups do.
pub fn sync_holdings(
    mut game: ResMut<Game>,
    area: Option<ResMut<AreaRes>>,
    camp: Option<ResMut<CampRes>>,
    mut last: Local<Option<(u8, u8)>>,
) {
    let Some(mut area) = area else { return };
    let here = (area.current.world, area.current.area);
    let tick = game.tick;
    // The first area's op 0x0f works on the world's row as an arrival's does.
    if last.is_none()
        && area.current.members.record.is_some()
        && let Some(to) = game.save.party_table.get_mut(here.0 as usize)
    {
        *to = game_save::row_with_presets(*to, &area.current.members.presets);
    }
    let objects = area.current.members.objects.clone();
    party_room::keep_reserve(&mut game, &objects, here.0);
    party_room::keep_actors(&mut game, &objects, here.0);
    let mut camp = camp;
    let Game {
        items, drive, save, ..
    } = &mut *game;
    let stock = match (items.as_mut(), camp.as_mut()) {
        (Some(i), _) => Some(&mut i.stock),
        (None, Some(c)) => Some(&mut c.stock),
        _ => None,
    };
    let p = area.source.progress_mut();
    if *last != Some(here) {
        *last = Some(here);
        p.see_area(here.0, here.1);
        p.enter_stage_area(here.0, here.1);
    }
    let before = p.holdings.clone();
    let count = |item: u16| stock.as_ref().and_then(|s| s.get(&item).copied());
    for item in [0x17, 0x20, 0x57] {
        p.holdings.items.insert(item, count(item).unwrap_or(0));
    }
    p.holdings.byte_352a = save.base.u8_at(0x352a);
    p.holdings.summon_level = save.base.u8_at(0x3526);
    if let Some(d) = drive.as_ref() {
        for (to, from) in p.holdings.form_levels.iter_mut().zip(&d.forms.records) {
            *to = from.level;
        }
    }
    if p.holdings.count(0x20) > before.count(0x20) {
        p.check_torn_pages();
    }
    if [0x17, 0x57]
        .iter()
        .any(|&i| p.holdings.count(i) > before.count(i))
    {
        p.check_atlantica();
    }
    if p.holdings.form_levels != before.form_levels
        || p.holdings.summon_level != before.summon_level
    {
        p.check_levels();
    }
    let taken = std::mem::take(&mut p.taken);
    if !taken.is_empty() {
        if let Some(stock) = stock {
            for item in &taken {
                if let Some(n) = stock.get_mut(item) {
                    *n = n.saturating_sub(1);
                }
            }
        }
        game.event(serde_json::json!({"event":"flag_took_items","tick":tick,"items":taken}));
    }
}

pub fn count_journal(
    mut game: ResMut<Game>,
    area: Option<Res<AreaRes>>,
    camp: Option<ResMut<CampRes>>,
) {
    let kills = std::mem::take(&mut game.defeated_keys);
    let presses = std::mem::take(&mut game.command_presses);
    if kills.is_empty() && presses.is_empty() {
        return;
    }
    let counts = area
        .as_ref()
        .is_none_or(|a| a.source.progress().test(state::COUNTERS_FLAG));
    let Some(mut camp) = camp.filter(|_| counts) else {
        return;
    };
    let tick = game.tick;
    for key in kills {
        if state::kill_counter_at(key).is_some() {
            let n = camp.kills.entry(key).or_insert(0);
            *n = (*n + 1).min(state::KILL_MAX);
            let n = *n;
            game.event(serde_json::json!({"event":"kill_counted","tick":tick,"key":key,"n":n}));
        }
    }
    for counter in presses {
        let n = camp.command_uses.entry(counter).or_insert(0);
        *n = n.saturating_add(1);
        let n = *n;
        game.event(
            serde_json::json!({"event":"command_counted","tick":tick,"counter":counter,"n":n}),
        );
    }
}

/// One tick of the area's chests, and a press taken by the reaction search.
pub fn tick(
    mut game: ResMut<Game>,
    area: Option<ResMut<AreaRes>>,
    mut camp: Option<ResMut<CampRes>>,
) {
    let Some(mut area) = area else { return };
    let tick = game.tick;
    let wanted = game.chest_wanted.take();
    if game.chest_tour {
        tour(&mut game, &area, tick);
    }
    let sora = Vec3::from_array(game.sora.player.position);
    let mut at = sora;
    game.chest_camera = None;
    for k in 0..area.current.chests.len() {
        let chest = &area.current.chests[k];
        if chest.state == ChestState::Unread {
            let state = if opened(&game, chest) {
                ChestState::Opened
            } else {
                ChestState::Closed
            };
            area.current.chests[k].state = state;
        }
        if wanted == Some(k) && area.current.chests[k].state == ChestState::Closed {
            open(&mut game, &mut area.current.chests[k], camp.as_deref_mut());
        }
        let chest = &mut area.current.chests[k];
        // The opening motion's own triggers (se000 #22, effect 0).
        if let ChestState::Opening(t) = chest.state {
            for &(frame, id) in &chest.kind.sounds {
                if frame == t {
                    game.sound_queue.push((id, "chest"));
                }
            }
            for &(frame, id) in &chest.kind.effects {
                if frame == t
                    && let Some(pax) = chest.kind.pax.clone()
                {
                    game.fx_queue.push(drive_fx::Placed {
                        req: drive_fx::FxRequest {
                            pax,
                            id,
                            owner: CHEST_FX,
                        },
                        anchor: drive_fx::Anchor::World {
                            at: chest.position,
                            pitch: 0.,
                            yaw: chest.yaw,
                        },
                    });
                }
            }
            // FUN_1403db2f0: the big chest's event, based on the chest.
            if chest.kind.big
                && let Some(view) = event_view(chest, t)
            {
                game.chest_camera = Some(view);
            }
            if chest.kind.big
                && t == EVENT_WINDOW_TICK
                && let Some(item) = chest.item
            {
                game.treasure_events.push(item);
            }
            if t < TURN_TICKS && !chest.kind.big {
                let to = chest.position - sora;
                let target = to.x.atan2(to.z);
                game.sora.player.yaw = turned(game.sora.player.yaw, target);
            }
            // Holds the last pose; the next load stands on the opened one.
            if t + 1 < chest.kind.opening.frames.len() as u32 {
                chest.state = ChestState::Opening(t + 1);
            }
        }
        // [I] The kind-1 shape is the chest's solid body, as the townspeople's.
        let body = chest.kind.body;
        if let Some((_, back)) = crate::npcs::push(at, SORA_OBJ, chest.position, body, false) {
            at.x += back[0];
            at.z += back[1];
        }
    }
    game.sora.player.position[0] = at.x;
    game.sora.player.position[2] = at.z;
}

/// The big chest's event camera at this tick of its opening. [I] FUN_1403ad270's
/// state 2 reads the chest's +0x808, whose half turn from the scene yaw is
/// fitted to the view it gives; +0x808 was not read.
fn event_view(chest: &Chest, tick: u32) -> Option<boss::OverlayView> {
    let bytes = chest.kind.event.as_deref()?;
    boss::Overlay::at(bytes, chest.yaw + std::f32::consts::PI, chest.position)?
        .view_at(tick as f32 / 2.)
}

/// FUN_1404025c0 and FUN_1403a1b60.
fn open(game: &mut Game, chest: &mut Chest, camp: Option<&mut CampRes>) {
    let tick = game.tick;
    let big = chest.kind.big;
    chest.state = ChestState::Opening(0);
    let blend = if big { 0. } else { SMALL_BLEND };
    let played = boss::chest_capture(big)
        .and_then(|o| boss::capture_clip(o, CAPTURE_SLOT))
        .is_some_and(|clip| game.sora.player.capture_motion(clip, blend, &game.clips));
    if played {
        game.sora.player.queue_motion(0, 0.);
        game.action_started = tick;
    }
    // A chest without a row gives Land of Dragons' first and closes again
    // (FUN_1404025c0's fallback); no such row is modelled here.
    let Some(item) = chest.item else {
        game.event(
            serde_json::json!({"event":"chest_open","tick":tick,"object":chest.object,
            "row":false,"capture":played}),
        );
        return;
    };
    // FUN_1403c3f00, into the stock; a full stock loses the item while the bit
    // and the window still happen.
    let mut camp = camp;
    match (game.items.as_mut(), camp.as_mut()) {
        (Some(i), _) => {
            i.give(item);
        }
        (None, Some(c)) => {
            let n = c.stock.entry(item).or_insert(0);
            *n = n.saturating_add(1).min(99);
        }
        _ => {}
    }
    let Game { save, .. } = game;
    game_save::note_obtained(&mut save.base, &save.items, item);
    if chest.bit != 0 {
        let at = state::CHESTS + chest.bit as usize / 8;
        let b = game.save.base.u8_at(at);
        game.save.base.set_u8(at, b | 1 << (chest.bit % 8));
        if let Some(c) = camp.as_mut() {
            c.journal_save.opened_bits.push(chest.bit);
        }
    }
    // FUN_1403a1b20 hands it to the box window, which waits out Sora's motion.
    // A big chest's waits in the ring for its event's op 0x3b.
    if !big {
        game.treasure_boxes.push(item);
    }
    game.event(
        serde_json::json!({"event":"chest_open","tick":tick,"object":chest.object,
        "item":item,"bit":chest.bit,"big":big,"capture":played,
        "slots":game.items.as_ref().map(|i| i.slots.clone())}),
    );
}

/// The tour's placement, in front of the first closed chest.
fn tour(game: &mut Game, area: &AreaRes, tick: u64) {
    let first = area
        .current
        .chests
        .iter()
        .find(|c| matches!(c.state, ChestState::Closed | ChestState::Unread));
    if tick == 1 {
        let chests: Vec<_> = area
            .current
            .chests
            .iter()
            .map(|c| {
                serde_json::json!({"object":c.object,"item":c.item,"bit":c.bit,
                "at":c.position.to_array(),"yaw":c.yaw,"big":c.kind.big})
            })
            .collect();
        game.event(serde_json::json!({"event":"chests","list":chests}));
    }
    if let Some(c) = first {
        if (20..TOUR_PRESS).contains(&tick) {
            let front = Vec3::new(c.yaw.sin(), 0., c.yaw.cos());
            let to = c.position + front * 110.;
            game.sora.player.position = to.to_array();
            game.sora.player.yaw = (-front.x).atan2(-front.z);
        }
        if (TOUR_PRESS..TOUR_PRESS + 3).contains(&tick) {
            game.rc_queued = true;
        }
    }
}

/// Places and poses the chests.
pub fn present(
    game: Res<Game>,
    area: Option<Res<AreaRes>>,
    parts: Query<(&ChestPart, &Mesh3d)>,
    added: Query<(), Added<ChestPart>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut shown: Local<Option<u64>>,
) {
    let Some(area) = area else { return };
    if *shown == Some(game.tick) && added.is_empty() {
        return;
    }
    *shown = Some(game.tick);
    for (part, handle) in &parts {
        let Some(c) = area.current.chests.get(part.chest) else {
            continue;
        };
        let frame = c.pose();
        let positions = c.kind.model.parts[part.ordinal]
            .indices
            .iter()
            .map(|&i| frame[i as usize].to_array());
        visual::room::set_positions(&mut meshes, &handle.0, positions);
    }
}

/// A chest's mesh part.
#[derive(Component)]
pub struct ChestPart {
    pub chest: usize,
    pub ordinal: usize,
}

/// Spawns the chests' models under `parent`.
pub fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<FogMaterial>,
    images: &mut Assets<Image>,
    area: &LoadedArea,
    parent: Entity,
) {
    for (i, c) in area.chests.iter().enumerate() {
        let root = commands
            .spawn((
                RoomPart,
                ChildOf(parent),
                Transform::from_translation(c.position).with_rotation(Quat::from_rotation_y(c.yaw)),
                Visibility::default(),
            ))
            .id();
        for (ordinal, entity) in spawn_model(
            commands,
            &c.kind.model,
            c.pose(),
            root,
            meshes,
            materials,
            images,
        ) {
            if let Some(entity) = entity {
                commands
                    .entity(entity)
                    .insert((RoomPart, ChestPart { chest: i, ordinal }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_turn_takes_the_short_way_at_its_rate() {
        assert!((turned(0., 0.1) - 0.1).abs() < 1e-5);
        assert!((turned(0., 3.) - TURN_RATE).abs() < 1e-5);
        assert!((turned(3., -3.) - (3. + TURN_RATE)).abs() < 1e-5);
    }
}
