//! The world map's objects (objentry type 0x11, created by FUN_140393f50
//! from the objentry sub-kind: 0 symbol, 1 Gummi ship, 2 cursor, 3 dummy)
//! and the map objects and draw groups the symbol slots switch.
use area::world_state::{WorldRecord, WorldState};

/// The world map cursor (`WM_CURSOR`, sub-kind 2, `worldmap::Player`).
pub const CURSOR: u32 = area::WORLD_MAP_CURSOR;
/// The Gummi ship (`WM_GUMI_01`, sub-kind 1, `worldmap::Gumi`).
pub const GUMI: u32 = 0x3ae;

/// One row of worldmap::Symbol's table at 0x1405c07c0 (FUN_140390150).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolEntry {
    pub object: u32,
    pub world: u8,
    /// +5: the map draw group the symbol switches (0: none).
    pub group: u8,
    /// +6: the world's index on the info panels.
    pub info: u8,
}

const fn entry(object: u32, world: u8, group: u8, info: u8) -> SymbolEntry {
    SymbolEntry {
        object,
        world,
        group,
        info,
    }
}

pub const SYMBOLS: [SymbolEntry; 12] = [
    entry(0x3a2, 2, 0x28, 9),
    entry(0x3a3, 4, 0x2a, 0),
    entry(0x3a4, 8, 0, 1),
    entry(0x3a5, 5, 0, 2),
    entry(0x3a6, 6, 0, 5),
    entry(0x3a7, 12, 0x32, 0),
    entry(0x3a8, 11, 0, 7),
    entry(0x3a9, 16, 0, 8),
    entry(0x3aa, 14, 0, 3),
    entry(0x3ab, 7, 0, 4),
    entry(0x3ac, 10, 0, 6),
    entry(0x456, 18, 0, 0),
];

/// The table row of a symbol object.
pub fn symbol(object: u32) -> Option<&'static SymbolEntry> {
    SYMBOLS.iter().find(|s| s.object == object)
}

/// The tint per world state at 0x1405c0820 (`{r, g, b, a}`, 0x80 = 1.0),
/// copied to the object's colour bytes (+0x4d8..+0x4db).
const TINTS: [[u8; 4]; 6] = [
    [0, 0, 0, 0],
    [0x20, 0x20, 0x20, 0x80],
    [0x80, 0x80, 0x80, 0x80],
    [0x80, 0x80, 0x80, 0x80],
    [0x80, 0x80, 0x80, 0x80],
    [0x80, 0x80, 0x80, 0x80],
];

/// Story flag that switches The World That Never Was symbol's effect
/// (FUN_140390510: effect 8 with it, 6 without).
pub const EH_EFFECT_FLAG: u32 = 0x481c;

/// What FUN_140390510 makes of a symbol for its world record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolLook {
    /// Object flag 0x2 clear (world state ≥ 1).
    pub drawn: bool,
    /// Object flag 0x20 clear (world state ≥ 2).
    pub open: bool,
    /// Object flag 0x40 clear: the cursor can land on it (state ≥ 2 and
    /// record flag bit 1 clear).
    pub reacts: bool,
    /// The motion it plays (FUN_1403c6dc0): 0xa1 open, 0x97 otherwise.
    pub motion: u16,
    /// FUN_1403906f0 (on) / FUN_1403906d0 (off) on the entry's draw group.
    pub group_on: bool,
    /// Effect slot 3 (FUN_1403b6630) is kept while record flag bit 0 is set.
    pub effect_3: bool,
    /// Effect slot 5 (FUN_1403b6670) is kept from state 3.
    pub effect_5: bool,
    pub tint: [u8; 4],
    /// The World That Never Was only (world 0x12): the effect at +0xd70,
    /// stopped below state 2.
    pub eh_effect: Option<u8>,
}

