//! The Customize battle style (character record +0xf4) in a party
//! member's targeting, FUN_1401b8910: the leash from Sora that sends the
//! member back to him, then each tick either the style's keep test
//! (0x140723ad0) holds the current target or the style's pick
//! (0x140723b30) chooses again.
//!
//! The distances are the personality header's (Donald's and Goofy's are
//! the same): +0x68 search 800 (FUN_1401cc8f0), +0x6c keep 1200
//! (FUN_1401cc870), +0x80 leash 1200 (FUN_1401cc800), +0x84 near Sora 100
//! (FUN_1401cc960), +0x88 away from Sora 1000 (FUN_1401cc7f0). A pick
//! measures to the enemy's nearest collision point (FUN_1403c9c00: the
//! distance less its radius); a keep test to its position.
use crate::Vec3;

pub const TECHNIC: u8 = 0;
pub const TARGET: u8 = 1;
pub const HUDDLE: u8 = 2;
pub const PARTY: u8 = 3;
pub const SORA: u8 = 4;
pub const RELENTLESS: u8 = 5;

const SEARCH: f32 = 800.;
const KEEP: f32 = 1200.;
const LEASH: f32 = 1200.;
const NEAR_SORA: f32 = 100.;
const AWAY_FROM_SORA: f32 = 1000.;

/// An enemy a member may fight: a key that stays its own across ticks,
/// where it stands and the radius its distances leave out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub key: u64,
    pub position: Vec3,
    pub radius: f32,
}

/// What the keep tests and picks read: the member, Sora, the member's last
/// attacker (obj+0xf88), Sora's lock-on (DAT_140ad6ac8) and Sora's current
/// target (DAT_140ad6a78), and the enemies that can be fought.
#[derive(Clone, Copy, Debug)]
pub struct Field<'a> {
    pub me: Vec3,
    pub sora: Vec3,
    pub attacker: Option<u64>,
    pub lock: Option<u64>,
    pub sora_target: Option<u64>,
    pub enemies: &'a [Candidate],
    /// The member's current action entry (obj+0xee8) is an action or an
    /// item (entry flag bit 0 clear), not a wait command
    /// ([`crate::party_members::Follower::acted`]).
    pub acted: bool,
}

fn distance(a: Vec3, b: Vec3) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

impl Field<'_> {
    fn find(&self, key: u64) -> Option<&Candidate> {
        self.enemies.iter().find(|c| c.key == key)
    }

    /// FUN_1401b7ba0: that enemy when its surface is within `reach`.
    fn within(&self, key: Option<u64>, reach: f32) -> Option<u64> {
        let c = self.find(key?)?;
        (distance(self.me, c.position) - c.radius < reach).then_some(c.key)
    }
}

/// FUN_1401b8910's leash: how far from Sora the member may be before it
/// goes back to him (Huddle × 0.5, Party and Relentless × 2).
pub fn leash(style: u8) -> f32 {
    LEASH
        * match style {
            HUDDLE => 0.5,
            PARTY | RELENTLESS => 2.,
            _ => 1.,
        }
}

/// The style's keep test: true drops the current target.
pub fn drops(style: u8, current: &Candidate, f: &Field) -> bool {
    let me = distance(current.position, f.me);
    let sora = distance(current.position, f.sora);
    match style {
        // FUN_1401b3520: another last attacker, or the target out of reach.
        TECHNIC => f.attacker.is_some_and(|a| a != current.key) || me > KEEP,
        // FUN_1401b3710: a lock-on on another enemy.
        TARGET => f.lock.is_some_and(|l| l != current.key),
        // FUN_1401b35e0: kept within reach and near Sora.
        HUDDLE => !(me <= KEEP && sora <= (NEAR_SORA + AWAY_FROM_SORA) * 0.5),
        // FUN_1401b33f0: dropped after an action entry; else kept within
        // reach and away from Sora.
        PARTY => f.acted || !(me <= KEEP && sora >= AWAY_FROM_SORA),
        // FUN_1401b3700 always picks again; FUN_1401b35d0 never does.
        SORA => true,
        _ => false,
    }
}

/// The style's pick (None: no target).
pub fn pick(style: u8, f: &Field) -> Option<u64> {
    let surface = |c: &Candidate, at: Vec3| distance(at, c.position) - c.radius;
    match style {
        // FUN_1401b74a0: Sora's current target, else his lock-on within reach.
        TARGET => f
            .sora_target
            .and_then(|k| f.find(k))
            .map(|c| c.key)
            .or_else(|| f.within(f.lock, SEARCH)),
        // FUN_1401b6fb0: the nearest enemy within reach that is near Sora.
        HUDDLE => {
            let mut best: Option<(f32, u64)> = None;
            for c in f.enemies {
                let d = surface(c, f.me);
                if d < best.map_or(SEARCH, |b| b.0) && surface(c, f.sora) < NEAR_SORA {
                    best = Some((d, c.key));
                }
            }
            best.map(|b| b.1)
        }
        // FUN_1401b6960: within twice the reach, the one farthest from Sora.
        PARTY => {
            let mut best: Option<(f32, u64)> = None;
            for c in f.enemies {
                if surface(c, f.me) >= SEARCH * 2. {
                    continue;
                }
                let d = surface(c, f.sora);
                if d > best.map_or(-1., |b| b.0) {
                    best = Some((d, c.key));
                }
            }
            best.map(|b| b.1)
        }
        // FUN_1401b7490: none.
        SORA => None,
        // FUN_1401b6b80 (Technic, and Relentless through its thunk): the
        // last attacker within reach, else the nearest enemy within reach.
        _ => f.within(f.attacker, SEARCH).or_else(|| {
            let mut best: Option<(f32, u64)> = None;
            for c in f.enemies {
                let d = surface(c, f.me);
                if d < best.map_or(SEARCH, |b| b.0) {
                    best = Some((d, c.key));
                }
            }
            best.map(|b| b.1)
        }),
    }
}

