//! Goofy's Bash: the `blade` act of his own object script (`p_ex030`), run by
//! the object-script engine of the `enemy` crate, as `enemy_script` runs an
//! enemy's. The act spawns a thread that detaches his shield from the hand
//! (`obj+0xd60`), flies it as an object of its own with the attack ATKP 1161
//! on it, and puts it back when the throw ends; the act plays motion 192, waits
//! for the thread and plays 193. The host supplies what the engine does not
//! have: the motion clips, the floor, the target's place and the hand bone.
use super::enemy_script::{ScriptAttack, script_attacks};
use bevy::math::{Mat4, Quat, Vec3 as GlamVec3};
use enemy::{
    env::{Clips, Env},
    motion::MotionEvents,
    sysobj::{Sysobj, Target},
    world::{Event, Object, World as EnemyWorld, object_handle},
};
use player::World;
use std::collections::HashMap;
use std::sync::Arc;

/// The ids of the engine objects: Goofy, his shield (weapon slot 0) and the
/// enemy the throw aims at.
const GOOFY: u32 = 0;
const SHIELD: u32 = 1;
const ENEMY: u32 = 2;

/// One motion of Goofy's, as the engine reads it.
pub struct BladeClip {
    pub length: u32,
    pub loop_start: u32,
    pub roots: Vec<Vec3>,
    pub events: MotionEvents,
}

/// The motions the act plays (0, 192, 193, 201).
#[derive(Default)]
pub struct BladeClips(pub HashMap<usize, BladeClip>);

impl Clips for BladeClips {
    fn length(&self, slot: usize) -> u32 {
        self.0.get(&slot).map_or(0, |c| c.length)
    }
    fn loop_start(&self, slot: usize) -> u32 {
        self.0.get(&slot).map_or(0, |c| c.loop_start)
    }
    fn root_delta(&self, slot: usize, tick: u32) -> Vec3 {
        let Some(c) = self.0.get(&slot) else {
            return [0.; 3];
        };
        let at = |t: u32| c.roots.get(t as usize).copied().unwrap_or([0.; 3]);
        let (a, b) = (at(tick), at(tick + 1));
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
    }
    fn events(&self, slot: usize) -> Option<&MotionEvents> {
        self.0.get(&slot).map(|c| &c.events)
    }
}

/// What the engine reads each tick.
struct Surroundings<'a, W: World> {
    world: &'a W,
    clips: &'a BladeClips,
    player: Vec3,
    /// The enemy the throw aims at (the centre of its target shape).
    target: Option<Vec3>,
    /// Goofy's hand bone in model space this tick.
    hand: Mat4,
}

impl<W: World> Env for Surroundings<'_, W> {
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        self.world.floor(x, z, top)
    }
    fn clips(&self, _model: u32) -> &dyn Clips {
        self.clips
    }
    fn player_position(&self) -> Vec3 {
        self.player
    }
    fn target_position(&self, id: u32) -> Option<Vec3> {
        (id == ENEMY).then_some(self.target).flatten()
    }
    fn pick_target(&self, _kind: i32, _from: Vec3) -> Option<u32> {
        self.target.map(|_| ENEMY)
    }
    fn check_step(&self, _: Vec3, _: Vec3, _: f32, _: f32, _: f32) -> Option<u8> {
        Some(0)
    }
    /// Vertical segments only: the floor where it crosses the segment (as
    /// the script brain's host answers it).
    fn line_hit(&self, from: Vec3, to: Vec3) -> Option<Vec3> {
        if from[0] != to[0] || from[2] != to[2] {
            return None;
        }
        let top = from[1].max(to[1]);
        let floor = self.world.floor(from[0], from[2], top)?;
        (from[1].min(to[1]) <= floor && floor <= top).then_some([from[0], floor, from[2]])
    }
    fn bone_point(
        &self,
        object: u32,
        _bone: i32,
        local: Vec3,
        position: Vec3,
        yaw: f32,
    ) -> Option<Vec3> {
        (object == GOOFY).then(|| {
            let p = self.hand.transform_point3(GlamVec3::from_array(local));
            let p = Quat::from_rotation_y(yaw) * p;
            [position[0] + p.x, position[1] + p.y, position[2] + p.z]
        })
    }
}

type Vec3 = [f32; 3];

/// The shield once it is out of the hand.
#[derive(Clone, Copy, Debug)]
pub struct Shield {
    pub position: Vec3,
    pub yaw: f32,
    pub motion: u16,
    pub tick: u32,
}

/// One tick of the act.
pub struct Frame {
    /// Goofy's motion and its tick, and where the act put him.
    pub motion: u16,
    pub tick: u32,
    pub position: Vec3,
    pub yaw: f32,
    pub shield: Option<Shield>,
    /// The attacks the script placed (the shield's ATKP 1161).
    pub attacks: Vec<ScriptAttack>,
    /// Elements of Goofy's own effect container the act started.
    pub effects: Vec<u16>,
    /// The script spent MP (`trap_obj_use_mp`).
    pub spent_mp: bool,
    /// Traps the engine does not run yet, by name.
    pub unsupported: Vec<&'static str>,
    /// The engine's count of motion starts and the blend left on the one
    /// playing.
    pub serial: u64,
    pub blend: f32,
    /// The motions the engine queued behind the playing one (motion, blend).
    pub queue: Vec<(u16, f32)>,
    /// The act and its last motion are over.
    pub done: bool,
}

pub struct Blade {
    world: EnemyWorld,
    goofy: Object,
    clips: Arc<BladeClips>,
}

