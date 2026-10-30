//! Synthetic objects only.
use super::*;
use crate::env::{Clips, Env};

struct Thirty;

impl Clips for Thirty {
    fn length(&self, _: usize) -> u32 {
        30
    }
    fn loop_start(&self, _: usize) -> u32 {
        0
    }
    fn root_delta(&self, _: usize, _: u32) -> Vec3 {
        [0.; 3]
    }
    fn events(&self, _: usize) -> Option<&crate::motion::MotionEvents> {
        None
    }
}

/// A flat floor at zero.
struct Flat;

impl Env for Flat {
    fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
        Some(0.)
    }
    fn clips(&self, _: u32) -> &dyn Clips {
        &Thirty
    }
    fn player_position(&self) -> Vec3 {
        [0.; 3]
    }
    fn target_position(&self, _: u32) -> Option<Vec3> {
        None
    }
}

/// A map whose floor (y = 0, for x < 100) only the body solver sees: the
/// room's own floor test finds nothing.
struct BodyFloor;

impl Env for BodyFloor {
    fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
        None
    }
    fn collide(
        &self,
        _: &crate::env::MapBody,
        from: Vec3,
        d: Vec3,
        _: bool,
        normal: Vec3,
    ) -> Option<crate::env::MapMove> {
        let mut position = [from[0] + d[0], from[1] + d[1], from[2] + d[2]];
        let floor = position[0] < 100.;
        if floor && position[1] <= 0. {
            position[1] = 0.;
        }
        Some(crate::env::MapMove {
            position,
            no_floor: !(floor && position[1] <= 1.),
            normal,
        })
    }
    fn clips(&self, _: u32) -> &dyn Clips {
        &Thirty
    }
    fn player_position(&self) -> Vec3 {
        [0.; 3]
    }
    fn target_position(&self, _: u32) -> Option<Vec3> {
        None
    }
}

const BODY: crate::env::MapBody = crate::env::MapBody {
    radius: 150.,
    height: 150.,
    offset: 170.,
    mode: 0,
    class: 1,
};

#[test]
fn a_body_leaves_and_lands_by_the_solver_not_the_room_floor() {
    // Standing where the solver finds no floor: it leaves the ground.
    let mut s = Sysobj::bare();
    s.map_body = Some(BODY);
    s.position = [150., 0., 0.];
    s.end_tick(&BodyFloor);
    assert!(s.no_floor && s.is_jump(), "no floor under the body: a fall");
    // Over the solver's floor it comes down and lands, though the room's
    // floor test answers nothing there.
    let mut s = Sysobj::bare();
    s.map_body = Some(BODY);
    s.position = [50., 120., 0.];
    s.end_tick(&BodyFloor);
    assert!(s.is_jump());
    for _ in 0..200 {
        s.end_tick(&BodyFloor);
    }
    assert!(!s.is_jump() && close(s.position[1], 0.) && !s.no_floor);
    // A gravity range keeps it from landing (FUN_1403ba1e0).
    let mut s = Sysobj::bare();
    s.map_body = Some(BODY);
    s.position = [50., 0., 0.];
    s.no_floor = false;
    s.start_fall(0.);
    s.motion.flags.word |= motion::flag::GRAVITY_FROZEN;
    s.end_tick(&BodyFloor);
    assert!(s.is_jump(), "range 3 holds the landing");
}

#[test]
fn a_battle_object_slides_by_its_knock_back_without_a_reaction() {
    // FUN_1403b89a0 adds FUN_1403d4570 in every mode (`obj_hitback` on Sora
    // during a limit).
    let mut s = Sysobj::bare();
    s.knock = actor_physics::Knock::new([1., 0., 0.], 100., 0.);
    s.end_tick(&Flat);
    assert!(s.position[0] > 5., "{:?}", s.position);
    let mut other = Sysobj::bare();
    other.class_flags &= !class_flag::BATTLE;
    other.knock = actor_physics::Knock::new([1., 0., 0.], 100., 0.);
    other.end_tick(&Flat);
    assert_eq!(other.position[0], 0.);
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0e-3
}

