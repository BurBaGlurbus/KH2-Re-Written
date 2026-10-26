//! The camp's Items screens (camp-05 spec): the member list with Stock and
//! its preview panel (state 1: FUN_14034ff50 / FUN_14034df80 /
//! FUN_1403504e0), the stock grid with its category tabs and scrollbar
//! (state 10: FUN_14034fa20 / FUN_14034dba0 / FUN_1403509b0 / FUN_14034ccc0),
//! the Use / Throw away menu (11: FUN_14034ef80 / FUN_14034d2d0), the use
//! target with the party block (12: FUN_14034f5f0 / FUN_14034d640) and the
//! two dialog boxes (13 / 14: FUN_14034da70 / FUN_14034d120); the item
//! picture (`picture`) shows in states 10 and 11.
use super::*;
use std::collections::BTreeMap;

const PRIO_ITEM_ROW: u32 = 0x7541;
const PRIO_ITEM_BADGE: u32 = 0x7542;
const PRIO_ITEM_PANEL: u32 = 0x753d;
const PRIO_ITEM_BAR: u32 = 0x753e;
const PRIO_ITEM_TEXT: u32 = 0x753f;
const PRIO_STOCK_FRAME: u32 = 0x753d;
const PRIO_STOCK_ROW: u32 = 0x753e;
const PRIO_STOCK_BADGE: u32 = 0x7540;
/// The item picture: the stock list's own frame {appear, close, idle} at
/// 0x753f; on the command screen's holder at 0x753c.
const PRIO_STOCK_PICTURE: u32 = 0x753f;
const PRIO_COMMAND_PICTURE: u32 = 0x753c;
const PICTURE_FRAME: [usize; 3] = [348, 350, 349];
const PRIO_TAB_BAR: u32 = 0x753c;
const PRIO_TAB_LABEL: u32 = 0x753d;
const PRIO_HIGHLIGHT: u32 = 0x753e;
const PRIO_COMMAND: u32 = 0x753b;
const PRIO_COMMAND_ROW: u32 = 0x753c;
const PRIO_COMMAND_LABEL: u32 = 0x753d;
const PRIO_MARKER: u32 = 0x753e;
const PRIO_DIALOG: u32 = 0x7546;
const PRIO_CHOICE: u32 = 0x7549;

/// Preview bars per section (0x1405bbed8): main weapon, form weapons,
/// armor, accessories, items.
const BAR_WEAPON: usize = 150;
const BAR_FORM: usize = 151;
const BAR_ARMOR: usize = 139;
const BAR_ACCESSORY: usize = 137;
const BAR_ITEM: usize = 143;
/// Text styles: white, grey, red, yellow.
const WHITE: usize = 123;
const GREY: usize = 119;
const RED: usize = 122;
const YELLOW: usize = 124;
pub(super) const BADGE: [usize; 2] = [50, 49];
/// Item type of the magic (Fire, Blizzard, ...: the "new magic" checks).
const MAGIC_KIND: u8 = 0x12;

const STOCK_FRAME: [usize; 3] = [293, 294, 295];
const STOCK_EMPTY: usize = 302;
pub(super) const STOCK_ROWS: RowGroups = RowGroups {
    appear: 304,
    close: 304,
    idle: 304,
    selected: 303,
};
const TAB_BAR: [usize; 3] = [241, 242, 243];
const HIGHLIGHT: usize = 240;
/// L2 / R2 arrows: appear, idle, close.
const ARROWS: [[usize; 3]; 2] = [[354, 356, 358], [355, 357, 359]];
/// Scrollbar players 1..3 (0x1405bad50): top cap, thumb, bottom cap.
const SCROLL: [[usize; 3]; 3] = [[84, 85, 86], [81, 82, 83], [87, 88, 89]];
const COMMAND_FRAME: [usize; 3] = [296, 297, 298];
const HOLDER: [usize; 3] = [305, 306, 307];
const TARGET_FRAME: [usize; 3] = [299, 300, 301];
const NAME: usize = 292;
const MARKER: usize = 290;
pub(super) const COMMAND_ROWS: RowGroups = RowGroups {
    appear: 291,
    close: 291,
    idle: 291,
    selected: 291,
};
const DIALOG: [usize; 3] = [6, 7, 8];
pub(super) const CHOICE_ROWS: RowGroups = RowGroups {
    appear: 3,
    close: 5,
    idle: 4,
    selected: 2,
};
/// The party block in mode 1 (0x1405bc4f8 + 6): appear, idle, close.
const BLOCK_MODE1: [usize; 3] = [104, 105, 106];
/// The dimmed hand (alt of G25).
const GLOVE_DIM: usize = 23;
/// se000 #37: an item used (FUN_1402eb740(0x11)).
pub const SOUND_USE: u32 = 37;

/// Struggle Sword / Wand / Hammer, hidden with menu flag 0x77
/// (0x1405bbeb0).
pub(super) const STRUGGLE: [u16; 3] = [0x180, 0x1f5, 0x1f6];
/// Party-wide items (FUN_1403c4780) and the Drive items (FUN_140351390).
const PARTY_WIDE: [u16; 4] = [5, 6, 7, 0x83];
const DRIVE: [u16; 2] = [0x112, 0x113];

/// One item record with its texts (03system `item`, sys.bar).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemInfo {
    pub id: u16,
    pub kind: u8,
    pub amount: u16,
    pub sell: u16,
    pub icon: u8,
    /// +0x14: the picture (`itempic/item-%03d.imd`).
    pub picture: u16,
    /// +6: an ability's kind (low byte) and AP cost (high byte); a
    /// weapon's stat entry.
    pub param: u16,
    /// +0x10 (consumables): the command it runs, and that command's name
    /// (03system `cmd` +8; FUN_1403c41c0 / FUN_1403b68b0).
    pub command: u16,
    pub command_name: Vec<u8>,
    pub name: Vec<u8>,
    pub description: Vec<u8>,
}

/// A member's equipment (FUN_1404ad270): item ids, 0 = empty slot;
/// `originals` are the item slots' original items (R+0x44).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Equipment {
    /// The member's character id (1 Sora, 2 Donald, 3 Goofy; 0 is taken
    /// as Sora): it picks the weapon type Equip offers.
    pub character: u8,
    pub weapon: u16,
    /// Member record +2: second member weapon, distinct from a form's weapon.
    /// The development room leaves this slot empty.
    pub secondary_weapon: u16,
    pub forms: Vec<u16>,
    /// The form each `forms` entry is for (Valor 1, Master 4, Final 5).
    pub form_ids: Vec<u8>,
    pub armor: Vec<u16>,
    pub accessories: Vec<u16>,
    pub items: Vec<u16>,
    pub originals: Vec<u16>,
}

/// The screens' messages.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemTexts {
    /// 0x8455 "Stock", 0x845a, 0x8456 "Use", 0x8457 "Throw away", 0x8448
    /// "–––", 0xb943 "Use on who?", 0xb944 "All".
    pub stock: Vec<u8>,
    pub empty: Vec<u8>,
    pub use_: Vec<u8>,
    pub throw: Vec<u8>,
    pub blank: Vec<u8>,
    pub use_on: Vec<u8>,
    pub all: Vec<u8>,
    /// 0xbb86 (members) and 0xbb87 (Stock).
    pub help: [Vec<u8>; 2],
    /// 0xb90c, 0xb90f, 0xb90d, 0xb90e, 0xb910.
    pub tabs: [Vec<u8>; 5],
    /// 0xb94f "Throw … away?" and 0xbb90 "… will have no effect".
    pub throw_query: Vec<u8>,
    pub no_effect: Vec<u8>,
    /// Category glyph messages by item type (0x1405ba790).
    pub categories: Vec<Vec<u8>>,
    /// 0x9fcf OK, 0x9fd0 Yes, 0x9fd1 No.
    pub ok: Vec<u8>,
    pub yes: Vec<u8>,
    pub no: Vec<u8>,
}

/// What the Items screens read and change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemsData {
    /// The item table in file order (FUN_1403e3960).
    pub table: Vec<ItemInfo>,
    /// Stock counts by item id (save+0x3580 / +0x36c0).
    pub counts: BTreeMap<u16, u8>,
    /// Per member.
    pub equipment: Vec<Equipment>,
    pub texts: ItemTexts,
    /// Menu flag 0x77 (Struggle weapons hidden).
    pub struggle_hidden: bool,
    /// "New" items (DAT_140bf0dc0) and the seen bits (save+0x41a8).
    pub new_items: Vec<u16>,
    pub seen: Vec<u16>,
}

/// FUN_14036aa70(−1): the new magic (item type 0x12) of this camp.
fn new_magic(d: &ItemsData) -> Vec<u16> {
    d.new_items
        .iter()
        .copied()
        .filter(|&id| d.table.iter().any(|i| i.id == id && i.kind == MAGIC_KIND))
        .collect()
}

/// FUN_14036aa70(id) / FUN_14036a9a0(id): `Some(seen)` while magic `id` is
/// new.
pub fn magic_badge(d: &ItemsData, id: u16) -> Option<bool> {
    (id != 0 && new_magic(d).contains(&id)).then(|| d.seen.contains(&id))
}

/// FUN_14036aa70(−1) / FUN_14036a9a0(−1): `Some(all seen)` when any magic is
/// new.
pub fn any_magic_badge(d: &ItemsData) -> Option<bool> {
    let new = new_magic(d);
    (!new.is_empty()).then(|| new.iter().all(|i| d.seen.contains(i)))
}

