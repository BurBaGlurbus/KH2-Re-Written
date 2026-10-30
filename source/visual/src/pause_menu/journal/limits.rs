//! Limits (Collection option 9): the list (state 0x25, FUN_140338c50 /
//! FUN_140337cc0) of the limits the party has used and the detail page of
//! one (state 0x26, FUN_1403386a0 / FUN_140337ac0). A limit is owned once
//! its use counter (save+0x3d48) is above zero; the list shows only owned
//! limits, or all of them with masked names under "show everything".
use super::*;

pub(super) const LIST: u8 = 0x25;
pub(super) const DETAIL: u8 = 0x26;
/// The list panel (ctx (88, 100), light X 355) and its scroll track; the
/// detail's panel (shared with the Ansem report reader), track and title.
const LIST_PANEL: usize = 110;
const LIST_TRACK: usize = 111;
const DETAIL_PANEL: usize = 97;
const DETAIL_TRACK: usize = 100;
const DETAIL_TITLE: usize = 99;
/// The row group, and the NEW labels: pulsing, then static once seen.
const ROW: usize = 2;
const MARK: [usize; 2] = [57, 58];
const PRIO_PANEL: u32 = 0x755c;
const PRIO_NEW: u32 = 0x755e;
const ROWS: usize = 10;
const PITCH: f32 = 26.;
/// Detail lines shown at once; the extra spacing is pad - spacing - size.
const LINES: usize = 9;
const LINE_EXTRA: f32 = 6.;
/// The value texts' RGB (0x60000008: near black, alpha from the animation).
const VALUE_COLOUR: u32 = 0x6000_0008;
/// "{u72}Hits" after the use counter.
const HITS: u16 = 0x4b06;
const HELP_LIST: u16 = 0x48f8;
const HELP_DETAIL: u16 = 0x4b00;

#[derive(Default)]
pub(super) struct Page {
    /// The `limi` rows listed.
    entries: Vec<usize>,
    rows: Vec<Id>,
    values: Vec<Id>,
    marks: Vec<Id>,
    /// First entry shown.
    scroll: usize,
    /// The entry the detail page shows.
    cur: usize,
    /// The detail's description lines window.
    desc: Option<Id>,
    lines: Vec<Vec<u8>>,
    top: usize,
    /// Rows seen so far and when the journal opened (save+0x4c38 + 0x84).
    pub seen: HashSet<u16>,
    pub seen_at_open: HashSet<u16>,
}

impl Journal {
    /// FUN_1403451f0: the limit has been used.
    fn limit_owned(&self, row: usize) -> bool {
        self.data.limit_counts.get(row).is_some_and(|&n| n > 0)
    }

    /// The use counter and "Hits" (FUN_1403e8150, message 0x4b06).
    fn limit_value(&self, row: usize) -> Vec<u8> {
        let n = self.data.limit_counts.get(row).copied().unwrap_or(0);
        let mut v = crate::pause_menu::encode_number(&n.to_string());
        v.extend(self.msg(HITS));
        v
    }

    /// FUN_140346e20 / FUN_140346ed0: the NEW label of an owned limit not
    /// seen when the journal opened.
    fn limit_mark(&self, row: usize) -> Option<usize> {
        let new = self.limit_owned(row) && !self.limits.seen_at_open.contains(&(row as u16));
        new.then(|| MARK[self.limits.seen.contains(&(row as u16)) as usize])
    }

    /// FUN_1403392b0: the visible rows' names, values and marks.
    fn limit_rows(&mut self, seq: &Sequence) {
        self.bar_to(seq, self.limits.scroll);
        for j in 0..ROWS {
            let k = self.limits.scroll + j;
            let (Some(&row), Some(&value), Some(&mark)) = (
                self.limits.rows.get(j),
                self.limits.values.get(j),
                self.limits.marks.get(j),
            ) else {
                continue;
            };
            let (name, value_text, group) = match self.limits.entries.get(k).copied() {
                Some(r) if self.limit_owned(r) => (
                    self.msg(self.data.limits[r].name),
                    self.limit_value(r),
                    self.limit_mark(r).unwrap_or(ROW),
                ),
                Some(r) => (Self::locked_text(), self.limit_value(r), ROW),
                None => (Vec::new(), Vec::new(), ROW),
            };
            self.set_text(Some(row), name);
            self.set_text(Some(value), value_text);
            self.sprites.set(seq, mark, group, None);
        }
    }

    /// FUN_1403391b0: the row under the cursor is seen.
    fn limit_view(&mut self, seq: &Sequence) {
        let k = self.list.cursor;
        let Some(&r) = self.limits.entries.get(k) else {
            return;
        };
        if !self.limit_owned(r) {
            return;
        }
        if self.limits.seen.insert(r as u16)
            && let Some(&m) = self.limits.marks.get(k.saturating_sub(self.limits.scroll))
            && !self.limits.seen_at_open.contains(&(r as u16))
        {
            self.sprites.set(seq, m, MARK[1], None);
        }
    }