/// The object effect elements the world map starts by id (FUN_1403b6630 /
/// FUN_1403b6670 → FUN_1402c6b30 on the object's PAX).
pub mod effect {
    /// FUN_1403900d0 (symbols) and FUN_140397240 (Gummi ships), op 0x105's
    /// pre-refresh: a one-shot.
    pub const PRE_REFRESH: u16 = 0;
    /// FUN_140390460: a world with a new episode, after the arrival's
    /// events (held at +0xd68).
    pub const NEW_EPISODE: u16 = 4;
}

/// The effects a symbol holds (+0xd58, +0xd60, +0xd70) and the element
/// each slot wants under this look; FUN_140390510 starts a slot's element
/// only when the slot is empty and frees it when the look wants none.
pub fn held_effects(look: &SymbolLook) -> [Option<u16>; 3] {
    [
        look.effect_3.then_some(3),
        look.effect_5.then_some(5),
        look.eh_effect.map(u16::from),
    ]
}

/// FUN_1403900d0: the symbol's last applied record (+0xd38) had flag bit 0
/// clear and the current one has it set.
pub fn symbol_pre_refresh(before: &WorldRecord, after: &WorldRecord) -> bool {
    before.flags & 1 == 0 && after.flags & 1 != 0
}

/// FUN_140397240: the ship's last applied slot state (+0xd40) was 1 and the
/// current one is 2.
pub fn gumi_pre_refresh(before: u8, after: u8) -> bool {
    before == 1 && after == 2
}

/// FUN_140390460, per symbol (all set flag bit 31 of +0x6c8 at creation):
/// its last applied state is above 1 and its world lists an unlocked
/// episode still marked new (FUN_1403f0810 / FUN_1403f0b20).
pub fn new_episode(state: u8, any_new: bool) -> bool {
    state > 1 && any_new
}

/// FUN_140390510. `eh_flag` is story flag 0x481c.
pub fn symbol_look(world: u8, record: &WorldRecord, eh_flag: bool) -> SymbolLook {
    let state = record.state;
    let open = state >= 2;
    SymbolLook {
        drawn: state != 0,
        open,
        reacts: open && record.flags & 2 == 0,
        motion: if open { 0xa1 } else { 0x97 },
        group_on: open,
        effect_3: record.flags & 1 != 0,
        effect_5: state >= 3,
        // The exe indexes the table without a bound; states past 5 do not
        // occur in the installed scripts.
        tint: TINTS[(state as usize).min(TINTS.len() - 1)],
        eh_effect: (world == 0x12 && open).then_some(if eh_flag { 8 } else { 6 }),
    }
}

/// FUN_14038df20's table at 0x140748230: per symbol slot 1..=0x15 two map
/// objects (`a`, `b`, the n-th drawn map object; 0 none) and a draw group
/// (`c`; 0 none). From the disassembly: state 1 shows `b`, hides `a`, group
/// `c` off; state 2 hides `b`, shows `a`, group `c` on; other states hide
/// both and switch `c` off.
pub const SLOT_OBJECTS: [[u8; 3]; 0x15] = [
    [0x1b, 0x30, 0x2c],
    [0x1c, 0x31, 0x2e],
    [0x1d, 0x32, 0x38],
    [0x1e, 0x33, 0x3a],
    [0x1f, 0x34, 0x30],
    [0x20, 0x35, 0x3c],
    [0x21, 0x36, 0x34],
    [0x22, 0x37, 0x36],
    [0x23, 0x38, 0x00],
    [0x24, 0x00, 0x00],
    [0x25, 0x00, 0x00],
    [0x26, 0x00, 0x00],
    [0x27, 0x00, 0x00],
    [0x28, 0x00, 0x00],
    [0x29, 0x00, 0x00],
    [0x2a, 0x00, 0x00],
    [0x2b, 0x00, 0x00],
    [0x2c, 0x00, 0x00],
    [0x2d, 0x00, 0x00],
    [0x2e, 0x00, 0x00],
    [0x2f, 0x00, 0x00],
];

