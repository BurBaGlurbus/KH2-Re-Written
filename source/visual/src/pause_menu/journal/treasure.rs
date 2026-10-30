//! Treasures (states 7 / 0x1a) and Maps (9 / 0x22): one grid page of 9
//! rows x 8 cells per the journal worlds (FUN_1403363e0 builds,
//! FUN_140334690 runs, FUN_140336170 moves, FUN_1403377c0 refreshes).
//! There is no decide action: the cursor peeks, cancel goes back.
use super::*;

const FRAME_GROUP: usize = 59;
const TRACK: usize = 69;
const LABEL: usize = 72;
const NOTHING: usize = 2;
const MARK: [usize; 2] = [63, 64];
const TOTAL_PILL: usize = 60;
const COUNTER: usize = 149;
const CAPTION: usize = 148;
const CURSOR: [usize; 2] = [65, 66];
/// (open, closed) cell groups: chests, maps.
const CELLS: [(usize, usize); 2] = [(74, 73), (62, 61)];
const PRIO_PANEL: u32 = 0x755c;
const PRIO_MARK: u32 = 0x755e;
const PRIO_CURSOR_FRAME: u32 = 0x755e;
const ROWS: usize = 9;
const COLS: usize = 8;
const PITCH: f32 = 28.;
const SIZE: f32 = 18.;
const HELP: [u16; 2] = [0x48f4, 0x48f5];
const NOTHING_TEXT: [u16; 2] = [0x4a0f, 0x4a12];
const TOTAL_TEXT: [u16; 2] = [0x4a11, 0x4a13];

/// One listed chest or map.
#[derive(Clone, Copy, Debug)]
struct Entry {
    world: u8,
    opened: bool,
    /// The `trsr` row (the NEW bit).
    index: u16,
    item: u16,
}

/// A world's block of rows.
#[derive(Clone, Copy, Debug)]
struct Block {
    world: u8,
    first_entry: usize,
    n: usize,
    rows: usize,
    first_row: usize,
}

#[derive(Default)]
pub(super) struct Page {
    maps: bool,
    entries: Vec<Entry>,
    blocks: Vec<Block>,
    total_rows: usize,
    /// Top row S and the cursor cell C = r * 8 + c.
    top: usize,
    cell: usize,
    opened_count: usize,
    p0: Option<Id>,
    cells: Vec<Id>,
    labels: Vec<Id>,
    subs: Vec<Id>,
    marks: Vec<Id>,
    caption: Option<Id>,
    frame: Option<Id>,
    counter: Option<Id>,
    frame_made: bool,
    /// The seen rows (save+0x4d24 chests, +0x4d64 maps), now and when the
    /// journal opened.
    pub(super) seen_chests: HashSet<u16>,
    pub(super) seen_maps: HashSet<u16>,
    pub(super) seen_chests_at_open: HashSet<u16>,
    pub(super) seen_maps_at_open: HashSet<u16>,
}

impl Journal {
    /// The rows behind this page's NEW mark in world `w`: available, and for
    /// the first number of each pair not seen when the journal opened.
    fn treasure_rows(&self, maps: bool, w: u8) -> Vec<u16> {
        let world = |t: &data::journal_table::Treasure| {
            data::journal_table::journal_world(t.game_world) == w as i8
        };
        if maps {
            self.data
                .map_rows()
                .filter(|(_, t)| world(t) && self.data.owned_items.contains(&t.item))
                .map(|(i, _)| i)
                .collect()
        } else {
            self.data
                .treasures
                .iter()
                .enumerate()
                .filter(|(_, t)| world(t) && self.data.chest_available(t))
                .map(|(k, _)| k as u16)
                .collect()
        }
    }

    /// FUN_140347c80 / FUN_140347360 (a row was new at the opening) and
    /// FUN_140347bb0 / FUN_140347290 (all of those are seen now).
    fn treasure_mark(&self, maps: bool, w: u8) -> Option<bool> {
        let (at_open, seen) = if maps {
            (&self.treasure.seen_maps_at_open, &self.treasure.seen_maps)
        } else {
            (
                &self.treasure.seen_chests_at_open,
                &self.treasure.seen_chests,
            )
        };
        let fresh: Vec<u16> = self
            .treasure_rows(maps, w)
            .into_iter()
            .filter(|r| !at_open.contains(r))
            .collect();
        (!fresh.is_empty()).then(|| fresh.iter().all(|r| seen.contains(r)))
    }

