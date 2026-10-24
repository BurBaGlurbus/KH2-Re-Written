//! The party's members as the executable builds them (`FUN_1401b06e0`):
//! the personality each character id selects, its records (the resource
//! blocks at `DAT_14071d800`, 0x80-byte records from header +0xa4) and the
//! command table its static initializer fills (entries of 0x48 bytes: the
//! weapon attacks run `FUN_1401cd0a0`, the specials start the act the entry
//! names through `FUN_1401cc650`, the Cures run `FUN_1401cc1a0`; every
//! table also holds the HP and MP item entries).
//!
//! | char | personality | class (table) | specials |
//! |---|---|---|---|
//! | 2 | 0 (Pride Lands 0xc) | Donald (0x140724d40) | fire, blizzard, thunder, cure |
//! | 3 | 1 (Pride Lands 0xd) | Goofy (0x1407253b0) | tornado, blade, turbo |
//! | 5 | 2 | Auron (0x140725990) | yukizakura, cure |
//! | 6 | 4 | Mulan (0x140726040) | mushufire, flametongue |
//! | 7 | 5 | Aladdin (0x1407263e0) | slashrush, quickplay |
//! | 8 | 6 | Sparrow (0x140726780) | slash, shadowsnipe, bonecrusher |
//! | 9 | 7 | Beast (0x140726b70) | shout, rush |
//! | 10 | 8 | Jack (0x140726f10) | slidingfire, backstepblizzard, airthunder |
//! | 11 | 9 | Simba (0x140727300) | rush, quake, cure |
//! | 12 | 10 | Tron (0x1407276f0) | normal_attack1-4 (his swings), overflow, thunder |
//! | 13 | 11 | Riku (0x140727a90) | cure |
//! | 15 | 3 | Pin (0x140725d30) | none |
//!
//! A record's height window is the target's height over the member
//! (records +0x18, +0x1c), its jump offset (+0x20) makes it an air attack.

use crate::party_members::{
    AIR_MAX, DONALD, DONALD_PERSONALITY, GOOFY, ItemNeed, Personality, Record, Spell, Urgent,
};
use crate::party_motion::{FOLLOW, Follow};

/// A member's personality as the room needs it.
#[derive(Clone, Copy, Debug)]
pub struct Member {
    /// The character id (objentry +0x4c).
    pub character: u8,
    /// The name of its battle voices (`voice/us/battle/<world>0_<name>`,
    /// the table at 0x14074beb0 indexed by character) and the world whose
    /// set holds them for a guest (it only joins there).
    pub voice: &'static str,
    pub home: &'static str,
    pub records: &'static [Record],
    /// The header's movement (the room puts the PRTY jump height in).
    pub personality: Personality,
    /// The target search radius (header +0x68).
    pub search: f32,
}

const fn rec(id: u8, motion: u16, window: [f32; 4], jump: f32) -> Record {
    Record {
        id,
        motion,
        near: window[0],
        far: window[1],
        low: window[2],
        high: window[3],
        air: jump != 0.,
        jump,
        cost: 0,
        spell: None,
        heal_below: 0.,
        item: None,
        urgent: Urgent::No,
    }
}

const fn act(id: u8, name: &'static str, window: [f32; 4], cost: u8) -> Record {
    act_motion(id, 0, name, window, cost)
}

/// An act whose record names a motion its script reads (Tron's swings).
const fn act_motion(id: u8, motion: u16, name: &'static str, window: [f32; 4], cost: u8) -> Record {
    Record {
        cost,
        spell: Some(Spell::Act(name)),
        ..rec(id, motion, window, 0.)
    }
}

/// `FUN_1401cc1a0`: the Cure of Auron, Simba and Riku (cost 99, a member
/// under 0.4 of its HP; entry flag 0x10, the HP emergency pass).
const fn cure(id: u8) -> Record {
    Record {
        heal_below: 0.4,
        urgent: Urgent::Hp,
        ..act(id, "cure", [0., 0., 0., 0.], 99)
    }
}

