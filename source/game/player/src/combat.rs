//! Melee hits and an enemy's hit reactions, translated from the PC
//! executable: hit-shape tests FUN_1403c94c0, swept attack spheres in
//! FUN_14040f610, damage FUN_1403c12f0, reaction classes FUN_1403ce980,
//! and the reaction classes used by the enemy (see `shadow`).
//!
//! Positions here are in the simulation's Y-up space.

pub type Vec3 = [f32; 3];

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Strict sphere overlap: |c1 - c2|^2 < (r1 + r2)^2.
pub fn spheres_overlap(a: Vec3, ra: f32, b: Vec3, rb: f32) -> bool {
    let d = sub(a, b);
    dot(d, d) < (ra + rb) * (ra + rb)
}

/// Attack sphere centres for one frame, with extra copies filling the gap
/// back toward last frame's centre whenever it is farther than 1.5 radii
/// (copies every radius, 0x1405b22e0 = 1.5).
pub fn swept(current: Vec3, previous: Option<Vec3>, radius: f32) -> Vec<Vec3> {
    let mut out = vec![current];
    let Some(prev) = previous else { return out };
    let delta = sub(prev, current);
    let mut dist = dot(delta, delta).sqrt();
    if dist <= 0. || radius <= 0. {
        return out;
    }
    let dir = delta.map(|c| c / dist);
    let mut at = current;
    while dist > 1.5 * radius {
        at = [0, 1, 2].map(|k| at[k] + dir[k] * radius);
        out.push(at);
        dist -= radius;
    }
    out
}

/// An ATTACK_MAGIC volume (sphere when radius equals height, else an
/// ellipsoid, FUN_1404376b0) against a sphere (FUN_1403c94c0 /
/// FUN_1403c9890): strict, with the ellipsoid's horizontal and vertical
/// extents each widened by the sphere's radius.
pub fn volume_hits_sphere(c: Vec3, r: f32, h: f32, b: Vec3, rb: f32) -> bool {
    if r == h {
        return spheres_overlap(c, r, b, rb);
    }
    let d = sub(b, c);
    let (hr, vr) = (r + rb, h + rb);
    (d[0] * d[0] + d[2] * d[2]) / (hr * hr) + d[1] * d[1] / (vr * vr) < 1.
}

/// A volume's centres this frame: the current one, plus copies stepping back
/// toward last frame's centre by min(r, h) while more than half that
/// remains (FUN_1404376b0).
pub fn swept_volume(current: Vec3, previous: Option<Vec3>, r: f32, h: f32) -> Vec<Vec3> {
    let mut out = vec![current];
    let Some(prev) = previous else { return out };
    let step = r.min(h);
    let delta = sub(prev, current);
    let mut dist = dot(delta, delta).sqrt();
    if dist <= 0. || step <= 0. {
        return out;
    }
    let dir = delta.map(|c| c / dist);
    let mut at = current;
    while dist > 0.5 * step {
        at = [0, 1, 2].map(|k| at[k] + dir[k] * step);
        out.push(at);
        dist -= step;
    }
    out
}

/// Attack parameters used by the damage routine (from ATKP).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackParams {
    pub power: u16,
    /// 0 strength, 1 magic, else no stat.
    pub stat: u8,
    pub element: u8,
    pub reaction: u8,
    pub knockback: [i16; 2],
    /// ATKP Type (+4): 5 and 6 are cures.
    pub attack_type: u8,
    /// ATKP Kind (+0x2e): 1 ground finisher, 2 air finisher, 4 reaction
    /// command; selects the combo-boost terms.
    pub combo_kind: u8,
    /// ATKP Flags (+0x12).
    pub flags: u8,
}

