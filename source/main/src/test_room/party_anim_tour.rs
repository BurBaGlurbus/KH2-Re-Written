//! `--party-anim-tour <new-capture-dir>`: an unattended run of Donald's and
//! Goofy's motions. Sora is invincible (the magic tour's setup) and moves
//! on a fixed schedule:
//!
//! - standing, then a slow walk, a run and a stop: the members follow in
//!   their field stance (idle, walk and run with their blends, the walk
//!   in and out of a run);
//! - the Shadow comes 300 ahead: the battle flag turns the members' stance
//!   to battle (the weapon draw, motion 6) and their AI attacks, casts and
//!   uses items against the frozen Shadow, then against the Shadow's own AI;
//! - the Shadow goes away: once the battle flag drops the stance returns to
//!   the field at a motion's end (the weapon put away), and Sora walks on.
//!
//! The log carries `party_anim` (each member's controller whenever its
//! motion, slot or blend changes: motion id, slot, clock, blend, stance,
//! speed) and `party_pose_jump` (a shown vertex moving more than
//! [`POSE_JUMP`] in the tick of a motion switch, the mark of a switch that
//! does not blend, or more than three times that at any time).
use super::*;

/// The run's end.
pub const END: u64 = 3100;
/// Specials ordered on the way, by member and by their place among the
/// member's specials (each starts once its member is free). With Donald and
/// Goofy: Tornado, Fire, Turbo, Thunder, Blizzard, the Bash and Cure.
const ORDERS: [(u64, usize, usize); 7] = [
    (1000, 1, 0),
    (1060, 0, 0),
    (1180, 1, 2),
    (1240, 0, 2),
    (1400, 0, 1),
    (1450, 1, 1),
    (1600, 0, 3),
];
/// The Shadow comes and goes.
const ENEMY_ON: u64 = 720;
const ENEMY_FIGHTS: u64 = 1300;
const ENEMY_OFF: u64 = 1900;
/// A shown vertex that moves more than this in one tick is logged.
pub const POSE_JUMP: f32 = 12.;

/// Sora's stick: still, a slow walk and a run toward the camera (the
/// members trail behind him in view), a stop, a turn; after the fight a
/// walk and a run again.
pub fn stick(tick: u64) -> Vec2 {
    match tick {
        120..300 => Vec2::new(0., -0.35),
        300..420 => Vec2::new(0., -1.),
        540..600 => Vec2::new(1., 0.),
        2050..2200 => Vec2::new(0., -0.35),
        2200..2350 => Vec2::new(-1., 0.),
        _ => Vec2::ZERO,
    }
}

/// Frames: through each transition every few ticks.
pub fn captures() -> Vec<u64> {
    let mut at: Vec<u64> = Vec::new();
    for (from, to, step) in [
        (30, 130, 50),
        (120, 170, 5),
        (290, 330, 5),
        (470, 540, 5),
        (540, 640, 10),
        (720, 900, 6),
        (900, 1300, 20),
        (1300, 1900, 15),
        (1900, 2100, 6),
        (2100, 3100, 10),
    ] {
        let mut t = from;
        while t < to {
            if at.last().is_none_or(|&l| l < t) {
                at.push(t);
            }
            t += step;
        }
    }
    at
}

/// The tour's memory of what it logged last.
#[derive(Default)]
pub struct State {
    seen: Vec<(u16, usize, u32, bool, bool)>,
}

/// Runs the schedule; called before the party's tick.
pub fn drive(game: &mut Game, tick: u64) {
    if tick <= 1 {
        for m in &mut game.party {
            m.hp = m.max_hp;
        }
        if let Some(rt) = game.enemy.as_mut() {
            rt.enabled = false;
        }
    }
    let sora = Vec3::from_array(game.sora.player.position);
    let forward = Vec3::new(game.sora.player.yaw.sin(), 0., game.sora.player.yaw.cos());
    if tick == ENEMY_ON
        && let Some(rt) = game.enemy.as_mut()
    {
        let at = sora + forward * 300.;
        rt.enabled = true;
        rt.enemy.position = at.to_array();
        rt.enemy.spawn = at.to_array();
        game.party_spells = true;
        game.event(serde_json::json!({"event":"party_anim_tour","tick":tick,"step":"enemy_on"}));
    }
    if tick == ENEMY_FIGHTS {
        game.cheats.enemy_frozen = false;
        game.event(
            serde_json::json!({"event":"party_anim_tour","tick":tick,"step":"enemy_fights"}),
        );
    }
    if tick == ENEMY_OFF {
        if let Some(rt) = game.enemy.as_mut() {
            rt.enabled = false;
        }
        game.party_spells = false;
        for m in game.party.iter_mut() {
            m.hp = m.max_hp;
        }
        game.event(serde_json::json!({"event":"party_anim_tour","tick":tick,"step":"enemy_off"}));
    }
    for &(at, member, nth) in &ORDERS {
        if tick == at {
            let spell = party_room::order_special(game, member, nth);
            game.event(serde_json::json!({"event":"party_anim_tour","tick":tick,
                "step":"order","member":member,"spell":spell.map(|s| format!("{s:?}"))}));
        }
    }
    let mut state = std::mem::take(&mut game.party_anim_state);
    state
        .seen
        .resize(game.party.len(), (u16::MAX, 0, 0, false, false));
    let mut records = Vec::new();
    for (i, m) in game.party.iter().enumerate() {
        let c = &m.follower.ctl;
        let slot = c.clip.map_or(usize::MAX, |c| c.slot);
        let key = (
            c.motion,
            slot,
            c.serial,
            c.blend > 0.,
            m.follower.battle_stance,
        );
        if state.seen[i] != key || tick.is_multiple_of(60) {
            state.seen[i] = key;
            records.push(serde_json::json!({"event":"party_anim","tick":tick,
                "member":m.kind.name,"motion":c.motion,"slot":slot,"serial":c.serial,
                "clock":c.clock,"frame":c.frame(),"blend":c.blend,"weight":c.weight(),
                "battle_stance":m.follower.battle_stance,"speed":m.follower.speed,
                "attack":m.follower.attack.map(|a| a.motion),
                "reaction":m.follower.reaction.map(|r| r.motion),
                "queued":c.queued().collect::<Vec<_>>(),
                "position":m.follower.position,"yaw":m.follower.yaw}));
        }
    }
    for r in records {
        game.event(r);
    }
    game.party_anim_state = state;
}