const fn item(id: u8, need: ItemNeed) -> Record {
    Record {
        item: Some(need),
        urgent: match need {
            ItemNeed::Hp => Urgent::Hp,
            ItemNeed::Mp => Urgent::Mp,
        },
        ..rec(id, crate::party_members::ITEM_MOTION, [0., 0., 0., 0.], 0.)
    }
}

const HP: Record = item(200, ItemNeed::Hp);
const MP: Record = item(201, ItemNeed::Mp);

/// Auron (personality 2 at 0x14071e1f0).
pub const AURON: [Record; 8] = [
    rec(0x8, 151, [0., 300., -100., 200.], 0.),
    rec(0x9, 152, [0., 500., -100., 200.], 0.),
    rec(0xa, 181, [0., 280., 150., 585.], 1.),
    rec(0xb, 182, [0., 280., 150., 585.], 1.),
    act(0x5f, "yukizakura", [0., 750., -100., 1000.], 10),
    cure(0x42),
    HP,
    MP,
];

/// Pin, Mulan disguised (personality 3 at 0x14071e610): weapon attacks
/// only.
pub const PIN: [Record; 6] = [
    rec(0xc, 151, [0., 300., -100., 150.], 0.),
    rec(0xd, 152, [250., 300., -100., 150.], 0.),
    rec(0xe, 153, [0., 300., 100., 585.], 0.),
    rec(0xf, 181, [0., 100., 80., 185.], 20.),
    HP,
    MP,
];

/// Mulan (personality 4 at 0x14071e910).
pub const MULAN: [Record; 8] = [
    rec(0x10, 151, [0., 300., -100., 200.], 0.),
    rec(0x11, 152, [0., 450., -100., 150.], 0.),
    rec(0x12, 181, [0., 150., 100., 585.], -50.),
    rec(0x13, 182, [0., 150., 100., 585.], -50.),
    act(0x60, "mushufire", [0., 250., -100., 200.], 10),
    act(0x61, "flametongue", [250., 750., -100., 1000.], 10),
    HP,
    MP,
];

/// Aladdin (personality 5 at 0x14071ed50).
pub const ALADDIN: [Record; 8] = [
    rec(0x14, 151, [0., 250., -100., 150.], 0.),
    rec(0x15, 152, [0., 300., -100., 100.], 0.),
    rec(0x16, 181, [0., 100., 100., 385.], 50.),
    rec(0x17, 182, [0., 150., 100., 385.], -50.),
    act(0x63, "slashrush", [0., 500., -100., 150.], 10),
    act(0x64, "quickplay", [250., 750., -100., 500.], 10),
    HP,
    MP,
];

/// Jack Sparrow (personality 6 at 0x14071f190).
pub const SPARROW: [Record; 9] = [
    rec(0x18, 151, [0., 450., -100., 180.], 0.),
    rec(0x19, 152, [0., 150., -100., 180.], 0.),
    rec(0x1a, 181, [0., 250., 180., 385.], 1.),
    rec(0x1b, 182, [0., 400., 250., 485.], 1.),
    act(0x72, "slash", [0., 250., -100., 150.], 10),
    act(0x73, "shadowsnipe", [250., 750., -100., 150.], 10),
    act(0x74, "bonecrusher", [0., 500., -100., 500.], 10),
    HP,
    MP,
];

/// Beast (personality 7 at 0x14071f610).
pub const BEAST: [Record; 8] = [
    rec(0x1c, 151, [0., 300., -100., 150.], 0.),
    rec(0x1d, 152, [100., 350., -100., 150.], 0.),
    rec(0x1e, 181, [0., 100., 150., 385.], 1.),
    rec(0x1f, 182, [0., 100., 150., 385.], 1.),
    act(0x45, "shout", [0., 250., -100., 250.], 10),
    act(0x62, "rush", [250., 750., -100., 150.], 10),
    HP,
    MP,
];