/// The camp's "new" items (FUN_140369520): the stock's items the save has
/// not marked seen, except those the game started with.
pub fn new_items(counts: &BTreeMap<u16, u8>, seen: &[u16], starting: &[u16]) -> Vec<u16> {
    counts
        .iter()
        .filter(|(id, n)| **n > 0 && !seen.contains(id) && !starting.contains(id))
        .map(|(id, _)| *id)
        .collect()
}

/// FUN_1402ebdd0's table 0x1405ba700: type → (icon, alt icon).
pub fn type_icon(kind: u8, alt: bool) -> u8 {
    let (icon, other) = match kind {
        0 => (0, 0xc),
        1 => (1, 1),
        2 => (4, 0xe),
        3 => (5, 0xf),
        4 => (6, 0x10),
        5..=13 | 22 => (0xb, 0xb),
        14 => (7, 7),
        15 => (0x11, 0x11),
        16 => (9, 9),
        17 => (2, 2),
        18 => (8, 0x12),
        19 => (3, 0xd),
        20 => (0x1d, 0x1d),
        _ => (0xb, 0xb),
    };
    if alt { other } else { icon }
}

/// The item type of a character's weapons: the (character, type) pairs
/// at 0x1405bbe48 (FUN_14034bc80 filter 1).
pub fn weapon_type(character: u8) -> u8 {
    const PAIRS: [(u8, u8); 14] = [
        (1, 2),
        (2, 3),
        (3, 4),
        (7, 5),
        (5, 6),
        (9, 7),
        (10, 8),
        (6, 9),
        (13, 10),
        (11, 11),
        (8, 12),
        (12, 13),
        (14, 2),
        (15, 9),
    ];
    let c = character.max(1);
    PAIRS.iter().find(|p| p.0 == c).map_or(2, |p| p.1)
}

/// The types each stock tab lists (FUN_14034bc80 with the tab's filter).
pub fn tab_types(tab: usize, kind: u8) -> bool {
    match tab {
        0 => matches!(kind, 0..=17 | 20),
        1 => matches!(kind, 0 | 1),
        2 => matches!(kind, 2..=13),
        3 => matches!(kind, 14 | 15),
        _ => matches!(kind, 16 | 17 | 20),
    }
}

/// FUN_14034c770 + FUN_14034caa0: owned items of the tab's types in table
/// order (the sort key keeps file order), Struggle weapons hidden with the
/// menu flag.
pub fn stock_list(data: &ItemsData, tab: usize) -> Vec<(u16, u8)> {
    data.table
        .iter()
        .filter(|r| tab_types(tab, r.kind))
        .filter(|r| !(data.struggle_hidden && STRUGGLE.contains(&r.id)))
        .filter_map(|r| {
            let n = data.counts.get(&r.id).copied().unwrap_or(0);
            (n != 0).then_some((r.id, n))
        })
        .collect()
}

/// FUN_14034c1b0's count string: empty for 1, else "×N" ('*' = 0x2d).
pub fn count_text(n: u8) -> Vec<u8> {
    if n == 1 {
        return Vec::new();
    }
    let mut out = vec![0x2d];
    out.extend(encode_number(&n.to_string()));
    out
}

/// FUN_1403ae030: each 0x0d code takes the next argument in order.
pub fn format_message(template: &[u8], args: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut next = args.iter();
    let mut i = 0;
    while i < template.len() {
        let b = template[i];
        if b == 0x0d {
            out.extend_from_slice(next.next().copied().unwrap_or(&[]));
            i += 1;
        } else if b < 0x20 && b != 0x01 {
            let n = crate::dialogue::code_length(b).max(1);
            out.extend_from_slice(&template[i..(i + n).min(template.len())]);
            i += n;
        } else {
            out.push(b);
            i += 1;
        }
    }
    out
}

/// FUN_1403c3da0 / FUN_1403c4b80 for the HP/MP items on one member:
/// whether the item does anything, and its effect.
fn restore(item: &ItemInfo, m: &mut Member, apply: bool) -> bool {
    let part = |max: i32, a: u16| (max * a as i32 + 99) / 100;
    let hp = m.hp < m.max_hp;
    // FUN_1403c4e50 [inferred: MP not full].
    let mp = m.mp < m.max_mp;
    let (ok, dhp, dmp) = match item.id {
        1 | 2 | 5 => (hp, part(m.max_hp, item.amount), 0),
        3 | 6 => (mp, 0, part(m.max_mp, item.amount)),
        4 | 7 => (
            hp || mp,
            part(m.max_hp, item.amount),
            part(m.max_mp, item.amount),
        ),
        0x83 => (hp || mp, m.max_hp, part(m.max_mp, item.amount)),
        _ => (false, 0, 0),
    };
    if ok && apply {
        m.hp = (m.hp + dhp).min(m.max_hp);
        m.mp = (m.mp + dmp).min(m.max_mp);
    }
    ok
}

/// The stock grid (list +0x25 = 1: 2 columns, 9 rows, page 18).
#[derive(Clone, Debug, Default)]
pub(super) struct Grid {
    pub(super) list: MenuList,
    pub(super) scroll: usize,
    /// Columns and visible lines (the stock list: 2 × 9; a plain scrolling
    /// list: 1 × 9).
    pub(super) cols: usize,
    pub(super) lines: usize,
}

const COLS: usize = 2;
const LINES: usize = 9;
const PAGE: usize = COLS * LINES;

impl Grid {
    fn index(&self) -> usize {
        self.list.cursor + self.scroll
    }

    /// FUN_1403e2… the grid branch of FUN_1402f9440 / FUN_1402f8240 /
    /// FUN_1402f8650 (camp-05 §3.8).
    pub(super) fn input(&mut self, p: &Pad) -> (Pick, bool) {
        use bits::*;
        let (cols, lines) = (self.cols, self.lines);
        let page = cols * lines;
        let n = self.list.total;
        if n == 0 {
            return (Pick::None, false);
        }
        let before = self.index();
        let (mut cur, mut sc) = (self.list.cursor, self.scroll);
        let a = cur + sc;
        let (row, col) = (a / cols, a % cols);
        let rows_n = n.div_ceil(cols);
        let mask = self.list.mask;
        // The jump to the last item (FUN_1402f8650's grid branch).
        let last = || -> (usize, usize) {
            let rem = n % page;
            let t = page - rem;
            let s = ((t % cols) as i64 - t as i64 - rem as i64 + n as i64).max(0) as usize;
            (n - s - 1, s)
        };
        if p.repeat & UP != 0 && mask & 0x1 != 0 {
            let blocked = (row == 0 && p.trig & UP == 0) || (rows_n == 1 && p.trig & UP != 0);
            if !blocked {
                if cur / cols >= 1 {
                    cur -= cols;
                } else if sc >= 1 {
                    sc -= cols;
                } else {
                    let vis = if page < n {
                        sc = (n - page).div_ceil(cols) * cols;
                        lines
                    } else {
                        rows_n
                    };
                    cur = (vis - 1) * cols + col;
                    if n <= cur + sc {
                        cur = n - sc - 1;
                    }
                }
            }
        } else if p.repeat & DOWN != 0 && mask & 0x2 != 0 {
            let blocked =
                (row + 1 >= rows_n && p.trig & DOWN == 0) || (rows_n == 1 && p.trig & DOWN != 0);
            if !blocked {
                if row + 1 < rows_n {
                    if cur / cols < lines - 1 || rows_n <= lines {
                        cur += cols;
                    } else {
                        sc += cols;
                    }
                    if n <= cur + sc {
                        cur = n - sc - 1;
                    }
                } else {
                    sc = 0;
                    cur = col;
                }
            }
        } else if p.repeat & (LEFT | RIGHT) != 0 && mask & 0xc != 0 {
            let last_cols = if !n.is_multiple_of(cols) && a >= n - n % cols {
                n % cols
            } else {
                cols
            };
            if p.repeat & LEFT != 0 {
                cur = if col > 0 { cur } else { cur + last_cols } - 1;
            } else if col + 1 < last_cols {
                cur += 1;
            } else {
                cur = cur + 1 - last_cols;
            }
        } else if p.repeat & L1 != 0 && mask & 0x2000 != 0 {
            if a > 0 || p.trig & L1 != 0 {
                if sc < page {
                    if sc == 0 && cur == 0 {
                        (cur, sc) = last();
                    } else if sc + cur < page {
                        (cur, sc) = (0, 0);
                    } else {
                        (cur, sc) = (a - page, 0);
                    }
                } else {
                    sc -= page;
                }
            }
        } else if p.repeat & R1 != 0 && mask & 0x1000 != 0 {
            if a + 1 < n {
                let s = sc + page;
                if s + cur < n {
                    if page + s >= n {
                        let s = rows_n.saturating_sub(lines) * cols;
                        cur = cur.min(n - s - 1);
                        sc = s;
                    } else {
                        sc = s;
                    }
                } else {
                    (cur, sc) = last();
                }
            } else if p.trig & R1 != 0 {
                (cur, sc) = (0, 0);
            }
        }
        self.list.cursor = cur;
        self.scroll = sc;
        if self.index() != before {
            return (Pick::None, true);
        }
        let pick = if p.trig & DECIDE != 0 && mask & 0x10 != 0 {
            Pick::Row(self.index())
        } else if p.trig & CANCEL != 0 && mask & 0x20 != 0 {
            Pick::Cancel
        } else if p.trig & START != 0 && mask & 0x40 != 0 {
            Pick::Start
        } else if let Some(b) = buttons(p, mask) {
            b
        } else if p.trig & R2 != 0 && mask & 0x4000 != 0 {
            Pick::Next
        } else if p.trig & L2 != 0 && mask & 0x8000 != 0 {
            Pick::Previous
        } else {
            Pick::None
        };
        (pick, false)
    }
}