/// One switch the world map makes on entry: a map object shown or hidden
/// (FUN_14016f3a0 / FUN_14016ec50, object flag 4) or a draw group switched
/// (FUN_140162a00 / FUN_1401629b0). Index 0 is ignored by all four.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    Object { index: u8, shown: bool },
    Group { group: u8, on: bool },
}

/// FUN_14038df20 (and FUN_140394050 after events): the switches of every
/// symbol slot, in the exe's order.
pub fn slot_switches(state: &WorldState) -> Vec<Switch> {
    let mut out = Vec::new();
    for (i, &[a, b, c]) in SLOT_OBJECTS.iter().enumerate() {
        let s = state.symbol_slot(i as u8 + 1);
        let (show_a, show_b, on) = match s {
            1 => (false, true, false),
            2 => (true, false, true),
            _ => (false, false, false),
        };
        for (index, shown) in [(b, show_b), (a, show_a)] {
            if index != 0 {
                out.push(Switch::Object { index, shown });
            }
        }
        if c != 0 {
            out.push(Switch::Group { group: c, on });
        }
    }
    out
}

/// FUN_140397490's table at 0x1405c2108: per Gummi ship (its spawn
/// argument 1..=9) three map objects (the route lines).
pub const GUMI_OBJECTS: [[u8; 3]; 9] = [
    [0x00, 0x09, 0x12],
    [0x01, 0x0a, 0x13],
    [0x05, 0x0e, 0x14],
    [0x06, 0x0f, 0x15],
    [0x02, 0x0b, 0x16],
    [0x07, 0x10, 0x17],
    [0x03, 0x0c, 0x18],
    [0x04, 0x0d, 0x19],
    [0x08, 0x11, 0x1a],
];

/// What FUN_140397490 makes of a Gummi ship for its symbol slot's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GumiLook {
    /// Object flags 0x62 clear (slot state 1 or 2).
    pub drawn: bool,
    pub switches: Vec<Switch>,
}

/// FUN_140397490 for ship `route` (spawn argument 1, 1..=9); the state is
/// symbol slot `route` (FUN_140396f10 / FUN_1403f1e10). From the
/// disassembly: state 1 shows line 0 and hides 1 and 2; state 2 hides 0 and
/// shows 1 and 2; other states hide all three.
pub fn gumi_look(route: u8, state: &WorldState) -> GumiLook {
    let s = state.symbol_slot(route);
    let lines = (route as usize)
        .checked_sub(1)
        .and_then(|i| GUMI_OBJECTS.get(i))
        .copied()
        .unwrap_or([0; 3]);
    let shown = match s {
        1 => [true, false, false],
        2 => [false, true, true],
        _ => [false; 3],
    };
    GumiLook {
        drawn: s == 1 || s == 2,
        // FUN_14016f3a0 / FUN_14016ec50 take the index as is (0 included).
        switches: lines
            .iter()
            .zip(shown)
            .map(|(&index, shown)| Switch::Object { index, shown })
            .collect(),
    }
}

/// dk::WM_OBJ's table at 0x1407494a0 (FUN_14039a630 / FUN_14039a410):
/// objentry id → the world that shows it and its index.
pub const WM_OBJECTS: [(u32, u8, u8); 9] = [
    (0x177d, 8, 0),
    (0x177e, 5, 1),
    (0x177f, 14, 2),
    (0x1780, 7, 3),
    (0x1781, 6, 4),
    (0x1782, 10, 5),
    (0x1783, 11, 6),
    (0x1784, 16, 7),
    (0x1785, 18, 8),
];