/// Jack Skellington (personality 8 at 0x14071fa30).
pub const JACK: [Record; 9] = [
    rec(0x20, 151, [0., 180., -100., 100.], 0.),
    rec(0x21, 152, [100., 280., -100., 150.], 0.),
    rec(0x22, 181, [0., 150., 150., 385.], -50.),
    rec(0x23, 182, [0., 250., 0., 185.], -50.),
    act(0x65, "slidingfire", [250., 750., -100., 150.], 10),
    act(0x66, "backstepblizzard", [0., 500., -100., 500.], 10),
    act(0x67, "airthunder", [0., 250., -100., 1000.], 10),
    HP,
    MP,
];

/// Simba (personality 9 at 0x14071feb0).
pub const SIMBA: [Record; 9] = [
    rec(0x24, 151, [0., 160., -160., 160.], 0.),
    rec(0x25, 152, [0., 160., -160., 160.], 0.),
    rec(0x26, 181, [0., 160., 160., 360.], 1.),
    rec(0x27, 182, [0., 160., 160., 360.], 1.),
    act(0x6b, "rush", [250., 750., -100., 150.], 10),
    act(0x6c, "quake", [0., 250., -100., 500.], 10),
    cure(0x51),
    HP,
    MP,
];

/// Tron (personality 10 at 0x140720330): his four swings are acts too; each
/// act's script starts the motion of its record (151, 152, 181, 182).
pub const TRON: [Record; 8] = [
    act_motion(0x28, 151, "normal_attack1", [0., 1000., -100., 100.], 0),
    act_motion(0x29, 152, "normal_attack2", [0., 350., -100., 100.], 0),
    act_motion(0x78, 181, "normal_attack3", [0., 1000., 100., 1000.], 0),
    act_motion(0x79, 182, "normal_attack4", [0., 450., 100., 385.], 0),
    act(0x6d, "overflow", [250., 1000., -1000., 1000.], 10),
    act(0x55, "thunder", [0., 500., -100., 300.], 10),
    HP,
    MP,
];

/// Riku (personality 11 at 0x140720730).
pub const RIKU: [Record; 7] = [
    rec(0x2c, 151, [0., 160., -160., 160.], 0.),
    rec(0x2d, 152, [0., 160., -160., 160.], 0.),
    rec(0x2e, 181, [0., 160., 160., 360.], 1.),
    rec(0x2f, 182, [0., 160., 160., 360.], 1.),
    cure(0x59),
    HP,
    MP,
];

/// Beast's header: he follows from farther away.
const BEAST_FOLLOW: Follow = Follow {
    enter: [[300., 340., 1000.], [300., 340., 1000.], [0., 240., 1000.]],
    exit: [[0., 260., 300.], [0., 260., 300.], [0., 160., 200.]],
};

const fn member(
    character: u8,
    voice: &'static str,
    home: &'static str,
    records: &'static [Record],
    hop_chance: f32,
    hops: &'static [[f32; 5]],
    waits: [[f32; 2]; 2],
) -> Member {
    Member {
        character,
        voice,
        home,
        records,
        personality: Personality {
            follow: FOLLOW,
            jump_max: AIR_MAX,
            hop_chance,
            hops,
            waits,
        },
        search: 800.,
    }
}

/// The hop rows most headers share: back 80 high, 12 a tick.
const BACK_80: &[[f32; 5]] = &[[180., 80., 12., 0.98, 18.]];
const WAITS_60_120: [[f32; 2]; 2] = [[6., 60.], [60., 120.]];

