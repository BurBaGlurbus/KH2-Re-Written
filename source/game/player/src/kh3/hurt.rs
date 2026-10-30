//! Hit reactions in the KH3 ruleset: the reference's damage kinds (a
//! flinch, a blow, a toss, a launch, a parabola, a beat, the spins) with
//! its speeds, gravity and timings, its armour rules and Aerial Recovery,
//! shown on KH2 Sora's damage motions (the flinches, the air flinch, the
//! blow and its knock-down, the recovery flip). A Sora in a reaction cannot
//! act until its change point (the reference's "change notify"): that is
//! the hit stun. Also here: the guarded hit's push-back, Final Blow's chase
//! and the duel's Risk Dodge.

use super::*;
use crate::hurt;

/// The reference's damage kinds that a duel's attacks give.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Knock {
    /// A flinch (SMALL).
    Small,
    Blow,
    Toss,
    Launch,
    Parabola,
    /// Slammed down (BEAT).
    Beat,
    SideSpin,
    VerticalRoll,
}

impl Knock {
    /// The heavy kinds break through armour; the light ones only deal
    /// their damage then.
    pub fn heavy(self) -> bool {
        !matches!(self, Knock::Small | Knock::Beat)
    }

    pub fn name(self) -> &'static str {
        match self {
            Knock::Small => "small",
            Knock::Blow => "blow",
            Knock::Toss => "toss",
            Knock::Launch => "launch",
            Knock::Parabola => "parabola",
            Knock::Beat => "beat",
            Knock::SideSpin => "sidespin",
            Knock::VerticalRoll => "verticalroll",
        }
    }

    /// Kinds that leave the victim blown away, which a counter turns into
    /// Final Blow's chase.
    pub fn blows_away(self) -> bool {
        matches!(self, Knock::Blow | Knock::SideSpin | Knock::VerticalRoll)
    }
}

/// A hit's damage kind and its row's power scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    pub knock: Knock,
    pub scale: f32,
}

/// What a hit came from, as the attacker's state tells it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    /// A swing of the KH2 combo (its record a finisher or not).
    Swing {
        finisher: bool,
        air: bool,
    },
    /// A flow attack's `hit`th landed hit (Sonic Slash, the wall-run
    /// attack, Buzz Saw by their stand-in records).
    Flow {
        record: u8,
        hit: u8,
    },
    Launch,
    Counter,
    /// Final Blow's chase, its `hit`th landed hit.
    Chase {
        hit: u8,
    },
    Bolt {
        ragnarok: bool,
    },
    /// A spell: its row a knock-down (a finisher's) or not, cast as a grand
    /// magic or not.
    Spell {
        heavy: bool,
        grand: bool,
    },
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Source::Swing {
                finisher: true,
                air: false,
            } => "finisher",
            Source::Swing {
                finisher: true,
                air: true,
            } => "air_finisher",
            Source::Swing { air: true, .. } => "air_swing",
            Source::Swing { .. } => "swing",
            Source::Flow {
                record: record::WALL_ATTACK,
                ..
            } => "wall_run_attack",
            Source::Flow {
                record: record::SPIN,
                ..
            } => "buzz_saw",
            Source::Flow { .. } => "sonic_slash",
            Source::Launch => "launch",
            Source::Counter => "counter",
            Source::Chase { .. } => "final_blow",
            Source::Bolt { ragnarok: true } => "ragnarok_bolt",
            Source::Bolt { .. } => "king_of_hearts_bolt",
            Source::Spell { grand: true, .. } => "grand_magic",
            Source::Spell { heavy: true, .. } => "spell_finisher",
            Source::Spell { .. } => "spell",
        }
    }
}

