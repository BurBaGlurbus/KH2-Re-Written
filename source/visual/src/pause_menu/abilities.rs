//! The camp's Abilities screens (camp-06 spec): the character list with
//! the preview of equipped abilities (state 0xf: FUN_140362ca0 /
//! FUN_14035fd30 / FUN_1403642a0), the ability list with its category tabs,
//! AP costs, scrollbar and AP gauge (0x10: FUN_140360920 / FUN_1403623b0 /
//! FUN_140363db0 / FUN_140363510 / FUN_14035f810), and the Equip / Don't
//! Equip popup (0x11–0x13: FUN_140361a00 / FUN_14035f610). The work area
//! is built from the member's 80 ability slots (bit 15 = equipped) and its
//! weapon's ability; a party member's action abilities carry their use
//! frequency (icons, the △ cycle and the four-row popup), and L2 / R2 step
//! through the members (FUN_14035f130). Drive forms, Lion and the NEW!
//! marks' producer are not built.
use super::items::{Grid, ItemInfo};
use super::*;

const PRIO_ROW: u32 = 0x7541;
const PRIO_PANEL: u32 = 0x753d;
const PRIO_BAR: u32 = 0x753e;
const PRIO_TEXT: u32 = 0x753f;
const PRIO_LIST: u32 = 0x753c;
const PRIO_LIST_BADGE: u32 = 0x753e;
const PRIO_FRAME: u32 = 0x753e;
const PRIO_GAUGE: u32 = 0x753c;
const PRIO_POPUP_TEXT: u32 = 0x753e;
const PRIO_HEADER: u32 = 0x753d;

/// Preview bar groups per category (0x1405bc2e0): Action, Growth,
/// Support, Weapons.
const PREVIEW: [usize; 4] = [137, 139, 143, 150];
const LIST_PANEL: [usize; 3] = [230, 231, 232];
/// Row bar (0x1405bc2e8) and category tab (0x1405bc2f0) groups.
const LIST_ROW: [usize; 4] = [248, 252, 262, 265];
const LIST_TAB: [usize; 4] = [276, 278, 280, 283];
const LIST_EMPTY: usize = 236;
const LIST_CURSOR: usize = 249;
const AP_CELL: usize = 251;
const BUBBLE: usize = 24;
const GAUGE: [usize; 3] = [272, 273, 274];
const GAIN: usize = 269;
const NOW: usize = 270;
const LOSS: usize = 271;
/// Popup per category (0x1405bc2c8..): panel appear / close / idle,
/// header, rows, cursor frame.
const POPUP_PANEL: [[usize; 3]; 3] = [[153, 155, 154], [171, 173, 172], [180, 182, 181]];
const POPUP_HEADER: [usize; 3] = [161, 179, 188];
const POPUP_ROW: [usize; 3] = [157, 175, 184];
const POPUP_CURSOR: [usize; 3] = [156, 174, 183];
const POPUP_AP: usize = 152;
const WHITE: usize = 123;
const GREY: usize = 119;
const VISIBLE: usize = 9;
/// Item type of abilities; the "on" name icon (0x1405ba700).
const ABILITY_TYPE: u8 = 0x13;
const EQUIPPED_ICON: u8 = 0x0d;
/// A character row's badge offset from the row (camp-06 §2.3).
const CHAR_BADGE_X: f32 = 100.;

/// One member's abilities: 80 slots (bit 15 = equipped, 0 = empty), AP
/// maximum and the equipped weapon's ability.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemberAbilities {
    pub slots: Vec<u16>,
    pub ap_max: i32,
    pub weapon_ability: u16,
    /// The abilities of the Valor, Master and Final Form weapons
    /// (FUN_140361200 lists each owned form's own weapon too), 0 = none.
    pub form_weapon_abilities: Vec<u16>,
    /// A party member's frequencies (record +0xf5, 31 bytes, indexed by
    /// FUN_1401dc060); empty for Sora.
    pub frequency: Vec<u8>,
    /// The member's obtained-order list (ids, save+0x4278 for Sora,
    /// +0x4318 + 0x60·(t−2) for the party) with each position's "new" bit
    /// (DAT_140bf0dc0, set at the camp's start by FUN_140368ed0) and
    /// "seen" bit (save+0x41f8 / +0x4204). Empty: no NEW! marks.
    pub order: Vec<u16>,
    pub new: Vec<bool>,
    pub seen: Vec<bool>,
}

impl MemberAbilities {
    /// The NEW! state of ability `id` (camp-06 §1.5): `Some(seen)` while
    /// its position in the obtained-order list is new.
    pub fn badge(&self, id: u16) -> Option<bool> {
        let i = self.order.iter().position(|&o| o == id & 0x7fff)?;
        self.new
            .get(i)
            .copied()
            .unwrap_or(false)
            .then(|| self.seen.get(i).copied().unwrap_or(false))
    }

    /// FUN_14036a5f0 / FUN_14036a3a0 for a whole member: `Some(all seen)`
    /// when any position is new.
    pub fn any_badge(&self) -> Option<bool> {
        let new: Vec<usize> = (0..self.order.len())
            .filter(|&i| self.new.get(i).copied().unwrap_or(false))
            .collect();
        (!new.is_empty()).then(|| {
            new.iter()
                .all(|&i| self.seen.get(i).copied().unwrap_or(false))
        })
    }

    /// FUN_14036a120: visiting the row of `id` sets its seen bit.
    pub fn see(&mut self, id: u16) {
        if let Some(i) = self.order.iter().position(|&o| o == id & 0x7fff) {
            if self.seen.len() <= i {
                self.seen.resize(i + 1, false);
            }
            self.seen[i] = true;
        }
    }
}

