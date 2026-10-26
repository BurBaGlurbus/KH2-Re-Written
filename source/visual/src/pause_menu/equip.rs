//! The camp's Equip screens (camp-09 spec): a member's slot list with the
//! info panel (state 2: FUN_140357710 / FUN_140358c50 / FUN_140355ae0 /
//! FUN_1402fd390 mode 1), the pick lists for weapon, armor, accessory and
//! item slots with the stat comparison (5, 7–9: FUN_140356b00 /
//! FUN_1403581a0 / FUN_140355770 / FUN_1402fd410 modes 2–3), the
//! auto-reload toggle and its box (3: FUN_140354dc0 / FUN_140355660) and
//! the accessory AP box (4: FUN_140359200 / FUN_140355570), and the pick
//! lists' NEW! badges (FUN_14036a8f0 / FUN_14036a7f0, seen on each cursor
//! change by FUN_140358ba0). Form sub-weapons (6), item pictures, the
//! SELECT notice and the status recompute after an equip are not built.
use super::items::{Grid, type_icon};
use super::*;

const PRIO_PANEL: u32 = 0x753c;
const PRIO_LABEL: u32 = 0x753d;
const PRIO_TEXT: u32 = 0x753e;
const PRIO_INFO: u32 = 0x753f;
const PRIO_VALUE: u32 = 0x7540;
/// The item picture (FUN_1403022b0(0x753f, …)).
const PRIO_PICTURE: u32 = 0x753f;
const PANEL: [usize; 3] = [227, 228, 229];
const HOLDER: [usize; 3] = [287, 288, 289];
/// Slot rows and section labels per section (0x1405bc0d0 / 0x1405bc0f0):
/// weapon, form, armor, accessory, items.
const ROW: [usize; 5] = [265, 266, 252, 248, 262];
const LABEL: [usize; 5] = [283, 284, 278, 276, 280];
const FRAME: usize = 249;
const BUBBLE: usize = 24;
/// Pick lists per section (§3.1): panel {appear, close, idle}, header,
/// rows, cursor frame, ability box, holder {appear, close, idle}.
const PICK_PANEL: [[usize; 3]; 5] = [
    [207, 209, 208],
    [217, 219, 218],
    [171, 173, 172],
    [153, 155, 154],
    [180, 182, 181],
];
const PICK_HEADER: [usize; 5] = [216, 226, 179, 161, 188];
const PICK_ROW: [usize; 5] = [211, 221, 175, 157, 184];
const PICK_FRAME: [usize; 5] = [210, 220, 174, 156, 183];
const ABILITY_BOX: [usize; 2] = [212, 222];
const PICK_HOLDER: [[usize; 3]; 5] = [
    [213, 215, 214],
    [223, 225, 224],
    [176, 178, 177],
    [158, 160, 159],
    [185, 187, 186],
];
/// Section → state (0x1405bc198) and pick filter types (0x1405bc1a8).
const WHITE: usize = 123;
const YELLOW: usize = 124;
const RED: usize = 122;
const CYAN: usize = 118;
const VISIBLE: usize = 9;
const PICK_VISIBLE: usize = 8;
/// Info label groups per mode: (label, column B / ability).
const INFO_MODE1: [usize; 2] = [32, 33];
const INFO_MODE2: [usize; 2] = [34, 35];
const INFO_MODE3: [usize; 2] = [37, 38];

/// One member's stat totals (status +0x18e AP, +0x188 Str, +0x18a Mag,
/// +0x18c Def, resist bytes +0x1a9.. (100 = none)).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Totals {
    pub ap: i32,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
    pub resists: [u8; 4],
}

/// An equipment stat record (FUN_14041e8f0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ItemStats {
    pub id: u16,
    pub ability: u16,
    pub strength: u8,
    pub magic: u8,
    pub defense: u8,
    pub ap: u8,
    pub resists: [u8; 4],
    /// The physical, neutral and general resistance bytes (+8, +0xd, +0xe)
    /// the info rows leave out but the status totals multiply in.
    pub other_resists: [u8; 3],
}

