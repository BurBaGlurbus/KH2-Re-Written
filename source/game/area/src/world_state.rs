//! The world map's part of the story state and the rules that change it:
//! world records, symbol slots, world events and their queue, episode bits,
//! the draw-group set and the world points (save points) touched. Save
//! offsets are `address − 0x1409a98b0` (FUN_140152380).
use crate::progress::Progress;
use data::{
    bar,
    world_map::{
        self, Condition, DefaultLanding, DrawGroupSet, Episode, LandingCell, Link, ScriptOp,
        VisitKeys, WorldEvent, WorldPoint,
    },
};
use std::io;

/// World records (FUN_1403f1e20: `DAT_1409ad89c + world * 4`, save +0x3fec).
pub const WORLDS: usize = 19;
/// Symbol slots (FUN_1403f1e10: `DAT_1409ada08 + slot − 1`, save +0x4158;
/// FUN_14038df20 walks slots 1..=0x15).
pub const SYMBOL_SLOTS: usize = 0x15;
/// World-event bits (FUN_1403deab0: `DAT_1409ad8e8`, save +0x4038).
pub const EVENT_WORDS: usize = 8;
/// Queued world events (`DAT_1409ad908`, save +0x4058; the `u8` count is
/// `DAT_1409ada07`, save +0x4157).
pub const QUEUE: usize = 0xff;
/// Episode open / new bits (`DAT_1409ada28` / `DAT_1409ada38`, save
/// +0x4178 / +0x4188).
pub const EPISODE_WORDS: usize = 4;
/// World-point bits (`DAT_1409ab7a0`, save +0x1ef0).
pub const POINT_WORDS: usize = 2;
/// Menu flags (FUN_1403e38f0: `DAT_1409abc40`, save +0x2390, up to the
/// chest bits at +0x23ac).
pub const MENU_WORDS: usize = 7;

/// FUN_14039ac80 (`DAT_1409ab5c4 >> 11 & 1`): while this story flag is set,
/// kind-1 world events are marked but not queued.
pub const HOLD_FLAG: u32 = 0x102b;

/// The world map's world (FUN_140154e20 stores 0xf).
pub const WORLD_MAP: u8 = 0xf;

/// FUN_140154e20's table at 0x1405b1900: `{entrance, world}`, the world-map
/// entrance beside each world.
pub const WORLD_MAP_ENTRANCES: [(u8, u8); 15] = [
    (1, 2),
    (2, 4),
    (3, 8),
    (4, 5),
    (5, 6),
    (6, 12),
    (7, 11),
    (8, 16),
    (9, 14),
    (10, 7),
    (11, 10),
    (12, 18),
    (13, 9),
    (14, 13),
    (15, 17),
];

/// The tables the world map state reads (00progress.bin, 70landing.bar,
/// 00worldpoint.bin).
#[derive(Clone, Debug, Default)]
pub struct WorldTables {
    pub visit_keys: Vec<VisitKeys>,
    /// Sorted by id (the exe bsearches them, FUN_1403defd0).
    pub events: Vec<WorldEvent>,
    pub episodes: Vec<Episode>,
    pub default_landings: Vec<DefaultLanding>,
    pub links: Vec<Link>,
    /// Sorted by id (bsearch in FUN_1403f0c00).
    pub draw_group_sets: Vec<DrawGroupSet>,
    pub landing_cells: Vec<LandingCell>,
    pub world_points: Vec<WorldPoint>,
}

/// The BAR entry named `name` (4 bytes, zero-padded).
fn entry<'a>(bytes: &'a [u8], name: &[u8]) -> io::Result<&'a [u8]> {
    let table = bar::read_bar(bytes)?;
    let mut key = [0u8; 4];
    key[..name.len()].copy_from_slice(name);
    let e = table
        .entries
        .iter()
        .find(|e| e.name_bytes == key)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "world map table missing"))?;
    Ok(&bytes[e.offset as usize..e.offset as usize + e.size as usize])
}

