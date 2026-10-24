//! The engine object ("sysobj") an enemy script drives, and the per-tick engine
//! work around the script: motion clock, root motion, jump/blow/knock physics
//! and movement through the collision world. The motion and physics rules are
//! those the earlier Shadow/Dusk/Soldier translations established from the
//! executable (FUN_1403c6740 motion advance, FUN_1403b89a0 physics sampling,
//! FUN_1403cbb00 jumps, FUN_1403cb360/cb500 arcs, FUN_1403d4570 knock-slide);
//! offsets in comments are the executable's.

use crate::env::{Clips, Env};
use crate::motion::{self, Effect, Flags};
use player::{actor_physics, combat};

pub type Vec3 = [f32; 3];

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [0, 1, 2].map(|k| a[k] - b[k])
}
pub fn hlen(v: Vec3) -> f32 {
    v[0].hypot(v[2])
}

pub fn wrap(a: f32) -> f32 {
    crate::traps::math::wrap_angle(a)
}

/// One HP pool of the battle sheet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pool {
    pub hp: i32,
    pub max: i32,
    pub min: i32,
}

/// +0x158: the motion player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Motion {
    /// +0x28/+0x180: the playing motion slot.
    pub slot: usize,
    /// Clock in ticks.
    pub tick: u32,
    /// Increments on every motion start (attack-instance identity).
    pub serial: u64,
    /// Remaining blend, in motion-clock ticks (0 = none).
    pub blend: f32,
    blend_elapsed: u32,
    blend_duration: u32,
    /// +0x34.. : the event flags (see [`crate::motion`]).
    pub flags: Flags,
    /// Root displacement of this tick's clock step, in model space.
    pub root_step: Vec3,
}

/// Reaction states the engine holds between a hit and returning to the script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    None,
    Flinch,
    Blow { back: bool },
    BlowLand,
}

/// A request to leave the ground once a motion's take-off event fires
/// (`trap_obj_flyjump`, physics mode 11).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingFly {
    pub direction: Vec3,
    pub height: f32,
    pub speed: f32,
}

/// Physics mode 5 (`trap_obj_fly`): a free velocity that follows the object's
/// move request. The routine is FUN_140421c80 with FUN_1403b7ad0 as its
/// steering step. Mode 6 (`trap_obj_hover`) runs the very same routine
/// (FUN_140422900 hands over to mode 5's update) but its mode flags are 0x100,
/// so it counts neither as a jump nor as a fly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Steer {
    /// +0x108: the share of the velocity kept each step.
    pub inertia: f32,
    /// Mode 6 instead of mode 5.
    pub hover: bool,
}

/// +0x118 / +0x11c: the motions a jump plays near its apex and on landing
/// (-1: none). Arming any jump sets (-1, 5): no fall motion, motion 5 on
/// landing (FUN_1403cbb00, FUN_1403cbd00).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JumpMotions {
    pub fall: i32,
    pub land: i32,
}

impl Default for JumpMotions {
    fn default() -> Self {
        Self { fall: -1, land: 5 }
    }
}

/// Physics mode 0x34 (FUN_14053d850): flight at the run speed along the
/// heading, which turns toward the request (`+0x10`, with +0x134 a frame);
/// the vertical speed blends the request's toward the last by the inertia
/// (+0x108 = +0x140).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LimitFlight {
    pub run: f32,
    pub inertia: f32,
    /// The turn rate before the mode.
    pub turn: f32,
}

/// Physics mode 0x2a (FUN_14053d720): the velocity steers toward the move
/// request (FUN_1403b7ad0) with the spec's inertias, the heading follows
/// it, and the height above the floor is held at the spec's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LimitHover {
    /// DAT_142b59470..+0x24 as entered.
    pub spec: [f32; 9],
    /// The turn rate before the mode.
    pub turn: f32,
}

impl LimitHover {
    /// One step's displacement.
    fn step(&self, s: &mut Sysobj, env: &dyn Env) -> Vec3 {
        let dt = actor_physics::PHYSICS_STEP;
        let [_, _, _, steer, fast, slow, between, height, climb] = self.spec;
        let v = s.velocity;
        let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        // FUN_1403b9da0: a slower request is raised to the speed.
        if 0. < s.move_speed && s.move_speed <= speed {
            let d = s.move_direction;
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if l != 0. {
                s.move_direction = d.map(|c| c / l);
            }
            s.move_speed = speed;
        }
        let keep = 1. - if between < s.move_speed { fast } else { slow };
        // FUN_1403b7ad0 (FUN_140141ae0 blends toward the request).
        let nv = if s.move_speed > 0. {
            let t = s.move_direction.map(|c| c * s.move_speed);
            [0, 1, 2].map(|k| t[k] * (1. - steer) + v[k] * steer)
        } else {
            [v[0], v[1], v[2]].map(|c| c * keep)
        };
        s.velocity = [nv[0], nv[1], nv[2], v[3]];
        let mut d = nv.map(|c| c * dt);
        if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > 0. {
            s.turn_dir(d, 0.);
        }
        // FUN_140540470: the floor along a line from 200 above to 10200
        // below (10000 without one); the height moves `climb` a frame
        // toward the spec's when it is more than that plus 10 off.
        let p = s.position;
        let above = env
            .floor(p[0], p[2], p[1] + 200.)
            .filter(|f| *f >= p[1] - 10200.)
            .map_or(10000., |f| p[1] - f);
        let step = dt * climb;
        let off = above - height;
        if step + 10. < off.abs() {
            d[1] += if off >= 0. { -step } else { step };
        }
        d
    }
}

impl LimitFlight {
    /// The velocity (+0x20) of one step. [I] The forward vector
    /// (FUN_1403b56e0, +0x680) is read as the heading's.
    fn velocity(&self, s: &mut Sysobj) -> Vec3 {
        let d = s.move_direction;
        // FUN_1403b65c0: the wished heading (+0x7a8).
        if s.move_speed > 0. && d[0] != 0. && d[2] != 0. {
            s.wish_yaw = Some(d[0].atan2(d[2]));
        }
        let (sin, cos) = s.yaw.sin_cos();
        let vy = d[1] * s.move_speed * (1. - self.inertia) + self.inertia * s.velocity[1];
        [sin * self.run, vy, cos * self.run]
    }
}

/// Physics mode 9 (`trap_obj_hop`): a ballistic hop whose vertical speed is
/// integrated every step (FUN_1403cbdd0) and whose horizontal velocity follows
/// the object's move request with one inertia (+0x108) for both the pull and
/// the decay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hop {
    /// Horizontal velocity (x, z), Y-up frame.
    pub velocity: [f32; 2],
    /// +0x24: vertical speed, Y-down (negative rises).
    pub vy_down: f32,
    /// +0x10c: acceleration downwards per tick squared.
    pub gravity: f32,
    /// +0x108.
    pub inertia: f32,
}

/// What `obj_hop` armed (FUN_1403cbec0): the hop begins when the playing
/// motion's take-off event (frame event 0) fires, like a jump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingHop {
    pub hop: Hop,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fly {
    pub arc: actor_physics::Arc,
    pub velocity: [f32; 2],
    pub initial_height: f32,
    pub original_airborne: bool,
    pub fall_started: bool,
}

/// An alpha fade (sysobj+0xa08: value, rate per tick; FUN_1403b7810/77c0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fade {
    pub value: f32,
    pub rate: f32,
}

impl Fade {
    /// FUN_1403b7810: from `from` to `to` over `ticks` (<= 2 means one tick).
    pub fn start(&mut self, from: f32, to: f32, mut ticks: f32) {
        if ticks <= 2.0 {
            ticks = 1.0;
        }
        if ticks > 0.0 {
            self.value = from;
            self.rate = (to - from) / ticks;
        } else {
            self.value = to;
            self.rate = 0.0;
        }
    }

    /// FUN_1403b77c0: true while the fade runs.
    pub fn step(&mut self, frame_time: f32) -> bool {
        if self.rate == 0.0 {
            return false;
        }
        self.value += self.rate * frame_time;
        if self.value > 0.0 {
            if 1.0 <= self.value {
                self.value = 1.0;
                self.rate = 0.0;
            }
            return true;
        }
        self.value = 0.0;
        self.rate = 0.0;
        true
    }
}

/// The record at sysobj+0xbf8: the actor this one is aiming at.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Target {
    /// 0 = nobody.
    pub id: u32,
    /// Search kind it was made with (+0xc), -1 when unset.
    pub kind: i32,
    /// The record's group word (+4; `target_set_group` / `target_get_group`,
    /// FUN_1403bee60 writes it).
    pub group: i32,
}

