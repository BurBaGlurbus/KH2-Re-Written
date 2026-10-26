//! Party members (Donald, Goofy) as the executable's `kn::Friend` objects:
//! the follow behaviour of FUN_1401b41a0 / FUN_1401b4820, the slot
//! formation, and the attack choice of FUN_1401b1d40 with its action gate
//! FUN_1401b2880, on the shared movement rules.
//!
//! The thresholds are the personality resource's header floats read by
//! FUN_1401cc890 / FUN_1401cc8c0; the formation distance is not decoded
//! (`DAT_14071d760` is filled at run time) and is a test-room value. The
//! friend AI keeps no combo chain: each decision picks one eligible record
//! at random; a pick that would make a fourth repeat falls back on the
//! Rare entries.
//!
//! Donald's four spells are records too (57-60, `FUN_1401cc650` /
//! `FUN_1401cc1a0`): they need the think loop's random MP-scaled permit
//! (`FUN_1401b53b0`) and an MP bar that is not charging, and play the act of
//! `party_spell`. Each spell is an ability: the member's character record
//! ([`Save`]) must hold it equipped, and its Customize frequency decides
//! when the loop tries it.
//!
//! The personality's table also holds the item entries (`FUN_1401cc970` for
//! HP, `FUN_1401cce90` for MP): a member under 45% of its HP or MP uses the
//! first item of a preference list that its own item slots hold, on itself,
//! Sora or the other member, and a partner in need (`FUN_1401b3a30`, entry
//! flags 0x10 and 0x20) is served before any attack is chosen.

use crate::actor_physics::{self, Knock, Sampler, Sstm};
use crate::party_hit;
use crate::party_motion::{self, BLEND, Clips, Ctl, FOLLOW, Goal, Wish};
use crate::{Body, Hit, Vec3, World};

/// The falling motion (id 4) a member plays after an air attack.
pub const FALL: u16 = 4;
/// The horizontal speed of an air attack's jump (`FUN_1401b0420`) and the
/// highest jump of Donald and Goofy (PRTY word 2, `obj+0x130`: 385).
const AIR_SPEED: f32 = 8.;
pub const AIR_MAX: f32 = 385.;

/// Walk and run speeds per tick (PRTY words 0 and 1: 2.0 and 8.0).
pub const WALK_SPEED: f32 = 2.;
pub const RUN_SPEED: f32 = 8.;
/// Turn rate per tick (prty +0xc = 0.262, FUN_1403b4780).
const TURN: f32 = 0.262;
/// Slot yaw offset (the exe uses 0.349 × 2i − 0.349; widened here so the
/// two bodies do not overlap, as the formation table is not decoded) and
/// the formation distance behind the leader [test-room value].
const SLOT_YAW: f32 = 0.6;
const FORMATION: f32 = 150.;
/// Target search radius and the leash from the leader (personality +0x68,
/// +0x80).
pub const SEARCH_RADIUS: f32 = 800.;
pub const LEADER_LEASH: f32 = 1200.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Stand,
    Walk,
    Run,
}

/// What a personality's header gives a member's movement: the follow
/// distances, the highest jump (PRTY word 2), the chance of a hop after an
/// action (+0x64) with its rows (+0xa8/+0xac: heading offset in degrees,
/// height, horizontal speed, inertia, rise time) and the two wait commands'
/// timers (+0x70/+0x74 holding before the enemy, +0x78/+0x7c following).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Personality {
    pub follow: party_motion::Follow,
    pub jump_max: f32,
    pub hop_chance: f32,
    pub hops: &'static [[f32; 5]],
    pub waits: [[f32; 2]; 2],
}

/// Donald's header (0x14071d850).
pub const DONALD_PERSONALITY: Personality = Personality {
    follow: FOLLOW,
    jump_max: AIR_MAX,
    hop_chance: 0.1,
    hops: &[[180., 15., 12., 0.98, 12.]],
    waits: [[6., 60.], [6., 180.]],
};

/// A wait command of the personality's table (entries 0 and 1, flag 0x201,
/// `FUN_1401ccba0` / `FUN_1401ccdf0`): chosen only while no action is
/// possible, kept while its timer runs and no action becomes possible.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Wait {
    /// Stand still facing the enemy.
    Hold(f32),
    /// Follow: close in on the enemy (`FUN_1401b23d0`).
    Follow(f32),
}

/// An idle hop (physics mode 2, `FUN_1403cbec0`), armed until the jump
/// motion's take-off.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Hop {
    armed: bool,
    velocity: [f32; 2],
    /// Y-down vertical speed (negative rises).
    vy_down: f32,
    gravity: f32,
    inertia: f32,
    sampler: Sampler,
    /// The fall motion has started.
    fell: bool,
}

/// The acts of the party's BDX scripts: Donald's `p_ex020` spells (`fire`,
/// `blizzard`, `thunder`, `cure`) and Goofy's `p_ex030` specials
/// (`tornado`, `turbo`, and `blade`, his Bash, which the room runs on the
/// object-script engine).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spell {
    Fire,
    Blizzard,
    Thunder,
    Cure,
    Tornado,
    Turbo,
    Bash,
    /// Any other act of the member's own script, by name, run by the
    /// object-script engine as the Bash is (`FUN_1401cc650` starts the act
    /// the command entry names: Auron's `yukizakura`, Tron's
    /// `normal_attack1`, ...).
    Act(&'static str),
}

impl Spell {
    /// The script act the engine runs for this record (the Bash is Goofy's
    /// `blade`).
    pub fn act(self) -> Option<&'static str> {
        match self {
            Spell::Bash => Some("blade"),
            Spell::Act(name) => Some(name),
            _ => None,
        }
    }
}

/// FUN_1401cd270 / FUN_1401cd230 (the map at 0x1407249d0, filled into each
/// action entry by FUN_1401cccf0): a spell's ability item and its index in
/// the member's frequencies (FUN_1401dc060: 7 + the index). A guest's
/// own act has no row here [U: only Donald's and Goofy's rows are listed].
pub fn ability_of(spell: Spell) -> Option<(u16, usize)> {
    Some(match spell {
        Spell::Fire => (0xa5, 7),
        Spell::Blizzard => (0xa6, 8),
        Spell::Thunder => (0xa7, 9),
        Spell::Cure => (0xa8, 10),
        Spell::Tornado => (0x1a7, 7),
        Spell::Bash => (0x1ad, 8),
        Spell::Turbo => (0x1a9, 9),
        Spell::Act(_) => return None,
    })
}

/// FUN_1401dc060: an item's or ability's place in the frequencies: items
/// 1..7 (Potion .. Megalixir, 0x1407282d0) at 0..6, a party ability at
/// 7 + its per-character index (FUN_1401cd330, the table at 0x1407249d0;
/// only Donald's and Goofy's rows are listed here).
pub fn frequency_index(id: u16) -> Option<usize> {
    const ABILITIES: [Spell; 7] = [
        Spell::Fire,
        Spell::Blizzard,
        Spell::Thunder,
        Spell::Cure,
        Spell::Tornado,
        Spell::Bash,
        Spell::Turbo,
    ];
    match id {
        1..=7 => Some(usize::from(id - 1)),
        _ => ABILITIES
            .iter()
            .filter_map(|&s| ability_of(s))
            .find(|&(a, _)| a == id)
            .map(|(_, k)| k),
    }
}

/// The Customize frequencies (record +0xf5..): 0 Free (items: Often),
/// 1 Well-Balanced, 2 Rare (items: Last hope); 3 makes FUN_1401b2880 refuse.
pub mod frequency {
    pub const FREE: u8 = 0;
    pub const WELL_BALANCED: u8 = 1;
    pub const RARE: u8 = 2;
    pub const OFF: u8 = 3;
}

/// A member's character record (save +0x24f0 + 0x114 * (t - 1),
/// FUN_1403a14e0): what the AI's ability and item tests read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Save {
    /// +0x54: the ability slots; the low 15 bits the ability item, bit 15
    /// set when it is equipped.
    pub abilities: [u16; 80],
    /// +0x34: the item slots, the first `item_slots` (+0x12) in use, and
    /// +0x44 their auto-reload originals (0 = off), which only Customize
    /// and Equip write (FUN_1403598e0, FUN_140354dc0).
    pub items: [u16; 8],
    pub originals: [u16; 8],
    pub item_slots: u8,
    /// +0: the weapon (0 on a new game: no new-game code writes it).
    pub weapon: u16,
    /// +0x14 armour and +0x24 accessories, the first +0x10 / +0x11 in use.
    pub armor: [u16; 8],
    pub armor_slots: u8,
    pub accessories: [u16; 8],
    pub accessory_slots: u8,
    /// +8..+0xb: the AP, Strength, Magic and Defense bonuses the status
    /// build adds to the level row (FUN_1403c1d90).
    pub bonus: [u8; 4],
    /// +0xf4: the Customize battle style (0 Technic .. 5 Relentless).
    pub style: u8,
    /// +0xf5: the frequencies, [0..7] items 1..7, [7 + k] ability k.
    pub frequency: [u8; 31],
}

impl Save {
    pub const EMPTY: Save = Save {
        abilities: [0; 80],
        items: [0; 8],
        originals: [0; 8],
        item_slots: 0,
        weapon: 0,
        armor: [0; 8],
        armor_slots: 0,
        accessories: [0; 8],
        accessory_slots: 0,
        bonus: [0; 4],
        style: 0,
        frequency: [frequency::FREE; 31],
    };

    /// FUN_1403e4530(0) with one plrp row of key 0 on a zeroed record
    /// (FUN_1403a0e70, FUN_1401dc010): the item-slot count (+0xb), then each
    /// id through FUN_1403c3f00(id & 0x7fff, char, id >> 15): a consumable
    /// takes the first empty item slot (FUN_1403a1380), an ability the first
    /// empty ability slot with its equip bit (FUN_1403a1330).
    pub fn new_game(item_slots: u8, ids: &[u16], consumable: impl Fn(u16) -> bool) -> Save {
        let mut s = Save {
            item_slots: item_slots.min(8),
            ..Save::EMPTY
        };
        for &id in ids.iter().filter(|&&id| id != 0) {
            let item = id & 0x7fff;
            let n = usize::from(s.item_slots);
            let (slots, value): (&mut [u16], u16) = if consumable(item) {
                (&mut s.items[..n], item)
            } else {
                (&mut s.abilities, id)
            };
            if let Some(slot) = slots.iter_mut().find(|v| **v == 0) {
                *slot = value;
            }
        }
        s
    }

    /// FUN_1403c4380 for a consumable: how many item slots hold it.
    pub fn count(&self, item: u16) -> u32 {
        self.items[..usize::from(self.item_slots)]
            .iter()
            .filter(|&&v| v == item)
            .count() as u32
    }

    /// FUN_1403c49c0: the last item slot holding it empties.
    pub fn take(&mut self, item: u16) -> bool {
        let n = usize::from(self.item_slots);
        match self.items[..n].iter().rposition(|&v| v == item) {
            Some(j) => {
                self.items[j] = 0;
                true
            }
            None => false,
        }
    }

    /// FUN_1401b2880's ability test: the spell's frequency, or None when it
    /// is off (3), not held or held but not equipped.
    pub fn spell_frequency(&self, spell: Spell) -> Option<u8> {
        // A guest's act is not gated by an ability.
        let Some((item, index)) = ability_of(spell) else {
            return Some(frequency::FREE);
        };
        let f = self.frequency[index];
        let equipped = self
            .abilities
            .iter()
            .any(|&v| v & 0x7fff == item && v & 0x8000 != 0);
        (f != frequency::OFF && equipped).then_some(f)
    }

    /// An item's frequency (FUN_1401dc060: items 1..7 at 0..6).
    pub fn item_frequency(&self, item: u16) -> u8 {
        match item {
            1..=7 => self.frequency[usize::from(item - 1)],
            _ => frequency::FREE,
        }
    }
}

/// The consumables an item entry looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemNeed {
    Hp,
    Mp,
}

/// Which emergency pass of `FUN_1401b3a30` an entry belongs to (entry flag
/// 0x10 or 0x20): run first when a partner is in need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Urgent {
    No,
    Hp,
    Mp,
}

/// The share of the maximum below which a member uses an item
/// (`DAT_1405b41c0`).
const ITEM_BELOW: f32 = 0.45;
/// The item-use motion (id 9, `FUN_1403d72d0`).
pub const ITEM_MOTION: u16 = 9;

/// One action parameter entry (record of 0x80 bytes): the attack's motion
/// and its range window (FUN_1401b2880).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub id: u8,
    pub motion: u16,
    pub near: f32,
    pub far: f32,
    /// The target's height above the member must lie in
    /// `[low − r, high + r]` (the exe's Y-down window, negated).
    pub low: f32,
    pub high: f32,
    /// Air records jump first (+0x20 ≠ 0, `FUN_1401b0420`): the jump
    /// reaches `jump` above the target's height, at most the member's
    /// highest jump.
    pub air: bool,
    pub jump: f32,
    /// MP cost (+8); 255 starts an MP Charge.
    pub cost: u8,
    /// The act this record starts instead of a weapon motion.
    pub spell: Option<Spell>,
    /// Cure: a member is healed while its HP is below this share of its
    /// maximum (+0x3c).
    pub heal_below: f32,
    /// An item entry: the need it serves (it plays the item-use motion).
    pub item: Option<ItemNeed>,
    pub urgent: Urgent,
}