#[test]
fn hover_runs_the_steering_mode_but_is_neither_a_jump_nor_a_fly() {
    let mut s = Sysobj::bare();
    s.start_hover([3., 4., 0., 5.], 0.5);
    assert!(!s.is_jump());
    assert!(!s.is_fly());
    assert_eq!(s.mode_flags(), 0x100);
    assert!(!s.airborne, "mode 6 does not mark the object airborne");
    assert!(close(s.velocity[0], 3.) && close(s.velocity[1], 4.));
    // Mode 6 hands its update to mode 5's: the first run keeps half.
    s.end_tick(&Flat);
    assert!(close(s.velocity[0], 1.5) && close(s.velocity[1], 2.));
    // Mode 5 is a fly and not a hover.
    let mut f = Sysobj::bare();
    f.start_steer([1., 0., 0.], 1., 0.5);
    assert_eq!(f.mode_flags(), 0x102);
    assert!(f.is_fly());
}

#[test]
fn a_hop_starts_at_the_take_off_event_and_rises_with_the_executables_integration() {
    let mut s = Sysobj::bare();
    // Apex 100 in 20 ticks: g = 2h / t^2 = 0.5, rising speed g * t = 10.
    s.arm_hop([0., 100., 0., 0.], 20., 0.9);
    assert_eq!(s.pending_mode, 2);
    assert!(!s.is_jump(), "armed, not yet airborne");
    s.take_off(&Flat);
    assert!(s.is_jump());
    assert_eq!(s.mode_flags(), 0x101);
    let mut peak = 0f32;
    let mut ticks = 0;
    while s.is_jump() && ticks < 400 {
        s.end_tick(&Flat);
        peak = peak.max(s.position[1]);
        ticks += 1;
    }
    // The vertical speed is updated before it is applied (FUN_1403cbdd0), so
    // ten two-tick steps of 9, 8, ... 0 rise 90 of the 100 asked for.
    assert!(close(peak, 90.), "peak {peak}");
    assert!(!s.is_jump(), "it lands");
    assert!(close(s.position[1], 0.));
    assert!(!s.airborne);
}

#[test]
fn a_landing_queues_idle_behind_the_land_motion() {
    let mut s = Sysobj::bare();
    s.arm_hop([0., 100., 0., 0.], 20., 0.9);
    s.take_off(&Flat);
    for _ in 0..400 {
        s.end_tick(&Flat);
        if !s.is_jump() {
            break;
        }
    }
    assert!(!s.is_jump());
    // FUN_1403cb940 -> FUN_1403c8310(motion 0): the land motion hands over
    // to idle when it ends, so its no-control range does not stay on.
    assert_eq!(s.motion_queue, [(0, 0.)]);
}

/// A floor at zero found only from at or above it.
struct FloorFromAbove;

impl Env for FloorFromAbove {
    fn floor(&self, _: f32, _: f32, top: f32) -> Option<f32> {
        (top >= 0.).then_some(0.)
    }
    fn clips(&self, _: u32) -> &dyn Clips {
        &Thirty
    }
    fn player_position(&self) -> Vec3 {
        [0.; 3]
    }
    fn target_position(&self, _: u32) -> Option<Vec3> {
        None
    }
}

#[test]
fn a_blow_that_starts_just_under_the_floor_lands_on_it() {
    let mut s = Sysobj::bare();
    s.position = [0., -0.25, 0.];
    s.blow_now([0., 0., 50.]);
    for _ in 0..400 {
        s.end_tick(&FloorFromAbove);
        if s.blow.is_none() {
            break;
        }
    }
    assert!(s.blow.is_none(), "it lands");
    assert!(close(s.position[1], 0.), "y {}", s.position[1]);
}

#[test]
fn a_hop_is_capped_by_the_spec_fall_speed() {
    let mut s = Sysobj::bare();
    // A script writes the spec's maximum fall speed (+0x138) as it does for
    // the hop.
    assert!(s.poke_word(0x138, 3f32.to_bits()));
    s.position = [0., 500., 0.];
    s.arm_hop([0., 1., 0., 0.], 2., 0.9);
    s.take_off(&Flat);
    for _ in 0..40 {
        s.end_tick(&Flat);
    }
    let fall = s.moved[1].abs();
    assert!(fall <= 3.01, "falling {fall} per tick");
}