/// The damage kind and scale of a hit, by the reference's rows for the
/// move the KH2 record stands in for: the combo's swings (KB_STAB /
/// KB_TURN 0.8 SMALL), the ground finisher (KB_BREAK 1.2 BLOW), the air
/// finisher's end (0.8 BLOW), Sonic Slash (1.0 SMALL, then 1.5 TOSS), the
/// wall-run attack (1.5 TOSS), Buzz Saw (0.5 SMALL, then 1.0 VERTICALROLL),
/// Rising Spiral (0.6 TOSS), Counter Slash (1.6 VERTICALROLL), Final Blow
/// (0.3, then 0.6 SIDESPIN), King of Hearts' bolts (0.8 SIDESPIN),
/// Ragnarok's (0.9 TOSS) and a spell's finish row or grand magic
/// (PARABOLA). [duel choice] A spell's other rows flinch at 1.0.
pub fn strike(source: Source) -> Strike {
    let s = |knock, scale| Strike { knock, scale };
    match source {
        Source::Swing {
            finisher: true,
            air: false,
        } => s(Knock::Blow, 1.2),
        Source::Swing {
            finisher: true,
            air: true,
        } => s(Knock::Blow, 0.8),
        Source::Swing { .. } => s(Knock::Small, 0.8),
        Source::Flow {
            record: record::WALL_ATTACK,
            ..
        } => s(Knock::Toss, 1.5),
        Source::Flow {
            record: record::SPIN,
            hit: 0,
        } => s(Knock::Small, 0.5),
        Source::Flow {
            record: record::SPIN,
            ..
        } => s(Knock::VerticalRoll, 1.0),
        Source::Flow { hit: 0, .. } => s(Knock::Small, 1.0),
        Source::Flow { .. } => s(Knock::Toss, 1.5),
        Source::Launch => s(Knock::Toss, 0.6),
        Source::Counter => s(Knock::VerticalRoll, 1.6),
        Source::Chase { hit: 0 } => s(Knock::Small, 0.3),
        Source::Chase { .. } => s(Knock::SideSpin, 0.6),
        Source::Bolt { ragnarok: false } => s(Knock::SideSpin, 0.8),
        Source::Bolt { ragnarok: true } => s(Knock::Toss, 0.9),
        Source::Spell { heavy, grand } => {
            if heavy || grand {
                s(Knock::Parabola, 1.0)
            } else {
                s(Knock::Small, 1.0)
            }
        }
    }
}

impl Kh3 {
    /// What his own state makes of a melee hit he lands now.
    pub fn source(&self, player: &Player, clips: &Clips) -> Source {
        match self.mode {
            Mode::Flow { .. } => match self.flow {
                Some((_, true)) => Source::Chase {
                    hit: self.flow_hits,
                },
                Some((record, false)) => Source::Flow {
                    record,
                    hit: self.flow_hits,
                },
                None => Source::Flow {
                    record: record::SONIC,
                    hit: self.flow_hits,
                },
            },
            Mode::Launch { .. } => Source::Launch,
            _ => {
                let (i, air) = match player.action {
                    Action::Attack(i) => (Some(i), false),
                    Action::AirAttack(i) => (Some(i), true),
                    _ => (None, player.in_air()),
                };
                let rec = i.and_then(|i| clips.records.get(i));
                if i.is_some() && self.counter_swing == Some(player.action) {
                    return Source::Counter;
                }
                if rec.is_some_and(|r| r.id == record::LAUNCH) && self.launched {
                    return Source::Launch;
                }
                let finisher = rec.is_some_and(|r| r.flags & ptya::flag::FINISHER != 0);
                Source::Swing { finisher, air }
            }
        }
    }

    /// For 0.1 s after a command starts (a swing, a cast) hits deal their
    /// damage but no flinch.
    pub fn command_armour(player: &Player) -> bool {
        matches!(
            player.action,
            Action::Attack(_) | Action::AirAttack(_) | Action::Cast(_) | Action::AirCast(_)
        ) && player.frame < t::COMMAND_ARMOUR
    }

    /// A light hit that only deals its damage now: the flow states' armour
    /// or a command's first tenth of a second.
    pub fn shrugs_strike(&self, player: &Player, strike: Strike) -> bool {
        !strike.knock.heavy() && (self.armoured() || Self::command_armour(player))
    }

