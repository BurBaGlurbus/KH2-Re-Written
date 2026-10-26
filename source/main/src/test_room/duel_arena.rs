//! The duel's arenas: where a match is fought and where each Sora starts.
//! A row per arena. The two Soras start on a line through the arena's
//! centre (the room's own start), the same distance either side of it,
//! facing each other; the host places both and the snapshots carry it.

use super::*;
use std::f32::consts::PI;

/// One arena.
pub(crate) struct Arena {
    pub name: &'static str,
    /// Each Sora's distance from the centre.
    pub spacing: f32,
}

/// The arenas [duel choice: the test room's arena, the room the launcher's
/// duel opens; no enemies, talk or exits in a duel].
pub(crate) const ARENAS: [Arena; 1] = [Arena {
    name: "Test room",
    spacing: 300.,
}];

/// Where the host's Sora (seat 0) and the joiner's (seat 1) start, as
/// (position, yaw): mirrored through `centre` along the heading `yaw`,
/// each facing the other.
pub(crate) fn starts(arena: &Arena, centre: [f32; 3], yaw: f32) -> [([f32; 3], f32); 2] {
    let (x, z) = (yaw.sin() * arena.spacing, yaw.cos() * arena.spacing);
    [
        ([centre[0] - x, centre[1], centre[2] - z], yaw),
        ([centre[0] + x, centre[1], centre[2] + z], wrap(yaw + PI)),
    ]
}

/// Host: both Soras to the arena's starts, standing (their builds, gauges
/// and forms stay; the motion starts over).
pub(crate) fn place(game: &mut Game) {
    let arena = ARENAS.get(game.duel_arena).unwrap_or(&ARENAS[0]);
    let (centre, yaw) = game.room_start;
    let starts = starts(arena, centre, yaw);
    let tick = game.tick;
    for (seat, (at, facing)) in starts.into_iter().enumerate() {
        let slot = seat as u8;
        if slot == game.local_slot {
            game.sora.player.place(at, facing);
            game.action_started = tick;
        } else if let Some(r) = game.rivals.iter_mut().find(|r| r.slot == slot) {
            r.fighter.player.place(at, facing);
            r.started = tick;
        }
    }
    game.event(
        serde_json::json!({"event":"duel_arena","tick":tick,"arena":arena.name,
        "starts":starts.map(|(at, yaw)| serde_json::json!({"at":at,"yaw":yaw}))}),
    );
}

/// An angle in (-π, π].
fn wrap(a: f32) -> f32 {
    let w = (a + PI).rem_euclid(2. * PI) - PI;
    if w <= -PI { w + 2. * PI } else { w }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_starts_mirror_through_the_centre_and_face_each_other() {
        let centre = [100., 5., -40.];
        for yaw in [0., 0.7, -2.5, PI] {
            let [(a, ya), (b, yb)] = starts(&ARENAS[0], centre, yaw);
            for k in 0..3 {
                assert!(((a[k] + b[k]) / 2. - centre[k]).abs() < 1e-3, "mirrored");
            }
            let apart = ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
            assert!((apart - 2. * ARENAS[0].spacing).abs() < 1e-2);
            // Each faces the other: its heading points at the other's place.
            let toward = (b[0] - a[0]).atan2(b[2] - a[2]);
            assert!(wrap(toward - ya).abs() < 1e-3);
            let back = (a[0] - b[0]).atan2(a[2] - b[2]);
            assert!(wrap(back - yb).abs() < 1e-3);
        }
    }
}
