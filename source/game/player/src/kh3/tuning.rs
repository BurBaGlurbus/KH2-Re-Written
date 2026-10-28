//! The KH3 ruleset's numbers. The behaviour reference (StrawHatPro's MIT
//! reimplementation of KH3's Sora, CREDITS.md) gives centimetres, seconds
//! and degrees for a Sora 180 tall; this controller runs 60 ticks a second
//! in KH2's units, where Sora stands about 160. Every length is scaled by
//! [`SCALE`], so the moves keep their size against Sora's body and their
//! timing. Values the reference does not give are marked [duel choice].

/// KH2 Sora's height over KH3 Sora's.
pub const SCALE: f32 = 160. / 180.;

/// cm/s to units per tick.
const fn speed(cm_s: f32) -> f32 {
    cm_s * SCALE / 60.
}

/// cm/s² to units per tick².
const fn accel(cm_s2: f32) -> f32 {
    cm_s2 * SCALE / 3600.
}

/// Seconds to ticks.
const fn ticks(s: f32) -> u32 {
    (s * 60. + 0.5) as u32
}

/// cm to units.
const fn length(cm: f32) -> f32 {
    cm * SCALE
}

/// cm to units, for the layer's own lengths.
pub const fn length_of(cm: f32) -> f32 {
    length(cm)
}

// Gravity: -980 x 1.5, times the phase's multiplier.
pub const GRAVITY: f32 = accel(1470.);
/// Rising with the jump button held (x2.8), and after it is released (the
/// short hop: the scale 3.0 x 2.8).
pub const GRAVITY_RISE: f32 = accel(4116.);
pub const GRAVITY_CUT: f32 = accel(8232.);
/// The hang after the apex of a jump: x0.1 for 0.15 s (8 ticks).
pub const GRAVITY_HANG: f32 = accel(147.);
pub const HANG_TICKS: u32 = 8;
pub const FALL_MAX: f32 = speed(4000.);

/// Jump powers (vertical speed at take-off): normal, High Jump,
/// Doubleflight and the flow (super) jump.
pub const JUMP: f32 = speed(1480.);
pub const HIGH_JUMP: f32 = speed(1950.);
pub const DOUBLEFLIGHT: f32 = speed(2300.);
pub const SUPER_JUMP: f32 = speed(2500.);
/// A ground jump leaves on the 5th tick after the press; Doubleflight on
/// the 4th.
pub const JUMP_TAKEOFF: u32 = 4;
pub const DOUBLEFLIGHT_TAKEOFF: u32 = 3;
/// Doubleflight is taken once the vertical speed is at most this.
pub const DOUBLEFLIGHT_BELOW: f32 = speed(500.);
/// The flow jump's push along its launch, decaying x0.92 a tick.
pub const SUPER_PUSH: f32 = speed(175.);
pub const SUPER_PUSH_DECAY: f32 = 0.92;

// Air control.
pub const AIR_MAX: f32 = speed(680.);
pub const AIR_ACCEL: f32 = accel(3672. * 0.95);
pub const AIR_BRAKE: f32 = accel(2048.);
/// Facing toward the stick in the air (6·dt) and on the ground (0.1·dt·60),
/// shares of the remaining turn a tick.
pub const AIR_TURN: f32 = 0.1;
pub const RUN_TURN: f32 = 0.1;

// The ground.
pub const WALK: f32 = speed(140.);
pub const RUN: f32 = speed(680.);
pub const SPRINT: f32 = speed(980.);
pub const SPRINT_AFTER: u32 = ticks(1.0);
/// Stick magnitudes: active, and the walk / run split.
pub const STICK_ACTIVE: f32 = 0.1;
pub const RUN_STICK: f32 = 0.8;
/// Braking with the stick released: friction 16 /s and 2048 cm/s².
pub const BRAKE_FRICTION: f32 = 16. / 60.;
pub const BRAKE: f32 = accel(2048.);
/// A landing: commands again after 0.0991 s, a jump from the second tick.
pub const LAND_CONTROL: u32 = 6;
pub const LAND_JUMP: u32 = 2;