impl WorldTables {
    /// Reads the tables from `00progress.bin`, `70landing.bar` and
    /// `00worldpoint.bin` (FUN_14039acd0, FUN_1403f1470, FUN_1403f2250).
    pub fn read(progress: &[u8], landing: &[u8], world_points: &[u8]) -> io::Result<Self> {
        let mut events = world_map::read_world_events(entry(progress, b"wldf")?)?;
        events.sort_by_key(|e| e.id);
        let mut draw_group_sets = world_map::read_draw_group_sets(entry(progress, b"dark")?)?;
        draw_group_sets.sort_by_key(|s| s.id);
        Ok(Self {
            visit_keys: world_map::read_visit_keys(entry(progress, b"wmky")?)?,
            events,
            episodes: world_map::read_episodes(entry(progress, b"epsd")?)?,
            default_landings: world_map::read_default_landings(entry(progress, b"wlds")?)?,
            links: world_map::read_links(entry(progress, b"link")?)?,
            draw_group_sets,
            landing_cells: world_map::read_landing_cells(entry(landing, b"land")?)?,
            world_points: world_map::read_world_points(world_points)?,
        })
    }

    pub fn event(&self, id: u8) -> Option<&WorldEvent> {
        self.events
            .binary_search_by_key(&id, |e| e.id)
            .ok()
            .map(|i| &self.events[i])
    }

    pub fn draw_group_set(&self, id: i32) -> Option<&DrawGroupSet> {
        self.draw_group_sets
            .binary_search_by_key(&id, |s| s.id)
            .ok()
            .map(|i| &self.draw_group_sets[i])
    }

    /// FUN_14038d280: the world's `wmky` record.
    pub fn visit_keys(&self, world: u8) -> Option<&VisitKeys> {
        self.visit_keys.iter().find(|k| k.world == world as i32)
    }

    /// FUN_1403f2030: the world point at `{world, area}`.
    pub fn world_point_at(&self, world: u8, area: u8) -> Option<usize> {
        self.world_points
            .iter()
            .position(|p| p.world == world && p.area == area)
    }
}

/// A world record (4 bytes, FUN_1403f1e20).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorldRecord {
    /// +0: 0 hidden, 1 seen, 2 open, 3 and up revisited (world map symbol
    /// look, FUN_140390510).
    pub state: u8,
    /// +1: the `wlds` set the default landing uses (progress op 13).
    pub landing_set: u8,
    /// +2: progress op 13's word.
    pub value: u8,
    /// +3: bit 0 keeps the symbol's effect slot 3, bit 1 makes the symbol
    /// unreactive (FUN_140390510).
    pub flags: u8,
}

impl WorldRecord {
    /// FUN_1403f1e30.
    pub fn set_state(&mut self, state: u8) {
        if state == 3 && self.state > 2 {
            self.state = self.state.wrapping_add(1);
            return;
        }
        self.state = state;
        if state == 2 {
            self.flags &= !1;
        }
    }
}

/// A jump target as FUN_140152990 takes it (`{world, area, entrance}`; the
/// spawn ids at +4/+6/+8 are −1 unless a spawn id picks the place).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub world: u8,
    pub area: u8,
    pub entrance: u8,
    /// Set when the target is a spawn id (FUN_14039fbc0), which the
    /// world's spawn table turns into an area.
    pub spawn: Option<u16>,
}

impl Target {
    /// `DAT_14074d238` as installed: the "no target" FUN_1403e4190 returns
    /// when no `wlds` record matches.
    pub const NONE: Self = Self {
        world: 0xff,
        area: 0xff,
        entrance: 0xff,
        spawn: None,
    };

    pub fn at(world: u8, area: u8, entrance: u8) -> Self {
        Self {
            world,
            area,
            entrance,
            spawn: None,
        }
    }
}

