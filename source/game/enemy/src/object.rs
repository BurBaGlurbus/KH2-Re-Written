//! Object lifecycle and the per-tick order of an enemy: creation and its
//! script triggers (FUN_1403b44d0), the script tick (FUN_1403b4460: the main
//! thread, then the act manager when the main thread has not ended) and the
//! act manager loop (FUN_1403ca550).

use crate::act::{self, Action};
use crate::env::Env;
use crate::motion::Effect;
use crate::script::{Callback, Instance, Slot, Thread};
use crate::sysobj::Sysobj;
use crate::world::{Event, ObjId, Object, ScriptSpace, World, object_handle, run_thread};

/// Script trigger keys (OBJ_INIT, OBJ_MAIN, OBJ_DAMAGE, OBJ_FALL ...).
pub const INIT: i32 = 0;
pub const MAIN: i32 = 1;
pub const DAMAGE: i32 = 6;
pub const FALL: i32 = 13;
pub const LAND: i32 = 14;
pub const ATTACK: i32 = 15;
/// Triggers the motion events fire: frame event 1 (effect or sound) and 7.
pub const EVENT_EFFECT: i32 = 16;
pub const EVENT_SCRIPT: i32 = 17;

/// Frame slots of the object's threads (see [`crate::script`]).
const MAIN_SLOT: u32 = 0;
const ACT_SLOT: u32 = 1;
const EVENT_SLOT: u32 = 2;

#[derive(Debug)]
pub enum SpawnError {
    Program,
}

impl Object {
    /// FUN_1403b44d0: build the object around a script. OBJ_INIT runs to its
    /// first yield on a throw-away thread with the object as its argument; then
    /// the persistent OBJ_MAIN thread is created.
    pub fn spawn(
        world: &mut World,
        env: &dyn Env,
        id: ObjId,
        payload: &[u8],
        sys: Sysobj,
    ) -> Result<Object, SpawnError> {
        let instance = Instance::new(id, payload).ok_or(SpawnError::Program)?;
        let init = instance.trigger(INIT);
        let main = instance.trigger(MAIN);
        let (stack_bytes, temp_bytes) = (instance.stack_bytes, instance.temp_bytes);
        let mut obj = Object::bare(id);
        obj.sys = sys;
        obj.script = ScriptSpace {
            instance,
            stack_bytes,
            temp_bytes,
            main: None,
            main_ended: false,
            act_thread: None,
            act_generation: 0,
            faults: Vec::new(),
        };
        if init > 0 {
            let mut t = Thread::with_sizes(
                stack_bytes,
                temp_bytes,
                EVENT_SLOT,
                init,
                &[Slot::address(object_handle(id))],
            );
            run_thread(world, env, &mut obj, &mut t);
        }
        if main > 0 {
            obj.script.main = Some(Thread::with_sizes(
                stack_bytes,
                temp_bytes,
                MAIN_SLOT,
                main,
                &[],
            ));
        } else {
            obj.script.main_ended = true;
        }
        Ok(obj)
    }

    /// FUN_1403e1840: fire a trigger on a throw-away thread, run once.
    pub fn fire(&mut self, world: &mut World, env: &dyn Env, key: i32, args: &[Slot]) {
        let entry = self.script.instance.trigger(key);
        if entry <= 0 {
            return;
        }
        let mut t = Thread::with_sizes(
            self.script.stack_bytes,
            self.script.temp_bytes,
            EVENT_SLOT,
            entry,
            args,
        );
        run_thread(world, env, self, &mut t);
    }

    /// An attack's hit callback (`attack_set_hit_callback`): `entry` runs on a
    /// throw-away thread, until its first yield, with the script's own
    /// argument, the object the attack struck and one more word.
    pub fn run_hit_callback(
        &mut self,
        world: &mut World,
        env: &dyn Env,
        callback: (i32, u32),
        struck: u32,
    ) {
        let (entry, ai) = callback;
        if entry <= 0 {
            return;
        }
        let struck = world.ai_reference(struck);
        let mut t = Thread::with_sizes(
            self.script.stack_bytes,
            self.script.temp_bytes,
            EVENT_SLOT,
            entry,
            &[Slot::address(ai), Slot::address(struck), Slot::int(0)],
        );
        run_thread(world, env, self, &mut t);
    }

