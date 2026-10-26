//! The Keyblade's swing trail: the weapon motion's range trigger 12 marks
//! the swing, the weapon PAX's trail element (id 10, flag 0x10000, a
//! BonePos pair on bone 2) draws it, and every tick the two points of the
//! pair on the blade bone are handed to the running instance
//! (FUN_1402c5f70 → FUN_1403b54f0 build them the same way).
//!
//! The window rule is the exe's (FUN_1403effa0, see [`tick`]): one effect
//! per open window, ended when the window closes. [I] The bone length
//! (FUN_1401a7af0's skeleton field, taken from the blade mesh's extent along
//! its bone) is inferred; the element, its points and the strip are the exe's.
use super::*;
use data::object_effect::{BonePos, Pax};

/// The weapon PAX element the trail uses.
const TRAIL_ELEMENT: u16 = 10;

/// A slot's blade and hand matrices and its weapon vertices per frame.
fn slot_poses(cp: &ClipPoses, slot: usize) -> (&[Mat4], &[Mat4], &[Vec<Vec3>]) {
    if slot == 0 {
        (&cp.blade, &cp.weapon, &cp.weapon_frames)
    } else {
        (&cp.blade2, &cp.weapon2, &cp.weapon2_frames)
    }
}

/// The two points of `bone` on weapon slot `slot`'s blade in engine world
/// space for this tick, or `None` without a weapon pose.
pub fn points(game: &Game, bone: &BonePos, slot: usize) -> Option<[[f32; 3]; 2]> {
    let clip = *game
        .poses
        .get(&(game.sora.player.action, game.sora.player.armed))?;
    let cp = game.clip_poses.get(clip)?;
    let frame = game.sora.player.motion_frame(&game.clips) as usize;
    let (blades, hands, _) = slot_poses(cp, slot);
    let blade = blades.get(frame.min(blades.len().checked_sub(1)?))?;
    let hand = hands.get(frame.min(hands.len().checked_sub(1)?))?;
    let length = game.trail_length[slot]?;
    let at = Vec3::from_array(game.sora.player.position);
    let turn = Quat::from_rotation_y(game.sora.player.yaw);
    let to_scene = |ratio: f32| {
        let p = at + turn * (*hand * *blade).transform_point3(Vec3::X * length * ratio);
        to_world(p.to_array())
    };
    Some([to_scene(bone.ratio_a), to_scene(bone.ratio_b)])
}

/// The blade bone's length: the mesh extent along the bone's x axis divided
/// by the trail element's far ratio (3.7 for the Keyblades).
fn calibrate(game: &Game, far_ratio: f32, slot: usize) -> Option<f32> {
    let clip = *game
        .poses
        .get(&(game.sora.player.action, game.sora.player.armed))?;
    let cp = game.clip_poses.get(clip)?;
    let (blades, _, frames) = slot_poses(cp, slot);
    let blade = blades.first()?;
    let verts = frames.first()?;
    let inverse = blade.inverse();
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for v in verts {
        let x = inverse.transform_point3(*v).x;
        lo = lo.min(x);
        hi = hi.max(x);
    }
    if lo > hi || far_ratio == 0. {
        return None;
    }
    // The ratios are negative: the tip lies on the bone's -x side.
    Some(lo.abs().max(hi.abs()) / far_ratio.abs())
}

/// The effect owner ids of each slot's trail instance.
pub const OWNERS: [u8; 2] = [0xfc, 0xf8];
/// A duel rival's, by his slot then the weapon slot.
pub const RIVAL_OWNERS: [[u8; 2]; 2] = [[0xf0, 0xf1], [0xf2, 0xf3]];

/// The trail owner of weapon `slot` on the room Sora (`rival` None) or a
/// rival.
pub fn owner(rival: Option<u8>, slot: usize) -> u8 {
    match rival {
        None => OWNERS[slot],
        Some(r) => RIVAL_OWNERS[usize::from(r).min(1)][slot],
    }
}

/// FUN_1403effa0, the weapon object's per-frame trail routine: with no live
/// trail effect (`obj+0xc00` clear), an open range-12 window (`obj+0x18c`
/// bit 11) starts element 10 once; while the effect lives it stays as long as
/// the window holds, and when the window closes the effect is ended
/// (FUN_1402be8e0) and the handle cleared, so the next window starts a new
/// one. [I] FUN_1403ba840's blocking conditions (`obj+0x124 & 0x180`, an event
/// mode, a menu) are taken as clear.
/// Every weapon object runs it (FUN_1403efa60), so slot 1's Keyblade
/// (Valor, Master, Final) trails on its own motion's windows with its own
/// element 10; slot 0's room Keyblade only while drawn. A duel rival runs it
/// with his state swapped in (`rival`: his slot).
pub fn tick(game: &mut Game, pax: Option<&Arc<Pax>>, slot: usize, rival: Option<u8>) {
    let Some(pax) = pax else { return };
    let Some(element) = pax.by_id(TRAIL_ELEMENT) else {
        return;
    };
    let owner = owner(rival, slot);
    let window = window_open(game, slot);
    if window && game.trail_length[slot].is_none() {
        game.trail_length[slot] = calibrate(game, element.bone_pos.ratio_b, slot);
        let tick = game.tick;
        let length = game.trail_length[slot];
        let mut e = serde_json::json!({"event":"weapon_trail_length","tick":tick,"slot":slot,"length":length});
        if let Some(r) = rival {
            e["rival"] = r.into();
        }
        game.event(e);
    }
    if !game.trail_live[slot] {
        if window {
            game.fx_queue.push(drive_fx::Placed {
                req: drive_fx::FxRequest {
                    pax: pax.clone(),
                    id: TRAIL_ELEMENT,
                    owner,
                },
                anchor: match rival {
                    None => drive_fx::Anchor::Weapon(slot),
                    Some(r) => drive_fx::Anchor::RivalWeapon(r, slot),
                },
            });
            game.trail_live[slot] = true;
        }
    } else if !window {
        game.fx_release.push(owner);
        game.trail_live[slot] = false;
    }
}

/// Whether the slot's weapon motion holds range 12 at this frame.
fn window_open(game: &Game, slot: usize) -> bool {
    if slot == 0 && !game.sora.player.armed {
        return false;
    }
    let Some(&clip) = game
        .poses
        .get(&(game.sora.player.action, game.sora.player.armed))
    else {
        return false;
    };
    let frame = game.sora.player.motion_frame(&game.clips);
    let cp = &game.clip_poses[clip];
    let windows = if slot == 0 { &cp.trails } else { &cp.trails2 };
    windows
        .iter()
        .any(|&(start, end)| start <= frame && frame <= end)
}