/// Dodge Roll: the root curve's 30 fps keys (cm), 0.7 s, invulnerable
/// until 0.4659 s, when it can be cancelled.
pub const ROLL_KEYS: [f32; 21] = [
    0., 45.6, 109.8, 173.5, 234.3, 289.1, 337.0, 378.3, 416.1, 449.8, 479.1, 504.5, 526.6, 545.7,
    561.9, 575.2, 586.0, 593.8, 598.5, 600., 600.,
];
pub const ROLL_TICKS: u32 = ticks(0.7);
pub const ROLL_CANCEL: u32 = ticks(0.4659);

/// Air Slide (level 1): 1130 cm x 0.725 over 0.7333 s, front-loaded (145
/// cm in the first 1/30 s), invulnerable throughout, cancelled at 0.5 s;
/// 0.6 s before the next once it ends in the air.
pub const SLIDE_DISTANCE: f32 = length(1130. * 0.725);
pub const SLIDE_TICKS: u32 = ticks(0.7333);
pub const SLIDE_CANCEL: u32 = ticks(0.5);
pub const SLIDE_COOLDOWN: u32 = ticks(0.6);
/// The slide's curve: the share covered by tick t of n is
/// 1 - (1 - t/n)^k, k fitted to the first 1/30 s [duel choice].
pub const SLIDE_CURVE: f32 = 4.2;

/// Glide: 1500 cm/s at a gravity scale of 0.001, the facing 3 % a tick
/// toward the stick, travel along the facing.
pub const GLIDE: f32 = speed(1500.);
pub const GLIDE_FIRST: f32 = speed(1425.);
pub const GLIDE_SINK: f32 = accel(1470. * 0.001);
pub const GLIDE_TURN: f32 = 0.03;

/// Superslide: 4000 cm/s losing 1000 a second for 1 s, then the slide's
/// end (426 cm over 0.7667 s); steering 2.5 % a tick; walls and poles from
/// 0.1 s.
pub const SUPERSLIDE: f32 = speed(4000.);
pub const SUPERSLIDE_DECAY: f32 = accel(1000.);
pub const SUPERSLIDE_TICKS: u32 = ticks(1.0);
pub const SLIDE_END_DISTANCE: f32 = length(426.);
pub const SLIDE_END_TICKS: u32 = ticks(0.7667);
pub const SUPERSLIDE_TURN: f32 = 0.025;
pub const SUPERSLIDE_CONTACT: u32 = ticks(0.1);
/// A Superslide after a pole aims at a target within 2500 cm and ~50°.
pub const SUPERSLIDE_AIM: f32 = length(2500.);
pub const SUPERSLIDE_AIM_COS: f32 = 0.643;

/// Wall run: up or along the wall at 1251 cm/s, drifting sideways at 980
/// in an up-run; up within 35° of the stick into the wall, along at
/// 50°-130°; it ends 0.2 s after the stick lets go.
pub const WALL_RUN: f32 = speed(1251.);
pub const WALL_DRIFT: f32 = speed(980.);
pub const WALL_UP_COS: f32 = 0.819; // cos 35°
pub const WALL_SIDE_COS: f32 = 0.643; // cos 50°
pub const WALL_GRACE: u32 = ticks(0.2);
pub const WALL_STICK: f32 = 0.5;
/// The roll over the top edge rises 495 cm.
pub const WALL_TOP_RISE: f32 = length(495.);
/// [duel choice] An up-run longer than this kicks off (the reference has no
/// timer; an arena wall must not hold a Sora out of reach).
pub const WALL_RUN_MAX: u32 = ticks(3.0);

/// Wall kick (straight): 12 frames on the wall, then 80 cm out and a turn;
/// commands from 0.1 s, the change at 0.4674 s, 0.5 s long.
pub const KICK_HOLD: u32 = ticks(0.4);
pub const KICK_OUT: f32 = length(80.);
pub const KICK_JUMP: u32 = ticks(0.1);
pub const KICK_TICKS: u32 = ticks(0.5);