    /// Whether the other Sora's hits miss him now: his invulnerable ticks,
    /// Aerial Recovery and a Risk Dodge (the reference's), and while he is
    /// down [duel choice]; a flinch or a body in the air takes every hit
    /// (no invulnerability after a hit, so a juggle goes on); otherwise
    /// the KH2 motion's own frames.
    pub fn untouchable(&self, player: &Player, clips: &Clips) -> bool {
        if player.invulnerable > 0. {
            return true;
        }
        match self.mode {
            Mode::Down { .. } | Mode::Recover { .. } | Mode::Risk { .. } => true,
            Mode::Hurt { .. } => false,
            _ => {
                if matches!(player.action, Action::Hurt(i) if i < hurt::BLOW[0]) {
                    return false;
                }
                player.range_invulnerable(clips)
            }
        }
    }

    /// A hit lands on him: the KH2 controller's own state ends and the
    /// reaction begins along `dir` (from the attacker toward him), on the
    /// front or back motion by where it came from. A body already knocked
    /// into the air takes the new hit's speed on the motion it shows (its
    /// motion, voice and facing do not start over). Returns whether a
    /// motion started.
    pub fn hurt(
        &mut self,
        player: &mut Player,
        strike: Strike,
        dir: [f32; 2],
        high: bool,
        clips: &Clips,
    ) -> bool {
        let air = player.in_air() || (self.owns() && self.airborne());
        let flying = match self.mode {
            Mode::Hurt {
                air: true,
                back,
                knock,
                high,
                ..
            } => Some((back, Self::hurt_motion(knock, true, back, high, clips))),
            _ => None,
        };
        let back = flying.map(|f| f.0).unwrap_or_else(|| {
            let f = facing(player.yaw);
            f[0] * dir[0] + f[1] * dir[1] >= 0.
        });
        // What he was doing ends.
        player.cast = None;
        player.pending_jump = None;
        player.knock = None;
        player.flight = None;
        player.glide = None;
        player.quick_run = None;
        player.dodge_ready = false;
        player.jump_cut = crate::JumpCut::default();
        player.combo = ptya::Combo::default();
        player.connected = false;
        player.queued.clear();
        player.sampler.reset();
        player.vertical_speed = 0.;
        player.air_velocity = [0.; 2];
        self.aim = None;
        self.traction = 0.;
        self.lean_target = [0.; 2];
        self.counter = 0;
        self.chase_offer = 0;
        self.risk_offer = None;
        self.flow = None;
        let s = strike.scale;
        let along = |v: f32| [dir[0] * v * s, dir[1] * v * s];
        let (h, vy) = match strike.knock {
            Knock::Small => (along(t::SMALL_KNOCK), 0.),
            Knock::Blow | Knock::SideSpin => (along(t::BLOW_SPEED), t::BLOW_UP),
            Knock::Toss => ([0.; 2], t::TOSS_UP * s),
            // [duel choice] The roll has no table of its own: a toss that
            // also carries him off at a flinch's speed.
            Knock::VerticalRoll => (along(t::SMALL_KNOCK), t::TOSS_UP * s),
            Knock::Launch => ([0.; 2], t::LAUNCH_UP * s),
            Knock::Parabola => (along(t::PARABOLA), t::PARABOLA * s),
            Knock::Beat => (along(t::SMALL_KNOCK), -t::BEAT_DOWN),
        };
        self.velocity = [h[0], vy, h[1]];
        let flies = air || strike.knock != Knock::Small;
        // A ground flinch keeps the facing; the others turn him to (or
        // away from) the attacker.
        if flies && flying.is_none() {
            player.yaw = if back {
                yaw_of(dir)
            } else {
                yaw_of([-dir[0], -dir[1]])
            };
        }
        let name = self.mode.name();
        self.mode = Mode::Hurt {
            knock: strike.knock,
            tick: 0,
            back,
            air: flies,
            high,
            apex: 0,
            fell: 0,
        };
        let action = Action::Hurt(Self::hurt_motion(strike.knock, flies, back, high, clips));
        let again = flying.is_some_and(|(_, shown)| Action::Hurt(shown) == action);
        if self.mode.name() != name {
            self.pending.push(self.mode.name());
        }
        if !again {
            let mut step = Step::default();
            self.shown = None;
            self.show(player, action, clips, &mut step);
        }
        !again
    }