#[test]
fn a_jump_plays_its_fall_motion_near_the_apex_and_its_land_motion_on_touching_down() {
    let mut s = Sysobj::bare();
    s.launch_jump([0., 0., 0.], 100., 0.);
    s.jump_motions = JumpMotions { fall: 7, land: 8 };
    s.take_off(&Flat);
    assert!(s.is_jump());
    let mut seen = Vec::new();
    for _ in 0..400 {
        s.end_tick(&Flat);
        if seen.last() != Some(&s.motion.slot) {
            seen.push(s.motion.slot);
        }
        if !s.is_jump() {
            break;
        }
    }
    assert!(!s.is_jump());
    assert_eq!(
        seen,
        [0, 7, 8],
        "idle, the fall motion once, then the land motion"
    );
}

#[test]
fn arming_a_jump_resets_the_fall_and_land_motions() {
    let mut s = Sysobj::bare();
    s.jump_motions = JumpMotions { fall: 7, land: 8 };
    s.launch_jump([0., 0., 1.], 50., 4.);
    assert_eq!(s.jump_motions, JumpMotions { fall: -1, land: 5 });
    // With no fall motion the arc plays none; the land motion is 5.
    s.take_off(&Flat);
    let mut slots = vec![s.motion.slot];
    for _ in 0..400 {
        s.end_tick(&Flat);
        if slots.last() != Some(&s.motion.slot) {
            slots.push(s.motion.slot);
        }
        if !s.is_jump() {
            break;
        }
    }
    assert_eq!(slots, [0, 5]);
}

#[test]
fn a_grounded_object_left_in_the_air_falls_once_no_range_holds_it() {
    // FUN_1403b9090: a range 0x1e (or 3) keeps a grounded object's root-driven
    // air phase going; it does not leave the ground.
    let mut s = Sysobj::bare();
    s.class_flags |= class_flag::SUMMON;
    s.position = [0., 140., 0.];
    s.motion.flags.word |= motion::flag::GRAVITY_FROZEN_2;
    s.end_tick(&Flat);
    assert!(!s.is_jump(), "held by the range");
    // Range 0x1d (`+0x18c` bit 23) stops it leaving; the script flag word's
    // bit 23 (`+0x120`) does not.
    s.motion.flags.word = 0x80_0000;
    s.end_tick(&Flat);
    assert!(!s.is_jump(), "held by range 0x1d");
    s.flags |= 1 << 23;
    // Without a range, off the floor: it leaves the ground (the summon class
    // first takes motion 4) and falls to the floor; the fall (FUN_1403b7850)
    // sets `+0x120` bit 26.
    s.motion.flags.word = 0;
    s.end_tick(&Flat);
    assert!(s.is_jump(), "off the floor in the ground mode: a fall");
    assert_eq!(s.motion.slot, 4);
    assert_eq!(s.jump_motions, JumpMotions::default());
    assert_ne!(s.flags & 0x400_0000, 0);
    for _ in 0..200 {
        s.end_tick(&Flat);
    }
    assert!(!s.is_jump() && close(s.position[1], 0.));
    // On the floor nothing happens.
    let mut g = Sysobj::bare();
    g.end_tick(&Flat);
    assert!(!g.is_jump());
}

#[test]
fn limit_flight_runs_along_the_heading_and_blends_the_climb() {
    let mut s = Sysobj::bare();
    s.turn_rate = 0.1;
    let mut spec = actor_physics::ObjParams::defaults(-30.);
    spec.run = 10.;
    spec.turn = 0.2;
    spec.word10 = 0.5;
    s.to_limit_flight(Some(spec));
    assert_eq!(s.mode_flags(), 0x10a);
    assert!(close(s.turn_rate, 0.2));
    // A straight-up request: no new heading, half of its climb.
    s.move_direction = [0., 1., 0.];
    s.move_speed = 4.;
    s.end_tick(&Flat);
    assert!(
        close(s.velocity[2], 10.) && close(s.velocity[1], 2.),
        "{:?}",
        s.velocity
    );
    assert!(s.wish_yaw.is_none());
    // A request with x and z sets the wished heading.
    s.move_direction = [1., 0., 1.];
    s.end_tick(&Flat);
    s.end_tick(&Flat);
    assert!(
        s.wish_yaw
            .is_some_and(|y| close(y, std::f32::consts::FRAC_PI_4))
    );
    // Leaving gives the turn rate back.
    s.start_steer([1., 0., 0.], 1., 0.5);
    assert!(s.limit_flight.is_none() && close(s.turn_rate, 0.1));
}