/// Ability terms of the damage routine on the attacker's side
/// (FUN_1403c12f0; the ability block is documented in `abilities`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackerMods {
    /// The attacker is a player-type object (obj+0x6c8 bit 2).
    pub player_type: bool,
    /// MP is charging (stats +0x1c0 > 0).
    pub mp_charging: bool,
    /// Berserk Charge: added to Strength while MP charges (+0x226).
    pub berserk_strength: i32,
    /// Fire, Blizzard and Thunder damage percent (+0x218..), 100 default.
    pub element_percent: [i32; 3],
    /// The leader's combo step (obj+0xd74) and the limits it is capped by
    /// (stats +0x200 / +0x201).
    pub combo_step: i32,
    pub ground_limit: i32,
    pub air_limit: i32,
    /// Combo Boost, Air Combo Boost and Reaction Boost percents
    /// (+0x21e / +0x220 / +0x222).
    pub combo_boost: i32,
    pub air_combo_boost: i32,
    pub reaction_boost: i32,
    /// Ability bits 102, 103 and 104 gate their terms independently of
    /// the accumulated percent (including a percent that wrapped to zero).
    pub combo_boost_enabled: bool,
    pub air_combo_boost_enabled: bool,
    pub reaction_boost_enabled: bool,
}

impl Default for AttackerMods {
    fn default() -> Self {
        AttackerMods {
            player_type: false,
            mp_charging: false,
            berserk_strength: 0,
            element_percent: [100; 3],
            combo_step: 0,
            ground_limit: 3,
            air_limit: 3,
            combo_boost: 0,
            air_combo_boost: 0,
            reaction_boost: 0,
            combo_boost_enabled: false,
            air_combo_boost_enabled: false,
            reaction_boost_enabled: false,
        }
    }
}

/// Attacker stats (player: LVUP + PLRP + equipment).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackerStats {
    pub strength: i32,
    pub magic: i32,
    /// Clamp defaults for a player stat struct: 99999 / 1.
    pub max: i32,
    pub min: i32,
    pub mods: AttackerMods,
}

/// The difficulty byte `DAT_1409abd48` of FUN_1403c12f0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Difficulty {
    Beginner = 0,
    #[default]
    Standard = 1,
    Proud = 2,
    Critical = 3,
}

impl Difficulty {
    /// Percent applied to damage taken by a player-type target.
    fn taken(self) -> i32 {
        [50, 100, 200, 200][self as usize]
    }
    /// Percent applied to damage dealt by a player-type attacker.
    fn dealt(self) -> i32 {
        [150, 100, 100, 125][self as usize]
    }
}

/// What the target contributes beyond its [`TargetStats`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetState {
    /// Current HP of the hit pool (pool 0 for the low-HP terms).
    pub hp: i32,
    /// The target is a player-type object.
    pub player_type: bool,
    /// Defender: Defence taken off the attack below a quarter of HP
    /// (+0x204 byte).
    pub defender: i32,
    /// Damage Control: the HP fraction at or below which the hit is
    /// reduced (+0x248, 0 = never) and SSTM DefenseMasterRatio (50).
    pub damage_control: f32,
    pub damage_control_ratio: i32,
    pub difficulty: Difficulty,
    /// The target is the lead player (obj+0x6c8 bit 7) in Antiform
    /// (obj+0xde0 = 6): the damage is × SSTM +0x94.
    pub antiform: bool,
}

impl TargetState {
    /// A full-health target with no abilities, Standard difficulty.
    pub fn full(max_hp: i32) -> Self {
        TargetState {
            hp: max_hp,
            player_type: false,
            defender: 0,
            damage_control: 0.,
            damage_control_ratio: 50,
            difficulty: Difficulty::Standard,
            antiform: false,
        }
    }
}

/// Target stats (enemy: ENMP x LVPM).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetStats {
    pub max_hp: i32,
    pub defense: i32,
    /// (enmp cap x maxHP + 9999) / 10000 and the same for the minimum.
    pub cap: i32,
    pub min: i32,
    /// Weakness percents: physical..light, then general.
    pub weakness: [i32; 7],
}

impl TargetStats {
    /// Enemy stats: HP pool = (hp% x lvpm HP + 99) / 100; cap/minimum are
    /// per 10000 of max HP (FUN_1403c2240).
    pub fn enemy(
        hp_percent: u16,
        damage_cap: u16,
        damage_min: u16,
        weakness: [u16; 7],
        level_hp: u16,
        level_defense: u16,
    ) -> Self {
        let max_hp = (hp_percent as i32 * level_hp as i32 + 99) / 100;
        TargetStats {
            max_hp,
            defense: level_defense as i32,
            cap: (damage_cap as i32 * max_hp + 9999) / 10000,
            min: (damage_min as i32 * max_hp + 9999) / 10000,
            weakness: weakness.map(|w| w as i32),
        }
    }
}