/// FUN_140154e20: the world map, at the entrance beside `from_world` (0
/// when the table has none).
pub fn world_map_entry(from_world: u8) -> Target {
    let entrance = WORLD_MAP_ENTRANCES
        .iter()
        .find(|&&(_, w)| w == from_world)
        .map_or(0, |&(e, _)| e);
    Target::at(WORLD_MAP, 0, entrance)
}

/// The world map's save fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldState {
    pub records: [WorldRecord; WORLDS],
    /// Slot `n` (1-based) is `symbol_slots[n - 1]`: 0 hidden, 1 seen, 2
    /// open, 3 (FUN_14038df20).
    pub symbol_slots: [u8; SYMBOL_SLOTS],
    pub event_bits: [u32; EVENT_WORDS],
    pub queue: Vec<u8>,
    pub episodes: [u32; EPISODE_WORDS],
    pub episodes_new: [u32; EPISODE_WORDS],
    pub draw_group_set: u8,
    pub world_points: [u32; POINT_WORDS],
    pub menu_flags: [u32; MENU_WORDS],
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            records: [WorldRecord::default(); WORLDS],
            symbol_slots: [0; SYMBOL_SLOTS],
            event_bits: [0; EVENT_WORDS],
            queue: Vec::new(),
            episodes: [0; EPISODE_WORDS],
            episodes_new: [0; EPISODE_WORDS],
            draw_group_set: 0,
            world_points: [0; POINT_WORDS],
            menu_flags: [0; MENU_WORDS],
        }
    }
}

fn bit(words: &[u32], n: u32) -> bool {
    words
        .get((n >> 5) as usize)
        .is_some_and(|w| w >> (n & 0x1f) & 1 != 0)
}

fn set_bit(words: &mut [u32], n: u32) {
    if let Some(w) = words.get_mut((n >> 5) as usize) {
        *w |= 1 << (n & 0x1f);
    }
}

fn clear_bit(words: &mut [u32], n: u32) {
    if let Some(w) = words.get_mut((n >> 5) as usize) {
        *w &= !(1 << (n & 0x1f));
    }
}

impl WorldState {
    pub fn record(&self, world: u8) -> Option<&WorldRecord> {
        self.records.get(world as usize)
    }

    /// FUN_1403f1e10: slot `n` (1-based).
    pub fn symbol_slot(&self, slot: u8) -> u8 {
        (slot as usize)
            .checked_sub(1)
            .and_then(|i| self.symbol_slots.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// FUN_1403deab0.
    pub fn event_seen(&self, id: u32) -> bool {
        bit(&self.event_bits, id)
    }

    /// FUN_1403f0b20: the episode's NEW mark.
    pub fn episode_new(&self, e: &Episode) -> bool {
        bit(&self.episodes_new, e.bit as u32)
    }

    /// FUN_1403f0810: the open episodes of `world`, in table order. The exe
    /// starts at the first record of the world and stops at the first
    /// record of another world.
    pub fn open_episodes<'a>(
        &'a self,
        tables: &'a WorldTables,
        world: u8,
    ) -> impl Iterator<Item = &'a Episode> + 'a {
        tables
            .episodes
            .iter()
            .skip_while(move |e| e.world != world)
            .take_while(move |e| e.world == world)
            .filter(move |e| bit(&self.episodes, e.bit as u32))
    }

    /// FUN_1403e38f0.
    pub fn menu_flag(&self, flag: u32) -> bool {
        bit(&self.menu_flags, flag)
    }

    /// FUN_1403f2270.
    pub fn touch_point(&mut self, point: u32) {
        set_bit(&mut self.world_points, point);
    }

    /// FUN_1403f2230.
    pub fn forget_point(&mut self, point: u32) {
        clear_bit(&mut self.world_points, point);
    }

    pub fn point_touched(&self, point: u32) -> bool {
        bit(&self.world_points, point)
    }

    /// The world point objects (objentry 0x239, FUN_140418c20 /
    /// FUN_140418e30) mark the point of the area they are created in.
    pub fn enter_area_with_world_point(&mut self, tables: &WorldTables, world: u8, area: u8) {
        if let Some(i) = tables.world_point_at(world, area) {
            self.touch_point(i as u32);
        }
    }

    /// FUN_1403df010: the queued event ids, emptying the queue.
    pub fn take_queue(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.queue)
    }