/// FUN_14039a410: a dk::WM_OBJ is hidden (flags 0x42, +0x124 bit 8) while
/// its world's state is below 2; `None` for other objects.
pub fn wm_object_shown(object: u32, state: &WorldState) -> Option<bool> {
    let &(_, world, _) = WM_OBJECTS.iter().find(|o| o.0 == object)?;
    Some(state.record(world).is_some_and(|r| r.state >= 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(state: u8, flags: u8) -> WorldRecord {
        WorldRecord {
            state,
            flags,
            ..Default::default()
        }
    }

    #[test]
    fn symbol_looks_follow_the_world_state() {
        let hidden = symbol_look(2, &record(0, 0), false);
        assert!(!hidden.drawn && !hidden.open && !hidden.reacts);
        assert_eq!((hidden.motion, hidden.tint), (0x97, [0; 4]));
        let seen = symbol_look(2, &record(1, 1), false);
        assert!(seen.drawn && !seen.open && seen.effect_3);
        assert_eq!(seen.tint, [0x20, 0x20, 0x20, 0x80]);
        let open = symbol_look(2, &record(2, 2), false);
        assert!(open.open && !open.reacts && open.group_on);
        assert_eq!(open.motion, 0xa1);
        let again = symbol_look(2, &record(3, 0), false);
        assert!(again.reacts && again.effect_5);
        assert_eq!(symbol_look(0x12, &record(2, 0), true).eh_effect, Some(8));
        assert_eq!(symbol_look(0x12, &record(2, 0), false).eh_effect, Some(6));
        assert_eq!(symbol_look(0x12, &record(1, 0), true).eh_effect, None);
        assert_eq!(symbol(0x3a9).map(|s| s.world), Some(16));
    }

    #[test]
    fn symbol_effects_follow_the_record() {
        let flagged = symbol_look(2, &record(3, 1), false);
        assert_eq!(held_effects(&flagged), [Some(3), Some(5), None]);
        let eh = symbol_look(0x12, &record(2, 0), true);
        assert_eq!(held_effects(&eh), [None, None, Some(8)]);
        assert!(symbol_pre_refresh(&record(2, 0), &record(2, 1)));
        assert!(!symbol_pre_refresh(&record(2, 1), &record(2, 1)));
        assert!(gumi_pre_refresh(1, 2) && !gumi_pre_refresh(2, 2));
        assert!(new_episode(2, true) && !new_episode(1, true) && !new_episode(3, false));
    }

    #[test]
    fn slot_switches_follow_the_disassembly() {
        let mut s = WorldState::default();
        s.symbol_slots[0] = 1;
        s.symbol_slots[1] = 2;
        let sw = slot_switches(&s);
        assert_eq!(
            &sw[..6],
            &[
                Switch::Object {
                    index: 0x30,
                    shown: true
                },
                Switch::Object {
                    index: 0x1b,
                    shown: false
                },
                Switch::Group {
                    group: 0x2c,
                    on: false
                },
                Switch::Object {
                    index: 0x31,
                    shown: false
                },
                Switch::Object {
                    index: 0x1c,
                    shown: true
                },
                Switch::Group {
                    group: 0x2e,
                    on: true
                },
            ]
        );
        // Slot 10 has no `b` and no group.
        assert!(sw.contains(&Switch::Object {
            index: 0x24,
            shown: false
        }));
    }

    #[test]
    fn gumi_and_world_objects() {
        let mut s = WorldState::default();
        s.symbol_slots[2] = 2;
        let g = gumi_look(3, &s);
        assert!(g.drawn);
        assert_eq!(
            g.switches,
            vec![
                Switch::Object {
                    index: 5,
                    shown: false
                },
                Switch::Object {
                    index: 0xe,
                    shown: true
                },
                Switch::Object {
                    index: 0x14,
                    shown: true
                },
            ]
        );
        assert!(!gumi_look(1, &s).drawn);
        s.records[8].state = 2;
        assert_eq!(wm_object_shown(0x177d, &s), Some(true));
        assert_eq!(wm_object_shown(0x177e, &s), Some(false));
        assert_eq!(wm_object_shown(0x3a2, &s), None);
    }
}