    fn treasure_world_unlocked(&self, w: u8) -> bool {
        self.data
            .worlds
            .iter()
            .find(|r| r.world == w as i8)
            .is_some_and(|r| self.flag(r.unlock))
    }

    fn treasure_block(&self, row: usize) -> Option<(Block, usize)> {
        self.treasure
            .blocks
            .iter()
            .find(|b| b.rows > 0 && row >= b.first_row && row < b.first_row + b.rows)
            .map(|b| (*b, row - b.first_row))
    }

    /// FUN_1403354f0: the cells of an absolute row.
    fn treasure_cells_in(&self, row: usize) -> usize {
        match self.treasure_block(row) {
            None => 0,
            Some((b, _)) if b.n == 0 => 1,
            Some((b, r)) if r + 1 == b.rows => match b.n % COLS {
                0 => COLS,
                k => k,
            },
            Some(_) => COLS,
        }
    }

    /// FUN_1403363e0.
    pub(super) fn build_treasure(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        let maps = matches!(self.state, 9 | 0x22);
        self.reset_list();
        self.treasure = Page {
            maps,
            seen_chests: std::mem::take(&mut self.treasure.seen_chests),
            seen_maps: std::mem::take(&mut self.treasure.seen_maps),
            seen_chests_at_open: std::mem::take(&mut self.treasure.seen_chests_at_open),
            seen_maps_at_open: std::mem::take(&mut self.treasure.seen_maps_at_open),
            ..Page::default()
        };
        self.cat.detail = false;
        let all = self.show_all();
        // FUN_1403363e0's entry tables (4.3).
        let mut entries = Vec::new();
        for (k, t) in self.data.treasures.iter().enumerate() {
            let item = self.data.item_names.get(&t.item);
            let is_map = item.is_some_and(|i| i.map);
            let world = data::journal_table::journal_world(t.game_world);
            if world < 0 {
                continue;
            }
            if maps {
                if !is_map {
                    continue;
                }
                let owned = self.data.owned_items.contains(&t.item);
                if owned || all {
                    entries.push(Entry {
                        world: world as u8,
                        opened: owned,
                        index: k as u16,
                        item: t.item,
                    });
                }
            } else if t.kind == 0 && t.flag & 0xf7ff != 0 && (self.flag(t.flag) || all) {
                entries.push(Entry {
                    world: world as u8,
                    opened: t.opened != 0 && self.data.opened_bits.contains(&t.opened),
                    index: k as u16,
                    item: t.item,
                });
            }
        }
        entries.sort_by_key(|e| (e.world, e.index));
        self.treasure.opened_count = entries.iter().filter(|e| e.opened).count();
        let mut blocks = Vec::new();
        let mut row = 0;
        for w in 0..15u8 {
            let first = entries.iter().position(|e| e.world == w).unwrap_or(0);
            let n = entries.iter().filter(|e| e.world == w).count();
            let mut rows = n.div_ceil(COLS);
            if n == 0 && self.treasure_world_unlocked(w) && !(!maps && w == 8) {
                rows = 1;
            }
            if rows > 0 {
                blocks.push(Block {
                    world: w,
                    first_entry: first,
                    n,
                    rows,
                    first_row: row,
                });
            }
            row += rows;
        }
        self.treasure.entries = entries;
        self.treasure.blocks = blocks;
        self.treasure.total_rows = row;
        // FUN_140334570: from Notes the list opens on the current world.
        let from_collection = matches!(self.state, 0x1a | 0x22);
        let total = self.treasure.total_rows;
        let vis = total.min(ROWS);
        if !from_collection && self.world >= 0 {
            let before: usize = self
                .treasure
                .blocks
                .iter()
                .filter(|b| (b.world as i32) < self.world)
                .map(|b| b.rows)
                .sum();
            if total - vis < before {
                self.treasure.top = total - vis;
                self.treasure.cell = (before - self.treasure.top) * COLS;
            } else {
                self.treasure.top = before;
            }
        }
        self.category_frame(seq);
        let c = self.container;
        let p0 = self.page_sprite(seq, PRIO_PANEL, FRAME_GROUP, c, None);
        self.treasure.p0 = Some(p0);
        let track = self.page_sprite(seq, PRIO_PANEL, TRACK, c, None);
        self.bar_in(seq, track, self.treasure.total_rows, ROWS);
        let g = &seq.groups[FRAME_GROUP];
        let at = [g.light_position_x as f32, g.text_position_y as f32];
        let mut t = Self::text(Vec::new(), Self::size(seq, CAPTION));
        t.group_position = false;
        t.keep_rgb = false;
        t.align = 2;
        let cap = self.page_sprite(seq, BASE, CAPTION, p0, Some(t));
        self.sprites.get_mut(cap).offset = at;
        self.treasure.caption = Some(cap);
        for i in 0..ROWS {
            let mut t = Self::text(Vec::new(), SIZE);
            t.group_position = false;
            t.keep_rgb = false;
            let l = self.page_sprite(seq, BASE, NOTHING, p0, Some(t));
            self.sprites.get_mut(l).offset = [0., PITCH * i as f32];
            self.treasure.labels.push(l);
            let mut t = Self::text(Vec::new(), SIZE);
            t.group_position = false;
            let s = self.page_sprite(seq, BASE, NOTHING, l, Some(t));
            self.sprites.get_mut(s).offset = [200., 0.];
            self.treasure.subs.push(s);
            let m = self.page_sprite(seq, PRIO_MARK, NOTHING, l, None);
            self.treasure.marks.push(m);
        }
        if all {
            let mut t = Self::text(
                self.msg(TOTAL_TEXT[maps as usize]),
                Self::size(seq, TOTAL_PILL),
            );
            t.group_position = false;
            t.keep_rgb = false;
            let total_label = self.page_sprite(seq, BASE, TOTAL_PILL, p0, Some(t));
            let mut t = Self::text(Vec::new(), Self::size(seq, COUNTER));
            t.align = 1;
            t.group_position = false;
            t.keep_rgb = false;
            let ctr = self.page_sprite(seq, BASE, COUNTER, total_label, Some(t));
            let lg = &seq.groups[TOTAL_PILL];
            self.sprites.get_mut(ctr).offset = [lg.light_position_x as f32, 0.];
            self.treasure.counter = Some(ctr);
        }
        for r in 0..ROWS {
            for col in 0..COLS {
                let id = self.page_sprite(seq, BASE, NOTHING, p0, None);
                self.sprites.get_mut(id).offset = [PITCH * col as f32, PITCH * r as f32];
                self.treasure.cells.push(id);
            }
        }
        // The cursor frame comes once the page is idle; the generic hand
        // and light attach to it as the page's only row.
        self.list = MenuList::new(Vec::new(), 0);
        self.no_cursor = false;
        self.no_light = false;
        self.show_help(seq, HELP[maps as usize]);
        self.treasure_refresh(seq);
    }