#[derive(Clone, Debug)]
pub struct Sysobj {
    /// The model the host selects clips with.
    pub model: u32,
    /// +0x670: position.
    pub position: Vec3,
    pub spawn: Vec3,
    /// +0x68c: heading about Y.
    pub yaw: f32,
    /// +0x7a8: the heading `wish_dir` asks the engine to turn toward.
    pub wish_yaw: Option<f32>,
    /// +0x30: scale (y animated by scripts).
    pub scale: Vec3,
    /// Alpha as faded by +0xa08.
    pub alpha: f32,
    pub fade: Fade,
    pub hidden: bool,
    /// +0x9b8 bit 4: leaving; the object is removed once its alpha is 0
    /// (see [`Sysobj::end_tick`]).
    pub left: bool,
    pub gone: bool,
    /// +0x120: script-settable flags (`obj_set_flag`).
    pub flags: u32,
    pub hp: i32,
    pub stats: combat::TargetStats,
    pub sstm: actor_physics::Sstm,
    pub motion: Motion,
    pub airborne: bool,
    pub reaction: Reaction,
    /// +0x10/+0x1c: the movement the script requested (direction, speed).
    pub move_direction: Vec3,
    pub move_speed: f32,
    /// The weapon objects of this object (`obj+0xd60 + 8 * slot`) and whether
    /// a weapon's motion follows its owner's (`trap_obj_motion_sync`). The
    /// object a weapon is attached to is [`Sysobj::attached`].
    pub weapons: Vec<crate::world::ObjId>,
    pub synced: bool,
    /// +0xa28 / +0xa38: wished and actual displacement of the last tick.
    pub wished: Vec3,
    pub moved: Vec3,
    /// The map collision body (obj+0x6d0; obj+0x9b8 bit 0 "has body"). The
    /// host sets it from the model; without it the object moves by
    /// [`Env::movement`].
    pub map_body: Option<crate::env::MapBody>,
    /// obj+0x744 bit 0: the last solve found no floor under the body.
    pub no_floor: bool,
    /// obj+0x758: the floor normal the last solve left (engine frame).
    pub floor_normal: Vec3,
    pub target: Target,
    /// +0x9f4: spawn arguments, and the spawn type byte at +0x1c.
    pub spawn_args: [i32; 8],
    pub appear_way: i32,
    /// Spawned flying (`obj_is_entry_fly`).
    pub entry_fly: bool,
    /// The attacker of the last hit (the damage record's source).
    pub last_attacker: Option<u32>,
    /// The reaction value of the damage record being handled (+0x26).
    pub damage_reaction: u32,
    /// +0xa58/+0xa64: the second movement request (`trap_obj_move_request`).
    pub request_direction: Vec3,
    pub request_speed: f32,
    /// `trap_sysobj_motion_push`: motions queued behind the playing one.
    pub motion_queue: Vec<(usize, f32)>,
    /// A made object's HIT spheres (model-space offset, radius), which the
    /// host's spawner reads from its model's collision table.
    pub hit_shapes: Vec<(Vec3, f32)>,
    /// +0xd10: enabled attack/behaviour patterns.
    pub pattern_mask: u32,
    /// Pool 0's HP floor (sheet +8) and the pools above it (host-provided).
    pub min_hp: i32,
    pub extra_pools: Vec<Pool>,
    /// +0x1c4 / +0x1c8: prize range and hit-back addition set by scripts.
    pub prize_range: i32,
    pub hitback_addition: i32,
    /// The battle parameter sheet (ENMP) number the script bound.
    pub sheet: i32,
    /// +0x8c8 bit 6: sunk and slithering (Shadow).
    pub shadow_move: bool,
    /// +0x134: turn rate parameter (radians per tick; scripts pass 0 for it).
    pub turn_rate: f32,
    /// `obj_set_movement`'s `+0x118/+0x11c`: the fall and land motions of the
    /// current jump.
    pub jump_motions: JumpMotions,
    /// +0x648: the physics mode the next take-off event (frame event 0) enters
    /// (1 jump, 3 fly).
    pub pending_mode: u8,
    /// The jump a pending mode 1 launches: direction, height, speed.
    pub(crate) pending_jump: Option<(Vec3, f32, f32)>,
    /// Effects the motion events asked for this tick, for the object to carry out.
    pub effects: Vec<Effect>,
    /// Range event 0x18 (+0x108) and 0x19/0x1a (+0x638) values, and the
    /// pattern words range events 0x24/0x25 set (+0xd14 / +0xd18).
    pub event_speed: f32,
    pub event_parameter: u32,
    pub pattern_a: u32,
    pub pattern_b: u32,
    /// +0x4dc: the team (`obj_set_team`).
    pub team: i32,
    /// +0xd48: the revenge ("karma") gauge of an enemy-class object
    /// (FUN_1403db840 starts it at 0): each hit adds its ATKP RevengeDamage
    /// (FUN_1403dbb90), it falls by the frame time while the act manager is
    /// active (FUN_1403dbbf0), and frame event 0x1c starts `revenge` once it
    /// reaches the limit.
    pub revenge: f32,
    /// +0xd4c: the gauge's limit, 100.0 at creation; `enemy_set_karma_limit`
    /// stores the script's float (FUN_140431e30).
    pub karma_limit: f32,
    /// The damage record's original reaction (+0x27): the ATKP reaction
    /// before the `patn` override. [`Sysobj::damage_reaction`] is the one
    /// played (+0x26).
    pub damage_orig_reaction: u32,
    /// The battle sheet's attacker side (+0x188 strength, +0x198/+0x19c the
    /// clamps), when `btlobj_set_sheet` bound one (FUN_1403c2240).
    pub attacker: Option<combat::AttackerStats>,
    /// +0x5a8: the collision shape masks `obj_disable_collision` and
    /// `obj_reset_collision` edit (word `n` of shapes 32n..).
    pub collision: Vec<u32>,
    /// The object entry's id and part id (`obj_get_entry_id`, `obj_part`).
    pub entry_id: u32,
    pub entry_part: u16,
    /// The spawn record's serial (the ushort at +0x1e of the record +0x9f0
    /// points to; `trap_obj_search_by_serial` matches it). 0 when the object
    /// has no spawn record.
    pub spawn_serial: u16,
    /// +0xccc: routine option flags (`rtn_option_flag`), and +0xcd8..+0xcfc:
    /// the routine time parameters (`set_rtn_time_param`; word 7 is `touch_zone`).
    pub rtn_flags: Vec<u32>,
    pub rtn_time: [u32; 9],
    /// +0x20: the steering velocity (four lanes; mode 5 moves by it).
    pub velocity: [f32; 4],
    /// The entry's unit group (`obj_unit_group`) and the damage record's
    /// "reaction executed" flag (+0x18 bit 0).
    pub entry_group: u16,
    pub damage_exec: bool,
    /// The element of the attack being handled (`damage_element`).
    pub damage_element: u8,
    /// +0x2a8: the motion playback speed scripts set (`obj_motion_speed`).
    pub motion_speed: f32,
    /// The attack parameter id of the hit being handled (the damage record's
    /// attack, `damage_attack_param_id`).
    pub damage_param_id: u32,
    /// +0x3c: the fourth lane of the scale vector.
    pub scale_w: f32,
    pub(crate) pending_fly: Option<PendingFly>,
    pub(crate) fly: Option<Fly>,
    /// Physics mode 5 state, and +0x124 bit 27 (the last step moved).
    pub(crate) steer: Option<Steer>,
    pub steer_moving: bool,
    /// A plain jump (FUN_1403cbb00): the arc and its horizontal velocity.
    pub(crate) jump: Option<(actor_physics::Arc, [f32; 2])>,
    /// The jump's last tick was held by a gravity range, and its vertical
    /// step (FUN_1403b4780's re-seed when the range ends).
    jump_float: (bool, f32),
    /// Physics mode 9 (hop), and the hop armed for the next take-off.
    pub(crate) hop: Option<Hop>,
    pub(crate) pending_hop: Option<PendingHop>,
    pub(crate) blow: Option<actor_physics::Blow>,
    pub(crate) knock: Option<actor_physics::Knock>,
    pub(crate) sampler: actor_physics::Sampler,
    pub record: combat::HitRecord,
    /// +0x18c bit 18: the act manager is paused (set by engine states).
    pub act_paused: bool,
    /// Words of the engine object a script wrote through its handle at
    /// offsets nothing else models (the spec block +0x128.., for one). They
    /// read back as written; an engine reader maps its own offset to a field
    /// above when it exists.
    pub raw_words: std::collections::BTreeMap<u32, u32>,
    /// +0x124: state bits (see [`state_flag`]) besides the steering bit.
    pub state_flags: u32,
    /// +0x6c8: the object class bits (see [`class_flag`]); the host sets them
    /// for objects it knows are Sora, summons and so on.
    pub class_flags: u32,
    /// +0x8c8 bit 5: the scissoring draw state (`obj_set_scissoring`).
    pub scissoring: bool,
    /// +0x7e9..+0x7eb: the stealth colour (`obj_set_stealth_color`).
    pub stealth_color: [u8; 3],
    /// +0xd1c: the medal count (`obj_set_medal`, `obj_get_medal`).
    pub medal: i32,
    /// +0xd24..+0xd33: the knock-back request (`obj_hitback` writes the
    /// direction and strength, `obj_clear_hitback` zeroes it).
    pub hitback: [f32; 4],
    /// +0x9d8: the object that rides this one (0 = none).
    pub rider: u32,
    /// +0xac0 == 1 with the entry's pirate bit: a pirate shade
    /// (`obj_is_pirate_shade`, FUN_1403baa90).
    pub pirate_entry: bool,
    pub shade_state: u8,
    /// Stop requests (+0x64c, `enemy_stop_all_start`): a stopped object does
    /// not advance.
    pub stop_requests: Vec<u32>,
    /// The attacker kept for friendly fire (+0xd40, `enemy_set_attacker`).
    pub friend_attacker: Option<u32>,
    /// The attack type byte (ATKP +4) and the hit part (record +0x25) of the
    /// hit being handled.
    pub damage_attack_type: u8,
    pub damage_parts: u8,
    /// +0x6a0..: carried by another object (`obj_attach`, physics mode 3).
    pub attached: Option<Attach>,
    /// Physics mode 0x34 (Peter Pan's limit flight, `obj_limit_peterpan_idle`).
    pub limit_flight: Option<LimitFlight>,
    /// Physics mode 0x2a (Genie's limit hover, `obj_limit_hover`).
    pub limit_hover: Option<LimitHover>,
    /// Physics mode 0x3e (object 0x140750d08) or 0x3c (0x140750ce8), both
    /// of word 0x900: FUN_1401cb6c0 puts Sora in 0x3e for Chicken Little's
    /// limit, FUN_1401c9d60 in 0x3c for Stitch's. Their entry, exit and
    /// step slots are empty (FUN_140409ab0 / ac0 / b30 / b40, FUN_140536250
    /// / 240), so the object does not move; 0x3e's motion slot is the ground
    /// idle picker (FUN_140422520), 0x3c's is empty (FUN_1403cb670); their
    /// fall and landing slots change to mode 0x3f / mode 1.
    pub limit_stand: Option<u8>,
    /// Physics mode 3 with a camera-space matrix (FUN_1403b4da0(obj, 3),
    /// FUN_1403cbfc0: +0x124 bit 6, +0x8c8 bit 12): the object does not
    /// move by itself; its host places it from the camera (Stitch).
    pub camera_space: bool,
    /// FUN_1401a7cf0 / FUN_1401a8a90: the model is hidden (`obj_hide_part`
    /// and `obj_show_part` take a part but the routines ignore it).
    pub model_hidden: bool,
    /// FUN_1401a87a0 / FUN_1401a8570: the model's texture animation plays.
    pub texture_animation: bool,
    /// +0xaa4: the texture fade (`obj_tex_fade_start`).
    pub tex_fade: Fade,
    /// +0x630: the route the object follows (`obj_set_path`,
    /// `create_active_path`).
    pub route: Option<Box<crate::route::Route>>,
    /// +0x9e8: the spawn point (unit) the object came from, which owns the
    /// walk paths its routes are made of.
    pub unit: Option<u32>,
    /// +0xc28..: the routine action state.
    pub rtn: crate::rtn::Rtn,
    /// The object's sound bank (`obj_play_se` ids 500000..500999 take its
    /// thousand); the host sets it from the object entry.
    pub sound_bank: Option<i32>,
    /// +0x5d0: the head look.
    pub head_look: HeadLook,
    /// The captured motion playing, if any.
    pub capture: Option<Capture>,
    /// +0x184 for a motion of a limit file's set: the token
    /// `limit_motion_start` handed out (FUN_1403c64a0), and whether that
    /// motion still counts as playing for the limit (+0x1b0).
    pub limit_token: i32,
    pub limit_motion: bool,
    /// +0xd48: the magic thread `obj_hook` started.
    pub hook: Option<u32>,
    /// The hook table of the motion set in use (+0x160; `obj_motion_hook`).
    pub motion_hook: Option<i32>,
    /// +0x4e0, +0x540, +0x5a0: the reaction-command enables (see
    /// [`crate::reaction`]); the script's group bits (+0x5a8) are
    /// [`Sysobj::collision`].
    pub reaction_groups: crate::reaction::Groups,
    /// +0xa78: how far the map pushed the object out of itself
    /// (`obj_get_bg_press`), in the script frame.
    pub bg_press: [f32; 4],
}