    /// FUN_1403df2e0 without the hold test (`hold` is FUN_14039ac80).
    fn mark_event(&mut self, tables: &WorldTables, id: u8, hold: bool) {
        if !gate_open(&self.event_bits, id) || self.event_seen(id as u32) {
            return;
        }
        set_bit(&mut self.event_bits, id as u32);
        let kind = tables.event(id).map_or(0, |e| e.kind);
        if (kind != 1 || !hold) && self.queue.len() < QUEUE {
            self.queue.push(id);
        }
    }

    /// FUN_1403deb40 on one script; `now` is `DAT_142a24f50` (the queued
    /// events playing on the world map apply a draw-group set at once).
    /// Returns the draw-group set to apply now, if any.
    pub fn run_event_script(&mut self, script: &[ScriptOp], now: bool) -> Option<u8> {
        let mut apply = None;
        for op in script {
            let args = op.args.iter().map(|&a| a as u16);
            match op.op {
                1..=3 => {
                    for w in args {
                        if let Some(r) = self.records.get_mut(w as usize) {
                            r.set_state(op.op as u8 - 1);
                        }
                    }
                }
                4 => {
                    for w in args {
                        if let Some(r) = self.records.get_mut(w as usize) {
                            r.set_state(2);
                            r.flags &= !2;
                        }
                    }
                }
                5 | 6 => {
                    let mask = if op.op == 5 { 2 } else { 1 };
                    for w in args {
                        if let Some(r) = self.records.get_mut(w as usize) {
                            r.flags |= mask;
                        }
                    }
                }
                7 => {
                    for w in args {
                        if let Some(r) = self.records.get_mut(w as usize) {
                            r.set_state(3);
                        }
                    }
                }
                8..=11 => {
                    for s in args {
                        if let Some(slot) = (s as usize)
                            .checked_sub(1)
                            .and_then(|i| self.symbol_slots.get_mut(i))
                        {
                            *slot = op.op as u8 - 8;
                        }
                    }
                }
                // FUN_1403f08e0 / FUN_1403f08c0.
                12 => {
                    for b in args {
                        set_bit(&mut self.episodes, b as u32);
                        set_bit(&mut self.episodes_new, b as u32);
                    }
                }
                13 => {
                    for b in args {
                        clear_bit(&mut self.episodes_new, b as u32);
                    }
                }
                15 => {
                    if let Some(&set) = op.args.first() {
                        self.draw_group_set = set as u8;
                        if now {
                            apply = Some(set as u8);
                        }
                    }
                }
                _ => {}
            }
        }
        apply
    }
}

/// FUN_1403df1b0 / FUN_1403df2e0: events 0x25, 0x2a, 0x2d and 0x2e are not
/// marked once events 0x73, 0x56, 0x54 and 0x72 have been (bit tests on
/// `DAT_1409ad8f4 >> 0x13`, `DAT_1409ad8f0 >> 0x16`, `>> 0x14`,
/// `DAT_1409ad8f4 >> 0x12`).
fn gate_open(bits: &[u32], id: u8) -> bool {
    let after = match id {
        0x25 => 0x73,
        0x2a => 0x56,
        0x2d => 0x54,
        0x2e => 0x72,
        _ => return true,
    };
    !bit(bits, after)
}

/// A cell of a world's landing grid (`+0x2850 + 0x130 * n` of
/// dk::WM_LANDING, filled by FUN_140394550).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridCell {
    pub point: u8,
    pub picture: u8,
    /// Bit 0 of the cell flags: the point is touched but story-blocked;
    /// its message is FUN_1403f20c0's.
    pub blocked: Option<u16>,
    pub target: Target,
}

