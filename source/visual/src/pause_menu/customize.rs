//! The camp's Customize screens (camp-07 spec): the character list with
//! its settings preview (state 0x19: FUN_14035d2a0 / FUN_14035aa80 /
//! FUN_14035e670), the settings list (0x1a / 0x1b: FUN_14035ccb0 /
//! FUN_14035d6a0 / FUN_14035a4e0) — for Sora four shortcut slots, MP Charge
//! per owned spell and Auto-Reload per item in his item slots; for a party
//! member the Battle Style, each equipped action ability's use frequency
//! and Auto-Reload with each item's use frequency (the glyphs of
//! FUN_14035add0) — the choice popups (0x1d–0x21: FUN_14035c750 /
//! FUN_14035dde0 / FUN_14035a180 / FUN_14035af10), the Auto-Reload warning
//! box (0x1c: FUN_14035c710 / FUN_14035a080) and the L2 / R2 member switch.
//! The NEW! marks are not built.
use super::abilities::{FREQUENCIES, frequency_glyph, frequency_record};
use super::items::{Grid, type_icon};
use super::*;

const PRIO_ROW: u32 = 0x7541;
const PRIO_PANEL: u32 = 0x753d;
const PRIO_BAR: u32 = 0x753e;
const PRIO_TEXT: u32 = 0x753f;
/// P (DAT_140743710) while the camp runs.
const P: u32 = 0x753c;

/// Preview bars per category (0x1405bc2b8): shortcut, MP Charge,
/// Auto-Reload, Battle Style, Abilities.
const PREVIEW: [usize; 5] = [149, 144, 143, 138, 137];
const LIST_PANEL: [usize; 3] = [227, 228, 229];
/// Row and heading groups per category (0x1405bc218 / 0x1405bc228).
const ROW: [usize; 5] = [264, 263, 262, 250, 248];
const HEADING: [usize; 5] = [282, 281, 280, 277, 276];
const LIST_FRAME: usize = 249;
const BUBBLE: usize = 24;
/// Popup panel {appear, close, idle}, header, rows, frame per category
/// (0x1405bc250 / 270 / 280 / 28a).
const POPUP_PANEL: [[usize; 3]; 5] = [
    [198, 200, 199],
    [189, 191, 190],
    [180, 182, 181],
    [162, 164, 163],
    [153, 155, 154],
];
const POPUP_HEADER: [usize; 5] = [206, 197, 188, 170, 161];
const POPUP_ROW: [usize; 5] = [202, 193, 184, 166, 157];
const POPUP_FRAME: [usize; 5] = [201, 192, 183, 165, 156];
/// Category order (0x1405bc1b8 Sora, 0x1405bc1bc party members).
const SORA_ORDER: [usize; 3] = [0, 1, 2];
const PARTY_ORDER: [usize; 3] = [3, 4, 2];
/// The item frequency table (0x1405bc208): icon and value of "Often" and
/// "Last hope!".
const ITEM_FREQUENCIES: [(u8, u8); 2] = [(0x1a, 0), (0x1c, 2)];
/// Battle styles (0x1405bc1c0): Technic .. Relentless.
const STYLES: u8 = 6;
const WHITE: usize = 123;
/// The yellow of the party item popup's frequency text (callback colour
/// [I]).
const YELLOW: usize = 124;
const VISIBLE: usize = 9;
const POPUP_VISIBLE: usize = 8;
/// Spells in MP-charge order (0x1405c5d18): Fire, Blizzard, Thunder, Cure,
/// Magnet, Reflect.
const SPELLS: [u16; 6] = [0x15, 0x16, 0x17, 0x18, 0x57, 0x58];
/// Cure's MP Charge cannot be changed (FUN_14035c180).
const CURE: u16 = 0x18;
/// Shortcut button glyphs in the 0x19 preview (0x1405bc298).
const SHORTCUT_GLYPHS: [u8; 4] = [0xdb, 0xdf, 0xde, 0xda];
const MAGIC_TYPE: u8 = 0x12;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomizeTexts {
    /// 0xbb89 (Sora) / 0xbb8a (party members).
    pub help: [Vec<u8>; 2],
    /// Category labels 0x1405bc238: 0x847d, 0x8481, 0x8482, 0x8483 Battle
    /// Style, 0x8484 Abilities.
    pub labels: [Vec<u8>; 5],
    /// Battle styles 0x8486..0x848b and their help 0x9e55..0x9e5a.
    pub styles: [Vec<u8>; 6],
    pub style_help: [Vec<u8>; 6],
    /// Ability frequencies 0x9e4e Free, 0x9e4f Well-Balanced, 0x9e50 Rare,
    /// help 0x9e5b..0x9e5d (each begins with a line break).
    pub ability_frequency: [Vec<u8>; 3],
    pub ability_frequency_help: [Vec<u8>; 3],
    /// Item frequencies 0x9e51 Often, 0x9e52 Last hope!, help 0x9e5e /
    /// 0x9e5f.
    pub item_frequency: [Vec<u8>; 2],
    pub item_frequency_help: [Vec<u8>; 2],
    /// Shortcut headings 0x847d..0x8480 (`09` button glyph).
    pub shortcut_headings: [Vec<u8>; 4],
    /// 0x9df7 "Remove Shortcut settings."
    pub remove_help: Vec<u8>,
    /// 0x9df9 / 0x9df8, help 0x9dfb / 0x9dfa.
    pub mp_choices: [Vec<u8>; 2],
    pub mp_help: [Vec<u8>; 2],
    /// 0x9dfd / 0x9dfc, help 0x9dff / 0x9dfe.
    pub reload_choices: [Vec<u8>; 2],
    pub reload_help: [Vec<u8>; 2],
    /// 0x846a: the Auto-Reload OFF warning.
    pub reload_warning: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomizeData {
    /// save+0x36f8: four shortcut command ids (0 = empty).
    pub shortcuts: [u16; 4],
    /// save+0x3744: bit m set = "Don't MP Charge" for spell m.
    pub no_charge: u8,
    /// Each member's battle style (record +0xf4; Sora's is unused).
    pub styles: Vec<u8>,
    pub texts: CustomizeTexts,
}

/// A row of the list: item id (0 = empty shortcut or the battle-style
/// row), on (MP Charge / Auto-Reload; an ability row is always on),
/// category 0 shortcut / 1 MP Charge / 2 Auto-Reload / 3 Battle Style /
/// 4 Abilities, the index inside its category and a party row's frequency
/// (work area entry +3). The battle-style row has no work-area entry in the
/// exe; here it is the party list's first row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    id: u16,
    on: bool,
    cat: usize,
    k: usize,
    freq: u8,
}

