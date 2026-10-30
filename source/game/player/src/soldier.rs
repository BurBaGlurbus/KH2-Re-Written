//! Ordinary Soldier decisions and source action continuations, with shared
//! engine motion/physics/hit consumers. Claw, kick, step, idle and rotated
//! movement and jump attacks execute their own script paths. Non-arena leave and
//! special step/flying/capture branches remain explicit faults.
pub use crate::dusk::{Env, Input, Reaction};
use crate::shadow::ScriptRandom;
use crate::{actor_physics, combat};

pub trait Clips: crate::dusk::Clips {
    /// Qualified RANGE events 3/24/30 suppress the generic airborne Y result.
    /// Missing motion/event admission must not silently disable gravity flags.
    fn no_gravity(&self, slot: usize, tick: u32) -> Option<bool>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Idle,
    IdleTime,
    MoveTarget,
    MoveBack,
    MoveRoll,
    MoveStep,
    Claw,
    Kick,
    JumpAttack,
    Fight,
}
impl Action {
    pub fn source_name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::IdleTime => "idle_time",
            Self::MoveTarget => "move_target",
            Self::MoveBack => "move_back",
            Self::MoveRoll => "move_roll",
            Self::MoveStep => "move_step",
            Self::Claw => "atk_craw",
            Self::Kick => "atk_kick",
            Self::JumpAttack => "atk_jump",
            Self::Fight => "fight",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Start(Action),
    Push(Action),
    Clear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    BattleAttack,
    BattleShort,
    BattleLong,
    RevengeShort,
    RevengeLong,
}

/// Raw work-object counters consumed by the Soldier callbacks. Their native
/// producers are not inferred from elapsed ticks or incoming damage here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counters {
    pub battle: i32,
    pub revenge: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The long revenge callback first calls the native leave helper, which
    /// can yield. An action host must qualify that continuation separately.
    LeaveContinuation,
    MissingMotion(usize),
    MissingGravityFlags(usize),
    UnsupportedStep(u8),
    MissingStepQuery,
    UnsupportedAppearance,
    UnsupportedCapture,
    NonYieldingActions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActKind {
    Appear,
    ModeBattle,
    ModeRevenge,
    Decision(Decision),
    Action(Action),
    Dead,
    Leave,
}
impl ActKind {
    fn locked(self) -> bool {
        matches!(self, Self::Dead | Self::Leave)
    }
}
#[derive(Clone, Copy)]
struct Pending {
    kind: ActKind,
    stage: u8,
    timer: f32,
    angle: f32,
    direction: [f32; 3],
    speed: f32,
}
impl Pending {
    fn new(kind: ActKind) -> Self {
        Self {
            kind,
            stage: 0,
            timer: 0.,
            angle: 0.,
            direction: [0.; 3],
            speed: 0.,
        }
    }
}