/// Pole spin: grabbed within 100 cm of a point 120 ahead and 90 up; a 0.5 s
/// attach, then four 0.3 s turns.
pub const POLE_REACH: f32 = length(100.);
pub const POLE_AHEAD: f32 = length(120.);
pub const POLE_UP: f32 = length(90.);
pub const POLE_ATTACH: u32 = ticks(0.5);
pub const POLE_TURN: u32 = ticks(0.3);
pub const POLE_TURNS: u32 = 4;
/// [duel choice] Sora's distance from the pole's surface while spinning.
pub const POLE_GAP: f32 = length(55.);

/// Airstep (Athletic Flow): legs of 1 s at max(2d/1 s, min(60 d, 6000))
/// cm/s, 3000 at most inside 600 cm, arriving inside 200 cm; the hop back
/// after (150 back, 96 up over 0.6333 s, commands from 0.1 s, change at
/// 0.2667 s). Range 3750 cm.
pub const AIRSTEP_RANGE: f32 = length(3750.);
pub const AIRSTEP_LEG: f32 = 60.;
pub const AIRSTEP_FAST: f32 = 6000.;
pub const AIRSTEP_NEAR: f32 = 600.;
pub const AIRSTEP_NEAR_SPEED: f32 = 3000.;
pub const AIRSTEP_ARRIVE: f32 = length(200.);
pub const AIRSTEP_TURN: f32 = 0.2;
pub const HOP_BACK: f32 = length(150.);
pub const HOP_UP: f32 = length(96.);
pub const HOP_TICKS: u32 = ticks(0.6333);
pub const HOP_INPUT: u32 = ticks(0.1);
pub const HOP_CHANGE: u32 = ticks(0.2667);
/// [duel choice] An airstep that has not arrived after this gives up.
pub const AIRSTEP_MAX: u32 = ticks(1.5);

/// Attack homing ("traction"): 800 cm/s toward the target when it is
/// within 800 cm and beyond 160, easing 20 % a tick (the first air attack
/// of an air combo 1200 cm/s, 30 %). Air-mode attacks hover and close the
/// height at 3x the difference (clamped to 500 cm) per second, then fall
/// at 0.1 of gravity.
pub const TRACTION: f32 = speed(800.);
pub const TRACTION_AIR_FIRST: f32 = speed(1200.);
pub const TRACTION_RATE: f32 = 0.2;
pub const TRACTION_RATE_AIR_FIRST: f32 = 0.3;
pub const TRACTION_STOP: f32 = length(160.);
/// [duel choice] The counter and the launch home to this close: their KH2
/// stand-in records reach less far than the reference's swings.
pub const CLOSE_STOP: f32 = length(60.);
pub const TRACTION_RANGE: f32 = length(800.);
/// [duel choice] The traction window of a KH2 swing: the reference's
/// windows run about 0.1-0.33 s into a swing.
pub const TRACTION_FROM: u32 = ticks(0.1);
pub const TRACTION_TO: u32 = ticks(0.3333);
pub const HOVER_GAIN: f32 = 3.0;
pub const HOVER_CLAMP: f32 = length(500.);
pub const HOVER_FALL: f32 = accel(147.);
/// Sora's target point above his feet (the source's root + 100 is the
/// search origin; 90 cm puts it at his chest) [duel choice].
pub const CHEST: f32 = length(90.);

/// Launch (Rising Spiral): rises at 1000 cm/s for half a second, homing in
/// from 0.1 s to 0.6 s of the swing; once per chain.
pub const LAUNCH_RISE: f32 = speed(1000.);
pub const LAUNCH_FROM: u32 = ticks(0.1);
pub const LAUNCH_TO: u32 = ticks(0.6);
/// [duel choice] The rise starts once Upper Slash (its KH2 stand-in) has
/// struck, at 0.35 s, and keeps the reference's half second: rising from
/// 0.1 s, the record's hit would pass over the other Sora.
pub const LAUNCH_RISE_FROM: u32 = ticks(0.35);
pub const LAUNCH_END: u32 = ticks(0.85);