    /// FUN_140334860: the cursor frame.
    fn treasure_frame(&mut self, seq: &Sequence) {
        let Some(p0) = self.treasure.p0 else { return };
        let f = self
            .sprites
            .create(seq, PRIO_CURSOR_FRAME, ART, CURSOR[0], None, None);
        self.sprites.attach(f, p0);
        self.sprites.get_mut(f).die_with_parent = true;
        self.page.push(f);
        self.treasure.frame = Some(f);
        self.treasure.frame_made = true;
        self.list = MenuList::new(vec![f], 0);
        self.list.mask = 0;
        self.treasure_place_frame(seq);
    }

    fn treasure_place_frame(&mut self, seq: &Sequence) {
        let Some(f) = self.treasure.frame else { return };
        let (r, c) = (self.treasure.cell / COLS, self.treasure.cell % COLS);
        self.sprites.get_mut(f).offset = [PITCH * c as f32, PITCH * r as f32];
        let empty = self
            .treasure_block(self.treasure.top + r)
            .is_none_or(|(b, _)| b.n == 0);
        let group = CURSOR[empty as usize];
        if self.sprites.get(f).player.group != group {
            self.sprites.set(seq, f, group, None);
        }
    }

    /// FUN_1403377c0: marks, labels, cells and the caption.
    fn treasure_refresh(&mut self, seq: &Sequence) {
        let (s, cell) = (self.treasure.top, self.treasure.cell);
        let (r, c) = (cell / COLS, cell % COLS);
        self.bar_to(seq, s);
        // The world under the cursor counts as viewed.
        if let Some((b, _)) = self.treasure_block(s + r)
            && b.n > 0
        {
            // FUN_1403467a0 / FUN_1403464a0.
            let rows = self.treasure_rows(self.treasure.maps, b.world);
            let seen = if self.treasure.maps {
                &mut self.treasure.seen_maps
            } else {
                &mut self.treasure.seen_chests
            };
            seen.extend(rows);
        }
        let all = self.show_all();
        let maps = self.treasure.maps;
        for i in 0..ROWS {
            self.sprites
                .set(seq, self.treasure.labels[i], NOTHING, None);
            self.set_text(Some(self.treasure.labels[i]), Vec::new());
            self.set_text(Some(self.treasure.subs[i]), Vec::new());
            self.sprites.set(seq, self.treasure.marks[i], NOTHING, None);
            if let Some((b, row)) = self.treasure_block(s + i)
                && row == 0
            {
                let unlocked = self.treasure_world_unlocked(b.world);
                let name = self.world_name_by_index(b.world);
                let text = if unlocked { name } else { vec![0x49; 5] };
                self.sprites.set(seq, self.treasure.labels[i], LABEL, None);
                self.set_text(Some(self.treasure.labels[i]), text);
                if b.n == 0 {
                    let m = self.msg(NOTHING_TEXT[maps as usize]);
                    self.set_text(Some(self.treasure.subs[i]), m);
                } else if unlocked && let Some(viewed) = self.treasure_mark(maps, b.world) {
                    self.sprites
                        .set(seq, self.treasure.marks[i], MARK[viewed as usize], None);
                }
            }
        }
        // Cells.
        let (open, closed) = CELLS[maps as usize];
        for (k, &id) in self.treasure.cells.clone().iter().enumerate() {
            self.sprites.set(seq, id, NOTHING, None);
            let (row, col) = (k / COLS, k % COLS);
            if let Some((b, r_in)) = self.treasure_block(s + row)
                && b.n > 0
            {
                let e = r_in * COLS + col;
                if e < b.n {
                    let entry = self.treasure.entries[b.first_entry + e];
                    let g = if entry.opened { open } else { closed };
                    self.sprites.set(seq, id, g, None);
                }
            }
        }
        // The caption.
        let mut text = Vec::new();
        if let Some((b, r_in)) = self.treasure_block(s + r)
            && r_in * COLS + c < b.n
        {
            let entry = self.treasure.entries[b.first_entry + r_in * COLS + c];
            if let Some(info) = self.data.item_names.get(&entry.item) {
                text = if entry.opened {
                    let mut t = Vec::new();
                    if !maps && info.icon != 0 {
                        t.extend([0x09, info.icon]);
                    }
                    t.extend(&info.name);
                    t
                } else {
                    vec![0x49; 5]
                };
            }
        }
        self.set_text(self.treasure.caption, text);
        if all && let Some(ctr) = self.treasure.counter {
            let n = format!(
                "{}/{}",
                self.treasure.opened_count,
                self.treasure.entries.len()
            );
            self.set_text(Some(ctr), crate::pause_menu::encode_number(&n));
        }
        self.treasure_place_frame(seq);
        let _ = all;
    }