#[test]
fn limit_hover_slows_without_a_request_and_climbs_to_its_height() {
    let mut s = Sysobj::bare();
    // walk, run, turn, steering, fast, slow, between, height, climb
    let spec = [0., 0., 0.3, 0.5, 0.2, 0.4, 5., 100., 3.];
    s.to_limit_hover([2., 0., 0., 4.], spec);
    assert_eq!(s.mode_flags(), 0x100);
    assert!(close(s.velocity[0], 4.) && close(s.turn_rate, 0.3));
    s.end_tick(&Flat);
    // Without a request the slow inertia applies; 100 below its height it
    // rises 3 a frame.
    assert!(close(s.velocity[0], 2.4), "{:?}", s.velocity);
    assert!(s.position[1] > 0., "{:?}", s.position);
    // A request faster than the speed is steered toward with the steering
    // inertia.
    s.move_direction = [0., 0., 1.];
    s.move_speed = 10.;
    s.end_tick(&Flat);
    s.end_tick(&Flat);
    assert!(
        close(s.velocity[2], 5.) && close(s.velocity[0], 1.2),
        "{:?}",
        s.velocity
    );
}

#[test]
fn a_gravity_range_holds_a_jump_at_its_height_then_it_falls_from_there() {
    let mut s = Sysobj::bare();
    s.jump_now([0., 0., 1.], 60., 0.);
    for _ in 0..8 {
        s.end_tick(&Flat);
    }
    // Range 3 (gravity frozen) from here on.
    s.motion.flags.word |= motion::flag::GRAVITY_FROZEN;
    s.end_tick(&Flat);
    let held = s.position[1];
    assert!(held > 10., "risen to {held}");
    for _ in 0..60 {
        s.end_tick(&Flat);
    }
    assert!(close(s.position[1], held), "held at {}", s.position[1]);
    s.motion.flags.word &= !motion::flag::GRAVITY_FROZEN;
    for _ in 0..4 {
        s.end_tick(&Flat);
    }
    let first = s.position[1];
    assert!(first < held && held - first < 5., "a soft fall: {first}");
    for _ in 0..200 {
        s.end_tick(&Flat);
        if !s.is_jump() {
            break;
        }
    }
    assert!(!s.is_jump() && close(s.position[1], 0.));
}

#[test]
fn a_leaving_object_is_removed_when_its_alpha_reaches_zero() {
    // obj_leave: alpha out at once, then removal on that frame.
    let mut s = Sysobj::bare();
    s.fade.start(1.0, 0.0, 0.0);
    s.left = true;
    s.end_tick(&Flat);
    assert!(s.gone);
    // A slower fade out keeps it until the alpha is 0.
    let mut s = Sysobj::bare();
    s.fade.start(1.0, 0.0, 4.0);
    s.left = true;
    for _ in 0..3 {
        s.end_tick(&Flat);
        assert!(!s.gone, "still fading at {}", s.fade.value);
    }
    s.end_tick(&Flat);
    assert!(s.gone);
    // Faded out without leaving: it stays.
    let mut s = Sysobj::bare();
    s.fade.start(1.0, 0.0, 0.0);
    s.end_tick(&Flat);
    assert!(!s.gone);
}

#[test]
fn a_queued_motion_starts_at_the_end_with_the_end_bit_still_set() {
    // FUN_1403c6740 sets +0x18c bit 1 after starting the queued motion, so
    // a script waiting on `motion_is_end` sees the first motion end.
    let mut s = Sysobj::bare();
    s.motion_start(191, 0., &Thirty);
    s.motion_queue.push((0, 0.));
    let mut seen = None;
    for tick in 0..40 {
        s.begin_tick(&Flat);
        if s.motion_ended() {
            seen = Some((tick, s.motion.slot));
            break;
        }
    }
    let (_, slot) = seen.expect("the end is seen");
    assert_eq!(slot, 0);
    // The next step clears it again.
    s.begin_tick(&Flat);
    assert!(!s.motion_ended());
}