    /// The KH2 damage motion for a reaction: the flinches by front / back
    /// and high / low (the partner when one is missing), the air flinch,
    /// the blow.
    fn hurt_motion(knock: Knock, flies: bool, back: bool, high: bool, clips: &Clips) -> usize {
        if !flies {
            let mut i = if back { 2 } else { 0 } + (!high) as usize;
            if i >= clips.hurt.len() {
                i ^= 1;
            }
            return i;
        }
        if knock == Knock::Small {
            hurt::AIR[back as usize]
        } else {
            hurt::BLOW[back as usize]
        }
    }

    /// A guarded hit: the reference's push-back (light or heavy, by the
    /// strike), and a heavy one offers the Risk Dodge toward `attacker`
    /// (his feet; `attacker_air` when he is off the ground).
    pub fn guarded(
        &mut self,
        player: &mut Player,
        strike: Strike,
        dir: [f32; 2],
        attacker: Vec3,
        attacker_air: bool,
    ) {
        let push = t::GUARD_PUSH[strike.knock.heavy() as usize] * strike.scale;
        player.recoil(dir, push);
        if strike.knock.heavy() {
            self.risk_offer = Some((t::RISK_OFFER, attacker, attacker_air));
        }
    }

    /// His hit landed with `strike`: the flow attack's count, and a
    /// counter that blew the other Sora away offers Final Blow.
    pub fn struck(&mut self, source: Source, strike: Strike) {
        if matches!(source, Source::Flow { .. } | Source::Chase { .. }) {
            self.flow_hits = self.flow_hits.saturating_add(1);
        }
        if source == Source::Counter && strike.knock.blows_away() {
            self.chase_offer = t::CHASE_OFFER;
        }
    }

