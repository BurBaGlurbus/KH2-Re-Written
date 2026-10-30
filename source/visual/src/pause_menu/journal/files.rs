//! Character Files / The Heartless / The Nobodies: the world menu (state
//! 0xe, FUN_140331da0), the lists (0xf / 0x12 / 0x15, FUN_140331870) and
//! the detail page (0x10 / 0x13 / 0x16, FUN_1403315b0) with the portrait
//! from `menu/jmface_<code>.2ld`. The 3D model, the "Slain" count and the
//! reaction rows need data the room does not carry yet and are not built.
use super::*;

const MENU_STATE: u8 = 0xe;
const LIST_PANEL: usize = 90;
const LIST_TRACK: usize = 95;
const ROW: usize = 2;
const MARK: [usize; 2] = [92, 93];
const DETAIL_PANEL: usize = 75;
const NAME: usize = 76;
const ORIGIN: usize = 77;
const UP_ARROW: usize = 81;
const DOWN_ARROW: usize = 82;
const FRAME: usize = 10;
const PRIO_PANEL: u32 = 0x755c;
const PRIO_NEW: u32 = 0x755e;
const PRIO_FACE: u32 = 0x755f;
const HELP_MENU: u16 = 0x48ee;
const HELP_LIST: [u16; 3] = [0x48ef, 0x48f0, 0x48f1];
const HELP_DETAIL: u16 = 0x48f2;
const NOTHING_YET: u16 = 0x36b3;
const HELP_REPORTS: [u16; 2] = [0x48ec, 0x48ed];
const REPORT_PANEL: usize = 97;
const REPORT_TITLE: usize = 98;
const REPORT_TRACK: usize = 100;
const REPORT_LINES: usize = 9;
const ROWS: usize = 10;
const PITCH: f32 = 26.;
const SIZE: f32 = 18.;
/// Description lines shown at once; the extra spacing is pad − 2 − size.
const LINES: usize = 7;
const LINE_EXTRA: f32 = 6.;
/// Journal world of "Other" and the Heartless / Nobodies kinds.
const OTHER: u8 = 0x10;
const HEARTLESS: u8 = 0x11;
const NOBODIES: u8 = 0x12;

#[derive(Default)]
pub(super) struct Page {
    /// The kind the lists show (journal world, 0x10, 0x11, 0x12).
    pub kind: u8,
    /// Menu rows: the kind of each.
    kinds: Vec<u8>,
    /// List rows: `char` rows.
    entries: Vec<usize>,
    rows: Vec<Id>,
    marks: Vec<Id>,
    /// First entry shown (FUN_1402f8f30).
    scroll: usize,
    panel: Option<Id>,
    /// The scroll track's art (G95), the bar's parent.
    track: Option<Id>,
    /// The entry on the detail page.
    cur: usize,
    /// The Characters page the detail was opened from (6 / 0xc / 0x28).
    pub ret: Option<u8>,
    desc: Option<Id>,
    lines: Vec<Vec<u8>>,
    top: usize,
    frame: Option<Id>,
    face_made: bool,
    up: Option<Id>,
    down: Option<Id>,
    /// The lists show Ansem Reports (`entries` are `anse` rows).
    reports: bool,
    /// Lines the description window shows; whether it has arrows.
    visible: usize,
    arrows: bool,
    pub(super) seen_reports: HashSet<u16>,
    pub(super) seen_reports_at_open: HashSet<u16>,
}

/// The list state of a kind and the detail state it opens.
fn detail_state(list: u8) -> u8 {
    match list {
        0x12 => 0x13,
        0x15 => 0x16,
        _ => 0x10,
    }
}

fn list_state(detail: u8) -> u8 {
    match detail {
        0x13 => 0x12,
        0x16 => 0x15,
        _ => 0xf,
    }
}

impl Journal {
    pub(super) fn world_unlocked(&self, w: u8) -> bool {
        if w > 0x0f {
            return true;
        }
        self.data
            .worlds
            .iter()
            .find(|r| r.world == w as i8)
            .is_some_and(|r| self.flag(r.unlock))
    }