/// FUN_140368ed0 for one member at the camp's start: the abilities in the
/// record's slots that the obtained-order list lacks are appended (up to
/// `cap`, 80 for Sora and 48 for the party), each position whose seen bit
/// is clear is new, and a listed ability no longer in the slots is marked
/// seen.
pub fn new_marks(
    order: &mut Vec<u16>,
    seen: &mut Vec<bool>,
    slots: &[u16],
    cap: usize,
) -> Vec<bool> {
    for &s in slots {
        let id = s & 0x7fff;
        if id == 0 {
            continue;
        }
        if order.len() >= cap {
            break;
        }
        if !order.contains(&id) {
            order.push(id);
        }
    }
    seen.resize(order.len(), false);
    let new: Vec<bool> = seen.iter().map(|&s| !s).collect();
    for (i, &id) in order.iter().enumerate() {
        if !slots.iter().any(|&s| s & 0x7fff == id) {
            seen[i] = true;
        }
    }
    new
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AbilityTexts {
    /// 0xbb8b (members) and 0xbb8c (forms).
    pub help: [Vec<u8>; 2],
    /// 0xb2ee Action, 0xb2ef Growth, 0xb2f0 Support, 0xb2ed Weapons.
    pub categories: [Vec<u8>; 4],
    /// 0xb372 "There are no Abilities."
    pub empty: Vec<u8>,
    /// 0xb629 "Don´t Equip", 0xb627 "Equip".
    pub choices: [Vec<u8>; 2],
    /// 0xb62a / 0xb628 popup help.
    pub choice_help: [Vec<u8>; 2],
    /// 0xb373 "<09>ÓItems", 0xb619 "<09>ÓAbilities", 0xb61a "Switch".
    pub items_banner: Vec<u8>,
    pub abilities_banner: Vec<u8>,
    pub switch: Vec<u8>,
    /// 0x8454 "<09>Ô" (the TRIANGLE bubble).
    pub triangle: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AbilitiesData {
    pub members: Vec<MemberAbilities>,
    pub texts: AbilityTexts,
}

/// A work-area entry (8 B at +0xc): id, equipped, source slot (None = the
/// weapon), category 0 Action / 1 Growth / 2 Support / 3 Weapons, AP cost,
/// the record's kind and a party action ability's frequency (+6; None =
/// 0xff).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: u16,
    pub equipped: bool,
    pub slot: Option<usize>,
    pub cat: u8,
    pub cost: i32,
    pub kind: u8,
    pub freq: Option<u8>,
}

/// The frequency table (customize table 4, 0x1405bc1f0): icon and value of
/// Free, Well-Balanced, Rare.
pub(super) const FREQUENCIES: [(u8, u8); 3] = [(0x1a, 0), (0x1b, 1), (0x1c, 2)];

/// FUN_14035b2c0(4, value): the record with that value.
pub(super) fn frequency_record(value: u8) -> Option<usize> {
    FREQUENCIES.iter().position(|r| r.1 == value)
}

/// `09 icon` of a frequency (empty when no record has the value).
pub(super) fn frequency_glyph(value: u8) -> Vec<u8> {
    frequency_record(value).map_or_else(Vec::new, |k| vec![0x09, FREQUENCIES[k].0])
}

/// The record's kind (+6) → category: 0 Support, 1 Limit / 2 Action →
/// Action, 3 Growth.
fn category(kind: u8) -> u8 {
    match kind {
        0 => 2,
        1 | 2 => 0,
        3 => 1,
        _ => 0,
    }
}

/// FUN_140360920 + FUN_140361200 + FUN_1403632f0: the entries of a member
/// in the exe's order (exchange sort by category, then by item-table
/// position inside a category; weapons keep their place).
pub fn entries(table: &[ItemInfo], m: &MemberAbilities) -> Vec<Entry> {
    let record = |id: u16| table.iter().find(|r| r.id == id);
    let mut out = Vec::new();
    for (s, &v) in m.slots.iter().enumerate() {
        let id = v & 0x7fff;
        if id == 0 {
            continue;
        }
        let param = record(id).map_or(0, |r| r.param);
        let kind = param as u8;
        // FUN_140360f40: a party member's action abilities.
        let freq = (kind == 2)
            .then(|| player::party_members::frequency_index(id))
            .flatten()
            .and_then(|k| m.frequency.get(k).copied());
        out.push(Entry {
            id,
            equipped: v & 0x8000 != 0,
            slot: Some(s),
            cat: category(kind),
            cost: (param >> 8) as i32,
            kind,
            freq,
        });
    }
    for ability in std::iter::once(m.weapon_ability).chain(m.form_weapon_abilities.iter().copied())
    {
        if ability != 0 {
            out.push(Entry {
                id: ability,
                equipped: true,
                slot: None,
                cat: 3,
                cost: 0,
                kind: 0,
                freq: None,
            });
        }
    }
    let n = out.len();
    for a in 0..n {
        for b in a + 1..n {
            if out[b].cat < out[a].cat {
                out.swap(a, b);
            }
        }
    }
    let key = |e: &Entry| -> i64 {
        table
            .iter()
            .position(|r| r.id == e.id && r.kind == ABILITY_TYPE)
            .map_or(-1, |p| p as i64)
    };
    let mut keys: Vec<i64> = out.iter().map(key).collect();
    for a in 0..n {
        if out[a].cat == 3 {
            continue;
        }
        for b in a + 1..n {
            if out[b].cat != out[a].cat {
                break;
            }
            if keys[b] < keys[a] {
                out.swap(a, b);
                keys.swap(a, b);
            }
        }
    }
    out
}

/// AP used: Σ cost of the equipped entries (FUN_14035edc0).
pub fn used(entries: &[Entry]) -> i32 {
    entries.iter().filter(|e| e.equipped).map(|e| e.cost).sum()
}

/// The screens' sprites and lists.
#[derive(Default)]
pub(super) struct Ui {
    pub(super) chars: MenuList,
    panel: Option<Id>,
    bars: Vec<Id>,
    texts: Vec<Id>,
    entries: Vec<Entry>,
    member: usize,
    // 0x10.
    pub(super) list: Grid,
    rows: Vec<Id>,
    names: Vec<Id>,
    costs: Vec<Id>,
    /// The frequency icons the party rows' draw callback shows
    /// (FUN_140360270), right-aligned at the LightCur point.
    freqs: Vec<Id>,
    tabs: Vec<Id>,
    /// The list rows' and the character rows' NEW! badges.
    badges: Vec<Id>,
    char_badges: Vec<Id>,
    frame: Option<Id>,
    bubble: Option<Id>,
    gauge: Option<Id>,
    gain: Option<Id>,
    now: Option<Id>,
    // Popup.
    pub(super) popup: MenuList,
    cat: usize,
    header: Option<Id>,
}

impl Camp {
    fn ability_name(&self, e: &Entry) -> Vec<u8> {
        let r = self.info(e.id);
        let icon = if e.equipped { EQUIPPED_ICON } else { r.icon };
        let mut out = vec![0x09, icon];
        out.extend(r.name);
        out
    }

    /// A party member's row (p ≠ 0): its action abilities have frequencies.
    fn party_member(&self) -> bool {
        self.data
            .abilities
            .members
            .get(self.ability_ui.member)
            .is_some_and(|m| !m.frequency.is_empty())
    }

    /// A party member's action ability (cat 0, kind ≠ 1).
    fn party_action(&self, e: &Entry) -> bool {
        self.party_member() && e.cat == 0 && e.kind != 1
    }

    /// `rec[0xf5 + FUN_1401dc060(id)] = value`.
    fn write_frequency(&mut self, id: u16, value: u8) {
        let member = self.ability_ui.member;
        if let (Some(k), Some(m)) = (
            player::party_members::frequency_index(id),
            self.data.abilities.members.get_mut(member),
        ) && let Some(f) = m.frequency.get_mut(k)
        {
            *f = value;
        }
    }

    fn ap_max(&self) -> i32 {
        self.data
            .abilities
            .members
            .get(self.ability_ui.member)
            .map_or(0, |m| m.ap_max)
    }

    fn rebuild_entries(&mut self) {
        let m = self
            .data
            .abilities
            .members
            .get(self.ability_ui.member)
            .cloned()
            .unwrap_or_default();
        self.ability_ui.entries = entries(&self.data.items.table, &m);
    }

    /// FUN_1403035b0: the top-right banner (G237 → 238), right-aligned.
    pub(super) fn show_banner(&mut self, seq: &Sequence, text: Vec<u8>) {
        if let Some(b) = self.banner
            && self.sprites.alive(b)
        {
            self.set_text(b, text);
            return;
        }
        let mut t = Text::new(text, TEXT_SIZE);
        t.align = 1;
        t.group_position = false;
        let b = self
            .sprites
            .create(seq, PRIO_ROW, ART, BANNER[0], Some(BANNER[1]), Some(t));
        self.closers.push((b, BANNER[2]));
        self.banner = Some(b);
    }

    pub(super) fn close_banner(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.banner, BANNER[2]);
    }

    /// Whether a top-level entry is offered (DAT_140beec20 bits).
    pub(super) fn entry_enabled(&self, id: u8) -> bool {
        self.data.entries.iter().any(|e| e.id == id)
    }

    // ---- 0xf ---------------------------------------------------------------

    /// FUN_140362ca0: tab 0, the character rows, their preview panel.
    pub(super) fn enter_ability_chars(&mut self, seq: &Sequence, member: Option<usize>) {
        self.state = State::AbilityChars;
        self.substate = 0;
        self.create_tab(seq, 0);
        let names: Vec<Vec<u8>> = self
            .data
            .panels
            .members
            .iter()
            .map(|m| m.name.clone())
            .collect();
        let n = names.len().max(1);
        if let Some(m) = member
            && m != self.memory.ability_char
        {
            self.memory.ability_list = (0, 0);
            self.memory.ability_char = m;
        }
        let cursor = self.memory.ability_char.min(n - 1);
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
            rows.push(r);
        }
        // A character row's NEW! badge at row base + (100, 0) (camp-06
        // §2.3): any new ability, dim once all are seen.
        self.ability_ui.char_badges.clear();
        for (i, &r) in rows.iter().enumerate() {
            let group = self
                .data
                .abilities
                .members
                .get(i)
                .and_then(MemberAbilities::any_badge)
                .map_or(BLANK, |seen| items::BADGE[seen as usize]);
            let b = self
                .sprites
                .create(seq, PRIO_ROW + 1, ART, group, None, None);
            self.child(b, r);
            self.sprites.get_mut(b).offset = [CHAR_BADGE_X, 0.];
            self.ability_ui.char_badges.push(b);
        }
        self.ability_ui.chars = MenuList::new(rows, cursor);
        let panel = self.sprites.create(
            seq,
            PRIO_PANEL,
            ART,
            STATUS_PANEL[0],
            Some(STATUS_PANEL[1]),
            None,
        );
        self.closers.push((panel, STATUS_PANEL[2]));
        self.ability_ui.panel = Some(panel);
        let g = &seq.groups[PREVIEW[3]];
        let pitch = [g.text_position_x as f32, g.ui_padding as f32];
        let value_at = [g.light_position_x as f32, g.text_position_y as f32];
        self.ability_ui.bars.clear();
        self.ability_ui.texts.clear();
        for k in 0..20 {
            let l = self.sprites.create(
                seq,
                PRIO_BAR,
                ART,
                PREVIEW[3],
                None,
                Some(Self::kept_text(Vec::new(), TEXT_SIZE)),
            );
            self.sprites.set(seq, l, BLANK, None);
            self.child(l, panel);
            self.sprites.get_mut(l).offset =
                [pitch[0] * (k & 1) as f32, pitch[1] * (k >> 1) as f32];
            let mut vt = self.row_text(Vec::new());
            vt.align = 1;
            let v = self
                .sprites
                .create(seq, PRIO_TEXT, ART, WHITE, None, Some(vt));
            self.child(v, l);
            self.sprites.get_mut(v).offset = value_at;
            self.ability_ui.bars.push(l);
            self.ability_ui.texts.push(v);
        }
        self.ability_ui.member = cursor;
        self.ability_preview(seq);
        self.ability_chars_mask();
        if self.entry_enabled(0) {
            let t = self.data.abilities.texts.items_banner.clone();
            self.show_banner(seq, t);
        }
    }

    fn ability_chars_mask(&mut self) {
        self.ability_ui.chars.mask = 0x73 | if self.entry_enabled(0) { 0x100 } else { 0 };
    }

    /// FUN_1403642a0: the cursor character's equipped abilities, row-major
    /// in two columns; the panel is not drawn when there are none.
    fn ability_preview(&mut self, seq: &Sequence) {
        self.ability_ui.member = self.ability_ui.chars.cursor;
        self.rebuild_entries();
        for k in 0..self.ability_ui.bars.len() {
            let (l, v) = (self.ability_ui.bars[k], self.ability_ui.texts[k]);
            self.sprites.set(seq, l, BLANK, None);
            self.set_text(l, Vec::new());
            self.sprites.set(seq, v, BLANK, None);
            self.set_text(v, Vec::new());
        }
        let equipped: Vec<Entry> = self
            .ability_ui
            .entries
            .iter()
            .copied()
            .filter(|e| e.equipped)
            .take(20)
            .collect();
        for (k, e) in equipped.iter().enumerate() {
            let l = self.ability_ui.bars[k];
            self.sprites.set(seq, l, PREVIEW[e.cat as usize], None);
            let name = self.ability_name(e);
            self.set_text(l, name);
            if let Some(f) = e.freq {
                let v = self.ability_ui.texts[k];
                self.sprites.set(seq, v, WHITE, None);
                self.set_text(v, frequency_glyph(f));
            }
        }
        let hide = self.ability_ui.entries.is_empty() || equipped.is_empty();
        let tagged: Vec<Id> = self
            .ability_ui
            .panel
            .into_iter()
            .chain(self.ability_ui.bars.iter().copied())
            .chain(self.ability_ui.texts.iter().copied())
            .collect();
        for id in tagged {
            self.sprites.get_mut(id).hidden = hide;
        }
    }

    /// FUN_140363770 in 0xf.
    fn ability_chars_help(&mut self, seq: &Sequence) {
        let h = self.data.abilities.texts.help[0].clone();
        self.show_help(seq, h);
    }

    fn close_ability_chars(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.ability_ui.panel, STATUS_PANEL[2]);
        for &r in &self.ability_ui.chars.rows.clone() {
            self.close_opt(seq, Some(r), TOP_ROWS.close);
        }
    }

    /// FUN_14035fd30.
    pub(super) fn ability_chars(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let prev = self.ability_ui.chars.cursor;
        let (pick, moved) = self.ability_ui.chars.input(pad);
        if self.substate == 2 {
            let p = self.pending;
            let busy = (p == Some(State::Top) && self.alive_opt(self.banner))
                || self.alive_opt(self.ability_ui.panel)
                || self.rows_alive(&self.ability_ui.chars.clone())
                || (matches!(p, Some(State::Top) | Some(State::Items)) && self.tab_alive(0));
            if !busy {
                match self.pending.take() {
                    Some(State::Top) => self.return_top(seq),
                    Some(State::Items) => {
                        self.kill_tab(0);
                        self.memory.entry = 0;
                        let label = self.entry_label(0);
                        self.tab_text(0, label);
                        let slot = self.memory.ability_char;
                        self.enter_items(seq, Some(slot));
                    }
                    _ => self.enter_ability_list(seq),
                }
            } else if pad.trig != 0 {
                self.kill_opt(self.ability_ui.panel);
                for &r in &self.ability_ui.chars.rows {
                    self.sprites.kill(r);
                }
                if p == Some(State::Top) {
                    self.kill_opt(self.banner);
                }
                if matches!(p, Some(State::Top) | Some(State::Items)) {
                    self.kill_tab(0);
                    self.kill_tab(1);
                }
            }
            return false;
        }
        let row0 = self.group_of(self.ability_ui.chars.rows.first().copied());
        if (pick != Pick::None || moved) && row0 == Some(TOP_ROWS.appear) {
            if let Some(p) = self.ability_ui.panel {
                self.sprites.set(seq, p, STATUS_PANEL[1], None);
            }
            for (i, &r) in self.ability_ui.chars.rows.clone().iter().enumerate() {
                let g = if i == prev {
                    TOP_ROWS.selected
                } else {
                    TOP_ROWS.idle
                };
                self.sprites.set(seq, r, g, None);
            }
            if let Some(b) = self.banner
                && self.sprites.alive(b)
            {
                self.sprites.set(seq, b, BANNER[1], None);
            }
        }
        let row0 = self.group_of(self.ability_ui.chars.rows.first().copied());
        if self.substate == 0
            && matches!(row0, Some(g) if g == TOP_ROWS.idle || g == TOP_ROWS.selected)
        {
            self.ability_chars_help(seq);
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            self.ability_preview(seq);
            self.ability_chars_help(seq);
        }
        let cursor = self.ability_ui.chars.cursor;
        if self.memory.ability_char != cursor {
            self.memory.ability_list = (0, 0);
            self.memory.ability_char = cursor;
        }
        match pick {
            Pick::Square | Pick::Cancel => {
                self.hide_help(seq);
                self.close_banner(seq);
                self.close_tab(seq, 0);
                self.close_ability_chars(seq);
                self.pending = Some(if pick == Pick::Square {
                    State::Items
                } else {
                    State::Top
                });
                self.substate = 2;
            }
            Pick::Start => self.exit = true,
            Pick::Row(r) => {
                self.hide_help(seq);
                self.close_ability_chars(seq);
                let name = self.data.panels.members[r].name.clone();
                self.pending_tab_text[1] = Some(name);
                self.pending = Some(State::AbilityList);
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }

    pub(super) fn entry_label(&self, id: u8) -> Vec<u8> {
        self.data
            .entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.label.clone())
            .unwrap_or_default()
    }

    /// FUN_140362200(0xf, member): from Items (SQUARE) into this screen.
    pub(super) fn abilities_from_items(&mut self, seq: &Sequence, member: usize) {
        self.kill_tab(0);
        self.memory.entry = 1;
        let label = self.entry_label(1);
        self.tab_text(0, label);
        self.enter_ability_chars(seq, Some(member));
    }

    /// FUN_140362200(0x10, member): straight into a member's list (from
    /// Equip's SQUARE).
    pub(super) fn enter_ability_list_for(&mut self, seq: &Sequence, member: usize) {
        self.ability_ui.member = member;
        self.enter_ability_list(seq);
    }

    // ---- 0x10 --------------------------------------------------------------

    /// FUN_1403623b0: the list panel, ≤ 9 rows with names, AP cells,
    /// badges and category tabs, the scrollbar and the AP gauge.
    fn enter_ability_list(&mut self, seq: &Sequence) {
        self.state = State::AbilityList;
        self.substate = 0;
        self.create_tab(seq, 0);
        self.create_tab(seq, 1);
        self.rebuild_entries();
        let n = self.ability_ui.entries.len();
        let panel = self.sprites.create(
            seq,
            PRIO_LIST,
            ART,
            LIST_PANEL[0],
            Some(LIST_PANEL[1]),
            None,
        );
        self.closers.push((panel, LIST_PANEL[2]));
        self.ability_ui.panel = Some(panel);
        let visible = if n == 0 { 1 } else { n.min(VISIBLE) };
        let rg = &seq.groups[LIST_ROW[0]];
        let pitch = rg.ui_padding as f32;
        let badge_at = [rg.light_position_x as f32, rg.text_position_y as f32];
        let tab_pitch = seq.groups[LIST_TAB[0]].ui_padding as f32;
        let tab_size = seq.groups[LIST_TAB[0]].text_scale as f32;
        self.ability_ui.rows.clear();
        self.ability_ui.names.clear();
        self.ability_ui.costs.clear();
        self.ability_ui.freqs.clear();
        self.ability_ui.tabs.clear();
        self.ability_ui.badges.clear();
        for i in 0..visible {
            let r = self.sprites.create(seq, PRIO_LIST, ART, BLANK, None, None);
            self.child(r, panel);
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            let nm = self.sprites.create(
                seq,
                PRIO_LIST,
                ART,
                WHITE,
                None,
                Some(self.row_text(Vec::new())),
            );
            self.child(nm, r);
            let mut ct = Self::kept_text(Vec::new(), TEXT_SIZE);
            ct.align = 1;
            ct.group_position = true;
            let c = self
                .sprites
                .create(seq, PRIO_LIST, ART, BLANK, None, Some(ct));
            self.child(c, r);
            let b = self
                .sprites
                .create(seq, PRIO_LIST_BADGE, ART, BLANK, None, None);
            self.child(b, r);
            self.sprites.get_mut(b).offset = badge_at;
            self.ability_ui.badges.push(b);
            let mut ft = self.row_text(Vec::new());
            ft.align = 1;
            let f = self
                .sprites
                .create(seq, PRIO_LIST, ART, WHITE, None, Some(ft));
            self.child(f, r);
            self.sprites.get_mut(f).offset = badge_at;
            self.ability_ui.rows.push(r);
            self.ability_ui.names.push(nm);
            self.ability_ui.costs.push(c);
            self.ability_ui.freqs.push(f);
        }
        for j in 0..visible {
            let mut t = Text::new(Vec::new(), tab_size);
            t.align = 2;
            t.keep_rgb = false;
            let h = self
                .sprites
                .create(seq, PRIO_LIST, ART, BLANK, None, Some(t));
            self.child(h, panel);
            self.sprites.get_mut(h).offset = [0., tab_pitch * j as f32];
            self.ability_ui.tabs.push(h);
        }
        self.build_scrollbar(seq, panel, PRIO_LIST, n.max(visible), visible);
        let total = n.max(1);
        let (mut cur, mut sc) = self.memory.ability_list;
        if total < visible + sc {
            sc = total.saturating_sub(visible);
        }
        if total <= visible && cur >= total {
            sc = 0;
            cur = total - 1;
        }
        let mut list = MenuList::new(self.ability_ui.rows.clone(), cur.min(visible - 1));
        list.total = total;
        list.page = Some(VISIBLE);
        self.ability_ui.list = Grid {
            list,
            scroll: sc,
            cols: 1,
            lines: visible,
        };
        self.memory.ability_list = (0, 0);
        // DAT_140beec22: no hand or LightCur on an empty list.
        self.item_ui.no_pointer = n == 0;
        self.ability_list_mask();
        let mut banner = self.data.abilities.texts.switch.clone();
        if self.entry_enabled(0) {
            banner.push(0x01);
            banner.extend(self.data.abilities.texts.items_banner.clone());
        }
        self.show_banner(seq, banner);
        if self.alive_opt(self.ability_ui.gauge) {
            self.sprites
                .set(seq, self.ability_ui.gauge.unwrap(), GAUGE[1], None);
        } else {
            self.build_gauge(seq);
        }
        self.see_cursor_ability();
        self.ability_skin(seq);
        self.update_gauge(seq, true);
    }

    /// FUN_140363c70: the cursor's ability is seen (on entry and every
    /// cursor change), before the rows are skinned again.
    fn see_cursor_ability(&mut self) {
        let member = self.ability_ui.member;
        if let Some(e) = self.cursor_entry()
            && e.slot.is_some()
            && let Some(m) = self.data.abilities.members.get_mut(member)
        {
            m.see(e.id);
        }
    }

    fn cursor_entry(&self) -> Option<Entry> {
        self.ability_ui
            .entries
            .get(self.ability_ui.list.list.cursor + self.ability_ui.list.scroll)
            .copied()
    }

    /// The list's enable mask (§3.1): page keys, L2/R2, TRIANGLE when the
    /// cursor ability costs AP; no decide on an empty list.
    fn ability_list_mask(&mut self) {
        let n = self.ability_ui.entries.len();
        let mut mask = 0x73 | 0x3000 | 0xc000;
        if self.cursor_entry().is_some_and(|e| e.cost != 0) {
            mask |= 0x200;
        }
        // SELECT: the Abilities notice (category 2).
        if self.notice_available(2) {
            mask |= 0x400;
        }
        if n == 0 {
            mask &= !0x10;
        }
        self.ability_ui.list.list.mask = mask;
    }

    /// FUN_140363db0: the visible rows' groups, names, AP costs and
    /// category tabs.
    fn ability_skin(&mut self, seq: &Sequence) {
        let visible = self.ability_ui.rows.len();
        for i in 0..visible {
            for id in [
                self.ability_ui.rows[i],
                self.ability_ui.costs[i],
                self.ability_ui.tabs[i],
            ] {
                self.sprites.set(seq, id, BLANK, None);
                self.set_text(id, Vec::new());
            }
            if let Some(&b) = self.ability_ui.badges.get(i) {
                self.sprites.set(seq, b, BLANK, None);
            }
            let nm = self.ability_ui.names[i];
            self.set_text(nm, Vec::new());
            if let Some(t) = self.sprites.get_mut(nm).text.as_mut() {
                t.align = 0;
            }
            self.set_text(self.ability_ui.freqs[i], Vec::new());
        }
        let n = self.ability_ui.entries.len();
        if n == 0 {
            self.sprites
                .set(seq, self.ability_ui.rows[0], LIST_EMPTY, None);
            let nm = self.ability_ui.names[0];
            self.sprites.set(seq, nm, WHITE, None);
            let empty = self.data.abilities.texts.empty.clone();
            if let Some(t) = self.sprites.get_mut(nm).text.as_mut() {
                t.bytes = empty;
                t.align = 2;
            }
            return;
        }
        let s = self.ability_ui.list.scroll;
        self.scroll_to(seq, s, n, visible);
        let entries = self.ability_ui.entries.clone();
        let mut prev = if s == 0 || entries[s - 1].cat != entries[s].cat {
            None
        } else {
            Some(entries[s].cat)
        };
        let room = (self.ap_max() - used(&entries)).max(0);
        for j in 0..visible {
            let Some(e) = entries.get(s + j) else {
                break;
            };
            let c = e.cat as usize;
            if prev != Some(e.cat) {
                let h = self.ability_ui.tabs[j];
                self.sprites.set(seq, h, LIST_TAB[c], None);
                let label = self.data.abilities.texts.categories[c].clone();
                self.set_text(h, label);
            }
            prev = Some(e.cat);
            self.sprites
                .set(seq, self.ability_ui.rows[j], LIST_ROW[c], None);
            let dim = !e.equipped && e.cost > room;
            let nm = self.ability_ui.names[j];
            self.sprites
                .set(seq, nm, if dim { GREY } else { WHITE }, None);
            let name = self.ability_name(e);
            self.set_text(nm, name);
            if e.cost != 0 {
                let ct = self.ability_ui.costs[j];
                self.sprites.set(seq, ct, AP_CELL, None);
                self.set_text(ct, encode_number(&e.cost.to_string()));
            }
            // FUN_140360270 draws it while the ability is equipped.
            if let Some(f) = e.freq.filter(|_| e.equipped) {
                self.set_text(self.ability_ui.freqs[j], frequency_glyph(f));
            }
            // The NEW! badge (camp-06 §1.5); the weapon's ability has none.
            let member = self.ability_ui.member;
            if let (Some(&b), Some(seen)) = (
                self.ability_ui.badges.get(j),
                e.slot
                    .and_then(|_| self.data.abilities.members.get(member)?.badge(e.id)),
            ) {
                self.sprites.set(seq, b, items::BADGE[seen as usize], None);
            }
        }
    }

    /// The AP gauge (G272 → 273), its preview and current bars.
    fn build_gauge(&mut self, seq: &Sequence) {
        let size = seq.groups[GAUGE[0]].text_scale as f32;
        let mut t = Text::new(Vec::new(), size);
        t.align = 1;
        let g = self
            .sprites
            .create(seq, PRIO_GAUGE, ART, GAUGE[0], Some(GAUGE[1]), Some(t));
        self.closers.push((g, GAUGE[2]));
        self.ability_ui.gauge = Some(g);
        let gain = self
            .sprites
            .create(seq, PRIO_GAUGE + 1, ART, GAIN, None, None);
        self.child(gain, g);
        let now = self
            .sprites
            .create(seq, PRIO_GAUGE + 2, ART, NOW, None, None);
        self.child(now, g);
        self.ability_ui.gain = Some(gain);
        self.ability_ui.now = Some(now);
    }

    /// FUN_140363510: "used/max", the current bar and the preview of the
    /// cursor ability's change (`flag`: in the popup, the Equip row).
    fn update_gauge(&mut self, seq: &Sequence, flag: bool) {
        let (Some(g), Some(gain), Some(now)) = (
            self.ability_ui.gauge,
            self.ability_ui.gain,
            self.ability_ui.now,
        ) else {
            return;
        };
        if self.ability_ui.entries.is_empty() {
            self.sprites.get_mut(gain).hidden = true;
            self.sprites.get_mut(now).hidden = true;
            return;
        }
        let max = self.ap_max().max(1);
        let u = used(&self.ability_ui.entries);
        self.set_text(g, encode_number(&format!("{u}/{}", self.ap_max())));
        let cur = (u as f32 / max as f32).min(1.);
        let e = if self.state == State::AbilityList {
            self.cursor_entry()
        } else {
            self.ability_ui
                .entries
                .get(self.ability_ui.list.list.cursor + self.ability_ui.list.scroll)
                .copied()
        };
        let mut d = e.map_or(0, |e| e.cost);
        if let Some(e) = e {
            if self.state == State::AbilityList {
                if e.equipped {
                    d = -d;
                }
            } else if !e.equipped {
                d = if flag { d } else { 0 };
            } else {
                d = if flag { 0 } else { -d };
            }
        }
        if u + d > max {
            d = max - u;
        }
        let new = ((u + d).abs() as f32 / max as f32).min(1.);
        self.sprites.get_mut(gain).hidden = d == 0;
        self.sprites.get_mut(now).hidden = false;
        let group = if cur <= new { GAIN } else { LOSS };
        self.sprites.set(seq, gain, group, None);
        self.sprites.get_mut(gain).scale = [cur.max(new), 1.];
        self.sprites.get_mut(now).scale = [cur.min(new), 1.];
    }

    /// FUN_140363770 in 0x10: the cursor ability's description.
    fn ability_list_help(&mut self, seq: &Sequence) {
        let e = self.cursor_entry();
        let mut h = e.map(|e| self.info(e.id).description).unwrap_or_default();
        // A party action ability: + its frequency's help.
        if let Some(e) = e.filter(|e| self.party_action(e) && e.equipped)
            && let Some(k) = e.freq.and_then(frequency_record)
        {
            h.extend(self.data.customize.texts.ability_frequency_help[k].clone());
        }
        self.show_help(seq, h);
    }

    /// FUN_140364480: the cursor frame on the hand's row and the △ bubble
    /// on the hand when the cursor ability can be toggled.
    fn ability_cursor_frame(&mut self, seq: &Sequence, group: usize) {
        let glove = self.glove.filter(|&g| self.sprites.alive(g));
        let Some(glove) = glove else {
            let gone = self.ability_ui.frame.take();
            self.kill_opt(gone);
            let gone = self.ability_ui.bubble.take();
            self.kill_opt(gone);
            return;
        };
        let row_offset = self
            .sprites
            .get(glove)
            .parent
            .map_or([0.; 2], |r| self.sprites.get(r).offset);
        let parent = self.ability_ui.panel.unwrap();
        let f = match self.ability_ui.frame {
            Some(f) if self.sprites.alive(f) => f,
            _ => {
                let f = self.sprites.create(seq, PRIO_FRAME, ART, group, None, None);
                self.child(f, parent);
                self.ability_ui.frame = Some(f);
                f
            }
        };
        self.sprites.get_mut(f).offset = row_offset;
        let toggles =
            self.state == State::AbilityList && self.cursor_entry().is_some_and(|e| e.cost != 0);
        if toggles {
            if !self.alive_opt(self.ability_ui.bubble) {
                let mut t = Text::new(self.data.abilities.texts.triangle.clone(), TEXT_SIZE);
                t.align = 2;
                let b = self
                    .sprites
                    .create(seq, PRIO_LIGHT, ART, BUBBLE, None, Some(t));
                self.ability_ui.bubble = Some(b);
            }
            let b = self.ability_ui.bubble.unwrap();
            self.sprites.get_mut(b).parent = Some(glove);
        } else {
            let gone = self.ability_ui.bubble.take();
            self.kill_opt(gone);
        }
    }

    fn close_ability_list(&mut self, seq: &Sequence, keep_gauge: bool) {
        self.close_opt(seq, self.ability_ui.panel, LIST_PANEL[2]);
        self.close_scrollbar(seq);
        if !keep_gauge {
            self.close_opt(seq, self.ability_ui.gauge, GAUGE[2]);
        }
        let gone = self.ability_ui.frame.take();
        self.kill_opt(gone);
        let gone = self.ability_ui.bubble.take();
        self.kill_opt(gone);
    }

    /// FUN_14035ee10 + FUN_1403617a0: equip or unequip entry `e`.
    fn set_equip(&mut self, e: usize, on: bool) {
        let Some(entry) = self.ability_ui.entries.get_mut(e) else {
            return;
        };
        entry.equipped = on;
        let (slot, member) = (entry.slot, self.ability_ui.member);
        if let (Some(s), Some(m)) = (slot, self.data.abilities.members.get_mut(member)) {
            let v = m.slots[s] & 0x7fff;
            m.slots[s] = if on { v | 0x8000 } else { v };
        }
    }

    /// FUN_14035f810.
    pub(super) fn ability_list(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let scroll = self.ability_ui.list.scroll;
        let (pick, moved) = self.ability_ui.list.input(pad);
        if self.substate == 2 {
            let p = self.pending;
            let popup = matches!(p, Some(State::AbilityPopup(_)));
            let busy = (!popup && self.tab_alive(1))
                || (popup && self.alive_opt(self.banner))
                || self.alive_opt(self.ability_ui.panel)
                || (!popup && self.alive_opt(self.ability_ui.gauge))
                || self.item_ui.scroll.iter().any(|&s| self.sprites.alive(s));
            if !busy {
                self.item_ui.no_pointer = false;
                match self.pending.take() {
                    Some(State::AbilityPopup(c)) => self.enter_ability_popup(seq, c),
                    Some(State::AbilityList) => self.enter_ability_list(seq),
                    _ => {
                        self.kill_tab(1);
                        self.enter_ability_chars(seq, None);
                    }
                }
            } else if pad.trig != 0 {
                self.kill_opt(self.ability_ui.panel);
                for &s in &self.item_ui.scroll {
                    self.sprites.kill(s);
                }
                if popup {
                    self.kill_opt(self.banner);
                } else {
                    self.kill_opt(self.ability_ui.gauge);
                    self.kill_tab(1);
                }
            }
            return false;
        }
        if (pick != Pick::None || moved)
            && self.group_of(self.ability_ui.panel) != Some(LIST_PANEL[1])
        {
            for (id, g) in [
                (self.ability_ui.panel, LIST_PANEL[1]),
                (self.ability_ui.gauge, GAUGE[1]),
                (self.banner, BANNER[1]),
            ] {
                if let Some(i) = id
                    && self.sprites.alive(i)
                {
                    self.sprites.set(seq, i, g, None);
                }
            }
        }
        if self.substate == 0 && self.group_of(self.ability_ui.panel) == Some(LIST_PANEL[1]) {
            self.ability_list_help(seq);
            self.substate = 1;
        }
        // FUN_140361620.
        let room = (self.ap_max() - used(&self.ability_ui.entries)).max(0);
        let fits = |e: &Entry| e.equipped || e.cost <= room;
        let valid = match pick {
            Pick::Next | Pick::Previous => self.data.panels.members.len() > 1,
            Pick::Triangle => self.cursor_entry().is_some_and(|e| fits(&e)),
            Pick::Row(r) => self
                .ability_ui
                .entries
                .get(r)
                .is_some_and(|e| e.cost != 0 && fits(e)),
            _ => true,
        };
        if let Some(s) = pick_sound(pick, moved, valid) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            self.see_cursor_ability();
            self.ability_skin(seq);
            let _ = scroll;
            self.ability_list_mask();
            self.update_gauge(seq, true);
            self.ability_list_help(seq);
        }
        let cursor = (
            self.ability_ui.list.list.cursor,
            self.ability_ui.list.scroll,
        );
        if valid {
            match pick {
                Pick::Triangle => {
                    let e = cursor.0 + cursor.1;
                    let entry = self.ability_ui.entries[e];
                    // FUN_14035ee10: a party action ability cycles Free →
                    // Well-Balanced → Rare → unequipped.
                    let on = if self.party_action(&entry) {
                        let u = if entry.equipped {
                            entry.freq.and_then(frequency_record).unwrap_or(3) as i32
                        } else {
                            -1
                        };
                        if u + 1 < 3 {
                            let value = FREQUENCIES[(u + 1) as usize].1;
                            self.ability_ui.entries[e].freq = Some(value);
                            self.write_frequency(entry.id, value);
                            true
                        } else {
                            self.ability_ui.entries[e].freq = None;
                            false
                        }
                    } else {
                        !entry.equipped
                    };
                    self.set_equip(e, on);
                    self.ability_skin(seq);
                    self.update_gauge(seq, true);
                    self.ability_list_help(seq);
                }
                // FUN_14035f130: the next / previous character row.
                Pick::Next | Pick::Previous => {
                    let n = self.data.panels.members.len();
                    let m = self.ability_ui.member;
                    let next = if pick == Pick::Next {
                        (m + 1) % n
                    } else {
                        (m + n - 1) % n
                    };
                    self.memory.ability_char = next;
                    self.memory.ability_list = (0, 0);
                    self.ability_ui.member = next;
                    self.pending_tab_text[1] = Some(self.data.panels.members[next].name.clone());
                    self.hide_help(seq);
                    self.close_tab(seq, 1);
                    self.close_ability_list(seq, false);
                    self.pending = Some(State::AbilityList);
                    self.substate = 2;
                }
                Pick::Select => {
                    self.memory.ability_list = cursor;
                    self.start_notices(seq, Some(2), State::AbilityList);
                }
                Pick::Start => {
                    self.memory.ability_list = cursor;
                    self.exit = true;
                }
                Pick::Cancel => {
                    self.memory.ability_list = cursor;
                    self.hide_help(seq);
                    self.close_tab(seq, 1);
                    self.close_ability_list(seq, false);
                    self.pending = Some(State::AbilityChars);
                    self.substate = 2;
                }
                Pick::Row(r) => {
                    self.memory.ability_list = cursor;
                    let c = self.ability_ui.entries[r].cat.min(2);
                    let label = self.data.abilities.texts.categories[c as usize].clone();
                    self.pending_tab_text[2] = Some(label);
                    self.close_banner(seq);
                    self.hide_help(seq);
                    self.close_ability_list(seq, true);
                    self.pending = Some(State::AbilityPopup(c));
                    self.substate = 2;
                }
                _ => {}
            }
        }
        self.ability_cursor_frame(seq, LIST_CURSOR);
        moved
    }

    // ---- 0x11–0x13 ---------------------------------------------------------

    /// FUN_140361a00: tab 2, the popup panel, its header with the ability
    /// name and AP cost, "Don´t Equip" / "Equip" and the scrollbar.
    fn enter_ability_popup(&mut self, seq: &Sequence, cat: u8) {
        self.state = State::AbilityPopup(cat);
        self.substate = 0;
        self.create_tab(seq, 2);
        let c = cat as usize;
        self.ability_ui.cat = c;
        let panel = self.sprites.create(
            seq,
            PRIO_LIST,
            ART,
            POPUP_PANEL[c][0],
            Some(POPUP_PANEL[c][2]),
            None,
        );
        self.closers.push((panel, POPUP_PANEL[c][1]));
        self.ability_ui.panel = Some(panel);
        let e = self.memory.ability_list.0 + self.memory.ability_list.1;
        let entry = self.ability_ui.entries.get(e).copied();
        let header = self
            .sprites
            .create(seq, PRIO_HEADER, ART, POPUP_HEADER[c], None, None);
        self.child(header, panel);
        self.ability_ui.header = Some(header);
        let title = entry.map(|e| self.info(e.id).name).unwrap_or_default();
        let t = self.sprites.create(
            seq,
            PRIO_POPUP_TEXT,
            ART,
            WHITE,
            None,
            Some(self.row_text(title)),
        );
        self.child(t, header);
        let hg = &seq.groups[POPUP_HEADER[c]];
        let ap_at = [hg.light_position_x as f32, hg.text_position_y as f32];
        let mut ct = self.row_text(encode_number(&entry.map_or(0, |e| e.cost).to_string()));
        ct.align = 1;
        let ap = self
            .sprites
            .create(seq, PRIO_POPUP_TEXT, ART, POPUP_AP, None, Some(ct));
        self.child(ap, header);
        self.sprites.get_mut(ap).offset = ap_at;
        // A party action ability: "Don´t Equip" and three "Equip" rows, each
        // with its frequency drawn by the row callback (FUN_140360270).
        let party = entry.is_some_and(|e| self.party_action(&e));
        let count = if party { 4 } else { 2 };
        let rg = &seq.groups[POPUP_ROW[c]];
        let pitch = rg.ui_padding as f32;
        let light = [rg.light_position_x as f32, rg.text_position_y as f32];
        let mut rows = Vec::new();
        for i in 0..count {
            let r = self
                .sprites
                .create(seq, PRIO_LIST, ART, POPUP_ROW[c], None, None);
            self.child(r, panel);
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            let label = self.data.abilities.texts.choices[i.min(1)].clone();
            let tx = self.sprites.create(
                seq,
                PRIO_POPUP_TEXT,
                ART,
                WHITE,
                None,
                Some(self.row_text(label)),
            );
            self.child(tx, r);
            if party && i > 0 {
                let mut ft =
                    self.row_text(self.data.customize.texts.ability_frequency[i - 1].clone());
                ft.align = 1;
                let f = self
                    .sprites
                    .create(seq, PRIO_POPUP_TEXT, ART, WHITE, None, Some(ft));
                self.child(f, r);
                self.sprites.get_mut(f).offset = light;
            }
            rows.push(r);
        }
        self.build_scrollbar(seq, panel, PRIO_HEADER, count, count);
        // FUN_1403618a0.
        let cursor = match entry {
            Some(e) if e.equipped && party => 1 + e.freq.and_then(frequency_record).unwrap_or(0),
            Some(e) => usize::from(e.equipped),
            None => 0,
        };
        self.ability_ui.popup = MenuList::new(rows, cursor);
        // The gauge's preview follows the popup's highlighted choice.
        self.ability_ui.list.list.cursor = self.memory.ability_list.0;
        self.ability_ui.list.scroll = self.memory.ability_list.1;
        self.update_gauge(seq, cursor != 0);
    }

    /// FUN_140363770 in the popup: "Don´t equip" / "Equip", and for a party
    /// action ability the chosen frequency's help after it.
    fn ability_popup_help(&mut self, seq: &Sequence) {
        let row = self.ability_ui.popup.cursor;
        let mut h = self.data.abilities.texts.choice_help[row.min(1)].clone();
        let e = self.memory.ability_list.0 + self.memory.ability_list.1;
        let party = self
            .ability_ui
            .entries
            .get(e)
            .is_some_and(|x| self.party_action(x));
        if party && row > 0 {
            h.extend(self.data.customize.texts.ability_frequency_help[(row - 1).min(2)].clone());
        }
        self.show_help(seq, h);
    }

    /// FUN_14035f610.
    pub(super) fn ability_popup(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let c = self.ability_ui.cat;
        let (pick, moved) = self.ability_ui.popup.input(pad);
        if self.substate == 2 {
            let busy = self.alive_opt(self.ability_ui.panel)
                || self.item_ui.scroll.iter().any(|&s| self.sprites.alive(s))
                || self.tab_alive(2)
                || self.alive_opt(self.banner);
            if !busy {
                self.pending = None;
                self.kill_tab(2);
                self.enter_ability_list(seq);
            } else if pad.trig != 0 {
                self.kill_opt(self.ability_ui.panel);
                for &s in &self.item_ui.scroll {
                    self.sprites.kill(s);
                }
            }
            return false;
        }
        if (pick != Pick::None || moved)
            && self.group_of(self.ability_ui.panel) != Some(POPUP_PANEL[c][2])
            && let Some(p) = self.ability_ui.panel
        {
            self.sprites.set(seq, p, POPUP_PANEL[c][2], None);
        }
        if self.substate == 0 && self.group_of(self.ability_ui.panel) == Some(POPUP_PANEL[c][2]) {
            self.ability_popup_help(seq);
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            let on = self.ability_ui.popup.cursor != 0;
            self.update_gauge(seq, on);
            self.ability_popup_help(seq);
        }
        match pick {
            Pick::Start => self.exit = true,
            Pick::Cancel | Pick::Row(_) => {
                if let Pick::Row(r) = pick {
                    // FUN_14035efd0.
                    let e = self.memory.ability_list.0 + self.memory.ability_list.1;
                    let entry = self.ability_ui.entries.get(e).copied();
                    self.set_equip(e, r != 0);
                    if let Some(entry) = entry.filter(|x| r != 0 && self.party_action(x)) {
                        let value = FREQUENCIES[(r - 1).min(2)].1;
                        self.ability_ui.entries[e].freq = Some(value);
                        self.write_frequency(entry.id, value);
                    }
                }
                self.hide_help(seq);
                self.close_banner(seq);
                self.close_tab(seq, 2);
                self.close_opt(seq, self.ability_ui.panel, POPUP_PANEL[c][1]);
                self.close_scrollbar(seq);
                let gone = self.ability_ui.frame.take();
                self.kill_opt(gone);
                self.pending = Some(State::AbilityList);
                self.substate = 2;
            }
            _ => {}
        }
        self.ability_cursor_frame(seq, POPUP_CURSOR[c]);
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ability(id: u16, kind: u8, cost: u8) -> ItemInfo {
        ItemInfo {
            id,
            kind: ABILITY_TYPE,
            param: u16::from(kind) | u16::from(cost) << 8,
            ..ItemInfo::default()
        }
    }

    #[test]
    fn entries_sort_by_category_then_table_order() {
        // Table order: Combo Plus (support), High Jump (growth), Guard
        // (action), Upper Slash (action).
        let table = vec![
            ability(0xa2, 0, 1),
            ability(0x5e, 3, 2),
            ability(0x52, 2, 2),
            ability(0x89, 2, 4),
        ];
        let m = MemberAbilities {
            slots: vec![0x89, 0xa2 | 0x8000, 0x52 | 0x8000, 0x5e],
            ap_max: 5,
            weapon_ability: 0x21e,
            ..MemberAbilities::default()
        };
        let e = entries(&table, &m);
        let ids: Vec<u16> = e.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![0x52, 0x89, 0x5e, 0xa2, 0x21e]);
        let cats: Vec<u8> = e.iter().map(|e| e.cat).collect();
        assert_eq!(cats, vec![0, 0, 1, 2, 3]);
        assert_eq!(e[0].slot, Some(2));
        assert_eq!(e[4].slot, None);
        // Guard 2 + Combo Plus 1 (the weapon costs nothing).
        assert_eq!(used(&e), 3);
        assert!(e.iter().all(|e| e.freq.is_none()));
    }

    #[test]
    fn new_abilities_badge_until_seen_and_stay_seen() {
        // First camp: two abilities learned, both new.
        let (mut order, mut seen) = (Vec::new(), Vec::new());
        let slots = vec![0x52 | 0x8000, 0x89];
        let new = new_marks(&mut order, &mut seen, &slots, 80);
        assert_eq!(order, vec![0x52, 0x89]);
        assert_eq!(new, vec![true, true]);
        let mut m = MemberAbilities {
            slots: slots.clone(),
            order,
            new,
            seen,
            ..MemberAbilities::default()
        };
        assert_eq!(m.badge(0x52 | 0x8000), Some(false));
        assert_eq!(m.any_badge(), Some(false));
        m.see(0x52);
        assert_eq!(m.badge(0x52), Some(true));
        assert_eq!(m.any_badge(), Some(false));
        m.see(0x89);
        assert_eq!(m.any_badge(), Some(true));
        // Next camp: a third ability; the seen two are no longer new.
        let (mut order, mut seen) = (m.order, m.seen);
        let new = new_marks(&mut order, &mut seen, &[0x52, 0x89, 0x5e], 80);
        assert_eq!(new, vec![false, false, true]);
        // An ability no longer in the slots is marked seen.
        let new = new_marks(&mut order, &mut seen, &[0x52], 80);
        assert_eq!(new, vec![false, false, true]);
        assert_eq!(seen, vec![true, true, true]);
    }

    #[test]
    fn a_party_members_action_abilities_carry_their_frequency() {
        // Donald's Thunder (action) and a support ability.
        let table = vec![ability(0xa7, 2, 2), ability(0x19b, 0, 1)];
        let mut frequency = vec![0; 31];
        frequency[9] = 2;
        let m = MemberAbilities {
            slots: vec![0xa7 | 0x8000, 0x19b | 0x8000],
            ap_max: 5,
            frequency,
            ..MemberAbilities::default()
        };
        let e = entries(&table, &m);
        assert_eq!((e[0].id, e[0].freq), (0xa7, Some(2)));
        assert_eq!(e[1].freq, None);
        assert_eq!(frequency_glyph(2), vec![0x09, 0x1c]);
        assert!(frequency_glyph(3).is_empty());
    }
}