/// Damage of a normal attack on a full-health target at Standard
/// difficulty (see [`damage_with`] for the ability and state terms).
pub fn damage(attacker: &AttackerStats, attack: &AttackParams, target: &TargetStats) -> i32 {
    damage_with(attacker, attack, target, &TargetState::full(target.max_hp))
}

/// FUN_1403c12f0 with the ability, low-HP, difficulty and Antiform terms.
/// The special battle mode (`DAT_142a11400` bit 17) is not modelled.
pub fn damage_with(
    attacker: &AttackerStats,
    attack: &AttackParams,
    target: &TargetStats,
    state: &TargetState,
) -> i32 {
    let m = &attacker.mods;
    let mut s = match attack.stat {
        0 => attacker.strength,
        1 => attacker.magic,
        _ => 0,
    };
    // Berserk Charge: strength attacks of a player-type owner while MP
    // charges (the +0x226 term).
    if attack.stat == 0 && m.player_type && m.mp_charging {
        s += m.berserk_strength;
    }
    match attack.attack_type {
        // Cure: a fraction of the target's max HP, or of the stat alone.
        5 => return (target.max_hp * attack.power as i32 * s + 9999) / 10000,
        6 => return (attack.power as i32 * s + 99) / 100,
        _ => {}
    }
    s -= target.defense;
    // Defender: below a quarter of HP (signed /4).
    if state.hp <= target.max_hp / 4 {
        s -= state.defender;
    }
    s = s.min(attacker.max.min(target.cap));
    s = s.max(attacker.min.max(target.min));
    let weak = target.weakness[(attack.element as usize).min(5)] as i64;
    let general = target.weakness[6] as i64;
    let mut dmg = ((weak * general * attack.power as i64 * s as i64 + 999_999) / 1_000_000) as i32;
    if state.player_type {
        dmg = (dmg * state.difficulty.taken() + 99) / 100;
    } else if m.player_type {
        dmg = (dmg * state.difficulty.dealt() + 99) / 100;
    }
    // 0x1403c15e0: the lead player in Antiform takes × AntiFormDamageRate,
    // truncated.
    if state.antiform {
        dmg = (dmg as f32 * crate::drive::ANTI_DAMAGE_RATE) as i32;
    }
    if (1..=3).contains(&attack.element) {
        dmg = (m.element_percent[attack.element as usize - 1] * dmg + 99) / 100;
    }
    // Combo Boost / Air Combo Boost / Reaction Boost by the attack's Kind.
    let k = attack.combo_kind;
    if m.combo_boost_enabled && k & 1 != 0 {
        let n = m.combo_step.min(m.ground_limit);
        dmg = dmg.wrapping_add(boost_damage(n, m.combo_boost, dmg));
    } else if m.air_combo_boost_enabled && k & 2 != 0 {
        let n = m.combo_step.min(m.air_limit);
        dmg = dmg.wrapping_add(boost_damage(n, m.air_combo_boost, dmg));
    } else if m.reaction_boost_enabled && k & 4 != 0 {
        dmg = dmg.wrapping_add(boost_damage(1, m.reaction_boost, dmg));
    }
    // Damage Control: at or below its HP fraction the hit is reduced.
    if state.hp <= (target.max_hp as f32 * state.damage_control) as i32 {
        dmg = (state.damage_control_ratio * dmg + 99) / 100;
    }
    dmg
}

/// FUN_1403c12f0 multiplies in 32 bits, then divides the signed result.
fn boost_damage(count: i32, percent: i32, damage: i32) -> i32 {
    count
        .wrapping_mul(percent)
        .wrapping_mul(damage)
        .wrapping_add(99)
        / 100
}

/// FUN_1403d3ba0 for a damaging hit: the HP left after `d`, honouring the
/// kill veto (a vetoed lethal hit leaves exactly `floor + 1`).
pub fn apply_damage(cur: i32, floor: i32, max: i32, d: i32, veto: bool) -> i32 {
    let mut d = d;
    if cur - d <= floor && veto {
        d = (cur - floor - 1).max(0);
    }
    (cur - d).clamp(floor, max)
}

/// Sora's kill veto (FUN_1403d6670): Second Chance (ability 127) leaves 1 HP
/// from anything above 1; Once More (ability 128) holds at 1 HP while a
/// damage motion (10-39) plays.
pub fn sora_veto(
    second_chance: bool,
    once_more: bool,
    cur: i32,
    floor: i32,
    in_damage_motion: bool,
) -> bool {
    (second_chance && floor == 0 && cur > 1) || (once_more && in_damage_motion)
}