    /// FUN_140326960(w, 1): the world's second name.
    pub(super) fn world_name2(&self, w: u8) -> Vec<u8> {
        let Some(r) = self.data.worlds.iter().find(|r| r.world == w as i8) else {
            return Vec::new();
        };
        if self.flag(r.alt_flag) {
            self.msg(r.alt_names[1])
        } else {
            self.msg(r.names[1])
        }
    }

    /// A `char` row is listed: some `diag` record that holds lists it.
    fn file_seen(&self, row: usize) -> bool {
        self.char_available(row as u16)
    }

    /// "?" per glyph, five in English (FUN_1403008d0).
    pub(super) fn locked_text() -> Vec<u8> {
        vec![0x49; 5]
    }

    /// NEW (92) / viewed (93) mark of a `char` row, or None.
    fn file_mark(&self, row: usize) -> Option<usize> {
        let new = self.file_seen(row) && !self.seen_chars_at_open.contains(&(row as u16));
        new.then(|| MARK[self.seen_chars.contains(&(row as u16)) as usize])
    }

    fn files_rows(&mut self, seq: &Sequence, menu: bool) {
        let scroll = self.files.scroll;
        self.bar_to(seq, scroll);
        for j in 0..ROWS {
            let k = scroll + j;
            let (Some(&row), Some(&mark)) = (self.files.rows.get(j), self.files.marks.get(j))
            else {
                continue;
            };
            let (text, group) = if menu {
                match self.files.kinds.get(k).copied() {
                    Some(w) => {
                        let unlocked = self.world_unlocked(w);
                        let text = if unlocked {
                            self.world_name2(w)
                        } else {
                            Self::locked_text()
                        };
                        let rows: Vec<usize> = (0..self.data.chars.len())
                            .filter(|&r| self.data.chars[r].world == w as i8)
                            .filter(|&r| self.file_mark(r).is_some())
                            .collect();
                        let group = if unlocked && !rows.is_empty() {
                            let all = rows.iter().all(|&r| self.file_mark(r) == Some(MARK[1]));
                            MARK[all as usize]
                        } else {
                            ROW
                        };
                        (text, group)
                    }
                    None => (Vec::new(), ROW),
                }
            } else if self.files.reports {
                match self.files.entries.get(k).copied() {
                    Some(i) => {
                        let a = self.data.reports[i];
                        if self.report_owned(i) {
                            let new = !self.files.seen_reports_at_open.contains(&(i as u16));
                            let seen = self.files.seen_reports.contains(&(i as u16));
                            (
                                self.msg(a.title),
                                if new { MARK[seen as usize] } else { ROW },
                            )
                        } else {
                            (Self::locked_text(), ROW)
                        }
                    }
                    None => (Vec::new(), ROW),
                }
            } else if self.files.entries.is_empty() {
                (
                    if j == 0 {
                        self.msg(NOTHING_YET)
                    } else {
                        Vec::new()
                    },
                    ROW,
                )
            } else {
                match self.files.entries.get(k).copied() {
                    Some(r) => {
                        let ch = self.data.chars[r];
                        if self.file_seen(r) {
                            (self.msg(ch.name), self.file_mark(r).unwrap_or(ROW))
                        } else {
                            (Self::locked_text(), ROW)
                        }
                    }
                    None => (Vec::new(), ROW),
                }
            };
            self.set_text(Some(row), text);
            self.sprites.set(seq, mark, group, None);
        }
    }

    fn files_window(&mut self, seq: &Sequence, menu: bool) {
        let n = self.list.total;
        let c = self.list.cursor;
        let mut s = self.files.scroll;
        if c < s {
            s = c;
        } else if c >= s + ROWS {
            s = c + 1 - ROWS;
        }
        s = s.min(n.saturating_sub(ROWS));
        if s != self.files.scroll {
            self.files.scroll = s;
            self.files_rows(seq, menu);
        }
        self.scroll = s;
    }