/// Flow attacks: Sonic Slash's dash (900 cm of root, scaled to the target)
/// and its height gain 4.5. [duel choice] The dash runs at 1500 cm/s for
/// at most 0.6 s.
pub const FLOW_SPEED: f32 = speed(1500.);
pub const FLOW_DASH: u32 = ticks(0.6);
pub const FLOW_GAIN: f32 = 4.5;
pub const FLOW_STOP: f32 = length(120.);

/// A successful guard opens the counter window for 1 s.
pub const COUNTER_WINDOW: u32 = ticks(1.0);

/// Focus: 100, aim burns 20 a second; nothing refills it by itself in the
/// reference (prizes do). [duel choice] With no prizes in a duel, each hit
/// dealt gives 3 (a small prize), each hit taken 5 (a large one), and the
/// gauge refills 1 a second.
pub const FOCUS_MAX: f32 = 100.;
pub const FOCUS_AIM: f32 = 20. / 60.;
pub const FOCUS_HIT_DEALT: f32 = 3.;
pub const FOCUS_HIT_TAKEN: f32 = 5.;
pub const FOCUS_REGEN: f32 = 1. / 60.;

/// Shotlock aim: the button held 0.2667 s, then 0.1 s before the first
/// lock; 24 locks, one every 2.24 / 24 s, on a target within 5000 cm.
pub const AIM_HOLD: u32 = ticks(0.2667);
pub const AIM_DELAY: u32 = ticks(0.1);
pub const MAX_LOCKS: u8 = 24;
pub const LOCK_INTERVAL: f32 = 2.24 * 60. / 24.;
pub const AIM_RANGE: f32 = length(5000.);

/// The volleys: lifted 500 cm above the floor, invulnerable; bolts spawn 7
/// a tick, ring out to 200 cm over 0.5 s, launch, and the state ends.
/// King of Hearts (1-23 locks): launch at 3.07 s, end 4.0 s, 1500 cm/s,
/// homing from 0.0333 s at 180°/s rising 360°/s² to 360°/s. Ragnarok (24
/// locks): launch 2.9 s, end 4.1333 s, 1000 cm/s + 500 cm/s² to 2000,
/// homing 720°/s. Homing stops inside 100 cm; bolts live 5 s / 8 s.
pub const VOLLEY_LIFT: f32 = length(500.);
/// [duel choice] The lift takes this long (the reference snaps in a tick).
pub const VOLLEY_LIFT_TICKS: u32 = 8;
pub const VOLLEY_PER_TICK: u8 = 7;
pub const RING_RADIUS: f32 = length(200.);
pub const RING_OPEN: u32 = ticks(0.5);
pub const RING_SPIN: f32 = std::f32::consts::PI / 60.;
pub const KOH_LAUNCH: u32 = ticks(3.07);
pub const KOH_END: u32 = ticks(4.0);
pub const KOH_SPEED: f32 = speed(1500.);
pub const KOH_HOMING_FROM: u32 = 2;
pub const KOH_TURN: f32 = std::f32::consts::PI / 60.;
pub const KOH_TURN_RISE: f32 = 2. * std::f32::consts::PI / 3600.;
pub const KOH_TURN_MAX: f32 = 2. * std::f32::consts::PI / 60.;
pub const KOH_LIFE: u32 = ticks(5.0);
pub const RAGNAROK_LAUNCH: u32 = ticks(2.9);
pub const RAGNAROK_END: u32 = ticks(4.1333);
pub const RAGNAROK_SPEED: f32 = speed(1000.);
pub const RAGNAROK_ACCEL: f32 = accel(500.);
pub const RAGNAROK_MAX: f32 = speed(2000.);
pub const RAGNAROK_TURN: f32 = 4. * std::f32::consts::PI / 60.;
pub const RAGNAROK_LIFE: u32 = ticks(8.0);
pub const HOMING_OFF: f32 = length(100.);
/// [duel choice] One volley takes at most this share of the target's max
/// HP (the reference's volleys are tuned against enemies).
pub const VOLLEY_CAP_PERCENT: i32 = 35;