/// The members by character id.
pub const MEMBERS: [Member; 12] = [
    Member {
        character: 2,
        voice: "donald",
        home: "",
        records: &DONALD,
        personality: DONALD_PERSONALITY,
        search: 800.,
    },
    member(
        3,
        "goofy",
        "",
        &GOOFY,
        0.5,
        &[[180., 100., 18., 0.98, 18.], [0., 100., 10., 0.98, 18.]],
        [[6., 60.], [600., 900.]],
    ),
    member(
        5,
        "auron",
        "he",
        &AURON,
        0.2,
        &[[180., 150., 15., 0.98, 24.], [0., 150., 15., 0.98, 24.]],
        [[6., 60.], [30., 180.]],
    ),
    member(
        6,
        "mulan",
        "mu",
        &MULAN,
        0.4,
        &[
            [45., 25., 22., 0.98, 10.8],
            [-45., 25., 22., 0.98, 10.8],
            [180., 40., 20., 0.98, 12.],
        ],
        [[6., 60.], [300., 600.]],
    ),
    member(
        7,
        "aladdin",
        "al",
        &ALADDIN,
        0.6,
        &[
            [60., 40., 20., 0.98, 10.8],
            [-60., 40., 20., 0.98, 10.8],
            [180., 40., 20., 0.98, 10.8],
        ],
        [[6., 60.], [120., 300.]],
    ),
    member(
        8,
        "sparrow",
        "ca",
        &SPARROW,
        0.6,
        &[[180., 60., 28., 0.82, 15.]],
        [[6., 60.], [300., 600.]],
    ),
    {
        let mut beast = member(
            9,
            "beast",
            "bb",
            &BEAST,
            0.2,
            &[[180., 50., 12., 0.98, 18.], [0., 50., 12., 0.98, 18.]],
            [[6., 60.], [6., 60.]],
        );
        beast.personality.follow = BEAST_FOLLOW;
        beast
    },
    member(10, "jack", "nm", &JACK, 0.4, BACK_80, WAITS_60_120),
    Member {
        search: 1000.,
        ..member(11, "simba", "lk", &SIMBA, 0.2, BACK_80, WAITS_60_120)
    },
    member(12, "tron", "tr", &TRON, 0.2, BACK_80, WAITS_60_120),
    member(13, "riku", "eh", &RIKU, 0.2, BACK_80, WAITS_60_120),
    Member {
        search: 250.,
        ..member(
            15,
            "pin",
            "mu",
            &PIN,
            0.2,
            &[[0., 150., 12., 0.98, 30.]],
            [[180., 360.], [120., 300.]],
        )
    },
];

/// The character whose level and status rows a character reads
/// (`FUN_1403e60c0`: Roxas 0xe reads Sora's, Pin 0xf Mulan's).
pub fn stats_character(character: u8) -> u8 {
    match character {
        0xe => 1,
        0xf => 6,
        c => c,
    }
}

/// The member of a character id.
pub fn by_character(character: u8) -> Option<&'static Member> {
    MEMBERS.iter().find(|m| m.character == character)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_act_record_reads_back_as_the_executable_keeps_it() {
        // Tron's first swing: the script reads its motion (+4) and the jump
        // offset (+0x20) through `trap_friend_get_action_param`.
        let w = TRON[0].raw_words();
        assert_eq!((w[0], w[1], w[2]), (0x28, 151, 0));
        assert_eq!(f32::from_bits(w[5]), 1000.);
        assert_eq!(f32::from_bits(w[6]), -100.);
        assert_eq!(f32::from_bits(w[8]), 0.);
        let cure = RIKU.iter().find(|r| r.id == 0x59).unwrap().raw_words();
        assert_eq!((w.len(), cure[2]), (32, 99));
        assert_eq!(f32::from_bits(cure[15]), 0.4);
    }

    #[test]
    fn every_member_has_its_items_and_a_ground_attack_or_an_act() {
        for m in &MEMBERS {
            assert!(m.records.iter().any(|r| r.item == Some(ItemNeed::Hp)));
            assert!(m.records.iter().any(|r| r.item == Some(ItemNeed::Mp)));
            assert!(
                m.records
                    .iter()
                    .any(|r| r.item.is_none() && (r.spell.is_none() || r.cost == 0)),
                "{}",
                m.voice
            );
        }
        assert_eq!(
            by_character(9).unwrap().personality.follow.enter[0][0],
            300.
        );
        assert!(by_character(4).is_none());
    }

    #[test]
    fn a_nonzero_jump_offset_makes_an_air_attack() {
        assert!(MULAN.iter().any(|r| r.air && r.jump == -50.));
        assert!(PIN.iter().any(|r| r.motion == 153 && !r.air));
    }
}
