//! Sora and the room's battle objects push each other apart
//! (FUN_1403b9090 → FUN_14040ad70, `player::body_push`): each object's
//! kind-1 collision shapes placed around it, its objentry weight, and the
//! motion ranges that make it heavy (44) or immovable (31).
use super::*;
use player::body_push::{Body, HEAVY, Shape};

/// An object's kind-1 shapes around `at` (Y-up, facing `yaw`). The table's
/// offsets are Y-down model units; `position_height` lifts the centre.
/// [I] Shapes on a bone are taken at the object's origin like the others
/// (the bodies read so far sit on the object frame, bone 0x4000).
pub fn shapes(entries: &[object_collision::Entry], at: [f32; 3], yaw: f32) -> Vec<Shape> {
    let (s, c) = yaw.sin_cos();
    entries
        .iter()
        .map(|e| {
            let [x, y, z] = e.position.map(f32::from);
            Shape {
                kind: e.shape,
                centre: [
                    at[0] + x * c + z * s,
                    at[1] - y + f32::from(e.position_height),
                    at[2] - x * s + z * c,
                ],
                radius: f32::from(e.radius),
                height: f32::from(e.height),
            }
        })
        .collect()
}

/// Sora's side: his shown kit's shapes and objentry weight, ×100 heavy
/// under range 44, immovable under range 31.
pub fn sora(
    game_shapes: &[object_collision::Entry],
    weight: f32,
    player: &Player,
    clips: &Clips,
) -> Body {
    let clip = player.clip(clips);
    let frame = player.motion_frame(clips);
    let within = |r: &[(u32, u32)]| r.iter().any(|&(a, b)| (a..=b).contains(&frame));
    Body {
        shapes: shapes(game_shapes, player.position, player.yaw),
        weight: if within(&clip.heavy) { HEAVY } else { weight },
        fixed: within(&clip.fixed),
    }
}