/// A motion borrowed from another object's motion set (`obj_motion_capture`,
/// FUN_1403c6cc0): the object plays `motion` of `captor`'s model until it
/// starts a motion of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capture {
    pub captor: crate::world::ObjId,
    pub motion: i32,
    /// What `obj_motion_capture` returned and `obj_motion_capture_id` reads
    /// back (+0x184) for as long as the capture lasts.
    pub token: i32,
    /// Ticks the borrowed motion has left (the length of the captor's clip).
    pub remaining: u32,
    /// The clip's length and loop start: a looping clip starts over at its
    /// loop start instead of ending.
    pub length: u32,
    pub loop_start: Option<u32>,
    /// FUN_1403c7140: the captured motion queued to follow this one.
    pub next: Option<(crate::world::ObjId, i32, u32, Option<u32>)>,
}

/// +0x5d0: the head look (FUN_1403e5650 / FUN_1403e4f70) the head tracking
/// reads: which object the head turns to and how far the turn has blended in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadLook {
    /// +0x10: the bone of the model's LOOK shape; `None` for a model without
    /// one, which never looks. The host sets it.
    pub bone: Option<u16>,
    /// +8: 2 while a look is on.
    pub state: i32,
    /// +0xc.
    pub kind: i32,
    /// +0x14: the blend (value and rate per tick).
    pub blend: Fade,
    /// +0x30 and +0x38: the object looked at and its facing when the look
    /// began.
    pub target: Option<crate::world::ObjId>,
    pub target_facing: [f32; 4],
}

impl Default for HeadLook {
    fn default() -> Self {
        Self {
            bone: None,
            state: 0,
            kind: 0,
            blend: Fade::default(),
            target: None,
            target_facing: [0.0; 4],
        }
    }
}

/// An object carried by another (FUN_1403d9fe0): it follows the holder's
/// bone `bone` instead of moving on its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attach {
    /// The holder.
    pub to: crate::world::ObjId,
    /// +0x6a4 and +0x6a6.
    pub bone: i32,
    pub flags: u16,
    /// The script entry and AI object the attach was given (`obj_attach`
    /// arguments 4 and 5).
    pub callback: Option<(i32, u32)>,
}

/// Bits of [`Sysobj::state_flags`] (+0x124).
pub mod state_flag {
    /// The jump mode's landing is armed (`FUN_1403cbd00` clears it).
    pub const GROUND_ARMED: u32 = 1;
    /// `obj_hide_shadow`.
    pub const HIDE_SHADOW: u32 = 1 << 9;
    /// The object `obj_search_by_glance` last picked, and one it passed over
    /// after it had been picked.
    pub const GLANCED: u32 = 1 << 12;
    pub const GLANCE_LOST: u32 = 1 << 13;
    /// `obj_is_limit` reads +0x126 bit 0.
    pub const LIMIT: u32 = 1 << 16;
    /// +0x9b8 bit 2 is the dead mark (`obj_dead_mark`); kept in the state
    /// word here.
    pub const DEAD_MARKED: u32 = 1 << 20;
    /// +0x9b8 bit 12: the position was set by `obj_set_pos_trans`.
    pub const TRANSFERRED: u32 = 1 << 21;
}

/// Bits of [`Sysobj::class_flags`] (+0x6c8).
pub mod class_flag {
    /// The enemy class: the objects `enemy_each` and `enemy_stop_all_start`
    /// walk (+0x6c8 bit 4).
    pub const ENEMY: u32 = 1 << 4;
    /// `sysobj_is_zako` (+0x6c8 bit 11): an ordinary enemy rather than a
    /// boss; the host sets it from the object entry.
    pub const ZAKO: u32 = 1 << 11;
    /// `sysobj_is_summon`.
    pub const SUMMON: u32 = 1 << 22;
    /// `sysobj_is_sora` reads +0x6cb bit 0.
    pub const SORA: u32 = 1 << 24;
    /// +0x6c8 bit 10: preferred by target search kind 5 (its distance counts
    /// 10000 shorter).
    pub const PRIORITY_TARGET: u32 = 1 << 10;
    /// The boss class (YS::BOSS, objentry type 3) sets the same bit
    /// (FUN_1403f8da0: `+0x6c8 |= 0x400`); `sysobj_is_boss` reads it.
    pub const BOSS: u32 = PRIORITY_TARGET;
    /// +0x6c8 bit 1: a battle object (FUN_1403d2da0); the per-tick reset of
    /// the pattern words (FUN_14040e5c0) and the range events 0x24/0x25 act
    /// only on these.
    pub const BATTLE: u32 = 1 << 1;
    /// +0x6c8 bit 7: the controlled leader (Sora).
    pub const LEADER: u32 = 1 << 7;
    /// +0x6c8 bit 14: a party member (FUN_1403c2db0).
    pub const PARTY: u32 = 1 << 14;
    /// +0x6c8 bit 3: the object follows routes (a townsperson); read by
    /// FUN_14054e130 when it builds a route.
    pub const ROUTINE: u32 = 1 << 3;
    /// +0x6c8 bit 7: a missing path is looked for among all the area's
    /// units (FUN_1403b9de0).
    pub const AREA_PATHS: u32 = 1 << 7;
}

impl Sysobj {
    /// A zeroed object for tools and tests.
    pub fn bare() -> Self {
        let stats = combat::TargetStats {
            max_hp: 100,
            defense: 0,
            cap: 100,
            min: 0,
            weakness: [100; 7],
        };
        let sstm = actor_physics::Sstm {
            fall_max: 20.,
            blow_brake_xz: 0.,
            blow_min_xz: 0.,
            blow_brake_up: 0.,
            blow_up: 0.,
            blow_speed: 0.,
            blow_to_hit_back: 0.,
            hit_back: 0.,
            hit_back_small: 0.,
            hit_back_to_jump: 0.,
        };
        Self::new(0, [0.; 3], 0., stats, sstm)
    }