/// The screens' sprites and lists.
#[derive(Default)]
pub(super) struct Ui {
    // State 1.
    pub(super) list: MenuList,
    stock_badge: Option<Id>,
    panel: Option<Id>,
    bars: Vec<Id>,
    texts: Vec<Id>,
    // State 10.
    tab: usize,
    entries: Vec<(u16, u8)>,
    pub(super) grid: Grid,
    frame: Option<Id>,
    badges: Vec<Id>,
    pub(super) scroll: Vec<Id>,
    scroll_scale: f32,
    scroll_parent: Option<Id>,
    bar: Option<Id>,
    arrows: Vec<Id>,
    highlight: Option<Id>,
    /// DAT_140beec22: no hand or LightCur (an empty stock list).
    pub(super) no_pointer: bool,
    // State 11.
    item: usize,
    command: Option<Id>,
    holder: Option<Id>,
    name: Option<Id>,
    count: Option<Id>,
    pub(super) commands: MenuList,
    marker: Option<Id>,
    // State 12.
    target: Option<Id>,
    caption: Option<Id>,
    pub(super) targets: MenuList,
    target_marker: Option<Id>,
    block: Option<Id>,
    party_wide: bool,
    // States 13 / 14.
    dialog: Option<Id>,
    pub(super) choices: MenuList,
    /// Dimmed hands left on earlier lists (alt group 23).
    dim: Vec<Id>,
}

impl Ui {
    /// The dialog box (DAT_140beed08) while one is up.
    pub(super) fn dialog_box(&self) -> Option<Id> {
        self.dialog
    }

    pub(super) fn kill_dim(&mut self, sp: &mut Sprites) {
        for g in self.dim.drain(..) {
            sp.kill(g);
        }
    }
}

impl Camp {
    pub(super) fn info(&self, id: u16) -> ItemInfo {
        self.data
            .items
            .table
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .unwrap_or_default()
    }

    /// FUN_1402ebf70: `09 icon` + the item's name.
    pub(super) fn item_name(&self, id: u16, alt: bool) -> Vec<u8> {
        let r = self.info(id);
        let icon = if alt { type_icon(r.kind, true) } else { r.icon };
        let mut out = vec![0x09, icon];
        out.extend(r.name);
        out
    }

    pub(super) fn alive_opt(&self, id: Option<Id>) -> bool {
        id.is_some_and(|i| self.sprites.alive(i))
    }

    pub(super) fn group_of(&self, id: Option<Id>) -> Option<usize> {
        id.filter(|&i| self.sprites.alive(i))
            .map(|i| self.sprites.get(i).player.group)
    }

    pub(super) fn set_text(&mut self, id: Id, bytes: Vec<u8>) {
        if let Some(t) = self.sprites.get_mut(id).text.as_mut() {
            t.bytes = bytes;
        }
    }

    /// A text with the font's own RGB (flag 8 kept), no group position.
    pub(super) fn kept_text(bytes: Vec<u8>, size: f32) -> Text {
        let mut t = Text::new(bytes, size);
        t.group_position = false;
        t
    }

    pub(super) fn close_opt(&mut self, seq: &Sequence, id: Option<Id>, group: usize) {
        if let Some(i) = id
            && self.sprites.alive(i)
        {
            self.sprites.close(seq, i, group);
        }
    }

    pub(super) fn kill_opt(&mut self, id: Option<Id>) {
        if let Some(i) = id {
            self.sprites.kill(i);
        }
    }

    pub(super) fn rows_alive(&self, list: &MenuList) -> bool {
        list.rows.iter().any(|&r| self.sprites.alive(r))
    }

    // ---- state 1 ---------------------------------------------------------

    /// FUN_14034ff50: tab 0, a row per member plus Stock, the preview
    /// panel with its 20 bars and texts, and the help.
    pub(super) fn enter_items(&mut self, seq: &Sequence, cursor: Option<usize>) {
        self.state = State::Items;
        self.substate = 0;
        self.create_tab(seq, 0);
        let members = self.data.panels.members.len();
        let n = members + 1;
        if let Some(c) = cursor {
            self.memory.items_row = c;
        }
        let cursor = self.memory.items_row.min(n - 1);
        let pitch = seq.groups[TOP_ROWS.appear].text_position_x as f32;
        let mut rows = Vec::new();
        for i in 0..n {
            let label = if i < members {
                self.data.panels.members[i].name.clone()
            } else {
                self.data.items.texts.stock.clone()
            };
            let queued = if i == cursor {
                TOP_ROWS.selected
            } else {
                TOP_ROWS.idle
            };
            let t = self.row_text(label);
            let r = self.sprites.create(
                seq,
                PRIO_ITEM_ROW,
                ART,
                TOP_ROWS.appear,
                Some(queued),
                Some(t),
            );
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            self.closers.push((r, TOP_ROWS.close));
            rows.push(r);
        }
        // The Stock row's NEW! badge (FUN_14036a8f0(−1) / a7f0(−1)).
        self.item_ui.stock_badge = None;
        let new: Vec<u16> = self.new_stock_items();
        if !new.is_empty() {
            let all_seen = new.iter().all(|id| self.data.items.seen.contains(id));
            let g = &seq.groups[TOP_ROWS.appear];
            let offset = [g.light_position_x as f32, g.text_position_y as f32];
            let b = self.sprites.create(
                seq,
                PRIO_ITEM_BADGE,
                ART,
                BADGE[all_seen as usize],
                None,
                None,
            );
            self.child(b, rows[n - 1]);
            self.sprites.get_mut(b).offset = offset;
            self.item_ui.stock_badge = Some(b);
        }
        self.item_ui.list = MenuList::new(rows, cursor);
        let panel = self.sprites.create(
            seq,
            PRIO_ITEM_PANEL,
            ART,
            STATUS_PANEL[0],
            Some(STATUS_PANEL[1]),
            None,
        );
        self.closers.push((panel, STATUS_PANEL[2]));
        self.item_ui.panel = Some(panel);
        let g = &seq.groups[BAR_WEAPON];
        let pitch = [g.text_position_x as f32, g.ui_padding as f32];
        self.item_ui.bars.clear();
        self.item_ui.texts.clear();
        for k in 0..20 {
            let l = self
                .sprites
                .create(seq, PRIO_ITEM_BAR, ART, BAR_WEAPON, None, None);
            self.sprites.set(seq, l, BLANK, None);
            self.child(l, panel);
            self.sprites.get_mut(l).offset =
                [pitch[0] * (k & 1) as f32, pitch[1] * (k >> 1) as f32];
            let v = self.sprites.create(
                seq,
                PRIO_ITEM_TEXT,
                ART,
                WHITE,
                None,
                Some(self.row_text(Vec::new())),
            );
            self.child(v, l);
            self.item_ui.bars.push(l);
            self.item_ui.texts.push(v);
        }
        self.item_preview(seq);
        self.items_help(seq);
        // SQUARE opens Abilities (mask 0x100) with its banner 0xb619.
        if self.entry_enabled(1) {
            self.item_ui.list.mask |= 0x100;
            let t = self.data.abilities.texts.abilities_banner.clone();
            self.show_banner(seq, t);
        }
    }

    /// Items of the "new" types (0x1405bc560) not yet in the stock list's
    /// seen bits.
    /// FUN_14036a8f0 / FUN_14036a7f0: `Some(seen)` while item `id` is new.
    pub(super) fn item_badge(&self, id: u16) -> Option<bool> {
        (id != 0 && self.data.items.new_items.contains(&id))
            .then(|| self.data.items.seen.contains(&id))
    }

    pub(super) fn magic_badge(&self, id: u16) -> Option<bool> {
        magic_badge(&self.data.items, id)
    }

    pub(super) fn any_magic_badge(&self) -> Option<bool> {
        any_magic_badge(&self.data.items)
    }

    pub(super) fn new_stock_items(&self) -> Vec<u16> {
        self.data
            .items
            .new_items
            .iter()
            .copied()
            .filter(|&id| matches!(self.info(id).kind, 0..=17 | 20))
            .collect()
    }

    /// FUN_1403504e0: the cursor row's preview (camp-05 §2.2).
    fn item_preview(&mut self, seq: &Sequence) {
        for k in 0..self.item_ui.bars.len() {
            let (l, v) = (self.item_ui.bars[k], self.item_ui.texts[k]);
            self.sprites.set(seq, l, BLANK, None);
            self.sprites.set(seq, v, WHITE, None);
            self.set_text(v, Vec::new());
        }
        let members = self.data.panels.members.len();
        let cursor = self.item_ui.list.cursor;
        // (bar group, text style, text) per slot; None = start a new row.
        let mut slots: Vec<Option<(usize, usize, Vec<u8>)>> = Vec::new();
        if cursor < members {
            let e = self
                .data
                .items
                .equipment
                .get(cursor)
                .cloned()
                .unwrap_or_default();
            let blank = |icon: u8, me: &Self| {
                let mut out = vec![0x09, icon];
                out.extend(me.data.items.texts.blank.clone());
                out
            };
            let named = |id: u16, glyph: u8, me: &Self| {
                if id == 0 {
                    blank(glyph, me)
                } else {
                    me.item_name(id, false)
                }
            };
            slots.push(Some((BAR_WEAPON, WHITE, named(e.weapon, 0xb, self))));
            for &f in &e.forms {
                slots.push(Some((BAR_FORM, WHITE, named(f, 0xb, self))));
            }
            slots.push(None);
            for &a in &e.armor {
                slots.push(Some((BAR_ARMOR, WHITE, named(a, 7, self))));
            }
            slots.push(None);
            for &a in &e.accessories {
                slots.push(Some((BAR_ACCESSORY, WHITE, named(a, 0x11, self))));
            }
            slots.push(None);
            for (j, &id) in e.items.iter().enumerate() {
                let original = e.originals.get(j).copied().unwrap_or(0);
                let (shown, counted) = if id == 0 && original != 0 {
                    (original, false)
                } else {
                    (id, true)
                };
                let alt = shown != 0 && shown == original;
                let text = if shown == 0 {
                    blank(0, self)
                } else {
                    self.item_name(shown, alt)
                };
                // FUN_140304d80: a consumed original shows red.
                let style = if self.info(shown).kind == 0 && alt && !counted {
                    RED
                } else {
                    WHITE
                };
                slots.push(Some((BAR_ITEM, style, text)));
            }
        } else {
            for (id, _) in stock_list(&self.data.items, 0).into_iter().take(20) {
                slots.push(Some((BAR_ITEM, WHITE, self.item_name(id, false))));
            }
        }
        let mut k = 0usize;
        for s in slots {
            match s {
                None => k = k.next_multiple_of(2),
                Some((bar, style, text)) if k < 20 => {
                    let (l, v) = (self.item_ui.bars[k], self.item_ui.texts[k]);
                    self.sprites.set(seq, l, bar, None);
                    self.sprites.set(seq, v, style, None);
                    self.set_text(v, text);
                    k += 1;
                }
                Some(_) => {}
            }
        }
    }