    /// Scrolls the 10-row window to hold the cursor.
    fn limit_window(&mut self, seq: &Sequence) {
        let n = self.list.total;
        let c = self.list.cursor;
        let mut s = self.limits.scroll;
        if c < s {
            s = c;
        } else if c >= s + ROWS {
            s = c + 1 - ROWS;
        }
        s = s.min(n.saturating_sub(ROWS));
        if s != self.limits.scroll {
            self.limits.scroll = s;
            self.limit_rows(seq);
        }
        self.scroll = s;
    }

    /// FUN_140338c50.
    pub(super) fn build_limits_list(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_light = true;
        self.cat.detail = false;
        self.category_frame(seq);
        let c = self.container;
        let panel = self.page_sprite(seq, PRIO_PANEL, LIST_PANEL, c, None);
        let track = self.page_sprite(seq, PRIO_PANEL, LIST_TRACK, c, None);
        let all = self.show_all();
        self.limits.entries = (0..self.data.limits.len())
            .filter(|&r| self.limit_owned(r) || all)
            .collect();
        let n = self.limits.entries.len();
        self.bar_in(seq, track, n.max(ROWS), ROWS);
        self.limits.rows.clear();
        self.limits.values.clear();
        self.limits.marks.clear();
        let value_x = seq.groups[LIST_PANEL].light_position_x as f32;
        for i in 0..ROWS.min(n) {
            let mut t = Self::text(Vec::new(), Self::size(seq, LIST_PANEL));
            t.group_position = false;
            t.keep_rgb = false;
            let r = self.page_sprite(seq, BASE, ROW, panel, Some(t));
            self.sprites.get_mut(r).offset = [0., PITCH * i as f32];
            let mut v = Self::text(Vec::new(), Self::size(seq, LIST_PANEL));
            v.group_position = false;
            v.align = 1;
            v.colour = VALUE_COLOUR;
            let value = self.page_sprite(seq, BASE, ROW, panel, Some(v));
            self.sprites.get_mut(value).offset = [value_x, PITCH * i as f32];
            let m = self.page_sprite(seq, PRIO_NEW, ROW, value, None);
            self.limits.rows.push(r);
            self.limits.values.push(value);
            self.limits.marks.push(m);
        }
        let cursor = self.memory[LIST as usize].min(n.saturating_sub(1));
        self.limits.scroll = cursor.saturating_sub(ROWS - 1).min(n.saturating_sub(ROWS));
        self.list = MenuList::new(self.limits.rows.clone(), cursor);
        self.list.total = n.max(1);
        self.list.mask = 0x73;
        if n == 0 {
            self.no_cursor = true;
            self.list.mask &= !0x10;
        }
        self.scroll = self.limits.scroll;
        self.limit_rows(seq);
        self.show_help(seq, HELP_LIST);
        self.limit_view(seq);
    }