/// Soldier-specific action continuations over the shared admitted motion,
/// collision and generic hit-reaction interfaces. Unqualified step/flying
/// branches stop explicitly instead of running another enemy's behavior.
pub struct Soldier {
    pub position: [f32; 3],
    pub spawn: [f32; 3],
    pub yaw: f32,
    pub hp: i32,
    pub stats: combat::TargetStats,
    pub sstm: actor_physics::Sstm,
    pub motion: usize,
    pub motion_tick: u32,
    pub motion_serial: u64,
    pub blend: f32,
    pub gone: bool,
    pub fault: Option<Fault>,
    pub reaction: Reaction,
    pub airborne: bool,
    /// Script SysObj wish vector, before the physics consumer clears its Y.
    pub wish_motion: [f32; 4],
    air: Option<actor_physics::Arc>,
    air_velocity: [f32; 2],
    blend_duration: u32,
    blend_elapsed: u32,
    counters: Counters,
    action: Option<Pending>,
    queue: Vec<ActKind>,
    record: combat::HitRecord,
    desired_yaw: Option<f32>,
    knock: Option<actor_physics::Knock>,
    blow: Option<actor_physics::Blow>,
    sampler: actor_physics::Sampler,
}
impl Soldier {
    pub fn new(
        position: [f32; 3],
        yaw: f32,
        stats: combat::TargetStats,
        sstm: actor_physics::Sstm,
    ) -> Self {
        Self {
            position,
            spawn: position,
            yaw,
            hp: stats.max_hp,
            stats,
            sstm,
            motion: 0,
            motion_tick: 0,
            motion_serial: 0,
            blend: 0.,
            gone: false,
            fault: None,
            reaction: Reaction::None,
            airborne: false,
            wish_motion: [0.; 4],
            air: None,
            air_velocity: [0.; 2],
            blend_duration: 0,
            blend_elapsed: 0,
            counters: Counters {
                battle: -1,
                revenge: -1,
            },
            action: Some(Pending::new(ActKind::Appear)),
            queue: vec![ActKind::ModeBattle],
            record: combat::HitRecord::new(false),
            desired_yaw: None,
            knock: None,
            blow: None,
            sampler: actor_physics::Sampler::default(),
        }
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
    pub fn record(&mut self) -> &mut combat::HitRecord {
        &mut self.record
    }
    pub fn respawn(&mut self) {
        *self = Self::new(self.spawn, self.yaw, self.stats, self.sstm)
    }
    pub fn defeat(&mut self) {
        if self.alive() {
            self.hp = 0;
            self.fault = None;
            self.start(ActKind::Dead)
        }
    }
    fn start(&mut self, kind: ActKind) {
        if !self.action.is_some_and(|a| a.kind.locked()) {
            self.queue.clear();
            self.action = Some(Pending::new(kind))
        }
    }
    fn push(&mut self, kind: ActKind) {
        if !self.action.is_some_and(|a| a.kind.locked()) && self.queue.len() < 15 {
            self.queue.push(kind)
        }
    }
    fn play(&mut self, slot: usize, blend: f32, clips: &impl Clips) -> bool {
        if clips.length(slot) == 0 {
            self.fault = Some(Fault::MissingMotion(slot));
            return false;
        }
        self.motion = slot;
        self.motion_tick = 0;
        self.motion_serial = self.motion_serial.wrapping_add(1);
        self.blend = if blend > 2. { blend } else { 0. };
        self.blend_duration = self.blend as u32;
        self.blend_elapsed = 0;
        true
    }
    fn reaction_motion(&mut self, slot: usize) {
        self.motion = slot;
        self.motion_tick = 0;
        self.motion_serial = self.motion_serial.wrapping_add(1);
        self.blend = 0.
    }
    pub fn hit(
        &mut self,
        direction: [f32; 3],
        high: bool,
        attack: &combat::AttackParams,
        attacker: &combat::AttackerStats,
        difficulty: combat::Difficulty,
    ) -> Option<i32> {
        if !self.targetable() {
            return None;
        }
        let damage = combat::damage_with(
            attacker,
            attack,
            &self.stats,
            &combat::TargetState {
                hp: self.hp,
                difficulty,
                ..combat::TargetState::full(self.stats.max_hp)
            },
        );
        self.hp = (self.hp - damage).max(0);
        let back = self.yaw.sin() * direction[0] + self.yaw.cos() * direction[2] >= 0.;
        let class = combat::reaction_class(attack.reaction);
        if class == 5 {
            self.fault = Some(Fault::UnsupportedCapture)
        } else if class == 4 {
            let [mut up, mut speed] = attack.knockback.map(f32::from);
            if up == 0. && speed == 0. {
                up = self.sstm.blow_up;
                speed = self.sstm.blow_speed
            }
            let d = horizontal(direction);
            self.blow = Some(actor_physics::Blow::new(
                [d[0] * speed, up, d[2] * speed],
                &self.sstm,
            ));
            self.knock = None;
            self.air = None;
            self.airborne = true;
            self.sampler.reset();
            self.reaction = Reaction::Blow { back };
            self.reaction_motion(if back { 18 } else { 16 });
        } else if class == 3 {
            self.knock = actor_physics::Knock::new(
                horizontal(direction),
                self.sstm
                    .knock_strength(attack.knockback[0], class, attack.reaction),
                0.,
            );
            self.reaction = Reaction::Flinch;
            self.reaction_motion(10 + (back as usize) * 2 + (!high) as usize);
        }
        if self.hp == 0 {
            self.start(ActKind::Dead)
        } else if class > 1 {
            self.counters.revenge = self.counters.revenge.wrapping_add(1);
            self.start(ActKind::ModeRevenge)
        }
        Some(damage)
    }
    /// Shared engine ground reflection, consuming the source attack row's
    /// motion/hit-back fields without replacing the Soldier's decisions.
    pub fn reflect(&mut self, reflect_motion: usize, hit_back: f32) {
        if !self.targetable() {
            return;
        }
        self.reaction_motion(reflect_motion);
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
        self.action = None;
        self.queue.clear();
        self.start(ActKind::ModeRevenge);
    }
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
        let reacting = self.reaction != Reaction::None && self.hp > 0;
        if reacting {
            self.update_reaction(clips, env)
        } else if !input.no_control || self.action.is_some_and(|a| a.kind.locked()) {
            let mut yielded = false;
            for _ in 0..64 {
                if self.action.is_none() {
                    self.action = Some(Pending::new(if self.queue.is_empty() {
                        ActKind::ModeBattle
                    } else {
                        self.queue.remove(0)
                    }))
                }
                let action = self.action.expect("Soldier action installed");
                if self.run(action, input, clips, env, rng) || self.fault.is_some() || self.gone {
                    yielded = true;
                    break;
                }
            }
            if !yielded {
                self.fault = Some(Fault::NonYieldingActions)
            }
        }
        if !reacting && self.fault.is_none() {
            if !input.no_control
                && let Some(desired) = self.desired_yaw
            {
                let d = crate::follow_camera::wrap(desired - self.yaw);
                let limit = input.turn_rate.max(0.) * 2.;
                self.yaw = crate::follow_camera::wrap(self.yaw + d.clamp(-limit, limit));
            }
            if self.air.is_some() {
                self.step_air(clips, env)
            } else {
                let root = if old_serial == self.motion_serial && self.blend <= 0. {
                    clips.root_delta(self.motion, self.motion_tick)
                } else {
                    [0.; 3]
                };
                let (s, c) = self.yaw.sin_cos();
                self.position = env.movement(
                    self.position,
                    [
                        root[0] * c + root[2] * s,
                        root[1],
                        -root[0] * s + root[2] * c,
                    ],
                );
            }
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
                self.motion_tick = clips.advance(self.motion, self.motion_tick)
            }
        }
    }
    fn step_air(&mut self, clips: &impl Clips, env: &impl Env) {
        let Some(mut arc) = self.air else { return };
        let Some(frozen) = clips.no_gravity(self.motion, self.motion_tick) else {
            self.fault = Some(Fault::MissingGravityFlags(self.motion));
            return;
        };
        if self.sampler.due(self.wish_motion[3]) {
            let dt = actor_physics::PHYSICS_STEP;
            self.wish_motion[1] = 0.;
            let dy = arc.step(self.position[1], self.sstm.fall_max, frozen, dt);
            self.air_velocity = actor_physics::steer(
                self.air_velocity,
                [self.wish_motion[0], self.wish_motion[2]],
                self.wish_motion[3],
                actor_physics::AIR_INERTIA,
                actor_physics::AIR_DRAG,
            );
            self.sampler
                .store([self.air_velocity[0] * dt, dy, self.air_velocity[1] * dt]);
        }
        let displacement = self.sampler.displacement();
        let from = self.position;
        self.position = env.movement(from, displacement);
        if displacement[1] <= 0.
            && env
                .floor(self.position[0], self.position[2], from[1])
                .is_some_and(|floor| self.position[1] <= floor)
        {
            self.air = None;
            self.airborne = false;
            self.air_velocity = [0.; 2];
            self.sampler.reset();
            self.play(5, 0., clips);
        } else {
            self.air = Some(arc);
        }
    }
    fn face(&mut self, d: [f32; 3]) {
        if d[0] != 0. || d[2] != 0. {
            self.desired_yaw = Some(d[0].atan2(d[2]))
        }
    }
    fn update_reaction(&mut self, clips: &impl Clips, env: &impl Env) {
        if let Reaction::Blow { back } = self.reaction {
            if let Some(blow) = &mut self.blow {
                if self.sampler.due(0.) {
                    self.sampler.store(blow.step(
                        &self.sstm,
                        self.sstm.fall_max,
                        actor_physics::PHYSICS_STEP,
                    ))
                }
                let d = self.sampler.displacement();
                let from = self.position;
                self.position = env.movement(from, d);
                if d[1] <= 0.
                    && env
                        .floor(self.position[0], self.position[2], from[1])
                        .is_some_and(|floor| self.position[1] <= floor)
                {
                    self.blow = None;
                    self.airborne = false;
                    self.reaction = Reaction::BlowLand;
                    self.reaction_motion(if back { 19 } else { 17 });
                }
            }
        } else {
            if let Some(knock) = &mut self.knock {
                if let Some(d) = knock.step() {
                    self.position = env.movement(self.position, d)
                } else {
                    self.knock = None
                }
            }
            if clips.ended(self.motion, self.motion_tick) {
                self.reaction = Reaction::None;
                self.reaction_motion(0)
            }
        }
    }
    fn run(
        &mut self,
        mut pending: Pending,
        input: Input,
        clips: &impl Clips,
        env: &impl Env,
        rng: &mut ScriptRandom,
    ) -> bool {
        let to = [0, 1, 2].map(|i| input.target[i] - self.position[i]);
        let distance = to[0].hypot(to[2]);
        let end = self.blend <= 0. && clips.ended(self.motion, self.motion_tick);
        match pending.kind {
            ActKind::ModeBattle => {
                if input.force_leave {
                    self.start(ActKind::Leave)
                } else if !input.no_leave {
                    self.fault = Some(Fault::LeaveContinuation)
                } else {
                    self.counters.battle = self.counters.battle.wrapping_add(1);
                    self.start(ActKind::Decision(if distance < 300. {
                        Decision::BattleAttack
                    } else if distance < 500. {
                        Decision::BattleShort
                    } else {
                        Decision::BattleLong
                    }));
                }
                return false;
            }
            ActKind::ModeRevenge => {
                self.start(ActKind::Decision(if distance < 500. {
                    Decision::RevengeShort
                } else {
                    Decision::RevengeLong
                }));
                self.push(ActKind::ModeBattle);
                return false;
            }
            ActKind::Decision(kind) => {
                match decide(kind, &mut self.counters, rng, input.force_leave) {
                    Ok(commands) => {
                        for command in commands {
                            match command {
                                Command::Start(a) => self.start(ActKind::Action(a)),
                                Command::Push(a) => self.push(ActKind::Action(a)),
                                Command::Clear => {
                                    self.action = None;
                                    self.queue.clear()
                                }
                            }
                        }
                    }
                    Err(fault) => self.fault = Some(fault),
                }
                return false;
            }
            ActKind::Appear => {
                if pending.stage == 0 {
                    if !matches!(input.appear_way, 2 | -1) || input.entry_fly {
                        self.fault = Some(Fault::UnsupportedAppearance);
                        return true;
                    }
                    let d = [0, 1, 2].map(|i| input.player[i] - self.position[i]);
                    if d[0] != 0. || d[2] != 0. {
                        self.yaw = d[0].atan2(d[2]);
                    }
                    self.play(44, 0., clips);
                    pending.stage = 1;
                } else if end {
                    self.play(0, 0., clips);
                    self.action = None;
                    return false;
                }
                self.face([0, 1, 2].map(|i| input.player[i] - self.position[i]));
            }
            ActKind::Action(action) => match action {
                Action::JumpAttack => match pending.stage {
                    0 => {
                        if self.airborne {
                            return true;
                        }
                        self.face(to);
                        self.wish_motion = [self.yaw.sin(), 0., self.yaw.cos(), 0.];
                        self.air = Some(actor_physics::Arc::launch(self.position[1], 260.));
                        self.air_velocity = [0.; 2];
                        self.airborne = true;
                        self.sampler.reset();
                        self.play(3, 0., clips);
                        pending.stage = 1;
                    }
                    1 => {
                        if !self.airborne {
                            return true;
                        }
                        self.face(to);
                        pending.timer = 24. - actor_physics::DT;
                        pending.stage = 2;
                    }
                    2 => {
                        if pending.timer > 0. {
                            pending.timer -= actor_physics::DT;
                        } else {
                            self.play(204, 8., clips);
                            let length = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
                            pending.direction = if length > 0. {
                                to.map(|v| v * (1. / length))
                            } else {
                                [0.; 3]
                            };
                            pending.speed = (length / 30.).min(30.);
                            pending.stage = 3;
                            self.wish_motion = [
                                pending.direction[0],
                                pending.direction[1],
                                pending.direction[2],
                                pending.speed,
                            ];
                            self.face(pending.direction);
                        }
                    }
                    3 => {
                        if end {
                            pending.stage = 4;
                        } else if self.airborne {
                            self.wish_motion = [
                                pending.direction[0],
                                pending.direction[1],
                                pending.direction[2],
                                pending.speed,
                            ];
                            self.face(pending.direction);
                        } else {
                            let length = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
                            let direction = if length > 0. {
                                to.map(|v| v * (1. / length))
                            } else {
                                [0.; 3]
                            };
                            self.wish_motion = [direction[0], direction[1], direction[2], 0.];
                            // Source exits the inner motion wait after this
                            // grounded write, then yields once before checking
                            // the outer is-jump predicate again.
                            pending.stage = 4;
                        }
                    }
                    _ => {
                        if self.airborne {
                            self.face(to);
                            pending.timer = 24. - actor_physics::DT;
                            pending.stage = 2;
                        } else {
                            self.action = None;
                            return false;
                        }
                    }
                },
                Action::Idle | Action::Claw | Action::Kick | Action::MoveStep | Action::Fight => {
                    if pending.stage == 0 {
                        if input.entry_fly {
                            self.fault = Some(Fault::UnsupportedAppearance);
                            return true;
                        }
                        self.face(to);
                        let slot = match action {
                            Action::Idle => 0,
                            Action::Claw => {
                                if distance > 200. {
                                    202
                                } else {
                                    201
                                }
                            }
                            Action::Kick => 203,
                            Action::MoveStep => {
                                if rng.getf(1.) < 0.5 {
                                    152
                                } else {
                                    151
                                }
                            }
                            Action::Fight => 154,
                            _ => unreachable!(),
                        };
                        let blend = if slot == 0 && !matches!(self.motion, 1 | 2) {
                            0.
                        } else {
                            0.35
                        };
                        self.play(slot, blend, clips);
                        pending.stage = 1;
                    } else if end {
                        if action == Action::Fight {
                            self.play(4, 0., clips);
                        }
                        self.action = None;
                        return false;
                    }
                    if action == Action::MoveStep {
                        self.face(to)
                    }
                }
                Action::IdleTime => {
                    if pending.stage == 0 {
                        pending.timer = rng.range(15., 45.);
                        self.face(to);
                        let blend = if matches!(self.motion, 1 | 2) {
                            0.35
                        } else {
                            0.
                        };
                        self.play(0, blend, clips);
                        pending.stage = 1;
                    }
                    if pending.timer <= 0. {
                        self.action = None;
                        return false;
                    }
                    pending.timer -= actor_physics::DT;
                }
                Action::MoveTarget | Action::MoveBack | Action::MoveRoll => {
                    if pending.stage == 0 {
                        pending.timer = if action == Action::MoveTarget {
                            rng.range(90., 180.)
                        } else {
                            rng.range(20., 40.)
                        };
                        let degrees: f32 = match action {
                            Action::MoveTarget => 0.,
                            Action::MoveBack => 180.,
                            _ => 90.,
                        };
                        pending.angle = if rng.get(2) == 0 {
                            degrees.to_radians()
                        } else {
                            -degrees.to_radians()
                        };
                        pending.stage = 1;
                    }
                    if pending.timer <= 0. || (action == Action::MoveTarget && distance < 300.) {
                        self.action = None;
                        return false;
                    }
                    let d = horizontal(to);
                    let (s, c) = pending.angle.sin_cos();
                    let direction = [d[0] * c + d[2] * s, 0., -d[0] * s + d[2] * c];
                    self.face(direction);
                    match env.check_step(self.position, direction) {
                        Some(0) => {}
                        Some(3) => {
                            self.action = None;
                            return false;
                        }
                        Some(step) => {
                            self.fault = Some(Fault::UnsupportedStep(step));
                            return true;
                        }
                        None => {
                            self.fault = Some(Fault::MissingStepQuery);
                            return true;
                        }
                    }
                    if self.motion != 2 {
                        self.play(2, 8., clips);
                    }
                    pending.timer -= actor_physics::DT;
                }
            },
            ActKind::Leave | ActKind::Dead => {
                if pending.stage == 0 {
                    pending.timer = 30.;
                    if pending.kind == ActKind::Leave {
                        self.play(45, 0., clips);
                    }
                    pending.stage = 1;
                }
                if pending.timer <= 0. {
                    self.gone = true;
                    return true;
                }
                pending.timer -= actor_physics::DT;
            }
        }
        self.action = Some(pending);
        true
    }
}
fn horizontal(v: [f32; 3]) -> [f32; 3] {
    let d = v[0].hypot(v[2]);
    if d > 0. {
        [v[0] / d, 0., v[2] / d]
    } else {
        [0.; 3]
    }
}