#[test]
fn a_blend_starts_its_clock_at_two_and_holds_the_start_frame() {
    // FUN_1403c8a40 sets the clock to 2 for a blend over 2 ticks; each
    // advance (FUN_1403c6740) adds one, and the clock reaching the blend
    // lets the motion run from its start frame.
    let mut s = Sysobj::bare();
    s.motion_start(5, 8., &Thirty);
    assert_eq!(s.motion.blend_weight(), Some(0.25));
    for clock in 3..8 {
        s.begin_tick(&Flat);
        assert_eq!(s.motion.tick, 0);
        assert_eq!(s.motion.blend_weight(), Some(clock as f32 / 8.));
    }
    s.begin_tick(&Flat);
    assert_eq!(s.motion.blend_weight(), None);
    assert_eq!(s.motion.tick, 0);
    // The plain step from 0 belongs to the host's clips (0 -> 2 there).
    s.begin_tick(&Flat);
    assert_eq!(s.motion.tick, 1);
    // A blend of 2 or less starts at once.
    s.motion_start(6, 2., &Thirty);
    assert_eq!(s.motion.blend_weight(), None);
}

#[test]
fn walking_off_the_floor_carries_the_move_into_the_fall_capped_at_8() {
    // FUN_1403b9090's leave branch: velocity = this tick's move, its length
    // at most 8 (DAT_140623bac); the fall keeps it.
    let mut s = Sysobj::bare();
    s.position = [0., 100., 0.];
    s.move_direction = [1., 0., 0.];
    s.move_speed = 12.;
    s.end_tick(&crate::env::NullEnv);
    assert!(s.is_jump());
    let (_, v) = s.jump.expect("falling");
    assert!((v[0] - 8.).abs() < 1e-4 && v[1].abs() < 1e-4, "{v:?}");
}

#[test]
fn an_edge_refusing_object_does_not_step_where_the_probe_finds_no_floor() {
    // FUN_1403b9090 with FUN_1403bab20 / FUN_1403ba750: the move of 5 is
    // probed 0.2 x 150 further (to x = 125, past the floor's end at 100).
    let walk = |range_1d: bool| {
        let mut s = Sysobj::bare();
        s.map_body = Some(BODY);
        s.position = [90., 0., 0.];
        s.move_direction = [1., 0., 0.];
        s.move_speed = 5.;
        if range_1d {
            s.motion.flags.word |= 0x80_0000;
        }
        s.end_tick(&BodyFloor);
        s
    };
    let held = walk(true);
    assert!(close(held.position[0], 90.) && !held.is_jump());
    // Without range 0x1d the step is taken (the floor is still under it).
    let free = walk(false);
    assert!(close(free.position[0], 95.) && !free.is_jump());
}

struct TakeoffAtZero(crate::motion::MotionEvents);

impl Clips for TakeoffAtZero {
    fn length(&self, _: usize) -> u32 {
        30
    }
    fn loop_start(&self, _: usize) -> u32 {
        0
    }
    fn root_delta(&self, _: usize, _: u32) -> Vec3 {
        [0.; 3]
    }
    fn events(&self, _: usize) -> Option<&crate::motion::MotionEvents> {
        Some(&self.0)
    }
}