impl Record {
    /// The record as the executable keeps it (0x80 bytes, the words a script
    /// reads through `trap_friend_get_action_param`): id, motion, cost, the
    /// distance and height windows (+0x10..+0x1c), the jump offset (+0x20)
    /// and the Cure's HP share (+0x3c). The flags word (+0xc) is not kept
    /// here and reads 0.
    pub fn raw_words(&self) -> [u32; 32] {
        let mut w = [0; 32];
        w[0] = u32::from(self.id);
        w[1] = u32::from(self.motion);
        w[2] = u32::from(self.cost);
        w[4] = self.near.to_bits();
        w[5] = self.far.to_bits();
        w[6] = self.low.to_bits();
        w[7] = self.high.to_bits();
        w[8] = self.jump.to_bits();
        w[15] = self.heal_below.to_bits();
        w
    }
}

const fn ground(id: u8, motion: u16, far: f32) -> Record {
    Record {
        id,
        motion,
        near: 0.,
        far,
        low: -100.,
        high: 150.,
        air: false,
        jump: 0.,
        cost: 0,
        spell: None,
        heal_below: 0.,
        item: None,
        urgent: Urgent::No,
    }
}

/// The HP or MP item entry of a personality table (ids are this port's: the
/// exe's entries have none).
const fn item_entry(id: u8, need: ItemNeed) -> Record {
    Record {
        item: Some(need),
        urgent: match need {
            ItemNeed::Hp => Urgent::Hp,
            ItemNeed::Mp => Urgent::Mp,
        },
        ..ground(id, ITEM_MOTION, 0.)
    }
}

/// An air attack: the target must stand 100-385 above the member (the
/// exe's Y-down window -385/-100), and the member jumps up to it first.
const fn air(id: u8, motion: u16, far: f32) -> Record {
    Record {
        low: 100.,
        high: 385.,
        air: true,
        jump: 1.,
        ..ground(id, motion, far)
    }
}

/// A spell record: the act's motion, its range window (distances and the
/// Y-down height window negated into `low`/`high`) and MP cost.
const fn spell(id: u8, spell: Spell, motion: u16, window: [f32; 4], cost: u8) -> Record {
    Record {
        id,
        motion,
        near: window[0],
        far: window[1],
        low: window[2],
        high: window[3],
        air: false,
        jump: 0.,
        cost,
        spell: Some(spell),
        heal_below: 0.,
        item: None,
        urgent: Urgent::No,
    }
}

/// Donald's records (resource at 0x14071d850): the two ground attacks, the
/// two air attacks (ids 2, 3, motions 181 and 182) and the spells 57-60
/// (Fire 0-350, Blizzard 250-750, Thunder 0-500 with their height windows;
/// Cure costs 255 and heals below 0.4 of the HP). The act motions are 192
/// (Fire, Thunder, Cure) and 191 (Blizzard).
pub const DONALD: [Record; 10] = [
    ground(0, 151, 200.),
    ground(1, 152, 180.),
    air(2, 181, 150.),
    air(3, 182, 130.),
    spell(57, Spell::Fire, 192, [0., 350., -100., 200.], 10),
    spell(58, Spell::Blizzard, 191, [250., 750., -100., 150.], 10),
    spell(59, Spell::Thunder, 192, [0., 500., -500., 1000.], 10),
    Record {
        heal_below: 0.4,
        urgent: Urgent::Hp,
        ..spell(60, Spell::Cure, 192, [0., 0., 0., 0.], 255)
    },
    item_entry(200, ItemNeed::Hp),
    item_entry(201, ItemNeed::Mp),
];
/// Goofy's records (personality 1): the two ground attacks, the two air
/// attacks (ids 6, 7) and the specials Tornado (id 92, range 0-500,
/// height window -100/500), Bash (id 93, 250-750, -500/1000) and Turbo
/// (id 94, 0-500, -100/150), 10 MP each.
pub const GOOFY: [Record; 9] = [
    ground(4, 151, 220.),
    ground(5, 152, 300.),
    air(6, 181, 100.),
    air(7, 182, 80.),
    spell(92, Spell::Tornado, 191, [0., 500., -100., 500.], 10),
    spell(93, Spell::Bash, 192, [250., 750., -500., 1000.], 10),
    spell(94, Spell::Turbo, 194, [0., 500., -100., 150.], 10),
    item_entry(200, ItemNeed::Hp),
    item_entry(201, ItemNeed::Mp),
];

/// The enemy a member fights: its centre and radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub position: Vec3,
    pub radius: f32,
}

/// What the object-script engine says about a scripted act this tick (the
/// Bash, a guest's special): the member's motion and its tick, the engine's
/// count of motion starts and the blend left on the one playing, its place,
/// and whether the act has ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scripted {
    pub motion: u16,
    pub tick: u32,
    pub serial: u64,
    pub blend: f32,
    /// The first two motions the engine queued (motion, blend).
    pub queue: [Option<(u16, f32)>; 2],
    pub position: Vec3,
    pub yaw: f32,
    pub done: bool,
}

/// An attack in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attack {
    pub record: u8,
    pub motion: u16,
    pub tick: u32,
    /// The spell this attack is the cast of.
    pub spell: Option<Spell>,
    /// Cure: the party slot healed (0 Sora, 1 Donald, 2 Goofy).
    pub heal: u8,
    /// An item use: the item id and the party slot it is for.
    pub item: Option<(u16, u8)>,
    /// Ticks the motion's blend holds its first frame (FUN_1403c8a40: a
    /// blend of 8 starts the clock at 2): the act's waits count them too.
    pub hold: u32,
}

/// A motion the member's state asks the controller for: started
/// (FUN_1403c86a0) or changed (FUN_1403c6dc0) with a blend, and the motion
/// queued behind it (FUN_1403c8310).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Request {
    motion: u16,
    blend: f32,
    change: bool,
    then: Option<(u16, f32)>,
}

impl Request {
    const fn start(motion: u16, blend: f32, then: Option<(u16, f32)>) -> Self {
        Request {
            motion,
            blend,
            change: false,
            then,
        }
    }
}

/// The blend and the motion queued behind a record's motion: the weapon
/// attacks start at once with idle (fall in the air) behind them
/// (FUN_1401b0420); Donald's spells `motion_start(m, 8)` then idle (p_ex020
/// L2039, L2611, L5085, L5522); Tornado changes to 191 with fall behind
/// (p_ex030 L1993), Turbo to 194 with idle (L2950); the Bash starts 192 at 8
/// (L3453); the item motion starts at 8 with idle (FUN_1403d6fc0).
fn request_of(a: &Attack, air: bool) -> Request {
    let idle = Some((party_motion::IDLE, 0.));
    match a.spell {
        _ if a.item.is_some() => Request::start(a.motion, BLEND, idle),
        Some(Spell::Fire | Spell::Blizzard | Spell::Thunder | Spell::Cure) => {
            Request::start(a.motion, BLEND, idle)
        }
        Some(Spell::Tornado) => Request {
            change: true,
            ..Request::start(a.motion, 0., Some((FALL, 0.)))
        },
        Some(Spell::Turbo) => Request {
            change: true,
            ..Request::start(a.motion, 0., idle)
        },
        // The engine's act plays its own motions (`animate` follows them).
        Some(Spell::Bash | Spell::Act(_)) => Request::start(a.motion, BLEND, None),
        None if air => Request::start(a.motion, 0., Some((FALL, 0.))),
        None => Request::start(a.motion, 0., idle),
    }
}

/// The motion calls of the reactions (damage-02 section 7): a flinch
/// starts at once with idle behind it (FUN_1403d43b0), an air flinch with
/// the fall behind it (FUN_1403d3840), the blows and their land motions
/// with nothing (FUN_1403d2fe0 case 4, FUN_140422700), the guard bounce
/// with idle (FUN_1403d4650), the mode-1 land with idle (FUN_1403cb940) and
/// the get-up blended over 8 with idle (FUN_1403c3c30).
fn reaction_request(r: &party_hit::Reaction) -> Request {
    let idle = Some((party_motion::IDLE, 0.));
    match r.phase {
        party_hit::Phase::Flinch | party_hit::Phase::Bounce => Request::start(r.motion, 0., idle),
        party_hit::Phase::Air => Request::start(r.motion, 0., Some((FALL, 0.))),
        party_hit::Phase::Blow => Request::start(r.motion, 0., None),
        party_hit::Phase::Land if r.motion == party_hit::motion::LAND => {
            Request::start(r.motion, 0., idle)
        }
        party_hit::Phase::Land => Request::start(r.motion, 0., None),
        party_hit::Phase::GetUp => Request::start(r.motion, BLEND, idle),
    }
}

/// The movement loop of Goofy's Tornado and Turbo (`p_ex030` L1967 and
/// L2924): `obj_set_movement(heading, speed)` each tick, the heading
/// turning toward the goal by at most 0.1745 per tick.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Special {
    heading: Vec3,
    /// Tornado's hover phase (+5 degrees a tick).
    spin: f32,
    /// Turbo's re-aim timer and count.
    timer: f32,
    count: u32,
    goal: Vec3,
    start_y: f32,
    started: bool,
}

impl Special {
    fn new() -> Self {
        Special {
            heading: [0., 1., 0.],
            spin: 0.,
            timer: 4.,
            count: 0,
            goal: [0.; 3],
            start_y: 0.,
            started: false,
        }
    }
}

/// A party member that follows the leader and fights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follower {
    pub position: Vec3,
    pub yaw: f32,
    pub gait: Gait,
    /// Party slot 0 or 1 (obj+0xdd0).
    pub slot: u8,
    /// Ticks in the current motion.
    pub tick: u32,
    pub attack: Option<Attack>,
    /// The last three chosen records (obj+0xed0..0xee0).
    history: [u8; 3],
    seed: u32,
    /// The think loop's spell permit (obj+0xe18 bit 5), rolled each
    /// decision.
    pub permit: bool,
    special: Special,
    /// Downward speed while falling after a special.
    fall: f32,
    /// A hit reaction in progress (FUN_1403d2fe0): the member cannot act.
    pub reaction: Option<party_hit::Reaction>,
    /// A knock-slide that has no reaction motion (reaction class 1).
    slide: Option<Knock>,
    /// The jump of an air attack (and the fall after it).
    air: Option<party_hit::Airborne>,
    /// KO: the revive timer (obj+0xdc0) while the dead bit (obj+0x9b8 bit
    /// 2) is set.
    pub down: Option<f32>,
    /// The DEAD motion has started (`FUN_1403c3660`).
    laid: bool,
    /// The motion controller (obj+0x158) and the motion the state asked
    /// for this tick.
    pub ctl: Ctl,
    request: Option<Request>,
    /// The controller start that belongs to the running attack.
    attack_serial: Option<u32>,
    /// The follow state (obj+0xe14: 0 stand, 1 walk, 2 run), the smoothed
    /// wish (FUN_1401b1b20) and the speed it snapped to (obj+0x1c).
    state: u8,
    wish: Wish,
    pub speed: f32,
    /// The battle stance (obj+0x120 bit 20): the battle variants of the
    /// motions play.
    pub battle_stance: bool,
    /// The personality (follow distances, highest jump, hops, waits).
    pub personality: Personality,
    /// The wait command running, the last command was an action, and an
    /// idle hop in progress.
    wait: Option<Wait>,
    last_action: bool,
    /// The current action entry (obj+0xee8) is an action or an item (entry
    /// flag bit 0 clear) rather than a wait command (0x201) or none.
    acted: bool,
    hop: Option<Hop>,
    /// The engine's motion start the controller follows during an act.
    act_serial: Option<u64>,
    /// The act's last frame when it ended on a motion it had just started
    /// (the Bash's 193): `animate` still starts it.
    act_tail: Option<Scripted>,
}

