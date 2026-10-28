//! Damage handling around a script object, translated from the executable
//! (Steam build 15194255): the enemy classes' hit slot (BTLOBJ vt+0xc0,
//! FUN_1403dbb90 -> FUN_140410d60), damage application with the kill veto
//! (FUN_1403d3ba0, FUN_1403d35f0), the HP change and death (FUN_1403d2eb0,
//! vt+0xb0), the `patn` reaction override (FUN_1403ee9b0) and the reaction
//! (FUN_1403d2fe0). The damage formula and reaction classes are the player
//! crate's translations; here they are applied to an [`Object`].
//!
//! [U] Element status effects, the mission hit counter, the damage voice and
//! the map scripts' damage broadcast (FUN_1403f47b0) belong to the host.

use crate::env::Env;
use crate::object::DAMAGE;
use crate::script::Slot;
use crate::sysobj::{Reaction, Vec3, class_flag, hlen, state_flag};
use crate::traps::target::PLAYER_TARGET;
use crate::world::{Event, Object, World, object_handle};
use player::{actor_physics, combat};

/// Where the damage record sits in its object's handle space.
pub const DAMAGE_RECORD_OFFSET: u32 = 0x400;

/// The damage record's handle (the argument OBJ_DAMAGE receives).
pub fn damage_handle(id: u32) -> u32 {
    object_handle(id) + DAMAGE_RECORD_OFFSET
}

/// One hit as the attack hands it to its target (the damage record
/// FUN_1403d23c0 builds).
#[derive(Clone, Copy, Debug)]
pub struct Hit<'a> {
    /// From the attack toward the target (horizontal is enough).
    pub direction: Vec3,
    /// The struck shape is a high one (the LOW flinch plays otherwise).
    pub high: bool,
    pub attack: &'a combat::AttackParams,
    pub attacker: &'a combat::AttackerStats,
    pub difficulty: combat::Difficulty,
    /// The ATKP id (`damage_attack_param_id`).
    pub param_id: u32,
    /// ATKP RevengeDamage (+0x2a).
    pub revenge: u8,
    /// The HP pool the struck shape names (+0x25, the shape's parts byte).
    pub pool: u8,
    /// The attacker is the controlled leader (+0x6c8 bit 7) or a party
    /// member (bit 14).
    pub by_leader: bool,
    pub by_party: bool,
}

/// FUN_1403ee9b0: the `patn` rows (00battle.bin; byte 0 the pattern bit,
/// byte 1 + r the replacement for original reaction r, 0xff none) whose bit
/// is in the active mask rewrite the reaction; later rows win. Bit 2 needs a
/// party-member attacker, bit 7 a grounded object and bit 8 one in the air
/// or flying. `None` keeps the reaction.
pub fn patn_override(
    patn: &[[u8; 32]],
    mask: u32,
    orig: u8,
    by_party: bool,
    airborne: bool,
    flying: bool,
) -> Option<u8> {
    let grounded = !airborne && !flying;
    let mut out = None;
    for row in patn {
        let c = u32::from(row[0]);
        if c >= 32 || mask >> c & 1 == 0 {
            continue;
        }
        if c == 2 && !by_party || c == 7 && !grounded || c == 8 && grounded {
            continue;
        }
        match row.get(1 + usize::from(orig)) {
            Some(&v) if v != 0xff => out = Some(v),
            _ => {}
        }
    }
    out
}

fn horizontal(v: Vec3) -> Vec3 {
    let l = hlen(v);
    if l > 0. {
        [v[0] / l, 0., v[2] / l]
    } else {
        [0.; 3]
    }
}

impl Object {
    /// Can this object be hit now.
    pub fn targetable(&self) -> bool {
        !self.sys.gone
            && self.sys.state_flags & state_flag::DEAD_MARKED == 0
            && self.sys.hp > 0
            && !self.acts.locked()
            && !self.sys.shadow_move
            && !self.sys.left
    }