    /// FUN_1403508c0 for state 1.
    fn items_help(&mut self, seq: &Sequence) {
        let last = self.item_ui.list.total.saturating_sub(1);
        let h = self.data.items.texts.help[(self.item_ui.list.cursor >= last) as usize].clone();
        self.show_help(seq, h);
    }

    fn close_items_screen(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.item_ui.panel, STATUS_PANEL[2]);
        for &r in &self.item_ui.list.rows.clone() {
            self.close_opt(seq, Some(r), TOP_ROWS.close);
        }
    }

    /// FUN_14034df80.
    pub(super) fn items(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let prev = self.item_ui.list.cursor;
        let (pick, moved) = self.item_ui.list.input(pad);
        if self.substate == 2 {
            let busy = self.alive_opt(self.item_ui.panel)
                || self.rows_alive(&self.item_ui.list.clone())
                || self.alive_opt(self.item_ui.stock_badge);
            let p = self.pending;
            let leaving = matches!(p, Some(State::Top) | Some(State::AbilityChars));
            let equip = p == Some(State::Equip);
            if !busy && (equip || !self.alive_opt(self.banner)) && (!leaving || !self.tab_alive(0))
            {
                match self.pending.take() {
                    Some(State::Equip) => {
                        self.kill_opt(self.banner);
                        self.enter_equip(seq);
                    }
                    Some(State::AbilityChars) => {
                        let m = self.memory.items_row;
                        let member = if m < self.data.panels.members.len() {
                            m
                        } else {
                            0
                        };
                        self.abilities_from_items(seq, member);
                    }
                    Some(State::Stock) => {
                        self.item_ui.tab = 0;
                        self.memory.stock = (0, 0);
                        self.enter_stock(seq);
                    }
                    _ => self.return_top(seq),
                }
            } else if pad.trig != 0 {
                self.kill_opt(self.item_ui.panel);
                for &r in &self.item_ui.list.rows {
                    self.sprites.kill(r);
                }
                self.kill_opt(self.banner);
                if leaving {
                    self.kill_tab(0);
                    self.kill_tab(1);
                }
            }
            return false;
        }
        let row0 = self.group_of(self.item_ui.list.rows.first().copied());
        if (pick != Pick::None || moved) && row0 == Some(TOP_ROWS.appear) {
            for (i, &r) in self.item_ui.list.rows.clone().iter().enumerate() {
                let g = if i == prev {
                    TOP_ROWS.selected
                } else {
                    TOP_ROWS.idle
                };
                self.sprites.set(seq, r, g, None);
            }
            for i in 0..self.tabs.len() {
                if let Some(tab) = &self.tabs[i]
                    && self.sprites.alive(tab.id)
                {
                    let id = tab.id;
                    self.sprites.set(seq, id, TAB[1], None);
                }
            }
        }
        let row0 = self.group_of(self.item_ui.list.rows.first().copied());
        if self.substate == 0
            && matches!(row0, Some(g) if g == TOP_ROWS.idle || g == TOP_ROWS.selected)
        {
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            self.item_preview(seq);
            self.items_help(seq);
        }
        let n = self.item_ui.list.total;
        let cursor = self.item_ui.list.cursor;
        match pick {
            Pick::Start => {
                self.memory.items_row = cursor;
                self.exit = true;
            }
            Pick::Cancel | Pick::Square => {
                self.memory.items_row = cursor;
                self.hide_help(seq);
                self.close_banner(seq);
                self.close_tab(seq, 0);
                self.close_items_screen(seq);
                self.pending = Some(if pick == Pick::Square {
                    State::AbilityChars
                } else {
                    State::Top
                });
                self.substate = 2;
            }
            // Stock; a member row would go to Equip (state 2, not built).
            Pick::Row(r) if r + 1 == n => {
                self.memory.items_row = cursor;
                let label = self.data.items.texts.stock.clone();
                self.tab_text(1, label);
                self.close_banner(seq);
                self.hide_help(seq);
                self.close_items_screen(seq);
                self.pending = Some(State::Stock);
                self.substate = 2;
            }
            // A member row: Equip (state 2), the banner kept.
            Pick::Row(r) => {
                if self.memory.items_row != r {
                    self.memory.equip = (0, 0);
                }
                self.memory.items_row = r;
                let name = self.data.panels.members[r].name.clone();
                self.tab_text(1, name);
                self.hide_help(seq);
                self.close_items_screen(seq);
                self.pending = Some(State::Equip);
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }

    // ---- state 10 --------------------------------------------------------

    /// FUN_14034fa20: the stock frame, 18 rows with badges, the scrollbar,
    /// the grid list and (if gone) the category tab bar.
    pub(super) fn enter_stock(&mut self, seq: &Sequence) {
        self.state = State::Stock;
        self.substate = 0;
        self.create_tab(seq, 1);
        let f = self.sprites.create(
            seq,
            PRIO_STOCK_FRAME,
            ART,
            STOCK_FRAME[0],
            Some(STOCK_FRAME[1]),
            None,
        );
        self.closers.push((f, STOCK_FRAME[2]));
        self.item_ui.frame = Some(f);
        self.picture.setup(PRIO_STOCK_PICTURE, Some(PICTURE_FRAME));
        self.item_ui.entries = stock_list(&self.data.items, self.item_ui.tab);
        let c = self.item_ui.entries.len();
        let g = &seq.groups[STOCK_ROWS.idle];
        let pitch = [g.text_position_x as f32, g.ui_padding as f32];
        let badge_at = [g.light_position_x as f32, g.text_position_y as f32];
        let mut rows = Vec::new();
        self.item_ui.badges.clear();
        for i in 0..PAGE {
            let r = self.sprites.create(
                seq,
                PRIO_STOCK_ROW,
                ART,
                STOCK_ROWS.idle,
                None,
                Some(Self::kept_text(Vec::new(), TEXT_SIZE)),
            );
            self.child(r, f);
            self.sprites.get_mut(r).offset = [pitch[0] * (i % 2) as f32, pitch[1] * (i / 2) as f32];
            if i < c {
                // FUN_14034e330: the count, right-aligned at the row's
                // (LightPositionX, TextPositionY), in the row's colour.
                let mut t = Text::new(Vec::new(), TEXT_SIZE);
                t.align = 1;
                t.keep_rgb = false;
                self.sprites.get_mut(r).after = Some(t);
            }
            let b = self
                .sprites
                .create(seq, PRIO_STOCK_BADGE, ART, BLANK, None, None);
            self.child(b, r);
            self.sprites.get_mut(b).offset = badge_at;
            self.item_ui.badges.push(b);
            rows.push(r);
        }
        let n = c.max(1);
        self.build_scrollbar(seq, f, PRIO_STOCK_ROW, n.div_ceil(2), LINES);
        let (mut cur, mut sc) = self.memory.stock;
        if n <= cur + sc {
            if cur > 0 {
                cur -= 1;
            } else if sc > 0 {
                sc -= 2;
            }
        }
        let cur = cur.min(n - 1);
        self.sprites.set(seq, rows[cur], STOCK_ROWS.selected, None);
        let mut list = MenuList::new(rows, cur);
        list.total = n;
        list.mask = 0x7f | 0x3000 | 0xc000;
        if c == 0 {
            list.mask &= !0x10;
        }
        self.item_ui.grid = Grid {
            list,
            scroll: sc,
            cols: COLS,
            lines: LINES,
        };
        self.stock_page(seq);
        self.mark_seen(seq);
        self.item_ui.no_pointer = c == 0;
        if !self.alive_opt(self.item_ui.bar) {
            self.build_tab_bar(seq);
        }
    }

    /// FUN_1402fa930: the scrollbar's three players on `parent` at its
    /// group's (LightPositionX, TextPositionY); the track is that group's
    /// TextScale.
    pub(super) fn build_scrollbar(
        &mut self,
        seq: &Sequence,
        parent: Id,
        prio: u32,
        rows_total: usize,
        visible: usize,
    ) {
        let group = self.sprites.get(parent).player.group;
        let length = seq.groups[group].text_scale as f32;
        let t = seq.groups[SCROLL[1][0]].text_scale as f32;
        let total = rows_total.max(visible);
        let s = (visible as f32 / total as f32).min(1.) * length / t;
        self.item_ui.scroll_scale = s;
        self.item_ui.scroll_parent = Some(parent);
        self.item_ui.scroll.clear();
        for (k, g) in SCROLL.iter().enumerate() {
            let id = self.sprites.create(seq, prio, ART, g[0], Some(g[1]), None);
            self.child(id, parent);
            if k == 1 {
                self.sprites.get_mut(id).scale = [1., s];
            }
            self.closers.push((id, g[2]));
            self.item_ui.scroll.push(id);
        }
        self.scroll_to(seq, 0, rows_total, visible);
    }

    /// FUN_1402facd0: the thumb at the first visible row.
    pub(super) fn scroll_to(
        &mut self,
        seq: &Sequence,
        row0: usize,
        rows_total: usize,
        visible: usize,
    ) {
        let Some(parent) = self.item_ui.scroll_parent else {
            return;
        };
        let g = &seq.groups[self.sprites.get(parent).player.group];
        let origin = [g.light_position_x as f32, g.text_position_y as f32];
        let length = g.text_scale as f32;
        let t = seq.groups[SCROLL[1][0]].text_scale as f32;
        let total = rows_total.max(visible);
        let row0 = row0.min(total - visible);
        let y = ((length / total as f32) * row0 as f32) as i32 as f32;
        let thumb = (t * self.item_ui.scroll_scale) as i32 as f32;
        for (k, &id) in self.item_ui.scroll.clone().iter().enumerate() {
            let dy = if k == 2 { y + thumb } else { y };
            self.sprites.get_mut(id).offset = [origin[0], origin[1] + dy];
        }
    }

    /// The scrollbar takes a new row count (its thumb length, then its
    /// place).
    pub(super) fn scroll_total(
        &mut self,
        seq: &Sequence,
        row0: usize,
        rows_total: usize,
        visible: usize,
    ) {
        let Some(parent) = self.item_ui.scroll_parent else {
            return;
        };
        let length = seq.groups[self.sprites.get(parent).player.group].text_scale as f32;
        let t = seq.groups[SCROLL[1][0]].text_scale as f32;
        let total = rows_total.max(visible).max(1);
        let s = (visible as f32 / total as f32).min(1.) * length / t;
        self.item_ui.scroll_scale = s;
        if let Some(&thumb) = self.item_ui.scroll.get(1) {
            self.sprites.get_mut(thumb).scale = [1., s];
        }
        self.scroll_to(seq, row0, rows_total, visible);
    }

    /// The scrollbar closes (FUN_1402fab60).
    pub(super) fn close_scrollbar(&mut self, seq: &Sequence) {
        for (k, &id) in self.item_ui.scroll.clone().iter().enumerate() {
            self.close_opt(seq, Some(id), SCROLL[k][2]);
        }
    }
    /// FUN_1403509b0: the page's names, counts and badges, then the help.
    fn stock_page(&mut self, seq: &Sequence) {
        let rows = self.item_ui.grid.list.rows.clone();
        for (j, &r) in rows.iter().enumerate() {
            self.set_text(r, Vec::new());
            if let Some(t) = self.sprites.get_mut(r).after.as_mut() {
                t.bytes.clear();
            }
            let b = self.item_ui.badges[j];
            self.sprites.set(seq, b, BLANK, None);
        }
        let c = self.item_ui.entries.len();
        if c == 0 {
            self.sprites.set(seq, rows[0], STOCK_EMPTY, None);
            let empty = self.data.items.texts.empty.clone();
            let s = self.sprites.get_mut(rows[0]);
            if let Some(t) = s.text.as_mut() {
                t.bytes = empty;
                t.align = 2;
            }
        } else {
            let first = self.item_ui.grid.scroll;
            self.scroll_to(seq, first / 2, c.div_ceil(2), LINES);
            for (j, &row) in rows.iter().enumerate() {
                let Some(&(id, n)) = self.item_ui.entries.get(first + j) else {
                    break;
                };
                let name = self.item_name(id, false);
                self.set_text(row, name);
                if let Some(t) = self.sprites.get_mut(row).after.as_mut() {
                    t.bytes = count_text(n);
                }
                if self.data.items.new_items.contains(&id) {
                    let seen = self.data.items.seen.contains(&id);
                    self.sprites
                        .set(seq, self.item_ui.badges[j], BADGE[seen as usize], None);
                }
            }
        }
        self.stock_help(seq);
    }

    /// FUN_140350b80: the cursor item counts as seen.
    fn mark_seen(&mut self, seq: &Sequence) {
        let Some(&(id, _)) = self.item_ui.entries.get(self.item_ui.grid.index()) else {
            return;
        };
        if self.data.items.new_items.contains(&id) {
            if !self.data.items.seen.contains(&id) {
                self.data.items.seen.push(id);
            }
            let b = self.item_ui.badges[self.item_ui.grid.list.cursor];
            self.sprites.set(seq, b, BADGE[1], None);
        }
    }

    /// FUN_1403508c0 for states 10–12: the item's description.
    fn stock_help(&mut self, seq: &Sequence) {
        let at = if self.state == State::Stock {
            self.item_ui.grid.index()
        } else {
            self.item_ui.item
        };
        let help = self
            .item_ui
            .entries
            .get(at)
            .map(|&(id, _)| self.info(id).description)
            .unwrap_or_default();
        self.show_help(seq, help);
        // FUN_1403508c0 in state 10 also asks for the picture.
        if self.state == State::Stock {
            let picture = self
                .item_ui
                .entries
                .get(at)
                .map(|&(id, _)| self.info(id).picture);
            if picture.is_some() {
                self.picture_request(seq, picture);
            }
        }
    }

    /// FUN_14034ccc0: the bar, its five labels, the L2/R2 arrows and the
    /// highlight on the current tab.
    fn build_tab_bar(&mut self, seq: &Sequence) {
        let b = self
            .sprites
            .create(seq, PRIO_TAB_BAR, ART, TAB_BAR[0], Some(TAB_BAR[1]), None);
        self.closers.push((b, TAB_BAR[2]));
        self.item_ui.bar = Some(b);
        let g = &seq.groups[TAB_BAR[0]];
        let (pitch, size) = (g.text_position_x as f32, g.text_scale as f32);
        for k in 0..5 {
            let mut t = Text::new(self.data.items.texts.tabs[k].clone(), size);
            t.align = 2;
            t.group_position = false;
            let l = self
                .sprites
                .create(seq, PRIO_TAB_LABEL, ART, WHITE, None, Some(t));
            self.child(l, b);
            self.sprites.get_mut(l).offset = [pitch * k as f32, 0.];
        }
        self.item_ui.arrows.clear();
        // {09, L2 glyph 0xe2} and {09, R2 glyph 0xe3} (table 0x1405ba7e0).
        for (k, glyph) in [0xe2u8, 0xe3].into_iter().enumerate() {
            let mut t = Text::new(vec![0x09, glyph], 18.);
            t.align = 2;
            let a = self.sprites.create(
                seq,
                PRIO_TAB_LABEL,
                ART,
                ARROWS[k][0],
                Some(ARROWS[k][1]),
                Some(t),
            );
            self.closers.push((a, ARROWS[k][2]));
            self.item_ui.arrows.push(a);
        }
        let h = self
            .sprites
            .create(seq, PRIO_HIGHLIGHT, ART, HIGHLIGHT, None, None);
        self.child(h, b);
        self.item_ui.highlight = Some(h);
        self.move_highlight(seq);
    }

    /// FUN_140350410.
    fn move_highlight(&mut self, seq: &Sequence) {
        let pitch = seq.groups[TAB_BAR[0]].text_position_x as f32;
        if let Some(h) = self.item_ui.highlight {
            self.sprites.get_mut(h).offset = [pitch * self.item_ui.tab as f32, 0.];
        }
    }

    fn close_tab_bar(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.item_ui.bar, TAB_BAR[2]);
        for (k, &a) in self.item_ui.arrows.clone().iter().enumerate() {
            self.close_opt(seq, Some(a), ARROWS[k][2]);
        }
    }

    fn tab_bar_alive(&self) -> bool {
        self.alive_opt(self.item_ui.bar)
            || self.item_ui.arrows.iter().any(|&a| self.sprites.alive(a))
    }

    /// FUN_14034e6a0: the frame and scrollbar close.
    fn close_stock_list(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.item_ui.frame, STOCK_FRAME[2]);
        self.close_scrollbar(seq);
    }

    /// FUN_14034dba0.
    pub(super) fn stock(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let scroll = self.item_ui.grid.scroll;
        let (pick, moved) = self.item_ui.grid.input(pad);
        if self.substate == 2 {
            let p = self.pending;
            let busy = self.alive_opt(self.item_ui.frame)
                || self.rows_alive(&self.item_ui.grid.list.clone())
                || self.item_ui.scroll.iter().any(|&s| self.sprites.alive(s))
                || (p != Some(State::Stock) && self.tab_bar_alive())
                || (p == Some(State::Items) && self.tab_alive(1))
                || self.item_ui.badges.iter().any(|&b| self.sprites.alive(b))
                || self.picture.alive();
            if !busy {
                self.item_ui.no_pointer = false;
                match self.pending.take() {
                    Some(State::Items) => {
                        self.kill_tab(1);
                        self.enter_items(seq, None);
                    }
                    Some(State::Stock) => self.enter_stock(seq),
                    _ => self.enter_command(seq),
                }
            } else if pad.trig != 0 {
                self.kill_opt(self.item_ui.frame);
                for &r in &self.item_ui.grid.list.rows {
                    self.sprites.kill(r);
                }
                for &s in &self.item_ui.scroll {
                    self.sprites.kill(s);
                }
                if p != Some(State::Stock) {
                    self.kill_opt(self.item_ui.bar);
                    for &a in &self.item_ui.arrows {
                        self.sprites.kill(a);
                    }
                }
            }
            return false;
        }
        if (pick != Pick::None || moved)
            && self.group_of(self.item_ui.frame) != Some(STOCK_FRAME[1])
        {
            if let Some(f) = self.item_ui.frame {
                self.sprites.set(seq, f, STOCK_FRAME[1], None);
            }
            if let Some(b) = self.item_ui.bar {
                self.sprites.set(seq, b, TAB_BAR[1], None);
            }
            for (k, &a) in self.item_ui.arrows.clone().iter().enumerate() {
                if self.sprites.alive(a) {
                    self.sprites.set(seq, a, ARROWS[k][1], None);
                }
            }
            for i in 0..self.tabs.len() {
                if let Some(tab) = &self.tabs[i]
                    && self.sprites.alive(tab.id)
                {
                    let id = tab.id;
                    self.sprites.set(seq, id, TAB[1], None);
                }
            }
        }
        if self.substate == 0 && self.group_of(self.item_ui.frame) == Some(STOCK_FRAME[1]) {
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            if self.item_ui.grid.scroll != scroll {
                self.stock_page(seq);
            }
            self.mark_seen(seq);
            self.stock_help(seq);
        }
        match pick {
            Pick::Next | Pick::Previous => {
                self.item_ui.tab = if pick == Pick::Next {
                    (self.item_ui.tab + 1) % 5
                } else {
                    (self.item_ui.tab + 4) % 5
                };
                self.move_highlight(seq);
                self.hide_help(seq);
                self.close_stock_list(seq);
                self.picture_close(seq);
                self.memory.stock = (0, 0);
                self.pending = Some(State::Stock);
                self.substate = 2;
            }
            Pick::Start => self.exit = true,
            Pick::Cancel => {
                self.hide_help(seq);
                self.close_tab_bar(seq);
                self.close_tab(seq, 1);
                self.close_stock_list(seq);
                self.picture_close(seq);
                self.memory.stock = (0, 0);
                self.pending = Some(State::Items);
                self.substate = 2;
            }
            Pick::Row(a) => {
                self.item_ui.item = a;
                self.hide_help(seq);
                self.close_stock_list(seq);
                self.close_tab_bar(seq);
                self.picture_close(seq);
                self.memory.stock = (self.item_ui.grid.list.cursor, self.item_ui.grid.scroll);
                self.pending = Some(State::ItemCommand);
                self.substate = 2;
            }
            _ => {}
        }
        if self.substate == 1 {
            self.picture_timers(seq);
        }
        moved
    }

    // ---- state 11 --------------------------------------------------------

    fn chosen(&self) -> ItemInfo {
        self.item_ui
            .entries
            .get(self.item_ui.item)
            .map(|&(id, _)| self.info(id))
            .unwrap_or_default()
    }

    /// FUN_14034e8b0: row r's action, 0 Use, 1 Throw away, −1 invalid.
    fn command_action(&self, r: usize) -> i32 {
        let info = self.chosen();
        if matches!(info.kind, 0 | 1) && r == 0 {
            0
        } else if info.sell != 0 {
            1
        } else {
            -1
        }
    }

    /// FUN_14034ef80: the command frame, holder, the item's name and
    /// count, and the Use / Throw away rows.
    fn enter_command(&mut self, seq: &Sequence) {
        self.state = State::ItemCommand;
        self.substate = 0;
        let info = self.chosen();
        let count = self
            .item_ui
            .entries
            .get(self.item_ui.item)
            .map_or(0, |e| e.1);
        if !self.alive_opt(self.item_ui.command) {
            let c = self.sprites.create(
                seq,
                PRIO_COMMAND,
                ART,
                COMMAND_FRAME[0],
                Some(COMMAND_FRAME[1]),
                None,
            );
            self.closers.push((c, COMMAND_FRAME[2]));
            self.item_ui.command = Some(c);
        }
        let c = self.item_ui.command.unwrap();
        if !self.alive_opt(self.item_ui.holder) {
            let h = self
                .sprites
                .create(seq, PRIO_COMMAND, ART, HOLDER[0], Some(HOLDER[1]), None);
            self.closers.push((h, HOLDER[2]));
            self.item_ui.holder = Some(h);
        }
        self.picture.setup(PRIO_COMMAND_PICTURE, None);
        if let Some(h) = self.item_ui.holder {
            self.picture.set_parent(h);
        }
        self.picture.reset();
        if !self.alive_opt(self.item_ui.name) {
            let name = self.item_name(info.id, false);
            let nm = self.sprites.create(
                seq,
                PRIO_COMMAND_ROW,
                ART,
                NAME,
                None,
                Some(Self::kept_text(name, TEXT_SIZE)),
            );
            self.child(nm, c);
            self.item_ui.name = Some(nm);
            let g = &seq.groups[NAME];
            let at = [g.light_position_x as f32, g.text_position_y as f32];
            let mut t = self.row_text(count_text(count));
            t.align = 1;
            let ct = self
                .sprites
                .create(seq, PRIO_COMMAND_LABEL, ART, YELLOW, None, Some(t));
            self.child(ct, nm);
            self.sprites.get_mut(ct).offset = at;
            self.item_ui.count = Some(ct);
        }
        if !self.rows_alive(&self.item_ui.commands.clone()) {
            let consumable = matches!(info.kind, 0 | 1);
            let labels: Vec<Vec<u8>> = if consumable {
                vec![
                    self.data.items.texts.use_.clone(),
                    self.data.items.texts.throw.clone(),
                ]
            } else {
                vec![self.data.items.texts.throw.clone()]
            };
            let pitch = seq.groups[COMMAND_ROWS.idle].ui_padding as f32;
            let mut rows = Vec::new();
            for (i, label) in labels.into_iter().enumerate() {
                let r =
                    self.sprites
                        .create(seq, PRIO_COMMAND_ROW, ART, COMMAND_ROWS.idle, None, None);
                self.child(r, c);
                self.sprites.get_mut(r).offset = [0., pitch * i as f32];
                let is_use = consumable && i == 0;
                let style = if info.sell == 0 && !is_use {
                    GREY
                } else {
                    WHITE
                };
                let v = self.sprites.create(
                    seq,
                    PRIO_COMMAND_LABEL,
                    ART,
                    style,
                    None,
                    Some(self.row_text(label)),
                );
                self.child(v, r);
                rows.push(r);
            }
            self.item_ui.commands = MenuList::new(rows, 0);
        }
        self.place_marker(seq, false);
        self.stock_help(seq);
        if !self.picture.alive() {
            self.picture_request(seq, Some(info.picture));
        }
    }

    /// FUN_140350d40: the selection marker (G290) on the list's row.
    fn place_marker(&mut self, seq: &Sequence, target: bool) {
        let (list, slot) = if target {
            (&self.item_ui.targets, self.item_ui.target_marker)
        } else {
            (&self.item_ui.commands, self.item_ui.marker)
        };
        let Some(&row) = list.rows.get(list.cursor) else {
            return;
        };
        let id = match slot {
            Some(m) if self.sprites.alive(m) => m,
            _ => {
                let m = self
                    .sprites
                    .create(seq, PRIO_MARKER, ART, MARKER, None, None);
                self.sprites.get_mut(m).die_with_parent = true;
                if target {
                    self.item_ui.target_marker = Some(m);
                } else {
                    self.item_ui.marker = Some(m);
                }
                m
            }
        };
        self.sprites.get_mut(id).parent = Some(row);
    }

    /// The current hand stays, dimmed (alt group), on its row.
    pub(super) fn dim_glove(&mut self, seq: &Sequence) {
        if let Some(g) = self.glove.take()
            && self.sprites.alive(g)
        {
            self.sprites.set(seq, g, GLOVE_DIM, None);
            self.item_ui.dim.push(g);
        }
    }

    pub(super) fn undim_last(&mut self) {
        if let Some(g) = self.item_ui.dim.pop() {
            self.sprites.kill(g);
        }
    }

    /// FUN_14034ebb0: the whole command screen goes.
    fn kill_command_screen(&mut self) {
        for id in [
            self.item_ui.command,
            self.item_ui.holder,
            self.item_ui.marker,
        ] {
            self.kill_opt(id);
        }
        let dim = std::mem::take(&mut self.item_ui.dim);
        for g in dim {
            self.sprites.kill(g);
        }
    }

    fn command_screen_alive(&self) -> bool {
        self.alive_opt(self.item_ui.command) || self.alive_opt(self.item_ui.holder)
    }

    /// FUN_14034d2d0.
    pub(super) fn item_command(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let (pick, moved) = self.item_ui.commands.input(pad);
        if self.substate == 2 {
            let p = self.pending;
            let ready = if p == Some(State::Stock) {
                !self.command_screen_alive()
            } else {
                !self.alive_opt(self.item_ui.holder)
            } && !self.picture.alive();
            if ready {
                match self.pending.take() {
                    Some(State::Stock) => self.enter_stock(seq),
                    _ => self.enter_target(seq),
                }
            } else if pad.trig != 0 {
                self.picture.kill(&mut self.sprites);
                if p == Some(State::Stock) {
                    self.kill_command_screen();
                } else {
                    self.kill_opt(self.item_ui.holder);
                }
            }
            return false;
        }
        if (pick != Pick::None || moved)
            && self.group_of(self.item_ui.command) != Some(COMMAND_FRAME[1])
            && self.group_of(self.item_ui.holder) != Some(HOLDER[1])
        {
            for (id, g) in [
                (self.item_ui.command, COMMAND_FRAME[1]),
                (self.item_ui.holder, HOLDER[1]),
            ] {
                if let Some(i) = id
                    && self.sprites.alive(i)
                {
                    self.sprites.set(seq, i, g, None);
                }
            }
        }
        if self.substate == 0
            && (self.group_of(self.item_ui.command) == Some(COMMAND_FRAME[1])
                || self.group_of(self.item_ui.holder) == Some(HOLDER[1]))
        {
            self.substate = 1;
        }
        let action = match pick {
            Pick::Row(r) => self.command_action(r),
            _ => 0,
        };
        if let Some(s) = pick_sound(pick, moved, action >= 0) {
            sounds.push(s);
        }
        if moved {
            self.place_marker(seq, false);
        }
        if action < 0 {
            return moved;
        }
        match pick {
            Pick::Start => self.exit = true,
            Pick::Cancel => {
                self.hide_help(seq);
                self.close_opt(seq, self.item_ui.command, COMMAND_FRAME[2]);
                self.close_opt(seq, self.item_ui.holder, HOLDER[2]);
                self.picture_close(seq);
                self.picture.reset();
                self.pending = Some(State::Stock);
                self.substate = 2;
            }
            Pick::Row(_) if action == 0 => {
                // DAT_140743fdc = 0: the hand stays (dimmed) on "Use".
                self.dim_glove(seq);
                self.picture_close(seq);
                self.picture.reset();
                self.close_opt(seq, self.item_ui.holder, HOLDER[2]);
                self.pending = Some(State::UseTarget);
                self.substate = 2;
            }
            Pick::Row(_) => {
                let info = self.chosen();
                let texts = &self.data.items.texts;
                let category = texts
                    .categories
                    .get(info.kind as usize)
                    .cloned()
                    .unwrap_or_default();
                let text = format_message(&texts.throw_query, &[&category, &info.name]);
                let (yes, no) = (texts.yes.clone(), texts.no.clone());
                self.dim_glove(seq);
                self.open_dialog(seq, text, vec![no, yes]);
                self.state = State::ThrowAway;
                self.substate = 0;
            }
            _ => {}
        }
        if self.substate == 1 && self.state == State::ItemCommand {
            self.picture_timers(seq);
        }
        moved
    }

    // ---- states 13 / 14 --------------------------------------------------

    /// FUN_140306f30 + FUN_140306cc0: the box (G6 → 7) and its choice rows
    /// on the box at its group's (LightPositionX, TextPositionY).
    pub(super) fn open_dialog(&mut self, seq: &Sequence, text: Vec<u8>, choices: Vec<Vec<u8>>) {
        let size = seq.groups[DIALOG[0]].text_scale as f32;
        let mut t = Self::kept_text(text, size);
        t.align = 2;
        let d = self
            .sprites
            .create(seq, PRIO_DIALOG, ART, DIALOG[0], Some(DIALOG[1]), Some(t));
        self.closers.push((d, DIALOG[2]));
        self.item_ui.dialog = Some(d);
        let bg = &seq.groups[DIALOG[0]];
        let at = [bg.light_position_x as f32, bg.text_position_y as f32];
        let rg = &seq.groups[CHOICE_ROWS.appear];
        let (pitch, dx) = (rg.ui_padding as f32, rg.text_position_x as f32);
        let ok_only = choices.len() == 1;
        let mut rows = Vec::new();
        for (i, label) in choices.into_iter().enumerate() {
            let queued = if i == 0 {
                CHOICE_ROWS.selected
            } else {
                CHOICE_ROWS.idle
            };
            // FUN_1402eb960 clears flag 0x10 only: the font's RGB stays.
            let mut t = Self::kept_text(label, TEXT_SIZE);
            t.align = 2;
            t.offset = [dx, 0.];
            let r = self.sprites.create(
                seq,
                PRIO_CHOICE,
                ART,
                CHOICE_ROWS.appear,
                Some(queued),
                Some(t),
            );
            self.child(r, d);
            self.sprites.get_mut(r).offset = [at[0] + pitch * i as f32, at[1]];
            self.closers.push((r, CHOICE_ROWS.close));
            rows.push(r);
        }
        let mut list = MenuList::new(rows, 0);
        list.keys = Some((bits::RIGHT, bits::LEFT));
        list.mask &= if ok_only { !0x43 } else { !0x40 };
        self.item_ui.choices = list;
    }

    pub(super) fn close_dialog(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.item_ui.dialog, DIALOG[2]);
        for &r in &self.item_ui.choices.rows.clone() {
            self.close_opt(seq, Some(r), CHOICE_ROWS.close);
        }
    }

    pub(super) fn dialog_alive(&self) -> bool {
        self.alive_opt(self.item_ui.dialog) || self.rows_alive(&self.item_ui.choices.clone())
    }

    pub(super) fn kill_dialog(&mut self) {
        self.kill_opt(self.item_ui.dialog);
        for &r in &self.item_ui.choices.rows {
            self.sprites.kill(r);
        }
    }

    /// The box skips to idle on a press; substate 1 once idle.
    pub(super) fn dialog_intro(&mut self, seq: &Sequence, pressed: bool) {
        if pressed
            && self.group_of(self.item_ui.dialog) == Some(DIALOG[0])
            && let Some(d) = self.item_ui.dialog
        {
            self.sprites.set(seq, d, DIALOG[1], None);
        }
        if self.substate == 0 && self.group_of(self.item_ui.dialog) == Some(DIALOG[1]) {
            self.substate = 1;
        }
    }

    /// FUN_14034d120.
    pub(super) fn throw_away(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let (pick, moved) = self.item_ui.choices.input(pad);
        if self.substate == 2 {
            let p = self.pending;
            let ready =
                !self.dialog_alive() && (p != Some(State::Stock) || !self.command_screen_alive());
            if ready {
                match self.pending.take() {
                    Some(State::Stock) => {
                        self.kill_command_screen();
                        self.enter_stock(seq);
                    }
                    _ => {
                        self.undim_last();
                        self.enter_command(seq);
                    }
                }
            } else if pad.trig != 0 {
                self.kill_dialog();
                if p == Some(State::Stock) {
                    self.kill_command_screen();
                }
            }
            return false;
        }
        self.dialog_intro(seq, pad.trig != 0);
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        match pick {
            Pick::Row(0) | Pick::Cancel => {
                self.close_dialog(seq);
                self.pending = Some(State::ItemCommand);
                self.substate = 2;
            }
            Pick::Row(_) => {
                let id = self.chosen().id;
                self.remove_item(id);
                self.close_dialog(seq);
                self.close_opt(seq, self.item_ui.command, COMMAND_FRAME[2]);
                self.close_opt(seq, self.item_ui.holder, HOLDER[2]);
                self.picture_close(seq);
                self.picture.reset();
                self.pending = Some(State::Stock);
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }

    /// FUN_1403c49c0(id, 100): one fewer in stock.
    fn remove_item(&mut self, id: u16) {
        if let Some(n) = self.data.items.counts.get_mut(&id) {
            *n = n.saturating_sub(1);
        }
    }

    /// FUN_14034da70.
    pub(super) fn no_effect(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let (pick, moved) = self.item_ui.choices.input(pad);
        if self.substate == 2 {
            if !self.dialog_alive() {
                // Back to the target list as it was (no rebuild).
                self.undim_last();
                self.state = State::UseTarget;
                self.substate = 1;
                self.pending = None;
            } else if pad.trig != 0 {
                self.kill_dialog();
            }
            return false;
        }
        self.dialog_intro(seq, pad.trig != 0);
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if matches!(pick, Pick::Row(0) | Pick::Cancel) {
            self.close_dialog(seq);
            self.pending = Some(State::UseTarget);
            self.substate = 2;
        }
        moved
    }

    // ---- state 12 --------------------------------------------------------

    /// FUN_14034f5f0: the target frame, its caption, a row per member (or
    /// "All") and the party block in mode 1.
    fn enter_target(&mut self, seq: &Sequence) {
        self.state = State::UseTarget;
        self.substate = 0;
        let info = self.chosen();
        let t0 = self.sprites.create(
            seq,
            PRIO_COMMAND,
            ART,
            TARGET_FRAME[0],
            Some(TARGET_FRAME[1]),
            None,
        );
        self.closers.push((t0, TARGET_FRAME[2]));
        self.item_ui.target = Some(t0);
        let caption = self.sprites.create(
            seq,
            PRIO_COMMAND_ROW,
            ART,
            NAME,
            None,
            Some(Self::kept_text(
                self.data.items.texts.use_on.clone(),
                TEXT_SIZE,
            )),
        );
        self.child(caption, t0);
        self.item_ui.caption = Some(caption);
        let party_wide = PARTY_WIDE.contains(&info.id);
        let drive = DRIVE.contains(&info.id);
        self.item_ui.party_wide = party_wide;
        let members = self.data.panels.members.len();
        let labels: Vec<Vec<u8>> = if party_wide {
            vec![self.data.items.texts.all.clone()]
        } else if drive {
            vec![self.data.panels.members[0].name.clone()]
        } else {
            self.data
                .panels
                .members
                .iter()
                .map(|m| m.name.clone())
                .collect()
        };
        let pitch = seq.groups[COMMAND_ROWS.idle].ui_padding as f32;
        let mut rows = Vec::new();
        for (i, label) in labels.into_iter().enumerate() {
            let r = self.sprites.create(
                seq,
                PRIO_COMMAND_ROW,
                ART,
                COMMAND_ROWS.idle,
                None,
                Some(Self::kept_text(label, TEXT_SIZE)),
            );
            self.child(r, t0);
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            rows.push(r);
        }
        self.item_ui.targets = MenuList::new(rows, 0);
        let g = &seq.groups[BLOCK_MODE1[0]];
        let pitch = [g.text_position_x as f32, g.ui_padding as f32];
        let count = if drive { 1 } else { members };
        let block = self.status_block_with(seq, BLOCK_MODE1, count, |i| {
            [pitch[0] * (i % 2) as f32, pitch[1] * (i / 2) as f32]
        });
        self.item_ui.block = Some(block);
        self.place_marker(seq, true);
        self.stock_help(seq);
    }

    fn close_target(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.item_ui.target, TARGET_FRAME[2]);
        self.close_opt(seq, self.item_ui.block, BLOCK_MODE1[2]);
    }

    /// FUN_14034d640.
    pub(super) fn use_target(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let (pick, moved) = self.item_ui.targets.input(pad);
        if self.substate == 2 {
            let p = self.pending;
            let busy = self.alive_opt(self.item_ui.target)
                || self.alive_opt(self.item_ui.block)
                || (p == Some(State::Stock) && self.command_screen_alive());
            if !busy {
                self.kill_command_screen_dims();
                match self.pending.take() {
                    Some(State::Stock) => {
                        self.kill_command_screen();
                        self.enter_stock(seq);
                    }
                    _ => self.enter_command(seq),
                }
            } else if pad.trig != 0 {
                self.kill_opt(self.item_ui.target);
                self.kill_opt(self.item_ui.block);
                if p == Some(State::Stock) {
                    self.kill_command_screen();
                }
            }
            return false;
        }
        if (pick != Pick::None || moved)
            && self.group_of(self.item_ui.target) == Some(TARGET_FRAME[0])
            && let Some(t) = self.item_ui.target
        {
            self.sprites.set(seq, t, TARGET_FRAME[1], None);
        }
        if self.substate == 0 && self.group_of(self.item_ui.target) == Some(TARGET_FRAME[1]) {
            self.substate = 1;
        }
        if !matches!(pick, Pick::Row(_))
            && let Some(s) = pick_sound(pick, moved, true)
        {
            sounds.push(s);
        }
        if moved {
            self.place_marker(seq, true);
        }
        match pick {
            Pick::Start => {
                self.close_opt(seq, self.item_ui.block, BLOCK_MODE1[2]);
                self.exit = true;
            }
            Pick::Cancel => {
                self.close_target(seq);
                self.pending = Some(State::ItemCommand);
                self.substate = 2;
            }
            Pick::Row(r) => {
                let info = self.chosen();
                let targets: Vec<usize> = if self.item_ui.party_wide {
                    (0..self.data.panels.members.len()).collect()
                } else {
                    vec![r]
                };
                let ok = targets.iter().any(|&m| {
                    let mut member = self.data.panels.members[m].clone();
                    restore(&info, &mut member, false)
                });
                if ok {
                    sounds.push(SOUND_USE);
                    for &m in &targets {
                        restore(&info, &mut self.data.panels.members[m], true);
                    }
                    self.remove_item(info.id);
                    self.close_target(seq);
                    self.close_opt(seq, self.item_ui.command, COMMAND_FRAME[2]);
                    self.close_opt(seq, self.item_ui.holder, HOLDER[2]);
                    // FUN_140303bf0: hands and LightCur go.
                    self.kill_command_screen_dims();
                    if let Some(g) = self.glove.take() {
                        self.sprites.kill(g);
                    }
                    self.pending = Some(State::Stock);
                    self.substate = 2;
                } else {
                    sounds.push(SOUND_DECIDE);
                    let texts = &self.data.items.texts;
                    let category = texts
                        .categories
                        .get(info.kind as usize)
                        .cloned()
                        .unwrap_or_default();
                    let text = format_message(&texts.no_effect, &[&category, &info.name]);
                    let ok = texts.ok.clone();
                    self.dim_glove(seq);
                    self.open_dialog(seq, text, vec![ok]);
                    self.state = State::NoEffect;
                    self.substate = 0;
                }
            }
            _ => {}
        }
        moved
    }

    fn kill_command_screen_dims(&mut self) {
        let dim = std::mem::take(&mut self.item_ui.dim);
        for g in dim {
            self.sprites.kill(g);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> ItemsData {
        let item = |id: u16, kind: u8| ItemInfo {
            id,
            kind,
            ..ItemInfo::default()
        };
        ItemsData {
            table: vec![
                item(1, 0),
                item(0x29, 2),
                item(0x180, 2),
                item(2, 0),
                item(8, 14),
            ],
            counts: [(1, 3), (0x29, 1), (0x180, 1), (8, 2)]
                .into_iter()
                .collect(),
            struggle_hidden: true,
            ..ItemsData::default()
        }
    }

    #[test]
    fn stock_lists_follow_table_order_tabs_and_flags() {
        let d = data();
        assert_eq!(stock_list(&d, 0), vec![(1, 3), (0x29, 1), (8, 2)]);
        assert_eq!(stock_list(&d, 1), vec![(1, 3)]);
        assert_eq!(stock_list(&d, 2), vec![(0x29, 1)]);
        assert_eq!(stock_list(&d, 3), vec![(8, 2)]);
        assert!(stock_list(&d, 4).is_empty());
        assert_eq!(count_text(1), Vec::<u8>::new());
        assert_eq!(count_text(12), vec![0x2d, 0x22, 0x23]);
        assert_eq!(type_icon(0, true), 0xc);
        assert_eq!(type_icon(9, false), 0xb);
    }

    #[test]
    fn only_new_magic_carries_the_customize_badge() {
        let mut d = data();
        d.table.push(ItemInfo {
            id: 0x15,
            kind: MAGIC_KIND,
            ..ItemInfo::default()
        });
        d.new_items = vec![1, 0x15];
        // The potion is new but not magic.
        assert_eq!(magic_badge(&d, 1), None);
        assert_eq!(magic_badge(&d, 0x15), Some(false));
        assert_eq!(any_magic_badge(&d), Some(false));
        d.seen.push(0x15);
        assert_eq!(magic_badge(&d, 0x15), Some(true));
        assert_eq!(any_magic_badge(&d), Some(true));
        d.new_items = vec![1];
        assert_eq!(any_magic_badge(&d), None);
    }

    #[test]
    fn messages_take_their_arguments_in_order() {
        // "{07 ff 00 00 80}T{0d} x{0d}" with a colour code whose bytes
        // include 0x0d-like values left alone.
        let t = [0x07, 0x0d, 0x00, 0x00, 0x80, 0x34, 0x0d, 0x01, 0x0d];
        let out = format_message(&t, &[&[0x09, 0x00], &[0x30]]);
        assert_eq!(
            out,
            vec![0x07, 0x0d, 0x00, 0x00, 0x80, 0x34, 0x09, 0x00, 0x01, 0x30]
        );
    }

    #[test]
    fn potions_restore_a_share_of_max_hp_rounded_up() {
        let potion = ItemInfo {
            id: 1,
            amount: 40,
            ..ItemInfo::default()
        };
        let mut m = Member {
            name: Vec::new(),
            level: 1,
            hp: 10,
            max_hp: 21,
            mp: 0,
            max_mp: 100,
        };
        assert!(restore(&potion, &mut m, true));
        // ceil(21 · 40 / 100) = 9.
        assert_eq!(m.hp, 19);
        m.hp = 21;
        assert!(!restore(&potion, &mut m, false));
    }

    fn grid(n: usize) -> Grid {
        let mut list = MenuList::new((0..PAGE).map(Id).collect(), 0);
        list.total = n;
        list.mask = 0x7f | 0x3000 | 0xc000;
        Grid {
            list,
            scroll: 0,
            cols: COLS,
            lines: LINES,
        }
    }

    fn press(g: &mut Grid, bit: u64) -> (Pick, bool) {
        g.input(&Pad {
            trig: bit,
            repeat: bit,
        })
    }

    #[test]
    fn the_grid_moves_scrolls_and_wraps_as_the_exe() {
        use bits::*;
        let mut g = grid(21);
        assert_eq!(press(&mut g, RIGHT), (Pick::None, true));
        assert_eq!(g.index(), 1);
        // Right in the last column wraps inside the row.
        press(&mut g, RIGHT);
        assert_eq!(g.index(), 0);
        // Down 9 times: the page scrolls by a row at the bottom line.
        for _ in 0..9 {
            press(&mut g, DOWN);
        }
        assert_eq!((g.list.cursor, g.scroll), (16, 2));
        // Down on the last row (only item 20 there) clamps to it, then a
        // fresh press wraps to the top.
        press(&mut g, DOWN);
        assert_eq!(g.index(), 20);
        press(&mut g, DOWN);
        assert_eq!((g.list.cursor, g.scroll), (0, 0));
        // Up at the top wraps to the last page's same column.
        press(&mut g, UP);
        assert_eq!((g.list.cursor, g.scroll), (16, 4));
        // A held up at the top does not wrap.
        let mut g = grid(4);
        assert_eq!(
            g.input(&Pad {
                trig: 0,
                repeat: UP
            }),
            (Pick::None, false)
        );
        // R1 at the start jumps to the last item when within a page.
        assert!(press(&mut g, R1).1);
        assert_eq!(g.index(), 3);
        assert_eq!(press(&mut g, DECIDE), (Pick::Row(3), false));
    }

    #[test]
    fn only_items_gained_after_the_start_and_unseen_are_new() {
        let counts = BTreeMap::from([(1, 3), (2, 0), (3, 1), (4, 2)]);
        // 1 starts in stock, 2 is empty, 3 was seen: only 4 is new.
        assert_eq!(new_items(&counts, &[3], &[1]), vec![4]);
        assert_eq!(new_items(&counts, &[], &[]), vec![1, 3, 4]);
    }
}