#[derive(Default)]
pub(super) struct Ui {
    pub(super) chars: MenuList,
    panel: Option<Id>,
    bars: Vec<Id>,
    texts: Vec<Id>,
    rights: Vec<Id>,
    entries: Vec<Entry>,
    pub(super) list: Grid,
    rows: Vec<Id>,
    labels: Vec<Id>,
    /// The frequency glyphs at the rows' right end (FUN_14035add0).
    freqs: Vec<Id>,
    headings: Vec<Id>,
    /// The list rows' NEW! badges (B+0xa960+0x1f8·i, prio P+3).
    badges: Vec<Id>,
    frame: Option<Id>,
    bubble: Option<Id>,
    /// The member an L2 / R2 switch goes to (DAT_140746588).
    switch_to: Option<usize>,
    // Popup.
    pub(super) popup: Grid,
    cat: usize,
    row: usize,
    candidates: Vec<u16>,
    popup_texts: Vec<Id>,
    /// 0x1c: the state that opened the box; the change waits for the
    /// popup to close (DAT_140bf0070 / 71).
    confirm_from: Option<State>,
    confirmed: bool,
}

impl Camp {
    /// The character whose settings show: the 0x19 cursor, later mem[0x19].
    fn customize_member(&self) -> usize {
        if self.state == State::CustomizeChars {
            self.custom_ui.chars.cursor
        } else {
            self.memory.customize_char
        }
    }

    fn customize_party(&self) -> bool {
        self.customize_member() != 0
    }

    /// A party member's frequency of `id` (record +0xf5 + FUN_1401dc060).
    fn member_frequency(&self, m: usize, id: u16) -> u8 {
        let k = player::party_members::frequency_index(id);
        self.data
            .abilities
            .members
            .get(m)
            .zip(k)
            .and_then(|(a, k)| a.frequency.get(k).copied())
            .unwrap_or(0)
    }

    fn set_member_frequency(&mut self, m: usize, id: u16, value: u8) {
        if let (Some(k), Some(a)) = (
            player::party_members::frequency_index(id),
            self.data.abilities.members.get_mut(m),
        ) && let Some(f) = a.frequency.get_mut(k)
        {
            *f = value;
        }
    }

    fn member_style(&self) -> u8 {
        let m = self.customize_member();
        self.data.customize.styles.get(m).copied().unwrap_or(0)
    }

    /// FUN_14035b440: the rows of the member's categories.
    fn customize_entries(&self) -> Vec<Entry> {
        let m = self.customize_member();
        if m != 0 {
            return self.party_entries(m);
        }
        let table = &self.data.items.table;
        let c = &self.data.customize;
        let mut out = Vec::new();
        for (k, &v) in c.shortcuts.iter().enumerate() {
            let id = if v == 0 {
                0
            } else {
                table
                    .iter()
                    .find(|r| (r.kind == 0 || r.kind == MAGIC_TYPE) && r.command == v)
                    .map_or(0, |r| r.id)
            };
            out.push(Entry {
                id,
                on: false,
                cat: 0,
                k,
                freq: 0,
            });
        }
        let mut k = 0;
        for (m, &spell) in SPELLS.iter().enumerate() {
            let owned = table.iter().any(|r| r.id == spell && r.kind == MAGIC_TYPE)
                && self.data.items.counts.get(&spell).copied().unwrap_or(0) != 0;
            if owned {
                out.push(Entry {
                    id: spell,
                    on: c.no_charge >> m & 1 == 0,
                    cat: 1,
                    k,
                    freq: 0,
                });
                k += 1;
            }
        }
        out.extend(self.reload_entries(0));
        // A shortcut shows the MP-charge / auto-reload state of its item.
        for i in 0..4 {
            if out[i].id == 0 {
                continue;
            }
            if let Some(later) = out[4..].iter().rev().find(|e| e.id == out[i].id) {
                out[i].on = later.on;
            }
        }
        out
    }

    /// Category 2: the distinct items of member `m`'s item slots (current,
    /// else the auto-reload original) with Auto-Reload on when the original
    /// is the item; a party member's carry the item's frequency.
    fn reload_entries(&self, m: usize) -> Vec<Entry> {
        let e = self
            .data
            .items
            .equipment
            .get(m)
            .cloned()
            .unwrap_or_default();
        let mut seen = Vec::new();
        let mut out = Vec::new();
        for (s, &cur) in e.items.iter().enumerate() {
            let orig = e.originals.get(s).copied().unwrap_or(0);
            let v = if cur != 0 { cur } else { orig };
            if v == 0 || seen.contains(&v) {
                continue;
            }
            seen.push(v);
            out.push(Entry {
                id: v,
                on: orig == v,
                cat: 2,
                k: seen.len() - 1,
                freq: if m != 0 {
                    self.member_frequency(m, v)
                } else {
                    0
                },
            });
        }
        out
    }

    /// FUN_14035b440 for a party member: the battle-style row, the equipped
    /// action abilities in item-table order (type 0x13 positions, exchange
    /// sort), the item rows.
    fn party_entries(&self, m: usize) -> Vec<Entry> {
        let table = &self.data.items.table;
        let mut out = vec![Entry {
            id: 0,
            on: false,
            cat: 3,
            k: 0,
            freq: 0,
        }];
        let slots = self
            .data
            .abilities
            .members
            .get(m)
            .map_or_else(Vec::new, |a| a.slots.clone());
        let mut abilities: Vec<u16> = slots
            .iter()
            .filter(|&&v| v & 0x8000 != 0 && v & 0x7fff != 0)
            .map(|&v| v & 0x7fff)
            .filter(|&id| self.info(id).param as u8 == 2)
            .collect();
        let key = |id: u16| {
            table
                .iter()
                .position(|r| r.id == id && r.kind == 0x13)
                .map_or(-1, |p| p as i64)
        };
        let n = abilities.len();
        for a in 0..n {
            for b in a + 1..n {
                if key(abilities[b]) < key(abilities[a]) {
                    abilities.swap(a, b);
                }
            }
        }
        for (k, id) in abilities.into_iter().enumerate() {
            out.push(Entry {
                id,
                on: true,
                cat: 4,
                k,
                freq: self.member_frequency(m, id),
            });
        }
        out.extend(self.reload_entries(m));
        out
    }

    /// The frequency glyph of a party row (B+0xdfe0: `09 icon`): abilities
    /// from table 4, items from the item table.
    fn freq_glyph(&self, e: &Entry) -> Vec<u8> {
        match e.cat {
            4 => frequency_glyph(e.freq),
            2 => ITEM_FREQUENCIES
                .iter()
                .find(|r| r.1 == e.freq)
                .map_or_else(Vec::new, |r| vec![0x09, r.0]),
            _ => Vec::new(),
        }
    }

    /// FUN_14035bd90: a row's text (`09 icon` + command or item name; the
    /// battle-style row the current style's label).
    fn customize_text(&self, e: &Entry, preview: bool) -> Vec<u8> {
        if e.cat == 3 {
            let s = usize::from(self.member_style()).min(5);
            return self.data.customize.texts.styles[s].clone();
        }
        let r = self.info(e.id);
        let mut out = Vec::new();
        if e.cat == 0 && preview {
            out.extend([0x09, SHORTCUT_GLYPHS[e.k]]);
        } else if e.id != 0 {
            let alt = if r.kind == MAGIC_TYPE { !e.on } else { e.on };
            out.extend([0x09, type_icon(r.kind, alt)]);
        }
        if e.id == 0 {
            out.extend(self.data.items.texts.blank.clone());
        } else if e.cat <= 1 && !r.command_name.is_empty() {
            out.extend(r.command_name);
        } else {
            out.extend(r.name);
        }
        out
    }