    /// BTLOBJ vt+0xc0 of the enemy classes (FUN_1403dbb90, then
    /// FUN_140410d60): the revenge gauge takes the hit's RevengeDamage
    /// first; then, unless the object ignores damage (pattern bit 13), is
    /// dead or cannot be damaged (FUN_1403d3570), the damage is applied
    /// (FUN_1403d3ba0, which may kill), the `patn` table overrides the
    /// reaction (FUN_1403ee9b0), the script's OBJ_DAMAGE runs, and the
    /// engine's reaction plays unless the script carried one out itself
    /// (FUN_1403d2fe0). Returns the damage dealt, or `None` when the hit
    /// changed nothing but the gauge.
    pub fn take_hit(&mut self, world: &mut World, env: &dyn Env, hit: &Hit<'_>) -> Option<i32> {
        if self.sys.gone || self.sys.left {
            return None;
        }
        self.sys.revenge += f32::from(hit.revenge);
        let mask = self.sys.active_patterns();
        if mask & 1 << 13 != 0 {
            return None;
        }
        if self.sys.state_flags & state_flag::DEAD_MARKED != 0 || self.sys.invulnerable() {
            return None;
        }
        let attack = hit.attack;
        let sys = &mut self.sys;
        // The damage record: reaction +0x26 and the original +0x27, flags
        // cleared, the pool the shape names.
        sys.damage_param_id = hit.param_id;
        sys.damage_reaction = u32::from(attack.reaction);
        sys.damage_orig_reaction = u32::from(attack.reaction);
        sys.damage_element = attack.element;
        sys.damage_attack_type = attack.attack_type;
        sys.damage_parts = hit.pool;
        sys.damage_exec = false;
        sys.last_attacker = Some(PLAYER_TARGET);
        // [U] Hits reach here from Sora's side only; a party member's hit
        // would name the member.
        sys.damage_attacker = Some(PLAYER_TARGET);
        let damage = self.apply_damage(world, env, hit);
        // FUN_1403ee9b0, skipped for a ZAKO the hit killed.
        let dead = self.sys.state_flags & state_flag::DEAD_MARKED != 0;
        if !(dead && self.sys.class_flags & class_flag::ZAKO != 0) {
            let flying = self.sys.mode_flags() & 2 != 0 || self.sys.motion.flags.word & 0x40 != 0;
            if let Some(r) = patn_override(
                &world.patn,
                mask,
                self.sys.damage_orig_reaction as u8,
                hit.by_party,
                self.sys.airborne,
                flying,
            ) {
                self.sys.damage_reaction = u32::from(r);
            }
        }
        // [U] The running act's damage callback (FUN_1403ca470) is not run:
        // no act of the scripts checked registers one.
        self.fire(world, env, DAMAGE, &[Slot::address(damage_handle(self.id))]);
        if !self.sys.damage_exec {
            self.react(env, hit);
        }
        Some(damage)
    }

    /// FUN_1403d3ba0: a cure heals; otherwise the kill veto (vt+0xa8,
    /// FUN_1403d35f0) leaves the pool one above its floor, a pattern-15
    /// object takes at most 1 from a hit with a normal reaction by anyone
    /// but the leader, and the HP change (FUN_1403d2eb0) runs.
    fn apply_damage(&mut self, world: &mut World, env: &dyn Env, hit: &Hit<'_>) -> i32 {
        let pool = usize::from(hit.pool);
        let Some(p) = self.sys.pool(pool) else {
            return 0;
        };
        let state = combat::TargetState {
            hp: p.hp,
            difficulty: hit.difficulty,
            ..combat::TargetState::full(self.sys.stats.max_hp)
        };
        let mut d = combat::damage_with(hit.attacker, hit.attack, &self.sys.stats, &state);
        if matches!(hit.attack.attack_type, 5 | 6) {
            self.change_hp(world, env, d, pool);
            return d;
        }
        if p.hp - d <= p.min && self.kill_veto(hit) {
            d = (p.hp - p.min - 1).max(0);
        }
        let normal = matches!(hit.attack.reaction, 0 | 1 | 3 | 4 | 6 | 7 | 15 | 16);
        if d > 1 && self.sys.active_patterns() & 1 << 15 != 0 && normal && !hit.by_leader {
            d = 1;
        }
        self.change_hp(world, env, -d, pool);
        d
    }

    /// FUN_1403d35f0 (BTLOBJ vt+0xa8): whether the hit may not bring the
    /// pool to its floor. ATKP Flags bit 5 is the "can kill" bit
    /// (FUN_1403cede0).
    fn kill_veto(&self, hit: &Hit<'_>) -> bool {
        let m = self.sys.active_patterns();
        let can_kill = hit.attack.flags & 0x20 != 0;
        if m & 1 << 9 != 0 {
            return true;
        }
        if m & 1 << 10 != 0 && (m & 1 << 19 != 0 || hit.pool == 0) && !can_kill {
            return true;
        }
        // The last term (+0x6c8 bit 7 with +0x18c bit 14) is the leader's
        // own; an enemy has neither.
        (m & 1 << 11 != 0 || hit.pool != 0) && !can_kill && !hit.by_leader
    }