/// Emit one ordinary callback. `leave` is the qualified native leave-helper
/// result, not distance guessed from an enemy name. The bounded debug arena
/// explicitly supplies false through its no-leave context.
pub fn decide(
    decision: Decision,
    counters: &mut Counters,
    rng: &mut ScriptRandom,
    leave: bool,
) -> Result<Vec<Command>, Fault> {
    use Action::*;
    use Command::{Clear, Push, Start};
    let mut commands = Vec::new();
    match decision {
        Decision::BattleAttack => {
            // The first RNG call is short-circuited when the counter is <3.
            if counters.battle >= 3 && rng.getf(1.) < 0.6 {
                counters.battle = 0;
                commands.extend([Start(MoveBack), Push(JumpAttack), Push(Idle)]);
            } else {
                if rng.getf(1.) < 0.6 {
                    commands.extend([Start(MoveRoll), Push(Claw)]);
                } else {
                    commands.push(Start(Claw));
                }
                commands.push(Push(IdleTime));
            }
            commands.push(Push(if rng.getf(1.) < 0.5 {
                MoveRoll
            } else {
                MoveStep
            }));
            if rng.getf(1.) < 0.5 {
                commands.push(Push(if rng.getf(1.) < 0.5 {
                    MoveRoll
                } else {
                    MoveStep
                }));
            }
            if rng.getf(1.) < 0.3 {
                commands.extend([Push(JumpAttack), Push(Idle)]);
            }
        }
        Decision::BattleShort => {
            // This selection shares one draw; the following attack uses a new
            // draw. Combining these changes the script's RNG consumption.
            let choice = rng.getf(1.);
            if choice < 0.4 {
                commands.extend([Start(Claw), Push(IdleTime)]);
            } else if choice < 0.8 {
                commands.push(Start(MoveRoll));
            } else {
                commands.push(Start(MoveTarget));
            }
            commands.push(Push(MoveRoll));
            if rng.getf(1.) < 0.4 {
                commands.extend([Push(Claw), Push(IdleTime)]);
            } else {
                commands.extend([Push(MoveStep), Push(Claw), Push(IdleTime)]);
            }
        }
        Decision::BattleLong => {
            if rng.getf(1.) < 0.3 {
                commands.push(Start(IdleTime));
            }
            // Preserve the apparently redundant source command order. Native
            // action scheduling determines when each command takes effect.
            commands.extend([Clear, Start(MoveTarget)]);
        }
        Decision::RevengeShort => {
            if counters.revenge >= 2 {
                counters.revenge = 0;
                if rng.getf(1.) < 0.9 {
                    commands.push(Start(Kick));
                } else {
                    commands.extend([Start(JumpAttack), Push(Idle)]);
                }
            } else {
                commands.push(Start(if rng.getf(1.) < 0.6 { JumpAttack } else { Claw }));
                commands.push(Push(Idle));
            }
        }
        Decision::RevengeLong => {
            if leave {
                return Err(Fault::LeaveContinuation);
            }
            commands.push(Start(if rng.getf(1.) < 0.7 {
                Fight
            } else {
                MoveTarget
            }));
            commands.extend([Push(JumpAttack), Push(Kick)]);
        }
    }
    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn low_battle_counter_does_not_consume_the_counter_branch_draw() {
        let mut low = ScriptRandom(1);
        let mut high = ScriptRandom(1);
        let mut counters = Counters {
            battle: 2,
            revenge: 0,
        };
        let below = decide(Decision::BattleAttack, &mut counters, &mut low, false).unwrap();
        counters.battle = 3;
        let above = decide(Decision::BattleAttack, &mut counters, &mut high, false).unwrap();
        assert_eq!(
            &below[..2],
            &[
                Command::Start(Action::MoveRoll),
                Command::Push(Action::Claw)
            ]
        );
        assert_eq!(
            &above[..3],
            &[
                Command::Start(Action::MoveBack),
                Command::Push(Action::JumpAttack),
                Command::Push(Action::Idle)
            ]
        );
        assert_eq!(counters.battle, 0);
        // Below the threshold the claw chooser consumes the first draw;
        // above it the successful counter branch consumes that same draw.
        assert_eq!(low.0, high.0);
    }
    #[test]
    fn leave_continuation_fails_without_consuming_random_or_state() {
        let mut rng = ScriptRandom(17);
        let mut counters = Counters {
            battle: 4,
            revenge: 3,
        };
        assert_eq!(
            decide(Decision::RevengeLong, &mut counters, &mut rng, true),
            Err(Fault::LeaveContinuation)
        );
        assert_eq!(rng.0, 17);
        assert_eq!(
            counters,
            Counters {
                battle: 4,
                revenge: 3
            }
        );
    }
    #[test]
    fn long_battle_retains_clear_after_the_conditional_start() {
        let mut counters = Counters {
            battle: 0,
            revenge: 0,
        };
        let commands = decide(
            Decision::BattleLong,
            &mut counters,
            &mut ScriptRandom(1),
            false,
        )
        .unwrap();
        assert_eq!(
            commands,
            [
                Command::Start(Action::IdleTime),
                Command::Clear,
                Command::Start(Action::MoveTarget)
            ]
        );
    }

    struct SyntheticClips;
    impl crate::dusk::Clips for SyntheticClips {
        fn length(&self, _: usize) -> u32 {
            4
        }
        fn loop_start(&self, _: usize) -> u32 {
            0
        }
        fn root(&self, _: usize, _: u32) -> [f32; 3] {
            [0.; 3]
        }
        fn range(&self, _: usize, _: u32, _: u8) -> Option<bool> {
            Some(false)
        }
    }
    impl Clips for SyntheticClips {
        fn no_gravity(&self, _: usize, _: u32) -> Option<bool> {
            Some(false)
        }
    }
    struct Flat;
    impl Env for Flat {
        fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
            Some(0.)
        }
        fn check_step(&self, _: [f32; 3], _: [f32; 3]) -> Option<u8> {
            Some(0)
        }
    }
    fn actor() -> Soldier {
        let mut record = [0u8; 0x48];
        record[0x14..0x18].copy_from_slice(&20f32.to_le_bytes());
        Soldier::new(
            [0.; 3],
            0.,
            combat::TargetStats::enemy(100, 10000, 0, [100; 7], 20, 0),
            actor_physics::Sstm::read(&record).unwrap(),
        )
    }
    #[test]
    fn claw_selects_source_near_and_far_variants_without_jumping() {
        for (distance, expected) in [(200., 201), (201., 202)] {
            let mut actor = actor();
            actor.start(ActKind::Action(Action::Claw));
            actor.tick(
                Input::arena([0., 0., distance], [0., 0., distance], 0.1),
                &SyntheticClips,
                &Flat,
                &mut ScriptRandom(1),
            );
            assert_eq!(actor.motion, expected);
            assert_eq!(actor.motion_tick, 0);
            assert!(actor.fault.is_none());
        }
    }
    #[test]
    fn source_jump_uses_height260_wait24_and_blend8_before_attack_clock() {
        let mut actor = actor();
        actor.start(ActKind::Action(Action::Idle));
        for _ in 0..20 {
            actor.push(ActKind::Action(Action::Kick));
        }
        assert_eq!(actor.queue.len(), 15);
        actor.start(ActKind::Action(Action::JumpAttack));
        let input = Input::arena([0., 0., 300.], [0., 0., 300.], 0.1);
        let mut rng = ScriptRandom(1);
        actor.tick(input, &SyntheticClips, &Flat, &mut rng);
        assert_eq!(actor.motion, 3);
        assert!(actor.airborne);
        assert_eq!(actor.air.unwrap().rise, actor_physics::rise_time(260.));
        for _ in 0..24 {
            actor.tick(input, &SyntheticClips, &Flat, &mut rng);
            assert_eq!(actor.motion, 3);
        }
        actor.tick(input, &SyntheticClips, &Flat, &mut rng);
        assert_eq!(actor.motion, 204);
        assert_eq!(actor.blend, 8.);
        assert_eq!(actor.motion_tick, 0);
        assert!(actor.wish_motion[3] > 0. && actor.wish_motion[3] <= 30.);
        for _ in 0..6 {
            actor.tick(input, &SyntheticClips, &Flat, &mut rng);
            assert!(actor.blend > 0.);
            assert_eq!(actor.motion_tick, 0);
        }
        actor.tick(input, &SyntheticClips, &Flat, &mut rng);
        assert_eq!(actor.blend, 0.);
        assert_eq!(actor.motion_tick, 0);
        assert!(actor.fault.is_none());
    }
}