    pub fn new(
        model: u32,
        position: Vec3,
        yaw: f32,
        stats: combat::TargetStats,
        sstm: actor_physics::Sstm,
    ) -> Self {
        Self {
            model,
            position,
            spawn: position,
            yaw,
            wish_yaw: None,
            scale: [1.; 3],
            alpha: 1.,
            fade: Fade::default(),
            hidden: false,
            left: false,
            gone: false,
            flags: 0,
            hp: stats.max_hp,
            stats,
            sstm,
            motion: Motion::default(),
            airborne: false,
            reaction: Reaction::None,
            weapons: Vec::new(),
            synced: false,
            move_direction: [0.; 3],
            move_speed: 0.,
            wished: [0.; 3],
            moved: [0.; 3],
            map_body: None,
            no_floor: false,
            floor_normal: [0., -1., 0.],
            target: Target::default(),
            spawn_args: [0; 8],
            appear_way: -1,
            entry_fly: false,
            last_attacker: None,
            damage_reaction: 0,
            request_direction: [0.; 3],
            request_speed: 0.,
            motion_queue: Vec::new(),
            hit_shapes: Vec::new(),
            pattern_mask: 0,
            min_hp: 0,
            extra_pools: Vec::new(),
            prize_range: 0,
            hitback_addition: 0,
            sheet: 0,
            shadow_move: false,
            turn_rate: 0.,
            jump_motions: JumpMotions::default(),
            pending_mode: 0,
            pending_jump: None,
            effects: Vec::new(),
            event_speed: 0.,
            event_parameter: 0,
            pattern_a: 0,
            pattern_b: 0,
            team: 0,
            revenge: 0.,
            karma_limit: 100.,
            damage_orig_reaction: 0,
            attacker: None,
            collision: Vec::new(),
            entry_id: 0,
            entry_part: 0,
            spawn_serial: 0,
            rtn_flags: Vec::new(),
            rtn_time: [0; 9],
            scale_w: 1.,
            velocity: [0.; 4],
            entry_group: 0,
            damage_exec: false,
            damage_element: 0,
            motion_speed: 1.,
            damage_param_id: 0,
            pending_fly: None,
            fly: None,
            steer: None,
            steer_moving: false,
            jump: None,
            jump_float: (false, 0.),
            hop: None,
            pending_hop: None,
            blow: None,
            knock: None,
            sampler: actor_physics::Sampler::default(),
            record: combat::HitRecord::new(false),
            act_paused: false,
            raw_words: std::collections::BTreeMap::new(),
            state_flags: 0,
            class_flags: class_flag::ENEMY | class_flag::BATTLE,
            scissoring: false,
            stealth_color: [0; 3],
            medal: 0,
            hitback: [0.; 4],
            rider: 0,
            pirate_entry: false,
            shade_state: 0,
            stop_requests: Vec::new(),
            friend_attacker: None,
            damage_attack_type: 0,
            damage_parts: 0,
            attached: None,
            camera_space: false,
            limit_flight: None,
            limit_hover: None,
            limit_stand: None,
            model_hidden: false,
            texture_animation: false,
            tex_fade: Fade::default(),
            route: None,
            unit: None,
            rtn: Default::default(),
            bg_press: [0.; 4],
            reaction_groups: Default::default(),
            head_look: HeadLook::default(),
            capture: None,
            limit_token: 0,
            limit_motion: false,
            hook: None,
            motion_hook: None,
            sound_bank: None,
        }
    }

    /// The flag word of the current physics mode (+0xc of the mode object the
    /// executable keeps in the table at DAT_140750d30): bit 0 air, bit 1 fly,
    /// bit 6 blow, bit 7 ground, bit 8 always. The modes this crate runs are
    /// listed; the others are not entered here.
    pub fn mode_flags(&self) -> u32 {
        if self.camera_space {
            // Mode 3's +0xc.
            0x100
        } else if self.limit_stand.is_some() {
            // Mode 0x3e's and 0x3c's +0xc.
            0x900
        } else if self.limit_flight.is_some() {
            // Mode 0x34's +0xc.
            0x10a
        } else if self.limit_hover.is_some() {
            // Mode 0x2a's +0xc.
            0x100
        } else if self.blow.is_some() {
            0x1c1
        } else if self.jump.is_some() {
            0x181
        } else if self.hop.is_some() {
            0x101
        } else if self.fly.is_some() {
            0x103
        } else if let Some(steer) = &self.steer {
            if steer.hover { 0x100 } else { 0x102 }
        } else {
            0x180
        }
    }

    /// FUN_1403c86a0 / FUN_1403c88c0: begin a motion. A blend of at most two
    /// ticks takes the immediate path.
    pub fn motion_start(&mut self, slot: usize, blend: f32, clips: &dyn Clips) -> bool {
        if clips.length(slot) == 0 {
            return false;
        }
        let m = &mut self.motion;
        m.slot = slot;
        m.tick = 0;
        m.serial = m.serial.wrapping_add(1);
        m.blend = if blend <= 2. { 0. } else { blend };
        m.blend_duration = m.blend as u32;
        m.blend_elapsed = 0;
        m.root_step = [0.; 3];
        // FUN_1403c8a40 rewrites the flag word when a motion starts, so the end
        // pulse of the previous motion does not carry over to the new one.
        m.flags.word &= !motion::flag::ENDED;
        m.flags.pulses = 0;
        // A motion of its own ends a captured one.
        m.flags.word &= !(1 << 14);
        self.capture = None;
        true
    }

    /// FUN_1403c7300 / FUN_14040e5c0 as FUN_1403c65e0 calls them when a
    /// motion has no entry: the object stops on an ended motion.
    pub fn motion_stop(&mut self) {
        let m = &mut self.motion;
        m.blend = 0.;
        m.root_step = [0.; 3];
        m.flags.word |= motion::flag::ENDED;
    }

    /// FUN_1403c6dc0: like start, but nothing happens if `slot` already plays.
    pub fn motion_change(&mut self, slot: usize, blend: f32, clips: &dyn Clips) -> bool {
        if self.motion.slot == slot {
            return true;
        }
        self.motion_start(slot, blend, clips)
    }

    /// FUN_1403ba070's motion windows: the command the window names
    /// (+0x638) is taken while flag bit 17 holds, or a command whose +0x22
    /// (`chain`) it names while range 0x1a (bit 21) holds; flag bit 30
    /// (range 0x28) refuses every other. None leaves the answer to the
    /// command lockout and the free windows. [I] The first test (command
    /// flag bit 1 with the object's states) is not applied.
    pub fn command_window(&self, command: u16, chain: u16) -> Option<bool> {
        let word = self.motion.flags.word;
        let param = self.event_parameter;
        if word & 0x2_0000 != 0 && param == u32::from(command)
            || chain != 0 && word & 0x20_0000 != 0 && param == u32::from(chain)
        {
            return Some(true);
        }
        (word & 0x4000_0000 != 0).then_some(false)
    }

    /// FUN_1403ba070's free windows for a command with flags `flags` (+0xc):
    /// flag bit 8 with motion flag bit 19; a range 0 to 2; flag bit 7 clear
    /// with motion flag bit 10 or +0x194 bit 5; or flag bit 2. [I] The
    /// command lockout (+0xb94) is not kept for engine objects, and the
    /// motion-less case (+0x158 null) does not arise.
    pub fn free_window(&self, flags: u32) -> bool {
        let (w, w3) = (self.motion.flags.word, self.motion.flags.word3);
        flags & 0x100 != 0 && w & 0x8_0000 != 0
            || w & 0x1c != 0
            || flags & 0x80 == 0 && (w & 0x400 != 0 || w3 & 0x20 != 0)
            || flags & 4 != 0
    }

    /// +0x18c bit 1 (the motion ended on this tick's step).
    pub fn motion_ended(&self) -> bool {
        self.motion.flags.word & motion::flag::ENDED != 0
    }

    /// Hit-stun suspension (FUN_1403ca7c0): a damage (10..=39) or reflect
    /// (140..=150) motion still plays.
    pub fn hit_stunned(&self) -> bool {
        let s = self.motion.slot;
        (10..=39).contains(&s) || (140..=150).contains(&s)
    }

    /// The object class an objentry's type byte (+4) builds
    /// (FUN_1403df930): type 3 BOSS (FUN_1403f8da0) and 0xc L_BOSS add the
    /// boss bit and the pattern bits {2, 10, 15} (`+0xd10 |= 0x8404`); type 4
    /// ZAKO and 0x15 L_ZAKO the ordinary-enemy bit 11. [U] The f32 2000.0
    /// the boss constructor stores at +0x8e0 has no reader found.
    pub fn set_class(&mut self, objentry_type: u8) {
        match objentry_type {
            3 | 0xc => {
                self.class_flags = self.class_flags & !class_flag::ZAKO | class_flag::BOSS;
                self.pattern_mask |= 0x8404;
            }
            4 | 0x15 => self.class_flags |= class_flag::ZAKO,
            _ => {}
        }
    }

    /// The active state pattern (`(+0xd14 | +0xd10) & ~+0xd18`): the
    /// constructor's and script's bits plus the current motion's range 0x24
    /// bits, without its range 0x25 bits.
    pub fn active_patterns(&self) -> u32 {
        (self.pattern_a | self.pattern_mask) & !self.pattern_b
    }

    /// FUN_1403d3570 (BTLOBJ vt+0xb8, "cannot be damaged"): +0x120 bit 8,
    /// motion range 0x1b (+0x18c bit 22), a stop request with bit 2, or the
    /// +0x120 bits that hide an object from searches. [U] The pause and
    /// mission-lock terms (FUN_140152940, FUN_1403a57f0) are the host's.
    pub fn invulnerable(&self) -> bool {
        self.flags & 0x100 != 0
            || self.motion.flags.word & 0x40_0000 != 0
            || self.stop_requests.iter().any(|r| r & 4 != 0)
            || self.flags & 0x1008_0000 != 0
    }