    /// The list pages' bar: total max(n, 10), visible 10.
    fn files_bar(&mut self, seq: &Sequence, n: usize) {
        if let Some(track) = self.files.track {
            self.bar_in(seq, track, n.max(ROWS), ROWS);
        }
    }

    fn files_panel(&mut self, seq: &Sequence) {
        let c = self.container;
        let panel = self.page_sprite(seq, PRIO_PANEL, LIST_PANEL, c, None);
        self.files.panel = Some(panel);
        self.files.track = Some(self.page_sprite(seq, PRIO_PANEL, LIST_TRACK, c, None));
        self.files.rows.clear();
        self.files.marks.clear();
        for i in 0..ROWS {
            let mut t = Self::text(Vec::new(), SIZE);
            t.group_position = false;
            t.keep_rgb = false;
            let r = self.page_sprite(seq, BASE, ROW, panel, Some(t));
            self.sprites.get_mut(r).offset = [0., PITCH * i as f32];
            let m = self.page_sprite(seq, PRIO_NEW, ROW, r, None);
            self.files.rows.push(r);
            self.files.marks.push(m);
        }
    }

    /// FUN_140331da0.
    pub(super) fn build_files_menu(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_light = true;
        self.cat.detail = false;
        self.files.ret = None;
        self.files.reports = false;
        self.category_frame(seq);
        self.files_panel(seq);
        let mut kinds = vec![OTHER];
        let all = self.show_all();
        kinds.extend((0..15u8).filter(|&w| self.world_unlocked(w) || all));
        self.files.kinds = kinds;
        let n = self.files.kinds.len();
        // Entered from Collection: the row of the world of a8[0].
        let cursor = match self
            .files
            .kinds
            .iter()
            .position(|&k| k as i32 == self.world)
        {
            Some(i) if self.world >= 0 && self.world <= 14 && self.memory[0xe] == 0 => i,
            _ => self.memory[0xe].min(n - 1),
        };
        self.files.scroll = cursor.saturating_sub(ROWS - 1).min(n.saturating_sub(ROWS));
        self.list = MenuList::new(self.files.rows.clone(), cursor);
        self.list.total = n;
        self.list.mask = 0x73;
        self.scroll = self.files.scroll;
        self.files_bar(seq, n);
        self.files_rows(seq, true);
        self.show_help(seq, HELP_MENU);
    }

    /// FUN_140331870.
    pub(super) fn build_files_list(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_light = true;
        self.cat.detail = false;
        self.files.ret = None;
        self.files.reports = false;
        match self.state {
            0x12 => self.files.kind = HEARTLESS,
            0x15 => self.files.kind = NOBODIES,
            _ => {}
        }
        self.category_frame(seq);
        if self.state == 0xf {
            let label = self.world_name2(self.files.kind);
            self.tab1_in(seq, label);
        }
        self.files_panel(seq);
        let all = self.show_all();
        let kind = self.files.kind as i8;
        self.files.entries = (0..self.data.chars.len())
            .filter(|&r| self.data.chars[r].world == kind && (self.file_seen(r) || all))
            .collect();
        let n = self.files.entries.len();
        let cursor = self.memory[0xf].min(n.saturating_sub(1));
        self.files.scroll = cursor.saturating_sub(ROWS - 1).min(n.saturating_sub(ROWS));
        self.list = MenuList::new(self.files.rows.clone(), cursor);
        self.list.total = n.max(1);
        self.list.mask = 0x73;
        if n == 0 {
            self.no_cursor = true;
            self.list.mask &= !0x10;
        }
        self.scroll = self.files.scroll;
        self.files_bar(seq, n);
        self.files_rows(seq, false);
        let h = match self.state {
            0x12 => HELP_LIST[1],
            0x15 => HELP_LIST[2],
            _ => HELP_LIST[0],
        };
        self.show_help(seq, h);
        self.files_view(seq);
    }