    /// FUN_1403d2eb0 (BTLOBJ vt+0xe8): the pool moves by `delta` within its
    /// floor and maximum; pool 0 reaching 0 is the object's death (vt+0xb0).
    pub fn change_hp(&mut self, world: &mut World, env: &dyn Env, delta: i32, pool: usize) {
        if self.move_hp(delta, pool) {
            self.die(world, Some(env));
        }
    }

    /// [`Object::change_hp`] from inside this object's own script (a trap):
    /// the death's act callbacks are queued for the interpreter.
    pub fn change_hp_queued(&mut self, world: &mut World, _env: &dyn Env, delta: i32, pool: usize) {
        if self.move_hp(delta, pool) {
            self.die(world, None);
        }
    }

    /// The pool moves within its floor and maximum; true when pool 0 has
    /// just reached 0 (the death).
    fn move_hp(&mut self, delta: i32, pool: usize) -> bool {
        let Some(p) = self.sys.pool(pool) else {
            return false;
        };
        if p.max == 0 || self.sys.state_flags & state_flag::DEAD_MARKED != 0 {
            return false;
        }
        self.sys.set_pool_hp(pool, p.hp.saturating_add(delta));
        pool == 0 && self.sys.hp == 0
    }

    /// BTLOBJ vt+0xb0: the enemy death FUN_1403dbaf0 -> FUN_140410af0 (the
    /// act `dead` starts by name when the table has it, then the dead mark),
    /// and for the boss class FUN_1403f8c60: element 0 of its own effects
    /// starts on it and the event signal 4 is posted with its entry id
    /// (FUN_1403aabe0), which the mission script turns into the clear. An
    /// ordinary enemy (YS::ENEMY, objentry types 4 and 21) posts event signal
    /// 0x4b with its spawn record's serial first (FUN_140419d10, its
    /// vt+0xb0, through FUN_1403b5bf0). [U] FUN_140419d10 also plays motion
    /// 16 when the handle at +0xc names a type-0x17 record, which is not
    /// identified. EXP, the bonus and the prize of the ENMP row are the
    /// host's.
    pub fn die(&mut self, world: &mut World, env: Option<&dyn Env>) {
        if self.sys.class_flags & class_flag::ZAKO != 0 {
            world.events.push(Event::Signal {
                code: 0x4b,
                argument: i32::from(self.sys.spawn_serial),
            });
        }
        if self.acts.find(b"dead").is_some() {
            let actions = self.acts.start(b"dead");
            match env {
                Some(env) => self.apply_actions(Some((world, env)), actions),
                None => self.apply_actions(None, actions),
            }
        }
        self.sys.state_flags |= state_flag::DEAD_MARKED;
        if self.sys.class_flags & class_flag::BOSS != 0 {
            let handle = world.next_effect_handle();
            world.events.push(Event::EffectStart {
                object: self.id,
                effect: 0,
                arguments: [0; 2],
                handle,
                position: None,
                bound: true,
                other: None,
            });
            world.events.push(Event::Signal {
                code: 4,
                argument: self.sys.entry_id as i32,
            });
        }
    }

    /// FUN_1403d2fe0: the engine's reaction to the record's (overridden)
    /// reaction, by class: 1 a knock slide, 2 the flinch motion, 3 both, 4
    /// the blow. The knock strength reads the original reaction
    /// (FUN_1403ceab0), as do the blow's KB1/KB2.
    fn react(&mut self, env: &dyn Env, hit: &Hit<'_>) {
        let sys = &mut self.sys;
        if sys.stop_requests.iter().any(|r| r & 2 != 0) {
            return;
        }
        let reaction = sys.damage_reaction as u8;
        let orig = sys.damage_orig_reaction as u8;
        let direction = hit.direction;
        let class = combat::reaction_class(reaction);
        let back = sys.yaw.sin() * direction[0] + sys.yaw.cos() * direction[2] >= 0.;
        let clips = env.clips(sys.model);
        let kb = sys.sstm.knock_strength(
            hit.attack.knockback[0],
            combat::reaction_class(orig),
            reaction,
        );
        let flinch = 10 + (back as usize) * 2 + (!hit.high) as usize;
        match class {
            1 => sys.knock = actor_physics::Knock::new(horizontal(direction), kb, 0.),
            2 => {
                sys.reaction = Reaction::Flinch;
                sys.motion_start(flinch, 0., clips);
            }
            3 => {
                sys.reaction = Reaction::Flinch;
                sys.motion_start(flinch, 0., clips);
                sys.knock = actor_physics::Knock::new(horizontal(direction), kb, 0.);
            }
            4 => {
                let [mut up, mut speed] = [0., 0.];
                if combat::reaction_class(orig) == 4 {
                    [up, speed] = hit.attack.knockback.map(f32::from);
                }
                if up == 0. && speed == 0. {
                    up = sys.sstm.blow_up;
                    speed = sys.sstm.blow_speed;
                }
                let d = horizontal(direction);
                sys.blow = Some(actor_physics::Blow::new(
                    [d[0] * speed, up, d[2] * speed],
                    &sys.sstm,
                ));
                sys.knock = None;
                sys.jump = None;
                sys.mode7 = false;
                sys.hop = None;
                sys.steer = None;
                sys.fly = None;
                sys.pending_fly = None;
                sys.sampler.reset();
                sys.airborne = true;
                sys.reaction = Reaction::Blow { back };
                sys.motion_start(if back { 18 } else { 16 }, 0., clips);
            }
            // Class 5 only captures (the host calls `capture`); 0 and 6..13
            // do nothing. [U] Class 14's special motions are not played.
            _ => {}
        }
        sys.damage_exec = true;
    }