/// FUN_140394550's result: the cells (`None` for an empty cell) up to the
/// highest cell used, and how many points were placed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LandingGrid {
    pub cells: Vec<Option<GridCell>>,
    pub placed: usize,
}

/// dk::WM_LANDING's cell array has 8 entries.
pub const GRID_CELLS: usize = 8;

impl Progress {
    /// FUN_1403f2110: 0 not touched, 1 touched, 2 touched and blocked.
    pub fn world_point_state(&self, tables: &WorldTables, point: u8) -> u8 {
        if !self.world.point_touched(point as u32) {
            return 0;
        }
        let Some(p) = tables.world_points.get(point as usize) else {
            return 1;
        };
        1 + u8::from(self.destination_blocked(p.world, p.area))
    }

    /// FUN_140394550: the landing grid of `world` from the touched world
    /// points; the place name is FUN_1403bac20(target area, world) and the
    /// picture the `land` record's (FUN_1403f1700).
    pub fn landing_grid(&self, tables: &WorldTables, world: u8) -> LandingGrid {
        let mut grid = LandingGrid::default();
        let mut cells = [None; GRID_CELLS];
        let mut highest = 0usize;
        for c in tables.landing_cells.iter().filter(|c| c.world == world) {
            let state = self.world_point_state(tables, c.point);
            if state == 0 {
                continue;
            }
            highest = highest.max(c.cell as usize);
            let target = tables
                .world_points
                .get(c.point as usize)
                .map_or(Target::NONE, |p| Target::at(p.world, p.area, p.entrance));
            let blocked = (state == 2).then(|| self.blocked_message(target.world, target.area));
            if let Some(slot) = (c.cell as usize)
                .checked_sub(1)
                .and_then(|i| cells.get_mut(i))
            {
                *slot = Some(GridCell {
                    point: c.point,
                    picture: c.picture,
                    blocked,
                    target,
                });
            }
            grid.placed += 1;
        }
        grid.cells = cells[..highest.min(GRID_CELLS)].to_vec();
        grid
    }

    /// FUN_1403e4190: where landing on `world` without a world point
    /// arrives, for the world record's landing set.
    pub fn default_landing(&self, tables: &WorldTables, world: u8) -> Target {
        let set = self.world.record(world).map_or(0, |r| r.landing_set);
        let Some(l) = tables
            .default_landings
            .iter()
            .find(|l| l.world == world && l.set == set)
        else {
            return Target::NONE;
        };
        if l.area < 0x33 {
            Target::at(world, l.area, l.entrance)
        } else {
            Target {
                world,
                area: 0xff,
                entrance: 0,
                spawn: Some(l.area as u16),
            }
        }
    }

    /// FUN_14038d330 / FUN_14038d2c0: the world's `wmky` pairs whose first
    /// flag is set, and those with both set.
    pub fn visits(&self, tables: &WorldTables, world: u8) -> Option<(usize, usize)> {
        let keys = tables.visit_keys(world)?;
        let held = |f: u16| f != 0 && self.test(f as u32);
        let open = keys.pairs.iter().filter(|p| held(p[0])).count();
        let done = keys
            .pairs
            .iter()
            .filter(|p| held(p[0]) && held(p[1]))
            .count();
        Some((open, done))
    }

    /// FUN_1403df060 (new game): event 0 is marked and its script runs at
    /// once (the world map has not loaded, so its draw-group set is only
    /// stored).
    pub fn start_world_events(&mut self) {
        let Some(tables) = self.world_tables.clone() else {
            return;
        };
        self.world.event_bits = [0; EVENT_WORDS];
        self.world.event_bits[0] = 1;
        self.world.queue.clear();
        let hold = self.test(HOLD_FLAG);
        let mut ids = Vec::new();
        if tables.event(0).is_none_or(|e| e.kind != 1) || !hold {
            ids.push(0u8);
        }
        for id in ids {
            if let Some(e) = tables.event(id) {
                self.world.run_event_script(&e.script, false);
            }
        }
    }