/// One member's targeting state: the target's key and the leash flag
/// (obj+0xe18 bit 6), which the member clears once it stands at its spot
/// behind Sora (FUN_1401b41a0). While the flag stands the member has no
/// target: `FUN_1401b4d20` (a target to act on) reads it as none, so no
/// action entry that needs one is eligible (`FUN_1401b2880`) and the
/// acts stop facing and closing in (`FUN_1401ccba0`, `FUN_1401ce8f0`),
/// and the style think (`FUN_1401b3340`, vtable 0x142b976d0) returns
/// before its battle states.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Targeting {
    pub target: Option<u64>,
    pub far: bool,
}

impl Targeting {
    /// FUN_1401b8910 for one tick: out of battle no target; the leash
    /// raises the flag; a target that is gone, or whose keep test drops
    /// it, is picked again. While the flag stands the member has no target
    /// and follows Sora.
    pub fn update(&mut self, style: u8, battle: bool, f: &Field) -> Option<u64> {
        let flat = ((f.me[0] - f.sora[0]).powi(2) + (f.me[2] - f.sora[2]).powi(2)).sqrt();
        if flat > leash(style) {
            self.far = true;
        }
        if !battle {
            self.target = None;
            return None;
        }
        let current = self.target.and_then(|k| f.find(k)).copied();
        let keep = !self.far && current.is_some_and(|c| !drops(style, &c, f));
        if !keep {
            self.target = pick(style, f);
        }
        if self.far { None } else { self.target }
    }

    /// FUN_1401b41a0: standing at the formation spot clears the flag.
    pub fn settled(&mut self) {
        self.far = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(key: u64, x: f32) -> Candidate {
        Candidate {
            key,
            position: [x, 0., 0.],
            radius: 40.,
        }
    }

    fn field(enemies: &[Candidate]) -> Field<'_> {
        Field {
            me: [0.; 3],
            sora: [0.; 3],
            attacker: None,
            lock: None,
            sora_target: None,
            enemies,
            acted: false,
        }
    }

    #[test]
    fn technic_takes_its_last_attacker_else_the_nearest() {
        let e = [at(1, 500.), at(2, 300.)];
        let mut f = field(&e);
        assert_eq!(pick(TECHNIC, &f), Some(2));
        f.attacker = Some(1);
        assert_eq!(pick(TECHNIC, &f), Some(1));
        // Another attacker drops the target; so does distance.
        assert!(drops(TECHNIC, &e[1], &f));
        f.attacker = None;
        assert!(!drops(TECHNIC, &e[1], &f));
        assert!(drops(TECHNIC, &at(3, 1300.), &f));
        // Beyond the search radius nothing is picked.
        assert_eq!(pick(TECHNIC, &field(&[at(1, 900.)])), None);
    }

    #[test]
    fn target_follows_sora_and_sora_style_never_picks() {
        let e = [at(1, 500.), at(2, 300.)];
        let mut f = field(&e);
        assert_eq!(pick(TARGET, &f), None);
        f.lock = Some(1);
        assert_eq!(pick(TARGET, &f), Some(1));
        f.sora_target = Some(2);
        assert_eq!(pick(TARGET, &f), Some(2));
        assert!(drops(TARGET, &e[1], &f));
        assert_eq!(pick(SORA, &f), None);
        assert!(drops(SORA, &e[0], &f));
        assert!(!drops(RELENTLESS, &at(1, 5000.), &f));
    }

    #[test]
    fn huddle_stays_by_sora_and_party_goes_far_from_him() {
        let e = [at(1, 120.), at(2, 600.), at(3, 1500.)];
        let f = field(&e);
        // Huddle: only an enemy whose surface is within 100 of Sora.
        assert_eq!(pick(HUDDLE, &f), Some(1));
        assert!(drops(HUDDLE, &e[1], &f));
        assert!(!drops(HUDDLE, &e[0], &f));
        // Party: within 1600, the farthest from Sora.
        assert_eq!(pick(PARTY, &f), Some(3));
        assert!(drops(PARTY, &e[1], &f));
        assert!(!drops(PARTY, &at(4, 1100.), &f));
    }

    #[test]
    fn party_attack_picks_again_after_an_action_entry() {
        let e = [at(4, 1100.)];
        let mut f = field(&e);
        assert!(!drops(PARTY, &e[0], &f));
        f.acted = true;
        assert!(drops(PARTY, &e[0], &f));
        // The other styles' keep tests do not read it.
        assert!(!drops(TARGET, &e[0], &f));
    }

    #[test]
    fn the_leash_sends_the_member_back_until_it_settles() {
        let e = [at(1, 300.)];
        let mut f = field(&e);
        let mut t = Targeting::default();
        assert_eq!(t.update(TECHNIC, true, &f), Some(1));
        assert_eq!(t.update(TECHNIC, false, &f), None);
        f.me = [700., 0., 0.];
        // Huddle's leash is 600: too far, follow Sora.
        assert_eq!(t.update(HUDDLE, true, &f), None);
        assert!(t.far);
        f.me = [0.; 3];
        assert_eq!(t.update(HUDDLE, true, &f), None);
        t.settled();
        assert_eq!(t.update(TECHNIC, true, &f), Some(1));
        assert_eq!((leash(PARTY), leash(SORA)), (2400., 1200.));
    }
}