#[test]
fn an_unblended_start_fires_the_frame_events_at_frame_0() {
    // FUN_1403c8a40 -> FUN_14040e960(start, -1.0): a take-off at frame 0
    // (Dusk's fly-jump motion 155) fires as the motion starts; blended, it
    // waits for the blend's end.
    let clips = TakeoffAtZero(crate::motion::MotionEvents {
        ranges: vec![],
        frames: vec![crate::motion::FrameEvent {
            frame: 0,
            kind: 0,
            params: vec![],
        }],
    });
    let mut s = Sysobj::bare();
    s.motion_start(155, 0., &clips);
    assert!(s.effects.contains(&crate::motion::Effect::Takeoff));
    let mut s = Sysobj::bare();
    s.motion_start(155, 8., &clips);
    assert!(!s.effects.contains(&crate::motion::Effect::Takeoff));
}
#[test]
fn turning_takes_over_the_wish_and_lands_on_the_heading() {
    // FUN_1403b9f70 -> FUN_1403b6560: an older wish does not pull against the
    // turn, and within reach the heading is the target's.
    let mut s = Sysobj::bare();
    s.turn_rate = 0.1;
    s.wish_yaw = Some(-1.);
    assert!(s.turn_dir([1., 0., 0.], 0.));
    assert!((s.yaw - 0.2).abs() < 1e-6);
    assert_eq!(s.wish_yaw, Some(s.yaw));
    s.yaw = std::f32::consts::FRAC_PI_2 - 0.1;
    assert!(!s.turn_dir([1., 0., 0.], 0.));
    assert!((s.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
}
#[test]
fn a_fly_jump_plays_its_own_motions_and_goes_back_to_flying() {
    // M_EX890's `obj_flyjump(v, 3, 0, 1)`: fall on 4 near the apex
    // (FUN_1403cb610), land on 5 (FUN_1403cb2a0), back in mode 5 (+0x108).
    let mut s = Sysobj::bare();
    let steer = Steer {
        inertia: 0.9,
        hover: false,
    };
    s.steer = Some(steer);
    s.airborne = true;
    s.position = [0., 300., 0.];
    s.motion_start(3, 0., &Thirty);
    s.launch_jump([0., 0., 1.], 100., 0.);
    s.pending_mode = 3;
    s.pending_fly = Some(PendingFly {
        direction: [0., 0., 1.],
        height: 100.,
        speed: 0.,
    });
    s.jump_motions = JumpMotions { fall: 4, land: 5 };
    s.effects.clear();
    s.take_off(&Flat);
    assert!(s.is_jump());
    let mut fell = false;
    for _ in 0..600 {
        s.end_tick(&Flat);
        fell |= s.motion.slot == 4;
        if s.fly.is_none() {
            break;
        }
    }
    assert!(fell);
    assert!(s.fly.is_none());
    assert_eq!(s.motion.slot, 5);
    assert_eq!(s.steer, Some(steer));
    assert_ne!(s.flags & 0x800_0000, 0);
}
#[test]
fn mode_7_rises_straight_turns_to_156_and_lands_on_157() {
    // FUN_1404229f0 / FUN_140422af0 / FUN_140422b30.
    let mut s = Sysobj::bare();
    s.motion_start(155, 0., &Thirty);
    s.launch_jump([1., 0., 0.], 60., 4.);
    s.enter_mode7();
    assert!(s.is_jump() && s.mode7);
    let mut rose_straight = true;
    let mut saw_156 = false;
    for _ in 0..300 {
        s.end_tick(&Flat);
        if let Some((arc, _)) = s.jump
            && arc.t + actor_physics::PHYSICS_STEP < arc.rise
            && s.position[0] != 0.
        {
            rose_straight = false;
        }
        saw_156 |= s.motion.slot == 156;
        if !s.is_jump() {
            break;
        }
    }
    assert!(rose_straight, "no horizontal move while it rises");
    assert!(saw_156);
    assert!(!s.is_jump() && !s.mode7);
    assert_eq!(s.motion.slot, 157);
    assert_ne!(s.flags & 0x800_0000, 0);
    assert!(s.position[0] > 0., "it moved out once past the apex");
}

#[test]
fn a_hook_changes_only_the_motion_started_after_it() {
    let mut s = Sysobj::bare();
    s.model = 7;
    assert!(s.motion_start(3, 0., &Thirty));
    assert_eq!(s.playing_model(), 7);
    s.set_motion_hook(Some(161));
    assert_eq!(s.motion.table, None, "the playing motion keeps its table");
    assert_eq!(model_entry(s.model), 7);
    assert_eq!(s.model >> MODEL_ENTRY_BITS, 162);
    assert_eq!(s.playing_model(), 7);
    assert!(s.motion_start(4, 0., &Thirty));
    assert_eq!(s.motion.table, Some(161));
    assert_eq!(s.playing_model(), s.model);
    // An unhook leaves the playing motion in the table too.
    s.set_motion_hook(None);
    assert_eq!(s.model, 7);
    assert_eq!(s.motion.table, Some(161));
    assert_eq!(s.playing_model() >> MODEL_ENTRY_BITS, 162);
}