    // ---- 0x19 --------------------------------------------------------------

    /// FUN_14035d2a0 + FUN_140359de0: tab 0, the character rows and the
    /// preview panel (18 bar slots with texts and right-aligned glyphs).
    pub(super) fn enter_customize_chars(&mut self, seq: &Sequence) {
        self.state = State::CustomizeChars;
        self.substate = 0;
        self.create_tab(seq, 0);
        let names: Vec<Vec<u8>> = self
            .data
            .panels
            .members
            .iter()
            .map(|m| m.name.clone())
            .collect();
        let cursor = self.memory.customize_char.min(names.len().max(1) - 1);
        let pitch = seq.groups[TOP_ROWS.appear].text_position_x as f32;
        let mut rows = Vec::new();
        for (i, name) in names.into_iter().enumerate() {
            let queued = if i == cursor {
                TOP_ROWS.selected
            } else {
                TOP_ROWS.idle
            };
            let t = self.row_text(name);
            let r = self
                .sprites
                .create(seq, PRIO_ROW, ART, TOP_ROWS.appear, Some(queued), Some(t));
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            self.closers.push((r, TOP_ROWS.close));
            // Sora's row carries the NEW! badge while some magic is new
            // (camp-07 §3.1), at the row group's (100, 0).
            if i == 0
                && let Some(seen) = self.any_magic_badge()
            {
                let at = &seq.groups[TOP_ROWS.appear];
                let b = self.sprites.create(
                    seq,
                    PRIO_ROW + 1,
                    ART,
                    items::BADGE[seen as usize],
                    None,
                    None,
                );
                self.child(b, r);
                self.sprites.get_mut(b).offset =
                    [at.light_position_x as f32, at.text_position_y as f32];
            }
            rows.push(r);
        }
        self.custom_ui.chars = MenuList::new(rows, cursor);
        let panel = self.sprites.create(
            seq,
            PRIO_PANEL,
            ART,
            STATUS_PANEL[0],
            Some(STATUS_PANEL[1]),
            None,
        );
        self.closers.push((panel, STATUS_PANEL[2]));
        self.custom_ui.panel = Some(panel);
        let g = &seq.groups[150];
        let pitch = [g.text_position_x as f32, g.ui_padding as f32];
        let right_at = [g.light_position_x as f32, g.text_position_y as f32];
        self.custom_ui.bars.clear();
        self.custom_ui.texts.clear();
        self.custom_ui.rights.clear();
        for k in 0..18 {
            let l = self.sprites.create(seq, PRIO_BAR, ART, 150, None, None);
            self.sprites.set(seq, l, BLANK, None);
            self.child(l, panel);
            self.sprites.get_mut(l).offset =
                [pitch[0] * (k & 1) as f32, pitch[1] * (k >> 1) as f32];
            let v = self.sprites.create(
                seq,
                PRIO_TEXT,
                ART,
                WHITE,
                None,
                Some(self.row_text(Vec::new())),
            );
            self.child(v, l);
            let mut rt = self.row_text(Vec::new());
            rt.align = 1;
            let r = self
                .sprites
                .create(seq, PRIO_TEXT, ART, WHITE, None, Some(rt));
            self.child(r, l);
            self.sprites.get_mut(r).offset = right_at;
            self.custom_ui.bars.push(l);
            self.custom_ui.texts.push(v);
            self.custom_ui.rights.push(r);
        }
        self.customize_preview(seq);
    }

    /// FUN_14035e670: each category's rows, a new grid line per category.
    fn customize_preview(&mut self, seq: &Sequence) {
        self.custom_ui.entries = self.customize_entries();
        for k in 0..self.custom_ui.bars.len() {
            let (l, v, r) = (
                self.custom_ui.bars[k],
                self.custom_ui.texts[k],
                self.custom_ui.rights[k],
            );
            self.sprites.set(seq, l, BLANK, None);
            self.sprites.set(seq, v, WHITE, None);
            self.set_text(v, Vec::new());
            self.set_text(r, Vec::new());
        }
        let party = self.customize_party();
        let order = if party { PARTY_ORDER } else { SORA_ORDER };
        let mut slot = 0usize;
        for c in order {
            let entries: Vec<Entry> = self
                .custom_ui
                .entries
                .iter()
                .copied()
                .filter(|e| e.cat == c)
                .collect();
            for e in &entries {
                if slot >= self.custom_ui.bars.len() {
                    break;
                }
                let text = self.customize_text(e, true);
                self.sprites
                    .set(seq, self.custom_ui.bars[slot], PREVIEW[c], None);
                self.set_text(self.custom_ui.texts[slot], text);
                if party && c != 3 {
                    let g = self.freq_glyph(e);
                    self.set_text(self.custom_ui.rights[slot], g);
                }
                slot += 1;
            }
            if entries.len() % 2 == 1 {
                slot += 1;
            }
        }
    }