    /// The journal world's second name by its index.
    fn world_name_by_index(&self, w: u8) -> Vec<u8> {
        let Some(r) = self.data.worlds.iter().find(|r| r.world == w as i8) else {
            return Vec::new();
        };
        if self.flag(r.alt_flag) {
            self.msg(r.alt_names[1])
        } else {
            self.msg(r.names[1])
        }
    }

    /// FUN_140336170.
    fn treasure_move(&mut self, pad: &Pad) -> bool {
        let rep = pad.repeat | pad.trig;
        let fresh = |bit: u64| pad.trig & bit != 0;
        let t = self.treasure.total_rows;
        if t == 0 {
            return false;
        }
        let w = t.min(ROWS);
        let (s0, c0) = (self.treasure.top, self.treasure.cell);
        let (mut s, mut r, mut c) = (s0, c0 / COLS, c0 % COLS);
        let n_of = |this: &Self, row: usize| this.treasure_cells_in(row).max(1);
        if rep & bits::UP != 0 {
            if r > 0 {
                r -= 1;
            } else if s > 0 {
                s -= 1;
            } else if fresh(bits::UP) {
                r = w - 1;
                s = t - w;
            }
            c = c.min(n_of(self, s + r) - 1);
        } else if rep & bits::DOWN != 0 {
            if r < w - 1 {
                r += 1;
            } else if s + ROWS < t {
                s += 1;
            } else if fresh(bits::DOWN) {
                r = 0;
                s = 0;
            }
            c = c.min(n_of(self, s + r) - 1);
        } else if rep & bits::L1 != 0 {
            if s > w {
                s -= w;
            } else if s == 0 {
                if c != 0 {
                    c = 0;
                } else if fresh(bits::L1) {
                    r = w - 1;
                    s = t - w;
                    c = n_of(self, s + r) - 1;
                }
            } else {
                r = 0;
                s = 0;
            }
            c = c.min(n_of(self, s + r) - 1);
        } else if rep & bits::R1 != 0 {
            if s + w < t.saturating_sub(w) {
                s += w;
            } else {
                let old = s;
                s = t - w;
                if old < t - w {
                    r = w - 1;
                } else if c + 1 < n_of(self, s + r) {
                    c = n_of(self, s + r) - 1;
                } else if fresh(bits::R1) {
                    r = 0;
                    s = 0;
                    c = 0;
                }
            }
            c = c.min(n_of(self, s + r) - 1);
        } else if rep & bits::LEFT != 0 {
            let n = n_of(self, s + r);
            c = if c != 0 { c } else { n } - 1;
        } else if rep & bits::RIGHT != 0 {
            let n = n_of(self, s + r);
            c = if c + 1 < n { c + 1 } else { 0 };
        }
        let new_cell = r * COLS + c;
        if (s, new_cell) == (s0, c0) {
            return false;
        }
        self.treasure.top = s;
        self.treasure.cell = new_cell;
        true
    }

