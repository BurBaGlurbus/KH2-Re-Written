//! Ordinary Dusk decisions and action continuations. Source motion flags,
//! ranges, collision response and host status are supplied by the caller.
//! This translation does not qualify native timing, effects or boss variants.

use crate::actor_physics;
use crate::combat::{self, AttackParams, AttackerStats, TargetStats};
use crate::shadow::ScriptRandom;

pub type Vec3 = [f32; 3];
const QUEUE_CAPACITY: usize = 15;

pub mod motion {
    pub const IDLE: usize = 0;
    pub const APPEAR: usize = 44;
    pub const LEAVE: usize = 45;
    pub const FOOTWORK: usize = 47;
    pub const FRONT: usize = 151;
    pub const BACK: usize = 153;
    pub const FLY_JUMP: usize = 155;
    pub const STEP: [usize; 2] = [163, 164];
    pub const DOWN: [usize; 2] = [181, 182];
    pub const CLAW: usize = 201;
    pub const KICK: usize = 202;
    pub const WALK_ATTACK: [usize; 4] = [203, 204, 205, 206];
    pub const FIGHT: usize = 233;
    pub const ESCAPE: usize = 235;
}

/// The host selects an explicit motion-clock/root evaluation profile. Ranges
/// must come from the source ANB events; absence is an error, not an empty range.
pub trait Clips {
    fn length(&self, slot: usize) -> u32;
    fn loop_start(&self, slot: usize) -> u32;
    fn root(&self, slot: usize, tick: u32) -> Vec3;
    /// Motion clock producer. The native host overrides the initial 0 -> 2
    /// special step; the default is only a synthetic inspection clock.
    fn advance(&self, _slot: usize, tick: u32) -> u32 {
        tick.saturating_add(1)
    }
    /// Root displacement over a clock step, including an admitted loop
    /// continuation. Override when stored roots wrap to avoid a reverse jump.
    fn root_delta(&self, slot: usize, tick: u32) -> Vec3 {
        sub(
            self.root(slot, self.advance(slot, tick)),
            self.root(slot, tick),
        )
    }
    fn range(&self, slot: usize, tick: u32, range: u8) -> Option<bool>;
    fn takeoff(&self, _slot: usize, _tick: u32) -> Option<bool> {
        None
    }
    fn gravity_frozen(&self, _slot: usize, _tick: u32) -> Option<bool> {
        None
    }
    fn ended(&self, slot: usize, tick: u32) -> bool {
        tick >= self.length(slot)
    }
    fn in_loop(&self, slot: usize, tick: u32) -> bool {
        tick >= self.loop_start(slot)
    }
}

/// Host queries and collision movement. `movement` returns the admitted final
/// position, permitting walls and floor to affect source movement ratios.
pub trait Env {
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32>;
    fn movement(&self, from: Vec3, wished: Vec3) -> Vec3 {
        let mut to = [0, 1, 2].map(|k| from[k] + wished[k]);
        if let Some(y) = self.floor(to[0], to[2], from[1].max(to[1]) + 200.) {
            to[1] = to[1].max(y);
        }
        to
    }
    /// Exact engine step-check result (0 clear, 1/2 jump, 3 blocked).
    /// A flat debug arena may explicitly provide 0.
    fn check_step(&self, _position: Vec3, _direction: Vec3) -> Option<u8> {
        None
    }
}

/// Engine predicates not implied by distance or a motion name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub target: Vec3,
    pub target_step: Vec3,
    pub player: Vec3,
    pub appear_way: i32,
    pub entry_fly: bool,
    pub no_control: bool,
    pub event_running: bool,
    pub tutorial_open: bool,
    pub force_leave: bool,
    pub no_leave: bool,
    pub culled: bool,
    /// Object movement turn parameter; defaults are native ObjParams values.
    pub turn_rate: f32,
}