    fn close_customize_chars(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.custom_ui.panel, STATUS_PANEL[2]);
        for &r in &self.custom_ui.chars.rows.clone() {
            self.close_opt(seq, Some(r), TOP_ROWS.close);
        }
    }

    /// FUN_14035aa80.
    pub(super) fn customize_chars(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let prev = self.custom_ui.chars.cursor;
        let (pick, moved) = self.custom_ui.chars.input(pad);
        if self.substate == 2 {
            let p = self.pending;
            let busy = self.alive_opt(self.custom_ui.panel)
                || self.rows_alive(&self.custom_ui.chars.clone())
                || (p == Some(State::Top) && self.tab_alive(0));
            if !busy {
                match self.pending.take() {
                    Some(State::Top) => self.return_top(seq),
                    _ => self.enter_customize_list(seq),
                }
            } else if pad.trig != 0 {
                self.kill_opt(self.custom_ui.panel);
                for &r in &self.custom_ui.chars.rows {
                    self.sprites.kill(r);
                }
            }
            return false;
        }
        let row0 = self.group_of(self.custom_ui.chars.rows.first().copied());
        if (pick != Pick::None || moved) && row0 == Some(TOP_ROWS.appear) {
            for (i, &r) in self.custom_ui.chars.rows.clone().iter().enumerate() {
                let g = if i == prev {
                    TOP_ROWS.selected
                } else {
                    TOP_ROWS.idle
                };
                self.sprites.set(seq, r, g, None);
            }
        }
        let row0 = self.group_of(self.custom_ui.chars.rows.first().copied());
        if self.substate == 0
            && matches!(row0, Some(g) if g == TOP_ROWS.idle || g == TOP_ROWS.selected)
        {
            let h = self.data.customize.texts.help[usize::from(self.custom_ui.chars.cursor != 0)]
                .clone();
            self.show_help(seq, h);
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        // FUN_14035aff0 in 0x19: the preview and the help follow the cursor.
        if moved && self.substate == 1 {
            self.customize_preview(seq);
            let h = self.data.customize.texts.help[usize::from(self.custom_ui.chars.cursor != 0)]
                .clone();
            self.show_help(seq, h);
        }
        let cursor = self.custom_ui.chars.cursor;
        match pick {
            Pick::Start => self.exit = true,
            Pick::Cancel => {
                self.hide_help(seq);
                self.close_tab(seq, 0);
                self.close_customize_chars(seq);
                self.pending = Some(State::Top);
                self.substate = 2;
            }
            Pick::Row(r) => {
                let name = self.data.panels.members[r].name.clone();
                self.pending_tab_text[1] = Some(name);
                self.hide_help(seq);
                self.close_customize_chars(seq);
                self.pending = Some(State::CustomizeList);
                self.substate = 2;
            }
            _ => {}
        }
        if matches!(pick, Pick::Start | Pick::Cancel | Pick::Row(_)) {
            if cursor != self.memory.customize_char {
                self.memory.customize_list = (0, 0);
            }
            self.memory.customize_char = cursor;
        }
        moved
    }

    // ---- 0x1a --------------------------------------------------------------

    /// FUN_14035ccb0: tab 1, the panel, 9 rows with labels and headings,
    /// the scrollbar and the Switch banner.
    fn enter_customize_list(&mut self, seq: &Sequence) {
        self.state = State::CustomizeList;
        self.substate = 0;
        self.create_tab(seq, 0);
        self.create_tab(seq, 1);
        self.custom_ui.entries = self.customize_entries();
        let n = self.custom_ui.entries.len();
        let panel = self
            .sprites
            .create(seq, P, ART, LIST_PANEL[0], Some(LIST_PANEL[1]), None);
        self.closers.push((panel, LIST_PANEL[2]));
        self.custom_ui.panel = Some(panel);
        let pitch = seq.groups[ROW[0]].ui_padding as f32;
        let heading_pitch = seq.groups[HEADING[0]].ui_padding as f32;
        let heading_size = seq.groups[HEADING[0]].text_scale as f32;
        let rg = &seq.groups[ROW[0]];
        let right_at = [rg.light_position_x as f32, rg.text_position_y as f32];
        self.custom_ui.rows.clear();
        self.custom_ui.labels.clear();
        self.custom_ui.freqs.clear();
        self.custom_ui.headings.clear();
        self.custom_ui.badges.clear();
        for i in 0..VISIBLE {
            let r = self.sprites.create(seq, P, ART, BLANK, None, None);
            self.child(r, panel);
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            let t = self.sprites.create(
                seq,
                P + 2,
                ART,
                WHITE,
                None,
                Some(self.row_text(Vec::new())),
            );
            self.child(t, r);
            let mut ft = self.row_text(Vec::new());
            ft.align = 1;
            let f = self.sprites.create(seq, P + 2, ART, WHITE, None, Some(ft));
            self.child(f, r);
            self.sprites.get_mut(f).offset = right_at;
            let b = self.sprites.create(seq, P + 3, ART, BLANK, None, None);
            self.child(b, r);
            self.sprites.get_mut(b).offset = right_at;
            self.custom_ui.rows.push(r);
            self.custom_ui.labels.push(t);
            self.custom_ui.freqs.push(f);
            self.custom_ui.badges.push(b);
        }
        for k in 0..VISIBLE {
            let mut t = Text::new(Vec::new(), heading_size);
            t.align = 2;
            t.keep_rgb = false;
            let h = self.sprites.create(seq, P + 1, ART, BLANK, None, Some(t));
            self.child(h, panel);
            self.sprites.get_mut(h).offset = [0., heading_pitch * k as f32];
            self.custom_ui.headings.push(h);
        }
        self.build_scrollbar(seq, panel, P + 1, n, n.min(VISIBLE));
        // §9: the remembered cursor, fixed up against the list size.
        let (mut cur, mut s) = self.memory.customize_list;
        let abs = cur + s;
        if n <= abs || n < VISIBLE + s {
            while s > 0 && n < VISIBLE + s {
                s -= 1;
            }
            cur = abs.saturating_sub(s).min(VISIBLE - 1);
            if n <= s + cur {
                cur = 0;
                s = 0;
            }
        }
        let mut list = MenuList::new(self.custom_ui.rows.clone(), cur);
        list.total = n;
        list.page = Some(VISIBLE);
        self.custom_ui.list = Grid {
            list,
            scroll: s,
            cols: 1,
            lines: VISIBLE,
        };
        self.customize_mask();
        self.see_cursor_magic();
        let t = self.data.abilities.texts.switch.clone();
        self.show_banner(seq, t);
        self.customize_skin(seq);
    }

    /// FUN_14035e490 / FUN_14036a220: resting on a shortcut or MP Charge
    /// row sees its new magic (on entry and every cursor change).
    fn see_cursor_magic(&mut self) {
        if let Some(e) = self.customize_cursor_entry()
            && e.cat < 2
            && self.magic_badge(e.id).is_some()
            && !self.data.items.seen.contains(&e.id)
        {
            self.data.items.seen.push(e.id);
        }
    }

    fn customize_cursor_entry(&self) -> Option<Entry> {
        self.custom_ui
            .entries
            .get(self.custom_ui.list.list.cursor + self.custom_ui.list.scroll)
            .copied()
    }

    /// The notice category of the row under the cursor, when one has its
    /// flag.
    fn notice_for_entry(&self) -> Option<u8> {
        let c = match self.customize_cursor_entry()?.cat {
            1 => 1,
            2 => 0,
            _ => return None,
        };
        self.notice_available(c).then_some(c)
    }

    /// §4.1 / §4.5: page keys, L2/R2, △ on a non-shortcut row.
    fn customize_mask(&mut self) {
        let mut mask = 0x73 | 0x3000 | 0xc000;
        if self.customize_cursor_entry().is_some_and(|e| e.cat != 0) {
            mask |= 0x200;
        }
        // SELECT: MP Charge rows show the MP Charge notice (1), Auto-Reload
        // rows the Auto-Reload one (0) [I].
        if self.notice_for_entry().is_some() {
            mask |= 0x400;
        }
        self.custom_ui.list.list.mask = mask;
    }

    /// FUN_14035d6a0: headings at each category's first row (each shortcut
    /// row has its own), row groups, labels.
    fn customize_skin(&mut self, seq: &Sequence) {
        for k in 0..VISIBLE {
            for id in [self.custom_ui.rows[k], self.custom_ui.headings[k]] {
                self.sprites.set(seq, id, BLANK, None);
            }
            self.set_text(self.custom_ui.headings[k], Vec::new());
            let t = self.custom_ui.labels[k];
            self.sprites.set(seq, t, WHITE, None);
            self.set_text(t, Vec::new());
            if let Some(&b) = self.custom_ui.badges.get(k) {
                self.sprites.set(seq, b, BLANK, None);
            }
        }
        let n = self.custom_ui.entries.len();
        let s = self.custom_ui.list.scroll;
        self.scroll_to(seq, s, n, n.min(VISIBLE));
        let entries = self.custom_ui.entries.clone();
        // FUN_14035add0 (0x1b): the glyph of visible row v is the buffer of
        // row v, not of row s + v — the exe ignores the scroll here.
        let party = self.customize_party();
        for v in 0..VISIBLE {
            let g = entries
                .get(v)
                .filter(|e| party && e.cat != 3 && v < n)
                .map(|e| self.freq_glyph(e))
                .unwrap_or_default();
            self.set_text(self.custom_ui.freqs[v], g);
        }
        for v in 0..VISIBLE {
            let Some(e) = entries.get(s + v) else { break };
            if e.k == 0 || e.cat == 0 {
                let h = self.custom_ui.headings[v];
                self.sprites.set(seq, h, HEADING[e.cat], None);
                let text = if e.cat == 0 {
                    self.data.customize.texts.shortcut_headings[e.k].clone()
                } else {
                    self.data.customize.texts.labels[e.cat].clone()
                };
                self.set_text(h, text);
            }
            self.sprites
                .set(seq, self.custom_ui.rows[v], ROW[e.cat], None);
            let text = self.customize_text(e, false);
            self.set_text(self.custom_ui.labels[v], text);
            // The row's new magic (camp-07 §4.2).
            if let (Some(&b), Some(seen)) = (self.custom_ui.badges.get(v), self.magic_badge(e.id)) {
                self.sprites.set(seq, b, items::BADGE[seen as usize], None);
            }
        }
    }

    /// FUN_14035dac0 in 0x1a / 0x1b: the battle style's help, or the row
    /// item's description and, for a party member, its frequency's help.
    fn customize_list_help(&mut self, seq: &Sequence) {
        let t = &self.data.customize.texts;
        let h = match self.customize_cursor_entry() {
            Some(e) if e.cat == 3 => t.style_help[usize::from(self.member_style()).min(5)].clone(),
            Some(e) if e.id != 0 => {
                let mut h = self.info(e.id).description;
                if self.customize_party() {
                    h.extend(self.freq_help(&e));
                }
                h
            }
            _ => Vec::new(),
        };
        self.show_help(seq, h);
    }

    fn freq_help(&self, e: &Entry) -> Vec<u8> {
        let t = &self.data.customize.texts;
        match e.cat {
            4 => frequency_record(e.freq)
                .map_or_else(Vec::new, |k| t.ability_frequency_help[k].clone()),
            2 => ITEM_FREQUENCIES
                .iter()
                .position(|r| r.1 == e.freq)
                .map_or_else(Vec::new, |k| t.item_frequency_help[k].clone()),
            _ => Vec::new(),
        }
    }

    /// FUN_14035e930: the cursor frame on the hand's row and, on a row △
    /// can change, the bubble on the hand.
    fn customize_frame(&mut self, seq: &Sequence, group: usize, bubble: bool) {
        let glove = self.glove.filter(|&g| self.sprites.alive(g));
        let Some(glove) = glove else {
            let f = self.custom_ui.frame.take();
            self.kill_opt(f);
            let b = self.custom_ui.bubble.take();
            self.kill_opt(b);
            return;
        };
        let offset = self
            .sprites
            .get(glove)
            .parent
            .map_or([0.; 2], |r| self.sprites.get(r).offset);
        let f = match self.custom_ui.frame {
            Some(f) if self.sprites.alive(f) => f,
            _ => {
                let f = self.sprites.create(seq, P + 2, ART, group, None, None);
                self.child(f, self.custom_ui.panel.unwrap());
                self.custom_ui.frame = Some(f);
                f
            }
        };
        self.sprites.get_mut(f).offset = offset;
        if bubble {
            if !self.alive_opt(self.custom_ui.bubble) {
                let mut t = Text::new(self.data.abilities.texts.triangle.clone(), TEXT_SIZE);
                t.align = 2;
                let b = self
                    .sprites
                    .create(seq, PRIO_LIGHT, ART, BUBBLE, None, Some(t));
                self.custom_ui.bubble = Some(b);
            }
            let b = self.custom_ui.bubble.unwrap();
            self.sprites.get_mut(b).parent = Some(glove);
        } else {
            let b = self.custom_ui.bubble.take();
            self.kill_opt(b);
        }
    }

    fn close_customize_list(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.custom_ui.panel, LIST_PANEL[2]);
        self.close_scrollbar(seq);
        let f = self.custom_ui.frame.take();
        self.kill_opt(f);
        let b = self.custom_ui.bubble.take();
        self.kill_opt(b);
    }

    /// FUN_1403597e0: MP Charge on / off for a spell entry.
    fn toggle_charge(&mut self, id: u16) {
        if let Some(m) = SPELLS.iter().position(|&s| s == id) {
            self.data.customize.no_charge ^= 1 << m;
        }
    }

    /// FUN_14035ebe0: turning `id`'s Auto-Reload off would empty a used-up
    /// slot (orig == id, cur ≠ id).
    fn reload_off_empties(&self, id: u16) -> bool {
        let e = self.data.items.equipment.get(self.customize_member());
        e.is_some_and(|e| {
            e.items
                .iter()
                .zip(&e.originals)
                .any(|(&cur, &orig)| orig == id && cur != id)
        })
    }

    /// FUN_1403598e0: Auto-Reload of row `row`'s item. Sora toggles it; a
    /// party member takes the popup's choice or (△ / the warning box) the
    /// next of OFF+Often, ON+Often, OFF+Last hope!, ON+Last hope!. Every
    /// slot holding the item follows, the frequency is written and the slots
    /// are re-sorted (FUN_140357f40).
    fn apply_reload(&mut self, row: usize, choice: Option<usize>) {
        let Some(e) = self.custom_ui.entries.get(row).copied() else {
            return;
        };
        let m = self.customize_member();
        let (on, freq) = reload_value(m != 0, e.on, e.freq, choice);
        if let Some(q) = self.data.items.equipment.get_mut(m) {
            q.originals.resize(q.items.len(), 0);
            for s in 0..q.items.len() {
                if q.items[s] == e.id || q.originals[s] == e.id {
                    q.originals[s] = if on { e.id } else { 0 };
                }
            }
        }
        if m != 0 {
            self.set_member_frequency(m, e.id, freq);
        }
        self.sort_item_slots(m);
    }

    /// After a change in the list (§6.3): the rows rebuilt and, when their
    /// number changed, the cursor and scroll kept on the list.
    fn customize_rebuild(&mut self, seq: &Sequence) {
        let old = self.custom_ui.entries.len();
        self.custom_ui.entries = self.customize_entries();
        let n = self.custom_ui.entries.len();
        if n != old {
            let vis = self.custom_ui.rows.len();
            let (cur, s) = fix_cursor(
                self.custom_ui.list.list.cursor,
                self.custom_ui.list.scroll,
                vis,
                n,
            );
            self.custom_ui.list.list.cursor = cur;
            self.custom_ui.list.list.total = n;
            self.custom_ui.list.scroll = s;
            self.scroll_total(seq, s, n, n.min(VISIBLE));
        }
        self.customize_skin(seq);
    }

    /// The battle style (FUN_140359760): rec+0xf4.
    fn set_style(&mut self, value: u8) {
        let m = self.customize_member();
        if let Some(s) = self.data.customize.styles.get_mut(m) {
            *s = value;
        }
    }

    /// FUN_14035a4e0 (Sora).
    pub(super) fn customize_list(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let scroll = self.custom_ui.list.scroll;
        let (pick, moved) = self.custom_ui.list.input(pad);
        if self.substate == 2 {
            let p = self.pending;
            let list = matches!(p, Some(State::CustomizeChars) | Some(State::CustomizeList));
            let busy = self.alive_opt(self.custom_ui.panel)
                || self.item_ui.scroll.iter().any(|&s| self.sprites.alive(s))
                || (list && self.tab_alive(1))
                || (!list && self.alive_opt(self.banner));
            if !busy {
                match self.pending.take() {
                    Some(State::CustomizeChars) => {
                        self.kill_tab(1);
                        self.kill_opt(self.banner);
                        self.enter_customize_chars(seq);
                    }
                    // The member switch: mem[0x19] = the new member.
                    Some(State::CustomizeList) => {
                        if let Some(m) = self.custom_ui.switch_to.take() {
                            self.memory.customize_char = m;
                        }
                        self.kill_opt(self.banner);
                        self.enter_customize_list(seq);
                    }
                    _ => self.enter_customize_popup(seq),
                }
            } else if pad.trig != 0 {
                self.kill_opt(self.custom_ui.panel);
                for &s in &self.item_ui.scroll {
                    self.sprites.kill(s);
                }
            }
            return false;
        }
        if (pick != Pick::None || moved)
            && self.group_of(self.custom_ui.panel) != Some(LIST_PANEL[1])
        {
            for (id, g) in [
                (self.custom_ui.panel, LIST_PANEL[1]),
                (self.banner, BANNER[1]),
            ] {
                if let Some(i) = id
                    && self.sprites.alive(i)
                {
                    self.sprites.set(seq, i, g, None);
                }
            }
        }
        if self.substate == 0 && self.group_of(self.custom_ui.panel) == Some(LIST_PANEL[1]) {
            self.customize_list_help(seq);
            self.substate = 1;
        }
        // FUN_14035c180: L2/R2 need another member; Cure's MP Charge is
        // fixed.
        let entry = self.customize_cursor_entry();
        let cure = !self.customize_party() && entry.is_some_and(|e| e.cat == 1 && e.id == CURE);
        let valid = match pick {
            Pick::Next | Pick::Previous => self.data.panels.members.len() > 1,
            Pick::Row(_) | Pick::Triangle => !cure,
            _ => true,
        };
        if let Some(s) = pick_sound(pick, moved, valid) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            self.see_cursor_magic();
            self.customize_skin(seq);
            let _ = scroll;
            self.customize_mask();
            self.customize_list_help(seq);
        }
        let cursor = (self.custom_ui.list.list.cursor, self.custom_ui.list.scroll);
        if valid {
            match pick {
                Pick::Triangle => {
                    if let Some(e) = entry {
                        let row = cursor.0 + cursor.1;
                        match e.cat {
                            1 => self.toggle_charge(e.id),
                            2 if e.on && self.reload_off_empties(e.id) => {
                                self.open_reload_warning(seq, State::CustomizeList, e.id);
                            }
                            2 => self.apply_reload(row, None),
                            // FUN_140359760(-1): the next style, 5 → 0.
                            3 => self.set_style((self.member_style() + 1) % STYLES),
                            // FUN_140359660(-1): the next record, Rare → Free.
                            4 => {
                                let k = frequency_record(e.freq).map_or(0, |k| (k + 1) % 3);
                                let m = self.customize_member();
                                self.set_member_frequency(m, e.id, FREQUENCIES[k].1);
                            }
                            _ => {}
                        }
                        self.customize_rebuild(seq);
                        self.customize_list_help(seq);
                    }
                }
                // R2 / L2: the next / previous member (DAT_140746588).
                Pick::Next | Pick::Previous => {
                    let n = self.data.panels.members.len();
                    let m = self.memory.customize_char;
                    let next = if pick == Pick::Next {
                        (m + 1) % n
                    } else {
                        (m + n - 1) % n
                    };
                    self.custom_ui.switch_to = Some(next);
                    self.memory.customize_list = (0, 0);
                    self.pending_tab_text[1] = Some(self.data.panels.members[next].name.clone());
                    self.hide_help(seq);
                    self.close_banner(seq);
                    self.close_tab(seq, 1);
                    self.close_customize_list(seq);
                    self.pending = Some(State::CustomizeList);
                    self.substate = 2;
                }
                Pick::Select => {
                    self.memory.customize_list = cursor;
                    if let Some(c) = self.notice_for_entry() {
                        self.start_notices(seq, Some(c), State::CustomizeList);
                    }
                }
                Pick::Start => {
                    self.memory.customize_list = cursor;
                    self.exit = true;
                }
                Pick::Cancel => {
                    self.memory.customize_list = cursor;
                    self.hide_help(seq);
                    self.close_banner(seq);
                    self.close_tab(seq, 1);
                    self.close_customize_list(seq);
                    self.pending = Some(State::CustomizeChars);
                    self.substate = 2;
                }
                Pick::Row(r) => {
                    self.memory.customize_list = cursor;
                    let e = self.custom_ui.entries[r];
                    let label = if e.cat == 0 {
                        self.data.customize.texts.shortcut_headings[e.k].clone()
                    } else {
                        self.data.customize.texts.labels[e.cat].clone()
                    };
                    self.pending_tab_text[2] = Some(label);
                    self.custom_ui.row = r;
                    self.close_banner(seq);
                    self.hide_help(seq);
                    self.close_customize_list(seq);
                    self.pending = Some(State::CustomizePopup(e.cat as u8));
                    self.substate = 2;
                }
                _ => {}
            }
        }
        if self.state == State::CustomizeList {
            let bubble = self.customize_cursor_entry().is_some_and(|e| e.cat != 0);
            self.customize_frame(seq, LIST_FRAME, bubble);
        }
        moved
    }

    // ---- 0x1d–0x1f -----------------------------------------------------------

    /// FUN_14034c770(filter 10) + FUN_14034caa0: spells, then consumables,
    /// owned, in item-table order.
    fn shortcut_candidates(&self) -> Vec<u16> {
        let owned = |id: u16| self.data.items.counts.get(&id).copied().unwrap_or(0) != 0;
        let table = &self.data.items.table;
        table
            .iter()
            .filter(|r| r.kind == MAGIC_TYPE && owned(r.id))
            .chain(table.iter().filter(|r| r.kind == 0 && owned(r.id)))
            .map(|r| r.id)
            .collect()
    }

    /// FUN_14035c750: tab 2, the popup panel, its header with the row's
    /// text, the choice rows, the scrollbar; the cursor starts on the
    /// current value.
    fn enter_customize_popup(&mut self, seq: &Sequence) {
        let e = self.custom_ui.entries[self.custom_ui.row];
        let c = e.cat;
        self.state = State::CustomizePopup(c as u8);
        self.substate = 0;
        self.custom_ui.cat = c;
        self.create_tab(seq, 2);
        self.custom_ui.candidates = if c == 0 {
            self.shortcut_candidates()
        } else {
            Vec::new()
        };
        let party = self.customize_party();
        let n = match c {
            0 => self.custom_ui.candidates.len() + 1,
            2 if party => 4,
            3 => usize::from(STYLES),
            4 => FREQUENCIES.len(),
            _ => 2,
        };
        let panel = self.sprites.create(
            seq,
            P,
            ART,
            POPUP_PANEL[c][0],
            Some(POPUP_PANEL[c][2]),
            None,
        );
        self.closers.push((panel, POPUP_PANEL[c][1]));
        self.custom_ui.panel = Some(panel);
        let header = self
            .sprites
            .create(seq, P + 1, ART, POPUP_HEADER[c], None, None);
        self.child(header, panel);
        let text = self.customize_text(&e, false);
        let h = self
            .sprites
            .create(seq, P + 2, ART, WHITE, None, Some(self.row_text(text)));
        self.child(h, header);
        let k = n.min(POPUP_VISIBLE);
        let rg = &seq.groups[POPUP_ROW[c]];
        let pitch = rg.ui_padding as f32;
        let freq_at = [rg.text_position_x as f32, 0.];
        let mut rows = Vec::new();
        self.custom_ui.popup_texts.clear();
        for i in 0..k {
            let r = self.sprites.create(seq, P, ART, POPUP_ROW[c], None, None);
            self.child(r, panel);
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            let t = self.sprites.create(
                seq,
                P + 2,
                ART,
                WHITE,
                None,
                Some(self.row_text(Vec::new())),
            );
            self.child(t, r);
            // FUN_14035af10 (0x1f, party): the item frequency of rows 2i,
            // 2i + 1.
            if c == 2 && party {
                let text = self.data.customize.texts.item_frequency[(i / 2).min(1)].clone();
                let f =
                    self.sprites
                        .create(seq, P + 2, ART, YELLOW, None, Some(self.row_text(text)));
                self.child(f, r);
                self.sprites.get_mut(f).offset = freq_at;
            }
            rows.push(r);
            self.custom_ui.popup_texts.push(t);
        }
        self.build_scrollbar(seq, panel, P + 1, n, k);
        // FUN_14035c2a0.
        let value = match c {
            0 => self
                .custom_ui
                .candidates
                .iter()
                .position(|&id| id == e.id && e.id != 0)
                .map_or(0, |p| p + 1),
            2 if party => usize::from(e.on) + if e.freq == 2 { 2 } else { 0 },
            3 => usize::from(self.member_style()),
            4 => frequency_record(e.freq).unwrap_or(0),
            _ => usize::from(e.on),
        };
        let (v, s) = popup_start(value, n);
        let mut list = MenuList::new(rows, v);
        list.total = n;
        list.page = Some(k);
        self.custom_ui.popup = Grid {
            list,
            scroll: s,
            cols: 1,
            lines: k,
        };
        self.customize_popup_texts(seq);
    }

    /// FUN_14035dde0: the visible choices.
    fn customize_popup_texts(&mut self, seq: &Sequence) {
        let c = self.custom_ui.cat;
        let s = self.custom_ui.popup.scroll;
        let n = self.custom_ui.popup.list.total;
        let k = self.custom_ui.popup_texts.len();
        self.scroll_to(seq, s, n, k);
        let t = self.data.customize.texts.clone();
        let choice = |i: usize| -> Vec<u8> {
            match c {
                1 => t.mp_choices[i.min(1)].clone(),
                3 => t.styles[i.min(5)].clone(),
                4 => t.ability_frequency[i.min(2)].clone(),
                _ => t.reload_choices[i & 1].clone(),
            }
        };
        for i in 0..k {
            let a = s + i;
            let text = if c == 0 {
                if a == 0 {
                    self.data.items.texts.blank.clone()
                } else {
                    let id = self.custom_ui.candidates[a - 1];
                    self.customize_text(
                        &Entry {
                            id,
                            on: false,
                            cat: 0,
                            k: 0,
                            freq: 0,
                        },
                        false,
                    )
                }
            } else {
                choice(a)
            };
            self.set_text(self.custom_ui.popup_texts[i], text);
        }
    }

    fn customize_popup_help(&mut self, seq: &Sequence) {
        let a = self.custom_ui.popup.list.cursor + self.custom_ui.popup.scroll;
        let t = &self.data.customize.texts;
        let party = self.customize_party();
        let h = match self.custom_ui.cat {
            0 if a == 0 => t.remove_help.clone(),
            0 => self.info(self.custom_ui.candidates[a - 1]).description,
            1 => t.mp_help[a.min(1)].clone(),
            3 => t.style_help[a.min(5)].clone(),
            4 => t.ability_frequency_help[a.min(2)].clone(),
            _ if party => {
                let mut h = t.reload_help[a & 1].clone();
                h.extend(t.item_frequency_help[(a / 2).min(1)].clone());
                h
            }
            _ => t.reload_help[a.min(1)].clone(),
        };
        self.show_help(seq, h);
    }

    /// FUN_14035c710 + FUN_1402f6520(0x1c): the warning box.
    fn open_reload_warning(&mut self, seq: &Sequence, from: State, id: u16) {
        self.custom_ui.confirm_from = Some(from);
        self.custom_ui.confirmed = false;
        self.custom_ui.row = self
            .custom_ui
            .entries
            .iter()
            .position(|e| e.cat == 2 && e.id == id)
            .unwrap_or(self.custom_ui.row);
        let text = self.data.customize.texts.reload_warning.clone();
        let (no, yes) = (
            self.data.items.texts.no.clone(),
            self.data.items.texts.yes.clone(),
        );
        self.dim_glove(seq);
        self.open_dialog(seq, text, vec![no, yes]);
        self.state = State::CustomizeConfirm;
        self.substate = 0;
    }

    /// FUN_14035a080.
    pub(super) fn customize_confirm(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let (pick, moved) = self.item_ui.choices.input(pad);
        if self.substate == 2 {
            if !self.dialog_alive() {
                self.undim_last();
                self.state = self
                    .custom_ui
                    .confirm_from
                    .take()
                    .unwrap_or(State::CustomizeList);
                self.substate = 1;
                self.pending = None;
            }
            return false;
        }
        if self.substate == 0 && self.group_of(self.item_ui.dialog_box()) == Some(7) {
            self.substate = 1;
        }
        if self.substate != 1 {
            return false;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if matches!(pick, Pick::Row(_) | Pick::Cancel) {
            if pick == Pick::Row(1) {
                if self.custom_ui.confirm_from == Some(State::CustomizeList) {
                    let row = self.custom_ui.row;
                    self.apply_reload(row, None);
                    self.customize_rebuild(seq);
                }
                self.custom_ui.confirmed = true;
            }
            self.close_dialog(seq);
            self.substate = 2;
        }
        moved
    }

    /// FUN_14035a180.
    pub(super) fn customize_popup(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let c = self.custom_ui.cat;
        let scroll = self.custom_ui.popup.scroll;
        let (pick, moved) = self.custom_ui.popup.input(pad);
        if self.substate == 2 {
            let busy = self.alive_opt(self.custom_ui.panel)
                || self.item_ui.scroll.iter().any(|&s| self.sprites.alive(s))
                || self.tab_alive(2);
            if !busy {
                self.pending = None;
                self.kill_tab(2);
                self.enter_customize_list(seq);
            } else if pad.trig != 0 {
                self.kill_opt(self.custom_ui.panel);
                for &s in &self.item_ui.scroll {
                    self.sprites.kill(s);
                }
            }
            return false;
        }
        if (pick != Pick::None || moved)
            && self.group_of(self.custom_ui.panel) != Some(POPUP_PANEL[c][2])
            && let Some(p) = self.custom_ui.panel
        {
            self.sprites.set(seq, p, POPUP_PANEL[c][2], None);
        }
        if self.substate == 0 && self.group_of(self.custom_ui.panel) == Some(POPUP_PANEL[c][2]) {
            self.customize_popup_help(seq);
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            if self.custom_ui.popup.scroll != scroll {
                self.customize_popup_texts(seq);
            }
            self.customize_popup_help(seq);
        }
        let mut close = self.custom_ui.confirmed;
        if !close {
            match pick {
                Pick::Start => self.exit = true,
                Pick::Cancel => close = true,
                Pick::Row(r) => {
                    let e = self.custom_ui.entries[self.custom_ui.row];
                    close = true;
                    match c {
                        0 => {
                            self.data.customize.shortcuts[e.k] = if r == 0 {
                                0
                            } else {
                                self.info(self.custom_ui.candidates[r - 1]).command
                            };
                        }
                        1 if e.on != (r != 0) => self.toggle_charge(e.id),
                        2 => {
                            let party = self.customize_party();
                            let cur = usize::from(e.on) + if party && e.freq == 2 { 2 } else { 0 };
                            if cur != r {
                                if r % 2 == 0 && self.reload_off_empties(e.id) {
                                    self.open_reload_warning(seq, State::CustomizePopup(2), e.id);
                                    close = false;
                                } else {
                                    self.apply_reload(self.custom_ui.row, Some(r));
                                }
                            }
                        }
                        3 => self.set_style(r as u8),
                        4 => {
                            let m = self.customize_member();
                            self.set_member_frequency(m, e.id, FREQUENCIES[r.min(2)].1);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        } else {
            // Back from the warning with "Yes": apply on the popup's exit
            // with the popup's choice.
            let r = self.custom_ui.popup.list.cursor + self.custom_ui.popup.scroll;
            self.apply_reload(self.custom_ui.row, Some(r));
            self.custom_ui.confirmed = false;
        }
        if close {
            self.hide_help(seq);
            self.close_tab(seq, 2);
            self.close_opt(seq, self.custom_ui.panel, POPUP_PANEL[c][1]);
            self.close_scrollbar(seq);
            let f = self.custom_ui.frame.take();
            self.kill_opt(f);
            self.pending = Some(State::CustomizeList);
            self.substate = 2;
        }
        if matches!(self.state, State::CustomizePopup(_)) && self.substate != 2 {
            self.customize_frame(seq, POPUP_FRAME[c], false);
        }
        moved
    }
}

/// FUN_1403598e0's new Auto-Reload and frequency: Sora toggles (or takes
/// the popup's row); a party member takes the popup's row v or the next of
/// ((freq ≠ 0) · 2 | on), 3 → 0, as on = v & 1, frequency v & 2.
fn reload_value(party: bool, on: bool, freq: u8, choice: Option<usize>) -> (bool, u8) {
    if party {
        let v = choice.map_or_else(
            || (((u8::from(freq != 0) * 2) | u8::from(on)) + 1) % 4,
            |r| r as u8,
        );
        (v & 1 != 0, v & 2)
    } else {
        (choice.map_or(!on, |r| r != 0), 0)
    }
}

/// §6.3: the cursor and scroll after the row count became `n` (visible
/// rows `vis`).
fn fix_cursor(cur: usize, s: usize, vis: usize, n: usize) -> (usize, usize) {
    let (mut cur, mut s) = (cur, s);
    if cur == 0 {
        s = s.saturating_sub(1);
    } else if s == 0 || vis + s <= n {
        if n <= cur {
            cur -= 1;
        }
    } else {
        s -= 1;
    }
    (cur, s)
}

/// FUN_14035c2a0: (cursor, scroll) for value index `value` of `n` choices
/// with 8 visible rows.
fn popup_start(value: usize, n: usize) -> (usize, usize) {
    let (mut v, mut s) = (value, 0);
    if v > 7 {
        s = v - 7;
        v = 7;
    }
    if s + v >= n {
        (v, s) = (n - 1, (v + 1).saturating_sub(n));
    }
    (v, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popups_open_on_the_current_value() {
        assert_eq!(popup_start(0, 4), (0, 0));
        assert_eq!(popup_start(3, 4), (3, 0));
        // Value 10 of 12: scrolled so it sits on the last visible row.
        assert_eq!(popup_start(10, 12), (7, 3));
    }

    #[test]
    fn a_party_items_triangle_cycles_reload_and_frequency() {
        // OFF+Often → ON+Often → OFF+Last hope! → ON+Last hope! → OFF+Often.
        assert_eq!(reload_value(true, false, 0, None), (true, 0));
        assert_eq!(reload_value(true, true, 0, None), (false, 2));
        assert_eq!(reload_value(true, false, 2, None), (true, 2));
        assert_eq!(reload_value(true, true, 2, None), (false, 0));
        // The popup's row 3 is ON+Last hope!; Sora only toggles.
        assert_eq!(reload_value(true, false, 0, Some(3)), (true, 2));
        assert_eq!(reload_value(false, true, 0, None), (false, 0));
        assert_eq!(reload_value(false, false, 0, Some(1)), (true, 0));
    }

    #[test]
    fn a_shorter_list_keeps_the_cursor_on_it() {
        // Last row of five removed: the cursor steps up.
        assert_eq!(fix_cursor(4, 0, 9, 4), (3, 0));
        // Scrolled with nine visible: the scroll gives way.
        assert_eq!(fix_cursor(5, 2, 9, 10), (5, 1));
        assert_eq!(fix_cursor(0, 2, 9, 10), (0, 1));
    }
}
