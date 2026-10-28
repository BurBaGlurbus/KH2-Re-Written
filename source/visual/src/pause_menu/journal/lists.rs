//! Missions (states 10 from Notes, 0x23 from Collection; FUN_14033c9f0 /
//! FUN_14033c170) and Minigames (0xb, 0x24; FUN_14033b500 / FUN_14033ad00):
//! single-row scroll lists of ten text rows under world and category
//! headers. The list cursor is the first visible entry; up / down step it
//! by one, L1 / R1 by ten.
use super::*;

pub(super) const MISSIONS_NOTES: u8 = 10;
pub(super) const MISSIONS_COLLECTION: u8 = 0x23;
pub(super) const MINIGAMES_NOTES: u8 = 0xb;
pub(super) const MINIGAMES_COLLECTION: u8 = 0x24;
/// The shared panel (ctx (60, 92), pad 28) and scroll track.
const PANEL: usize = 101;
const TRACK: usize = 109;
/// A blank row, the world header, the category header, a mission row
/// (open, done) and a minigame row.
const BLANK: usize = 2;
const WORLD_HEADER: usize = 102;
const CATEGORY_HEADER: usize = 104;
const MISSION_ROW: [usize; 2] = [106, 107];
const MINIGAME_ROW: usize = 108;
/// NEW labels: pulsing, then static (always static here: the visible rows
/// are marked seen before they are drawn).
const MARK: [usize; 2] = [57, 58];
const PRIO_PANEL: u32 = 0x755c;
const PRIO_NEW: u32 = 0x755e;
const ROWS: usize = 10;
const PITCH: f32 = 28.;
/// The rows' RGB (0x60000008: near black, alpha from the animation).
const TEXT_COLOUR: u32 = 0x6000_0008;
const HELP_MISSIONS: u16 = 0x48f6;
const HELP_MINIGAMES: u16 = 0x48f7;
/// "–––" for a minigame row not played yet, and "Round{u72}".
const NO_VALUE: u16 = 0x4b71;
const ROUND: u16 = 0x4b80;
/// The minigame record's states (save+0x3db4 + 8 * i): 4 is a time, 2 a
/// round.
const TIME: u8 = 4;
const ROUNDS: u8 = 2;

/// One listed entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    /// A world header.
    World(u16),
    /// A category header of a world (the heading message).
    Category(u16),
    /// A `ques` or `mini` row.
    Row(usize),
}

#[derive(Default)]
pub(super) struct Page {
    entries: Vec<Entry>,
    rows: Vec<Id>,
    values: Vec<Id>,
    marks: Vec<Id>,
    /// The dummy list item the hand and the cursor ride on.
    carrier: Option<Id>,
    /// Rows seen so far and when the journal opened (save+0x4c38 + 0x94 /
    /// 0x88).
    pub seen_quests: HashSet<u16>,
    pub seen_quests_at_open: HashSet<u16>,
    pub seen_minis: HashSet<u16>,
    pub seen_minis_at_open: HashSet<u16>,
}

/// FUN_1401575d0: frames to (hours, minutes, seconds, hundredths).
pub(super) fn frame_time(frames: u32) -> (u32, u32, u32, u32) {
    (
        frames / 216_000,
        frames % 216_000 / 3600,
        frames % 3600 / 60,
        ((frames % 60) as f32 * 1.666_666_6) as u32,
    )
}

/// FUN_14033c630 over the save's story flags, menu flags and minigame
/// records.
pub(super) fn quest_status(
    q: &data::journal_table::Quest,
    show_all: bool,
    story: impl Fn(u16) -> bool,
    menu: impl Fn(u16) -> bool,
    record: impl Fn(u16) -> (u8, u32),
) -> u8 {
    let hidden = if show_all { 3 } else { 0 };
    let f = q.flags;
    let available = if f & 1 != 0 {
        story(q.available)
    } else {
        menu(q.available)
    };
    if !available {
        return hidden;
    }
    let (state, value) = record(q.record);
    if f & 4 != 0 && state == 0 {
        return hidden;
    }
    if f & 0x10 != 0 {
        return if story(q.done) { 2 } else { 1 };
    }
    if f & 0x20 != 0 {
        return if menu(q.done) { 2 } else { 1 };
    }
    if f & 0x1000 != 0 {
        return (state != 0) as u8 + 1;
    }
    if f & 0x2000 != 0 {
        return if state != 0 && state != 1 { 2 } else { 1 };
    }
    let mut r = 1;
    if state != 0 && state != 1 {
        let mut v = value;
        if state == TIME {
            let (h, m, s, cs) = frame_time(v);
            v = (h * 60 + m) * 60 + s + (cs != 0) as u32;
        }
        if f & 0x100 != 0 {
            r = if (q.threshold as u32) < v { 1 } else { 2 };
        } else if f & 0x200 != 0 {
            r = if q.threshold as u32 <= v { 2 } else { 1 };
        }
    }
    r
}