    /// A guarded attack bounced off (FUN_140411060 on the ground): play the
    /// attack's reflect motion, slide back, clear the acts and start the
    /// revenge act.
    pub fn reflect(
        &mut self,
        world: &mut World,
        env: &dyn Env,
        reflect_motion: usize,
        hit_back: f32,
    ) {
        if !self.targetable() {
            return;
        }
        let sys = &mut self.sys;
        sys.motion_start(reflect_motion, 0., env.clips(sys.model));
        sys.reaction = Reaction::Flinch;
        let strength = if hit_back < 0. { 50. } else { hit_back };
        sys.knock = if strength > 0. {
            actor_physics::Knock::new([-sys.yaw.sin(), 0., -sys.yaw.cos()], strength, 0.)
        } else {
            None
        };
        let clear = self.acts.clear();
        self.apply_actions(Some((world, env)), clear);
        let start = self.acts.start(b"mode_revenge");
        self.apply_actions(Some((world, env)), start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::NullEnv;

    fn boss(hp: i32) -> (World, Object) {
        let mut o = Object::bare(1);
        o.sys.set_class(3);
        o.sys.entry_id = 2230;
        o.sys.stats = combat::TargetStats {
            max_hp: 3000,
            defense: 0,
            cap: 3000,
            min: 0,
            weakness: [100; 7],
        };
        o.sys.hp = hp;
        (World::default(), o)
    }

    fn hit<'a>(
        attack: &'a combat::AttackParams,
        attacker: &'a combat::AttackerStats,
        by_leader: bool,
    ) -> Hit<'a> {
        Hit {
            direction: [0., 0., 1.],
            high: true,
            attack,
            attacker,
            difficulty: combat::Difficulty::Standard,
            param_id: 7,
            revenge: 8,
            pool: 0,
            by_leader,
            by_party: !by_leader,
        }
    }

    fn strong(flags: u8, reaction: u8) -> combat::AttackParams {
        combat::AttackParams {
            power: 100,
            stat: 0,
            element: 0,
            reaction,
            knockback: [0x7fff, 0],
            attack_type: 0,
            combo_kind: 0,
            flags,
        }
    }

    const SORA: combat::AttackerStats = combat::AttackerStats {
        strength: 500,
        magic: 0,
        max: 99999,
        min: 1,
        mods: combat::AttackerMods {
            player_type: true,
            mp_charging: false,
            berserk_strength: 0,
            element_percent: [100; 3],
            combo_step: 0,
            ground_limit: 0,
            air_limit: 0,
            combo_boost: 0,
            air_combo_boost: 0,
            reaction_boost: 0,
            combo_boost_enabled: false,
            air_combo_boost_enabled: false,
            reaction_boost_enabled: false,
        },
    };