    /// FUN_1403ca250 (`act_active`) for the revenge decay: the act table
    /// exists, the manager is not locked, motion range 0x15 (+0x18c bit 18)
    /// is off, and no damage motion holds an act below priority 300.
    pub fn revenge_decays(&self, acts: &crate::act::Manager) -> bool {
        acts.has_table
            && !acts.locked()
            && self.motion.flags.word & 0x4_0000 == 0
            && !self.act_paused
            && !(acts.priority < 300 && self.hit_stunned())
    }

    /// FUN_1403b9f70 (`obj_turn_dir`): turn toward the direction's heading by
    /// at most `2 * rate` radians. Returns true while the heading differs.
    pub fn turn_dir(&mut self, direction: Vec3, rate: f32) -> bool {
        let rate = if rate == 0. { self.turn_rate } else { rate };
        let step = rate * actor_physics::PHYSICS_STEP;
        let delta = wrap(direction[0].atan2(direction[2]) - self.yaw);
        if delta > step {
            self.yaw = wrap(step + self.yaw);
            true
        } else if -step <= delta {
            self.yaw = wrap(self.yaw);
            false
        } else {
            self.yaw = wrap(self.yaw - step);
            true
        }
    }

    pub fn forward(&self) -> Vec3 {
        [self.yaw.sin(), 0., self.yaw.cos()]
    }

    /// Applies a requested displacement through the collision world.
    fn apply_movement(&mut self, wished: Vec3, env: &dyn Env) {
        let from = self.position;
        let solved = self
            .map_body
            .as_ref()
            .and_then(|body| env.collide(body, from, wished, self.air_query(), self.floor_normal));
        if let Some(m) = solved {
            self.position = m.position;
            self.no_floor = m.no_floor;
            self.floor_normal = m.normal;
        } else {
            self.position = env.movement(from, wished);
        }
        self.wished = wished;
        self.moved = sub(self.position, from);
    }

    /// The floor an air move came down on this tick, if it did. With a map
    /// body it is FUN_1403b9090's land branch: not rising (`disp.y >= 0` in
    /// the engine's frame), the solve found a floor (`+0x744` bit 0 clear)
    /// and FUN_1403ba1e0 lets it land (no range 3); the solver has already
    /// put the feet on it. [I] Without one, the room's floor within 30
    /// units above the feet.
    fn touchdown(&self, d: Vec3, env: &dyn Env) -> Option<f32> {
        if d[1] > 0. {
            return None;
        }
        if self.map_body.is_some() {
            let can_land = self.motion.flags.word & motion::flag::GRAVITY_FROZEN == 0;
            return (!self.no_floor && can_land).then_some(self.position[1]);
        }
        let [x, y, z] = self.position;
        env.floor(x, z, y + 30.).filter(|&floor| y <= floor)
    }

    /// FUN_1403b9090's airborne query (CB+0x70 bit 0): FUN_1403ba9a0 (an
    /// air mode, a gravity range or `+0x120` bit 23), the mode's flag bit 1,
    /// or a hold range.
    fn air_query(&self) -> bool {
        self.mode_flags() & 3 != 0 || self.gravity_frozen() || self.flags >> 23 & 1 != 0
    }

    /// The start of a frame (FUN_1403c6740): advance the motion clock, fire the
    /// frame events it passed, reset and re-evaluate the range flags. Runs
    /// before the script.
    pub fn begin_tick(&mut self, env: &dyn Env) {
        if self.gone {
            return;
        }
        let clips = env.clips(self.model);
        let m = &mut self.motion;
        m.flags.pulses = 0;
        m.root_step = [0.; 3];
        if m.flags.word & motion::flag::PAUSED != 0 {
            return;
        }
        m.flags.word &= !0x102;
        if clips.length(m.slot) == 0 {
            return;
        }
        let old = m.tick;
        let mut stepped = false;
        if m.blend > 0. {
            m.blend_elapsed = if m.blend_elapsed == 0 {
                2
            } else {
                m.blend_elapsed.saturating_add(1)
            };
            if m.blend_elapsed >= m.blend_duration {
                m.tick = m.blend_elapsed - m.blend_duration;
                m.blend = 0.;
                m.blend_elapsed = 0;
                stepped = true;
            } else {
                m.blend = (m.blend_duration - m.blend_elapsed) as f32;
            }
        } else {
            m.tick = clips.advance(m.slot, m.tick);
            m.root_step = clips.root_delta(m.slot, old);
            stepped = true;
        }
        if stepped {
            let ended = clips.ended(m.slot, m.tick);
            if ended {
                m.flags.word |= motion::flag::ENDED;
            }
            self.dispatch_frame_events(clips);
            // FUN_1403c6740: a clock that reached the end with a motion queued
            // behind it (`trap_sysobj_motion_push`) starts that motion now, and
            // the end bit is set after the start (`motion_is_end` sees it).
            if ended
                && !self.motion_queue.is_empty()
                && let (slot, blend) = self.motion_queue.remove(0)
                && self.motion_start(slot, blend, clips)
            {
                self.motion.flags.word |= motion::flag::ENDED;
                self.dispatch_ranges(clips);
                return;
            }
        }
        self.dispatch_ranges(clips);
    }

    /// Frame events whose frame the clock just passed.
    fn dispatch_frame_events(&mut self, clips: &dyn Clips) {
        let slot = self.motion.slot;
        let (t, duration, loop_top) =
            (self.motion.tick, clips.length(slot), clips.loop_start(slot));
        let Some(events) = clips.events(slot) else {
            return;
        };
        for e in &events.frames {
            if motion::frame_event_fires(e.frame, t, duration, loop_top) {
                self.motion.flags.frame_event(e, slot, &mut self.effects);
            }
        }
    }

    /// FUN_14040e5c0 / FUN_14040e740: reset, then the ranges the clock is in.
    fn dispatch_ranges(&mut self, clips: &dyn Clips) {
        let slot = self.motion.slot;
        self.motion.flags.reset();
        // FUN_14040e5c0: a battle object's pattern words +0xd14/+0xd18 and
        // its per-motion reaction-command enables are rebuilt every tick.
        if self.class_flags & class_flag::BATTLE != 0 {
            self.pattern_a = 0;
            self.pattern_b = 0;
        }
        self.reaction_groups.clear_motion();
        let clock =
            motion::native_clock(self.motion.tick, clips.length(slot), clips.loop_start(slot));
        let Some(events) = clips.events(slot) else {
            return;
        };
        for e in &events.ranges {
            if u32::from(e.start) <= clock && (e.end == u16::MAX || clock < u32::from(e.end)) {
                self.motion.flags.range_event(e, slot, &mut self.effects);
            }
        }
    }

    /// Take-off (frame event 0): enter the physics mode a script asked for.
    pub fn take_off(&mut self, env: &dyn Env) {
        match self.pending_mode {
            1 => {
                if let Some((direction, height, speed)) = self.pending_jump.take() {
                    self.start_jump(direction, height, speed);
                }
            }
            2 => {
                if let Some(PendingHop { hop }) = self.pending_hop.take() {
                    self.enter_hop(hop);
                }
            }
            3 => {
                if let Some(launch) = self.pending_fly.take() {
                    let floor = env.floor(self.position[0], self.position[2], self.position[1]);
                    self.steer = None;
                    self.fly = Some(Fly {
                        arc: actor_physics::Arc::launch(self.position[1], launch.height),
                        velocity: [
                            launch.direction[0] * launch.speed,
                            launch.direction[2] * launch.speed,
                        ],
                        initial_height: floor.map_or(0., |f| (self.position[1] - f).max(0.)),
                        original_airborne: self.airborne,
                        fall_started: false,
                    });
                    self.airborne = true;
                    self.sampler.reset();
                }
            }
            _ => {}
        }
        self.pending_mode = 0;
    }

    /// The rest of a frame after the script: heading, root motion, physics.
    pub fn end_tick(&mut self, env: &dyn Env) {
        if self.gone {
            return;
        }
        let clips = env.clips(self.model);
        let no_control = self.motion.flags.word & motion::flag::NO_CONTROL != 0;
        let reacting = self.reaction != Reaction::None && self.hp > 0;
        if reacting {
            self.update_reaction(clips, env);
        }
        if !reacting
            && !no_control
            && let Some(desired) = self.wish_yaw
        {
            let delta = wrap(desired - self.yaw);
            let step = self.turn_rate.max(0.) * 2.;
            self.yaw = wrap(self.yaw + delta.clamp(-step, step));
        }
        // FUN_1403cc730: the alpha fade (+0xa08), then the texture fade
        // (+0xaa4), run with the frame.
        self.fade.step(1.0);
        self.tex_fade.step(1.0);
        // FUN_1403bfd30, after the fades: a leaving object (`+0x9b8` bit 4)
        // whose alpha reached 0 goes to its class's vt+0x38 (FUN_1403b2e10
        // and the classes' wrappers), which frees its script and motion and
        // marks it for removal (`+0x120` bit 28).
        if self.left && self.fade.value == 0. {
            self.gone = true;
            return;
        }
        // A carried object does not move by itself (physics mode 3); the
        // world places it on its holder. Nor does one in camera space.
        if self.attached.is_some() || self.camera_space || self.limit_stand.is_some() {
            return;
        }
        if let Some(f) = self.limit_flight {
            // Like every mode routine it runs every `PHYSICS_STEP` frames.
            if self.sampler.due(self.move_speed) {
                let v = f.velocity(self);
                self.velocity = [v[0], v[1], v[2], self.velocity[3]];
                self.sampler
                    .store(v.map(|c| c * actor_physics::PHYSICS_STEP));
            }
            let d = self.sampler.displacement();
            self.apply_movement(d, env);
            return;
        }
        if let Some(h) = self.limit_hover {
            if self.sampler.due(self.move_speed) {
                let d = h.step(self, env);
                self.sampler.store(d);
            }
            let d = self.sampler.displacement();
            self.apply_movement(d, env);
            return;
        }
        if self.fly.is_some() {
            self.update_fly(clips, env);
        } else if let Some(steer) = self.steer {
            self.update_steer(steer, env);
        } else if let Some(hop) = self.hop {
            self.update_hop(hop, clips, env);
        } else if let Some((arc, velocity)) = self.jump {
            self.update_jump(arc, velocity, clips, env);
        } else if !reacting {
            let root = self.motion.root_step;
            let (sin, cos) = self.yaw.sin_cos();
            let wished = if self.move_speed != 0. {
                self.move_direction.map(|v| v * self.move_speed)
            } else {
                [
                    root[0] * cos + root[2] * sin,
                    root[1],
                    -root[0] * sin + root[2] * cos,
                ]
            };
            self.apply_movement(wished, env);
            self.leave_ground(clips, env);
        }
        // FUN_1403b89a0 adds the frame's move request (`obj_move_request`,
        // +0xa58 direction, +0xa64 speed) times the frame time, then clears
        // it. [I] Here it is a second step through the collision world.
        if self.request_speed != 0. {
            let dt = 1.0;
            let d = self.request_direction;
            let wished = d.map(|v| v * self.request_speed * dt);
            let earlier = self.moved;
            self.apply_movement(wished, env);
            self.moved = [0, 1, 2].map(|k| earlier[k] + self.moved[k]);
        }
        self.request_direction = [0.; 3];
        self.request_speed = 0.;
    }