/// Grand magic: the style gauge (0-100) fed by hits (Fire 15, Blizzard 12,
/// Thunder and the rest 6, a swing 4: the reference's swing rows give 1-8)
/// and decaying 5 a second after 20 s without gain; the offer lasts 20 s.
pub const STYLE_MAX: f32 = 100.;
pub const STYLE_SWING: f32 = 4.;
pub const STYLE_FIRE: f32 = 15.;
pub const STYLE_BLIZZARD: f32 = 12.;
pub const STYLE_OTHER: f32 = 6.;
pub const STYLE_IDLE: u32 = ticks(20.);
pub const STYLE_DECAY: f32 = 5. / 60.;
pub const OFFER_TICKS: u32 = ticks(20.);
/// The grand rows' power over the Firaga-tier ones (4.4 / 2.2).
pub const GRAND_DAMAGE: f32 = 2.;

/// The presentation's lean eases this share a tick toward its target.
pub const LEAN_EASE: f32 = 0.25;

// Hit reactions (the reference's player damage parameters). Damage motions
// fall at -980 cm/s² (gravity scale 1); a tossed body rises at x3 and
// hangs at x0.25 for 0.1 s after the apex.
pub const HURT_GRAVITY: f32 = accel(980.);
pub const HURT_RISE_GRAVITY: f32 = accel(2940.);
pub const HURT_APEX_GRAVITY: f32 = accel(245.);
pub const HURT_APEX_TICKS: u32 = ticks(0.1);
/// A flinch: 550 cm/s x the row's scale along the hit, x0.9 a frame on the
/// ground; in the air gravity waits while air braking (2048 cm/s²) brings
/// it under 150 cm/s. Acting again at 0.4667 s; the ground clip ends at
/// 1.1667 s, the air one at 0.6 s.
pub const SMALL_KNOCK: f32 = speed(550.);
pub const SMALL_DECAY: f32 = 0.9;
pub const SMALL_AIR_BRAKE: f32 = accel(2048.);
pub const SMALL_AIR_STOP: f32 = speed(150.);
pub const SMALL_ACT: u32 = ticks(0.4667);
pub const SMALL_END: u32 = ticks(1.1667);
pub const SMALL_AIR_END: u32 = ticks(0.6);
/// A blow: 2000 cm/s x scale along the hit and 300 up, x0.95 a frame;
/// gravity once under 700 cm/s.
pub const BLOW_SPEED: f32 = speed(2000.);
pub const BLOW_UP: f32 = speed(300.);
pub const BLOW_DECAY: f32 = 0.95;
pub const BLOW_GRAVITY_BELOW: f32 = speed(700.);
/// Toss and launch: straight up at 1500 / 2500 cm/s x scale.
pub const TOSS_UP: f32 = speed(1500.);
pub const LAUNCH_UP: f32 = speed(2500.);
/// A launch ends in the air after 1.2 s of falling.
pub const LAUNCH_FALL_END: u32 = ticks(1.2);
/// A parabola: 1500 cm/s x scale at 45°, the horizontal x0.98 a frame
/// while above 300 cm/s; acting again after 0.6 s.
pub const PARABOLA: f32 = speed(1500. * std::f32::consts::FRAC_1_SQRT_2);
pub const PARABOLA_DECAY: f32 = 0.98;
pub const PARABOLA_KEEP: f32 = speed(300.);
pub const PARABOLA_ACT: u32 = ticks(0.6);
/// A beat: 2500 cm/s straight down, sliding out from under the attacker
/// [duel choice: the slide at a flinch's speed].
pub const BEAT_DOWN: f32 = speed(2500.);
/// Knocked down: acting again 0.6 s after the landing, up at 0.93 s.
pub const DOWN_ACT: u32 = ticks(0.6);
pub const DOWN_END: u32 = ticks(0.93);
/// Aerial Recovery: Jump from 0.2 s into a blow, toss, launch or
/// parabola; invulnerable for its 0.5 s, moves again from 0.2667 s.
pub const RECOVERY_FROM: u32 = ticks(0.2);
pub const RECOVERY_TICKS: u32 = ticks(0.5);
pub const RECOVERY_CHANGE: u32 = ticks(0.2667);
/// For 0.1 s after a command starts, hits deal damage but no flinch.
pub const COMMAND_ARMOUR: u32 = ticks(0.1);
/// A guarded hit pushes back at 300 cm/s (light) or 1000 cm/s (heavy) x the
/// row's scale. KH2's knock-slide takes ten times its first tick's step.
pub const GUARD_PUSH: [f32; 2] = [speed(300.) * 10., speed(1000.) * 10.];
/// [duel choice] The reference has no invulnerability while down or getting
/// up; a duel keeps KH2's (the knock-down's own frames, and this long after
/// getting up), so a downed Sora cannot be hit over and over.
pub const WAKE_GUARD: u32 = ticks(0.25);