/// A per-object record of the attacks that hit it (FUN_1403cf600,
/// FUN_1403cf4b0): the 256-bit hit-once mask by attack id, the combo-group
/// counter selecting ATKP level rows, the 30-tick group lock of
/// player-type objects, and four interval re-hit slots.
#[derive(Clone, Debug, PartialEq)]
pub struct HitRecord {
    mask: [u64; 4],
    last_group: i32,
    count: u32,
    lock: f32,
    slots: [(u8, f32); 4],
    player_type: bool,
}

/// `0x41f00000`: the group lock of a player-type object.
pub const GROUP_LOCK: f32 = 30.;
/// An attack group that is never locked out (`0xff`).
pub const ANY_GROUP: i32 = 0xff;

impl HitRecord {
    pub fn new(player_type: bool) -> Self {
        HitRecord {
            mask: [0; 4],
            last_group: 0,
            count: 0,
            lock: 0.,
            slots: [(0, 0.); 4],
            player_type,
        }
    }

    /// Whether attack `id` is already in the mask.
    pub fn has(&self, id: u8) -> bool {
        self.mask[id as usize >> 6] >> (id & 63) & 1 != 0
    }

    fn set(&mut self, id: u8, on: bool) {
        let bit = 1u64 << (id & 63);
        if on {
            self.mask[id as usize >> 6] |= bit;
        } else {
            self.mask[id as usize >> 6] &= !bit;
        }
    }

    /// Records a clash or other contact without a hit (FUN_1403d2140).
    pub fn mark(&mut self, id: u8) {
        self.set(id, true);
    }
    /// Owned attack destruction clears only this bit. Native interval slots,
    /// consecutive-hit count and the player group lock survive destruction.
    pub fn clear_seen(&mut self, id: u8) {
        self.set(id, false);
    }

    /// FUN_1403cf600: records attack `id` of combo `group`. Returns whether
    /// it counts as a new hit and the ATKP row index `pick` chose for the
    /// updated consecutive-hit count (`pick` also returns the row's
    /// Interval, which queues a re-hit slot). The id is in the mask
    /// afterwards even when the hit did not count.
    pub fn hit(
        &mut self,
        id: u8,
        group: i32,
        cure: bool,
        pick: impl FnOnce(u32) -> (usize, u8),
    ) -> (bool, usize) {
        let new = cure
            || (!self.has(id)
                && (group == self.last_group || group == ANY_GROUP || self.lock <= 0.));
        if new {
            if group == self.last_group {
                self.count += 1;
            } else {
                self.last_group = group;
                self.count = 0;
            }
            if self.player_type {
                self.lock = GROUP_LOCK;
            }
        }
        let (row, interval) = pick(self.count);
        if interval != 0
            && let Some(slot) = self.slots.iter_mut().find(|s| s.0 == 0)
        {
            *slot = (id, interval as f32);
        }
        self.set(id, true);
        (new, row)
    }

    /// FUN_1403cf4b0, once per tick: the lock runs unless the owner is in a
    /// damage motion; interval slots re-arm their attack when they expire.
    pub fn tick(&mut self, dt: f32, in_damage_motion: bool) {
        if self.lock > 0. && !in_damage_motion {
            self.lock -= dt;
        }
        for k in 0..4 {
            let (id, t) = self.slots[k];
            if id != 0 {
                let t = t - dt;
                if t <= 0. {
                    self.set(id, false);
                    self.slots[k] = (0, 0.);
                } else {
                    self.slots[k].1 = t;
                }
            }
        }
    }
}

/// The global owned-attack pool. Exhaustion returns native ID zero; IDs
/// 1..=255 stay reserved until their owning attack is destroyed.
#[derive(Clone, Copy, Debug, Default)]
pub struct AttackIds {
    counter: u8,
    live: [u64; 4],
}