    /// FUN_140421c80: one step of physics mode 5. Like every mode routine it
    /// runs every `PHYSICS_STEP` frames (FUN_1403b89a0) and its displacement is
    /// spread over them.
    fn update_steer(&mut self, steer: Steer, env: &dyn Env) {
        if self.sampler.due(self.move_speed) {
            let dt = actor_physics::PHYSICS_STEP;
            // FUN_1403b7ad0: toward the request keeping `inertia` of the
            // velocity, or the velocity scaled by `inertia` without one.
            let v = self.velocity;
            let v3 = if self.move_speed > 0. {
                let target = self.move_direction.map(|c| c * self.move_speed);
                [0, 1, 2].map(|k| target[k] * (1. - steer.inertia) + v[k] * steer.inertia)
            } else {
                [v[0], v[1], v[2]].map(|c| c * steer.inertia)
            };
            self.velocity = [v3[0], v3[1], v3[2], v[3]];
            let step = v3.map(|c| c * dt);
            self.steer_moving = step[0] * step[0] + step[1] * step[1] + step[2] * step[2] > 1.0e-4;
            self.sampler.store(step);
        }
        self.steer = Some(steer);
        let d = self.sampler.displacement();
        self.apply_movement(d, env);
    }

    /// `trap_obj_fly` (FUN_140421ce0): enter physics mode 5 with a velocity of
    /// `direction` (normalised) × `speed`.
    pub fn start_steer(&mut self, direction: Vec3, speed: f32, inertia: f32) {
        let len = (direction[0] * direction[0]
            + direction[1] * direction[1]
            + direction[2] * direction[2])
            .sqrt();
        let v = if len != 0. {
            direction.map(|c| c / len * speed)
        } else {
            direction.map(|c| c * speed)
        };
        self.velocity = [v[0], v[1], v[2], speed * speed];
        self.leave_air_modes();
        self.steer = Some(Steer {
            inertia,
            hover: false,
        });
        self.sampler.reset();
        self.airborne = true;
    }

    /// `trap_obj_hover` (FUN_140422950): physics mode 6, the steering mode
    /// with the velocity `normalize(v.xyz) * v.w`. Unlike `start_steer` it
    /// does not mark the object airborne (the mode flags are 0x100).
    pub fn start_hover(&mut self, v: [f32; 4], inertia: f32) {
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let unit = if len != 0. {
            [v[0] / len, v[1] / len, v[2] / len]
        } else {
            [v[0], v[1], v[2]]
        };
        self.velocity = [unit[0] * v[3], unit[1] * v[3], unit[2] * v[3], v[3] * v[3]];
        self.leave_air_modes();
        self.steer = Some(Steer {
            inertia,
            hover: true,
        });
        self.sampler.reset();
    }

    /// Entering a physics mode leaves the others (FUN_1403b4da0).
    fn leave_air_modes(&mut self) {
        // Mode 0x34's exit loads PRTY record 1, mode 0x2a's (FUN_14053d980)
        // the object's own record (+0x148): the turn rate comes back [I:
        // as it was on entry].
        if let Some(f) = self.limit_flight.take() {
            self.turn_rate = f.turn;
        }
        if let Some(h) = self.limit_hover.take() {
            self.turn_rate = h.turn;
        }
        self.limit_stand = None;
        self.jump = None;
        self.hop = None;
        self.fly = None;
        self.pending_fly = None;
        self.pending_hop = None;
        self.pending_mode = 0;
    }

    /// FUN_1403cbec0 (`trap_obj_hop`): arm a hop that starts at the next
    /// take-off event. `v` is the script's vector: heading x/z, the apex
    /// height in y (script frame: positive rises here) and the horizontal
    /// speed in w; `time` is how long the rise takes.
    pub fn arm_hop(&mut self, v: [f32; 4], time: f32, inertia: f32) {
        let (x, z) = (v[0], v[2]);
        let len = (x * x + z * z).sqrt();
        let dir = if len != 0. {
            [x / len, z / len]
        } else {
            [x, z]
        };
        // FUN_1403cbec0: g = 2h / t^2, rising speed g * t (Y-down: negative).
        let gravity = (v[1] + v[1]) / (time * time);
        self.pending_mode = 2;
        self.pending_hop = Some(PendingHop {
            hop: Hop {
                velocity: [dir[0] * v[3], dir[1] * v[3]],
                vy_down: -(gravity * time),
                gravity,
                inertia,
            },
        });
        self.pending_jump = None;
        self.jump_motions = JumpMotions::default();
        self.state_flags &= !state_flag::GROUND_ARMED;
    }

    /// Enter the hop mode now (mode 9; FUN_1403cbd30 on entry).
    pub fn enter_hop(&mut self, hop: Hop) {
        self.leave_air_modes();
        self.steer = None;
        self.hop = Some(hop);
        self.state_flags |= state_flag::GROUND_ARMED;
        self.sampler.reset();
        self.airborne = true;
    }

    /// FUN_1403cbdd0, once per physics step: horizontal velocity by the
    /// steering step (the object's move request, one inertia for pull and
    /// decay), vertical speed by the hop's gravity capped at the spec's
    /// maximum fall speed (+0x138). The fall motion plays five ticks before
    /// the apex; landing plays the land motion.
    fn update_hop(&mut self, mut hop: Hop, clips: &dyn Clips, env: &dyn Env) {
        if self.sampler.due(self.move_speed) {
            let dt = actor_physics::PHYSICS_STEP;
            hop.velocity = actor_physics::steer(
                hop.velocity,
                [self.move_direction[0], self.move_direction[2]],
                self.move_speed,
                hop.inertia,
                hop.inertia,
            );
            // Y-down: positive falls. The cap is a spec word scripts write;
            // an unwritten one leaves the speed uncapped.
            let cap = self
                .raw_words
                .get(&0x138)
                .map_or(f32::INFINITY, |w| f32::from_bits(*w));
            hop.vy_down = (dt * hop.gravity + hop.vy_down).min(cap);
            // The vertical lane of the step is vy * dt (Y-up: negated).
            self.sampler.store([
                hop.velocity[0] * dt,
                -hop.vy_down * dt,
                hop.velocity[1] * dt,
            ]);
        }
        self.hop = Some(hop);
        let d = self.sampler.displacement();
        self.apply_movement(d, env);
        // FUN_1403cbe70 (mode slot 7): the fall motion, once, near the apex.
        if self.state_flags & state_flag::GROUND_ARMED != 0
            && self.jump_motions.fall >= 0
            && hop.vy_down + hop.gravity * actor_physics::FALL_MOTION_LEAD >= 0.
        {
            self.state_flags &= !state_flag::GROUND_ARMED;
            self.motion_change(
                self.jump_motions.fall as usize,
                actor_physics::MOTION_BLEND,
                clips,
            );
        }
        if let Some(floor) = self.touchdown(d, env) {
            self.position[1] = floor;
            self.hop = None;
            self.land(clips);
        }
    }

    /// FUN_1403cb940 (mode slot 6): touching down plays the land motion with
    /// idle queued behind it (FUN_1403c8310(motion 0)) and returns to the
    /// ground mode.
    fn land(&mut self, clips: &dyn Clips) {
        self.airborne = false;
        self.sampler = actor_physics::Sampler::default();
        self.state_flags &= !(1 << 27);
        let land = self.jump_motions.land;
        if land >= 0 {
            let slot = land as usize;
            let played = if land != 5 {
                self.motion_change(slot, 0., clips);
                true
            } else if clips.length(slot) != 0 {
                self.motion_start(slot, 0., clips);
                true
            } else {
                false
            };
            if played {
                self.motion_queue.push((0, 0.));
            }
        }
    }