    /// Turn the act manager's requests into thread changes. Inside the
    /// interpreter callbacks are queued (it runs them after the trap); outside
    /// they run now.
    pub fn apply_actions(&mut self, world: Option<(&mut World, &dyn Env)>, actions: Vec<Action>) {
        let mut later = Vec::new();
        for action in actions {
            match action {
                Action::Kill => {
                    self.script.act_thread = None;
                    self.script.act_generation = self.script.act_generation.wrapping_add(1);
                }
                Action::Spawn { entry } => {
                    self.script.act_thread = Some(Thread::with_sizes(
                        self.script.stack_bytes,
                        self.script.temp_bytes,
                        ACT_SLOT,
                        i32::from(entry),
                        &[Slot::address(self.acts.ai_object)],
                    ));
                    self.script.act_generation = self.script.act_generation.wrapping_add(1);
                }
                Action::Callback { word, .. } => later.push(Callback {
                    entry: word,
                    args: vec![Slot::address(self.acts.ai_object)],
                }),
            }
        }
        match world {
            None => self.pending.extend(later),
            Some((world, env)) => {
                for cb in later {
                    let mut t = Thread::with_sizes(
                        self.script.stack_bytes,
                        self.script.temp_bytes,
                        EVENT_SLOT,
                        i32::from(cb.entry),
                        &cb.args,
                    );
                    run_thread(world, env, self, &mut t);
                }
            }
        }
    }

    /// Run a script entry on a throw-away thread of this object (the
    /// executable's FUN_1403e2000 with three AI object arguments).
    fn run_callback(&mut self, world: &mut World, env: &dyn Env, entry: i32, args: &[Slot]) {
        let mut t = Thread::with_sizes(
            self.script.stack_bytes,
            self.script.temp_bytes,
            EVENT_SLOT,
            entry,
            args,
        );
        run_thread(world, env, self, &mut t);
    }

    /// An attack this object's script made struck `victim` (FUN_1403d1730):
    /// the attack records the strike, and the script's hit callback runs with
    /// the AI object it was registered with, the victim and the attack. The
    /// callback runs on `self`; an attack registered by another object calls
    /// that object's script instead.
    pub fn attack_struck(&mut self, world: &mut World, env: &dyn Env, slot: usize, victim: ObjId) {
        let Some(Some(a)) = world.attacks.get_mut(slot) else {
            return;
        };
        a.flags |= crate::attack::flag::STRUCK;
        let (callback, by) = (a.hit_callback, a.callback_object);
        let Some((entry, ai)) = callback else {
            return;
        };
        let args = [
            Slot::address(ai),
            Slot::address(object_handle(victim)),
            Slot::address(crate::world::ATTACK_HANDLES + 0x100 * slot as u32),
        ];
        self.run_callback_of(world, env, by, entry, &args);
    }

    /// BTLOBJ vt+0xe0 (FUN_1403f8d30 / FUN_1403dbbd0 -> FUN_140410ce0): an
    /// attack of this object reached `victim` (FUN_1403d1730 calls it after
    /// the hit whether or not the victim could be damaged); the script's
    /// OBJ_ATTACK gets the victim and the attack. An attack the motion made
    /// (a range-10 window, FUN_1403dafe0) has no object here: `slot` None
    /// stands a record of `param_id` in for the call.
    pub fn attack_reached(
        &mut self,
        world: &mut World,
        env: &dyn Env,
        victim: ObjId,
        slot: Option<usize>,
        param_id: i32,
    ) {
        if self.script.instance.trigger(ATTACK) <= 0 {
            return;
        }
        let (slot, transient) = match slot {
            Some(s) => (s, false),
            None => {
                let mut a =
                    crate::attack::Attack::new(Some(self.id), param_id, -1, 0, self.sys.team);
                world.attack_serial += 1;
                a.serial = world.attack_serial;
                (world.add_attack(a), true)
            }
        };
        let args = [
            Slot::address(object_handle(victim)),
            Slot::address(crate::world::ATTACK_HANDLES + 0x100 * slot as u32),
        ];
        self.fire(world, env, ATTACK, &args);
        if transient && let Some(s) = world.attacks.get_mut(slot) {
            *s = None;
        }
    }

    /// A guard turned the attack back (FUN_1403d2140): it is marked reflected
    /// by `reflecter`, and the reflect callback runs with the AI object it
    /// was registered with, the attack and the reflecter.
    pub fn attack_reflected(
        &mut self,
        world: &mut World,
        env: &dyn Env,
        slot: usize,
        reflecter: ObjId,
    ) {
        let Some(Some(a)) = world.attacks.get_mut(slot) else {
            return;
        };
        a.reflect_count = a.reflect_count.wrapping_add(1);
        a.flags |= crate::attack::flag::REFLECTED;
        a.reflecter = Some(reflecter);
        let (callback, by) = (a.reflect_callback, a.callback_object);
        let Some((entry, ai)) = callback else {
            return;
        };
        let args = [
            Slot::address(ai),
            Slot::address(crate::world::ATTACK_HANDLES + 0x100 * slot as u32),
            Slot::address(object_handle(reflecter)),
        ];
        self.run_callback_of(world, env, by, entry, &args);
    }