/// Final Blow: a counter that leaves the other Sora blown away offers the
/// chase for the next attack; after 0.2 s he is put 150 cm short of the
/// target and 100 cm under it. [duel choice] The offer lasts 1 s.
pub const CHASE_OFFER: u32 = ticks(1.0);
pub const CHASE_WARP: u32 = ticks(0.2);
pub const CHASE_SHORT: f32 = length(150.);
pub const CHASE_UNDER: f32 = length(100.);

/// Risk Dodge [duel choice: the reference only offers it from an enemy's
/// marked attack; here guarding a heavy hit (a finisher's blow or harder)
/// offers it for 0.5 s]: Jump slides under to behind the attacker (800 cm
/// of root scaled to end behind him, clamped to 300-1000 cm, a 180° turn,
/// moves again at 0.4667 s, invulnerable throughout); an attacker in the
/// air is warped behind instead after 0.2 s. A counter is offered for 2 s
/// after.
pub const RISK_OFFER: u32 = ticks(0.5);
pub const RISK_SLIDE_TICKS: u32 = ticks(0.4667);
pub const RISK_MIN: f32 = length(300.);
pub const RISK_MAX: f32 = length(1000.);
pub const RISK_BEHIND: f32 = length(150.);
pub const RISK_WARP: u32 = ticks(0.2);
pub const RISK_COUNTER: u32 = ticks(2.0);

/// Focus Syphon (shotlock kind 36): with the focus gauge emptied (until it
/// is full again) the shotlock takes up to 18 locks, each after 0.0002 s
/// per cm to the target, and refills focus by trunc(1.4 x locks); no
/// damage.
pub const SYPHON_LOCKS: u8 = 18;
pub const SYPHON_PER_CM: f32 = 0.0002 * 60.;
pub const SYPHON_RATE: f32 = 1.4;
/// [duel choice] The syphon's pull plays this long before the focus comes.
pub const SYPHON_TICKS: u32 = ticks(0.6);

/// The lock-on camera: from Sora toward the target with a -20° pitch
/// (none when the target is more than 10° below), pitch -70..+20, at least
/// 400 cm back or far enough to frame both, eased at 8 a second; 0.5 s
/// blend in.
pub const LOCK_PITCH: f32 = -20. * std::f32::consts::PI / 180.;
pub const LOCK_BELOW: f32 = -10. * std::f32::consts::PI / 180.;
pub const LOCK_PITCH_MIN: f32 = -70. * std::f32::consts::PI / 180.;
pub const LOCK_PITCH_MAX: f32 = 20. * std::f32::consts::PI / 180.;
pub const LOCK_DISTANCE: f32 = length(400.);
pub const LOCK_EASE: f32 = 8. / 60.;
pub const LOCK_BLEND: u32 = ticks(0.5);
/// The look point, 150 cm above Sora's feet.
pub const LOCK_LOOK_UP: f32 = length(150.);