    /// FUN_14038dc60 (the world map's first task): the queued events' scripts
    /// run at once, then the distinct world map events they name, in
    /// priority order. Returns those event ids and the last draw-group set
    /// a script applied.
    pub fn play_queued_world_events(&mut self) -> (Vec<u32>, Option<u8>) {
        let Some(tables) = self.world_tables.clone() else {
            return (Vec::new(), None);
        };
        let mut ids = self.world.take_queue();
        let mut apply = None;
        for &id in &ids {
            if let Some(e) = tables.event(id)
                && let Some(set) = self.world.run_event_script(&e.script, true)
            {
                apply = Some(set);
            }
        }
        // qsort with FUN_14038da10 (priority byte); the C sort is not
        // stable, equal priorities do not occur in the installed table.
        ids.sort_by_key(|&id| tables.event(id).map_or(0, |e| e.priority));
        let mut events: Vec<u32> = Vec::new();
        for id in ids {
            if let Some(e) = tables.event(id)
                && e.event != 0
                && !events.contains(&e.event)
            {
                events.push(e.event);
            }
        }
        (events, apply)
    }

    /// The tail of FUN_14039afc0 that this module owns: the link table
    /// (FUN_1403f0ea0), FUN_1403a5540's world point, the triggered world
    /// events (FUN_1403df1b0) and FUN_1403f2190's world points.
    pub(crate) fn after_flag(&mut self, flag: u32) {
        if self.world_tables.is_some() {
            self.run_links();
        }
        if flag == 0x401e {
            self.world.touch_point(0xd);
        }
        if let Some(tables) = self.world_tables.clone() {
            let hold = self.test(HOLD_FLAG);
            for e in tables.events.iter().filter(|e| e.trigger == flag) {
                self.world.mark_event(&tables, e.id, hold);
            }
        }
        // FUN_1403f2190.
        let point = match flag {
            0x2023 => Some(32 + 9),
            0x2822 => Some(32 + 6),
            0x3012 => Some(19),
            0x401e => Some(16),
            0x8b8 => Some(32 + 15),
            0x1061 => Some(30),
            0x141f => Some(9),
            0x1822 | 0x1877 => Some(31),
            _ => None,
        };
        if let Some(p) = point {
            self.world.touch_point(p);
        }
    }

    fn condition(&self, c: Condition) -> bool {
        match c.kind {
            1 => self.test(c.id as u32),
            2 => self.world.event_seen(c.id as u32),
            3 => self.world.menu_flag(c.id as u32),
            _ => false,
        }
    }

    /// FUN_1403f0ea0: every `link` record whose result does not hold yet is
    /// set once `need` of its conditions hold.
    pub(crate) fn run_links(&mut self) {
        let Some(tables) = self.world_tables.clone() else {
            return;
        };
        for link in &tables.links {
            if self.condition(link.result) {
                continue;
            }
            let held = link
                .conditions
                .iter()
                .filter(|&&c| self.condition(c))
                .count() as i32;
            if link.need > held {
                continue;
            }
            match link.result.kind {
                1 => self.set(link.result.id as u32),
                2 => {
                    let hold = self.test(HOLD_FLAG);
                    self.world.mark_event(&tables, link.result.id as u8, hold);
                }
                3 => self.set_menu_flag(link.result.id as u32),
                _ => {}
            }
        }
    }

    /// FUN_1403e3930 (progress op 8): sets a menu flag, then the link table.
    pub fn set_menu_flag(&mut self, flag: u32) {
        set_bit(&mut self.world.menu_flags, flag);
        self.run_links();
    }