    /// Run a callback on the object that registered it: `self`, or one of the
    /// world's.
    fn run_callback_of(
        &mut self,
        world: &mut World,
        env: &dyn Env,
        by: Option<ObjId>,
        entry: i32,
        args: &[Slot],
    ) {
        match by {
            Some(id) if id != self.id => {
                if let Some(mut other) = world.objects.get_mut(id as usize).and_then(Option::take) {
                    other.run_callback(world, env, entry, args);
                    world.objects[id as usize] = Some(other);
                }
            }
            _ => self.run_callback(world, env, entry, args),
        }
    }

    /// One frame of this object's script (FUN_1403b4460).
    /// The script's main thread only: a summon taking part in its limit,
    /// whose think and acts the hooked limit blocks (FUN_1401b15f0 with
    /// FUN_1403d7280).
    pub fn tick_main(&mut self, world: &mut World, env: &dyn Env) {
        let Some(mut main) = self.script.main.take() else {
            return;
        };
        run_thread(world, env, self, &mut main);
        if main.finished() {
            self.script.main_ended = true;
        } else {
            self.script.main = Some(main);
        }
    }

    pub fn tick_script(&mut self, world: &mut World, env: &dyn Env) {
        let Some(mut main) = self.script.main.take() else {
            return;
        };
        run_thread(world, env, self, &mut main);
        if main.finished() {
            self.script.main_ended = true;
        } else {
            self.script.main = Some(main);
            self.tick_acts(world, env);
        }
    }

    /// A shout reached this object (FUN_1403ca950): if its act table has an
    /// act of that name, the act's timeout routine runs with this object's AI
    /// object and the shouter's.
    pub fn deliver_shout(&mut self, world: &mut World, env: &dyn Env, name: &[u8], shouter: u32) {
        let Some(index) = self.acts.find(name) else {
            return;
        };
        let timeout = self.acts.table[index].timeout;
        if timeout == 0 {
            return;
        }
        let args = [Slot::address(self.acts.ai_object), Slot::address(shouter)];
        self.run_callback(world, env, i32::from(timeout), &args);
    }

    /// FUN_1403ca550.
    fn tick_acts(&mut self, world: &mut World, env: &dyn Env) {
        if !self.acts.has_table {
            return;
        }
        // A shout that came with a delay arrives when its timer runs out.
        if let Some(pending) = self.acts.pending_shout.as_mut() {
            pending.timer -= world.frame_time;
            if pending.timer <= 0. {
                let done = self.acts.pending_shout.take().expect("pending shout");
                self.deliver_shout(world, env, &done.name, done.shouter);
            }
        }
        self.acts.flags |= act::JUST_STARTED;
        let mut highest = 0u16;
        loop {
            if !self.acts.locked() && self.sys.act_paused {
                return;
            }
            let mut actions = Vec::new();
            let started = self.acts.pop(&mut actions);
            self.apply_actions(Some((world, env)), actions);
            if !started && self.acts.flags & act::JUST_STARTED == 0 {
                return;
            }
            let flags = self.acts.flags;
            self.acts.flags &= !act::JUST_STARTED;
            if self.script.act_thread.is_none() {
                continue;
            }
            if flags & act::GATED != 0 {
                if self.sys.air_blocked() || self.sys.no_control() {
                    return;
                }
                self.acts.flags &= !act::GATED;
            }
            highest = highest.max(self.acts.priority);
            if highest < 300 && self.sys.hit_stunned() {
                return;
            }
            let generation = self.script.act_generation;
            let mut thread = self.script.act_thread.take().expect("act thread");
            run_thread(world, env, self, &mut thread);
            let replaced = generation != self.script.act_generation;
            let ended = thread.finished();
            if !replaced && !ended {
                self.script.act_thread = Some(thread);
            }
            if ended && self.acts.flags & act::JUST_STARTED == 0 {
                let mut actions = Vec::new();
                self.acts.thread_ended(&mut actions);
                self.apply_actions(Some((world, env)), actions);
            }
        }
    }