    #[test]
    fn obj_damage_reads_the_attacker_from_the_damage_record() {
        // M_EX350_10 / B_EX380: OBJ_DAMAGE takes the record and reads its
        // +0x10 (`push.d.sp.d 16`), the attacker's handle (FUN_1403ce740).
        const START: u32 = 18;
        let mut b = vec![0u8; 16];
        b[..4].copy_from_slice(b"test");
        for v in [64u32, 64, 64] {
            b.extend(v.to_le_bytes());
        }
        for v in [0u32, START, DAMAGE as u32, START + 1, 0, 0] {
            b.extend(v.to_le_bytes());
        }
        // OBJ_INIT: exit; OBJ_DAMAGE: pop.sp 0, push.d.sp.d 16, pop.wp W0,
        // push.d.sp.d 20, pop.wp W1, exit.
        for w in [
            0x0049u16, 0x0001, 0, 0x00b0, 16, 0x0041, 0, 0x00b0, 20, 0x0041, 4, 0x0049,
        ] {
            b.extend(w.to_le_bytes());
        }
        let (mut world, template) = boss(100);
        let mut o = Object::spawn(&mut world, &NullEnv, 0, &b, template.sys).unwrap();
        o.take_hit(&mut world, &NullEnv, &hit(&strong(0, 3), &SORA, true));
        let word = |i: usize| {
            u32::from_le_bytes(o.script.instance.work[i * 4..i * 4 + 4].try_into().unwrap())
        };
        assert_eq!(word(0), crate::world::PLAYER_HANDLE);
        assert_eq!(word(1), object_handle(o.id));
    }
    #[test]
    fn only_a_can_kill_attack_takes_a_bosss_last_hp() {
        let (mut world, mut o) = boss(10);
        let plain = strong(0, 3);
        o.take_hit(&mut world, &NullEnv, &hit(&plain, &SORA, true));
        assert_eq!(o.sys.hp, 1, "the veto leaves floor + 1");
        assert_eq!(o.sys.revenge, 8.);
        let finisher = strong(0x20, 3);
        o.take_hit(&mut world, &NullEnv, &hit(&finisher, &SORA, true));
        assert_eq!(o.sys.hp, 0);
        assert!(o.sys.state_flags & state_flag::DEAD_MARKED != 0);
        assert!(world.events.contains(&Event::Signal {
            code: 4,
            argument: 2230
        }));
        // Dead: further hits only feed the gauge.
        assert_eq!(
            o.take_hit(&mut world, &NullEnv, &hit(&finisher, &SORA, true)),
            None
        );
    }

    #[test]
    fn an_ordinary_enemys_death_posts_its_spawn_serial() {
        // FUN_140419d10: signal 0x4b with the spawn record's serial; the
        // boss's signal 4 is not posted.
        let (mut world, mut o) = boss(10);
        o.sys.class_flags = 0;
        o.sys.set_class(4);
        o.sys.spawn_serial = 7;
        o.take_hit(&mut world, &NullEnv, &hit(&strong(0x20, 3), &SORA, true));
        assert_eq!(o.sys.hp, 0);
        assert!(world.events.contains(&Event::Signal {
            code: 0x4b,
            argument: 7
        }));
        assert!(
            !world
                .events
                .iter()
                .any(|e| matches!(e, Event::Signal { code: 4, .. }))
        );
    }

    #[test]
    fn a_party_members_normal_hit_does_one_damage_to_a_boss() {
        let (mut world, mut o) = boss(3000);
        let a = strong(0, 3);
        assert_eq!(
            o.take_hit(&mut world, &NullEnv, &hit(&a, &SORA, false)),
            Some(1)
        );
        assert_eq!(o.sys.hp, 2999);
        // A special reaction (2) is not one of the normal ones.
        let b = strong(0, 2);
        assert!(
            o.take_hit(&mut world, &NullEnv, &hit(&b, &SORA, false))
                .unwrap()
                > 1
        );
    }

    #[test]
    fn patn_rows_rewrite_by_the_active_bits_and_the_last_row_wins() {
        let mut row2 = [0xffu8; 32];
        row2[0] = 2;
        row2[1 + 3] = 0;
        let mut row14 = [0xffu8; 32];
        row14[0] = 14;
        row14[1 + 5] = 3;
        let patn = [row2, row14];
        // Bit 2 only for party attackers.
        assert_eq!(patn_override(&patn, 1 << 2, 3, true, false, false), Some(0));
        assert_eq!(patn_override(&patn, 1 << 2, 3, false, false, false), None);
        assert_eq!(
            patn_override(&patn, 1 << 14, 5, false, false, false),
            Some(3)
        );
        assert_eq!(patn_override(&patn, 0, 5, false, false, false), None);
    }

    #[test]
    fn the_revenge_gauge_drains_while_the_act_manager_is_active() {
        let (_, mut o) = boss(3000);
        o.sys.revenge = 5.;
        o.decay_revenge(1.);
        assert_eq!(o.sys.revenge, 5., "no act table: no decay");
        o.acts.init_table();
        o.decay_revenge(1.);
        assert_eq!(o.sys.revenge, 4.);
        o.decay_revenge(10.);
        assert_eq!(o.sys.revenge, 0.);
    }
}