    /// FUN_1403e3910 (progress op 9).
    pub fn clear_menu_flag(&mut self, flag: u32) {
        clear_bit(&mut self.world.menu_flags, flag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn op(op: u16, args: &[u32]) -> ScriptOp {
        ScriptOp {
            op,
            args: args.to_vec(),
        }
    }

    fn event(
        id: u8,
        kind: u8,
        priority: u8,
        trigger: u32,
        ev: u32,
        script: Vec<ScriptOp>,
    ) -> WorldEvent {
        WorldEvent {
            id,
            kind,
            priority,
            trigger,
            event: ev,
            script,
        }
    }

    #[test]
    fn record_state_rules() {
        let mut r = WorldRecord {
            flags: 3,
            ..Default::default()
        };
        r.set_state(2);
        assert_eq!((r.state, r.flags), (2, 2));
        r.set_state(3);
        assert_eq!(r.state, 3);
        r.set_state(3);
        assert_eq!(r.state, 4);
        r.set_state(1);
        assert_eq!(r.state, 1);
    }

    #[test]
    fn event_script_ops() {
        let mut s = WorldState::default();
        let apply = s.run_event_script(
            &[
                op(3, &[2, 4]),
                op(5, &[4]),
                op(4, &[4]),
                op(6, &[8]),
                op(7, &[2]),
                op(10, &[21]),
                op(8, &[1, 2]),
                op(12, &[33]),
                op(15, &[3]),
            ],
            true,
        );
        assert_eq!(s.records[2].state, 3);
        assert_eq!((s.records[4].state, s.records[4].flags), (2, 0));
        assert_eq!(s.records[8].flags, 1);
        assert_eq!(s.symbol_slot(21), 2);
        assert_eq!(s.symbol_slot(1), 0);
        assert!(s.episodes[1] & 2 != 0 && s.episodes_new[1] & 2 != 0);
        s.run_event_script(&[op(13, &[33])], false);
        assert!(s.episodes[1] & 2 != 0 && s.episodes_new[1] & 2 == 0);
        assert_eq!((apply, s.draw_group_set), (Some(3), 3));
        assert_eq!(s.run_event_script(&[op(15, &[7])], false), None);
        assert_eq!(s.draw_group_set, 7);
    }

    fn progress_with(tables: WorldTables) -> Progress {
        let mut p = Progress::new(vec![None; 19]);
        p.set_world_tables(Arc::new(tables));
        p
    }

    #[test]
    fn flags_queue_world_events_and_hold_kind_one() {
        let mut p = progress_with(WorldTables {
            events: vec![
                event(1, 1, 5, 0x8ad, 0x1f6, vec![]),
                event(2, 0, 2, 0x8ad, 0x1f7, vec![op(3, &[2])]),
                event(3, 0, 9, 0x8ae, 0x1f7, vec![op(15, &[4])]),
                event(0x25, 0, 1, 0x8af, 0, vec![]),
            ],
            ..Default::default()
        });
        p.set(HOLD_FLAG);
        p.set(0x8ad);
        // Both marked; the kind-1 event is held back.
        assert!(p.world.event_seen(1) && p.world.event_seen(2));
        assert_eq!(p.world.queue, vec![2]);
        p.set(0x8ae);
        // Marked once only.
        p.set(0x8ad);
        assert_eq!(p.world.queue, vec![2, 3]);
        let (events, apply) = p.play_queued_world_events();
        assert_eq!(events, vec![0x1f7]);
        assert_eq!(apply, Some(4));
        assert_eq!(p.world.records[2].state, 2);
        assert!(p.world.queue.is_empty());
        // Event 0x25 is gated by event 0x73.
        p.world.event_bits[3] |= 1 << 0x13;
        p.set(0x8af);
        assert!(!p.world.event_seen(0x25));
    }

    #[test]
    fn link_table_sets_results_and_recurses() {
        let c = |kind, id| Condition { kind, id };
        let mut conditions = [Condition::default(); 13];
        conditions[0] = c(1, 0x200b);
        conditions[1] = c(1, 0x140f);
        let mut second = [Condition::default(); 13];
        second[0] = c(1, 0x1834);
        let mut p = progress_with(WorldTables {
            links: vec![
                Link {
                    result: c(1, 0x1834),
                    conditions,
                    need: 2,
                },
                Link {
                    result: c(3, 5),
                    conditions: second,
                    need: 1,
                },
            ],
            ..Default::default()
        });
        p.set(0x200b);
        assert!(!p.test(0x1834));
        p.set(0x140f);
        assert!(p.test(0x1834));
        assert!(p.world.menu_flag(5));
    }

    #[test]
    fn world_points_landing_grid_and_default_landing() {
        let tables = WorldTables {
            landing_cells: vec![
                LandingCell {
                    point: 0,
                    world: 7,
                    cell: 1,
                    picture: 0,
                },
                LandingCell {
                    point: 1,
                    world: 7,
                    cell: 3,
                    picture: 1,
                },
                LandingCell {
                    point: 2,
                    world: 7,
                    cell: 2,
                    picture: 2,
                },
            ],
            world_points: vec![
                WorldPoint {
                    world: 7,
                    area: 2,
                    entrance: 0x63,
                },
                WorldPoint {
                    world: 7,
                    area: 6,
                    entrance: 0x63,
                },
                WorldPoint {
                    world: 7,
                    area: 8,
                    entrance: 0x63,
                },
            ],
            default_landings: vec![
                DefaultLanding {
                    world: 7,
                    set: 1,
                    area: 0,
                    entrance: 0,
                },
                DefaultLanding {
                    world: 7,
                    set: 2,
                    area: 0x34,
                    entrance: 0,
                },
            ],
            ..Default::default()
        };
        let mut p = progress_with(tables.clone());
        assert_eq!(p.landing_grid(&tables, 7), LandingGrid::default());
        p.world.enter_area_with_world_point(&tables, 7, 6);
        p.world.touch_point(0);
        let grid = p.landing_grid(&tables, 7);
        assert_eq!(grid.placed, 2);
        assert_eq!(grid.cells.len(), 3);
        assert!(grid.cells[1].is_none());
        assert_eq!(grid.cells[2].unwrap().target, Target::at(7, 6, 0x63));
        assert_eq!(p.default_landing(&tables, 7), Target::NONE);
        p.world.records[7].landing_set = 1;
        assert_eq!(p.default_landing(&tables, 7), Target::at(7, 0, 0));
        p.world.records[7].landing_set = 2;
        assert_eq!(p.default_landing(&tables, 7).spawn, Some(0x34));
    }

    #[test]
    fn world_map_entry_and_special_points() {
        assert_eq!(world_map_entry(2), Target::at(WORLD_MAP, 0, 1));
        assert_eq!(world_map_entry(17), Target::at(WORLD_MAP, 0, 15));
        assert_eq!(world_map_entry(3), Target::at(WORLD_MAP, 0, 0));
        let mut p = Progress::new(vec![None; 19]);
        p.set(0x401e);
        assert!(p.world.point_touched(0xd) && p.world.point_touched(16));
        p.set(0x2023);
        assert!(p.world.point_touched(41));
    }

    #[test]
    fn open_episodes_stop_at_the_next_world() {
        let ep = |bit, world, index| Episode {
            bit,
            world,
            index,
            title: 0,
            help: 0,
        };
        let tables = WorldTables {
            episodes: vec![ep(32, 2, 1), ep(33, 2, 2), ep(2, 4, 1), ep(40, 2, 3)],
            ..Default::default()
        };
        let mut s = WorldState::default();
        s.run_event_script(&[op(12, &[33, 2, 40])], false);
        let open: Vec<u16> = s.open_episodes(&tables, 2).map(|e| e.bit).collect();
        assert_eq!(open, vec![33]);
        assert_eq!(s.open_episodes(&tables, 4).count(), 1);
    }
}