    /// FUN_140334690.
    pub(super) fn treasure_page(
        &mut self,
        layout_: &Layout,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        let key = pad.trig & 0xc1fc != 0;
        if self.substate == 2 {
            let input = if key { In::Row(0) } else { In::None };
            self.leave(layout_, input, false, false);
            return false;
        }
        self.settle(seq, if key { In::Row(0) } else { In::None }, false, true);
        if self.substate == 1 && !self.treasure.frame_made {
            self.treasure_frame(seq);
        }
        if self.substate != 1 {
            return false;
        }
        if (pad.repeat | pad.trig) & 0xc0f0 != 0 && self.treasure_move(pad) {
            sounds.push(SOUND_CURSOR);
            self.treasure_refresh(seq);
            self.light.retarget();
            return true;
        }
        if pad.trig & bits::CANCEL != 0 {
            sounds.push(SOUND_CLOSE);
            self.pill_out(seq);
            self.hide_help(seq);
            self.slide(seq, 1, 1);
            self.pending = Some(if matches!(self.state, 0x1a | 0x22) {
                COLLECTION_PAGE
            } else {
                NOTES_PAGE
            });
            self.substate = 2;
        } else if pad.trig & bits::START != 0 {
            sounds.push(SOUND_CLOSE);
            self.exit = true;
        }
        false
    }
}