impl ItemStats {
    /// The row FUN_1403c1190 adds to a member's totals.
    fn row(&self) -> player::equipment_stats::Row {
        let [fire, blizzard, thunder, dark] = self.resists;
        let [physical, neutral, general] = self.other_resists;
        player::equipment_stats::Row {
            strength: self.strength,
            magic: self.magic,
            defense: self.defense,
            ap: self.ap,
            resist: [physical, fire, blizzard, thunder, dark, neutral, general],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EquipTexts {
    /// 0x845b Weapons, 0x845d Valor, 0x8463 Armor, 0x8465 Accessories,
    /// 0x8467 Items.
    pub sections: [Vec<u8>; 5],
    /// 0x845d Valor, 0x845f Master, 0x8461 Final: the form weapons' labels.
    pub form_labels: [Vec<u8>; 3],
    /// 0x8449 "None", 0x844a "There are no items.".
    pub none: Vec<u8>,
    pub no_items: Vec<u8>,
    /// 0xcb8d / 0xcb8e / 0x8469 "Unequip …".
    pub unequip_help: [Vec<u8>; 3],
    /// 0x846a (auto-reload off) and 0xc22e (accessory AP) boxes.
    pub reload_box: Vec<u8>,
    pub accessory_box: Vec<u8>,
    /// Info labels: AP, Strength, Magic, Defense (0xc648..0xc64b), Fire,
    /// Blizzard, Thunder, Dark (0xc651..0xc654), Ability (0xc64c).
    pub stat_labels: [Vec<u8>; 9],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EquipData {
    pub stats: Vec<ItemStats>,
    pub totals: Vec<Totals>,
    /// Each member's totals before any equipment (the level row and the
    /// record's bonuses, resists 100), to rebuild `totals` as items change.
    pub bases: Vec<player::equipment_stats::Totals>,
    /// The Drive form weapon (item id, 0 = none) counted into each member's
    /// totals while a form is active.
    pub form_weapons: Vec<u16>,
    pub texts: EquipTexts,
}

/// A slot of the member's list: section, index in the section, shown id,
/// count (0 for an emptied auto-reload slot), alt (auto-reload on).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Slot {
    sec: usize,
    j: usize,
    id: u16,
    count: u8,
    alt: bool,
}

#[derive(Default)]
pub(super) struct Ui {
    panel: Option<Id>,
    holder: Option<Id>,
    slots: Vec<Slot>,
    pub(super) list: Grid,
    rows: Vec<Id>,
    texts: Vec<Id>,
    labels: Vec<Id>,
    frame: Option<Id>,
    bubble: Option<Id>,
    info: Vec<Id>,
    // Pick lists.
    picks: Vec<(u16, u8)>,
    pub(super) pick: Grid,
    pick_rows: Vec<Id>,
    pick_boxes: Vec<Id>,
    /// The pick rows' NEW! badges (P+0xa960+0x1f8·i, prio 0x753f).
    pick_badges: Vec<Id>,
    sec: usize,
    slot: usize,
    reload_id: u16,
}

/// FUN_1403e3ef0: weapons (types 2..13) by +6, armor / accessories by +4.
fn stat_entry(r: &items::ItemInfo) -> Option<u16> {
    match r.kind {
        2..=13 => Some(r.param),
        14 | 15 => Some(r.amount),
        _ => None,
    }
}

/// An item's stat record (FUN_1403e3ef0 → FUN_14041e8f0).
pub fn stats_of(table: &[items::ItemInfo], stats: &[ItemStats], id: u16) -> Option<ItemStats> {
    let r = table.iter().find(|r| r.id == id)?;
    let e = stat_entry(r)?;
    stats.iter().find(|s| s.id == e).copied()
}

/// FUN_1403c1d90's gear share: a member's totals from its base and the
/// stat rows of its weapons, the form weapon, armor and accessories, and
/// its weapon's ability.
pub fn gear_totals(
    table: &[items::ItemInfo],
    stats: &[ItemStats],
    base: player::equipment_stats::Totals,
    e: &items::Equipment,
    form: Option<u16>,
) -> (Totals, u16) {
    let rows: Vec<_> = [e.weapon, e.secondary_weapon]
        .into_iter()
        .chain(form)
        .chain(e.armor.iter().copied())
        .chain(e.accessories.iter().copied())
        .filter_map(|id| stats_of(table, stats, id))
        .map(|s| s.row())
        .collect();
    let t = player::equipment_stats::totals(base, rows);
    let totals = Totals {
        ap: i32::from(t.ap),
        strength: i32::from(t.strength),
        magic: i32::from(t.magic),
        defense: i32::from(t.defense),
        resists: [t.resist[1], t.resist[2], t.resist[3], t.resist[4]],
    };
    let ability = stats_of(table, stats, e.weapon).map_or(0, |s| s.ability);
    (totals, ability)
}

/// FUN_1402fd100: the info rows an item shows (bit k: 0 AP, 1 Str, 2 Mag,
/// 3 Def, 4–7 resists, 8 ability).
fn row_mask(kind: u8, s: &ItemStats) -> u32 {
    let mut m = match kind {
        2..=13 => 6 | if s.ability != 0 { 0x100 } else { 0 },
        14 => 0xf8,
        15 => 7,
        _ => 0,
    };
    for (k, v) in [s.ap, s.strength, s.magic, s.defense]
        .into_iter()
        .enumerate()
    {
        if v != 0 {
            m |= 1 << k;
        }
    }
    for (k, &r) in s.resists.iter().enumerate() {
        if r != 100 {
            m |= 1 << (4 + k);
        }
    }
    m
}

/// A stat value (k < 4) or a resist ("%d%%" of 100 − v).
fn stat_value(s: &ItemStats, k: usize) -> i32 {
    match k {
        0 => s.ap as i32,
        1 => s.strength as i32,
        2 => s.magic as i32,
        3 => s.defense as i32,
        _ => 100 - s.resists[k - 4] as i32,
    }
}

fn total_value(t: &Totals, k: usize) -> i32 {
    match k {
        0 => t.ap,
        1 => t.strength,
        2 => t.magic,
        3 => t.defense,
        _ => 100 - t.resists[k - 4] as i32,
    }
}

fn number_text(k: usize, v: i32) -> Vec<u8> {
    if k >= 4 {
        encode_number(&format!("{v}%"))
    } else {
        encode_number(&v.to_string())
    }
}

impl Camp {
    fn item_stats(&self, id: u16) -> Option<ItemStats> {
        stats_of(&self.data.items.table, &self.data.equip.stats, id)
    }

    fn member(&self) -> usize {
        self.memory.items_row
    }

    /// FUN_14034c4b0: weapon, armor, accessories, item slots.
    fn equip_slots(&self) -> Vec<Slot> {
        let e = self
            .data
            .items
            .equipment
            .get(self.member())
            .cloned()
            .unwrap_or_default();
        let mut out = vec![Slot {
            sec: 0,
            j: 0,
            id: e.weapon,
            count: u8::from(e.weapon != 0),
            alt: false,
        }];
        // FUN_14034c4b0 section 1: the form weapons of the owned forms.
        for (j, &id) in e.forms.iter().enumerate() {
            out.push(Slot {
                sec: 1,
                j,
                id,
                count: u8::from(id != 0),
                alt: false,
            });
        }
        for (sec, ids) in [(2, &e.armor), (3, &e.accessories)] {
            for (j, &id) in ids.iter().enumerate() {
                out.push(Slot {
                    sec,
                    j,
                    id,
                    count: u8::from(id != 0),
                    alt: false,
                });
            }
        }
        for (j, &cur) in e.items.iter().enumerate() {
            let auto = e.originals.get(j).copied().unwrap_or(0);
            let id = if cur != 0 { cur } else { auto };
            out.push(Slot {
                sec: 4,
                j,
                id,
                count: u8::from(cur != 0),
                alt: auto == id,
            });
        }
        out
    }

    /// The section label of a slot: a form weapon takes its form's name
    /// (0x845d Valor, 0x845f Master, 0x8461 Final).
    fn section_label(&self, s: &Slot) -> Vec<u8> {
        let texts = &self.data.equip.texts;
        if s.sec == 1 {
            let form = self
                .data
                .items
                .equipment
                .get(self.member())
                .and_then(|e| e.form_ids.get(s.j))
                .copied();
            let at = match form {
                Some(1) => 0,
                Some(4) => 1,
                Some(5) => 2,
                _ => return texts.sections[1].clone(),
            };
            if !texts.form_labels[at].is_empty() {
                return texts.form_labels[at].clone();
            }
        }
        texts.sections[s.sec].clone()
    }

    /// FUN_14034c1b0 (char mode): `09 g` + name; an empty slot `09 g`
    /// + "–––".
    fn slot_text(&self, s: &Slot) -> Vec<u8> {
        if s.id == 0 {
            let g = match s.sec {
                2 => 7,
                3 => 0x11,
                4 => 0,
                _ => 0xb,
            };
            let mut out = vec![0x09, g];
            out.extend(self.data.items.texts.blank.clone());
            return out;
        }
        let r = self.info(s.id);
        let mut out = vec![
            0x09,
            if s.alt {
                type_icon(r.kind, true)
            } else {
                r.icon
            },
        ];
        out.extend(r.name);
        out
    }

    // ---- state 2 -----------------------------------------------------------

    /// FUN_140357710.
    pub(super) fn enter_equip(&mut self, seq: &Sequence) {
        self.state = State::Equip;
        self.substate = 0;
        self.create_tab(seq, 0);
        self.create_tab(seq, 1);
        let panel = self
            .sprites
            .create(seq, PRIO_PANEL, ART, PANEL[0], Some(PANEL[1]), None);
        self.closers.push((panel, PANEL[2]));
        self.equip_ui.panel = Some(panel);
        let pitch = seq.groups[ROW[0]].ui_padding as f32;
        let label_size = seq.groups[LABEL[0]].text_scale as f32;
        self.equip_ui.rows.clear();
        self.equip_ui.texts.clear();
        self.equip_ui.labels.clear();
        for i in 0..VISIBLE {
            let r = self.sprites.create(seq, PRIO_PANEL, ART, BLANK, None, None);
            self.child(r, panel);
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            let t = self.sprites.create(
                seq,
                PRIO_TEXT,
                ART,
                WHITE,
                None,
                Some(self.row_text(Vec::new())),
            );
            self.child(t, r);
            let mut lt = Text::new(Vec::new(), if label_size > 0. { label_size } else { 13. });
            lt.align = 2;
            lt.keep_rgb = false;
            let l = self
                .sprites
                .create(seq, PRIO_LABEL, ART, BLANK, None, Some(lt));
            self.child(l, panel);
            self.sprites.get_mut(l).offset = [0., pitch * i as f32];
            self.equip_ui.rows.push(r);
            self.equip_ui.texts.push(t);
            self.equip_ui.labels.push(l);
        }
        self.equip_ui.slots = self.equip_slots();
        let n = self.equip_ui.slots.len();
        self.build_scrollbar(seq, panel, PRIO_LABEL, n, n.min(VISIBLE));
        let holder = self
            .sprites
            .create(seq, PRIO_PANEL, ART, HOLDER[0], Some(HOLDER[1]), None);
        self.closers.push((holder, HOLDER[2]));
        self.equip_ui.holder = Some(holder);
        // The item picture on the holder (items-spec §3.6).
        self.picture.setup(PRIO_PICTURE, None);
        self.picture.set_parent(holder);
        self.picture.reset();
        let (cur, sc) = self.memory.equip;
        let mut list = MenuList::new(self.equip_ui.rows.clone(), cur.min(VISIBLE - 1));
        list.total = n;
        list.page = Some(VISIBLE);
        self.equip_ui.list = Grid {
            list,
            scroll: sc.min(n.saturating_sub(1)),
            cols: 1,
            lines: VISIBLE,
        };
        self.equip_mask();
        self.equip_fill(seq);
        if self.entry_enabled(1) {
            let mut t = self.data.abilities.texts.switch.clone();
            t.push(0x01);
            t.extend(self.data.abilities.texts.abilities_banner.clone());
            self.show_banner(seq, t);
        }
    }

    fn equip_cursor(&self) -> Option<Slot> {
        self.equip_ui
            .slots
            .get(self.equip_ui.list.list.cursor + self.equip_ui.list.scroll)
            .copied()
    }

    /// The notice category of the slot under the cursor, when one has its
    /// flag.
    fn notice_for_slot(&self) -> Option<u8> {
        let c = match self.equip_cursor()?.sec {
            4 => 0,
            1 => 3,
            _ => return None,
        };
        self.notice_available(c).then_some(c)
    }

    fn equip_mask(&mut self) {
        let mut mask = 0x73 | 0x3000 | 0xc000;
        if self.entry_enabled(1) {
            mask |= 0x100;
        }
        if self.equip_cursor().is_some_and(|s| s.sec == 4) {
            mask |= 0x200;
        }
        // SELECT: the Auto-Reload notice on the Items section (0), the
        // Sub-Weapons notice on a form row (3).
        if self.notice_for_slot().is_some() {
            mask |= 0x400;
        }
        self.equip_ui.list.list.mask = mask;
    }

    /// FUN_140358c50: section labels on each section's first entry, bars,
    /// names (red for an emptied auto-reload slot).
    fn equip_fill(&mut self, seq: &Sequence) {
        for i in 0..VISIBLE {
            let (r, t, l) = (
                self.equip_ui.rows[i],
                self.equip_ui.texts[i],
                self.equip_ui.labels[i],
            );
            self.sprites.set(seq, r, BLANK, None);
            self.sprites.set(seq, t, WHITE, None);
            self.set_text(t, Vec::new());
            self.sprites.set(seq, l, BLANK, None);
            self.set_text(l, Vec::new());
        }
        let n = self.equip_ui.slots.len();
        let sc = self.equip_ui.list.scroll;
        self.scroll_to(seq, sc, n, n.min(VISIBLE));
        let slots = self.equip_ui.slots.clone();
        for v in 0..VISIBLE {
            let Some(s) = slots.get(sc + v) else { break };
            if s.j == 0 || s.sec == 1 {
                let l = self.equip_ui.labels[v];
                self.sprites.set(seq, l, LABEL[s.sec], None);
                self.set_text(l, self.section_label(s));
            }
            self.sprites
                .set(seq, self.equip_ui.rows[v], ROW[s.sec], None);
            let consumed = s.sec == 4 && s.alt && s.count == 0 && s.id != 0;
            let t = self.equip_ui.texts[v];
            self.sprites
                .set(seq, t, if consumed { RED } else { WHITE }, None);
            let text = self.slot_text(s);
            self.set_text(t, text);
        }
    }

    /// FUN_140358620 for state 2: help, the info panel (mode 1) and the
    /// picture.
    fn equip_help(&mut self, seq: &Sequence) {
        let s = self.equip_cursor();
        let id = s.map_or(0, |s| s.id);
        let h = if id == 0 {
            Vec::new()
        } else {
            self.info(id).description
        };
        self.show_help(seq, h);
        self.info_mode1(seq, id);
        let picture = (id != 0).then(|| self.info(id).picture);
        self.picture_request(seq, picture);
    }

    fn clear_info(&mut self) {
        for id in std::mem::take(&mut self.equip_ui.info) {
            self.sprites.kill(id);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn info_sprite(
        &mut self,
        seq: &Sequence,
        prio: u32,
        group: usize,
        parent: Id,
        offset: [f32; 2],
        text: Vec<u8>,
        align: u8,
    ) -> Id {
        let mut t = self.row_text(text);
        t.align = align;
        let s = self.sprites.create(seq, prio, ART, group, None, Some(t));
        self.child(s, parent);
        self.sprites.get_mut(s).offset = offset;
        self.equip_ui.info.push(s);
        s
    }

    /// FUN_1402fd390 mode 1: label, the item's value and the member's total
    /// per row; an ability row with the ability's name.
    fn info_mode1(&mut self, seq: &Sequence, id: u16) {
        self.clear_info();
        let Some(holder) = self.equip_ui.holder else {
            return;
        };
        let r = self.info(id);
        let Some(st) = (id != 0).then(|| self.item_stats(id)).flatten() else {
            return;
        };
        let mask = row_mask(r.kind, &st);
        let hg = &seq.groups[HOLDER[1]];
        let base = [hg.light_position_x as f32, hg.text_position_y as f32];
        let g = &seq.groups[INFO_MODE1[0]];
        let value_at = [g.light_position_x as f32, 0.];
        let total_at = [g.text_position_x as f32, 0.];
        let totals = self
            .data
            .equip
            .totals
            .get(self.member())
            .copied()
            .unwrap_or_default();
        let mut s = 0;
        for k in 0..9 {
            if mask >> k & 1 == 0 {
                continue;
            }
            let offset = [base[0], base[1] + 20. * s as f32];
            let label = self.data.equip.texts.stat_labels[k].clone();
            if k == 8 {
                let l = self.info_sprite(seq, PRIO_INFO, INFO_MODE1[1], holder, offset, label, 0);
                let name = self.info(st.ability).name;
                let ag = &seq.groups[INFO_MODE1[1]];
                let at = [ag.light_position_x as f32, ag.text_position_y as f32];
                self.info_sprite(seq, PRIO_VALUE, YELLOW, l, at, name, 2);
            } else {
                let l = self.info_sprite(seq, PRIO_INFO, INFO_MODE1[0], holder, offset, label, 0);
                let v = number_text(k, stat_value(&st, k));
                self.info_sprite(seq, PRIO_VALUE, WHITE, l, value_at, v, 1);
                let t = number_text(k, total_value(&totals, k));
                self.info_sprite(seq, PRIO_VALUE, YELLOW, l, total_at, t, 1);
            }
            s += 1;
        }
    }

    fn equip_frame(&mut self, seq: &Sequence, group: usize, bubble: bool) {
        let glove = self.glove.filter(|&g| self.sprites.alive(g));
        let Some(glove) = glove else {
            let f = self.equip_ui.frame.take();
            self.kill_opt(f);
            let b = self.equip_ui.bubble.take();
            self.kill_opt(b);
            return;
        };
        let offset = self
            .sprites
            .get(glove)
            .parent
            .map_or([0.; 2], |r| self.sprites.get(r).offset);
        let f = match self.equip_ui.frame {
            Some(f) if self.sprites.alive(f) => f,
            _ => {
                let f = self.sprites.create(seq, PRIO_TEXT, ART, group, None, None);
                self.child(f, self.equip_ui.panel.unwrap());
                self.equip_ui.frame = Some(f);
                f
            }
        };
        self.sprites.get_mut(f).offset = offset;
        if bubble {
            if !self.alive_opt(self.equip_ui.bubble) {
                let mut t = Text::new(self.data.abilities.texts.triangle.clone(), TEXT_SIZE);
                t.align = 2;
                let b = self
                    .sprites
                    .create(seq, PRIO_LIGHT, ART, BUBBLE, None, Some(t));
                self.equip_ui.bubble = Some(b);
            }
            let b = self.equip_ui.bubble.unwrap();
            self.sprites.get_mut(b).parent = Some(glove);
        } else {
            let b = self.equip_ui.bubble.take();
            self.kill_opt(b);
        }
    }

    /// FUN_140356500: panel and holder close with the scrollbar.
    fn close_equip(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.equip_ui.panel, PANEL[2]);
        self.close_opt(seq, self.equip_ui.holder, HOLDER[2]);
        self.close_scrollbar(seq);
        let b = self.equip_ui.bubble.take();
        self.kill_opt(b);
        let f = self.equip_ui.frame.take();
        self.kill_opt(f);
    }

    fn equip_busy(&self) -> bool {
        self.alive_opt(self.equip_ui.panel)
            || self.alive_opt(self.equip_ui.holder)
            || self.item_ui.scroll.iter().any(|&s| self.sprites.alive(s))
    }

    /// FUN_140357e30: empty armor / accessory slots go last; item slots are
    /// grouped by item in table order (FUN_140357f40).
    fn equip_tidy(&mut self) {
        let m = self.member();
        let Some(e) = self.data.items.equipment.get_mut(m) else {
            return;
        };
        for list in [&mut e.armor, &mut e.accessories] {
            let n = list.len();
            for i in 0..n {
                for k in i + 1..n {
                    if list[i] == 0 && list[k] != 0 {
                        list.swap(i, k);
                    }
                }
            }
        }
        self.sort_item_slots(m);
    }

    /// FUN_140357f40: member `m`'s item slots grouped by item in item-table
    /// order, a slot in stock before its used-up one, empty slots last.
    pub(super) fn sort_item_slots(&mut self, m: usize) {
        let table = &self.data.items.table;
        let Some(e) = self.data.items.equipment.get_mut(m) else {
            return;
        };
        let n = e.items.len();
        let mut slots: Vec<(u16, u16, u32)> = (0..n)
            .map(|j| (e.items[j], e.originals.get(j).copied().unwrap_or(0), 999))
            .collect();
        let mut distinct = Vec::new();
        for &(cur, auto, _) in &slots {
            let v = if cur != 0 { cur } else { auto };
            if v != 0 && !distinct.contains(&v) {
                distinct.push(v);
            }
        }
        let mut counter = 0;
        for rec in table.iter().filter(|r| r.kind == 0) {
            if distinct.contains(&rec.id) {
                for s in slots.iter_mut() {
                    if s.0 == rec.id || s.1 == rec.id {
                        s.2 = counter + u32::from(s.0 == 0);
                    }
                }
                counter += 2;
            }
        }
        for i in 0..n {
            for k in i + 1..n {
                if slots[k].2 < slots[i].2 {
                    slots.swap(i, k);
                }
            }
        }
        e.originals.resize(n, 0);
        for (j, (cur, auto, _)) in slots.into_iter().enumerate() {
            e.items[j] = cur;
            e.originals[j] = auto;
        }
    }

    /// FUN_140354dc0: auto-reload of `id` on / off in every slot with it.
    fn equip_reload(&mut self, id: u16, on: bool) {
        let m = self.member();
        if let Some(e) = self.data.items.equipment.get_mut(m) {
            e.originals.resize(e.items.len(), 0);
            for j in 0..e.items.len() {
                if e.items[j] == id || e.originals[j] == id {
                    e.originals[j] = if on { 0 } else { id };
                }
            }
        }
        self.equip_ui.slots = self.equip_slots();
    }

    /// FUN_140355ae0.
    pub(super) fn equip(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let scroll = self.equip_ui.list.scroll;
        let (pick, moved) = self.equip_ui.list.input(pad);
        if self.substate == 2 {
            let p = self.pending;
            let tab_busy = match p {
                Some(State::Items) | Some(State::Equip) => self.tab_alive(1),
                Some(State::AbilityList) => {
                    self.tabs.iter().flatten().any(|t| self.sprites.alive(t.id))
                        || self.alive_opt(self.banner)
                }
                _ => self.alive_opt(self.banner),
            };
            if !self.equip_busy() && !tab_busy {
                self.clear_info();
                match self.pending.take() {
                    Some(State::Items) => {
                        self.kill_tab(1);
                        self.enter_items(seq, None);
                    }
                    Some(State::AbilityList) => {
                        let m = self.member();
                        self.abilities_list_from_equip(seq, m);
                    }
                    Some(State::EquipPick(sec)) => self.enter_equip_pick(seq, sec as usize),
                    _ => self.enter_equip(seq),
                }
            } else if pad.trig != 0 {
                for id in [self.equip_ui.panel, self.equip_ui.holder] {
                    self.kill_opt(id);
                }
                for &s in &self.item_ui.scroll {
                    self.sprites.kill(s);
                }
                self.clear_info();
                if !matches!(p, Some(State::Items) | Some(State::Equip)) {
                    self.kill_opt(self.banner);
                }
            }
            return false;
        }
        if (pick != Pick::None || moved) && self.group_of(self.equip_ui.panel) != Some(PANEL[1]) {
            for (id, g) in [
                (self.equip_ui.panel, PANEL[1]),
                (self.equip_ui.holder, HOLDER[1]),
                (self.banner, BANNER[1]),
            ] {
                if let Some(i) = id
                    && self.sprites.alive(i)
                {
                    self.sprites.set(seq, i, g, None);
                }
            }
        }
        if self.substate == 0 && self.group_of(self.equip_ui.panel) == Some(PANEL[1]) {
            self.equip_help(seq);
            self.substate = 1;
        }
        // FUN_140356760.
        let cursor_slot = self.equip_cursor();
        let valid = match pick {
            Pick::Next | Pick::Previous => self.data.panels.members.len() > 1,
            Pick::Triangle => cursor_slot.is_some_and(|s| s.id != 0),
            _ => true,
        };
        if let Some(s) = pick_sound(pick, moved, valid) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            // FUN_140356380: the old picture closes first.
            self.picture_close(seq);
            if self.equip_ui.list.scroll != scroll {
                self.equip_fill(seq);
            }
            self.equip_mask();
            self.equip_help(seq);
        }
        let memo = (self.equip_ui.list.list.cursor, self.equip_ui.list.scroll);
        if valid {
            match pick {
                // FUN_140357e30, then the next / previous member.
                Pick::Next | Pick::Previous => {
                    self.equip_tidy();
                    let n = self.data.panels.members.len();
                    let m = self.member();
                    let next = if pick == Pick::Next {
                        (m + 1) % n
                    } else {
                        (m + n - 1) % n
                    };
                    self.memory.items_row = next;
                    self.memory.equip = (0, 0);
                    self.pending_tab_text[1] = Some(self.data.panels.members[next].name.clone());
                    self.hide_help(seq);
                    self.close_tab(seq, 1);
                    self.close_equip(seq);
                    self.clear_info();
                    self.picture_close(seq);
                    self.pending = Some(State::Equip);
                    self.substate = 2;
                }
                Pick::Triangle => {
                    let s = cursor_slot.unwrap();
                    let e = self.data.items.equipment.get(self.member());
                    let emptied = e.is_some_and(|e| {
                        e.items
                            .iter()
                            .zip(&e.originals)
                            .any(|(&cur, &auto)| cur != s.id && auto == s.id)
                    });
                    if s.alt && emptied {
                        self.equip_ui.reload_id = s.id;
                        let text = self.data.equip.texts.reload_box.clone();
                        let (no, yes) = (
                            self.data.items.texts.no.clone(),
                            self.data.items.texts.yes.clone(),
                        );
                        self.dim_glove(seq);
                        self.open_dialog(seq, text, vec![no, yes]);
                        self.state = State::EquipReload;
                        self.substate = 0;
                    } else {
                        self.equip_reload(s.id, s.alt);
                        self.equip_fill(seq);
                    }
                }
                Pick::Select => {
                    self.memory.equip = memo;
                    if let Some(c) = self.notice_for_slot() {
                        self.start_notices(seq, Some(c), State::Equip);
                    }
                }
                Pick::Square => {
                    self.memory.equip = memo;
                    self.equip_tidy();
                    self.hide_help(seq);
                    self.close_tab(seq, 0);
                    self.close_tab(seq, 1);
                    self.close_banner(seq);
                    self.close_equip(seq);
                    self.clear_info();
                    self.picture_close(seq);
                    self.pending = Some(State::AbilityList);
                    self.substate = 2;
                }
                Pick::Start => {
                    self.memory.equip = memo;
                    self.equip_tidy();
                    self.clear_info();
                    self.exit = true;
                }
                Pick::Cancel => {
                    self.memory.equip = memo;
                    self.equip_tidy();
                    self.hide_help(seq);
                    self.close_tab(seq, 1);
                    self.close_banner(seq);
                    self.close_equip(seq);
                    self.clear_info();
                    self.picture_close(seq);
                    self.pending = Some(State::Items);
                    self.substate = 2;
                }
                Pick::Row(r) => {
                    self.memory.equip = memo;
                    let s = self.equip_ui.slots[r];
                    self.pending_tab_text[2] = Some(self.section_label(&s));
                    self.equip_ui.slot = r;
                    self.close_banner(seq);
                    self.hide_help(seq);
                    self.close_equip(seq);
                    self.clear_info();
                    self.pending = Some(State::EquipPick(s.sec as u8));
                    self.substate = 2;
                }
                _ => {}
            }
        }
        if self.state == State::Equip && self.substate == 1 {
            self.picture_timers(seq);
        }
        if self.state == State::Equip && self.substate != 2 {
            let bubble = self.equip_cursor().is_some_and(|s| s.sec == 4);
            self.equip_frame(seq, FRAME, bubble);
        }
        moved
    }

    /// FUN_140355660 (state 3): turning auto-reload off empties the slots
    /// that only held the item by auto-reload.
    pub(super) fn equip_reload_box(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let (pick, moved) = self.item_ui.choices.input(pad);
        if self.substate == 2 {
            if !self.dialog_alive() {
                self.undim_last();
                self.state = State::Equip;
                self.substate = 1;
                self.pending = None;
            }
            return false;
        }
        self.dialog_intro(seq, pad.trig != 0);
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if matches!(pick, Pick::Row(_) | Pick::Cancel) {
            if pick == Pick::Row(1) {
                let id = self.equip_ui.reload_id;
                self.equip_reload(id, true);
                self.equip_fill(seq);
            }
            self.close_dialog(seq);
            self.substate = 2;
        }
        moved
    }

    /// FUN_140362200(0x10, member) from Equip's SQUARE.
    fn abilities_list_from_equip(&mut self, seq: &Sequence, member: usize) {
        self.kill_tab(0);
        self.kill_tab(1);
        self.memory.entry = 1;
        self.tab_text(0, self.entry_label(1));
        let name = self.data.panels.members[member].name.clone();
        self.tab_text(1, name);
        if self.memory.ability_char != member {
            self.memory.ability_list = (0, 0);
            self.memory.ability_char = member;
        }
        self.enter_ability_list_for(seq, member);
    }

    // ---- states 5–9 ----------------------------------------------------------

    /// FUN_14034c770(buf, member+1, filter): owned items of the section's
    /// types (the member's weapon type; Sora: keyblades), file order.
    fn pick_candidates(&self, sec: usize) -> Vec<(u16, u8)> {
        let character = self
            .data
            .items
            .equipment
            .get(self.member())
            .map_or(1, |e| e.character);
        let weapon = items::weapon_type(character);
        let wanted = |kind: u8| match sec {
            0 | 1 => kind == weapon,
            2 => kind == 14,
            3 => kind == 15,
            _ => kind == 0,
        };
        self.data
            .items
            .table
            .iter()
            .filter(|r| wanted(r.kind))
            .filter(|r| !(self.data.items.struggle_hidden && items::STRUGGLE.contains(&r.id)))
            .filter_map(|r| {
                let n = self.data.items.counts.get(&r.id).copied().unwrap_or(0);
                (n != 0).then_some((r.id, n))
            })
            .collect()
    }

    /// FUN_140356b00.
    fn enter_equip_pick(&mut self, seq: &Sequence, sec: usize) {
        self.state = State::EquipPick(sec as u8);
        self.substate = 0;
        self.equip_ui.sec = sec;
        self.create_tab(seq, 2);
        let slot = self.equip_ui.slots[self.equip_ui.slot];
        self.equip_ui.picks = self.pick_candidates(sec);
        let c = self.equip_ui.picks.len();
        let weapons = sec <= 1;
        let total = if weapons { c.max(1) } else { c + 1 };
        let panel = self.sprites.create(
            seq,
            PRIO_PANEL,
            ART,
            PICK_PANEL[sec][0],
            Some(PICK_PANEL[sec][2]),
            None,
        );
        self.closers.push((panel, PICK_PANEL[sec][1]));
        self.equip_ui.panel = Some(panel);
        let header = self
            .sprites
            .create(seq, PRIO_LABEL, ART, PICK_HEADER[sec], None, None);
        self.child(header, panel);
        let text = self.slot_text(&slot);
        let h = self
            .sprites
            .create(seq, PRIO_TEXT, ART, WHITE, None, Some(self.row_text(text)));
        self.child(h, header);
        if weapons
            && let Some(st) = self.item_stats(slot.id)
            && st.ability != 0
        {
            let hg = &seq.groups[PICK_HEADER[sec]];
            let at = [hg.light_position_x as f32, hg.text_position_y as f32];
            let size = hg.text_position_x as f32;
            let mut t = self.row_text(self.info(st.ability).name);
            t.align = 1;
            t.size = if size > 0. { size } else { 13. };
            let a = self
                .sprites
                .create(seq, PRIO_TEXT, ART, YELLOW, None, Some(t));
            self.child(a, header);
            self.sprites.get_mut(a).offset = at;
        }
        let rows_n = total.min(PICK_VISIBLE);
        let rg = &seq.groups[PICK_ROW[sec]];
        let pitch = rg.ui_padding as f32;
        let mut rows = Vec::new();
        self.equip_ui.pick_rows.clear();
        self.equip_ui.pick_boxes.clear();
        self.equip_ui.pick_badges.clear();
        let badge_at = [rg.light_position_x as f32, rg.text_position_y as f32];
        for i in 0..rows_n {
            let mut t = Self::kept_text(Vec::new(), TEXT_SIZE);
            t.group_position = false;
            let r = self
                .sprites
                .create(seq, PRIO_PANEL, ART, PICK_ROW[sec], None, Some(t));
            self.child(r, panel);
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            // FUN_140356280: the "×N" count right-aligned at the row
            // group's (LightPositionX, TextPositionY).
            let mut ct = Text::new(Vec::new(), TEXT_SIZE);
            ct.align = 1;
            ct.keep_rgb = false;
            self.sprites.get_mut(r).after = Some(ct);
            let nb = self.sprites.create(seq, PRIO_INFO, ART, BLANK, None, None);
            self.child(nb, r);
            self.sprites.get_mut(nb).offset = badge_at;
            self.equip_ui.pick_badges.push(nb);
            rows.push(r);
            if weapons {
                let bp = seq.groups[ABILITY_BOX[sec]].ui_padding as f32;
                let mut bt = Text::new(Vec::new(), 13.);
                bt.align = 2;
                bt.keep_rgb = false;
                let b = self
                    .sprites
                    .create(seq, PRIO_LABEL, ART, BLANK, None, Some(bt));
                self.child(b, panel);
                self.sprites.get_mut(b).offset = [0., bp * i as f32];
                self.equip_ui.pick_boxes.push(b);
            }
        }
        self.equip_ui.pick_rows = rows.clone();
        self.build_scrollbar(seq, panel, PRIO_LABEL, total, rows_n);
        let holder = self.sprites.create(
            seq,
            PRIO_PANEL,
            ART,
            PICK_HOLDER[sec][0],
            Some(PICK_HOLDER[sec][2]),
            None,
        );
        self.closers.push((holder, PICK_HOLDER[sec][1]));
        self.equip_ui.holder = Some(holder);
        self.picture.setup(PRIO_PICTURE, None);
        self.picture.set_parent(holder);
        self.picture.reset();
        // The cursor starts on the equipped item.
        let idx = self
            .equip_ui
            .picks
            .iter()
            .position(|&(id, _)| id == slot.id && slot.id != 0);
        let mut k = match idx {
            Some(i) if weapons => i,
            Some(i) => i + 1,
            None => 0,
        };
        let mut sc = 0;
        if k > 7 {
            sc = k - 7;
            k = 7;
        }
        if total != 0 && total <= sc + k {
            (k, sc) = (total - 1, (k + 1).saturating_sub(total));
        }
        let mut list = MenuList::new(rows, k);
        list.total = total;
        list.page = Some(rows_n);
        // SELECT: the Auto-Reload notice from the item pick list (0), the
        // Sub-Weapons one from the form pick list (3).
        let cat = match sec {
            4 => Some(0),
            1 => Some(3),
            _ => None,
        };
        if cat.is_some_and(|c| self.notice_available(c)) {
            list.mask |= 0x400;
        }
        self.equip_ui.pick = Grid {
            list,
            scroll: sc,
            cols: 1,
            lines: rows_n,
        };
        self.pick_fill(seq);
    }

    /// FUN_1403581a0: names, counts, weapon abilities; "None" for an empty
    /// weapon list; the unequip row first for armor, accessories and items.
    fn pick_fill(&mut self, seq: &Sequence) {
        let sec = self.equip_ui.sec;
        let weapons = sec <= 1;
        let sc = self.equip_ui.pick.scroll;
        let total = self.equip_ui.pick.list.total;
        let rows = self.equip_ui.pick_rows.clone();
        self.scroll_to(seq, sc, total, rows.len());
        for (i, &r) in rows.iter().enumerate() {
            self.set_text(r, Vec::new());
            if let Some(t) = self.sprites.get_mut(r).after.as_mut() {
                t.bytes.clear();
            }
            if let Some(&b) = self.equip_ui.pick_boxes.get(i) {
                self.sprites.set(seq, b, BLANK, None);
                self.set_text(b, Vec::new());
            }
            if let Some(&b) = self.equip_ui.pick_badges.get(i) {
                self.sprites.set(seq, b, BLANK, None);
            }
        }
        let picks = self.equip_ui.picks.clone();
        if weapons && picks.is_empty() {
            self.set_text(rows[0], self.data.equip.texts.none.clone());
            return;
        }
        for (i, &r) in rows.iter().enumerate() {
            let j = sc + i;
            if !weapons && j == 0 {
                let g = match sec {
                    2 => 7,
                    3 => 0x11,
                    _ => 0,
                };
                let mut t = vec![0x09, g];
                t.extend(self.data.items.texts.blank.clone());
                self.set_text(r, t);
                continue;
            }
            let e = if weapons { j } else { j - 1 };
            let Some(&(id, n)) = picks.get(e) else { break };
            let name = self.item_name(id, false);
            self.set_text(r, name);
            if let Some(t) = self.sprites.get_mut(r).after.as_mut() {
                t.bytes = items::count_text(n);
            }
            // A new item's badge, dim once seen (FUN_14036a8f0 / 7f0).
            if let (Some(&b), Some(seen)) = (self.equip_ui.pick_badges.get(i), self.item_badge(id))
            {
                self.sprites.set(seq, b, items::BADGE[seen as usize], None);
            }
            if weapons
                && let Some(st) = self.item_stats(id)
                && st.ability != 0
                && let Some(&b) = self.equip_ui.pick_boxes.get(i)
            {
                self.sprites.set(seq, b, ABILITY_BOX[sec], None);
                let name = self.info(st.ability).name;
                self.set_text(b, name);
            }
        }
    }

    /// The candidate under the pick cursor (None = the unequip row or an
    /// empty weapon list).
    fn pick_cursor(&self) -> Option<u16> {
        let a = self.equip_ui.pick.list.cursor + self.equip_ui.pick.scroll;
        let e = if self.equip_ui.sec <= 1 {
            a
        } else {
            a.checked_sub(1)?
        };
        self.equip_ui.picks.get(e).map(|p| p.0)
    }

    /// FUN_140358620 for the pick lists: help and the comparison (modes 2
    /// / 3).
    fn pick_help(&mut self, seq: &Sequence) {
        let sec = self.equip_ui.sec;
        let a = self.equip_ui.pick.list.cursor + self.equip_ui.pick.scroll;
        let cand = self.pick_cursor();
        let t = &self.data.equip.texts;
        let h = match (sec, cand) {
            (0 | 1, None) => t.no_items.clone(),
            (_, None) if a == 0 => t.unequip_help[(sec - 2).min(2)].clone(),
            (_, Some(id)) => self.info(id).description,
            _ => Vec::new(),
        };
        self.show_help(seq, h);
        let cur = self.equip_ui.slots[self.equip_ui.slot].id;
        self.info_compare(seq, cur, cand.unwrap_or(0));
        let picture = cand.map(|id| self.info(id).picture);
        self.picture_request(seq, picture);
    }

    /// FUN_1402fd410: rows of either item; current value, candidate value
    /// and the resulting total, coloured better (cyan) / worse (red).
    fn info_compare(&mut self, seq: &Sequence, cur: u16, cand: u16) {
        self.clear_info();
        let Some(holder) = self.equip_ui.holder else {
            return;
        };
        let weapons = self.equip_ui.sec <= 1;
        let none = ItemStats {
            resists: [100; 4],
            ..ItemStats::default()
        };
        let cs = self.item_stats(cur).unwrap_or(none);
        let ns = self.item_stats(cand).unwrap_or(none);
        let kind = |id: u16| self.info(id).kind;
        let mask = (if cur != 0 {
            row_mask(kind(cur), &cs)
        } else {
            0
        } | if cand != 0 {
            row_mask(kind(cand), &ns)
        } else {
            0
        }) & !0x100;
        let (groups, pad) = if weapons {
            (INFO_MODE3, 40.)
        } else {
            (INFO_MODE2, 20.)
        };
        let hg = &seq.groups[PICK_HOLDER[self.equip_ui.sec][2]];
        let base = [hg.light_position_x as f32, hg.text_position_y as f32];
        let totals = self
            .data
            .equip
            .totals
            .get(self.member())
            .copied()
            .unwrap_or_default();
        let colour = |better: bool, worse: bool| {
            if better {
                CYAN
            } else if worse {
                RED
            } else {
                WHITE
            }
        };
        let mut r = 0;
        for k in 0..8 {
            if mask >> k & 1 == 0 {
                continue;
            }
            let offset = [base[0], base[1] + pad * r as f32];
            let label = self.data.equip.texts.stat_labels[k].clone();
            let l = self.info_sprite(seq, PRIO_INFO, groups[0], holder, offset, label, 0);
            let b = self.info_sprite(seq, PRIO_INFO, groups[1], holder, offset, Vec::new(), 0);
            let (cv, nv) = (stat_value(&cs, k), stat_value(&ns, k));
            let old = total_value(&totals, k);
            let new = old - cv + nv;
            let cand_style = colour(nv > cv, nv < cv);
            let total_style = colour(new > old, new < old);
            let (cur_at, cand_at, total_at, total_parent) = if weapons {
                ([0., 0.], [54., 0.], [101., 0.], l)
            } else {
                ([110., 0.], [0., 0.], [51., 0.], b)
            };
            let cur_parent = if weapons { b } else { l };
            self.info_sprite(
                seq,
                PRIO_VALUE,
                WHITE,
                cur_parent,
                cur_at,
                number_text(k, cv),
                1,
            );
            self.info_sprite(
                seq,
                PRIO_VALUE,
                cand_style,
                b,
                cand_at,
                number_text(k, nv),
                1,
            );
            self.info_sprite(
                seq,
                PRIO_VALUE,
                total_style,
                total_parent,
                total_at,
                number_text(k, new),
                1,
            );
            r += 1;
        }
    }

    /// FUN_140354ea0: the old item back to stock, the new one out of it.
    fn equip_item(&mut self, new: u16) {
        let slot = self.equip_ui.slots[self.equip_ui.slot];
        if slot.id == new {
            return;
        }
        let pick_count = self
            .equip_ui
            .picks
            .iter()
            .find(|p| p.0 == new)
            .map_or(0, |p| p.1);
        let m = self.member();
        let mut back = Vec::new();
        let mut out = Vec::new();
        if let Some(e) = self.data.items.equipment.get_mut(m) {
            let place: &mut u16 = match slot.sec {
                0 => &mut e.weapon,
                1 => &mut e.forms[slot.j],
                2 => &mut e.armor[slot.j],
                3 => &mut e.accessories[slot.j],
                _ => &mut e.items[slot.j],
            };
            let old = *place;
            if old != 0 {
                back.push(old);
            }
            if slot.sec == 4 {
                e.originals.resize(e.items.len(), 0);
                if new == 0 || pick_count != 0 {
                    e.items[slot.j] = new;
                    e.originals[slot.j] = 0;
                    if new != 0 {
                        out.push(new);
                    }
                } else {
                    e.originals[slot.j] = new;
                    e.items[slot.j] = 0;
                }
            } else {
                *place = new;
                if new != 0 {
                    out.push(new);
                }
            }
        }
        for id in back {
            let n = self.data.items.counts.entry(id).or_insert(0);
            if *n < 99 {
                *n += 1;
            }
        }
        for id in out {
            if let Some(n) = self.data.items.counts.get_mut(&id) {
                *n = n.saturating_sub(1);
            }
        }
        self.refresh_totals();
    }

    /// FUN_1402fc760 after an equip: the member's totals and weapon ability
    /// follow the equipment (FUN_1403c1d90).
    fn refresh_totals(&mut self) {
        self.refresh_member_totals(self.member());
    }

    /// The same for member `m` (the camp's opening build of a party
    /// member's totals, FUN_140354340).
    pub(super) fn refresh_member_totals(&mut self, m: usize) {
        let Some(base) = self.data.equip.bases.get(m).copied() else {
            return;
        };
        let Some(e) = self.data.items.equipment.get(m) else {
            return;
        };
        let form = self.data.equip.form_weapons.get(m).copied();
        let (t, ability) = gear_totals(
            &self.data.items.table,
            &self.data.equip.stats,
            base,
            e,
            form,
        );
        if let Some(slot) = self.data.equip.totals.get_mut(m) {
            *slot = t;
        }
        let form_abilities: Vec<u16> = self
            .data
            .items
            .equipment
            .get(m)
            .map(|e| e.forms.clone())
            .unwrap_or_default()
            .iter()
            .map(|&id| self.item_stats(id).map_or(0, |s| s.ability))
            .collect();
        if let Some(a) = self.data.abilities.members.get_mut(m) {
            a.ap_max = t.ap;
            a.weapon_ability = ability;
            a.form_weapon_abilities = form_abilities;
        }
    }

    /// FUN_140355410 + FUN_1403a1610 (leaving a pick list): auto-reload
    /// follows the items now in slots; emptied auto-reload slots refill
    /// from stock.
    fn equip_after_pick(&mut self) {
        let m = self.member();
        let Some(e) = self.data.items.equipment.get_mut(m) else {
            return;
        };
        e.originals.resize(e.items.len(), 0);
        let autos: Vec<u16> = e.originals.iter().copied().filter(|&a| a != 0).collect();
        for j in 0..e.items.len() {
            if autos.contains(&e.items[j]) {
                e.originals[j] = e.items[j];
            }
        }
        for j in 0..e.items.len() {
            let auto = e.originals[j];
            if auto != 0 && e.items[j] == 0 {
                let n = self.data.items.counts.entry(auto).or_insert(0);
                if *n > 0 {
                    *n -= 1;
                    e.items[j] = auto;
                }
            }
        }
    }

    /// FUN_140356760 for the pick lists: the item going back must fit in
    /// stock (≤ 98); weapons need a candidate.
    fn pick_valid(&self, r: usize) -> bool {
        let sec = self.equip_ui.sec;
        let cur = self.equip_ui.slots[self.equip_ui.slot].id;
        let room = |id: u16| self.data.items.counts.get(&id).copied().unwrap_or(0) <= 98;
        if sec <= 1 {
            !self.equip_ui.picks.is_empty() && r < self.equip_ui.picks.len() && room(cur)
        } else {
            cur == 0 || room(cur)
        }
    }

    /// FUN_140359200: an accessory change must leave AP for the equipped
    /// abilities.
    fn accessory_blocks(&self, new: u16) -> bool {
        let cur = self.equip_ui.slots[self.equip_ui.slot].id;
        if cur == 0 {
            return false;
        }
        let m = self.member();
        let used = self.data.abilities.members.get(m).map_or(0, |a| {
            abilities::used(&abilities::entries(&self.data.items.table, a))
        });
        let max = self.data.abilities.members.get(m).map_or(0, |a| a.ap_max);
        let ap = |id: u16| self.item_stats(id).map_or(0, |s| s.ap as i32);
        let new_ap = if new == 0 { 0 } else { ap(new) };
        new_ap - ap(cur) - used + max < 0
    }

    /// FUN_140355770.
    pub(super) fn equip_pick(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let sec = self.equip_ui.sec;
        let scroll = self.equip_ui.pick.scroll;
        let (pick, moved) = self.equip_ui.pick.input(pad);
        if self.substate == 2 {
            if !self.equip_busy() && !self.tab_alive(2) {
                self.clear_info();
                self.kill_tab(2);
                self.equip_after_pick();
                self.pending = None;
                self.equip_ui.slots = self.equip_slots();
                self.enter_equip(seq);
            } else if pad.trig != 0 {
                for id in [self.equip_ui.panel, self.equip_ui.holder] {
                    self.kill_opt(id);
                }
                for &s in &self.item_ui.scroll {
                    self.sprites.kill(s);
                }
                self.clear_info();
            }
            return false;
        }
        if (pick != Pick::None || moved)
            && self.group_of(self.equip_ui.panel) != Some(PICK_PANEL[sec][2])
        {
            for (id, g) in [
                (self.equip_ui.panel, PICK_PANEL[sec][2]),
                (self.equip_ui.holder, PICK_HOLDER[sec][2]),
            ] {
                if let Some(i) = id
                    && self.sprites.alive(i)
                {
                    self.sprites.set(seq, i, g, None);
                }
            }
        }
        if self.substate == 0 && self.group_of(self.equip_ui.panel) == Some(PICK_PANEL[sec][2]) {
            self.pick_help(seq);
            self.substate = 1;
        }
        let valid = match pick {
            Pick::Row(r) => self.pick_valid(r),
            _ => true,
        };
        if let Some(s) = pick_sound(pick, moved, valid) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            self.picture_close(seq);
            // FUN_140358ba0: the new item under the cursor is seen.
            if let Some(id) = self.pick_cursor()
                && self.item_badge(id) == Some(false)
            {
                self.data.items.seen.push(id);
            }
            self.pick_fill(seq);
            let _ = scroll;
            self.pick_help(seq);
        }
        let mut leave = false;
        if valid {
            match pick {
                Pick::Start => {
                    self.equip_after_pick();
                    self.equip_tidy();
                    self.clear_info();
                    self.exit = true;
                }
                Pick::Cancel => leave = true,
                Pick::Select => {
                    let cat = if sec == 4 { 0 } else { 3 };
                    self.start_notices(seq, Some(cat), State::EquipPick(sec as u8));
                }
                Pick::Row(r) => {
                    let new = if sec <= 1 {
                        self.equip_ui.picks[r].0
                    } else if r == 0 {
                        0
                    } else {
                        self.equip_ui.picks[r - 1].0
                    };
                    if sec == 3 && self.accessory_blocks(new) {
                        let text = self.data.equip.texts.accessory_box.clone();
                        let ok = self.data.items.texts.ok.clone();
                        self.dim_glove(seq);
                        self.open_dialog(seq, text, vec![ok]);
                        self.state = State::EquipApBox;
                        self.substate = 0;
                    } else {
                        self.equip_item(new);
                        leave = true;
                    }
                }
                _ => {}
            }
        }
        if leave {
            self.hide_help(seq);
            self.close_tab(seq, 2);
            self.close_opt(seq, self.equip_ui.panel, PICK_PANEL[sec][1]);
            self.close_opt(seq, self.equip_ui.holder, PICK_HOLDER[sec][1]);
            self.close_scrollbar(seq);
            let f = self.equip_ui.frame.take();
            self.kill_opt(f);
            self.clear_info();
            self.picture_close(seq);
            self.substate = 2;
        }
        if matches!(self.state, State::EquipPick(_)) && self.substate == 1 {
            self.picture_timers(seq);
        }
        if matches!(self.state, State::EquipPick(_)) && self.substate != 2 {
            self.equip_frame(seq, PICK_FRAME[sec], false);
        }
        moved
    }

    /// FUN_140355570 (state 4): the accessory OK box; back to the list.
    pub(super) fn equip_ap_box(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let (pick, moved) = self.item_ui.choices.input(pad);
        if self.substate == 2 {
            if !self.dialog_alive() {
                self.undim_last();
                self.state = State::EquipPick(self.equip_ui.sec as u8);
                self.substate = 1;
            }
            return false;
        }
        self.dialog_intro(seq, pad.trig != 0);
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if matches!(pick, Pick::Row(0) | Pick::Cancel) {
            self.close_dialog(seq);
            self.substate = 2;
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_rows_follow_the_item_kind() {
        let kk = ItemStats {
            id: 0x50,
            ability: 0x21e,
            strength: 3,
            magic: 1,
            resists: [100; 4],
            ..ItemStats::default()
        };
        // A keyblade: Strength, Magic and its ability.
        assert_eq!(row_mask(2, &kk), 0b1_0000_0110);
        let armor = ItemStats {
            defense: 1,
            resists: [100, 80, 100, 100],
            ..ItemStats::default()
        };
        assert_eq!(row_mask(14, &armor), 0xf8);
        assert_eq!(stat_value(&armor, 5), 20);
        assert_eq!(number_text(5, 20), encode_number("20%"));
    }
}