fn wrap(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

/// What a tick needs from the room.
pub struct Scene<'a, W: World> {
    pub leader: Vec3,
    pub leader_yaw: f32,
    /// The enemy to fight, when one can be hit and a fight is on.
    pub target: Option<Target>,
    pub world: &'a W,
    pub body: Body,
    /// Ticks of each attack motion (None: not loaded).
    pub length: &'a dyn Fn(u16) -> Option<u32>,
    /// Root-motion displacement of a motion at a tick (model space).
    pub root: &'a dyn Fn(u16, u32) -> Vec3,
    /// A spell act is still running (the act manager's lock, obj+0x4a8):
    /// no new decision.
    pub busy: bool,
    /// The AI may choose spells (the tour orders its casts instead).
    pub spells: bool,
    /// The enemy target's centre (`target_pos`), for the specials' goals.
    pub centre: Option<Vec3>,
    /// Sora's feet height (`trap_sysobj_player`), Tornado's hover reference.
    pub leader_y: f32,
    /// The motion's range 16 (`range_bit(obj, 1)`): the ticks the special
    /// moves in.
    pub range: &'a dyn Fn(u16) -> Option<(u32, u32)>,
    /// This member's MP: current, maximum and whether an MP Charge runs.
    pub mp: Option<(i32, i32, bool)>,
    /// HP and maximum HP of Sora, Donald and Goofy (None: absent).
    pub hp: [Option<(i32, i32)>; 3],
    /// MP and maximum MP of the same three.
    pub mp_all: [Option<(i32, i32)>; 3],
    /// The member's character record: its abilities, item slots and
    /// frequencies.
    pub save: &'a Save,
    /// The motion's range 2 start, the tick a reaction lets the member act
    /// again (`obj+0x18c` bits 2-4).
    pub actionable: &'a dyn Fn(u16) -> Option<u32>,
    /// SSTM (knock-back and blow constants) and the KO rates.
    pub sstm: Sstm,
    pub rates: party_hit::Rates,
    /// The battle flag (`DAT_142a11404`): the KO timer runs at 1.0 a tick
    /// instead of SSTM+0x5c.
    pub battle: bool,
    /// The running Bash, as the engine ran it this tick (None while the room
    /// has not started it).
    pub scripted: Option<Scripted>,
    /// The member's own script is loaded, so its acts (the Bash, the guests'
    /// specials) can run.
    pub can_bash: bool,
    /// The motion set as the controller reads it (None: the controller
    /// does not run and the attack counts its own ticks).
    pub clips: Option<&'a dyn Clips>,
}

impl Follower {
    /// FUN_1403c2fc0: at the leader's position, facing its direction.
    pub fn spawn(leader: Vec3, leader_yaw: f32, slot: u8) -> Self {
        Follower {
            position: leader,
            yaw: leader_yaw,
            gait: Gait::Stand,
            slot,
            tick: 0,
            attack: None,
            history: [255; 3],
            seed: 0x1234_5678 ^ (slot as u32 * 0x9e37_79b9),
            permit: false,
            special: Special::new(),
            fall: 0.,
            reaction: None,
            slide: None,
            air: None,
            down: None,
            laid: false,
            ctl: Ctl::default(),
            request: Some(Request::start(party_motion::IDLE, 0., None)),
            attack_serial: None,
            state: 0,
            wish: Wish::default(),
            speed: 0.,
            battle_stance: false,
            personality: DONALD_PERSONALITY,
            wait: None,
            last_action: false,
            acted: false,
            hop: None,
            act_serial: None,
            act_tail: None,
        }
    }

    /// A scene drives the member (a party limit): it stands where the scene
    /// puts it and plays the scene's motion as an attack (its windows hit),
    /// or stands idle between motions. Its own decisions wait.
    pub fn follow_script(&mut self, s: Scripted) {
        self.position = s.position;
        self.yaw = s.yaw;
        self.tick = s.tick;
        self.reaction = None;
        self.slide = None;
        self.air = None;
        self.special = Special::new();
        self.gait = Gait::Stand;
        // The scene's motion is shown, not the controller's; the controller
        // starts again from idle once the scene lets go.
        self.ctl = Ctl::default();
        self.request = Some(Request::start(party_motion::IDLE, 0., None));
        self.speed = 0.;
        self.state = 0;
        self.wait = None;
        self.attack = (s.motion != 0 && !s.done).then_some(Attack {
            record: 0,
            motion: s.motion,
            tick: s.tick,
            spell: None,
            heal: 0,
            item: None,
            hold: 0,
        });
    }

    /// The personality's follow distances and the member's highest jump
    /// (PRTY word 2).
    pub fn with_personality(mut self, personality: Personality) -> Self {
        self.personality = personality;
        self
    }

    /// The current action entry is an action or an item (Party Attack's
    /// keep test drops its target then, `FUN_1401b33f0`).
    pub fn acted(&self) -> bool {
        self.acted
    }

    /// The motion id playing: an attack, or the gait (idle 0, walk 1,
    /// run 2).
    pub fn motion(&self) -> u16 {
        if let Some(r) = self.reaction {
            return r.motion;
        }
        if self.laid {
            return party_hit::motion::DEAD;
        }
        if let Some(a) = self.attack {
            return a.motion;
        }
        if self.air.is_some() {
            // Falling after the attack's motion has ended.
            return FALL;
        }
        match self.gait {
            Gait::Stand => 0,
            Gait::Walk => 1,
            Gait::Run => 2,
        }
    }