impl Journal {
    fn record(&self, index: u16) -> (u8, u32) {
        self.data
            .minigame_records
            .get(index as usize)
            .copied()
            .unwrap_or((0, 0))
    }

    /// FUN_1403e38f0: a menu flag.
    fn menu_flag(&self, f: u16) -> bool {
        self.data.menu_flags.contains(&f)
    }

    /// FUN_14033c630: 0 hidden, 1 open, 2 done, 3 only through show-all.
    pub(super) fn quest_status(&self, q: &data::journal_table::Quest) -> u8 {
        quest_status(
            q,
            self.show_all(),
            |f| self.flag(f),
            |f| self.menu_flag(f),
            |r| self.record(r),
        )
    }

    /// FUN_14033d370: the Missions option shows (some row of the world,
    /// or of any world, has a status).
    pub(super) fn missions_available(&self, world: Option<i32>) -> bool {
        self.show_all()
            || self
                .data
                .quests
                .iter()
                .any(|q| world.is_none_or(|w| q.world as i32 == w) && self.quest_status(q) != 0)
    }

    /// FUN_14033bdd0: the Minigames option shows.
    pub(super) fn minigames_available(&self, world: Option<i32>) -> bool {
        self.show_all()
            || self
                .data
                .minigames
                .iter()
                .any(|m| world.is_none_or(|w| m.world as i32 == w) && self.record(m.record).0 > 1)
    }

    /// FUN_14033d5a0: a row of the heading is open or done.
    fn heading_known(&self, heading: u16) -> bool {
        self.data
            .quests
            .iter()
            .any(|q| q.heading == heading && matches!(self.quest_status(q), 1 | 2))
    }