    /// FUN_140332820: the entry under the cursor counts as viewed.
    fn files_view(&mut self, seq: &Sequence) {
        let k = self.list.cursor;
        if self.files.reports {
            let Some(&i) = self.files.entries.get(k) else {
                return;
            };
            if self.report_owned(i)
                && self.files.seen_reports.insert(i as u16)
                && let Some(&m) = self.files.marks.get(k.saturating_sub(self.files.scroll))
                && !self.files.seen_reports_at_open.contains(&(i as u16))
            {
                self.sprites.set(seq, m, MARK[1], None);
            }
            return;
        }
        let Some(&r) = self.files.entries.get(k) else {
            return;
        };
        if !self.file_seen(r) {
            return;
        }
        if self.seen_chars.insert(r as u16)
            && let Some(&m) = self.files.marks.get(k.saturating_sub(self.files.scroll))
            && self.file_mark(r).is_some()
        {
            self.sprites.set(seq, m, MARK[1], None);
        }
    }

    /// FUN_1402f9440 plus the scroll window of the 10-row list.
    fn files_pick(&mut self, layout_: &Layout, pad: &Pad, menu: bool) -> (In, bool, bool) {
        let seq = &layout_.sequences[SEQUENCE];
        let (input, moved) = self.input(pad);
        if self.substate == 2 {
            self.leave(layout_, input, moved, true);
            return (input, moved, true);
        }
        self.settle(seq, input, moved, true);
        if moved {
            self.files_window(seq, menu);
        }
        (input, moved, false)
    }