    fn update_jump(
        &mut self,
        mut arc: actor_physics::Arc,
        mut velocity: [f32; 2],
        clips: &dyn Clips,
        env: &dyn Env,
    ) {
        // A gravity range (range 3, `+0x18c` bit 5) holds the height while
        // the curve's time runs on (FUN_1403cb500); when it ends the arc is
        // re-seeded as a fall from the height at the last step's vertical
        // speed (FUN_1403b4780).
        let floating = self.gravity_frozen();
        if self.jump_float.0 && !floating {
            arc = actor_physics::Arc::fall(self.position[1], self.jump_float.1);
        }
        if self.sampler.due(0.) {
            let dt = actor_physics::PHYSICS_STEP;
            // Without a move request the take-off velocity decays (FUN_1403b7ad0).
            velocity = actor_physics::steer(
                velocity,
                [0.; 2],
                0.,
                actor_physics::AIR_INERTIA,
                actor_physics::AIR_DRAG,
            );
            let dy = arc.step(self.position[1], self.sstm.fall_max, floating, dt);
            self.sampler.store([velocity[0] * dt, dy, velocity[1] * dt]);
        }
        self.jump = Some((arc, velocity));
        let d = self.sampler.displacement();
        self.jump_float = (floating, d[1]);
        self.apply_movement(d, env);
        // FUN_1403cb680 (mode slot 7): the fall motion starts five ticks before
        // the apex, once.
        if self.state_flags & state_flag::GROUND_ARMED != 0
            && self.jump_motions.fall >= 0
            && arc.falling_motion()
        {
            self.state_flags &= !state_flag::GROUND_ARMED;
            self.motion_change(
                self.jump_motions.fall as usize,
                actor_physics::MOTION_BLEND,
                clips,
            );
        }
        if let Some(floor) = self.touchdown(d, env) {
            self.position[1] = floor;
            self.jump = None;
            self.land(clips);
        }
    }

    fn update_fly(&mut self, clips: &dyn Clips, env: &dyn Env) {
        let mut fly = self.fly.expect("active fly jump");
        if !fly.fall_started && fly.arc.falling_motion() && self.reaction == Reaction::None {
            if !self.motion_start(155 + 1, actor_physics::MOTION_BLEND, clips) {
                return;
            }
            fly.fall_started = true;
        }
        let height = env
            .floor(self.position[0], self.position[2], self.position[1])
            .map_or(0., |f| (self.position[1] - f).max(0.));
        if fly.arc.t >= fly.arc.rise && height <= fly.initial_height {
            self.airborne = fly.original_airborne;
            self.fly = None;
            self.sampler.reset();
            self.motion_start(155 + 2, 0., clips);
            return;
        }
        let frozen = self.gravity_frozen();
        if self.sampler.due(self.move_speed) {
            let dt = actor_physics::PHYSICS_STEP;
            let dy = fly
                .arc
                .step(self.position[1], self.sstm.fall_max, frozen, dt);
            fly.velocity = actor_physics::steer(
                fly.velocity,
                [self.move_direction[0], self.move_direction[2]],
                self.move_speed,
                actor_physics::AIR_INERTIA,
                actor_physics::AIR_DRAG,
            );
            self.sampler
                .store([fly.velocity[0] * dt, dy, fly.velocity[1] * dt]);
        }
        let root = self.motion.root_step;
        let (sin, cos) = self.yaw.sin_cos();
        let root = [
            root[0] * cos + root[2] * sin,
            root[1],
            -root[0] * sin + root[2] * cos,
        ];
        let base = self.sampler.displacement();
        let wished = [0, 1, 2].map(|axis| base[axis] + root[axis]);
        self.apply_movement(wished, env);
        fly.arc.base -= self.moved[1] - wished[1];
        self.fly = Some(fly);
    }

    fn update_reaction(&mut self, clips: &dyn Clips, env: &dyn Env) {
        if let Reaction::Blow { back } = self.reaction {
            if let Some(blow) = &mut self.blow {
                if self.sampler.due(0.) {
                    self.sampler.store(blow.step(
                        &self.sstm,
                        self.sstm.fall_max,
                        actor_physics::PHYSICS_STEP,
                    ));
                }
                let d = self.sampler.displacement();
                let from = self.position;
                self.position = env.movement(from, d);
                // The floor is looked for from above the step's start, as
                // the hop's landing does: a blow that starts on the floor
                // and drops at once still lands on it.
                if d[1] <= 0.
                    && let Some(y) = env.floor(self.position[0], self.position[2], from[1] + 30.)
                    && self.position[1] <= y
                {
                    self.position[1] = y;
                    self.blow = None;
                    self.airborne = false;
                    self.reaction = Reaction::BlowLand;
                    self.motion_start(if back { 19 } else { 17 }, 0., clips);
                }
            }
        } else {
            if let Some(knock) = &mut self.knock {
                if let Some(d) = knock.step() {
                    self.position = env.movement(self.position, d);
                } else {
                    self.knock = None;
                }
            }
            if clips.ended(self.motion.slot, self.motion.tick) {
                self.reaction = Reaction::None;
                self.motion_start(0, 0., clips);
            }
        }
    }

    /// FUN_1403cbb00 (`method_obj_jump`): arm a jump. The object leaves the
    /// ground when the playing motion's take-off event (frame event 0) fires
    /// (`+0x648` = 1). `height` is the Y-up apex height, `speed` the
    /// horizontal speed along `direction`.
    pub fn launch_jump(&mut self, direction: Vec3, height: f32, speed: f32) {
        self.pending_mode = 1;
        self.pending_jump = Some((direction, height, speed));
        self.pending_hop = None;
        // Arming resets the motions the jump plays near its apex and when it
        // lands (FUN_1403cbb00): none, and motion 5.
        self.jump_motions = JumpMotions::default();
        self.state_flags &= !state_flag::GROUND_ARMED;
    }

    /// Enter the jump physics mode now.
    fn start_jump(&mut self, direction: Vec3, height: f32, speed: f32) {
        let len = hlen(direction);
        let dir = if len > 0. {
            [direction[0] / len, direction[2] / len]
        } else {
            [0.; 2]
        };
        self.steer = None;
        self.hop = None;
        self.jump = Some((
            actor_physics::Arc::launch(self.position[1], height),
            [dir[0] * speed, dir[1] * speed],
        ));
        // FUN_1403cbd30 on entering the jump mode arms the fall motion.
        self.state_flags |= state_flag::GROUND_ARMED;
        self.sampler.reset();
        self.airborne = true;
    }

    /// Arc frozen by a gravity range (`+0x18c & 0x1000020` or bit 6).
    pub fn gravity_frozen(&self) -> bool {
        let w = self.motion.flags.word;
        w & (motion::flag::GRAVITY_FROZEN | motion::flag::GRAVITY_FROZEN_2) != 0
            || w & motion::flag::HOLD != 0
    }

    /// FUN_1403ba9a0: true while the object is airborne or in a special state;
    /// an act first run waits for it to clear.
    pub fn air_blocked(&self) -> bool {
        self.is_jump()
            || self.motion.flags.word
                & (motion::flag::GRAVITY_FROZEN | motion::flag::GRAVITY_FROZEN_2)
                != 0
            || self.flags >> 23 & 1 != 0
    }

    /// +0x18c bit 9: the playing clip holds the actor out of control.
    pub fn no_control(&self) -> bool {
        self.motion.flags.word & motion::flag::NO_CONTROL != 0
    }

    /// FUN_1403b9090's leave-ground branch: an object in a grounded mode
    /// (mode flag bits 0 and 1 clear) with no gravity range (`+0x18c &
    /// 0x1000020`) and no hold range (bit 6) that has no floor contact
    /// (`+0x744` bit 0, the collision query's flag) leaves the ground unless
    /// range 0x1d holds (`+0x18c` bit 23). It clears `+0x120` bit 26 (set by
    /// a handler to cancel the rest), then sends script event 0xd and runs
    /// the mode's slot 5 (a no-op for the ground mode) and its class's leave
    /// handler (vt+0x50). Every
    /// class wrapper ends in FUN_1403b7850: the mode's slot 4 with height 0
    /// (FUN_140409b00 -> FUN_1403cbc30: no vertical speed, no fall motion,
    /// motion 5 on landing; mode 1). The summon class (FUN_1403d6430) first
    /// changes to motion 4 over 16 frames unless +0x194 bit 4. (The other
    /// branch, with FUN_1403ba1e0, is the landing.) [I] The collision-enabled
    /// test (FUN_1403ba940) is taken as true; "on the floor" is within one
    /// unit of the floor under the object; event 0xd is not sent.
    fn leave_ground(&mut self, clips: &dyn Clips, env: &dyn Env) {
        let w = self.motion.flags.word;
        let grounded_mode = self.mode_flags() & 3 == 0;
        let ranged = w
            & (motion::flag::GRAVITY_FROZEN | motion::flag::GRAVITY_FROZEN_2 | motion::flag::HOLD)
            != 0;
        if !grounded_mode || ranged {
            return;
        }
        // `+0x744` bit 0 from the solve; without a body, the floor within
        // one unit.
        let on_floor = if self.map_body.is_some() {
            !self.no_floor
        } else {
            let [x, y, z] = self.position;
            env.floor(x, z, y + 1.).is_some_and(|f| y - f <= 1.)
        };
        if on_floor || w & 0x80_0000 != 0 {
            return;
        }
        self.flags &= !0x400_0000;
        if self.class_flags & class_flag::SUMMON != 0 && self.motion.flags.word3 & 0x10 == 0 {
            self.motion_change(4, 16., clips);
        }
        self.jump_motions = JumpMotions::default();
        self.start_fall(0.);
        self.state_flags &= !state_flag::GROUND_ARMED;
        // FUN_1403b7850 sets it again after the fall.
        self.flags |= 0x400_0000;
    }