    /// The entries (FUN_14033c9f0 / FUN_14033abe0).
    fn list_entries(&self, missions: bool) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        let all = self.show_all();
        if missions {
            let mut last: Option<(u16, u16)> = None;
            for (k, q) in self.data.quests.iter().enumerate() {
                if self.quest_status(q) == 0 {
                    continue;
                }
                if last.is_none_or(|(w, _)| w != q.world) {
                    out.push(Entry::World(q.world));
                }
                if last.is_none_or(|(_, h)| h != q.heading) {
                    out.push(Entry::Category(q.heading));
                }
                out.push(Entry::Row(k));
                last = Some((q.world, q.heading));
            }
        } else {
            let mut last_world: Option<u16> = None;
            for (k, m) in self.data.minigames.iter().enumerate() {
                if self.record(m.record).0 <= 1 && !all {
                    continue;
                }
                if last_world != Some(m.world) {
                    out.push(Entry::World(m.world));
                }
                out.push(Entry::Row(k));
                last_world = Some(m.world);
            }
        }
        out
    }

    fn masked() -> Vec<u8> {
        Self::locked_text()
    }

    /// FUN_14033d290 / FUN_14033bd00: the visible rows count as seen.
    fn list_seen(&mut self, missions: bool) {
        let first = self.list.cursor;
        let shown: Vec<Entry> = self
            .lists
            .entries
            .iter()
            .skip(first)
            .take(ROWS)
            .copied()
            .collect();
        for e in shown {
            let Entry::Row(k) = e else { continue };
            if missions {
                if self.quest_status(&self.data.quests[k]) != 3 {
                    self.lists.seen_quests.insert(k as u16);
                }
            } else if self.record(self.data.minigames[k].record).0 != 0 {
                self.lists.seen_minis.insert(k as u16);
            }
        }
    }

    /// FUN_14033d000 / FUN_14033b940: the ten rows.
    fn list_rows(&mut self, seq: &Sequence, missions: bool) {
        self.list_seen(missions);
        let first = self.list.cursor;
        for j in 0..ROWS {
            let (Some(&row), Some(&value), Some(&mark)) = (
                self.lists.rows.get(j),
                self.lists.values.get(j),
                self.lists.marks.get(j),
            ) else {
                continue;
            };
            let mut group = BLANK;
            let mut name = Vec::new();
            let mut value_text = Vec::new();
            let mut mark_group = BLANK;
            match self.lists.entries.get(first + j).copied() {
                Some(Entry::World(w)) => {
                    group = WORLD_HEADER;
                    name = if self.world_unlocked(w as u8) {
                        self.world_name2(w as u8)
                    } else {
                        Self::masked()
                    };
                }
                Some(Entry::Category(h)) => {
                    group = CATEGORY_HEADER;
                    name = if self.heading_known(h) {
                        self.msg(h)
                    } else {
                        Self::masked()
                    };
                }
                Some(Entry::Row(k)) if missions => {
                    let q = self.data.quests[k];
                    let status = self.quest_status(&q);
                    group = MISSION_ROW[(status == 2) as usize];
                    if status == 3 {
                        name = Self::masked();
                    } else {
                        name = self.msg(q.text);
                        let new = !self.lists.seen_quests_at_open.contains(&(k as u16));
                        if new && status != 0 {
                            mark_group = MARK[1];
                        }
                    }
                }
                Some(Entry::Row(k)) => {
                    let m = self.data.minigames[k];
                    group = MINIGAME_ROW;
                    let (state, value) = self.record(m.record);
                    if state < 2 {
                        name = Self::masked();
                        value_text = self.msg(NO_VALUE);
                    } else {
                        name = self.msg(m.name);
                        value_text = self.minigame_value(m.label, state, value);
                        if !self.lists.seen_minis_at_open.contains(&(k as u16)) {
                            mark_group = MARK[1];
                        }
                    }
                }
                None => {}
            }
            self.sprites.set(seq, row, group, None);
            self.set_text(Some(row), name);
            self.set_text(Some(value), value_text);
            self.sprites.set(seq, mark, mark_group, None);
        }
    }

    /// The minigame value text (US): a time as "Time:{u72}" and m'ss"cs, a
    /// round as "Round{u72}" and the number, otherwise the number and the
    /// label.
    fn minigame_value(&self, label: u16, state: u8, value: u32) -> Vec<u8> {
        use crate::pause_menu::encode_number;
        match state {
            TIME => {
                let (_, m, s, cs) = frame_time(value);
                let mut v = self.msg(label);
                v.extend(encode_number(&format!("{m:02}'{s:02}\"{cs:02}")));
                v
            }
            ROUNDS => {
                let mut v = self.msg(ROUND);
                v.extend(encode_number(&value.to_string()));
                v
            }
            _ => {
                let mut v = encode_number(&value.to_string());
                v.extend(self.msg(label));
                v
            }
        }
    }

    /// FUN_14033c9f0 / FUN_14033b500.
    pub(super) fn build_list(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        let missions = matches!(self.state, MISSIONS_NOTES | MISSIONS_COLLECTION);
        self.reset_list();
        self.no_cursor = true;
        self.cat.detail = false;
        self.category_frame(seq);
        let c = self.container;
        let panel = self.page_sprite(seq, PRIO_PANEL, PANEL, c, None);
        self.page_sprite(seq, PRIO_PANEL, TRACK, c, None);
        let size = Self::size(seq, MINIGAME_ROW);
        let value_x = seq.groups[if missions {
            MISSION_ROW[0]
        } else {
            MINIGAME_ROW
        }]
        .light_position_x as f32;
        self.lists.rows.clear();
        self.lists.values.clear();
        self.lists.marks.clear();
        for i in 0..ROWS {
            let mut t = Self::text(Vec::new(), size);
            t.group_position = false;
            t.colour = TEXT_COLOUR;
            let row = self.page_sprite(seq, BASE, BLANK, panel, Some(t));
            self.sprites.get_mut(row).offset = [0., PITCH * i as f32];
            let mut v = Self::text(Vec::new(), size);
            v.group_position = false;
            v.align = 1;
            v.colour = TEXT_COLOUR;
            let value = self.page_sprite(seq, BASE, BLANK, row, Some(v));
            self.sprites.get_mut(value).offset = [value_x, 0.];
            // Mission marks ride the row, minigame marks the value.
            let m = self.page_sprite(
                seq,
                PRIO_NEW,
                BLANK,
                if missions { row } else { value },
                None,
            );
            if missions {
                self.sprites.get_mut(m).offset = [value_x, 0.];
            }
            self.lists.rows.push(row);
            self.lists.values.push(value);
            self.lists.marks.push(m);
        }
        self.lists.entries = self.list_entries(missions);
        let n = self.lists.entries.len();
        let total = if n > ROWS { n - ROWS + 1 } else { 1 };
        // The Notes pages start on the current world's header.
        let start = if matches!(self.state, MISSIONS_NOTES | MINIGAMES_NOTES) {
            self.lists
                .entries
                .iter()
                .position(|e| matches!(e, Entry::World(w) if *w as i32 == self.world))
                .unwrap_or(0)
        } else {
            0
        };
        let carrier = self.page_sprite(seq, BASE, BLANK, panel, None);
        self.lists.carrier = Some(carrier);
        self.list = MenuList::new(vec![carrier], start.min(total - 1));
        self.list.total = total;
        // No decide; page keys when it scrolls.
        self.list.mask = 0x63;
        if total > 1 {
            self.list.page = Some(ROWS);
            self.list.mask |= 0x3000;
        }
        self.list_rows(seq, missions);
        self.show_help(
            seq,
            if missions {
                HELP_MISSIONS
            } else {
                HELP_MINIGAMES
            },
        );
    }

    /// FUN_14033c170 / FUN_14033ad00 (not leaving).
    pub(super) fn list_page(&mut self, layout_: &Layout, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        let missions = matches!(self.state, MISSIONS_NOTES | MISSIONS_COLLECTION);
        let (input, moved) = self.input(pad);
        if self.substate == 2 {
            self.leave(layout_, input, moved, false);
            return false;
        }
        self.settle(seq, input, moved, true);
        if let Some(s) = sound(input, moved, true) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            self.list_rows(seq, missions);
        }
        match input {
            In::Start => self.exit = true,
            In::Cancel => {
                self.pill_out(seq);
                self.hide_help(seq);
                self.slide(seq, 1, 1);
                self.pending = Some(if matches!(self.state, MISSIONS_NOTES | MINIGAMES_NOTES) {
                    NOTES_PAGE
                } else {
                    COLLECTION_PAGE
                });
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::journal_table::Quest;

    fn quest(flags: u16, threshold: u16) -> Quest {
        Quest {
            world: 0,
            heading: 0,
            text: 0,
            flags,
            available: 0x8b0,
            record: 0xe,
            threshold,
            done: 0x109c,
        }
    }

    #[test]
    fn frames_split_into_the_clock_fields() {
        assert_eq!(frame_time(3138), (0, 0, 52, 30));
        assert_eq!(frame_time(216_000 + 3600 + 60), (1, 1, 1, 0));
    }

    #[test]
    fn a_mission_is_hidden_open_or_done_by_its_flags_and_record() {
        let all_on = |_: u16| true;
        let off = |_: u16| false;
        // Not available: hidden, or masked under show-all.
        let q = quest(0x0145, 14);
        assert_eq!(quest_status(&q, false, off, off, |_| (0, 0)), 0);
        assert_eq!(quest_status(&q, true, off, off, |_| (0, 0)), 3);
        // Needs a played record.
        assert_eq!(quest_status(&q, false, all_on, off, |_| (0, 0)), 0);
        // Time limit 14 s: 52 s is too slow, 12 s is done (state 4 = time).
        assert_eq!(quest_status(&q, false, all_on, off, |_| (4, 52 * 60)), 1);
        assert_eq!(quest_status(&q, false, all_on, off, |_| (4, 12 * 60)), 2);
        // A score needs the threshold.
        let s = quest(0x0245, 100);
        assert_eq!(quest_status(&s, false, all_on, off, |_| (3, 99)), 1);
        assert_eq!(quest_status(&s, false, all_on, off, |_| (3, 100)), 2);
        // Completed by a story flag.
        let d = quest(0x0011, 0);
        assert_eq!(quest_status(&d, false, |f| f == 0x8b0, off, |_| (0, 0)), 1);
        assert_eq!(quest_status(&d, false, all_on, off, |_| (0, 0)), 2);
    }
}