impl AttackIds {
    pub fn allocate(&mut self) -> u8 {
        for _ in 0..256 {
            self.counter = ((self.counter as u16 + 1) % 255) as u8;
            let id = self.counter + 1;
            let index = id as usize >> 6;
            let bit = 1u64 << (id & 63);
            if self.live[index] & bit == 0 {
                self.live[index] |= bit;
                return id;
            }
        }
        0
    }
    pub fn release(&mut self, id: u8) {
        self.live[id as usize >> 6] &= !(1u64 << (id & 63));
    }
    pub fn live_ids(&self) -> impl Iterator<Item = u8> + '_ {
        (1u16..=255)
            .map(|id| id as u8)
            .filter(|&id| self.live[id as usize >> 6] >> (id & 63) & 1 != 0)
    }
}

pub fn allocate_group(counter: &mut i32) -> i32 {
    let next = counter.wrapping_add(1);
    *counter = if next > 0x1000_0000 { 0x100 } else { next };
    *counter
}

/// Reaction class (FUN_1403ce980): 3 flinch, 4 blow.
pub fn reaction_class(reaction: u8) -> u8 {
    match reaction {
        1 | 12 => 1,
        3 | 6 | 8 | 10 | 13 | 16 | 18 => 3,
        4 | 7 | 9 | 11 | 15 | 17 => 4,
        r => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_attack_ids_skip_live_entries_exhaust_and_reuse_only_released_ids() {
        let mut ids = AttackIds::default();
        let allocated = (0..255).map(|_| ids.allocate()).collect::<Vec<_>>();
        assert_eq!(&allocated[..4], &[2, 3, 4, 5]);
        assert_eq!(allocated[254], 1);
        assert_eq!(ids.live_ids().count(), 255);
        assert_eq!(ids.allocate(), 0);
        ids.release(100);
        assert_eq!(ids.allocate(), 100);
        assert_eq!(ids.allocate(), 0);
        for id in allocated {
            ids.release(id);
        }
        assert_eq!(ids.live_ids().count(), 0);
        let mut group = 0x1000_0000;
        assert_eq!(allocate_group(&mut group), 0x100);
    }

    #[test]
    fn attack_destruction_preserves_interval_slots_and_combo_lock() {
        let mut record = HitRecord::new(true);
        assert!(record.hit(12, 0x104, false, |_| (0, 5)).0);
        let slots = record.slots;
        let group = (record.last_group, record.count, record.lock);
        record.clear_seen(12);
        assert!(!record.has(12));
        assert_eq!(record.slots, slots);
        assert_eq!((record.last_group, record.count, record.lock), group);
        assert!(!record.hit(13, 0x105, false, |_| (0, 0)).0);
    }

    fn shadow(level_hp: u16, level_def: u16) -> TargetStats {
        TargetStats::enemy(50, 10000, 1000, [100; 7], level_hp, level_def)
    }

    #[test]
    fn damage_matches_the_traced_level_one_numbers() {
        let sora = AttackerStats {
            strength: 5,
            magic: 7,
            max: 99999,
            min: 1,
            ..AttackerStats::default()
        };
        let t = shadow(20, 2);
        assert_eq!((t.max_hp, t.cap, t.min), (10, 10, 1));
        let hit = |power| AttackParams {
            power,
            stat: 0,
            element: 0,
            reaction: 3,
            knockback: [125, 0],
            ..AttackParams::default()
        };
        assert_eq!(damage(&sora, &hit(100), &t), 3);
        assert_eq!(damage(&sora, &hit(65), &t), 2);
        assert_eq!(damage(&sora, &hit(200), &t), 6);
        // Level 3: HP 12, defence 3; the 10% minimum floor gives 2.
        let t3 = shadow(23, 3);
        assert_eq!(t3.max_hp, 12);
        assert_eq!(damage(&sora, &hit(100), &t3), 2);
    }

    fn sora_mods() -> AttackerStats {
        AttackerStats {
            strength: 5,
            magic: 7,
            max: 99999,
            min: 1,
            mods: AttackerMods {
                player_type: true,
                ..AttackerMods::default()
            },
        }
    }

    fn dummy() -> TargetStats {
        TargetStats {
            max_hp: 100,
            defense: 2,
            cap: 99999,
            min: 1,
            weakness: [100; 7],
        }
    }

    #[test]
    fn ability_terms_follow_the_damage_routine() {
        let target = dummy();
        let finisher = AttackParams {
            power: 200,
            combo_kind: 1,
            ..AttackParams::default()
        };
        let base = damage(&sora_mods(), &finisher, &target);
        assert_eq!(base, 6);
        // Combo Boost: step 3 of limit 3 at 10% per step: +30%.
        let mut boosted = sora_mods();
        boosted.mods.combo_boost = 10;
        boosted.mods.combo_boost_enabled = true;
        boosted.mods.combo_step = 3;
        let expect = 6 + (3 * 10 * 6 + 99) / 100;
        assert_eq!(damage(&boosted, &finisher, &target), expect);
        // Beyond the limit the step is capped.
        boosted.mods.combo_step = 9;
        assert_eq!(damage(&boosted, &finisher, &target), expect);
        // A normal swing (Kind 0) takes no boost.
        let swing = AttackParams {
            power: 100,
            ..AttackParams::default()
        };
        assert_eq!(
            damage(&boosted, &swing, &target),
            damage(&sora_mods(), &swing, &target)
        );
        // Berserk Charge: +2 Strength only while MP charges.
        let mut b = sora_mods();
        b.mods.berserk_strength = 2;
        assert_eq!(damage(&b, &swing, &target), 3);
        b.mods.mp_charging = true;
        assert_eq!(damage(&b, &swing, &target), 5);
        // Fire Boost: +20% on element 1.
        let fire = AttackParams {
            power: 100,
            stat: 1,
            element: 1,
            ..AttackParams::default()
        };
        let mut f = sora_mods();
        assert_eq!(damage(&f, &fire, &target), 5);
        f.mods.element_percent[0] = 120;
        assert_eq!(damage(&f, &fire, &target), 6);
    }

    #[test]
    fn boost_bits_control_branch_priority_even_with_a_zero_rate() {
        let target = dummy();
        let mixed = AttackParams {
            power: 200,
            combo_kind: 7,
            ..AttackParams::default()
        };
        let mut a = sora_mods();
        a.mods.combo_step = 3;
        a.mods.combo_boost = 10;
        a.mods.air_combo_boost = 20;
        a.mods.reaction_boost = 50;
        // Rates alone do not confer abilities.
        assert_eq!(damage(&a, &mixed, &target), 6);
        a.mods.reaction_boost_enabled = true;
        assert_eq!(damage(&a, &mixed, &target), 9);
        a.mods.air_combo_boost_enabled = true;
        assert_eq!(damage(&a, &mixed, &target), 10);
        a.mods.combo_boost_enabled = true;
        assert_eq!(damage(&a, &mixed, &target), 8);
        // A zero ground percent still selects that branch, suppressing
        // the air and reaction branches exactly like the native bit gate.
        a.mods.combo_boost = 0;
        assert_eq!(damage(&a, &mixed, &target), 6);
    }

    #[test]
    fn boost_products_wrap_before_signed_division() {
        let target = dummy();
        let finisher = AttackParams {
            power: 200,
            combo_kind: 1,
            ..AttackParams::default()
        };
        let mut a = sora_mods();
        a.strength = 502;
        a.mods.combo_boost_enabled = true;
        a.mods.combo_step = 127;
        a.mods.ground_limit = 127;
        a.mods.combo_boost = 65535;
        // Base 1000; 127 × 65535 × 1000 + 99 wraps to -266989493.
        assert_eq!(damage(&a, &finisher, &target), -2_668_894);
    }

    #[test]
    fn low_hp_terms_difficulty_and_veto() {
        let sora = TargetStats {
            max_hp: 20,
            ..dummy()
        };
        let claw = AttackerStats {
            strength: 5,
            magic: 5,
            max: 3,
            min: 1,
            ..AttackerStats::default()
        };
        let hit = AttackParams {
            power: 65,
            ..AttackParams::default()
        };
        let mut st = TargetState::full(20);
        st.player_type = true;
        assert_eq!(damage_with(&claw, &hit, &sora, &st), 2);
        // A stronger attacker makes the other terms visible.
        let big = AttackerStats {
            strength: 12,
            max: 99,
            ..claw
        };
        let hit = AttackParams { power: 100, ..hit };
        assert_eq!(damage_with(&big, &hit, &sora, &st), 10);
        // Defender below a quarter of HP removes 3 more Defence.
        st.hp = 5;
        st.defender = 3;
        assert_eq!(damage_with(&big, &hit, &sora, &st), 7);
        // Damage Control at 25%: the hit is halved at or below 5 HP.
        st.defender = 0;
        st.damage_control = 0.25;
        assert_eq!(damage_with(&big, &hit, &sora, &st), 5);
        st.hp = 6;
        assert_eq!(damage_with(&big, &hit, &sora, &st), 10);
        // Critical doubles what Sora takes; Beginner halves it.
        st.difficulty = Difficulty::Critical;
        assert_eq!(damage_with(&big, &hit, &sora, &st), 20);
        st.difficulty = Difficulty::Beginner;
        assert_eq!(damage_with(&big, &hit, &sora, &st), 5);
        // Antiform takes × 1.5, truncated, after the difficulty term.
        st.antiform = true;
        assert_eq!(damage_with(&big, &hit, &sora, &st), 7);
        st.difficulty = Difficulty::Standard;
        assert_eq!(damage_with(&big, &hit, &sora, &st), 15);
        // A veto leaves floor + 1.
        assert_eq!(apply_damage(3, 0, 20, 10, true), 1);
        assert_eq!(apply_damage(3, 0, 20, 10, false), 0);
        assert!(sora_veto(true, false, 5, 0, false));
        assert!(!sora_veto(true, false, 1, 0, false));
        assert!(sora_veto(false, true, 1, 0, true));
        assert!(!sora_veto(false, true, 1, 0, false));
    }

    #[test]
    fn hit_record_locks_groups_and_rearms_intervals() {
        let mut r = HitRecord::new(true);
        let pick = |_| (0, 0u8);
        // First hit counts and arms the 30-tick lock.
        assert!(r.hit(1, 0x100, false, pick).0);
        // A different group during the lock is recorded, not counted, and
        // stays masked after the lock ends.
        assert!(!r.hit(2, 0x101, false, pick).0);
        assert!(r.has(2));
        // The lock is frozen while a damage motion plays.
        for _ in 0..40 {
            r.tick(1., true);
        }
        assert!(!r.hit(3, 0x102, false, pick).0);
        for _ in 0..30 {
            r.tick(1., false);
        }
        assert!(r.hit(4, 0x103, false, pick).0);
        // The same group keeps counting and selects the ATKP rows.
        let (new, row) = r.hit(5, 0x103, false, |c| (c as usize, 0));
        assert!(new);
        assert_eq!(row, 1);
        // Enemies never lock; Interval re-arms the id when it expires.
        let mut e = HitRecord::new(false);
        assert!(e.hit(9, 0x100, false, |_| (0, 4)).0);
        assert!(e.hit(10, 0x101, false, pick).0);
        assert!(e.has(9));
        for _ in 0..4 {
            e.tick(1., false);
        }
        assert!(!e.has(9));
        assert_eq!(AttackIds::default().allocate(), 2);
    }

    #[test]
    fn sweep_fills_large_moves_and_spheres_are_strict() {
        assert_eq!(swept([0.; 3], Some([100., 0., 0.]), 40.).len(), 2);
        assert_eq!(swept([0.; 3], Some([130., 0., 0.]), 40.).len(), 3);
        assert_eq!(swept([0.; 3], Some([50., 0., 0.]), 40.).len(), 1);
        assert!(spheres_overlap([0.; 3], 1., [1.9, 0., 0.], 1.));
        assert!(!spheres_overlap([0.; 3], 1., [2., 0., 0.], 1.));
        // Ellipsoid 200 x 80 against a 50 sphere: reach 250 across, 130
        // up; strict at the edge.
        assert!(volume_hits_sphere([0.; 3], 200., 80., [240., 0., 0.], 50.));
        assert!(!volume_hits_sphere([0.; 3], 200., 80., [250., 0., 0.], 50.));
        assert!(!volume_hits_sphere([0.; 3], 200., 80., [0., 130., 0.], 50.));
        assert!(volume_hits_sphere([0.; 3], 80., 80., [0., 100., 0.], 30.));
        // A 40-unit shard moving 100 a tick fills the gap every 40.
        assert_eq!(
            swept_volume([0.; 3], Some([100., 0., 0.]), 40., 40.).len(),
            3
        );
        assert_eq!(
            swept_volume([0.; 3], Some([15., 0., 0.]), 40., 40.).len(),
            1
        );
        assert_eq!(reaction_class(7), 4);
        assert_eq!(reaction_class(3), 3);
    }
}