    /// FUN_140337cc0 (not leaving).
    pub(super) fn limits_list(
        &mut self,
        layout_: &Layout,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        let (input, moved) = self.input(pad);
        if self.substate == 2 {
            self.leave(layout_, input, moved, true);
            return false;
        }
        self.settle(seq, input, moved, true);
        if moved {
            self.limit_window(seq);
        }
        // FUN_140338630: only an owned limit opens.
        let valid = match input {
            In::Row(k) => self
                .limits
                .entries
                .get(k)
                .is_some_and(|&r| self.limit_owned(r)),
            _ => true,
        };
        if let Some(s) = sound(input, moved, valid) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            self.limit_view(seq);
        }
        if !valid {
            return moved;
        }
        match input {
            In::Start => self.exit = true,
            In::Cancel => {
                self.memory[LIST as usize] = self.list.cursor;
                self.pill_out(seq);
                self.hide_help(seq);
                self.slide(seq, 1, 1);
                self.pending = Some(COLLECTION_PAGE);
                self.substate = 2;
            }
            In::Row(k) => {
                self.memory[LIST as usize] = k;
                self.limits.cur = k;
                self.hide_help(seq);
                self.slide(seq, 0, 1);
                self.pending = Some(DETAIL);
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }

    /// FUN_1403386a0.
    pub(super) fn build_limit_detail(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_cursor = true;
        self.cat.detail = false;
        self.category_frame(seq);
        let c = self.container;
        let p0 = self.page_sprite(seq, PRIO_PANEL, DETAIL_PANEL, c, None);
        let track = self.page_sprite(seq, PRIO_PANEL, DETAIL_TRACK, c, None);
        let row = self
            .limits
            .entries
            .get(self.limits.cur)
            .copied()
            .unwrap_or(0);
        let limit = self.data.limits.get(row).copied();
        // The title with the use counter right-aligned inside it.
        let mut t = Self::text(
            limit.map(|l| self.msg(l.name)).unwrap_or_default(),
            Self::size(seq, DETAIL_TITLE),
        );
        t.group_position = false;
        t.keep_rgb = false;
        let title = self.page_sprite(seq, BASE, DETAIL_TITLE, c, Some(t));
        let mut v = Self::text(self.limit_value(row), Self::size(seq, DETAIL_TITLE));
        v.group_position = false;
        v.align = 1;
        v.colour = VALUE_COLOUR;
        let value = self.page_sprite(seq, PRIO_NEW, ROW, title, Some(v));
        self.sprites.get_mut(value).offset = [
            seq.groups[DETAIL_TITLE].light_position_x as f32,
            seq.groups[DETAIL_TITLE].text_position_y as f32,
        ];
        let text = limit.map(|l| self.msg(l.description)).unwrap_or_default();
        self.limits.lines = text.split(|&b| b == 0x02).map(<[u8]>::to_vec).collect();
        self.limits.top = 0;
        let mut t = Self::text(Vec::new(), Self::size(seq, DETAIL_PANEL));
        t.group_position = false;
        t.keep_rgb = false;
        t.extra = Some(LINE_EXTRA);
        let d = self.page_sprite(seq, BASE, ROW, p0, Some(t));
        self.limits.desc = Some(d);
        self.bar_in(seq, track, self.limits.lines.len(), LINES);
        self.limit_description();
        let owned = (0..self.data.limits.len())
            .filter(|&r| self.limit_owned(r))
            .count();
        self.list = MenuList::new(vec![d], 0);
        self.list.mask = 0x60 | if owned > 1 { 0xc000 } else { 0 };
        self.show_help(seq, HELP_DETAIL);
    }

    /// The visible lines of the description.
    fn limit_description(&mut self) {
        let total = self.limits.lines.len();
        let top = self.limits.top;
        let bytes = self.limits.lines[top.min(total.saturating_sub(1))..(top + LINES).min(total)]
            .join(&0x02u8);
        self.set_text(self.limits.desc, bytes);
    }

    /// FUN_140337ac0 (not leaving).
    pub(super) fn limit_detail(
        &mut self,
        layout_: &Layout,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        let (input, moved) = self.input(pad);
        if self.substate == 2 {
            self.leave(layout_, input, moved, false);
            return false;
        }
        self.settle(seq, input, moved, true);
        let total = self.limits.lines.len();
        let max = total.saturating_sub(LINES);
        let step = pad.trig | pad.repeat;
        let before = self.limits.top;
        if step & bits::UP != 0 {
            self.limits.top = self.limits.top.saturating_sub(1);
        } else if step & bits::DOWN != 0 {
            self.limits.top = (self.limits.top + 1).min(max);
        } else if total > LINES && step & bits::L1 != 0 {
            self.limits.top = self.limits.top.saturating_sub(LINES);
        } else if total > LINES && step & bits::R1 != 0 {
            self.limits.top = (self.limits.top + LINES).min(max);
        }
        if self.limits.top != before {
            self.bar_to(seq, self.limits.top);
            self.limit_description();
            sounds.push(SOUND_CURSOR);
        }
        match input {
            In::Start => {
                sounds.push(SOUND_CLOSE);
                self.exit = true;
            }
            In::Cancel => {
                sounds.push(SOUND_CLOSE);
                self.hide_help(seq);
                self.slide(seq, 1, 1);
                self.pending = Some(LIST);
                self.substate = 2;
            }
            // FUN_140338430: the next / previous owned limit (with wrap).
            In::R2 | In::L2 => {
                let n = self.limits.entries.len();
                let cur = self.limits.cur;
                let own = |s: &Self, i: usize| s.limit_owned(s.limits.entries[i]);
                let next = if input == In::R2 {
                    (cur + 1..n)
                        .find(|&i| own(self, i))
                        .or_else(|| (0..cur).find(|&i| own(self, i)))
                } else {
                    (0..cur)
                        .rev()
                        .find(|&i| own(self, i))
                        .or_else(|| (cur + 1..n).rev().find(|&i| own(self, i)))
                };
                if let Some(i) = next {
                    sounds.push(SOUND_DECIDE);
                    self.limits.cur = i;
                    self.memory[LIST as usize] = i;
                    self.hide_help(seq);
                    self.slide(seq, (input != In::R2) as usize, 1);
                    self.pending = Some(DETAIL);
                    self.substate = 2;
                }
            }
            _ => {}
        }
        moved
    }
}