    /// One frame, in the executable's order: the motion clock and its events,
    /// the script (main thread, then acts), then root motion and physics.
    pub fn tick(&mut self, world: &mut World, env: &dyn Env) {
        self.decay_revenge(world.frame_time);
        // The tail of BTLOBJ vt+0x10, FUN_1403b4980: the movement request
        // (+0x10 direction, +0x1c speed) lasts one tick; a script that wants
        // to keep moving sets it again.
        self.sys.move_direction = [0.; 3];
        self.sys.move_speed = 0.;
        self.sys.begin_tick(env);
        self.carry_out_motion_effects(world, env);
        // FUN_1401b15f0: a friend's think runs before its acts (FUN_1403b4f80),
        // so a script's movement request has the last word.
        self.tick_friend(world, env);
        self.tick_script(world, env);
        self.tick_threads(world, env);
        self.sys.defer_ground = true;
        self.sys.end_tick(env);
        self.finish_ground(world, env);
    }

    /// FUN_1403b9090's ground transition, after the solve: the act manager's
    /// event (FUN_1403ca1d0: 1 leaving the ground, 2 landing), then the
    /// script's trigger 0xd or 0xe (FUN_1403e1840, no arguments), then the
    /// engine's own leave or land, each only while `+0x120` bit 26 (leave) or
    /// 27 (land) stays clear: `obj_exec_fall` and `obj_exec_land` set them
    /// when the script has done the move itself (Soldier's OBJ_FALL jumps
    /// with height 0 and plays its fall motion).
    fn finish_ground(&mut self, world: &mut World, env: &dyn Env) {
        let Some(ground) = self.sys.ground.take() else {
            return;
        };
        let (landing, bit, trigger) = match ground {
            crate::sysobj::Ground::Leave => (false, 0x400_0000, 0xd),
            crate::sysobj::Ground::Land => (true, 0x800_0000, 0xe),
        };
        let actions = self.acts.ground_event(landing);
        self.apply_actions(Some((world, env)), actions);
        if self.sys.flags & bit == 0 {
            self.fire(world, env, trigger, &[]);
        }
        if self.sys.flags & bit == 0 {
            let clips = env.clips(self.sys.model);
            if landing {
                self.sys.land(clips);
            } else {
                self.sys.finish_leave(clips);
            }
        }
        self.sys.leave_velocity = None;
    }

    /// One step of each side thread of this object's program (the magic
    /// task FUN_1403c55f0, skipped while DAT_142a11400 bit 5 holds; the
    /// hooked object's FUN_1403d7280). A hook its holder dropped (a new
    /// hook or `obj_hook_stop`) is gone; a thread whose code returned is
    /// unlinked, and a finished hook frees its holder's slot
    /// (FUN_1403d6f80). [I] All are stepped here, after the script.
    pub fn tick_threads(&mut self, world: &mut World, env: &dyn Env) {
        if self.threads.is_empty() || world.status & 1 << 5 != 0 {
            return;
        }
        let mut kept = Vec::new();
        for mut t in std::mem::take(&mut self.threads) {
            if let Some(holder) = t.hooked
                && self.hook_of(world, holder) != Some(t.handle)
            {
                continue;
            }
            run_thread(world, env, self, &mut t.thread);
            if !t.thread.finished() {
                kept.push(t);
            } else if let Some(holder) = t.hooked {
                self.clear_hook(world, holder, t.handle);
            }
        }
        // Threads the steps started come after the old ones.
        kept.append(&mut self.threads);
        self.threads = kept;
    }

    fn hook_of(&self, world: &World, holder: ObjId) -> Option<u32> {
        if holder == self.id {
            return self.sys.hook;
        }
        if holder == crate::world::PLAYER_OBJECT {
            return world.player_proxy.as_ref()?.sys.hook;
        }
        world.objects.get(holder as usize)?.as_ref()?.sys.hook
    }

    fn clear_hook(&mut self, world: &mut World, holder: ObjId, handle: u32) {
        let sys = if holder == self.id {
            Some(&mut self.sys)
        } else if holder == crate::world::PLAYER_OBJECT {
            world.player_proxy.as_mut().map(|o| &mut o.sys)
        } else {
            world
                .objects
                .get_mut(holder as usize)
                .and_then(|o| o.as_mut())
                .map(|o| &mut o.sys)
        };
        if let Some(s) = sys
            && s.hook == Some(handle)
        {
            s.hook = None;
        }
    }

    /// BTLOBJ vt+0x10 of the enemy classes (FUN_1403db440 / FUN_1403dbbf0),
    /// before the motion step: the revenge gauge falls by the frame time,
    /// not below 0, while the act manager is active and the running act is
    /// not still waiting on its gate.
    pub fn decay_revenge(&mut self, frame_time: f32) {
        let s = &mut self.sys;
        if s.class_flags & crate::sysobj::class_flag::ENEMY != 0
            && s.revenge_decays(&self.acts)
            && self.acts.flags & act::GATED == 0
            && s.revenge > 0.
        {
            s.revenge = (s.revenge - frame_time).max(0.);
        }
    }