    fn rand(&mut self, n: u32) -> u32 {
        // CRT-style LCG [the seed is unknown, U].
        self.seed = self.seed.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.seed >> 16) % n.max(1)
    }

    /// The goal point: behind the leader at the slot's yaw offset.
    pub fn goal(&self, leader: Vec3, leader_yaw: f32) -> Vec3 {
        self.goal_at(leader, leader_yaw, FORMATION)
    }

    fn goal_at(&self, leader: Vec3, leader_yaw: f32, distance: f32) -> Vec3 {
        let offset = SLOT_YAW * 2. * self.slot as f32 - SLOT_YAW;
        let a = leader_yaw + offset;
        [
            leader[0] - a.sin() * distance,
            leader[1],
            leader[2] - a.cos() * distance,
        ]
    }

    /// Party Attack in battle doubles the formation distance
    /// (FUN_1401b2450 / FUN_1401b4bd0) and the follow thresholds toward the
    /// leader (FUN_1401b41a0).
    fn party_attack<W: World>(scene: &Scene<W>) -> f32 {
        if scene.battle && scene.save.style == crate::party_style::PARTY {
            2.
        } else {
            1.
        }
    }

    /// FUN_1401b2880's range test of one record against the target.
    pub fn eligible(&self, r: &Record, t: &Target) -> bool {
        // FUN_1401b2880 tests the range only for an action entry whose
        // flag byte has bit 1. In Donald's table (static constructor
        // FUN_140009d40, entries from 0x1407250a0) Fire, Blizzard and
        // Thunder have flags 2; Cure (record 60, runner FUN_1401cc1a0) has
        // 0x10 and the item entries 0x18 / 0x28, so their records' range
        // (Cure's reads 0-0) is never read: the cast waits on the heal
        // target alone. A guest's heal record waits the same way.
        if r.spell == Some(Spell::Cure) || r.heal_below > 0. || r.item.is_some() {
            return true;
        }
        let dx = t.position[0] - self.position[0];
        let dz = t.position[2] - self.position[2];
        let d = dx.hypot(dz);
        let rise = t.position[1] - self.position[1];
        let front = self.yaw.sin() * dx + self.yaw.cos() * dz >= 0.;
        d >= r.near - t.radius
            && d <= r.far + t.radius
            && rise >= r.low - t.radius
            && rise <= r.high + t.radius
            && front
    }

    /// Partner-in-need (obj+0xf78, FUN_1401b53b0): Sora when his HP is at
    /// most a quarter of the maximum, else the other member under it.
    fn partner_in_need(&self, hp: &[Option<(i32, i32)>; 3]) -> Option<usize> {
        let own = self.slot as usize + 1;
        [0, 3 - own]
            .into_iter()
            .find(|&i| hp[i].is_some_and(|(cur, max)| cur * 4 <= max))
    }

    /// FUN_1401cc1a0: the member Cure heals: the partner-in-need, else
    /// the first of itself, Sora and the other member whose HP is under
    /// `max × ratio` (FUN_1401b3310 truncates the product).
    pub fn heal_target(&self, ratio: f32, hp: &[Option<(i32, i32)>; 3]) -> Option<usize> {
        if let Some(i) = self.partner_in_need(hp) {
            return Some(i);
        }
        let own = self.slot as usize + 1;
        [own, 0, 3 - own]
            .into_iter()
            .find(|&i| hp[i].is_some_and(|(cur, max)| cur < (max as f32 * ratio) as i32))
    }

    /// The spell gate of FUN_1401b2880: the ability held, equipped and not
    /// off; a record with a cost needs the permit and an MP bar that is not
    /// charging; Cure also a member to heal.
    fn spell_ready<W: World>(&self, r: &Record, scene: &Scene<W>) -> bool {
        let Some(spell) = r.spell else { return true };
        // An act needs the member's own script on the engine.
        if spell.act().is_some() && !scene.can_bash {
            return false;
        }
        // Acts without a cost (Tron's swings) are attacks, not spells.
        if (!scene.spells && r.cost > 0) || scene.save.spell_frequency(spell).is_none() {
            return false;
        }
        if let Some((_, _, charging)) = scene.mp
            && r.cost >= 1
            && !(self.permit && !charging)
        {
            return false;
        }
        r.heal_below <= 0. || self.heal_target(r.heal_below, &scene.hp).is_some()
    }

    /// `FUN_1401cc970` / `FUN_1401cce90`: the item an item entry uses and the
    /// party slot it is for. The target is the partner in need (HP only), else
    /// the first of this member, Sora and the other member under 45%; two or
    /// more under it prefer the Mega items. Items come from the member's own
    /// slots, each while its frequency is Often or a partner is in need.
    pub fn item_pick<W: World>(&self, need: ItemNeed, scene: &Scene<W>) -> Option<(u16, u8)> {
        let own = self.slot as usize + 1;
        let low = |i: usize| match need {
            ItemNeed::Hp => {
                scene.hp[i].is_some_and(|(cur, max)| cur < (max as f32 * ITEM_BELOW) as i32)
            }
            // A dead member is out of the MP check.
            ItemNeed::Mp => {
                scene.hp[i].is_some_and(|(cur, _)| cur > 0)
                    && scene.mp_all[i]
                        .is_some_and(|(cur, max)| cur < (max as f32 * ITEM_BELOW) as i32)
            }
        };
        let mut target = match need {
            ItemNeed::Hp => self.partner_in_need(&scene.hp),
            ItemNeed::Mp => None,
        };
        let mut count = 0;
        for i in [own, 0, 3 - own] {
            if low(i) {
                target.get_or_insert(i);
                count += 1;
            }
        }
        let list: &[u16] = match (need, count > 1) {
            (ItemNeed::Hp, false) => &[1, 2, 5, 4, 7],
            (ItemNeed::Hp, true) => &[5, 1, 2, 4, 7],
            (ItemNeed::Mp, false) => &[3, 6, 4, 7],
            (ItemNeed::Mp, true) => &[6, 3, 7, 4],
        };
        let in_need = self.partner_in_need(&scene.hp).is_some();
        let item = list.iter().copied().find(|&id| {
            (scene.save.item_frequency(id) < frequency::RARE || in_need) && scene.save.count(id) > 0
        })?;
        Some((item, target? as u8))
    }

    /// Whether a record can be chosen: its motion exists, the target is in
    /// its window, and a spell or an item has what it needs.
    fn ready<W: World>(&self, r: &Record, t: &Target, scene: &Scene<W>) -> bool {
        (scene.length)(r.motion).is_some()
            && self.eligible(r, t)
            && self.spell_ready(r, scene)
            && r.item
                .is_none_or(|need| self.item_pick(need, scene).is_some())
    }

    /// `FUN_1401b3a30`: with a partner in need the entries flagged for the
    /// HP pass (Cure, the HP item) and then for the MP pass (the MP item) are
    /// tried before any attack: one candidate is picked at random.
    fn emergency<W: World>(
        &mut self,
        records: &[Record],
        t: &Target,
        scene: &Scene<W>,
    ) -> Option<Record> {
        self.partner_in_need(&scene.hp)?;
        for pass in [Urgent::Hp, Urgent::Mp] {
            let candidates: Vec<Record> = records
                .iter()
                .copied()
                .filter(|r| r.urgent == pass && self.ready(r, t, scene))
                .collect();
            if !candidates.is_empty() {
                let pick = candidates[self.rand(candidates.len() as u32) as usize];
                self.history = [255; 3];
                return Some(pick);
            }
        }
        None
    }

    /// FUN_1401b1d40 over the attack records. Pass 0: the random pick among
    /// the eligible ones that are not Rare, a Well-Balanced spell only with
    /// MP for its cost. A pick that would make a fourth repeat of the last
    /// three goes to pass 1, the Rare ones alone; none there, or the repeat
    /// again, and the member follows instead. [I] The repeat test's
    /// obj+0xe08 < 0xe condition is not modelled.
    fn choose<W: World>(
        &mut self,
        records: &[Record],
        t: &Target,
        scene: &Scene<W>,
    ) -> Option<Record> {
        let freq = |r: &Record| {
            r.spell
                .and_then(|s| scene.save.spell_frequency(s))
                .unwrap_or(frequency::FREE)
        };
        let mp = scene.mp.map_or(i32::MAX, |m| m.0);
        let candidates: Vec<Record> = records
            .iter()
            .copied()
            .filter(|r| {
                let f = freq(r);
                f != frequency::RARE
                    && !(f == frequency::WELL_BALANCED && r.cost >= 1 && mp < i32::from(r.cost))
                    && self.ready(r, t, scene)
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let mut pick = candidates[self.rand(candidates.len() as u32) as usize];
        if self.history.iter().all(|&h| h == pick.id) {
            let rare: Vec<Record> = records
                .iter()
                .copied()
                .filter(|r| freq(r) == frequency::RARE && self.ready(r, t, scene))
                .collect();
            if rare.is_empty() {
                return None;
            }
            pick = rare[self.rand(rare.len() as u32) as usize];
            if self.history.iter().all(|&h| h == pick.id) {
                return None;
            }
        }
        self.history = [self.history[1], self.history[2], pick.id];
        Some(pick)
    }

    /// Starts a record's motion toward the target (FUN_1401b7d30 for an
    /// attack, FUN_1403caa90 for a spell act).
    pub fn begin(&mut self, r: &Record, t: &Target, heal: u8) {
        let to = [
            t.position[0] - self.position[0],
            t.position[2] - self.position[2],
        ];
        if to[0].hypot(to[1]) > 1e-3 {
            self.yaw = to[0].atan2(to[1]);
        }
        // An act starts no motion of its own (FUN_1401cc650): its script
        // reads the record's motion (`trap_friend_get_action_param`).
        let motion = match r.spell {
            Some(Spell::Act(_)) => party_motion::IDLE,
            _ => r.motion,
        };
        let mut attack = Attack {
            record: r.id,
            motion,
            tick: 0,
            spell: r.spell,
            heal,
            item: None,
            hold: 0,
        };
        let request = request_of(&attack, r.air);
        attack.hold = Ctl::hold(request.blend);
        self.attack = Some(attack);
        self.request = Some(request);
        self.attack_serial = None;
        if r.air {
            // The jump reaches the record's offset above the target's
            // height (+0x20 + rise, at most the highest jump) and carries the
            // member toward it at 8 a tick.
            let rise = t.position[1] - self.position[1];
            self.air = Some(party_hit::Airborne::launch(
                self.position[1],
                (r.jump + rise).clamp(0., self.personality.jump_max),
                [self.yaw.sin() * AIR_SPEED, self.yaw.cos() * AIR_SPEED],
            ));
        }
        self.gait = Gait::Stand;
        self.tick = 0;
        self.special = Special::new();
    }

    /// `FUN_1401b7d90`: the item-use motion; the member does not turn.
    pub fn begin_item(&mut self, r: &Record, item: (u16, u8)) {
        let mut attack = Attack {
            record: r.id,
            motion: r.motion,
            tick: 0,
            spell: None,
            heal: 0,
            item: Some(item),
            hold: 0,
        };
        let request = request_of(&attack, false);
        attack.hold = Ctl::hold(request.blend);
        self.attack = Some(attack);
        self.request = Some(request);
        self.attack_serial = None;
        self.gait = Gait::Stand;
        self.tick = 0;
        self.special = Special::new();
    }

    /// One tick of Tornado (`L1967`) or Turbo (`L2924`): the member waits for
    /// the motion's range 16, then moves along a heading it turns toward the
    /// goal; Tornado hovers over the target, Turbo dashes along the ground.
    fn special_step<W: World>(&mut self, a: &Attack, spell: Spell, scene: &Scene<W>) {
        let Some((lo, hi)) = (scene.range)(a.motion) else {
            return;
        };
        let sp = &mut self.special;
        let pos = self.position;
        if !sp.started {
            sp.started = true;
            sp.start_y = pos[1];
            if spell == Spell::Turbo {
                sp.heading = [self.yaw.sin(), 0., self.yaw.cos()];
                self.special.goal = self.random_ring(pos);
            }
        }
        let sp = &mut self.special;
        // The script waits for the range before it does anything.
        if a.tick < lo {
            if spell == Spell::Turbo
                && let Some(t) = scene.centre
            {
                // Range 22 turns the member toward the target.
                let d = [t[0] - pos[0], t[2] - pos[2]];
                if d[0].hypot(d[1]) > 1e-3 {
                    self.yaw = d[0].atan2(d[1]);
                }
            }
            return;
        }
        let ground = |x: f32, z: f32, top: f32| scene.world.floor(x, z, top);
        let mut home = true;
        let (goal, speed) = match spell {
            Spell::Tornado => {
                let hover = 65. + 35. * sp.spin.sin();
                match scene.centre {
                    Some(t) => {
                        let near = (t[0] - pos[0]).hypot(t[2] - pos[2]) < 150.;
                        if near {
                            ([t[0], scene.leader_y + hover, t[2]], 3.)
                        } else {
                            let g = ground(t[0], t[2], t[1] + 80.);
                            (
                                match g {
                                    Some(g) => [t[0], g + hover, t[2]],
                                    None => [t[0], t[1] + 80., t[2]],
                                },
                                10.,
                            )
                        }
                    }
                    None => (
                        [
                            pos[0] + self.yaw.sin() * 100.,
                            sp.start_y + hover,
                            pos[2] + self.yaw.cos() * 100.,
                        ],
                        2.,
                    ),
                }
            }
            _ => {
                // Turbo: the goal is the ground under the target; every 30
                // ticks a new search (one enemy here) and a new random point
                // 300-500 away, kept until the target is read
                // (FUN_1403f3fa0, drawn here from the member's own stream).
                if sp.timer >= 30. {
                    sp.count = if sp.count >= 5 { 0 } else { sp.count } + 1;
                    sp.timer = 0.;
                    self.special.goal = self.random_ring(pos);
                    home = false;
                } else if sp.timer >= 4. {
                    if let Some(t) = scene.centre {
                        let g = ground(t[0], t[2], t[1] + 80.);
                        self.special.goal = match g {
                            Some(g) => [t[0], g, t[2]],
                            None => t,
                        };
                    }
                } else {
                    home = false;
                }
                (self.special.goal, 15.)
            }
        };
        let sp = &mut self.special;
        let want = [goal[0] - pos[0], goal[1] - pos[1], goal[2] - pos[2]];
        if home {
            sp.heading = crate::magic::vector_homing(sp.heading, want, 0.174533);
        }
        if spell == Spell::Tornado {
            sp.spin = (sp.spin + 5f32.to_radians()).rem_euclid(std::f32::consts::TAU);
        } else {
            sp.timer += 1.;
        }
        if a.tick <= hi {
            let h = sp.heading;
            let flying = spell == Spell::Tornado;
            let disp = [
                h[0] * speed,
                if flying { h[1] * speed } else { 0. },
                h[2] * speed,
            ];
            let moved = scene
                .world
                .collide(self.position, disp, flying, &scene.body);
            self.position = moved.position;
            if !flying && h[0].hypot(h[2]) > 1e-3 {
                self.yaw = h[0].atan2(h[2]);
            }
        }
    }

    /// FUN_1403f3fa0: a point 300-500 from `pos` at a random bearing.
    fn random_ring(&mut self, pos: Vec3) -> Vec3 {
        let r = 300. + self.rand_unit() * 200.;
        let ang = self.rand_unit() * std::f32::consts::TAU;
        [pos[0] + ang.sin() * r, pos[1], pos[2] + ang.cos() * r]
    }

    fn rand_unit(&mut self) -> f32 {
        self.rand(10_000) as f32 / 10_000.
    }

    /// Whether the member is above the floor (`FUN_1403ba9a0`).
    pub fn airborne<W: World>(&self, world: &W) -> bool {
        world
            .floor(self.position[0], self.position[2], self.position[1])
            .is_some_and(|f| self.position[1] > f + 0.5)
    }

    /// The reaction a hit starts (`FUN_1403d2fe0` through FRIEND vt+0xc8):
    /// the member's attack or walk ends, its motion becomes the flinch or
    /// blow and the knock-slide begins. `has` tells which motions the
    /// member's set holds. Returns the motion that started.
    pub fn react<W: World>(
        &mut self,
        hit: &Hit,
        sstm: &Sstm,
        world: &W,
        has: &dyn Fn(u16) -> bool,
    ) -> Option<u16> {
        let started = party_hit::start(
            hit,
            sstm,
            [self.yaw.sin(), self.yaw.cos()],
            self.position[1],
            self.airborne(world),
            has,
        );
        if let Some(yaw) = started.yaw {
            self.yaw = yaw;
        }
        if started.slide.is_some() {
            self.slide = started.slide;
        }
        let r = started.reaction?;
        self.begin_reaction(r);
        Some(r.motion)
    }

    /// The guard bounce (`FUN_1403d4650` with motion -1).
    pub fn bounce<W: World>(
        &mut self,
        sstm: &Sstm,
        world: &W,
        has: &dyn Fn(u16) -> bool,
    ) -> Option<u16> {
        let r = party_hit::bounce(
            [self.yaw.sin(), self.yaw.cos()],
            self.airborne(world),
            sstm,
            has,
        )?;
        self.begin_reaction(r);
        Some(r.motion)
    }

    fn begin_reaction(&mut self, r: party_hit::Reaction) {
        self.attack = None;
        self.air = None;
        self.hop = None;
        self.wait = None;
        self.special = Special::new();
        self.gait = Gait::Stand;
        self.reaction = Some(r);
        self.tick = 0;
        self.laid = false;
        self.request = Some(reaction_request(&r));
    }

    /// HP reached 0 (`FUN_1403c33d0` through vt+0xb0): the dead bit and the
    /// revive timer (`prty[6]`).
    pub fn ko(&mut self, timer: f32) {
        self.down = Some(timer);
        self.attack = None;
        self.air = None;
        self.special = Special::new();
        self.laid = false;
    }

    /// The timer has run out (`FUN_1403c3660` calls `FUN_1403c3bd0`).
    pub fn revive_due(&self) -> bool {
        self.down.is_some_and(|t| t <= 0.)
    }

    /// A heal on the dead member (`FUN_1403c3c30`): the dead bit clears and
    /// motion 55 plays from 54.
    pub fn revive(&mut self, sstm: &Sstm) {
        if self.down.take().is_some() && self.laid {
            self.begin_reaction(party_hit::get_up(sstm));
        }
        self.laid = false;
    }

    /// One tick: finish or continue an attack, else choose one against the
    /// target, else approach the target or follow the leader; then the
    /// motion controller.
    pub fn tick<W: World>(&mut self, records: &[Record], scene: &Scene<W>) {
        // FUN_1403d68b0: the battle flag puts the member in its battle stance.
        if scene.battle {
            self.battle_stance = true;
        }
        self.step(records, scene);
        self.animate(scene);
    }

    /// The member's state for one tick (the think and the movement).
    fn step<W: World>(&mut self, records: &[Record], scene: &Scene<W>) {
        // The KO timer runs whatever the member is doing (FUN_1403c3660).
        if let Some(t) = self.down.as_mut() {
            *t -= scene.rates.countdown(scene.battle);
        }
        if let Some(k) = self.slide.as_mut() {
            match k.step() {
                Some(d) => {
                    self.position = scene
                        .world
                        .collide(self.position, d, false, &scene.body)
                        .position
                }
                None => self.slide = None,
            }
        }
        // A reaction locks the member until its motion lets it act.
        if let Some(mut r) = self.reaction {
            let env = party_hit::Env {
                world: scene.world,
                body: scene.body,
                length: scene.length,
                actionable: scene.actionable,
            };
            let before = r.motion;
            let (position, done) = r.step(self.position, &env);
            self.position = position;
            self.tick = r.tick;
            // A blow or an air flinch reaching the floor: its land motion.
            if r.motion != before && !done {
                self.request = Some(reaction_request(&r));
            }
            self.reaction = (!done).then_some(r);
            if done {
                self.tick = 0;
            }
            return;
        }
        // An idle hop carries the member until it lands.
        if self.hop.is_some() {
            self.hop_step(scene);
            return;
        }
        // An air attack's jump: the mover runs whatever the motion does, and
        // the floor ends the attack with the land motion.
        if let Some(mut air) = self.air {
            let (position, landed) =
                air.step(self.position, scene.sstm.fall_max, scene.world, &scene.body);
            self.position = position;
            if landed {
                self.air = None;
                self.attack = None;
                if (scene.length)(party_hit::motion::LAND).is_some() {
                    self.begin_reaction(party_hit::landing(&scene.sstm));
                }
                return;
            }
            self.air = Some(air);
            if self.attack.is_none() {
                self.tick += 1;
                return;
            }
        }
        // An attack plays out its motion with root motion, then ends.
        if let Some(mut a) = self.attack {
            // With a controller the attack's tick is its clock (`animate`).
            let driven = scene.clips.is_some();
            if !driven {
                a.tick += 1;
            }
            if a.spell.is_some_and(|s| s.act().is_some()) {
                // The act's script plays the motions and carries the member.
                if let Some(s) = scene.scripted {
                    // The engine's queue, as it stands now: an act often
                    // queues its last motion's follower as it ends.
                    if self.act_serial == Some(s.serial) && self.ctl.motion == s.motion {
                        self.ctl.set_queue(s.queue.into_iter().flatten());
                    }
                    self.position = s.position;
                    self.yaw = s.yaw;
                    a.motion = s.motion;
                    a.tick = s.tick;
                    self.tick = s.tick;
                    self.attack = (!s.done).then_some(a);
                    if s.done {
                        self.tick = 0;
                        if self.act_serial != Some(s.serial) {
                            self.act_tail = Some(s);
                        }
                    }
                } else {
                    self.attack = Some(Attack { tick: 0, ..a });
                }
                return;
            }
            if self.air.is_some() {
                // The jump carries the member; the motion's root does not.
            } else if let Some(spell @ (Spell::Tornado | Spell::Turbo)) = a.spell {
                // The script moves the member; the motion's root does not.
                self.special_step(&a, spell, scene);
            } else if !driven {
                self.root_motion(
                    a.motion,
                    a.tick.saturating_sub(1) as f32,
                    a.tick as f32,
                    scene,
                );
            }
            self.tick = a.tick;
            if driven {
                // The controller ends it: the queued motion takes over.
                self.attack = Some(a);
                return;
            }
            let end = (scene.length)(a.motion).unwrap_or(0);
            self.attack = (a.tick < end).then_some(a);
            if self.attack.is_none() {
                self.tick = 0;
            }
            return;
        }
        // After a flying special the member falls to the floor (motion 4).
        if let Some(floor) = scene
            .world
            .floor(self.position[0], self.position[2], self.position[1])
            && self.position[1] > floor + 0.01
        {
            self.fall = (self.fall + 0.408).min(40.);
            let moved = scene
                .world
                .collide(self.position, [0., -self.fall, 0.], true, &scene.body);
            self.position = moved.position;
            return;
        }
        self.fall = 0.;
        // Down: the DEAD motion starts once the member is on the ground
        // and can act (FUN_1403c3660), and holds until the revive.
        if self.down.is_some() {
            self.attack = None;
            if self.laid {
                self.tick += 1;
            } else {
                self.laid = true;
                self.tick = 0;
            }
            return;
        }
        // A running spell act holds the member where it stands; the think's
        // wish is empty (FUN_1401b15f0 clears it each tick).
        if scene.busy {
            self.gait = Gait::Stand;
            self.tick += 1;
            self.speed = self
                .wish
                .smooth(
                    [self.yaw.sin(), 0., self.yaw.cos()],
                    0.,
                    WALK_SPEED,
                    RUN_SPEED,
                    false,
                )
                .1;
            return;
        }
        // The think loop's spell permit: MP share scaled (FUN_1401b53b0).
        if let Some((cur, max, _)) = scene.mp {
            let p = (cur as f32 / max.max(1) as f32) * 0.75 + 0.25;
            self.permit = self.rand(20001) as f32 / 20000. <= p;
        }
        // A wait command ends when an action becomes possible
        // (`FUN_1401b2b20` in its check) or its timer runs out.
        if let (Some(_), Some(t)) = (self.wait, scene.target)
            && records.iter().any(|r| self.ready(r, &t, scene))
        {
            self.wait = None;
        }
        // FUN_1401b1d40: after an action, a hop with the personality's
        // chance (rows of heading offset, height, speed, inertia, rise time).
        if self.wait.is_none()
            && self.last_action
            && scene.target.is_some()
            && !self.personality.hops.is_empty()
            && scene.clips.is_none_or(|c| self.ctl.range(0, c))
        {
            let roll = self.rand(20001) as f32 / 20000.;
            if roll < self.personality.hop_chance {
                let rows = self.personality.hops;
                let row = rows[self.rand(rows.len() as u32) as usize];
                self.start_hop(row);
                return;
            }
        }
        // Decide: an eligible attack starts at once (FUN_1401b1d40).
        if self.wait.is_none()
            && let Some(t) = scene.target
            && let Some(r) = self
                .emergency(records, &t, scene)
                .or_else(|| self.choose(records, &t, scene))
            && (scene.length)(r.motion).is_some()
        {
            // Entries with flag 2 are actions; the item entries are not.
            // Both have flag bit 0 clear.
            self.last_action = r.item.is_none();
            self.acted = true;
            if let Some(need) = r.item
                && let Some(item) = self.item_pick(need, scene)
            {
                self.begin_item(&r, item);
                return;
            }
            let heal = if r.heal_below > 0. {
                self.heal_target(r.heal_below, &scene.hp).unwrap_or(0) as u8
            } else {
                0
            };
            self.begin(&r, &t, heal);
            return;
        }
        // Nothing to do against the enemy: one of the two wait commands, at
        // random, for the personality's time (`FUN_14019bf50`).
        if scene.target.is_some() && self.wait.is_none() {
            let k = self.rand(2) as usize;
            let [lo, hi] = self.personality.waits[k];
            let time = lo + self.rand_unit() * (hi - lo);
            self.wait = Some(if k == 0 {
                Wait::Hold(time)
            } else {
                Wait::Follow(time)
            });
            self.last_action = false;
            self.acted = false;
        }
        // Nothing chosen: the entry is cleared (FUN_1401b1d40).
        if scene.target.is_none() {
            self.wait = None;
            self.acted = false;
        }
        let holding = matches!(self.wait, Some(Wait::Hold(_)));
        if let Some(Wait::Hold(t) | Wait::Follow(t)) = self.wait.as_mut() {
            *t -= 1.;
            if *t <= 0. {
                self.wait = None;
            }
        }
        // Movement (FUN_1401b41a0): toward the target in a fight, else
        // behind the leader. Holding, the member stands and faces the enemy
        // (`FUN_1401ccba0`: `FUN_1403b65c0`, a wish of no speed). The follow state steps by the personality's
        // distances; the wish is smoothed and snapped (FUN_1401b1b20) and
        // carries the member along it at that speed (FUN_140422350) while
        // the facing turns toward it (FUN_1403b4780). Party Attack scales
        // the formation distance and the distances toward the leader.
        let scale = match scene.target {
            Some(_) => 1.,
            None => Self::party_attack(scene),
        };
        let (goal, kind) = match scene.target {
            Some(t) => (t.position, Goal::Target),
            None => (
                self.goal_at(scene.leader, scene.leader_yaw, FORMATION * scale),
                if scene.battle {
                    Goal::Leader
                } else {
                    Goal::Field
                },
            ),
        };
        let d = [goal[0] - self.position[0], goal[2] - self.position[2]];
        let dist = d[0].hypot(d[1]);
        self.state = self.personality.follow.next(self.state, dist / scale, kind);
        let want = match self.state {
            _ if holding => 0.,
            1 => WALK_SPEED,
            2 => RUN_SPEED,
            _ => 0.,
        };
        if holding && dist > 1e-3 {
            let delta = wrap(d[0].atan2(d[1]) - self.yaw);
            self.yaw = wrap(self.yaw + delta.clamp(-TURN, TURN));
        }
        let dir = if dist > 1e-3 {
            [d[0] / dist, 0., d[1] / dist]
        } else {
            [self.yaw.sin(), 0., self.yaw.cos()]
        };
        let (wish, speed) = self.wish.smooth(dir, want, WALK_SPEED, RUN_SPEED, false);
        self.speed = speed;
        let before = self.gait;
        self.gait = match party_motion::ground_motion(speed, WALK_SPEED, RUN_SPEED) {
            party_motion::IDLE => Gait::Stand,
            party_motion::WALK => Gait::Walk,
            _ => Gait::Run,
        };
        if self.gait != before {
            self.tick = 0;
        } else {
            self.tick += 1;
        }
        let len = wish[0].hypot(wish[2]);
        if speed > 0. && len > 1e-6 {
            let (x, z) = (wish[0] / len, wish[2] / len);
            let delta = wrap(x.atan2(z) - self.yaw);
            self.yaw = wrap(self.yaw + delta.clamp(-TURN, TURN));
            let disp = [x * speed, 0., z * speed];
            let moved = scene.world.collide(self.position, disp, false, &scene.body);
            self.position = moved.position;
        }
    }

    /// FUN_1403cbec0 with a hop row: the jump motion starts (fall 4 and land
    /// 5 behind it) and the hop leaves the ground at its take-off; height
    /// `row[1]` reached in `row[4]` ticks (g = 2h/t²), `row[2]` a tick along
    /// the facing turned by `row[0]` degrees, `row[3]` of it kept a step.
    fn start_hop(&mut self, row: [f32; 5]) {
        let heading = row[0].to_radians() + self.yaw;
        let gravity = (row[1] + row[1]) / (row[4] * row[4]);
        self.hop = Some(Hop {
            armed: true,
            velocity: [heading.sin() * row[2], heading.cos() * row[2]],
            vy_down: -(gravity * row[4]),
            gravity,
            inertia: row[3],
            sampler: Sampler::default(),
            fell: false,
        });
        self.request = Some(Request::start(3, 0., None));
        self.gait = Gait::Stand;
    }

    /// One tick of the hop (FUN_1403cbdd0 in mode 2): it waits for the jump
    /// motion's take-off, then the velocity keeps its inertia, gravity pulls
    /// and the fall motion starts five ticks before the apex; the floor ends
    /// it with the land motion.
    fn hop_step<W: World>(&mut self, scene: &Scene<W>) {
        let Some(mut hop) = self.hop else { return };
        if hop.armed {
            let takeoff = scene
                .clips
                .and_then(|c| {
                    let clip = c.clip(3, !self.battle_stance)?;
                    c.trigger(clip.slot, 0)
                })
                .unwrap_or(0);
            if self.ctl.motion == 3 && self.ctl.frame() < takeoff as f32 {
                return;
            }
            hop.armed = false;
        }
        let dt = actor_physics::PHYSICS_STEP;
        if hop.sampler.due(0.) {
            hop.velocity =
                actor_physics::steer(hop.velocity, [0., 0.], 0., hop.inertia, hop.inertia);
            hop.vy_down += dt * hop.gravity;
            hop.sampler.store([
                hop.velocity[0] * dt,
                -hop.vy_down * dt,
                hop.velocity[1] * dt,
            ]);
        }
        let moved =
            scene
                .world
                .collide(self.position, hop.sampler.displacement(), true, &scene.body);
        self.position = moved.position;
        if !hop.fell && hop.vy_down + hop.gravity * actor_physics::FALL_MOTION_LEAD >= 0. {
            hop.fell = true;
            self.request = Some(Request {
                change: true,
                ..Request::start(FALL, BLEND, None)
            });
        }
        if moved.on_floor && hop.vy_down > 0. {
            self.hop = None;
            if (scene.length)(party_hit::motion::LAND).is_some() {
                self.begin_reaction(party_hit::landing(&scene.sstm));
            }
            return;
        }
        self.hop = Some(hop);
    }

    /// The root translation of `motion` between two clock values, turned
    /// by the facing and moved through the map.
    fn root_motion<W: World>(&mut self, motion: u16, from: f32, to: f32, scene: &Scene<W>) {
        let (s, c) = self.yaw.sin_cos();
        let m = (scene.root)(motion, to.max(0.) as u32);
        let p = (scene.root)(motion, from.max(0.) as u32);
        let (dx, dz) = (m[0] - p[0], m[2] - p[2]);
        if dx == 0. && dz == 0. {
            return;
        }
        let disp = [dx * c + dz * s, 0., -dx * s + dz * c];
        let moved = scene.world.collide(self.position, disp, false, &scene.body);
        self.position = moved.position;
    }

    /// The motion controller's tick (obj+0x158): the motion this tick's
    /// state asked for, or the ground mode's (FUN_140422520: idle, walk or
    /// run by the speed, blended over 8, while range 0 is open, or range 2
    /// when moving), then one advance (FUN_1403c6740). The attack's tick and
    /// root motion follow the clock, and its end is the controller's: the
    /// motion queued behind it starts.
    fn animate<W: World>(&mut self, scene: &Scene<W>) {
        let Some(clips) = scene.clips else {
            return;
        };
        let field = !self.battle_stance;
        let on_ground = !self.airborne(scene.world);
        let tail = self.act_tail.take();
        if let Some(r) = self.request.take() {
            if r.change {
                self.ctl.change(r.motion, r.blend, field, clips);
            } else {
                self.ctl.start(r.motion, r.blend, field, clips);
            }
            if let Some((m, b)) = r.then {
                self.ctl.queue(m, b);
            }
            if self.attack.is_some() {
                self.attack_serial = Some(self.ctl.serial);
            }
        } else if let Some(a) = self.attack
            && a.spell.is_some_and(|s| s.act().is_some())
            && let Some(s) = scene.scripted
            && self.act_serial != Some(s.serial)
        {
            // An act on the engine: each motion it starts starts here with
            // the blend the script asked for (the Bash's `motion_start(193,
            // 8)`, p_ex030 L3491, ...).
            self.act_serial = Some(s.serial);
            self.ctl.start(s.motion, s.blend, field, clips);
            for (m, b) in s.queue.into_iter().flatten() {
                self.ctl.queue(m, b);
            }
            self.attack_serial = Some(self.ctl.serial);
        } else if let Some(s) = tail {
            // The act ended on the tick it started its last motion: that
            // motion and its queue still play.
            self.act_serial = Some(s.serial);
            self.ctl.start(s.motion, s.blend, field, clips);
            for (m, b) in s.queue.into_iter().flatten() {
                self.ctl.queue(m, b);
            }
        } else if self.attack.is_none()
            && self.reaction.is_none()
            && self.air.is_none()
            && self.ctl.motion == FALL
            && on_ground
        {
            // FUN_1403cb940: on the floor the fall ends in the land motion
            // with idle queued (idle at once without one).
            if self.ctl.start(party_hit::motion::LAND, 0., field, clips) {
                self.ctl.queue(party_motion::IDLE, 0.);
            } else {
                self.ctl.start(party_motion::IDLE, 0., field, clips);
            }
        } else if self.laid {
            // FUN_1403c3660: the DEAD motion, asked for every tick.
            self.ctl.change(party_hit::motion::DEAD, 0., field, clips);
        } else if self.attack.is_none()
            && self.reaction.is_none()
            && self.air.is_none()
            && self.down.is_none()
            && on_ground
        {
            let m = party_motion::ground_motion(self.speed, WALK_SPEED, RUN_SPEED);
            if self.ctl.range(0, clips) || (self.ctl.range(2, clips) && m != party_motion::IDLE) {
                self.ctl.change(m, BLEND, field, clips);
            }
        }
        let step = self.ctl.advance(field, clips);
        if step.battle_started {
            self.battle_stance = true;
        }
        // FUN_1403ba350: a motion's end returns the member to its field
        // stance once the battle flag is down and it stands on the ground.
        if step.stance_check && !scene.battle && on_ground {
            self.battle_stance = false;
        }
        let Some(mut a) = self.attack else {
            return;
        };
        if a.spell.is_some_and(|s| s.act().is_some()) {
            return;
        }
        if self.attack_serial != Some(self.ctl.serial) {
            // The motion queued behind the attack (or another) took over.
            self.attack = None;
            self.attack_serial = None;
            self.tick = 0;
            return;
        }
        if self.air.is_none() && !matches!(a.spell, Some(Spell::Tornado | Spell::Turbo)) {
            for &(m, _, from, to) in &step.spans {
                if m == a.motion {
                    self.root_motion(m, from, to, scene);
                }
            }
        }
        a.tick = self.ctl.frame().max(0.) as u32;
        self.tick = a.tick;
        self.attack = Some(a);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Collided;

    struct Flat;
    impl World for Flat {
        fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
            Some(0.)
        }
        fn resolve_walls(&self, p: [f32; 2], _: f32, _: f32, _: f32) -> [f32; 2] {
            p
        }
        fn collide(&self, start: Vec3, d: Vec3, _: bool, _: &Body) -> Collided {
            Collided {
                position: [start[0] + d[0], 0., start[2] + d[2]],
                on_floor: true,
            }
        }
    }
    const BODY: Body = Body {
        radius: 30.,
        height: 140.,
        step: 24.,
    };
    fn length(m: u16) -> Option<u32> {
        matches!(m, 151 | 152).then_some(60)
    }
    fn no_root(_: u16, _: u32) -> Vec3 {
        [0.; 3]
    }
    fn no_actionable(_: u16) -> Option<u32> {
        None
    }
    /// Every spell equipped, no items.
    const EVERY: Save = {
        let mut s = Save::EMPTY;
        let ids = [0xa5, 0xa6, 0xa7, 0xa8, 0x1a7, 0x1ad, 0x1a9];
        let mut k = 0;
        while k < ids.len() {
            s.abilities[k] = 0x8000 | ids[k];
            k += 1;
        }
        s
    };

    /// [`EVERY`] with these items in its slots.
    const fn with_items(items: [u16; 5]) -> Save {
        let mut s = EVERY;
        let mut k = 0;
        while k < 5 {
            s.items[k] = items[k];
            k += 1;
        }
        s.item_slots = 5;
        s
    }
    pub(super) fn test_sstm() -> Sstm {
        let mut record = [0u8; 0x3c];
        for (at, v) in [
            (0x14usize, 16f32),
            (0x18, 0.8),
            (0x1c, 0.1),
            (0x20, 0.8),
            (0x24, 24.),
            (0x28, 40.),
            (0x2c, 8.),
            (0x30, 200.),
            (0x34, 200.),
            (0x38, 0.048),
        ] {
            record[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        Sstm::read(&record).unwrap()
    }
    pub(super) fn test_rates() -> party_hit::Rates {
        party_hit::Rates {
            revive_keep: 0.5,
            revive_hp: 0.5,
            ko_speed: 8.,
        }
    }
    fn no_range(m: u16) -> Option<(u32, u32)> {
        match m {
            191 => Some((50, 380)),
            194 => Some((92, 330)),
            _ => None,
        }
    }
    fn scene(leader: Vec3, target: Option<Target>, world: &Flat) -> Scene<'_, Flat> {
        Scene {
            leader,
            leader_yaw: 0.,
            target,
            world,
            body: BODY,
            length: &length,
            root: &no_root,
            busy: false,
            spells: false,
            centre: None,
            leader_y: 0.,
            range: &no_range,
            mp: None,
            hp: [None; 3],
            mp_all: [None; 3],
            save: &EVERY,
            actionable: &no_actionable,
            sstm: test_sstm(),
            rates: test_rates(),
            battle: true,
            scripted: None,
            can_bash: true,
            clips: None,
        }
    }

    #[test]
    fn a_distant_member_walks_then_runs_and_settles_at_the_goal() {
        let mut f = Follower::spawn([0., 0., 0.], 0., 0);
        let leader = [0., 0., 600.];
        let mut gaits = Vec::new();
        for _ in 0..400 {
            f.tick(&DONALD, &scene(leader, None, &Flat));
            if gaits.last() != Some(&f.gait) {
                gaits.push(f.gait);
            }
        }
        assert_eq!(gaits, vec![Gait::Walk, Gait::Run, Gait::Walk, Gait::Stand]);
        let goal = f.goal(leader, 0.);
        let d = (goal[0] - f.position[0]).hypot(goal[2] - f.position[2]);
        assert!(d < 200., "{d}");
    }

    #[test]
    fn the_two_slots_stand_on_either_side_behind_the_leader() {
        let a = Follower::spawn([0.; 3], 0., 0).goal([0.; 3], 0.);
        let b = Follower::spawn([0.; 3], 0., 1).goal([0.; 3], 0.);
        assert!(a[0] > 0. && b[0] < 0. || a[0] < 0. && b[0] > 0.);
        assert!(a[2] < 0. && b[2] < 0.);
    }

    #[test]
    fn a_member_closes_in_on_the_enemy_and_attacks_in_range() {
        let mut f = Follower::spawn([0.; 3], 0., 0);
        let t = Target {
            position: [0., 0., 500.],
            radius: 40.,
        };
        let mut started = None;
        for tick in 0..200 {
            f.tick(&DONALD, &scene([0.; 3], Some(t), &Flat));
            if f.attack.is_some() && started.is_none() {
                started = Some((tick, f.position[2]));
            }
        }
        let (_, z) = started.expect("an attack starts");
        // Within the longest record's reach plus the target's radius.
        assert!(500. - z <= 200. + 40. + 1., "{z}");
    }

    #[test]
    fn attacks_play_to_their_end_and_never_repeat_three_times() {
        // No hops: they would carry the member out of one record's reach.
        let mut f = Follower::spawn([0., 0., 400.], 0., 0).with_personality(Personality {
            hop_chance: 0.,
            ..DONALD_PERSONALITY
        });
        let t = Target {
            position: [0., 0., 480.],
            radius: 40.,
        };
        let mut chosen = Vec::new();
        for _ in 0..3000 {
            let before = f.attack;
            f.tick(&DONALD, &scene([0.; 3], Some(t), &Flat));
            if before.is_none()
                && let Some(a) = f.attack
            {
                chosen.push(a.record);
            }
        }
        assert!(chosen.len() > 10, "{}", chosen.len());
        assert!(
            chosen
                .windows(4)
                .all(|w| !(w[0] == w[1] && w[1] == w[2] && w[2] == w[3]))
        );
        assert!(chosen.contains(&0) && chosen.contains(&1));
    }

    fn spell_length(m: u16) -> Option<u32> {
        matches!(m, 151 | 152 | 191 | 192).then_some(60)
    }

    fn spell_scene<'a>(
        t: Option<Target>,
        world: &'a Flat,
        mp: (i32, i32, bool),
        hp: [Option<(i32, i32)>; 3],
    ) -> Scene<'a, Flat> {
        Scene {
            leader: [0.; 3],
            leader_yaw: 0.,
            target: t,
            world,
            body: BODY,
            length: &spell_length,
            root: &no_root,
            busy: false,
            spells: true,
            centre: t.map(|t| t.position),
            leader_y: 0.,
            range: &no_range,
            mp: Some(mp),
            hp,
            mp_all: [None; 3],
            save: &EVERY,
            actionable: &no_actionable,
            sstm: test_sstm(),
            rates: test_rates(),
            battle: true,
            scripted: None,
            can_bash: true,
            clips: None,
        }
    }

    #[test]
    fn spells_need_the_permit_and_an_mp_bar_that_is_not_charging() {
        let t = Target {
            position: [0., 0., 300.],
            radius: 40.,
        };
        // Full MP: the permit odds are 1; the member casts something.
        let mut f = Follower::spawn([0., 0., 100.], 0., 0);
        let mut spells = 0;
        for _ in 0..4000 {
            f.tick(
                &DONALD,
                &spell_scene(Some(t), &Flat, (100, 100, false), [Some((20, 20)); 3]),
            );
            if f.attack.is_some_and(|a| a.spell.is_some() && a.tick == 0) {
                spells += 1;
            }
        }
        assert!(spells > 5, "{spells}");
        // An MP Charge running: no spell is ever chosen.
        let mut f = Follower::spawn([0., 0., 100.], 0., 0);
        for _ in 0..4000 {
            f.tick(
                &DONALD,
                &spell_scene(Some(t), &Flat, (0, 100, true), [Some((20, 20)); 3]),
            );
            assert!(f.attack.is_none_or(|a| a.spell.is_none()));
        }
    }

    #[test]
    fn blizzard_waits_for_distance_and_fire_for_the_enemy_to_be_near() {
        // At 600 only Blizzard (250-750) is in range.
        let mut f = Follower::spawn([0., 0., 0.], 0., 0);
        f.permit = true;
        let t = Target {
            position: [0., 0., 600.],
            radius: 40.,
        };
        let scene = spell_scene(Some(t), &Flat, (100, 100, false), [Some((20, 20)); 3]);
        let ready: Vec<u8> = DONALD
            .iter()
            .filter(|r| r.item.is_none() && f.eligible(r, &t) && f.spell_ready(r, &scene))
            .map(|r| r.id)
            .collect();
        assert_eq!(ready, vec![58]);
    }

    /// Donald's new-game record (plrp row 1): Potion and Ether, Thunder and
    /// Cure equipped.
    fn donald_new_game() -> Save {
        Save::new_game(2, &[0x8001, 0x8003, 0x80a7, 0x80a8], |id| id <= 7)
    }

    #[test]
    fn frequencies_are_indexed_by_item_then_by_ability() {
        assert_eq!(frequency_index(1), Some(0));
        assert_eq!(frequency_index(7), Some(6));
        assert_eq!(frequency_index(0xa7), Some(9));
        assert_eq!(frequency_index(0x1ad), Some(8));
        assert_eq!(frequency_index(8), None);
    }

    #[test]
    fn a_new_game_record_fills_items_and_equipped_abilities() {
        let s = donald_new_game();
        assert_eq!((s.count(1), s.count(3), s.count(2)), (1, 1, 0));
        assert_eq!(s.spell_frequency(Spell::Thunder), Some(frequency::FREE));
        assert_eq!(s.spell_frequency(Spell::Fire), None);
        let mut s = s;
        assert!(s.take(1) && !s.take(1));
        // Held but not equipped, or equipped but off: refused.
        let mut held = Save::EMPTY;
        held.abilities[0] = 0xa6;
        assert_eq!(held.spell_frequency(Spell::Blizzard), None);
        let mut off = EVERY;
        off.frequency[ability_of(Spell::Blizzard).unwrap().1] = frequency::OFF;
        assert_eq!(off.spell_frequency(Spell::Blizzard), None);
    }

    #[test]
    fn the_frequencies_decide_when_a_spell_is_chosen() {
        // At 600 only Blizzard (250-750) is in range.
        let t = Target {
            position: [0., 0., 600.],
            radius: 40.,
        };
        let pick = |save: &Save, mp: i32, history: [u8; 3]| {
            let mut f = Follower::spawn([0., 0., 0.], 0., 0);
            f.permit = true;
            f.history = history;
            let mut scene = spell_scene(Some(t), &Flat, (mp, 100, false), [Some((20, 20)); 3]);
            scene.save = save;
            f.choose(&DONALD, &t, &scene).map(|r| r.id)
        };
        let mut s = EVERY;
        let k = ability_of(Spell::Blizzard).unwrap().1;
        assert_eq!(pick(&s, 100, [255; 3]), Some(58));
        // A fourth Blizzard in a row is not taken while no Rare entry stands in.
        assert_eq!(pick(&s, 100, [58; 3]), None);
        // Not equipped: never.
        assert_eq!(pick(&donald_new_game(), 100, [255; 3]), None);
        // Well-Balanced needs the MP for its cost.
        s.frequency[k] = frequency::WELL_BALANCED;
        assert_eq!(pick(&s, 100, [255; 3]), Some(58));
        assert_eq!(pick(&s, 1, [255; 3]), None);
        // Rare: only in place of a fourth repeat.
        s.frequency[k] = frequency::RARE;
        assert_eq!(pick(&s, 100, [255; 3]), None);
    }

    #[test]
    fn cure_heals_the_partner_in_need_then_itself_then_sora_then_the_other_member() {
        let f = Follower::spawn([0.; 3], 0., 0); // Donald: party slot 1
        // Sora at a quarter of his HP is the partner in need, whoever else
        // is hurt.
        assert_eq!(
            f.heal_target(0.4, &[Some((5, 20)), Some((1, 20)), Some((1, 20))]),
            Some(0)
        );
        // Else Donald himself under 0.4 (8 of 20).
        assert_eq!(
            f.heal_target(0.4, &[Some((20, 20)), Some((7, 20)), Some((20, 20))]),
            Some(1)
        );
        // Goofy at a quarter is the partner in need before Donald's 0.4.
        assert_eq!(
            f.heal_target(0.4, &[Some((20, 20)), Some((7, 20)), Some((5, 20))]),
            Some(2)
        );
        // Nobody under 0.4: no heal.
        assert_eq!(
            f.heal_target(0.4, &[Some((20, 20)), Some((20, 20)), Some((20, 20))]),
            None
        );
        // Sora alone under 0.4 but above a quarter (7 of 20).
        assert_eq!(
            f.heal_target(0.4, &[Some((7, 20)), Some((20, 20)), Some((20, 20))]),
            Some(0)
        );
    }

    #[test]
    fn turbo_waits_for_its_range_then_dashes_fifteen_a_tick_at_the_target() {
        let mut f = Follower::spawn([0.; 3], 0., 1);
        let t = Target {
            position: [0., 0., 6000.],
            radius: 40.,
        };
        f.begin(&GOOFY[6], &t, 0);
        let scene = |t: Target| Scene {
            centre: Some(t.position),
            length: &|m| matches!(m, 194).then_some(400),
            ..spell_scene(Some(t), &Flat, (100, 100, false), [None; 3])
        };
        let mut z_at = Vec::new();
        for _ in 0..400 {
            f.tick(&GOOFY, &scene(t));
            z_at.push(f.position[2]);
        }
        // Motionless before the range opens (attack tick 92 is index 91).
        assert!(z_at[..91].iter().all(|&z| z == 0.), "{}", z_at[90]);
        // Fifteen a tick inside it, straight along the target's bearing.
        assert!((z_at[92] - z_at[91] - 15.).abs() < 1e-3);
        // And it stops when the range ends (attack tick 330).
        assert_eq!(z_at[340], z_at[345]);
        assert!(z_at[345] > 3000.);
    }

    fn all_motions(_: u16) -> Option<u32> {
        Some(70)
    }
    fn ready_at_58(_: u16) -> Option<u32> {
        Some(58)
    }
    fn reaction_scene<'a>(
        world: &'a Flat,
        target: Option<Target>,
        battle: bool,
    ) -> Scene<'a, Flat> {
        Scene {
            leader: [0., 0., 0.],
            leader_yaw: 0.,
            target,
            world,
            body: BODY,
            length: &all_motions,
            root: &no_root,
            busy: false,
            spells: false,
            centre: None,
            leader_y: 0.,
            range: &no_range,
            mp: None,
            hp: [None; 3],
            mp_all: [None; 3],
            save: &EVERY,
            actionable: &ready_at_58,
            sstm: test_sstm(),
            rates: test_rates(),
            battle,
            scripted: None,
            can_bash: true,
            clips: None,
        }
    }
    fn claw_hit() -> Hit {
        Hit {
            reaction: 3,
            original: 3,
            direction: [0., -1.],
            high: true,
            knockback: [0x7fff, 0],
        }
    }

    #[test]
    fn a_hit_ends_the_attack_and_locks_the_member_until_the_reaction_lets_it_act() {
        let t = Target {
            position: [0., 0., 100.],
            radius: 40.,
        };
        let mut f = Follower::spawn([0., 0., 0.], 0., 0);
        f.begin(&DONALD[0], &t, 0);
        assert!(f.attack.is_some());
        let sstm = test_sstm();
        let motion = f.react(&claw_hit(), &sstm, &Flat, &|m| m < 100);
        assert_eq!(motion, Some(10));
        assert!(f.attack.is_none());
        assert_eq!(f.motion(), 10);
        // An enemy stands in range, yet nothing is chosen while locked.
        for _ in 0..57 {
            f.tick(&DONALD, &reaction_scene(&Flat, Some(t), true));
            assert_eq!(f.motion(), 10);
            assert!(f.attack.is_none());
        }
        f.tick(&DONALD, &reaction_scene(&Flat, Some(t), true));
        assert!(f.reaction.is_none());
        // The slide carried the member along the hit, toward -z.
        assert!(f.position[2] < -30.);
        // Free again: the next tick decides.
        f.tick(&DONALD, &reaction_scene(&Flat, Some(t), true));
        assert!(f.attack.is_some() || f.gait != Gait::Stand);
    }

    #[test]
    fn a_ko_member_plays_its_reaction_then_lies_down_until_the_timer_and_a_heal() {
        let mut f = Follower::spawn([0., 0., 0.], 0., 0);
        let sstm = test_sstm();
        f.ko(3600.);
        f.react(&claw_hit(), &sstm, &Flat, &|m| m < 100);
        // The killing blow's reaction plays out first.
        for _ in 0..40 {
            f.tick(&DONALD, &reaction_scene(&Flat, None, false));
            assert_eq!(f.motion(), 10);
        }
        for _ in 0..40 {
            f.tick(&DONALD, &reaction_scene(&Flat, None, false));
        }
        assert_eq!(f.motion(), party_hit::motion::DEAD);
        // Outside battle the timer falls 8 a tick: 3600 lasts 450 ticks
        // from the KO.
        let mut ticks = 80;
        while !f.revive_due() {
            f.tick(&DONALD, &reaction_scene(&Flat, None, false));
            ticks += 1;
            assert!(ticks < 1000);
        }
        assert_eq!(ticks, 450);
        assert_eq!(f.motion(), party_hit::motion::DEAD);
        // The heal gets it up with motion 55, then it acts again.
        f.revive(&sstm);
        assert!(f.down.is_none());
        assert_eq!(f.motion(), party_hit::motion::REVIVE);
        for _ in 0..58 {
            f.tick(&DONALD, &reaction_scene(&Flat, None, false));
        }
        assert!(f.reaction.is_none());
        assert_eq!(f.motion(), 0);
    }

    #[test]
    fn the_ko_timer_runs_at_one_a_tick_in_battle() {
        let mut f = Follower::spawn([0., 0., 0.], 0., 0);
        f.ko(100.);
        for _ in 0..99 {
            f.tick(&DONALD, &reaction_scene(&Flat, None, true));
        }
        assert!(!f.revive_due());
        f.tick(&DONALD, &reaction_scene(&Flat, None, true));
        assert!(f.revive_due());
    }

    #[test]
    fn a_guard_bounce_slides_the_member_back_and_returns_it_to_the_ai() {
        let mut f = Follower::spawn([0., 0., 0.], 0., 0);
        let sstm = test_sstm();
        assert_eq!(f.bounce(&sstm, &Flat, &|_| true), Some(41));
        assert_eq!(f.motion(), 41);
        for _ in 0..70 {
            f.tick(&DONALD, &reaction_scene(&Flat, None, true));
        }
        assert!(f.reaction.is_none());
        // Facing +z, so the slide went toward -z.
        assert!(f.position[2] < -30.);
    }

    /// A flat floor at 0 that stops a fall.
    struct Ground;
    impl World for Ground {
        fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
            Some(0.)
        }
        fn resolve_walls(&self, p: [f32; 2], _: f32, _: f32, _: f32) -> [f32; 2] {
            p
        }
        fn collide(&self, start: Vec3, d: Vec3, _: bool, _: &Body) -> Collided {
            let y = start[1] + d[1];
            Collided {
                position: [start[0] + d[0], y.max(0.), start[2] + d[2]],
                on_floor: y <= 0. && d[1] <= 0.,
            }
        }
    }

    /// The air attack's motion ends well before the jump does.
    fn air_length(m: u16) -> Option<u32> {
        Some(if m == 181 { 20 } else { 70 })
    }

    fn air_scene<'a>(world: &'a Ground, target: Option<Target>) -> Scene<'a, Ground> {
        Scene {
            leader: [0.; 3],
            leader_yaw: 0.,
            target,
            world,
            body: BODY,
            length: &air_length,
            root: &no_root,
            busy: false,
            spells: false,
            centre: None,
            leader_y: 0.,
            range: &no_range,
            mp: None,
            hp: [None; 3],
            mp_all: [None; 3],
            save: &EVERY,
            actionable: &ready_at_58,
            sstm: test_sstm(),
            rates: test_rates(),
            battle: true,
            scripted: None,
            can_bash: true,
            clips: None,
        }
    }

    #[test]
    fn an_air_attack_needs_a_target_above_and_jumps_to_it_then_lands() {
        let high = Target {
            position: [0., 200., 150.],
            radius: 40.,
        };
        let level = Target {
            position: [0., 0., 150.],
            radius: 40.,
        };
        let mut f = Follower::spawn([0.; 3], 0., 0);
        // 200 above is inside the air window (100-385) and outside the
        // ground one (-100-150, each widened by the radius).
        assert!(f.eligible(&DONALD[2], &high));
        assert!(!f.eligible(&DONALD[0], &high));
        assert!(!f.eligible(&DONALD[2], &level));
        assert!(f.eligible(&DONALD[0], &level));
        f.begin(&DONALD[2], &high, 0);
        let mut peak = 0f32;
        let mut played = Vec::new();
        for _ in 0..400 {
            f.tick(&DONALD, &air_scene(&Ground, Some(high)));
            peak = peak.max(f.position[1]);
            if played.last() != Some(&f.motion()) {
                played.push(f.motion());
            }
            // Stop when the land motion lets the AI act again.
            if played.contains(&5) && f.reaction.is_none() {
                break;
            }
        }
        // The jump clears the target's height, carries the member toward
        // it, and ends with the fall and land motions before the AI resumes.
        assert!(peak >= 200., "{peak}");
        assert!(f.position[2] > 100.);
        assert_eq!(f.position[1], 0.);
        assert!(f.air.is_none() && f.reaction.is_none());
        assert_eq!(&played[..3], &[181, FALL, 5]);
    }

    #[test]
    fn a_hit_in_the_air_cancels_the_air_attack() {
        let high = Target {
            position: [0., 200., 150.],
            radius: 40.,
        };
        let mut f = Follower::spawn([0.; 3], 0., 0);
        f.begin(&DONALD[2], &high, 0);
        for _ in 0..10 {
            f.tick(&DONALD, &air_scene(&Ground, Some(high)));
        }
        assert!(f.position[1] > 0. && f.air.is_some());
        f.react(&claw_hit(), &test_sstm(), &Ground, &|m| m < 100);
        assert!(f.attack.is_none() && f.air.is_none());
    }

    fn item_scene<'a>(
        hp: [Option<(i32, i32)>; 3],
        mp: [Option<(i32, i32)>; 3],
        save: &'a Save,
        target: Option<Target>,
    ) -> Scene<'a, Flat> {
        Scene {
            hp,
            mp_all: mp,
            save,
            target,
            spells: true,
            mp: Some((100, 100, false)),
            ..spell_scene(target, &Flat, (100, 100, false), hp)
        }
    }

    const STOCK: Save = with_items([1, 2, 3, 5, 6]);

    #[test]
    fn an_hp_item_goes_to_the_first_member_under_45_percent() {
        let f = Follower::spawn([0.; 3], 0., 0);
        let full = Some((100, 100));
        // Donald (own slot 1) at 44: the Potion for himself.
        let s = item_scene([full, Some((44, 100)), full], [None; 3], &STOCK, None);
        assert_eq!(f.item_pick(ItemNeed::Hp, &s), Some((1, 1)));
        // 45 is not under 45.
        let s = item_scene([full, Some((45, 100)), full], [None; 3], &STOCK, None);
        assert_eq!(f.item_pick(ItemNeed::Hp, &s), None);
        // Sora under 45 and above a quarter: Sora, before Goofy.
        let s = item_scene(
            [Some((30, 100)), full, Some((40, 100))],
            [None; 3],
            &STOCK,
            None,
        );
        assert_eq!(f.item_pick(ItemNeed::Hp, &s).map(|p| p.1), Some(0));
        // Two members under it: the Mega-Potion first; without one, the
        // Potion (the list order, FUN_1401cc970).
        let s = item_scene(
            [Some((30, 100)), full, Some((10, 100))],
            [None; 3],
            &STOCK,
            None,
        );
        assert_eq!(f.item_pick(ItemNeed::Hp, &s).map(|p| p.0), Some(5));
        let no_mega = with_items([1, 0, 0, 0, 0]);
        let s = item_scene(
            [Some((30, 100)), full, Some((10, 100))],
            [None; 3],
            &no_mega,
            None,
        );
        assert_eq!(f.item_pick(ItemNeed::Hp, &s).map(|p| p.0), Some(1));
        // Nothing in the slots, nothing used.
        let none = EVERY;
        let s = item_scene([Some((30, 100)), full, full], [None; 3], &none, None);
        assert_eq!(f.item_pick(ItemNeed::Hp, &s), None);
    }

    #[test]
    fn a_partner_in_need_is_the_hp_items_target_first() {
        let f = Follower::spawn([0.; 3], 0., 0);
        let full = Some((100, 100));
        // Sora at a quarter and Donald under 45: Sora is served.
        let s = item_scene(
            [Some((25, 100)), Some((40, 100)), full],
            [None; 3],
            &STOCK,
            None,
        );
        assert_eq!(f.item_pick(ItemNeed::Hp, &s).map(|p| p.1), Some(0));
    }

    #[test]
    fn an_mp_item_prefers_ether_and_the_mega_one_for_two() {
        let f = Follower::spawn([0.; 3], 0., 1);
        let hp = [Some((100, 100)); 3];
        // Goofy (own slot 2) at 40% MP: the Ether for himself.
        let mp = [Some((100, 100)), Some((100, 100)), Some((40, 100))];
        let s = item_scene(hp, mp, &STOCK, None);
        assert_eq!(f.item_pick(ItemNeed::Mp, &s), Some((3, 2)));
        // Two under it: the Mega-Ether.
        let mp = [Some((10, 100)), Some((100, 100)), Some((40, 100))];
        let s = item_scene(hp, mp, &STOCK, None);
        assert_eq!(f.item_pick(ItemNeed::Mp, &s), Some((6, 2)));
        // A dead member needs no MP.
        let dead = [Some((100, 100)), Some((0, 100)), Some((100, 100))];
        let mp = [Some((100, 100)), Some((0, 100)), Some((100, 100))];
        let s = item_scene(dead, mp, &STOCK, None);
        assert_eq!(f.item_pick(ItemNeed::Mp, &s), None);
    }

    #[test]
    fn a_partner_in_need_is_served_before_the_attacks() {
        let t = Target {
            position: [0., 0., 100.],
            radius: 40.,
        };
        let mut f = Follower::spawn([0.; 3], 0., 0);
        // Sora at a quarter, an enemy in reach and an item in the slots.
        let full = Some((100, 100));
        let s = Scene {
            length: &all_motions,
            ..item_scene([Some((25, 100)), full, full], [None; 3], &STOCK, Some(t))
        };
        let mut used = false;
        for _ in 0..20 {
            f.tick(&DONALD, &s);
            if let Some(a) = f.attack {
                // Cure or the item, never a weapon swing.
                assert!(a.item.is_some() || a.spell == Some(Spell::Cure), "{a:?}");
                used = true;
                break;
            }
        }
        assert!(used);
    }

    #[test]
    fn a_scripted_bash_follows_the_engine_and_ends_when_it_says() {
        let mut f = Follower::spawn([0., 0., 0.], 0., 1);
        let t = Target {
            position: [0., 0., 500.],
            radius: 40.,
        };
        let bash = GOOFY
            .iter()
            .find(|r| r.spell == Some(Spell::Bash))
            .expect("Goofy has the Bash");
        f.begin(bash, &t, 0);
        // Before the room has started the engine's act the member waits.
        let waiting = Scene {
            ..spell_scene(Some(t), &Flat, (100, 100, false), [None; 3])
        };
        f.tick(&GOOFY, &waiting);
        assert_eq!(f.motion(), 192);
        let running = Scene {
            scripted: Some(Scripted {
                motion: 192,
                tick: 12,
                serial: 1,
                blend: 0.,
                queue: [None; 2],
                position: [5., 0., 7.],
                yaw: 0.5,
                done: false,
            }),
            ..spell_scene(Some(t), &Flat, (100, 100, false), [None; 3])
        };
        f.tick(&GOOFY, &running);
        assert_eq!(
            (f.position, f.yaw, f.motion(), f.tick),
            ([5., 0., 7.], 0.5, 192, 12)
        );
        let over = Scene {
            scripted: Some(Scripted {
                motion: 0,
                tick: 0,
                serial: 2,
                blend: 0.,
                queue: [None; 2],
                position: [5., 0., 9.],
                yaw: 0.5,
                done: true,
            }),
            ..spell_scene(Some(t), &Flat, (100, 100, false), [None; 3])
        };
        f.tick(&GOOFY, &over);
        assert!(f.attack.is_none());
        assert_eq!(f.position, [5., 0., 9.]);
    }

    #[test]
    fn the_bash_needs_the_script_and_a_target_250_to_750_away() {
        let f = Follower::spawn([0.; 3], 0., 1);
        let bash = GOOFY.iter().find(|r| r.spell == Some(Spell::Bash)).unwrap();
        let near = Target {
            position: [0., 0., 100.],
            radius: 40.,
        };
        let far = Target {
            position: [0., 0., 500.],
            radius: 40.,
        };
        assert!(!f.eligible(bash, &near));
        assert!(f.eligible(bash, &far));
        // Without the member's script no Bash is chosen.
        let mut s = spell_scene(Some(far), &Flat, (100, 100, false), [None; 3]);
        s.length = &spell_length_with_bash;
        s.can_bash = false;
        assert!(!f.ready(bash, &far, &s) || !s.spells);
    }

    fn spell_length_with_bash(m: u16) -> Option<u32> {
        matches!(m, 151 | 152 | 191 | 192 | 193 | 194).then_some(60)
    }

    /// A motion set for the controller: idle (slots 0 battle, 2 field),
    /// walk (4, 7), run (8, 11) and the weapon switch (24, 27) with range 0
    /// open throughout, the attacks 151 and 152 and the spell motion 192.
    struct Set;
    impl Clips for Set {
        fn clip(&self, motion: u16, field: bool) -> Option<party_motion::Clip> {
            let (slot, length, loop_top) = match (motion, field) {
                (0, false) => (0, 400, 0),
                (0, true) => (2, 300, 0),
                (1, false) => (4, 52, 0),
                (1, true) => (7, 48, 0),
                (2, false) => (8, 36, 0),
                (2, true) => (11, 36, 0),
                (6, false) => (24, 26, 26),
                (6, true) => (27, 40, 40),
                (151, _) => (604, 64, 64),
                (152, _) => (608, 84, 84),
                (192, _) => (768, 76, 76),
                _ => return None,
            };
            Some(party_motion::Clip {
                slot,
                length,
                loop_top,
            })
        }
        fn range(&self, slot: usize, kind: u8, _: u32) -> bool {
            kind == 0 && slot < 28
        }
    }

    fn set_length(m: u16) -> Option<u32> {
        Set.clip(m, false).map(|c| c.length + 1)
    }

    fn set_scene<'a>(target: Option<Target>, leader: Vec3, battle: bool) -> Scene<'a, Flat> {
        Scene {
            leader,
            length: &set_length,
            battle,
            clips: Some(&Set),
            ..scene(leader, target, &Flat)
        }
    }

    #[test]
    fn an_attack_runs_on_its_clock_and_ends_into_the_queued_idle() {
        let t = Target {
            position: [0., 0., 150.],
            radius: 40.,
        };
        let mut f = Follower::spawn([0.; 3], 0., 0);
        f.begin(&DONALD[0], &t, 0);
        // Out of reach of a new decision: no target for the ticks after.
        f.tick(&DONALD, &set_scene(None, [0.; 3], true));
        let a = f.attack.expect("attacking");
        // FUN_1401b0420: the motion starts unblended, the first step jumps
        // from 0 to 2.
        assert_eq!(
            (a.motion, a.tick, a.hold, f.ctl.weight()),
            (151, 2, 0, None)
        );
        assert_eq!(f.ctl.queued().collect::<Vec<_>>(), vec![0]);
        let mut ticks = 1;
        while f.attack.is_some() {
            f.tick(&DONALD, &set_scene(None, [0.; 3], true));
            ticks += 1;
            assert!(ticks < 100);
        }
        // 62 more steps reach tick 64, where the queued idle starts at once.
        assert_eq!(ticks, 63);
        assert_eq!((f.ctl.motion, f.ctl.clip.unwrap().slot), (0, 0));
    }

    #[test]
    fn a_spell_motion_holds_its_first_frame_through_its_blend() {
        let t = Target {
            position: [0., 0., 200.],
            radius: 40.,
        };
        let mut f = Follower::spawn([0.; 3], 0., 0);
        let fire = DONALD
            .iter()
            .find(|r| r.spell == Some(Spell::Fire))
            .unwrap();
        f.begin(fire, &t, 0);
        assert_eq!(f.attack.unwrap().hold, 6);
        let mut ticks = Vec::new();
        for _ in 0..8 {
            f.tick(&DONALD, &set_scene(None, [0.; 3], true));
            ticks.push(f.attack.unwrap().tick);
        }
        // The blend counts 3..8 on the held frame 0, then the frames run.
        assert_eq!(ticks, vec![0, 0, 0, 0, 0, 0, 2, 3]);
    }

    #[test]
    fn the_battle_flag_draws_the_weapon_and_an_idle_loop_end_puts_it_away() {
        let mut f = Follower::spawn([0.; 3], 0., 0);
        // Out of battle the field idle plays.
        f.tick(&DONALD, &set_scene(None, [0.; 3], false));
        assert_eq!(f.ctl.clip.unwrap().slot, 2);
        // The battle flag: the stance turns, idle in it is the switch
        // motion first (FUN_1403c88c0), then the battle idle.
        f.tick(&DONALD, &set_scene(None, [0.; 3], true));
        assert!(f.battle_stance);
        assert_eq!((f.ctl.motion, f.ctl.clip.unwrap().slot), (6, 24));
        for _ in 0..30 {
            f.tick(&DONALD, &set_scene(None, [0.; 3], true));
        }
        assert_eq!(f.ctl.clip.unwrap().slot, 0);
        // The flag drops: the stance holds until the idle loop ends.
        let mut ticks = 0;
        while f.battle_stance {
            f.tick(&DONALD, &set_scene(None, [0.; 3], false));
            ticks += 1;
            assert!(ticks < 500);
        }
        assert!(ticks > 300, "{ticks}");
        f.tick(&DONALD, &set_scene(None, [0.; 3], false));
        assert_eq!((f.ctl.motion, f.ctl.clip.unwrap().slot), (6, 27));
    }

    #[test]
    fn with_no_action_possible_the_member_waits_facing_the_enemy_then_attacks() {
        // The enemy is behind: no record passes the front test.
        let t = Target {
            position: [0., 0., -120.],
            radius: 40.,
        };
        let mut f = Follower::spawn([0.; 3], 0., 0);
        f.tick(&DONALD, &scene([0.; 3], Some(t), &Flat));
        assert!(f.wait.is_some() && f.attack.is_none());
        // Holding, the member stands and turns toward the enemy.
        f.wait = Some(Wait::Hold(30.));
        let mut turned = 0;
        while f.attack.is_none() {
            let before = f.position;
            f.tick(&DONALD, &scene([0.; 3], Some(t), &Flat));
            if f.attack.is_none() {
                assert_eq!(f.position, before);
            }
            turned += 1;
            assert!(turned < 30);
        }
        // Facing it, an attack is possible: the wait ends and it attacks.
        assert!(f.wait.is_none());
    }

    #[test]
    fn after_an_action_a_hop_leaves_the_ground_and_lands() {
        let t = Target {
            position: [0., 0., 150.],
            radius: 40.,
        };
        let mut f = Follower::spawn([0.; 3], 0., 0).with_personality(Personality {
            hop_chance: 1.,
            ..DONALD_PERSONALITY
        });
        f.begin(&DONALD[0], &t, 0);
        let mut peak = 0f32;
        let mut hopped = false;
        for _ in 0..200 {
            f.tick(&DONALD, &air_scene(&Ground, Some(t)));
            peak = peak.max(f.position[1]);
            hopped |= f.hop.is_some();
        }
        // Donald's row: 15 high, back from the enemy.
        assert!(hopped);
        assert!((peak - 15.).abs() < 3., "{peak}");
        assert!(f.position[2] < 0.);
        assert_eq!(f.position[1], 0.);
    }

    #[test]
    fn a_follower_blends_into_a_walk_and_keeps_the_phase_into_the_run() {
        let mut f = Follower::spawn([0.; 3], 0., 0);
        // Settled behind a leader close by, then the leader is far away.
        for _ in 0..20 {
            f.tick(&DONALD, &set_scene(None, [0., 0., 150.], false));
        }
        let mut seen: Vec<(u16, f32, Option<f32>)> =
            vec![(f.ctl.motion, f.ctl.frame(), f.ctl.weight())];
        for _ in 0..120 {
            f.tick(&DONALD, &set_scene(None, [0., 0., 900.], false));
            if seen.last().is_none_or(|s| s.0 != f.ctl.motion) {
                seen.push((f.ctl.motion, f.ctl.frame(), f.ctl.weight()));
            }
        }
        let motions: Vec<u16> = seen.iter().map(|s| s.0).collect();
        assert_eq!(&motions[..3], &[0, 1, 2]);
        // Each change blends over 8 (shown at 3/8 the tick it starts).
        assert_eq!(seen[1].2, Some(3. / 8.));
        assert_eq!(seen[2].2, Some(3. / 8.));
        // The run starts where the walk's phase put it, not at frame 0.
        assert!(seen[2].1 > 0., "{seen:?}");
    }
}