    /// FUN_14032fbc0.
    pub(super) fn files_menu(
        &mut self,
        layout_: &Layout,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        let (input, moved, left) = self.files_pick(layout_, pad, true);
        if left {
            return false;
        }
        let valid = match input {
            In::Row(k) => self
                .files
                .kinds
                .get(k)
                .is_some_and(|&w| self.world_unlocked(w)),
            _ => true,
        };
        if let Some(s) = sound(input, moved, valid) {
            sounds.push(s);
        }
        if !valid {
            return moved;
        }
        match input {
            In::Start => self.exit = true,
            In::Cancel => {
                self.pill_out(seq);
                self.hide_help(seq);
                self.slide(seq, 1, 1);
                self.pending = Some(COLLECTION_PAGE);
                self.substate = 2;
            }
            In::Row(k) => {
                self.files.kind = self.files.kinds[k];
                self.memory[0xe] = k;
                self.memory[0xf] = 0;
                self.hide_help(seq);
                self.slide(seq, 0, 1);
                self.pending = Some(0xf);
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }

    /// FUN_14032f900.
    pub(super) fn files_list(
        &mut self,
        layout_: &Layout,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        let (input, moved, left) = self.files_pick(layout_, pad, false);
        if left {
            return false;
        }
        let valid = match input {
            In::Row(k) => self
                .files
                .entries
                .get(k)
                .is_some_and(|&r| self.file_seen(r)),
            _ => true,
        };
        if let Some(s) = sound(input, moved, valid) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            self.files_view(seq);
        }
        if !valid {
            return moved;
        }
        match input {
            In::Start => self.exit = true,
            In::Cancel => {
                self.hide_help(seq);
                self.slide(seq, 1, 1);
                if self.state == 0xf {
                    self.tab1_out(seq);
                    self.pending = Some(MENU_STATE);
                } else {
                    self.pill_out(seq);
                    self.pending = Some(COLLECTION_PAGE);
                }
                self.substate = 2;
            }
            In::Row(k) => {
                self.memory[0xf] = k;
                self.files.cur = k;
                self.hide_help(seq);
                self.slide(seq, 0, 1);
                self.pending = Some(detail_state(self.state));
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }

    /// FUN_140331ce0: the Characters page's decide opens the detail of the
    /// selected character among the page's own.
    pub(super) fn files_from_characters(&mut self, seq: &Sequence, rows: Vec<usize>, sel: usize) {
        self.files.ret = Some(self.state);
        self.files.entries = rows;
        self.files.cur = sel;
        self.files.kind = OTHER;
        self.hide_help(seq);
        self.slide(seq, 0, 1);
        self.pending = Some(0x10);
        self.substate = 2;
    }

    /// FUN_1403315b0.
    pub(super) fn build_files_detail(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_cursor = true;
        self.cat.detail = true;
        self.category_frame(seq);
        let c = self.container;
        let p0 = self.page_sprite(seq, PRIO_PANEL, DETAIL_PANEL, c, None);
        self.files.panel = Some(p0);
        let r = self.files.entries.get(self.files.cur).copied();
        let ch = r.and_then(|r| self.data.chars.get(r)).copied();
        let name = ch.map(|c| self.msg(c.name)).unwrap_or_default();
        let mut t = Self::text(name, Self::size(seq, NAME));
        t.group_position = false;
        t.colour = 0x6000_0008;
        self.page_sprite(seq, PRIO_PANEL, NAME, c, Some(t));
        // Record 230 (Tigger) forces message 0x2874.
        let origin = ch
            .map(|c| if c.name == 0x2dc6 { 0x2874 } else { c.origin })
            .filter(|&o| o != 0);
        if let Some(o) = origin {
            let mut t = Self::text(self.msg(o), Self::size(seq, ORIGIN));
            t.group_position = false;
            t.colour = 0x6000_0008;
            self.page_sprite(seq, PRIO_PANEL, ORIGIN, c, Some(t));
        }
        // The portrait frame at P0 + (LightPositionX, TextPositionY).
        let g = &seq.groups[DETAIL_PANEL];
        let at = [g.light_position_x as f32, g.text_position_y as f32];
        let ft = ch.map_or(0, |c| c.frame as usize).min(5);
        let f = self.page_sprite(seq, BASE, FRAME + ft, p0, None);
        self.sprites.get_mut(f).offset = at;
        self.files.frame = Some(f);
        self.files.face_made = false;
        // The description: the page's own row, 7 lines at a time.
        let text = ch.map(|c| self.msg(c.description)).unwrap_or_default();
        self.files.lines = text.split(|&b| b == 0x02).map(<[u8]>::to_vec).collect();
        self.files.top = 0;
        self.files.visible = LINES;
        self.files.arrows = true;
        self.files.reports = false;
        let mut t = Self::text(Vec::new(), Self::size(seq, DETAIL_PANEL));
        t.group_position = false;
        t.keep_rgb = false;
        t.extra = Some(LINE_EXTRA);
        let d = self.page_sprite(seq, BASE, ROW, p0, Some(t));
        self.files.desc = Some(d);
        self.files.up = None;
        self.files.down = None;
        self.files_description(seq);
        // R2 / L2 need more than one seen entry.
        let seen = self
            .files
            .entries
            .iter()
            .filter(|&&r| self.file_seen(r))
            .count();
        self.list = MenuList::new(vec![d], 0);
        self.list.mask = 0x60 | if seen > 1 { 0xc000 } else { 0 };
        self.show_help(seq, HELP_DETAIL);
        // The character counts as viewed.
        if let Some(r) = r
            && self.file_seen(r)
        {
            self.seen_chars.insert(r as u16);
        }
    }

    /// The visible lines and the two scroll arrows (FUN_140330240 /
    /// FUN_14032e360).
    fn files_description(&mut self, seq: &Sequence) {
        let total = self.files.lines.len();
        let top = self.files.top;
        let visible = self.files.visible;
        self.bar_to(seq, top);
        let bytes = self.files.lines[top.min(total.saturating_sub(1))..(top + visible).min(total)]
            .join(&0x02u8);
        self.set_text(self.files.desc, bytes);
        let c = self.container;
        let arrows = self.files.arrows;
        let want_up = arrows && top > 0;
        let want_down = arrows && total > visible && top + visible < total;
        for (slot, want, group) in [(0, want_up, UP_ARROW), (1, want_down, DOWN_ARROW)] {
            let cur = if slot == 0 {
                self.files.up
            } else {
                self.files.down
            };
            let alive = cur.is_some_and(|a| self.sprites.alive(a));
            let id = match (alive, want) {
                (false, true) => Some(self.page_sprite(seq, BASE, group, c, None)),
                (true, false) => {
                    if let Some(a) = cur {
                        self.sprites.kill(a);
                    }
                    None
                }
                _ => cur,
            };
            if slot == 0 {
                self.files.up = id;
            } else {
                self.files.down = id;
            }
        }
    }

    /// FUN_14032f2a0 (not leaving).
    pub(super) fn files_detail(
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
        // The portrait appears once the face file is bound.
        if !self.files.face_made
            && let Some(f) = self.files.frame
            && let Some(&r) = self.files.entries.get(self.files.cur)
        {
            let ch = self.data.chars[r];
            let key = ch.world as u8;
            if self.cache.faces.contains(&key) {
                self.files.face_made = true;
                self.pictures.push(Picture {
                    prio: PRIO_FACE,
                    sequence: 0,
                    sheet: Sheet::Face(key, ch.image),
                    player: Player::new(&layout_.sequences[0], 1),
                    queued: Some(2),
                    parent: f,
                    offset: [0.; 2],
                    dead: false,
                });
            } else {
                self.cache.wanted_face = Some(key);
            }
        }
        // UP / DOWN scroll the description a line.
        let total = self.files.lines.len();
        let max = total.saturating_sub(self.files.visible);
        let step = pad.trig | pad.repeat;
        let before = self.files.top;
        if step & bits::UP != 0 {
            self.files.top = self.files.top.saturating_sub(1);
        } else if step & bits::DOWN != 0 {
            self.files.top = (self.files.top + 1).min(max);
        }
        if self.files.top != before {
            self.files_description(seq);
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
                self.pending = Some(self.files.ret.unwrap_or(list_state(self.state)));
                self.files.ret = None;
                self.substate = 2;
            }
            In::R2 | In::L2 => {
                let n = self.files.entries.len();
                let cur = self.files.cur;
                let seen = |s: &Self, i: usize| s.file_seen(s.files.entries[i]);
                let next = if input == In::R2 {
                    (cur + 1..n)
                        .find(|&i| seen(self, i))
                        .or_else(|| (0..cur).find(|&i| seen(self, i)))
                } else {
                    (0..cur)
                        .rev()
                        .find(|&i| seen(self, i))
                        .or_else(|| (cur + 1..n).rev().find(|&i| seen(self, i)))
                };
                if let Some(i) = next {
                    sounds.push(SOUND_DECIDE);
                    self.files.cur = i;
                    self.memory[0xf] = i;
                    self.hide_help(seq);
                    self.slide(seq, (input != In::R2) as usize, 1);
                    self.pending = Some(self.state);
                    self.substate = 2;
                }
            }
            _ => {}
        }
        moved
    }

    fn report_owned(&self, i: usize) -> bool {
        self.data
            .reports
            .get(i)
            .is_some_and(|a| self.data.owned_items.contains(&a.item))
    }

    /// FUN_140333df0.
    pub(super) fn build_reports_list(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_light = true;
        self.cat.detail = false;
        self.files.ret = None;
        self.files.reports = true;
        self.category_frame(seq);
        self.files_panel(seq);
        let all = self.show_all();
        self.files.entries = (0..self.data.reports.len())
            .filter(|&i| self.report_owned(i) || all)
            .collect();
        let n = self.files.entries.len();
        let cursor = self.memory[0x18].min(n.saturating_sub(1));
        self.files.scroll = cursor.saturating_sub(ROWS - 1).min(n.saturating_sub(ROWS));
        self.list = MenuList::new(self.files.rows.clone(), cursor);
        self.list.total = n.max(1);
        self.list.mask = 0x73;
        if n == 0 {
            self.no_cursor = true;
            self.list.mask &= !0x10;
        }
        self.scroll = self.files.scroll;
        self.files_bar(seq, n);
        self.files_rows(seq, false);
        self.show_help(seq, HELP_REPORTS[0]);
        self.files_view(seq);
    }

    /// FUN_1403330d0.
    pub(super) fn reports_list(
        &mut self,
        layout_: &Layout,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        let (input, moved, left) = self.files_pick(layout_, pad, false);
        if left {
            return false;
        }
        let valid = match input {
            In::Row(k) => self
                .files
                .entries
                .get(k)
                .is_some_and(|&i| self.report_owned(i)),
            _ => true,
        };
        if let Some(s) = sound(input, moved, valid) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            self.files_view(seq);
        }
        if !valid {
            return moved;
        }
        match input {
            In::Start => self.exit = true,
            In::Cancel => {
                self.pill_out(seq);
                self.hide_help(seq);
                self.slide(seq, 1, 1);
                self.pending = Some(COLLECTION_PAGE);
                self.substate = 2;
            }
            In::Row(k) => {
                self.memory[0x18] = k;
                self.files.cur = k;
                self.hide_help(seq);
                self.slide(seq, 0, 1);
                self.pending = Some(0x19);
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }

    /// FUN_140333900.
    pub(super) fn build_report_reader(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_cursor = true;
        self.cat.detail = false;
        self.files.reports = true;
        self.category_frame(seq);
        let c = self.container;
        let p0 = self.page_sprite(seq, PRIO_PANEL, REPORT_PANEL, c, None);
        let track = self.page_sprite(seq, PRIO_PANEL, REPORT_TRACK, c, None);
        let a = self
            .files
            .entries
            .get(self.files.cur)
            .and_then(|&i| self.data.reports.get(i))
            .copied();
        let mut t = Self::text(
            a.map(|a| self.msg(a.title)).unwrap_or_default(),
            Self::size(seq, REPORT_TITLE),
        );
        t.align = 2;
        t.group_position = false;
        t.keep_rgb = false;
        self.page_sprite(seq, BASE, REPORT_TITLE, c, Some(t));
        let text = a.map(|a| self.msg(a.text)).unwrap_or_default();
        self.files.lines = text.split(|&b| b == 0x02).map(<[u8]>::to_vec).collect();
        self.files.top = 0;
        self.files.visible = REPORT_LINES;
        self.files.arrows = false;
        self.bar_in(seq, track, self.files.lines.len(), REPORT_LINES);
        let mut t = Self::text(Vec::new(), Self::size(seq, REPORT_PANEL));
        t.group_position = false;
        t.keep_rgb = false;
        t.extra = Some(LINE_EXTRA);
        let d = self.page_sprite(seq, BASE, ROW, p0, Some(t));
        self.files.desc = Some(d);
        self.files.up = None;
        self.files.down = None;
        self.files_description(seq);
        let owned = (0..self.data.reports.len())
            .filter(|&i| self.report_owned(i))
            .count();
        self.list = MenuList::new(vec![d], 0);
        self.list.mask = 0x60 | if owned > 1 { 0xc000 } else { 0 };
        self.show_help(seq, HELP_REPORTS[1]);
    }

    /// FUN_140332ea0 (not leaving).
    pub(super) fn report_reader(
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
        let total = self.files.lines.len();
        let max = total.saturating_sub(REPORT_LINES);
        let step = pad.trig | pad.repeat;
        let before = self.files.top;
        if step & bits::UP != 0 {
            self.files.top = self.files.top.saturating_sub(1);
        } else if step & bits::DOWN != 0 {
            self.files.top = (self.files.top + 1).min(max);
        } else if total > REPORT_LINES && step & bits::L1 != 0 {
            self.files.top = self.files.top.saturating_sub(REPORT_LINES);
        } else if total > REPORT_LINES && step & bits::R1 != 0 {
            self.files.top = (self.files.top + REPORT_LINES).min(max);
        }
        if self.files.top != before {
            self.files_description(seq);
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
                self.pending = Some(0x18);
                self.substate = 2;
            }
            In::R2 | In::L2 => {
                let n = self.files.entries.len();
                let cur = self.files.cur;
                let own = |s: &Self, i: usize| s.report_owned(s.files.entries[i]);
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
                    self.files.cur = i;
                    self.memory[0x18] = i;
                    self.hide_help(seq);
                    self.slide(seq, (input != In::R2) as usize, 1);
                    self.pending = Some(0x19);
                    self.substate = 2;
                }
            }
            _ => {}
        }
        moved
    }
}