    /// A summon's kn::Friend update (FUN_1401b15f0): the think, then what
    /// it asked for: a script label (FUN_1403caa90) or a command for the
    /// host (FUN_1401b7d90).
    pub fn tick_friend(&mut self, world: &mut World, env: &dyn Env) {
        if self.friend.is_none() || self.sys.gone {
            return;
        }
        let id = self.id;
        let requests = crate::friend::tick(world, id, self, env);
        if let Some(label) = requests.label {
            let actions = self.acts.start_if_not_outranked(label);
            self.apply_actions(Some((world, env)), actions);
        }
        if let Some((index, target)) = requests.command {
            world.events.push(Event::Service {
                name: "friend_use_command",
                object: Some(id),
                other: Some(target),
                words: [index, 0, 0, 0, 0, 0, 0, 0],
                vector: None,
            });
        }
    }

    /// What the motion events asked for: take-off, and the script triggers
    /// 0x10 (`id`, effect handle) and 0x11 (`p0`, `p1`).
    pub fn carry_out_motion_effects(&mut self, world: &mut World, env: &dyn Env) {
        let effects = std::mem::take(&mut self.sys.effects);
        for effect in effects {
            match effect {
                Effect::Takeoff => self.sys.take_off(env),
                Effect::JumpMode7 => self.sys.enter_mode7(),
                Effect::EffectOrSound { id } => {
                    let handle = world.next_effect_handle();
                    world.events.push(Event::MotionEffect {
                        object: self.id,
                        id,
                        handle,
                    });
                    self.fire(
                        world,
                        env,
                        EVENT_EFFECT,
                        &[
                            Slot::address(object_handle(self.id)),
                            Slot::int(i32::from(id)),
                            Slot::address(handle),
                        ],
                    );
                }
                Effect::ScriptEvent { p0, p1 } => self.fire(
                    world,
                    env,
                    EVENT_SCRIPT,
                    &[
                        Slot::address(object_handle(self.id)),
                        Slot::int(i32::from(p0)),
                        Slot::int(i32::from(p1)),
                    ],
                ),
                Effect::SetSpeed { value } => self.sys.event_speed = f32::from(value) * 0.01,
                Effect::SetParameter { value } => self.sys.event_parameter = u32::from(value),
                Effect::PatternBit { word, bit } => {
                    if word == 0 {
                        self.sys.pattern_a |= 1 << bit;
                    } else {
                        self.sys.pattern_b |= 1 << bit;
                    }
                }
                Effect::Revenge => {
                    // FUN_14040dd70 case 0x1c: an enemy-class object whose
                    // gauge reached its limit starts `revenge` by name
                    // (FUN_1403cac50: no priority compare).
                    let s = &self.sys;
                    if s.class_flags & crate::sysobj::class_flag::ENEMY != 0
                        && s.revenge >= s.karma_limit
                    {
                        let actions = self.acts.start(b"revenge");
                        self.apply_actions(Some((world, env)), actions);
                    }
                }
                Effect::ReactionGroup { kind, p0, p1 } => {
                    self.sys.reaction_groups.range_event(kind, p0, p1)
                }
                // Range 28 (FUN_14040d810 -> FUN_1403d4b10 -> FUN_1403d3a90)
                // and frame event 21: turn toward the target by up to the
                // parameter in degrees, only beyond 50 units horizontally
                // (DAT_1405b532c). [I] Only Sora's object (a limit scene's)
                // turns here; others report the event.
                Effect::Other {
                    range,
                    kind,
                    ref params,
                } if self.id == crate::world::PLAYER_OBJECT
                    && (range && kind == 28 || !range && kind == 21) =>
                {
                    let degrees = f32::from(params.first().copied().unwrap_or(0));
                    if let Some(at) = env.target_position(self.sys.target.id) {
                        let d = [at[0] - self.sys.position[0], at[2] - self.sys.position[2]];
                        if d[0].hypot(d[1]) > 50. {
                            let delta = crate::sysobj::wrap(d[0].atan2(d[1]) - self.sys.yaw);
                            let step = degrees.to_radians();
                            self.sys.yaw =
                                crate::sysobj::wrap(self.sys.yaw + delta.clamp(-step, step));
                        }
                    }
                }
                Effect::Other {
                    range,
                    kind,
                    params,
                } => world.events.push(Event::MotionEvent {
                    object: self.id,
                    range,
                    kind,
                    params,
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests;
