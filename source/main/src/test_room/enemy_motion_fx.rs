//! Frame triggers of a battle enemy's own motion (FUN_14040dd70): type 8
//! plays a sound (id = p0 · 1000 + p1) and type 1 starts an element of the
//! enemy's own effect container by id, bound to the enemy. Each trigger
//! fires once when the motion's frame passes it; a new motion or a loop
//! rescans from the start.
use super::*;

/// Fires the triggers of `actor`'s current motion reached since the last
/// scan.
pub fn scan(game: &mut Game, actor: &enemy_gallery::GalleryActor) {
    let Some(battle) = actor.battle.as_ref() else {
        return;
    };
    let frame = battle.frame;
    let serial = battle.serial;
    game.actor_poses
        .insert(actor.id, (actor.position, actor.yaw));
    game.actor_bones.insert(actor.id, battle.bones.clone());
    let from = match game.actor_scan.get(&actor.id) {
        Some(&(s, f)) if s == serial && f <= frame => Some(f),
        _ => None,
    };
    game.actor_scan.insert(actor.id, (serial, frame));
    let object = battle.assets.object_id;
    for records in battle.motion.triggers.iter().map(|t| &t.records.frames) {
        for f in records {
            let at = f.frame.max(0) as f32;
            if !(from.is_none_or(|prev| at > prev) && at <= frame) {
                continue;
            }
            match (f.trigger, f.parameters.as_slice()) {
                (8, &[p0, p1, ..]) if p0 >= 0 && p1 >= 0 => {
                    game.sound_queue
                        .push((p0 as u32 * 1000 + p1 as u32, "enemy_motion"));
                }
                (1, &[id, ..]) if id >= 0 => {
                    game.effect_queue.push(EffectRequest::ActorElement {
                        object,
                        actor: actor.id,
                        id: id as u16,
                    });
                }
                _ => {}
            }
        }
    }
}