/// What the act starts from: Goofy's place and body, and the enemy's centre.
pub struct Start {
    pub position: Vec3,
    pub yaw: f32,
    pub stats: player::combat::TargetStats,
    pub sstm: player::actor_physics::Sstm,
    pub target: Option<Vec3>,
}

impl Blade {
    /// The act `act` starts where the member stands, aimed at the enemy.
    pub fn start<W: World>(
        payload: &[u8],
        clips: Arc<BladeClips>,
        room: &W,
        from: Start,
        act: &str,
    ) -> Result<Self, String> {
        let Start {
            position,
            yaw,
            stats,
            sstm,
            target,
        } = from;
        let mut world = EnemyWorld::default();
        world.tolerate_missing = true;
        let mut sys = Sysobj::new(GOOFY, position, yaw, stats, sstm);
        sys.weapons = vec![SHIELD];
        sys.target = Target {
            id: ENEMY,
            kind: 0,
            ..Target::default()
        };
        let mut shield = Object::bare(SHIELD);
        shield.sys.position = position;
        shield.sys.yaw = yaw;
        shield.sys.attached = Some(enemy::sysobj::Attach {
            to: GOOFY,
            bone: 0,
            flags: 0,
            callback: None,
        });
        shield.sys.synced = true;
        world.objects = vec![
            None,
            Some(Box::new(shield)),
            Some(Box::new(Object::bare(ENEMY))),
        ];
        let env = Surroundings {
            world: room,
            clips: &clips,
            player: position,
            target,
            hand: Mat4::IDENTITY,
        };
        let mut goofy = Object::spawn(&mut world, &env, GOOFY, payload, sys)
            .map_err(|e| format!("member script rejected: {e:?}"))?;
        let actions = goofy.acts.start(act.as_bytes());
        goofy.apply_actions(Some((&mut world, &env)), actions);
        Ok(Blade {
            world,
            goofy,
            clips,
        })
    }

    pub fn tick<W: World>(
        &mut self,
        room: &W,
        player: Vec3,
        target: Option<Vec3>,
        hand: Mat4,
    ) -> Frame {
        let env = Surroundings {
            world: room,
            clips: &self.clips,
            player,
            target,
            hand,
        };
        if let Some(e) = self
            .world
            .objects
            .get_mut(ENEMY as usize)
            .and_then(|o| o.as_mut())
            && let Some(t) = target
        {
            e.sys.position = t;
        }
        self.goofy.tick(&mut self.world, &env);
        // The shield is an object of its own: it plays its motion and the
        // second movement request (+0xa58 / +0xa64) carries it.
        if let Some(mut shield) = self.world.objects[SHIELD as usize].take() {
            if shield.sys.attached.is_none() {
                shield.tick(&mut self.world, &env);
                let (d, v) = (shield.sys.request_direction, shield.sys.request_speed);
                for (p, d) in shield.sys.position.iter_mut().zip(d) {
                    *p += d * v;
                }
            } else {
                shield.sys.position = self.goofy.sys.position;
                shield.sys.yaw = self.goofy.sys.yaw;
            }
            self.world.objects[SHIELD as usize] = Some(shield);
        }
        self.world.tick_attacks(&env);
        let mut effects = Vec::new();
        let mut spent_mp = false;
        let mut unsupported = Vec::new();
        for event in std::mem::take(&mut self.world.events) {
            match event {
                Event::MotionEffect { id, .. } => effects.push(id),
                Event::EffectStart { effect, .. } => {
                    if let Ok(id) = u16::try_from(effect) {
                        effects.push(id);
                    }
                }
                Event::Unsupported("obj_use_mp") => spent_mp = true,
                Event::Unsupported(name) => unsupported.push(name),
                _ => {}
            }
        }
        let shield = self.world.objects[SHIELD as usize]
            .as_ref()
            .filter(|s| s.sys.attached.is_none())
            .map(|s| Shield {
                position: s.sys.position,
                yaw: s.sys.yaw,
                motion: s.sys.motion.slot as u16,
                tick: s.sys.motion.tick,
            });
        Frame {
            motion: self.goofy.sys.motion.slot as u16,
            tick: self.goofy.sys.motion.tick,
            position: self.goofy.sys.position,
            yaw: self.goofy.sys.yaw,
            shield,
            attacks: script_attacks(&self.world),
            effects,
            spent_mp,
            unsupported,
            serial: self.goofy.sys.motion.serial,
            blend: self.goofy.sys.motion.blend,
            queue: self
                .goofy
                .sys
                .motion_queue
                .iter()
                .map(|&(slot, blend)| (slot as u16, blend))
                .collect(),
            // The act is over when its thread is; the motions it left
            // playing and queued go on in the member's own controller.
            done: self.goofy.script.act_thread.is_none(),
        }
    }

    /// The shield's attack struck the enemy: its hit callback runs (the
    /// script keeps the shield on it while the hits last).
    pub fn struck<W: World>(&mut self, room: &W, slot: usize, target: Option<Vec3>) {
        let Some(Some(attack)) = self.world.attacks.get_mut(slot) else {
            return;
        };
        attack.mark_hit(enemy::attack::flag::STRUCK);
        let Some(callback) = attack.hit_callback else {
            return;
        };
        let env = Surroundings {
            world: room,
            clips: &self.clips,
            player: self.goofy.sys.position,
            target,
            hand: Mat4::IDENTITY,
        };
        self.goofy
            .run_hit_callback(&mut self.world, &env, callback, object_handle(ENEMY));
    }
}