impl Input {
    pub fn arena(target: Vec3, player: Vec3, turn_rate: f32) -> Self {
        Self {
            target,
            target_step: target,
            player,
            appear_way: 2,
            entry_fly: false,
            no_control: false,
            event_running: false,
            tutorial_open: false,
            force_leave: false,
            no_leave: true,
            culled: false,
            turn_rate,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActKind {
    Appear,
    ModeBattle,
    ModeRevenge,
    BattleAttack,
    BattleShort,
    BattleLong,
    RevengeShort,
    RevengeLong,
    Idle,
    IdleTime,
    Footwork,
    MoveTarget,
    MoveStep,
    Jump,
    MoveFront,
    MoveBack,
    Fight,
    Escape,
    MoveDown,
    Claw,
    Kick,
    WalkAttack,
    Leave,
    Dead,
}

impl ActKind {
    fn locked(self) -> bool {
        matches!(self, Self::Leave | Self::Dead)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    MissingMotion(usize),
    MissingRange {
        slot: usize,
        range: u8,
    },
    MissingStepQuery,
    UnsupportedStep(u8),
    /// A defensive bound, not a native scheduling delay. It stops visibly.
    NonYieldingActions,
    UnsupportedCapture,
    MissingTakeoff(usize),
    MissingGravityFlags(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    None,
    Flinch,
    Blow { back: bool },
    BlowLand,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Action {
    kind: ActKind,
    stage: u8,
    timer: f32,
    direction: Vec3,
    value: f32,
    alternate: usize,
}

impl Action {
    fn new(kind: ActKind) -> Self {
        Self {
            kind,
            stage: 0,
            timer: 0.,
            direction: [0.; 3],
            value: 0.,
            alternate: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PendingFly {
    direction: Vec3,
    height: f32,
    speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Fly {
    arc: actor_physics::Arc,
    velocity: [f32; 2],
    initial_height: f32,
    original_airborne: bool,
    fall_started: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dusk {
    pub position: Vec3,
    pub spawn: Vec3,
    pub yaw: f32,
    pub hp: i32,
    pub stats: TargetStats,
    pub motion: usize,
    pub motion_tick: u32,
    pub motion_serial: u64,
    pub blend: f32,
    blend_elapsed: u32,
    blend_duration: u32,
    pub scale_y: f32,
    pub alpha: f32,
    pub gone: bool,
    pub airborne: bool,
    pub reaction: Reaction,
    pub fault: Option<Fault>,
    /// Source duration supplied by initialization (ordinary path: 30).
    pub death_duration: f32,
    pub close_distance: f32,
    pub short_distance: f32,
    pub leave_distance: f32,
    pub culled_leave_distance: f32,
    pub wished: Vec3,
    pub moved: Vec3,
    sstm: actor_physics::Sstm,
    action: Option<Action>,
    queue: Vec<ActKind>,
    blow: Option<actor_physics::Blow>,
    knock: Option<actor_physics::Knock>,
    sampler: actor_physics::Sampler,
    record: combat::HitRecord,
    desired_yaw: Option<f32>,
    pending_fly: Option<PendingFly>,
    fly: Option<Fly>,
    wish_direction: Vec3,
    wish_speed: f32,
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [0, 1, 2].map(|k| a[k] - b[k])
}
fn hlen(v: Vec3) -> f32 {
    v[0].hypot(v[2])
}
fn unit(v: Vec3) -> Vec3 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0. { v.map(|x| x / l) } else { [0.; 3] }
}
fn horizontal(v: Vec3) -> Vec3 {
    unit([v[0], 0., v[2]])
}

impl Dusk {
    pub fn new(position: Vec3, yaw: f32, stats: TargetStats, sstm: actor_physics::Sstm) -> Self {
        let mut actor = Self {
            position,
            spawn: position,
            yaw,
            hp: stats.max_hp,
            stats,
            motion: motion::IDLE,
            motion_tick: 0,
            motion_serial: 0,
            blend: 0.,
            blend_elapsed: 0,
            blend_duration: 0,
            scale_y: 1.,
            alpha: 1.,
            gone: false,
            airborne: false,
            reaction: Reaction::None,
            fault: None,
            death_duration: 30.,
            close_distance: 350.,
            short_distance: 650.,
            leave_distance: 2000.,
            culled_leave_distance: 1200.,
            wished: [0.; 3],
            moved: [0.; 3],
            sstm,
            action: None,
            queue: Vec::new(),
            blow: None,
            knock: None,
            sampler: actor_physics::Sampler::default(),
            record: combat::HitRecord::new(false),
            desired_yaw: None,
            pending_fly: None,
            fly: None,
            wish_direction: [0.; 3],
            wish_speed: 0.,
        };
        actor.start(ActKind::Appear);
        actor.push(ActKind::ModeBattle);
        actor
    }

    pub fn alive(&self) -> bool {
        !self.gone && self.hp > 0 && !self.action.is_some_and(|a| a.kind.locked())
    }
    pub fn targetable(&self) -> bool {
        self.alive() && self.fault.is_none()
    }
    pub fn current_act(&self) -> Option<ActKind> {
        self.action.map(|a| a.kind)
    }
    pub fn queued_acts(&self) -> &[ActKind] {
        &self.queue
    }
    pub fn record(&mut self) -> &mut combat::HitRecord {
        &mut self.record
    }
    pub fn defeat(&mut self) {
        if self.alive() {
            self.hp = 0;
            self.fault = None;
            self.start(ActKind::Dead);
        }
    }
    /// Explicit debug command only; no automatic invented respawn delay.
    pub fn respawn(&mut self) {
        *self = Self::new(self.spawn, self.yaw, self.stats, self.sstm);
    }

    fn start(&mut self, kind: ActKind) {
        if self.action.is_some_and(|a| a.kind.locked()) {
            return;
        }
        self.queue.clear();
        self.action = Some(Action::new(kind));
    }
    fn push(&mut self, kind: ActKind) {
        if self.action.is_some_and(|a| a.kind.locked()) || self.queue.len() == QUEUE_CAPACITY {
            return;
        }
        self.queue.push(kind);
    }
    fn clear(&mut self) {
        self.action = None;
        self.queue.clear();
    }
    fn play(&mut self, slot: usize, blend: f32, clips: &impl Clips) -> bool {
        if clips.length(slot) == 0 {
            self.fault = Some(Fault::MissingMotion(slot));
            return false;
        }
        self.motion = slot;
        self.motion_tick = 0;
        self.motion_serial = self.motion_serial.wrapping_add(1);
        // Native motion start takes the immediate path at blend <= 2.
        // The script's 0.35 remains in motion-clock units, without x60.
        self.blend = if blend <= 2. { 0. } else { blend };
        self.blend_duration = self.blend as u32;
        self.blend_elapsed = 0;
        true
    }
    fn face(&mut self, v: Vec3, _rate: f32) {
        if hlen(v) > 0. {
            self.desired_yaw = Some(v[0].atan2(v[2]));
        }
    }
    fn face_immediate(&mut self, v: Vec3) {
        if hlen(v) > 0. {
            self.yaw = v[0].atan2(v[2]);
            self.desired_yaw = None;
        }
    }
    fn range(&mut self, clips: &impl Clips, range: u8) -> bool {
        match clips.range(self.motion, self.motion_tick, range) {
            Some(value) => value,
            None => {
                self.fault = Some(Fault::MissingRange {
                    slot: self.motion,
                    range,
                });
                false
            }
        }
    }

    fn continue_action(&mut self, a: Action) -> bool {
        self.action = Some(a);
        false
    }

    pub fn hit(
        &mut self,
        direction: Vec3,
        high: bool,
        attack: &AttackParams,
        attacker: &AttackerStats,
        difficulty: combat::Difficulty,
    ) -> Option<i32> {
        if !self.targetable() {
            return None;
        }
        let state = combat::TargetState {
            hp: self.hp,
            difficulty,
            ..combat::TargetState::full(self.stats.max_hp)
        };
        let damage = combat::damage_with(attacker, attack, &self.stats, &state);
        self.hp = (self.hp - damage).max(0);
        let back = self.yaw.sin() * direction[0] + self.yaw.cos() * direction[2] >= 0.;
        let class = combat::reaction_class(attack.reaction);
        if class == 5 {
            self.fault = Some(Fault::UnsupportedCapture);
        } else if class == 4 {
            let [mut up, mut speed] = attack.knockback.map(f32::from);
            if up == 0. && speed == 0. {
                up = self.sstm.blow_up;
                speed = self.sstm.blow_speed;
            }
            let d = horizontal(direction);
            self.blow = Some(actor_physics::Blow::new(
                [d[0] * speed, up, d[2] * speed],
                &self.sstm,
            ));
            self.knock = None;
            self.sampler.reset();
            self.fly = None;
            self.pending_fly = None;
            self.airborne = true;
            self.reaction = Reaction::Blow { back };
            self.set_reaction_motion(if back { 18 } else { 16 });
        } else {
            let strength = self
                .sstm
                .knock_strength(attack.knockback[0], class, attack.reaction);
            self.knock = actor_physics::Knock::new(horizontal(direction), strength, 0.);
            self.reaction = Reaction::Flinch;
            self.set_reaction_motion(10 + (back as usize) * 2 + (!high) as usize);
        }
        if self.hp == 0 {
            self.start(ActKind::Dead);
        } else if class > 1 {
            self.start(ActKind::ModeRevenge);
        }
        Some(damage)
    }

    fn set_reaction_motion(&mut self, slot: usize) {
        self.motion = slot;
        self.motion_tick = 0;
        self.motion_serial = self.motion_serial.wrapping_add(1);
        self.blend = 0.;
    }

    /// Shared engine reflection response, using the caller's source ATKP
    /// motion and hit-back fields. Missing reaction clips fault on next tick.
    pub fn reflect(&mut self, reflect_motion: usize, hit_back: f32) {
        if !self.targetable() {
            return;
        }
        self.set_reaction_motion(reflect_motion);
        self.reaction = Reaction::Flinch;
        let strength = if hit_back < 0. {
            crate::shadow::DEFAULT_HIT_BACK
        } else {
            hit_back
        };
        self.knock = if strength > 0. {
            actor_physics::Knock::new([-self.yaw.sin(), 0., -self.yaw.cos()], strength, 0.)
        } else {
            None
        };
        self.clear();
        self.start(ActKind::ModeRevenge);
    }

    /// Advance a source-frame-time unit under the caller's admitted profile.
    /// No-control/reaction suspension does not discard queued continuations.
    pub fn tick(
        &mut self,
        input: Input,
        clips: &impl Clips,
        env: &impl Env,
        rng: &mut ScriptRandom,
    ) {
        if self.gone || self.fault.is_some() {
            return;
        }
        if clips.length(self.motion) == 0 {
            self.fault = Some(Fault::MissingMotion(self.motion));
            return;
        }
        let old_serial = self.motion_serial;
        let mut movement: Option<Vec3> = None;
        let reacting = self.reaction != Reaction::None && self.hp > 0;
        if reacting {
            self.update_reaction(clips, env);
        } else if !input.no_control || self.action.is_some_and(|a| a.kind.locked()) {
            // Source callbacks may start/finish successors before the next halt.
            // An invalid host with no yields faults instead of hanging the app.
            let mut yielded = false;
            for _ in 0..64 {
                if self.action.is_none() {
                    let next = if self.queue.is_empty() {
                        ActKind::ModeBattle
                    } else {
                        self.queue.remove(0)
                    };
                    self.action = Some(Action::new(next));
                }
                let action = self.action.expect("action installed");
                if self.run(action, input, clips, env, rng, &mut movement) {
                    yielded = true;
                    break;
                }
                if self.fault.is_some() || self.gone {
                    yielded = true;
                    break;
                }
            }
            if !yielded {
                self.fault = Some(Fault::NonYieldingActions);
            }
        }
        if !reacting
            && self.fault.is_none()
            && !input.no_control
            && let Some(desired) = self.desired_yaw
        {
            let delta = crate::follow_camera::wrap(desired - self.yaw);
            let step = input.turn_rate.max(0.) * 2.;
            self.yaw = crate::follow_camera::wrap(self.yaw + delta.clamp(-step, step));
        }
        if self.fault.is_none() && self.pending_fly.is_some() {
            match clips.takeoff(self.motion, self.motion_tick) {
                Some(true) => {
                    let launch = self.pending_fly.take().expect("pending launch");
                    let floor = env.floor(self.position[0], self.position[2], self.position[1]);
                    self.fly = Some(Fly {
                        arc: actor_physics::Arc::launch(self.position[1], launch.height),
                        velocity: [
                            launch.direction[0] * launch.speed,
                            launch.direction[2] * launch.speed,
                        ],
                        initial_height: floor
                            .map_or(0., |floor| (self.position[1] - floor).max(0.)),
                        original_airborne: self.airborne,
                        fall_started: false,
                    });
                    self.airborne = true;
                    self.sampler.reset();
                }
                Some(false) => {}
                None => self.fault = Some(Fault::MissingTakeoff(self.motion)),
            }
        }
        if self.fault.is_none() && self.fly.is_some() {
            self.update_fly(clips, env, old_serial);
        } else if !reacting && self.fault.is_none() {
            let root = if old_serial == self.motion_serial {
                clips.root_delta(self.motion, self.motion_tick)
            } else {
                [0.; 3]
            };
            let (sin, cos) = self.yaw.sin_cos();
            let wished = movement.unwrap_or([
                root[0] * cos + root[2] * sin,
                root[1],
                -root[0] * sin + root[2] * cos,
            ]);
            self.apply_movement(wished, env);
        }
        if self.motion_serial == old_serial {
            if self.blend > 0. {
                self.blend_elapsed = if self.blend_elapsed == 0 {
                    2
                } else {
                    self.blend_elapsed.saturating_add(1)
                };
                if self.blend_elapsed >= self.blend_duration {
                    self.motion_tick = self.blend_elapsed - self.blend_duration;
                    self.blend = 0.;
                    self.blend_elapsed = 0;
                } else {
                    self.blend = (self.blend_duration - self.blend_elapsed) as f32;
                }
            } else {
                self.motion_tick = clips.advance(self.motion, self.motion_tick);
            }
        }
    }

    fn update_fly(&mut self, clips: &impl Clips, env: &impl Env, old_serial: u64) {
        let mut fly = self.fly.expect("active fly jump");
        // Pending fly request 3 selects actual physics mode 11 at ANB event0.
        // Its tick uses the shared fall switch and restores the original mode
        // after reaching the launch height relative to the current floor.
        if !fly.fall_started && fly.arc.falling_motion() && self.reaction == Reaction::None {
            if !self.play(motion::FLY_JUMP + 1, actor_physics::MOTION_BLEND, clips) {
                return;
            }
            fly.fall_started = true;
        }
        let height = env
            .floor(self.position[0], self.position[2], self.position[1])
            .map_or(0., |floor| (self.position[1] - floor).max(0.));
        if fly.arc.t >= fly.arc.rise && height <= fly.initial_height {
            self.airborne = fly.original_airborne;
            self.fly = None;
            self.sampler.reset();
            self.play(motion::FLY_JUMP + 2, 0., clips);
            return;
        }
        let Some(frozen) = clips.gravity_frozen(self.motion, self.motion_tick) else {
            self.fault = Some(Fault::MissingGravityFlags(self.motion));
            return;
        };
        if self.sampler.due(self.wish_speed) {
            let dt = actor_physics::PHYSICS_STEP;
            let dy = fly
                .arc
                .step(self.position[1], self.sstm.fall_max, frozen, dt);
            fly.velocity = actor_physics::steer(
                fly.velocity,
                [self.wish_direction[0], self.wish_direction[2]],
                self.wish_speed,
                actor_physics::AIR_INERTIA,
                actor_physics::AIR_DRAG,
            );
            self.sampler
                .store([fly.velocity[0] * dt, dy, fly.velocity[1] * dt]);
        }
        let root = if old_serial == self.motion_serial {
            clips.root_delta(self.motion, self.motion_tick)
        } else {
            [0.; 3]
        };
        let (sin, cos) = self.yaw.sin_cos();
        let root = [
            root[0] * cos + root[2] * sin,
            root[1],
            -root[0] * sin + root[2] * cos,
        ];
        let base = self.sampler.displacement();
        let wished = [0, 1, 2].map(|axis| base[axis] + root[axis]);
        self.apply_movement(wished, env);
        // Shared movement corrects the native Y-down arc base by actual minus
        // wished displacement after collision; signs reverse at this API.
        fly.arc.base -= self.moved[1] - wished[1];
        self.fly = Some(fly);
    }

    fn request_movement(&mut self, direction: Vec3, speed: f32, movement: &mut Option<Vec3>) {
        self.wish_direction = unit(direction);
        self.wish_speed = speed;
        *movement = Some(self.wish_direction.map(|value| value * speed));
    }

    fn apply_movement(&mut self, wished: Vec3, env: &impl Env) {
        let from = self.position;
        self.position = env.movement(from, wished);
        self.wished = wished;
        self.moved = sub(self.position, from);
    }

    fn update_reaction(&mut self, clips: &impl Clips, env: &impl Env) {
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
                if d[1] <= 0.
                    && env
                        .floor(self.position[0], self.position[2], from[1])
                        .is_some_and(|y| self.position[1] <= y)
                {
                    self.blow = None;
                    self.airborne = false;
                    self.reaction = Reaction::BlowLand;
                    self.set_reaction_motion(if back { 19 } else { 17 });
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
            if clips.ended(self.motion, self.motion_tick) {
                self.reaction = Reaction::None;
                self.set_reaction_motion(motion::IDLE);
            }
        }
    }

    fn should_leave(&self, input: Input) -> bool {
        if input.force_leave {
            return true;
        }
        if input.no_leave {
            return false;
        }
        let d = sub(input.player, self.position);
        let distance = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        distance
            > if input.culled {
                self.culled_leave_distance
            } else {
                self.leave_distance
            }
    }

    /// Return true on a source halt, false on callback completion/start.
    fn run(
        &mut self,
        mut a: Action,
        input: Input,
        clips: &impl Clips,
        env: &impl Env,
        rng: &mut ScriptRandom,
        movement: &mut Option<Vec3>,
    ) -> bool {
        let to = sub(input.target, self.position);
        let distance = hlen(to);
        let end = clips.ended(self.motion, self.motion_tick);
        let in_loop = self.blend <= 0. && clips.in_loop(self.motion, self.motion_tick);
        match a.kind {
            ActKind::ModeBattle => {
                if self.should_leave(input) {
                    self.start(ActKind::Leave);
                } else if distance < self.close_distance {
                    self.start(ActKind::BattleAttack);
                } else if distance < self.short_distance {
                    self.start(ActKind::BattleShort);
                } else {
                    self.start(ActKind::BattleLong);
                }
                return false;
            }
            ActKind::ModeRevenge => {
                self.start(if distance < self.short_distance {
                    ActKind::RevengeShort
                } else {
                    ActKind::RevengeLong
                });
                self.push(ActKind::ModeBattle);
                return false;
            }
            ActKind::BattleAttack => {
                if rng.getf(1.) < 0.3 {
                    self.start(ActKind::Footwork);
                } else {
                    self.action = None;
                }
                if rng.getf(1.) < 0.6 {
                    self.push(ActKind::MoveStep);
                }
                if rng.getf(1.) < 0.3 {
                    self.push(ActKind::MoveStep);
                }
                let attack = if rng.getf(1.) < 0.8 {
                    ActKind::Kick
                } else {
                    ActKind::Claw
                };
                self.push(attack);
                if distance < self.close_distance {
                    self.push(ActKind::Escape);
                    if rng.getf(1.) < 0.5 {
                        self.push(ActKind::Escape);
                    }
                    self.push(ActKind::Fight);
                } else {
                    let next = if rng.getf(1.) < 0.7 {
                        ActKind::Escape
                    } else {
                        ActKind::MoveBack
                    };
                    self.push(next);
                }
                return false;
            }
            ActKind::BattleShort => {
                self.start(ActKind::MoveTarget);
                let r = rng.getf(1.);
                self.push(if r < 0.2 {
                    ActKind::Footwork
                } else if r < 0.5 {
                    ActKind::MoveStep
                } else {
                    ActKind::IdleTime
                });
                let r = rng.getf(1.);
                self.push(if r < 0.3 {
                    ActKind::Kick
                } else if r < 0.8 {
                    ActKind::WalkAttack
                } else {
                    ActKind::MoveTarget
                });
                return false;
            }
            ActKind::BattleLong => {
                if rng.getf(1.) < 0.3 {
                    self.start(ActKind::Jump);
                } else {
                    let r = rng.getf(1.);
                    if r < 0.2 {
                        self.start(ActKind::Footwork);
                    } else if r < 0.4 {
                        self.start(ActKind::IdleTime);
                    }
                    self.clear();
                    if rng.getf(1.) < 0.7 {
                        self.push(ActKind::MoveTarget);
                        self.push(ActKind::Jump);
                    } else {
                        self.push(ActKind::MoveFront);
                        self.push(ActKind::Kick);
                    }
                }
                return false;
            }
            ActKind::RevengeShort | ActKind::RevengeLong => {
                let r = rng.getf(1.);
                let next = if a.kind == ActKind::RevengeShort {
                    if r < 0.8 {
                        ActKind::Kick
                    } else {
                        ActKind::Claw
                    }
                } else if r < 0.8 {
                    ActKind::Claw
                } else {
                    ActKind::WalkAttack
                };
                // Native start clears the previously pushed ModeBattle; main
                // re-enters battle when the eventual action queue is empty.
                self.start(next);
                return false;
            }
            ActKind::Appear => {
                if a.stage == 0 {
                    if matches!(input.appear_way, 2 | -1) {
                        self.face_immediate(sub(input.player, self.position));
                        self.play(motion::APPEAR, 0., clips);
                        a.alternate = if input.entry_fly || !self.airborne {
                            0
                        } else {
                            4
                        };
                        // Fade argument is a logical continuation (0/4), not
                        // an invented spawn duration. Host presentation profile.
                        self.alpha = 1.;
                        a.stage = 1;
                        return self.continue_action(a);
                    } else {
                        self.play(motion::IDLE, 0., clips);
                        self.motion_tick = rng.getf(clips.length(motion::IDLE) as f32) as u32;
                        a.stage = 2;
                        return self.continue_action(a);
                    }
                } else if a.stage == 1 {
                    if end {
                        self.play(a.alternate, 0., clips);
                        self.action = None;
                        return false;
                    }
                    self.face(sub(input.player, self.position), input.turn_rate);
                } else if a.stage == 2 {
                    if !input.event_running && !input.tutorial_open {
                        a.timer = rng.getf(30.);
                        a.stage = 3;
                        return self.continue_action(a);
                    }
                } else if a.timer > 0. {
                    a.timer -= actor_physics::DT;
                } else {
                    self.action = None;
                    return false;
                }
            }
            ActKind::Idle
            | ActKind::IdleTime
            | ActKind::Footwork
            | ActKind::Fight
            | ActKind::Escape => {
                if self.airborne && !input.entry_fly {
                    self.action = Some(a);
                    return true;
                }
                if a.stage == 0 || a.kind == ActKind::Fight {
                    self.face(to, input.turn_rate);
                }
                if a.stage == 0 {
                    let slot = match a.kind {
                        ActKind::Footwork => motion::FOOTWORK,
                        ActKind::Fight => motion::FIGHT,
                        ActKind::Escape => motion::ESCAPE,
                        _ => motion::IDLE,
                    };
                    let blend = if slot == 0 && !matches!(self.motion, 1 | 2) {
                        0.
                    } else {
                        0.35
                    };
                    self.play(slot, blend, clips);
                    if a.kind == ActKind::IdleTime {
                        a.timer = rng.range(15., 45.);
                    }
                    a.stage = 1;
                    return self.continue_action(a);
                } else if (a.kind == ActKind::IdleTime && a.timer <= 0.)
                    || (a.kind != ActKind::IdleTime && end)
                {
                    if a.kind == ActKind::Escape {
                        self.start(ActKind::MoveDown);
                    } else {
                        if a.kind == ActKind::Fight {
                            self.play(motion::IDLE, 0., clips);
                        }
                        self.action = None;
                    }
                    return false;
                } else if a.kind == ActKind::IdleTime {
                    a.timer -= actor_physics::DT;
                }
            }
            ActKind::MoveStep => {
                self.face(to, input.turn_rate);
                if a.stage == 0 {
                    let choice = if rng.getf(1.) < 0.5 { 1 } else { 0 };
                    a.alternate = motion::STEP[1 - choice];
                    self.play(motion::STEP[choice], 0.35, clips);
                    a.stage = 1;
                    return self.continue_action(a);
                } else if end {
                    self.play(motion::IDLE, 0., clips);
                    if a.stage == 1 && rng.getf(1.) < 0.2 {
                        self.play(a.alternate, 0.35, clips);
                        a.stage = 2;
                        return self.continue_action(a);
                    } else {
                        self.action = None;
                        return false;
                    }
                }
            }
            ActKind::MoveTarget => {
                if a.stage == 0 {
                    a.timer = rng.range(150., 300.);
                    let _ = rng.get(2); // independent side draw even for angle 0.
                    a.stage = 1;
                }
                if a.timer <= 0. || distance < 100. {
                    self.action = None;
                    return false;
                }
                self.face(to, input.turn_rate);
                match env.check_step(self.position, horizontal(to)) {
                    None => self.fault = Some(Fault::MissingStepQuery),
                    Some(3) => {
                        self.action = None;
                        return false;
                    }
                    Some(value @ (1 | 2)) => {
                        self.fault = Some(Fault::UnsupportedStep(value));
                    }
                    Some(0) => {
                        if self.motion != 2 {
                            self.play(2, 8., clips);
                        }
                    }
                    Some(v) => self.fault = Some(Fault::UnsupportedStep(v)),
                }
                a.timer -= actor_physics::DT;
            }
            ActKind::Jump => {
                if a.stage == 0 {
                    self.wish_direction = unit(to);
                    self.pending_fly = Some(PendingFly {
                        direction: horizontal(to),
                        height: 500.,
                        speed: 35.,
                    });
                    self.play(motion::FLY_JUMP, 0., clips);
                    a.stage = 1;
                } else if a.stage == 1 && self.airborne {
                    a.stage = 2;
                } else if a.stage == 2 && !self.airborne {
                    self.action = None;
                    return false;
                }
            }
            ActKind::MoveFront | ActKind::MoveBack => {
                let base = if a.kind == ActKind::MoveFront {
                    motion::FRONT
                } else {
                    motion::BACK
                };
                if a.stage == 0 {
                    a.value = if a.kind == ActKind::MoveFront {
                        self.close_distance * rng.range(0.5, 1.)
                    } else {
                        self.short_distance * 1.2
                    };
                    self.play(base, 0.35, clips);
                    a.timer = 0.35;
                    a.stage = 1;
                } else if a.stage == 1 {
                    let facing = if a.kind == ActKind::MoveBack {
                        to.map(|v| -v)
                    } else {
                        to
                    };
                    self.face(facing, input.turn_rate);
                    if a.timer > 0. {
                        a.timer -= actor_physics::DT;
                    } else if in_loop {
                        a.timer = 60. * rng.range(1., 2.);
                        a.stage = 2;
                    }
                } else if a.stage == 2 {
                    if a.timer <= 0. || distance < a.value {
                        a.stage = 3;
                    } else {
                        a.timer -= actor_physics::DT;
                    }
                } else if a.stage == 3 && end {
                    self.play(base + 1, 0., clips);
                    a.stage = 4;
                } else if a.stage == 4 && end {
                    self.action = None;
                    return false;
                }
            }
            ActKind::Claw => {
                if a.stage == 0 {
                    self.play(motion::CLAW, 0.35, clips);
                    a.stage = 1;
                    return self.continue_action(a);
                } else if a.stage == 1 {
                    if !self.range(clips, 1) {
                        a.stage = 2;
                        return self.continue_action(a);
                    }
                } else if a.stage == 2 {
                    if self.range(clips, 2) {
                        self.face(to, input.turn_rate);
                        self.request_movement(
                            to,
                            if distance <= 300. { 0. } else { 7.8 },
                            movement,
                        );
                    } else {
                        a.stage = 3;
                        return self.continue_action(a);
                    }
                } else if end {
                    self.play(0, 0., clips);
                    self.action = None;
                    return false;
                }
            }
            ActKind::Kick => {
                if a.stage == 0 {
                    self.play(motion::KICK, 0.35, clips);
                    a.stage = 1;
                    return self.continue_action(a);
                } else if end {
                    self.play(0, 0., clips);
                    self.action = None;
                    return false;
                } else if a.stage == 1 && self.range(clips, 1) {
                    if distance <= 50. {
                        a.stage = 2;
                    } else {
                        self.request_movement(sub(input.target_step, self.position), 10., movement);
                    }
                }
            }
            ActKind::WalkAttack => {
                if a.stage == 0 {
                    self.face(to, input.turn_rate);
                    self.play(203, 0.35, clips);
                    a.stage = 1;
                    return self.continue_action(a);
                } else if a.stage == 1 && in_loop {
                    a.timer = 180.;
                    a.stage = 2;
                    return self.continue_action(a);
                } else if a.stage == 2 {
                    self.face(to, input.turn_rate);
                    if a.timer <= 0. || distance < 250. {
                        self.play(204, 0., clips);
                        a.direction = unit(to);
                        a.stage = 3;
                        return self.continue_action(a);
                    } else {
                        a.timer -= actor_physics::DT;
                    }
                } else if a.stage == 3 {
                    if end {
                        self.play(205, 0.35, clips);
                        a.value = 0.;
                        a.stage = 4;
                        return self.continue_action(a);
                    } else {
                        self.request_movement(a.direction, 1.5, movement);
                    }
                } else if a.stage == 4 {
                    self.vertical_continuation(&mut a, in_loop, movement);
                    if self.wished[1] < 0. && self.moved[1] / self.wished[1] < 0.1 {
                        self.play(206, 0., clips);
                        a.stage = 5;
                        return self.continue_action(a);
                    }
                } else if a.stage == 5 && end {
                    self.play(0, 0., clips);
                    self.action = None;
                    return false;
                }
            }
            ActKind::MoveDown => {
                if a.stage == 0 {
                    self.play(181, 0., clips);
                    a.value = 0.;
                    a.stage = 1;
                    return self.continue_action(a);
                } else if a.stage == 1 {
                    self.vertical_continuation(&mut a, in_loop, movement);
                    if self.wished[1] < 0. && self.moved[1] / self.wished[1] < 0.1 {
                        let start =
                            (clips.loop_start(181) as f32 - self.motion_tick as f32).max(0.) + 4.;
                        self.play(182, 0., clips);
                        self.motion_tick = start as u32;
                        a.stage = 2;
                        return self.continue_action(a);
                    }
                } else if end {
                    self.play(0, 0., clips);
                    self.action = None;
                    return false;
                }
            }
            ActKind::Leave | ActKind::Dead => {
                if a.stage == 0 {
                    if a.kind == ActKind::Leave {
                        self.play(motion::LEAVE, 0., clips);
                    }
                    a.timer = if a.kind == ActKind::Leave {
                        30.
                    } else {
                        self.death_duration
                    };
                    a.value = a.timer;
                    a.stage = 1;
                }
                let half = a.value * 0.5;
                self.alpha = if a.kind == ActKind::Dead && a.timer > half {
                    1.
                } else if a.kind == ActKind::Dead {
                    a.timer / half.max(f32::MIN_POSITIVE)
                } else {
                    a.timer / a.value.max(f32::MIN_POSITIVE)
                };
                a.timer -= actor_physics::DT;
                if a.timer <= 0. {
                    self.alpha = 0.;
                    self.gone = true;
                }
            }
        }
        self.action = Some(a);
        true
    }

    fn vertical_continuation(&self, a: &mut Action, in_loop: bool, movement: &mut Option<Vec3>) {
        if in_loop {
            if a.value == 0. {
                a.value = self.moved[1];
            }
            *movement = Some([0., a.value, 0.]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SyntheticClips;
    impl Clips for SyntheticClips {
        fn length(&self, _slot: usize) -> u32 {
            8
        }
        fn loop_start(&self, _slot: usize) -> u32 {
            1
        }
        fn root(&self, slot: usize, tick: u32) -> Vec3 {
            if matches!(slot, 181 | 205) {
                [0., -(tick as f32), 0.]
            } else if slot == 2 {
                [0., 0., tick as f32 * 3.]
            } else {
                [0.; 3]
            }
        }
        fn range(&self, _slot: usize, tick: u32, range: u8) -> Option<bool> {
            Some(match range {
                1 => tick < 2,
                2 => (2..3).contains(&tick),
                _ => false,
            })
        }
    }
    struct Flat;
    impl Env for Flat {
        fn floor(&self, _x: f32, _z: f32, _top: f32) -> Option<f32> {
            Some(0.)
        }
        fn check_step(&self, _position: Vec3, _direction: Vec3) -> Option<u8> {
            Some(0)
        }
    }
    fn actor() -> Dusk {
        Dusk::new(
            [0.; 3],
            0.,
            TargetStats::enemy(100, 10000, 0, [100; 7], 100, 0),
            actor_physics::Sstm {
                fall_max: 20.,
                blow_brake_xz: 0.9,
                blow_min_xz: 1.,
                blow_brake_up: 0.9,
                blow_up: 10.,
                blow_speed: 5.,
                blow_to_hit_back: 1.,
                hit_back: 30.,
                hit_back_small: 10.,
                hit_back_to_jump: 0.1,
                fly_blow_brake: 1.,
                fly_blow_stop: 0.,
                fly_blow_up: 1.,
            },
        )
    }
    fn input(z: f32) -> Input {
        Input::arena([0., 0., z], [0., 0., z], 0.13)
    }
    fn isolated(kind: ActKind) -> Dusk {
        let mut actor = actor();
        actor.start(kind);
        actor
    }

    #[test]
    fn queue_keeps_fifteen_fifo_requests_and_start_clears_them() {
        let mut actor = isolated(ActKind::Idle);
        for n in 0..20 {
            actor.push(if n % 2 == 0 {
                ActKind::Kick
            } else {
                ActKind::Claw
            });
        }
        assert_eq!(actor.queued_acts().len(), 15);
        assert_eq!(
            &actor.queued_acts()[..3],
            &[ActKind::Kick, ActKind::Claw, ActKind::Kick]
        );
        actor.start(ActKind::Footwork);
        assert!(actor.queued_acts().is_empty());
        actor.start(ActKind::Dead);
        actor.push(ActKind::Kick);
        actor.start(ActKind::Idle);
        assert_eq!(actor.current_act(), Some(ActKind::Dead));
        assert!(actor.queued_acts().is_empty());
    }

    #[test]
    fn short_decision_retains_both_rng_draws_and_fifo_order() {
        for seed in 1..100 {
            let mut rng = ScriptRandom(seed);
            let mut expected_rng = rng;
            let r1 = expected_rng.getf(1.);
            let r2 = expected_rng.getf(1.);
            let expected = [
                if r1 < 0.2 {
                    ActKind::Footwork
                } else if r1 < 0.5 {
                    ActKind::MoveStep
                } else {
                    ActKind::IdleTime
                },
                if r2 < 0.3 {
                    ActKind::Kick
                } else if r2 < 0.8 {
                    ActKind::WalkAttack
                } else {
                    ActKind::MoveTarget
                },
            ];
            let mut actor = isolated(ActKind::BattleShort);
            let a = actor.action.unwrap();
            let halt = actor.run(a, input(500.), &SyntheticClips, &Flat, &mut rng, &mut None);
            assert!(!halt);
            assert_eq!(rng, expected_rng);
            assert_eq!(actor.current_act(), Some(ActKind::MoveTarget));
            assert_eq!(actor.queued_acts(), expected);
        }
    }

    #[test]
    fn kick_movement_uses_source_range_and_step_target_not_visual_name() {
        let mut actor = isolated(ActKind::Kick);
        let mut rng = ScriptRandom::default();
        let mut input = input(500.);
        input.target_step = [500., 0., 0.];
        actor.tick(input, &SyntheticClips, &Flat, &mut rng);
        assert_eq!(actor.motion, 202);
        assert_eq!(actor.position, [10., 0., 0.]);
        assert_eq!(actor.motion_tick, 0);
        actor.motion_tick = 2;
        actor.tick(input, &SyntheticClips, &Flat, &mut rng);
        assert_eq!(actor.position, [10., 0., 0.]);
    }

    #[test]
    fn missing_range_stops_visibly_instead_of_guessing_attack_phase() {
        struct NoRanges;
        impl Clips for NoRanges {
            fn length(&self, _slot: usize) -> u32 {
                4
            }
            fn loop_start(&self, _slot: usize) -> u32 {
                0
            }
            fn root(&self, _slot: usize, _tick: u32) -> Vec3 {
                [0.; 3]
            }
            fn range(&self, _slot: usize, _tick: u32, _range: u8) -> Option<bool> {
                None
            }
        }
        let mut actor = isolated(ActKind::Kick);
        actor.tick(input(500.), &NoRanges, &Flat, &mut ScriptRandom::default());
        assert_eq!(
            actor.fault,
            Some(Fault::MissingRange {
                slot: 202,
                range: 1
            })
        );
        assert_eq!(actor.position, [0.; 3]);
    }

    #[test]
    fn fly_jump_requires_admitted_takeoff_events_before_leaving_the_ground() {
        let mut actor = isolated(ActKind::Jump);
        actor.tick(
            input(1000.),
            &SyntheticClips,
            &Flat,
            &mut ScriptRandom::default(),
        );
        assert_eq!(actor.fault, Some(Fault::MissingTakeoff(155)));
        assert_eq!(actor.position, [0.; 3]);
        assert!(!actor.airborne);
    }

    #[test]
    fn fly_jump_waits_for_takeoff_then_falls_and_restores_ground_mode() {
        struct FlyClips;
        impl Clips for FlyClips {
            fn length(&self, _: usize) -> u32 {
                60
            }
            fn loop_start(&self, _: usize) -> u32 {
                60
            }
            fn root(&self, _: usize, _: u32) -> Vec3 {
                [0.; 3]
            }
            fn range(&self, _: usize, _: u32, _: u8) -> Option<bool> {
                Some(false)
            }
            fn takeoff(&self, slot: usize, t: u32) -> Option<bool> {
                Some(slot == 155 && t == 2)
            }
            fn gravity_frozen(&self, _: usize, _: u32) -> Option<bool> {
                Some(false)
            }
        }
        let mut actor = isolated(ActKind::Jump);
        let mut rng = ScriptRandom::default();
        for _ in 0..3 {
            actor.tick(input(1000.), &FlyClips, &Flat, &mut rng);
            assert!(!actor.airborne);
        }
        let mut peak = 0f32;
        let mut took_off = false;
        let mut fall = false;
        let mut landed = false;
        for _ in 0..180 {
            actor.tick(input(1000.), &FlyClips, &Flat, &mut rng);
            assert_eq!(actor.fault, None);
            peak = peak.max(actor.position[1]);
            took_off |= actor.airborne;
            fall |= actor.motion == 156;
            if took_off && !actor.airborne {
                landed = true;
                break;
            }
        }
        assert!(took_off && fall && landed);
        assert!(peak > 490. && peak <= 500.);
        assert_eq!(actor.motion, 157);
        assert_eq!(actor.position[1], 0.);
    }

    #[test]
    fn reflection_interrupts_attack_without_dealing_damage_or_losing_revenge() {
        let mut actor = isolated(ActKind::Kick);
        actor.push(ActKind::WalkAttack);
        let hp = actor.hp;
        actor.reflect(140, -1.);
        assert_eq!(actor.hp, hp);
        assert_eq!(actor.motion, 140);
        assert_eq!(actor.reaction, Reaction::Flinch);
        assert_eq!(actor.current_act(), Some(ActKind::ModeRevenge));
        assert!(actor.queued_acts().is_empty());
        actor.tick(
            input(500.),
            &SyntheticClips,
            &Flat,
            &mut ScriptRandom::default(),
        );
        assert!(actor.position[2] < 0.);
        assert_eq!(actor.current_act(), Some(ActKind::ModeRevenge));
    }

    #[test]
    fn immediate_blend_threshold_keeps_source_units_and_loop_readiness() {
        let mut actor = actor();
        for (blend, remaining) in [(0.35, 0.), (2., 0.), (2.01, 2.01), (8., 8.)] {
            assert!(actor.play(203, blend, &SyntheticClips));
            assert_eq!(actor.blend, remaining);
        }
    }

    #[test]
    fn down_continuation_waits_for_collision_ratio_without_timeout() {
        let mut actor = isolated(ActKind::MoveDown);
        let mut rng = ScriptRandom::default();
        actor.tick(input(500.), &SyntheticClips, &Flat, &mut rng);
        actor.tick(input(500.), &SyntheticClips, &Flat, &mut rng);
        assert_eq!(actor.motion, 181);
        assert_eq!(actor.wished[1], -1.);
        assert_eq!(actor.moved[1], 0.);
        actor.tick(input(500.), &SyntheticClips, &Flat, &mut rng);
        assert_eq!(actor.motion, 182);
        // Missing vertical root data never causes a fabricated end timer.
        actor.start(ActKind::MoveDown);
        actor.wished = [0.; 3];
        actor.moved = [0.; 3];
        struct NoRoot;
        impl Clips for NoRoot {
            fn length(&self, _slot: usize) -> u32 {
                4
            }
            fn loop_start(&self, _slot: usize) -> u32 {
                1
            }
            fn root(&self, _slot: usize, _tick: u32) -> Vec3 {
                [0.; 3]
            }
            fn range(&self, _slot: usize, _tick: u32, _range: u8) -> Option<bool> {
                Some(false)
            }
        }
        for _ in 0..500 {
            actor.tick(input(500.), &NoRoot, &Flat, &mut rng);
        }
        assert_eq!(actor.motion, 181);
        assert_eq!(actor.current_act(), Some(ActKind::MoveDown));
        assert_eq!(actor.fault, None);
    }

    #[test]
    fn lethal_hit_finishes_death_without_automatic_respawn() {
        let mut actor = actor();
        actor.hp = 1;
        let hit = AttackParams {
            power: 100,
            ..AttackParams::default()
        };
        let attacker = AttackerStats {
            strength: 100,
            max: 99999,
            min: 1,
            ..AttackerStats::default()
        };
        assert!(
            actor
                .hit(
                    [0., 0., 1.],
                    false,
                    &hit,
                    &attacker,
                    combat::Difficulty::Standard
                )
                .is_some()
        );
        assert_eq!(actor.current_act(), Some(ActKind::Dead));
        assert!(!actor.targetable());
        let mut rng = ScriptRandom::default();
        for _ in 0..200 {
            actor.tick(input(100.), &SyntheticClips, &Flat, &mut rng);
        }
        assert!(actor.gone);
        assert_eq!(actor.hp, 0);
    }
}