    /// `obj_limit_peterpan_idle` (FUN_14053e900): physics mode 0x34 with
    /// its entry (FUN_14053da60): the movement block from PRTY record 0x3e
    /// (FUN_14040a480), no velocity, the vertical inertia from +0x140.
    pub fn to_limit_flight(&mut self, spec: Option<actor_physics::ObjParams>) {
        self.leave_air_modes();
        self.camera_space = false;
        self.steer = None;
        self.sampler.reset();
        let spec = spec.unwrap_or_else(|| actor_physics::ObjParams::defaults(self.sstm.fall_max));
        let turn = self.turn_rate;
        self.turn_rate = spec.turn;
        self.velocity = [0., 0., 0., self.velocity[3]];
        self.limit_flight = Some(LimitFlight {
            run: spec.run,
            inertia: spec.word10,
            turn,
        });
    }

    /// FUN_1403b4da0(obj, 0x3e or 0x3c): the current mode's exit, then the
    /// mode, which has no entry of its own.
    pub fn to_limit_stand(&mut self, mode: u8) {
        self.leave_air_modes();
        self.camera_space = false;
        self.limit_stand = Some(mode);
        self.sampler.reset();
    }

    /// `obj_limit_hover` (FUN_14053d680): a velocity of `v`'s direction
    /// times its w, then physics mode 0x2a with its entry (FUN_14053d9e0:
    /// the spec's turn rate, +0x134). `spec` is DAT_142b59470 with the
    /// call's inertias in place.
    pub fn to_limit_hover(&mut self, v: [f32; 4], spec: [f32; 9]) {
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let d = if len != 0. {
            [v[0] / len, v[1] / len, v[2] / len]
        } else {
            [v[0], v[1], v[2]]
        };
        self.leave_air_modes();
        self.camera_space = false;
        self.steer = None;
        self.sampler.reset();
        self.velocity = [d[0] * v[3], d[1] * v[3], d[2] * v[3], self.velocity[3]];
        let turn = self.turn_rate;
        self.turn_rate = spec[2];
        self.limit_hover = Some(LimitHover { spec, turn });
    }

    /// FUN_1403b4da0(obj, 3) with FUN_1403cbfc0: physics mode 3 in camera
    /// space; any jump, fall, steering or hop ends.
    pub fn to_camera_mode(&mut self) {
        self.camera_space = true;
        self.jump = None;
        self.fly = None;
        self.steer = None;
        self.hop = None;
        self.airborne = false;
    }

    /// Physics mode entry for a fall (FUN_1403b8150): the parabola continues
    /// from the given Y-up vertical speed with no horizontal velocity.
    pub fn start_fall(&mut self, vertical_up: f32) {
        self.steer = None;
        self.hop = None;
        self.jump = Some((
            actor_physics::Arc::fall(self.position[1], vertical_up),
            [0.; 2],
        ));
        self.sampler.reset();
        self.airborne = true;
    }

    /// Read a word of the engine object that scripts access directly through
    /// its handle (`.d` addressing). Only the offsets found in installed
    /// scripts are mapped: +0x30..+0x38 is the scale, +0x134 the turn rate
    /// (word 12 of the spec `obj_spec` returns).
    pub fn peek_word(&self, offset: u32) -> Option<u32> {
        match offset {
            0x30 => Some(self.scale[0].to_bits()),
            0x34 => Some(self.scale[1].to_bits()),
            0x38 => Some(self.scale[2].to_bits()),
            0x3c => Some(self.scale_w.to_bits()),
            0x20..=0x2c if offset.is_multiple_of(4) => {
                Some(self.velocity[((offset - 0x20) / 4) as usize].to_bits())
            }
            0x134 => Some(self.turn_rate.to_bits()),
            _ => self.raw_words.get(&offset).copied(),
        }
    }

    /// Write a word of the engine object (see [`Sysobj::peek_word`]).
    pub fn poke_word(&mut self, offset: u32, value: u32) -> bool {
        match offset {
            0x30 => self.scale[0] = f32::from_bits(value),
            0x34 => self.scale[1] = f32::from_bits(value),
            0x38 => self.scale[2] = f32::from_bits(value),
            0x3c => self.scale_w = f32::from_bits(value),
            0x20..=0x2c if offset.is_multiple_of(4) => {
                self.velocity[((offset - 0x20) / 4) as usize] = f32::from_bits(value)
            }
            0x134 => self.turn_rate = f32::from_bits(value),
            _ if offset < 0x1000 && offset.is_multiple_of(4) => {
                self.raw_words.insert(offset, value);
            }
            _ => return false,
        }
        true
    }

    /// `obj_jump_direct`: enter the jump mode now.
    pub fn jump_now(&mut self, direction: Vec3, height: f32, speed: f32) {
        self.pending_mode = 0;
        self.pending_jump = None;
        self.start_jump(direction, height, speed);
    }

    /// `obj_blow` (FUN_1403d3e50): knocked away with this velocity.
    pub fn blow_now(&mut self, velocity: Vec3) {
        self.blow = Some(actor_physics::Blow::new(velocity, &self.sstm));
        self.knock = None;
        self.jump = None;
        self.hop = None;
        self.fly = None;
        self.steer = None;
        self.pending_fly = None;
        self.sampler.reset();
        self.airborne = true;
        self.reaction = Reaction::Blow { back: false };
    }

    /// FUN_140418800: set routine option flag `index`.
    pub fn set_rtn_flag(&mut self, index: u32) {
        let w = (index >> 5) as usize;
        if self.rtn_flags.len() <= w {
            self.rtn_flags.resize(w + 1, 0);
        }
        self.rtn_flags[w] |= 1 << (index & 31);
    }

    /// FUN_140417630.
    pub fn rtn_flag(&self, index: u32) -> bool {
        self.rtn_flags
            .get((index >> 5) as usize)
            .is_some_and(|w| w >> (index & 31) & 1 != 0)
    }

    /// `trap_obj_is_fly` (FUN_14042ef50): the physics mode has the fly bit
    /// (the flyjump and steering modes here), or the motion holds the arc
    /// (+0x18c bit 6).
    pub fn is_fly(&self) -> bool {
        self.fly.is_some()
            || self.steer.is_some_and(|s| !s.hover)
            || self.motion.flags.word & motion::flag::HOLD != 0
    }

    /// FUN_1403f1fa0: enable collision shape `index` (bit in the first word of
    /// its pair).
    pub fn enable_collision(&mut self, index: u32) {
        let w = (index >> 5) as usize;
        if self.collision.len() <= w + 1 {
            self.collision.resize(w + 2, 0);
        }
        self.collision[w] |= 1 << (index & 31);
    }

    /// FUN_1403f1f80: disable collision shape `index` (bit in the second word of
    /// its pair).
    pub fn disable_collision(&mut self, index: u32) {
        let w = (index >> 5) as usize + 1;
        if self.collision.len() <= w {
            self.collision.resize(w + 1, 0);
        }
        self.collision[w] |= 1 << (index & 31);
    }

    /// FUN_1403f2000: reset collision shape `index` (clears the bit in both words).
    pub fn reset_collision(&mut self, index: u32) {
        let w = (index >> 5) as usize;
        if self.collision.len() <= w + 1 {
            self.collision.resize(w + 2, 0);
        }
        self.collision[w] &= !(1 << (index & 31));
        self.collision[w + 1] &= !(1 << (index & 31));
    }

    /// The script's collision-group words (+0x5a8: {enable, disable}).
    pub fn script_group_bits(&self) -> [u32; 2] {
        [0, 1].map(|k| self.collision.get(k).copied().unwrap_or(0))
    }

    /// Pool `index` of the battle sheet (0 is the object's HP).
    pub fn pool(&self, index: usize) -> Option<Pool> {
        if index == 0 {
            Some(Pool {
                hp: self.hp,
                max: self.stats.max_hp,
                min: self.min_hp,
            })
        } else {
            self.extra_pools.get(index - 1).copied()
        }
    }

    pub fn set_pool_min(&mut self, index: usize, value: i32) {
        if index == 0 {
            self.min_hp = value;
        } else if let Some(p) = self.extra_pools.get_mut(index - 1) {
            p.min = value;
        }
    }

    /// FUN_140431140: a value under the floor lands on the floor, one over the
    /// maximum on the maximum.
    pub fn set_pool_hp(&mut self, index: usize, value: i32) {
        let Some(p) = self.pool(index) else {
            return;
        };
        let hp = if value < p.min {
            p.min
        } else {
            value.min(p.max)
        };
        if index == 0 {
            self.hp = hp;
        } else if let Some(p) = self.extra_pools.get_mut(index - 1) {
            p.hp = hp;
        }
    }

    /// `trap_obj_idle`: back to the ground physics mode (FUN_1403b4da0 mode 0).
    pub fn to_ground_mode(&mut self) {
        self.jump = None;
        self.hop = None;
        self.fly = None;
        self.steer = None;
        self.pending_fly = None;
        self.pending_hop = None;
        self.airborne = false;
        self.sampler.reset();
    }

    /// `trap_obj_is_jump`: the physics state byte at +0xc bit 0 (jump, hop
    /// and flyjump have it; the steering modes do not).
    pub fn is_jump(&self) -> bool {
        self.jump.is_some() || self.hop.is_some() || self.fly.is_some()
    }

    /// A fall (`start_fall`, e.g. `obj_fly_to_jump`) is running.
    pub fn falling(&self) -> bool {
        self.jump.is_some()
    }
}

#[cfg(test)]
mod tests;