    /// A reaction's tick.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn hurt_tick(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Mode::Hurt {
            knock,
            tick,
            back,
            air,
            high,
            mut apex,
            mut fell,
        } = self.mode
        else {
            return;
        };
        let tick = tick + 1;
        let (m, _) = stick(input);
        let acted = input.attack || input.guard || input.jump || m >= t::STICK_ACTIVE;
        let motion = Action::Hurt(Self::hurt_motion(knock, air, back, high, clips));
        if !air {
            // The ground flinch: the slide, then he may act.
            if tick >= t::SMALL_ACT && acted || tick >= t::SMALL_END {
                self.velocity = [0.; 3];
                self.mode = Mode::Ground;
                self.ground(player, input, p, clips, world, scene, out);
                return;
            }
            self.velocity[0] *= t::SMALL_DECAY;
            self.velocity[2] *= t::SMALL_DECAY;
            self.mode = Mode::Hurt {
                knock,
                tick,
                back,
                air,
                high,
                apex,
                fell,
            };
            self.show(player, motion, clips, &mut out.step);
            let v = [self.velocity[0], 0., self.velocity[2]];
            let moved = Self::travel(player, v, false, p, world);
            if !moved.on_floor {
                self.mode = Mode::Hurt {
                    knock,
                    tick,
                    back,
                    air: true,
                    high,
                    apex,
                    fell,
                };
            }
            return;
        }
        if knock == Knock::Small {
            // The air flinch: braked level, then falling.
            if tick >= t::SMALL_ACT && acted || tick >= t::SMALL_AIR_END {
                self.mode = Mode::Air {
                    jump: Jump::None,
                    hang: t::HANG_TICKS,
                    released: true,
                    tick: 0,
                };
                self.air(player, input, p, clips, world, scene, out);
                return;
            }
            let h = [self.velocity[0], self.velocity[2]];
            let l = h[0].hypot(h[1]);
            if l > t::SMALL_AIR_STOP {
                let k = (l - t::SMALL_AIR_BRAKE).max(0.) / l;
                self.velocity = [h[0] * k, 0., h[1] * k];
            } else {
                self.velocity[1] = (self.velocity[1] - t::GRAVITY).max(-t::FALL_MAX);
            }
        } else {
            // Aerial Recovery out of a blow, a toss, a launch or a parabola.
            if tick >= t::RECOVERY_FROM
                && input.jump
                && knock != Knock::Beat
                && self.has(A::AerialRecovery)
            {
                self.velocity = [0.; 3];
                self.double = true;
                self.mode = Mode::Recover { tick: 0 };
                self.show(player, Action::Hurt(hurt::RECOVERY), clips, &mut out.step);
                return;
            }
            if knock == Knock::Parabola
                && tick >= t::PARABOLA_ACT
                && input.attack
                && self.swing(player, input, scene, clips, out, true)
            {
                return;
            }
            let decay = match knock {
                Knock::Parabola => {
                    if self.velocity[0].hypot(self.velocity[2]) > t::PARABOLA_KEEP {
                        t::PARABOLA_DECAY
                    } else {
                        1.
                    }
                }
                Knock::Beat => t::SMALL_DECAY,
                _ => t::BLOW_DECAY,
            };
            self.velocity[0] *= decay;
            self.velocity[2] *= decay;
            let vy = self.velocity[1];
            let level = matches!(knock, Knock::Blow | Knock::SideSpin)
                && self.velocity[0].hypot(self.velocity[2]) > t::BLOW_GRAVITY_BELOW;
            self.velocity[1] = if knock == Knock::Beat || level {
                vy
            } else if vy > 0. {
                let next = vy - t::HURT_RISE_GRAVITY;
                if next <= 0. {
                    apex = t::HURT_APEX_TICKS;
                }
                next.max(0.)
            } else if apex > 0 {
                apex -= 1;
                vy - t::HURT_APEX_GRAVITY
            } else {
                (vy - t::HURT_GRAVITY).max(-t::FALL_MAX)
            };
            if self.velocity[1] < 0. {
                fell += 1;
            }
            // A launch ends in the air: he falls as from a jump.
            if knock == Knock::Launch && fell >= t::LAUNCH_FALL_END {
                let v = self.velocity;
                self.fall(player, v, clips, &mut out.step);
                return;
            }
        }
        self.mode = Mode::Hurt {
            knock,
            tick,
            back,
            air,
            high,
            apex,
            fell,
        };
        self.show(player, motion, clips, &mut out.step);
        let v = self.velocity;
        let moved = Self::travel(player, v, true, p, world);
        if moved.on_floor && self.velocity[1] <= 0. {
            if knock == Knock::Small {
                self.land(player, clips, &mut out.step);
            } else {
                // Down: the knock-down and the getting up.
                self.velocity = [0.; 3];
                self.mode = Mode::Down { tick: 0, back };
                step_landed(&mut out.step);
                self.show(
                    player,
                    Action::Hurt(hurt::BLOW_LAND[back as usize]),
                    clips,
                    &mut out.step,
                );
            }
        }
    }

    /// Down after a heavy hit: up at the end, or on a button from the
    /// change point; a moment's invulnerability after [duel choice].
    #[allow(clippy::too_many_arguments)]
    pub(super) fn down_tick(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Mode::Down { tick, back } = self.mode else {
            return;
        };
        let tick = tick + 1;
        let (m, _) = stick(input);
        let acted = input.attack || input.guard || input.jump || m >= t::STICK_ACTIVE;
        if tick >= t::DOWN_ACT && acted || tick >= t::DOWN_END {
            player.invulnerable = player.invulnerable.max(t::WAKE_GUARD as f32);
            self.mode = Mode::Ground;
            self.ground(player, input, p, clips, world, scene, out);
            return;
        }
        self.mode = Mode::Down { tick, back };
        self.velocity = [0.; 3];
        self.show(
            player,
            Action::Hurt(hurt::BLOW_LAND[back as usize]),
            clips,
            &mut out.step,
        );
    }

    /// Aerial Recovery: in place, invulnerable; Doubleflight, an air slide
    /// or a command from its change point; then a fall.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn recover_tick(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Mode::Recover { tick } = self.mode else {
            return;
        };
        let tick = tick + 1;
        player.invulnerable = player.invulnerable.max(2.);
        // Payback Strike: Attack out of the recovery strikes back at the one
        // who knocked him (KH3's BlowCounter, armed by BLOW_COUNTER on the
        // recovery's dash toward the attacker, 0x141332930).
        if input.attack
            && self.has(A::PaybackStrike)
            && scene.foe.is_some()
            && scene.book.is_some()
            && self.start_attack(
                player,
                combo::id::BLOW_COUNTER,
                false,
                true,
                scene,
                &mut out.step,
                clips,
            )
        {
            out.began.push("payback_strike");
            return;
        }
        if input.attack
            && self.has(A::PaybackStrike)
            && scene.foe.is_some()
            && self.play_record(
                player,
                combo::stand_in(combo::id::BLOW_COUNTER),
                input,
                clips,
            )
        {
            self.mode = Mode::Kh2;
            self.shown = None;
            out.began.push("payback_strike");
            return;
        }
        if tick >= t::RECOVERY_CHANGE && (input.jump || input.attack || input.guard) {
            self.mode = Mode::Air {
                jump: Jump::None,
                hang: t::HANG_TICKS,
                released: true,
                tick: 0,
            };
            self.velocity = [0.; 3];
            self.air(player, input, p, clips, world, scene, out);
            return;
        }
        if tick >= t::RECOVERY_TICKS {
            self.fall(player, [0.; 3], clips, &mut out.step);
            return;
        }
        self.mode = Mode::Recover { tick };
        self.velocity = [0.; 3];
        self.show(player, Action::Hurt(hurt::RECOVERY), clips, &mut out.step);
    }

    /// Final Blow: the attack button while the chase is offered.
    pub(super) fn start_chase(
        &mut self,
        player: &mut Player,
        scene: &Scene,
        clips: &Clips,
        step: &mut Step,
    ) -> bool {
        if self.chase_offer == 0 || scene.foe.is_none() || !self.has(A::FinalBlow) {
            return false;
        }
        self.chase_offer = 0;
        self.counter = 0;
        self.velocity = [0.; 3];
        self.aim = None;
        self.mode = Mode::Chase { tick: 0 };
        player.invulnerable = player.invulnerable.max(t::CHASE_WARP as f32 + 1.);
        self.show(
            player,
            Action::Growth(growth::motion::QUICK_RUN),
            clips,
            step,
        );
        true
    }

    /// The chase's warp, then its attack (the second air finisher's record
    /// stands in for the chase attack).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn chase_tick(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Mode::Chase { tick } = self.mode else {
            return;
        };
        let tick = tick + 1;
        let Some(foe) = scene.foe else {
            self.fall(player, [0.; 3], clips, &mut out.step);
            return;
        };
        if tick < t::CHASE_WARP {
            self.mode = Mode::Chase { tick };
            self.show(
                player,
                Action::Growth(growth::motion::QUICK_RUN),
                clips,
                &mut out.step,
            );
            return;
        }
        // 150 short of the target on the line to it, 100 under it.
        let d = [
            foe.position[0] - player.position[0],
            foe.position[2] - player.position[2],
        ];
        let l = d[0].hypot(d[1]).max(1e-3);
        let u = [d[0] / l, d[1] / l];
        let to = [
            foe.position[0] - u[0] * t::CHASE_SHORT,
            (foe.centre[1] - t::CHASE_UNDER - t::CHEST).max(player.position[1]),
            foe.position[2] - u[1] * t::CHASE_SHORT,
        ];
        let disp = [
            to[0] - player.position[0],
            to[1] - player.position[1],
            to[2] - player.position[2],
        ];
        Self::travel(player, disp, true, p, world);
        player.yaw = yaw_of(u);
        out.began.push("chase_warp");
        if scene.book.is_some() {
            if !self.start_attack(player, 0x22, false, true, scene, &mut out.step, clips) {
                self.fall(player, [0.; 3], clips, &mut out.step);
            }
            return;
        }
        if !self.start_flow(player, record::WALL_ATTACK, input, clips) {
            self.fall(player, [0.; 3], clips, &mut out.step);
            return;
        }
        self.flow = Some((record::WALL_ATTACK, true));
    }

    /// The duel's Risk Dodge: Jump while it is offered (a heavy hit was
    /// just guarded).
    pub(super) fn start_risk(
        &mut self,
        player: &mut Player,
        clips: &Clips,
        step: &mut Step,
    ) -> bool {
        let Some((_, attacker, air)) = self.risk_offer.filter(|_| self.has(A::RiskDodge)) else {
            return false;
        };

        self.risk_offer = None;
        let d = [
            attacker[0] - player.position[0],
            attacker[2] - player.position[2],
        ];
        let l = d[0].hypot(d[1]).max(1e-3);
        let u = [d[0] / l, d[1] / l];
        // To just behind him, the slide's length kept within its limits.
        let reach = (l + t::RISK_BEHIND).clamp(t::RISK_MIN, t::RISK_MAX);
        let to = [
            player.position[0] + u[0] * reach,
            if air { attacker[1] } else { player.position[1] },
            player.position[2] + u[1] * reach,
        ];
        player.yaw = yaw_of(u);
        player.invulnerable = player.invulnerable.max(t::RISK_SLIDE_TICKS as f32 + 1.);
        self.velocity = [0.; 3];
        self.aim = None;
        self.counter = 0;
        self.mode = Mode::Risk {
            tick: 0,
            from: player.position,
            to,
            warp: air,
        };
        let motion = if air {
            Action::Growth(growth::motion::QUICK_RUN)
        } else {
            Action::Growth(growth::motion::DODGE_ROLL)
        };
        self.show(player, motion, clips, step);
        true
    }

    /// The slide under him (or, at one in the air, the warp behind him),
    /// turning round; the counter after.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn risk_tick(
        &mut self,
        player: &mut Player,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        out: &mut Outcome,
    ) {
        let Mode::Risk {
            tick,
            from,
            to,
            warp,
        } = self.mode
        else {
            return;
        };
        let tick = tick + 1;
        player.invulnerable = player.invulnerable.max(2.);
        let (share, last) = if warp {
            (
                if tick >= t::RISK_WARP { 1. } else { 0. },
                tick >= t::RISK_WARP,
            )
        } else {
            let u = tick as f32 / t::RISK_SLIDE_TICKS as f32;
            (1. - (1. - u.min(1.)).powi(2), tick >= t::RISK_SLIDE_TICKS)
        };
        let want = [
            from[0] + (to[0] - from[0]) * share,
            from[1] + (to[1] - from[1]) * share,
            from[2] + (to[2] - from[2]) * share,
        ];
        let disp = [
            want[0] - player.position[0],
            if warp {
                want[1] - player.position[1]
            } else {
                0.
            },
            want[2] - player.position[2],
        ];
        let moved = Self::travel(player, disp, warp, p, world);
        self.mode = Mode::Risk {
            tick,
            from,
            to,
            warp,
        };
        let motion = if warp {
            Action::Growth(growth::motion::QUICK_RUN)
        } else {
            Action::Growth(growth::motion::DODGE_ROLL)
        };
        self.show(player, motion, clips, &mut out.step);
        if last {
            // Round to face him, and the counter is offered.
            player.yaw = wrap(player.yaw + PI);
            self.counter = t::RISK_COUNTER;
            self.risk_counter = Some(if warp { 0x2b } else { 0x28 });
            if warp && !moved.on_floor {
                self.fall(player, [0.; 3], clips, &mut out.step);
            } else {
                self.velocity = [0.; 3];
                self.mode = Mode::Ground;
            }
        }
    }
}

fn step_landed(step: &mut Step) {
    step.landed = true;
}
