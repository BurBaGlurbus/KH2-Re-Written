//! The shop's screens on `menu/<lang>/itemshop.2ld` (FUN_14029b600's mode):
//! the top rows (FUN_14029d960), the goods lists (FUN_14029c860), the quantity
//! panel (FUN_14029d440), Yes or No (FUN_14029cfa0) and the first-visit notice.
//! Every sprite uses SEQD 2 on image 1 (`DAT_14073fb3c`, `DAT_14073fb38`).
//!
//! There are no categories: Buy is one list, Sell is one list. Synthesis, which
//! branches from the top rows' third row, isn't built, so that row closes the
//! screen's way back to the top.
use super::picture::{Picture, PictureCache};
use super::{
    MenuList, Pad, Pick, SOUND_CLOSE, SOUND_CURSOR, SOUND_DECIDE, SOUND_INVALID, SOUND_OPEN, bits,
    encode_number,
};
use crate::field_command_menu::Font;
use crate::sprite_player::{self, Id, LightCur, Sprites, Text};
use data::menu_layout::{Layout, LayoutTask};
use data::sprite_sequence::{Modifiers, Player, Quad, Sequence};
use std::collections::BTreeMap;

use super::equip::ItemStats;
use super::items::{Equipment, ItemInfo};
use player::shop as rules;

pub const SEQUENCE: usize = 2;
pub const IMAGE: usize = 1;
/// FUN_140194f20 numbers groups across the LAYD: SEQD 2 starts here.
const FLAT: usize = 6;

const TEXT_SIZE: f32 = 15.;

/// se000: a plain shop opens, one with synthesis (FUN_1402a0440), a trade's
/// done (FUN_14029cfa0) and Yes's own decide (sound mode 4).
const SOUND_OPEN_SYNTHESIS: u32 = 40;
const SOUND_TRADE: u32 = 38;
const SOUND_CONFIRM: u32 = 0;

const PRIO_FRAME: u32 = 0x7537;
const PRIO_PANEL: u32 = 0x753b;
const PRIO_ROW: u32 = 0x753c;
const PRIO_INFO: u32 = 0x753d;
const PRIO_VALUE: u32 = 0x753e;
const PRIO_PICTURE: u32 = 0x753f;
const PRIO_NOTICE: u32 = 0x7546;
const PRIO_CHOICE: u32 = 0x7549;
const PRIO_LIGHT: u32 = 0x754a;
const PRIO_FACE: u32 = 0x754b;

/// A layout page (FUN_14029be10): scene in, out and idle; help box prop; help
/// text in and out; title bar prop.
struct Page {
    scene: [usize; 3],
    help_box: [usize; 3],
    help: [usize; 2],
    title: [usize; 3],
}

const PAGES: [Page; 3] = [
    Page {
        scene: [15, 17, 16],
        help_box: [197, 199, 198],
        help: [177, 178],
        title: [207, 209, 208],
    },
    Page {
        scene: [4, 6, 5],
        help_box: [214, 216, 215],
        help: [177, 178],
        title: [223, 225, 224],
    },
    Page {
        scene: [12, 14, 13],
        help_box: [241, 243, 242],
        help: [179, 180],
        title: [249, 251, 250],
    },
];

/// A row's groups: in, out, idle, selected.
const TOP_ROWS: [usize; 4] = [194, 196, 195, 193];
const BUY_ROWS: [usize; 4] = [218, 220, 219, 217];
const SELL_ROWS: [usize; 4] = [245, 247, 246, 244];
const CONFIRM_ROWS: [usize; 4] = [237, 239, 238, 236];
const CHOICE_ROWS: [usize; 4] = [4, 6, 5, 3];
const FRAME: [usize; 3] = [190, 191, 192];
const HOLDER: [usize; 3] = [38, 39, 40];
const MONEY: [usize; 3] = [168, 169, 170];
/// Member and Stock columns.
const COLUMN: [usize; 3] = [164, 165, 166];
const STOCK: [usize; 3] = [161, 162, 163];
const STOCK_LABEL: usize = 188;
const NOTICE: [usize; 3] = [7, 8, 9];
const PANEL: [usize; 3] = [229, 230, 231];
const PROMPT: [usize; 3] = [233, 234, 235];
const ARROWS: usize = 232;
const GLOVE: usize = 27;
const GLOVE_DIM: usize = 29;
const LIGHT: [usize; 3] = [32, 34, 33];
const INFO_LABEL: usize = 37;
const INFO_VALUE: usize = 188;
const INFO_ABILITY: usize = 189;
const BLANK: usize = 36;
const BADGE_NEW: usize = 121;
const BADGE_SEEN: usize = 120;
/// FUN_1402fb450(obj, 0x24, 0x7a, ...): the money digits.
const DIGIT: usize = 0x7a;
/// The party's faces: SEQD 0's group 2.
const FACE_SEQUENCE: usize = 0;
const FACE_GROUP: usize = 2;
/// The goods list's scrollbar (FUN_1402fa930's table by part: in, idle, out):
/// the track, and on it the thumb's top, middle and bottom.
const SCROLL: [[usize; 3]; 4] = [
    [149, 150, 151],
    [155, 156, 157],
    [152, 153, 154],
    [158, 159, 160],
];
/// Rows the goods list shows.
const SHOWN: usize = 8;
/// FUN_1402a17f0: the model's motion and its turn; most stand 80 down the
/// screen, the three ducks 40 up (the slot's +0x844 and +0x854).
const MODEL_MOTION: u16 = 0;
const MODEL_TURN: f32 = 20.;
const MODEL_DROP: f32 = 80.;
const MODEL_DROP_DUCKS: f32 = -40.;
const DUCKS: [u16; 3] = [0x501, 0x502, 0x503];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Top,
    Goods { buying: bool },
    Quantity { buying: bool },
    Confirm { buying: bool },
    Notice,
}

#[derive(Clone, Debug, Default)]
pub struct Texts {
    /// Buy, Sell, Synthesize, Quit, and their help.
    pub rows: [Vec<u8>; 4],
    pub helps: [Vec<u8>; 4],
    /// 0x8ad6, put after an amount.
    pub munny: Vec<u8>,
    /// 0xc88f.
    pub none: Vec<u8>,
    /// 0x8ad4.
    pub stock: Vec<u8>,
    /// 0x8ad7, 0x8ad8.
    pub how_many: [Vec<u8>; 2],
    /// 0x8ad9, 0x8adb.
    pub sure: [Vec<u8>; 2],
    pub no: Vec<u8>,
    pub yes: Vec<u8>,
    pub ok: Vec<u8>,
    /// 0xc401, 0xc402.
    pub notice: [Vec<u8>; 2],
    /// 0x8ad5.
    pub total: Vec<u8>,
    /// FUN_1402fd780's labels (0 AP .. 3 Def, 4..8 resists, 8 ability).
    pub stats: [Vec<u8>; 9],
}

/// A party member's column.
#[derive(Clone, Debug, Default)]
pub struct Member {
    pub equipment: Equipment,
    /// FUN_1402fc960: Sora's forms count their weapons.
    pub sora: bool,
}

/// What the screen reads and changes.
#[derive(Clone, Debug, Default)]
pub struct Data {
    pub name: Vec<u8>,
    pub texts: Texts,
    pub table: Vec<ItemInfo>,
    pub stats: Vec<ItemStats>,
    /// Item +0xc by id.
    pub prices: BTreeMap<u16, u16>,
    pub goods: Vec<u16>,
    pub buy_sell: bool,
    pub synthesize: bool,
    /// FUN_1402fba40's byte +0xd.
    pub own_goods: bool,
    pub stock: BTreeMap<u16, u8>,
    pub munny: u32,
    /// `save+0x49e0` and the watch list that indexes it.
    pub seen: [u8; 0x10],
    pub watch: Vec<u16>,
    /// `save+0x49f0`.
    pub notices: u8,
    pub members: Vec<Member>,
    pub picture_sequence: Option<Sequence>,
    /// The shopkeeper: objentry id, projection offset and depth (row +6..+0xc).
    pub model: (u16, [i16; 2], i16),
}

impl Data {
    fn info(&self, id: u16) -> Option<&ItemInfo> {
        self.table.iter().find(|r| r.id == id)
    }

    fn stock_of(&self, id: u16) -> u32 {
        u32::from(self.stock.get(&id).copied().unwrap_or(0))
    }

    fn seen_bit(&self, id: u16) -> bool {
        self.watch
            .iter()
            .position(|&w| w == id)
            .is_some_and(|i| self.seen[i / 8] >> (i % 8) & 1 != 0)
    }

    fn mark_seen(&mut self, id: u16) {
        if let Some(i) = self.watch.iter().position(|&w| w == id)
            && let Some(b) = self.seen.get_mut(i / 8)
        {
            *b |= 1 << (i % 8);
        }
    }

    /// FUN_14029f160: the item table in file order, what can be sold of it.
    fn sell_list(&self) -> Vec<u16> {
        self.table
            .iter()
            .filter(|r| r.kind < 0x12 && r.sell != 0 && self.stock_of(r.id) != 0)
            .map(|r| r.id)
            .collect()
    }

    /// FUN_1402a1dc0: how many of a good the member holds.
    fn held(&self, member: &Member, id: u16) -> u32 {
        let e = &member.equipment;
        let mut n = 0;
        for v in [e.weapon, e.secondary_weapon]
            .into_iter()
            .chain(e.armor.iter().copied())
            .chain(e.accessories.iter().copied())
            .chain(e.items.iter().copied())
        {
            n += u32::from(v == id);
        }
        if member.sora {
            n += e.forms.iter().filter(|&&f| f == id).count() as u32;
        }
        n
    }
}

/// A trade in hand (`DAT_140b0d458`'s +2 .. +8).
#[derive(Clone, Copy, Debug, Default)]
struct Trade {
    id: u16,
    quantity: u32,
    price: u32,
}

pub struct Shop {
    pub data: Data,
    layout: LayoutTask,
    sprites: Sprites,
    pub state: State,
    substate: u8,
    next: Option<State>,
    pub exit: bool,
    page: usize,
    help: Option<Id>,
    /// The enabled top rows, by row index of the table.
    enabled: Vec<usize>,
    top: MenuList,
    top_cursor: usize,
    badges: Vec<Id>,
    goods: MenuList,
    /// The list's goods and the first shown (it scrolls circularly).
    list: Vec<u16>,
    scroll: usize,
    came_back: bool,
    scrollbar: Vec<Id>,
    /// The goods new at this visit (`DAT_140b0d468`).
    fresh: Vec<u16>,
    choice: MenuList,
    glove: Option<Id>,
    dim: Option<Id>,
    light: LightCur,
    frame_sprite: Option<Id>,
    holder: Option<Id>,
    info: Vec<Id>,
    columns: Vec<Id>,
    stock_label: Option<Id>,
    money: Option<Id>,
    money_digits: Vec<Player>,
    panel: Option<Id>,
    cells: Vec<Id>,
    prompt: Option<Id>,
    arrows: Option<Id>,
    notice: Option<Id>,
    trade: Trade,
    /// The host's load of the shopkeeper, and its clock.
    pub wanted_model: Option<(u16, u16)>,
    pub model_ready: Option<(u16, u16)>,
    model_ticks: u32,
    pub picture: Picture,
    pub pictures: PictureCache,
    picture_holder: Option<Id>,
    faces: Vec<Player>,
    empty: bool,
}

fn group_at(seq: &Sequence, g: usize) -> [f32; 2] {
    [
        seq.groups[g].light_position_x as f32,
        seq.groups[g].text_position_y as f32,
    ]
}

fn kept(bytes: Vec<u8>) -> Text {
    let mut t = Text::new(bytes, TEXT_SIZE);
    t.group_position = false;
    t
}

fn number(v: u32, suffix: &[u8]) -> Vec<u8> {
    let mut out = encode_number(&v.to_string());
    out.extend_from_slice(suffix);
    out
}

impl Shop {
    /// FUN_1402a09d0: the first screen. `(shop, sounds)`.
    pub fn open(layout: &Layout, data: Data) -> (Self, Vec<u32>) {
        let seq = &layout.sequences[SEQUENCE];
        let sound = if data.synthesize {
            SOUND_OPEN_SYNTHESIS
        } else {
            SOUND_OPEN
        };
        let faces = (0..data.members.len())
            .map(|_| Player::new(&layout.sequences[FACE_SEQUENCE], FACE_GROUP))
            .collect();
        let mut shop = Self {
            data,
            layout: LayoutTask::new(layout, PAGES[0].scene[0]),
            sprites: Sprites::default(),
            state: State::Top,
            substate: 0,
            next: None,
            exit: false,
            page: 0,
            help: None,
            enabled: Vec::new(),
            top: MenuList::new(Vec::new(), 0),
            top_cursor: 0,
            badges: Vec::new(),
            goods: MenuList::new(Vec::new(), 0),
            list: Vec::new(),
            scroll: 0,
            came_back: false,
            scrollbar: Vec::new(),
            fresh: Vec::new(),
            choice: MenuList::new(Vec::new(), 0),
            glove: None,
            dim: None,
            light: LightCur::default(),
            frame_sprite: None,
            holder: None,
            info: Vec::new(),
            columns: Vec::new(),
            stock_label: None,
            money: None,
            money_digits: Vec::new(),
            panel: None,
            cells: Vec::new(),
            prompt: None,
            arrows: None,
            notice: None,
            trade: Trade::default(),
            wanted_model: None,
            model_ready: None,
            model_ticks: 0,
            picture: Picture::default(),
            pictures: PictureCache::default(),
            picture_holder: None,
            faces,
            empty: false,
        };
        shop.layout
            .set_scene(PAGES[0].scene[0], Some(PAGES[0].scene[2]));
        shop.picture.setup(PRIO_PICTURE, None);
        shop.enter_top(seq);
        shop.open_notice(seq);
        (shop, vec![sound])
    }

    /// The screen is finished (FUN_14029bfe0 returns 1).
    pub fn done(&self) -> bool {
        self.exit
            && self.layout.dead
            && self.frame_sprite.is_none_or(|f| !self.sprites.alive(f))
            && self.top.rows.iter().all(|&r| !self.sprites.alive(r))
            && self.goods.rows.iter().all(|&r| !self.sprites.alive(r))
            && self.columns.iter().all(|&c| !self.sprites.alive(c))
            && self.panel.is_none_or(|p| !self.sprites.alive(p))
            && self.holder.is_none_or(|h| !self.sprites.alive(h))
    }

    fn create(
        &mut self,
        seq: &Sequence,
        prio: u32,
        group: usize,
        queued: Option<usize>,
        text: Option<Text>,
    ) -> Id {
        self.sprites.create(seq, prio, IMAGE, group, queued, text)
    }

    fn child(&mut self, id: Id, parent: Id) {
        self.sprites.attach(id, parent);
        self.sprites.get_mut(id).die_with_parent = true;
    }

    fn close(&mut self, seq: &Sequence, id: Option<Id>, group: usize) {
        if let Some(i) = id
            && self.sprites.alive(i)
        {
            self.sprites.close(seq, i, group);
        }
    }

    fn kill(&mut self, id: Option<Id>) {
        if let Some(i) = id
            && self.sprites.alive(i)
        {
            self.sprites.kill(i);
        }
    }

    fn rows_alive(&self, list: &MenuList) -> bool {
        list.rows.iter().any(|&r| self.sprites.alive(r))
    }

    fn close_rows(&mut self, seq: &Sequence, rows: &[Id], out: usize) {
        for &r in rows {
            if self.sprites.alive(r) {
                self.sprites.close(seq, r, out);
            }
        }
    }

    fn kill_badges(&mut self) {
        for b in std::mem::take(&mut self.badges) {
            self.sprites.kill(b);
        }
    }

    // ---- Help ------------------------------------------------------------

    /// FUN_1402fa5b0 and FUN_1402fa1d0.
    fn show_help(&mut self, seq: &Sequence, bytes: Vec<u8>) {
        let group = PAGES[self.page].help[0];
        if let Some(h) = self.help
            && self.sprites.alive(h)
        {
            if let Some(t) = self.sprites.get_mut(h).text.as_mut() {
                t.bytes = bytes;
            }
            if self.sprites.get(h).player.group != group {
                self.sprites.set(seq, h, group, None);
            }
            return;
        }
        let mut t = Text::new(bytes, TEXT_SIZE);
        t.keep_rgb = false;
        let h = self.create(seq, PRIO_ROW, group, None, Some(t));
        self.help = Some(h);
    }

    /// FUN_1402fa720.
    fn hide_help(&mut self, seq: &Sequence) {
        let group = PAGES[self.page].help[1];
        self.close(seq, self.help, group);
    }

    /// The help hangs off its page's box, a prop of the live scene.
    fn place_help(&mut self, layout: &Layout) {
        let Some(h) = self.help.filter(|&h| self.sprites.alive(h)) else {
            return;
        };
        let props = PAGES[self.page].help_box;
        let found = props
            .iter()
            .find_map(|&g| crate::world_map_menu::anchor(layout, &self.layout, g + FLAT));
        let Some(a) = found else { return };
        // (The buy and sell boxes' own offsets are their scrollbars'.)
        let s = self.sprites.get_mut(h);
        s.base = a.at;
        s.multiplier = [1., 1., 1., a.color[3]];
    }

    // ---- Top -------------------------------------------------------------

    /// FUN_1402a2660: the first enabled row shows whether goods are new.
    fn set_badges(&mut self, seq: &Sequence) {
        self.kill_badges();
        let unseen = self.fresh.iter().any(|&id| !self.data.seen_bit(id));
        if self.fresh.is_empty() {
            return;
        }
        let Some(&row) = self.top.rows.first() else {
            return;
        };
        if !self.data.buy_sell || self.data.goods.is_empty() {
            return;
        }
        let group = if unseen { BADGE_NEW } else { BADGE_SEEN };
        let b = self.create(seq, PRIO_ROW + 2, group, None, None);
        self.child(b, row);
        let at = group_at(seq, TOP_ROWS[0]);
        self.sprites.get_mut(b).offset = at;
        self.badges.push(b);
    }

    /// FUN_1402a17f0.
    fn enter_top(&mut self, seq: &Sequence) {
        self.state = State::Top;
        self.substate = 0;
        self.fresh = self
            .data
            .goods
            .iter()
            .copied()
            .filter(|&id| !self.data.seen_bit(id))
            .collect();
        self.scroll = 0;
        self.goods.cursor = 0;
        self.came_back = false;
        self.picture_holder = None;
        let has_goods = !self.data.goods.is_empty();
        self.enabled = (0..4)
            .filter(|&k| match k {
                0 | 1 => self.data.buy_sell && has_goods,
                2 => self.data.synthesize,
                _ => true,
            })
            .collect();
        let pitch = seq.groups[TOP_ROWS[0]].text_position_x as f32;
        let cursor = self.top_cursor.min(self.enabled.len().saturating_sub(1));
        let mut rows = Vec::new();
        for (i, &k) in self.enabled.clone().iter().enumerate() {
            let label = self.data.texts.rows[k].clone();
            let queued = if i == cursor {
                TOP_ROWS[3]
            } else {
                TOP_ROWS[2]
            };
            let r = self.create(seq, PRIO_ROW, TOP_ROWS[0], Some(queued), Some(kept(label)));
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            rows.push(r);
        }
        self.top = MenuList::new(rows, cursor);
        let f = self.create(seq, PRIO_FRAME, FRAME[0], Some(FRAME[1]), None);
        self.frame_sprite = Some(f);
        self.wanted_model = Some((self.data.model.0, MODEL_MOTION));
        self.set_badges(seq);
        self.top_help(seq);
    }

    fn top_help(&mut self, seq: &Sequence) {
        let k = self.enabled.get(self.top.cursor).copied().unwrap_or(3);
        let text = self.data.texts.helps[k].clone();
        self.show_help(seq, text);
    }

    /// FUN_1402a09d0's notice check (state 7).
    fn open_notice(&mut self, seq: &Sequence) {
        let moogle = self.data.own_goods;
        if !moogle && self.data.synthesize {
            return;
        }
        let bit = u8::from(moogle);
        if self.data.notices >> bit & 1 != 0 {
            return;
        }
        let mut t = kept(self.data.texts.notice[usize::from(moogle)].clone());
        t.align = 2;
        let n = self.create(seq, PRIO_NOTICE, NOTICE[0], Some(NOTICE[1]), Some(t));
        self.notice = Some(n);
        let mut label = kept(self.data.texts.ok.clone());
        label.align = 2;
        label.offset = [seq.groups[CHOICE_ROWS[0]].text_position_x as f32, 0.];
        let r = self.create(
            seq,
            PRIO_CHOICE,
            CHOICE_ROWS[0],
            Some(CHOICE_ROWS[3]),
            Some(label),
        );
        self.child(r, n);
        let mut list = MenuList::new(vec![r], 0);
        // FUN_1402f8d50(list, 0x60): only decide.
        list.mask = 0x10;
        self.choice = list;
        self.state = State::Notice;
        self.substate = 0;
    }

    fn notice_tick(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) {
        let moogle = self.data.own_goods;
        match self.substate {
            0 => {
                if self
                    .notice
                    .is_some_and(|n| self.sprites.get(n).player.group == NOTICE[1])
                {
                    self.substate = 1;
                }
            }
            1 => {
                let (pick, _) = self.choice.input(pad);
                if matches!(pick, Pick::Row(_)) {
                    sounds.push(SOUND_DECIDE);
                    self.close(seq, self.notice, NOTICE[2]);
                    let rows = self.choice.rows.clone();
                    self.close_rows(seq, &rows, CHOICE_ROWS[1]);
                    self.substate = 2;
                }
            }
            _ => {
                let gone = self.notice.is_none_or(|n| !self.sprites.alive(n));
                if gone && !self.rows_alive(&self.choice) {
                    self.data.notices |= 1 << u8::from(moogle);
                    self.notice = None;
                    self.choice = MenuList::new(Vec::new(), 0);
                    self.state = State::Top;
                    self.substate = 1;
                }
            }
        }
    }

    /// FUN_14029d960.
    fn top_tick(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        if self.substate == 2 {
            let dead = !self.rows_alive(&self.top)
                && self.frame_sprite.is_none_or(|f| !self.sprites.alive(f));
            if dead {
                self.kill_badges();
                if let Some(State::Goods { buying }) = self.next.take() {
                    self.enter_goods(seq, buying);
                }
            } else if pad.trig != 0 {
                let rows = self.top.rows.clone();
                for r in rows {
                    self.sprites.kill(r);
                }
                self.kill(self.frame_sprite);
            }
            return false;
        }
        if self.substate == 0
            && self
                .top
                .rows
                .first()
                .is_none_or(|&r| self.sprites.get(r).player.group != TOP_ROWS[0])
        {
            self.substate = 1;
        }
        let (pick, moved) = self.top.input(pad);
        if moved {
            sounds.push(SOUND_CURSOR);
            self.top_help(seq);
            return true;
        }
        match pick {
            Pick::Row(i) => {
                let k = self.enabled.get(i).copied().unwrap_or(3);
                self.top_cursor = i;
                match k {
                    0 | 1 => {
                        sounds.push(SOUND_DECIDE);
                        self.close(seq, self.frame_sprite, FRAME[2]);
                        let rows = self.top.rows.clone();
                        self.close_rows(seq, &rows, TOP_ROWS[1]);
                        self.hide_help(seq);
                        self.substate = 2;
                        self.next = Some(State::Goods { buying: k == 0 });
                    }
                    // Synthesis isn't built: the row refuses.
                    2 => sounds.push(SOUND_INVALID),
                    _ => {
                        sounds.push(SOUND_DECIDE);
                        self.start_exit(seq);
                    }
                }
            }
            Pick::Cancel | Pick::Start => {
                sounds.push(SOUND_CLOSE);
                self.start_exit(seq);
            }
            _ => {}
        }
        false
    }

    // ---- Goods -----------------------------------------------------------

    fn rows_for(buying: bool) -> [usize; 4] {
        if buying { BUY_ROWS } else { SELL_ROWS }
    }

    /// FUN_1402a0d30.
    fn enter_goods(&mut self, seq: &Sequence, buying: bool) {
        let back = self.came_back;
        self.state = State::Goods { buying };
        self.substate = 0;
        self.list = if buying {
            self.data.goods.clone()
        } else {
            self.data.sell_list()
        };
        let groups = Self::rows_for(buying);
        let count = self.list.len();
        let shown = count.clamp(1, SHOWN);
        let pitch = seq.groups[groups[0]].ui_padding as f32;
        // The cursor comes back clamped to what's left.
        let mut cursor = self.goods.cursor;
        if self.scroll == 0 {
            cursor = cursor.min(count.saturating_sub(1));
        } else if count <= self.scroll + SHOWN {
            let top = count as i32 - SHOWN as i32;
            if top < 0 {
                cursor += self.scroll;
                self.scroll = 0;
            } else {
                self.scroll = top as usize;
            }
        }
        cursor = cursor.min(shown - 1);
        let mut rows = Vec::with_capacity(shown);
        for i in 0..shown {
            let queued = if i == cursor { groups[3] } else { groups[2] };
            let r = self.create(
                seq,
                PRIO_ROW,
                groups[0],
                Some(queued),
                Some(kept(Vec::new())),
            );
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            rows.push(r);
        }
        let mut list = MenuList::new(rows, cursor);
        self.empty = count == 0;
        if self.empty {
            // FUN_1402f8d50(list, 0x10): no decide.
            list.mask &= !0x10;
        }
        self.goods = list;
        if !back {
            self.page = if buying { 1 } else { 2 };
            let p = &PAGES[self.page];
            self.layout.set_scene(p.scene[0], Some(p.scene[2]));
            let m = self.create(seq, PRIO_PANEL, MONEY[0], Some(MONEY[1]), None);
            self.money = Some(m);
            self.refresh_money(seq);
            self.build_columns(seq);
        }
        if !self.sprites_alive(self.holder) {
            let h = self.create(seq, PRIO_ROW, HOLDER[0], Some(HOLDER[1]), None);
            self.holder = Some(h);
        }
        if self.picture_holder.is_none_or(|h| !self.sprites.alive(h)) {
            let h = self.create(seq, PRIO_PICTURE - 1, BLANK, None, None);
            self.picture_holder = Some(h);
            self.picture.set_parent(h);
        }
        if count > SHOWN && self.scrollbar.is_empty() {
            for (k, g) in SCROLL.iter().enumerate() {
                let s = self.create(seq, PRIO_INFO, g[0], Some(g[1]), None);
                if let Some(&track) = self.scrollbar.first().filter(|_| k > 0) {
                    self.child(s, track);
                }
                self.scrollbar.push(s);
            }
        }
        self.place_scrollbar(seq);
        self.came_back = false;
        if buying && let Some(&id) = self.current_good().as_ref() {
            self.data.mark_seen(id);
        }
        self.fill_rows(seq);
        self.row_info(seq);
    }

    /// FUN_1402facd0: the thumb's length and place.
    fn place_scrollbar(&mut self, seq: &Sequence) {
        let &[_, top, middle, bottom] = self.scrollbar.as_slice() else {
            return;
        };
        let total = self.list.len().max(SHOWN);
        let length = seq.groups[SCROLL[0][0]].text_scale as f32;
        let t = seq.groups[SCROLL[2][0]].text_scale as f32;
        let scale = SHOWN as f32 / total as f32 * length / t;
        let y = ((length / total as f32) * self.scroll.min(total - SHOWN) as f32) as i32 as f32;
        let thumb = (t * scale) as i32 as f32;
        self.sprites.get_mut(top).offset = [0., y];
        let m = self.sprites.get_mut(middle);
        m.offset = [0., y];
        m.scale = [1., scale];
        self.sprites.get_mut(bottom).offset = [0., y + thumb];
    }

    fn sprites_alive(&self, id: Option<Id>) -> bool {
        id.is_some_and(|i| self.sprites.alive(i))
    }

    fn current_good(&self) -> Option<u16> {
        if self.list.is_empty() {
            return None;
        }
        self.list
            .get((self.scroll + self.goods.cursor) % self.list.len())
            .copied()
    }

    /// FUN_1402a2380.
    fn fill_rows(&mut self, seq: &Sequence) {
        let State::Goods { buying } = self.state else {
            return;
        };
        let groups = Self::rows_for(buying);
        for b in std::mem::take(&mut self.badges) {
            self.sprites.kill(b);
        }
        let rows = self.goods.rows.clone();
        let count = self.list.len();
        let suffix = self.data.texts.munny.clone();
        for (i, r) in rows.into_iter().enumerate() {
            let at = if count >= SHOWN {
                (self.scroll + i) % count.max(1)
            } else {
                self.scroll + i
            };
            let Some(&id) = self.list.get(at) else {
                let none = self.data.texts.none.clone();
                let s = self.sprites.get_mut(r);
                if let Some(t) = s.text.as_mut() {
                    t.bytes = none;
                }
                s.after = None;
                continue;
            };
            let Some(info) = self.data.info(id) else {
                continue;
            };
            let mut name = vec![0x09, info.icon];
            name.extend_from_slice(&info.name);
            let price = if buying {
                u32::from(self.data.prices.get(&id).copied().unwrap_or(0))
            } else {
                u32::from(info.sell)
            };
            let mut after = Text::new(number(price, &suffix), TEXT_SIZE);
            after.align = 1;
            let s = self.sprites.get_mut(r);
            if let Some(t) = s.text.as_mut() {
                t.bytes = name;
            }
            s.after = Some(after);
            if buying && self.fresh.contains(&id) {
                let group = if self.data.seen_bit(id) {
                    BADGE_SEEN
                } else {
                    BADGE_NEW
                };
                let b = self.create(seq, PRIO_ROW + 2, group, None, None);
                self.child(b, r);
                self.sprites.get_mut(b).offset = group_at(seq, groups[0]);
                self.badges.push(b);
            }
        }
    }

    /// FUN_1402a2090 and FUN_1402a1dc0: help, picture, stat panel, columns.
    fn row_info(&mut self, seq: &Sequence) {
        let good = self.current_good();
        let Some(id) = good else {
            self.show_help(seq, Vec::new());
            self.clear_info();
            self.update_columns(None);
            return;
        };
        let (description, picture) = match self.data.info(id) {
            Some(i) => (i.description.clone(), i.picture),
            None => (Vec::new(), 0),
        };
        self.show_help(seq, description);
        if let Some(ipic) = self.data.picture_sequence.clone() {
            self.picture.request(seq, &ipic, &mut self.sprites, picture);
        }
        self.stat_panel(seq, id);
        self.update_columns(Some(id));
    }

    fn clear_info(&mut self) {
        for s in std::mem::take(&mut self.info) {
            self.sprites.kill(s);
        }
    }

    /// FUN_1402fd390 in mode 4: the good's stats, no member's totals.
    fn stat_panel(&mut self, seq: &Sequence, id: u16) {
        self.clear_info();
        let Some(holder) = self.holder.filter(|&h| self.sprites.alive(h)) else {
            return;
        };
        let Some(info) = self.data.info(id) else {
            return;
        };
        let kind = info.kind;
        let Some(st) = super::equip::stats_of(&self.data.table, &self.data.stats, id) else {
            return;
        };
        let mask = super::equip::row_mask(kind, &st);
        let base = group_at(seq, HOLDER[1]);
        let pitch = seq.groups[HOLDER[1]].ui_padding as f32;
        let value_at = group_at(seq, INFO_LABEL);
        let mut n = 0;
        for k in 0..9 {
            if mask >> k & 1 == 0 {
                continue;
            }
            let label = self.data.texts.stats[k].clone();
            let l = self.create(seq, PRIO_INFO, INFO_LABEL, None, Some(kept(label)));
            self.child(l, holder);
            self.sprites.get_mut(l).offset = [base[0], base[1] + pitch * n as f32];
            self.info.push(l);
            let (bytes, group, at, align) = if k == 8 {
                let name = self
                    .data
                    .info(st.ability)
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                n += 1;
                (name, INFO_ABILITY, [0., pitch], 2)
            } else {
                (
                    super::equip::number_text(k, super::equip::stat_value(&st, k)),
                    INFO_VALUE,
                    value_at,
                    1,
                )
            };
            let mut t = kept(bytes);
            t.align = align;
            let v = self.create(seq, PRIO_VALUE, group, None, Some(t));
            self.child(v, l);
            self.sprites.get_mut(v).offset = at;
            self.info.push(v);
            n += 1;
        }
    }

    /// FUN_14029c350: a column for each member, then Stock, right to left.
    fn build_columns(&mut self, seq: &Sequence) {
        let n = self.data.members.len();
        let pitch = seq.groups[COLUMN[0]].text_position_x as f32;
        self.columns.clear();
        for i in 0..n {
            let mut t = Text::new(Vec::new(), TEXT_SIZE);
            t.align = 2;
            let c = self.create(seq, PRIO_ROW, COLUMN[0], Some(COLUMN[1]), Some(t));
            self.sprites.get_mut(c).offset = [(n - 1 - i) as f32 * pitch, 0.];
            self.columns.push(c);
        }
        let mut t = Text::new(Vec::new(), TEXT_SIZE);
        t.align = 2;
        let s = self.create(seq, PRIO_ROW, STOCK[0], Some(STOCK[1]), Some(t));
        self.columns.push(s);
        let mut label = kept(self.data.texts.stock.clone());
        label.align = 2;
        let l = self.create(seq, PRIO_ROW, STOCK_LABEL, None, Some(label));
        self.child(l, s);
        self.stock_label = Some(l);
    }

    /// FUN_1402a1dc0.
    fn update_columns(&mut self, id: Option<u16>) {
        let members = self.data.members.len();
        for (i, c) in self.columns.clone().into_iter().enumerate() {
            let text = id.map_or(Vec::new(), |id| {
                let n = if i < members {
                    self.data.held(&self.data.members[i], id)
                } else {
                    self.data.stock_of(id)
                };
                encode_number(&n.to_string())
            });
            if self.sprites.alive(c)
                && let Some(t) = self.sprites.get_mut(c).text.as_mut()
            {
                t.bytes = text;
            }
        }
    }

    /// FUN_14029eeb0 and the like: everything of the goods page but the page.
    fn close_goods_rows(&mut self, seq: &Sequence) {
        let State::Goods { buying } = self.state else {
            return;
        };
        let rows = self.goods.rows.clone();
        self.close_rows(seq, &rows, Self::rows_for(buying)[1]);
        self.kill_badges();
        for s in std::mem::take(&mut self.scrollbar) {
            self.sprites.kill(s);
        }
    }

    /// One step of the list: the row, else the circular scroll.
    fn move_list(&mut self, pad: &Pad) -> bool {
        use bits::*;
        let count = self.list.len();
        if count == 0 {
            return false;
        }
        let rows = self.goods.rows.len();
        let c = self.goods.cursor;
        let wide = count > SHOWN;
        if pad.repeat & UP != 0 {
            if c > 0 {
                self.goods.cursor -= 1;
                return true;
            }
            if wide {
                self.scroll = (self.scroll + count - 1) % count;
                return true;
            }
            if pad.trig & UP != 0 && rows > 1 {
                self.goods.cursor = rows - 1;
                return true;
            }
        } else if pad.repeat & DOWN != 0 {
            if c + 1 < rows {
                self.goods.cursor += 1;
                return true;
            }
            if wide {
                self.scroll = (self.scroll + 1) % count;
                return true;
            }
            if pad.trig & DOWN != 0 && rows > 1 {
                self.goods.cursor = 0;
                return true;
            }
        } else if wide && pad.repeat & L1 != 0 {
            self.scroll = (self.scroll + count - SHOWN.min(count)) % count;
            return true;
        } else if wide && pad.repeat & R1 != 0 {
            self.scroll = (self.scroll + SHOWN) % count;
            return true;
        }
        false
    }

    /// FUN_14029c860.
    fn goods_tick(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let State::Goods { buying } = self.state else {
            return false;
        };
        if self.substate == 2 {
            if !self.rows_alive(&self.goods) {
                match self.next.take() {
                    Some(State::Top) => {
                        self.close_money_and_columns(seq);
                        self.enter_top(seq);
                    }
                    Some(State::Quantity { buying }) => self.enter_quantity(seq, buying),
                    _ => {}
                }
            } else if pad.trig != 0 {
                let rows = self.goods.rows.clone();
                for r in rows {
                    self.sprites.kill(r);
                }
            }
            return false;
        }
        if self.substate == 0 {
            self.substate = 1;
        }
        if self.move_list(pad) {
            sounds.push(SOUND_CURSOR);
            if buying && let Some(id) = self.current_good() {
                self.data.mark_seen(id);
            }
            self.fill_rows(seq);
            self.row_info(seq);
            self.place_scrollbar(seq);
            return true;
        }
        if pad.trig & bits::DECIDE != 0 {
            let Some(id) = self.current_good().filter(|_| !self.empty) else {
                return false;
            };
            let info = self.data.info(id);
            let sell = u32::from(info.map_or(0, |i| i.sell));
            let buy = u32::from(self.data.prices.get(&id).copied().unwrap_or(0));
            let ok = rules::can_trade(buying, self.data.stock_of(id), buy, sell, self.data.munny);
            if !ok {
                sounds.push(SOUND_INVALID);
                return false;
            }
            sounds.push(SOUND_DECIDE);
            self.trade = Trade {
                id,
                quantity: 1,
                price: if buying { buy } else { sell },
            };
            self.hide_help(seq);
            self.close_goods_rows(seq);
            self.substate = 2;
            self.next = Some(State::Quantity { buying });
        } else if pad.trig & bits::CANCEL != 0 {
            sounds.push(SOUND_CLOSE);
            self.hide_help(seq);
            self.close_goods_rows(seq);
            self.close(seq, self.holder, HOLDER[2]);
            self.clear_info();
            if let Some(ipic) = self.data.picture_sequence.clone() {
                self.picture.close(seq, &ipic, &mut self.sprites);
            }
            self.scroll = 0;
            self.goods.cursor = 0;
            let (from, to) = (&PAGES[self.page], &PAGES[0]);
            self.layout.set_scene(from.scene[1], Some(to.scene[2]));
            self.substate = 2;
            self.next = Some(State::Top);
        } else if pad.trig & bits::START != 0 {
            sounds.push(SOUND_CLOSE);
            self.start_exit(seq);
        }
        false
    }

    fn close_money_and_columns(&mut self, seq: &Sequence) {
        self.close(seq, self.money, MONEY[2]);
        let members = self.data.members.len();
        for (i, c) in self.columns.clone().into_iter().enumerate() {
            let out = if i < members { COLUMN[2] } else { STOCK[2] };
            if self.sprites.alive(c) {
                self.sprites.close(seq, c, out);
            }
        }
    }

    fn refresh_money(&mut self, seq: &Sequence) {
        let mut v = self.data.munny.min(rules::MUNNY_CAP);
        self.money_digits = (0..6)
            .map(|k| {
                let g = if k == 0 || v > 0 {
                    DIGIT + (v % 10) as usize
                } else {
                    BLANK
                };
                v /= 10;
                Player::new(seq, g)
            })
            .collect();
    }

    // ---- Quantity --------------------------------------------------------

    /// FUN_14029c860's out for state 2 or 5.
    fn enter_quantity(&mut self, seq: &Sequence, buying: bool) {
        self.state = State::Quantity { buying };
        self.substate = 0;
        if self.sprites_alive(self.panel) {
            self.prompt_text(seq, buying, false);
            self.substate = 1;
            return;
        }
        let name = self
            .data
            .info(self.trade.id)
            .map(|i| {
                let mut n = vec![0x09, i.icon];
                n.extend_from_slice(&i.name);
                n
            })
            .unwrap_or_default();
        let p = self.create(seq, PRIO_PANEL, PANEL[0], Some(PANEL[1]), Some(kept(name)));
        self.panel = Some(p);
        let g = &seq.groups[PANEL[0]];
        let (a, b, c) = (
            g.light_position_x as f32,
            g.ui_padding as f32,
            g.text_position_x as f32,
        );
        // Cells of FUN_14029e440: price; quantity, total; munny, after.
        let cells: [(f32, f32, u8); 5] = [
            (a, 0., 1),
            (c, b, 1),
            (a, b, 1),
            (c, 2. * b, 1),
            (a, 2. * b, 1),
        ];
        self.cells.clear();
        for (x, y, align) in cells {
            let mut t = kept(Vec::new());
            t.align = align;
            let s = self.create(seq, PRIO_PANEL, BLANK, None, Some(t));
            self.child(s, p);
            self.sprites.get_mut(s).offset = [x, y];
            self.cells.push(s);
        }
        self.prompt_text(seq, buying, false);
        self.refresh_cells(buying);
    }

    /// The prompt: how many, or sure.
    fn prompt_text(&mut self, seq: &Sequence, buying: bool, sure: bool) {
        let set = if sure {
            &self.data.texts.sure
        } else {
            &self.data.texts.how_many
        };
        let bytes = set[usize::from(!buying)].clone();
        if let Some(p) = self.prompt.filter(|&p| self.sprites.alive(p))
            && let Some(t) = self.sprites.get_mut(p).text.as_mut()
        {
            t.bytes = bytes;
            return;
        }
        let p = self.create(
            seq,
            PRIO_PANEL,
            PROMPT[0],
            Some(PROMPT[1]),
            Some(kept(bytes)),
        );
        self.prompt = Some(p);
    }

    /// FUN_1402a21c0.
    fn refresh_cells(&mut self, buying: bool) {
        let t = self.trade;
        let suffix = self.data.texts.munny.clone();
        let total = t.price.saturating_mul(t.quantity);
        let after = rules::munny_after(self.data.munny, total, buying);
        let texts = [
            number(t.price, &suffix),
            encode_number(&t.quantity.to_string()),
            number(total, &suffix),
            number(self.data.munny, &suffix),
            number(after, &suffix),
        ];
        for (c, bytes) in self.cells.clone().into_iter().zip(texts) {
            if self.sprites.alive(c)
                && let Some(s) = self.sprites.get_mut(c).text.as_mut()
            {
                s.bytes = bytes;
            }
        }
    }

    fn close_panel(&mut self, seq: &Sequence) {
        self.close(seq, self.panel, PANEL[2]);
        self.close(seq, self.prompt, PROMPT[2]);
        let arrows = self.arrows.take();
        self.kill(arrows);
    }

    /// FUN_14029d440.
    fn quantity_tick(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) {
        use bits::*;
        let State::Quantity { buying } = self.state else {
            return;
        };
        if self.substate == 2 {
            if self.panel.is_none_or(|p| !self.sprites.alive(p)) {
                self.panel = None;
                self.prompt = None;
                self.cells.clear();
                self.came_back = true;
                self.enter_goods(seq, buying);
            }
            return;
        }
        if self.substate == 0 {
            if self
                .panel
                .is_some_and(|p| self.sprites.get(p).player.group == PANEL[1])
            {
                self.substate = 1;
                let a = self.create(seq, PRIO_ROW, ARROWS, None, None);
                self.arrows = Some(a);
            }
            return;
        }
        let step = if pad.repeat & UP != 0 {
            1
        } else if pad.repeat & DOWN != 0 {
            -1
        } else if pad.repeat & LEFT != 0 {
            -10
        } else if pad.repeat & RIGHT != 0 {
            10
        } else {
            0
        };
        if step != 0 {
            let t = self.trade;
            if let Some(q) = rules::step_quantity(
                t.quantity,
                step,
                buying,
                self.data.stock_of(t.id),
                self.data.munny,
                t.price,
            ) {
                self.trade.quantity = q;
                sounds.push(SOUND_CURSOR);
                self.refresh_cells(buying);
            }
            return;
        }
        if pad.trig & DECIDE != 0 {
            sounds.push(SOUND_DECIDE);
            self.enter_confirm(seq, buying);
        } else if pad.trig & CANCEL != 0 {
            sounds.push(SOUND_CLOSE);
            self.close_panel(seq);
            self.substate = 2;
        } else if pad.trig & START != 0 {
            sounds.push(SOUND_CLOSE);
            self.start_exit(seq);
        }
    }

    // ---- Confirm ---------------------------------------------------------

    fn enter_confirm(&mut self, seq: &Sequence, buying: bool) {
        self.state = State::Confirm { buying };
        self.substate = 0;
        self.prompt_text(seq, buying, true);
        let g = &seq.groups[CONFIRM_ROWS[3]];
        let (pad, text_x) = (g.ui_padding as f32, g.text_position_x as f32);
        let labels = [self.data.texts.no.clone(), self.data.texts.yes.clone()];
        let mut rows = Vec::new();
        for (i, label) in labels.into_iter().enumerate() {
            let mut t = kept(label);
            t.align = 2;
            t.offset = [text_x, 0.];
            let queued = if i == 0 {
                CONFIRM_ROWS[3]
            } else {
                CONFIRM_ROWS[2]
            };
            let r = self.create(seq, PRIO_CHOICE, CONFIRM_ROWS[0], Some(queued), Some(t));
            self.sprites.get_mut(r).offset = [if i == 1 { pad } else { 0. }, 0.];
            rows.push(r);
        }
        let mut list = MenuList::new(rows, 0);
        // FUN_1402f8eb0: Right steps back, Left on. Start is off.
        list.keys = Some((bits::RIGHT, bits::LEFT));
        list.mask &= !0x40;
        self.choice = list;
    }

    /// FUN_14029cfa0.
    fn confirm_tick(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let State::Confirm { buying } = self.state else {
            return false;
        };
        if self.substate == 2 {
            if !self.rows_alive(&self.choice) {
                let done = self.next.take() == Some(State::Goods { buying });
                self.choice = MenuList::new(Vec::new(), 0);
                if done {
                    self.state = State::Quantity { buying };
                    self.substate = 2;
                } else {
                    self.state = State::Quantity { buying };
                    self.prompt_text(seq, buying, false);
                    self.substate = 1;
                }
            }
            return false;
        }
        if self.substate == 0 {
            self.substate = 1;
        }
        let (pick, moved) = self.choice.input(pad);
        if moved {
            sounds.push(SOUND_CURSOR);
            return true;
        }
        match pick {
            Pick::Row(1) => {
                sounds.push(SOUND_CONFIRM);
                sounds.push(SOUND_TRADE);
                self.do_trade(seq, buying);
                self.close_panel(seq);
                let rows = self.choice.rows.clone();
                self.close_rows(seq, &rows, CONFIRM_ROWS[1]);
                self.substate = 2;
                self.next = Some(State::Goods { buying });
            }
            Pick::Row(_) | Pick::Cancel => {
                let sound = if pick == Pick::Cancel {
                    SOUND_CLOSE
                } else {
                    SOUND_DECIDE
                };
                sounds.push(sound);
                let rows = self.choice.rows.clone();
                self.close_rows(seq, &rows, CONFIRM_ROWS[1]);
                self.substate = 2;
                self.next = Some(State::Quantity { buying });
            }
            _ => {}
        }
        false
    }

    /// FUN_140302500 and FUN_140302470.
    fn do_trade(&mut self, seq: &Sequence, buying: bool) {
        let t = self.trade;
        let total = t.price.saturating_mul(t.quantity);
        self.data.munny = rules::munny_after(self.data.munny, total, buying);
        let held = self.data.stock_of(t.id);
        let left = rules::stock_after(held, t.quantity, buying);
        if left == 0 {
            self.data.stock.remove(&t.id);
        } else {
            self.data.stock.insert(t.id, left as u8);
        }
        self.refresh_money(seq);
    }

    // ---- Exit ------------------------------------------------------------

    /// FUN_14029d960's exit and FUN_14029dd40's first block.
    fn start_exit(&mut self, seq: &Sequence) {
        self.exit = true;
        let p = &PAGES[self.page];
        self.layout.set_scene(p.scene[1], None);
        self.close(seq, self.frame_sprite, FRAME[2]);
        let top = self.top.rows.clone();
        self.close_rows(seq, &top, TOP_ROWS[1]);
        if let State::Goods { .. } = self.state {
            self.close_goods_rows(seq);
        } else {
            let rows = self.goods.rows.clone();
            for r in rows {
                if self.sprites.alive(r) {
                    self.sprites.kill(r);
                }
            }
        }
        self.close_money_and_columns(seq);
        self.close(seq, self.holder, HOLDER[2]);
        self.clear_info();
        self.close_panel(seq);
        self.hide_help(seq);
        self.kill_badges();
        if let Some(ipic) = self.data.picture_sequence.clone() {
            self.picture.close(seq, &ipic, &mut self.sprites);
        }
        let (glove, dim) = (self.glove.take(), self.dim.take());
        self.kill(glove);
        self.kill(dim);
        self.light.kill(&mut self.sprites);
    }

    // ---- The frame -------------------------------------------------------

    fn active(&self) -> Option<(&MenuList, [usize; 4])> {
        match self.state {
            State::Top => Some((&self.top, TOP_ROWS)),
            State::Goods { buying } if !self.empty => Some((&self.goods, Self::rows_for(buying))),
            State::Confirm { .. } => Some((&self.choice, CONFIRM_ROWS)),
            State::Notice => Some((&self.choice, CHOICE_ROWS)),
            _ => None,
        }
    }

    /// FUN_14029dd40's glove and LightCur.
    fn driver(&mut self, seq: &Sequence, moved: bool) {
        let list = self.active().map(|(l, g)| (l.rows.clone(), l.cursor, g));
        let live = list
            .as_ref()
            .is_some_and(|(rows, _, _)| rows.iter().any(|&r| self.sprites.alive(r)));
        if self.substate == 1 && live {
            let (rows, cursor, groups) = list.expect("live");
            if moved {
                for (i, &r) in rows.iter().enumerate() {
                    let g = if i == cursor { groups[3] } else { groups[2] };
                    self.sprites.set(seq, r, g, None);
                }
                self.light.retarget();
            }
            let row = rows[cursor.min(rows.len() - 1)];
            match self.glove {
                Some(g) if self.sprites.alive(g) => self.sprites.get_mut(g).parent = Some(row),
                _ => {
                    let g = self.create(seq, PRIO_LIGHT, GLOVE, None, None);
                    self.sprites.get_mut(g).parent = Some(row);
                    self.glove = Some(g);
                }
            }
            if self.light.id.is_none_or(|l| !self.sprites.alive(l)) {
                let l = self.create(seq, PRIO_LIGHT, LIGHT[0], Some(LIGHT[1]), None);
                self.light = LightCur::new(l);
            }
            // On Yes or No the list's own glove stands dimmed on the goods.
            if matches!(self.state, State::Confirm { .. }) {
                if !self.sprites_alive(self.dim)
                    && let Some(&r) = self.goods.rows.get(self.goods.cursor)
                    && self.sprites.alive(r)
                {
                    let d = self.create(seq, PRIO_LIGHT, GLOVE_DIM, None, None);
                    self.sprites.get_mut(d).parent = Some(r);
                    self.dim = Some(d);
                }
            } else {
                let dim = self.dim.take();
                self.kill(dim);
            }
        } else if self.substate == 2 || !live {
            let (glove, dim) = (self.glove.take(), self.dim.take());
            self.kill(glove);
            self.kill(dim);
            if let Some(l) = self.light.id
                && self.sprites.alive(l)
                && self.sprites.get(l).player.group != LIGHT[2]
            {
                self.sprites.close(seq, l, LIGHT[2]);
            }
        }
    }

    fn place_light(&mut self, seq: &Sequence) {
        if !self.light.id.is_some_and(|l| self.sprites.alive(l)) {
            return;
        }
        let Some((list, _)) = self.active() else {
            return;
        };
        let Some(&row) = list.rows.get(list.cursor) else {
            return;
        };
        if self.sprites.alive(row) {
            let c = self.sprites.context(row);
            let g = group_at(seq, self.sprites.get(row).player.group);
            self.light
                .place(&mut self.sprites, [c[0] + g[0], c[1] + g[1]]);
        }
    }

    /// What the host renders: the shopkeeper while the frame stands.
    pub fn model_view(&self) -> Option<super::journal::ModelView> {
        let f = self.frame_sprite.filter(|&f| self.sprites.alive(f))?;
        let (object, at, depth) = self.data.model;
        let drop = if DUCKS.contains(&object) {
            MODEL_DROP_DUCKS
        } else {
            MODEL_DROP
        };
        Some(super::journal::ModelView {
            object,
            motion: MODEL_MOTION,
            at: at.map(f32::from),
            turn: MODEL_TURN,
            scale: 1.,
            z: f32::from(depth),
            drop,
            hidden_parts: 0,
            ticks: self.model_ticks,
            alpha: self.sprites.child_alpha(f),
            prio: PRIO_FRAME + 1,
        })
    }

    /// One frame: input, then the sprites. Returns the se000 sounds.
    pub fn frame(&mut self, layout: &Layout, trig: u64, repeat: u64) -> Vec<u32> {
        let seq = &layout.sequences[SEQUENCE];
        let pad = Pad { trig, repeat };
        let mut sounds = Vec::new();
        if !self.exit {
            let gated = !self.layout.dead
                && PAGES.iter().any(|p| p.scene[0] == self.layout.scene)
                && trig == 0;
            if !gated {
                let moved = match self.state {
                    State::Top => self.top_tick(seq, &pad, &mut sounds),
                    State::Goods { .. } => self.goods_tick(seq, &pad, &mut sounds),
                    State::Quantity { .. } => {
                        self.quantity_tick(seq, &pad, &mut sounds);
                        false
                    }
                    State::Confirm { .. } => self.confirm_tick(seq, &pad, &mut sounds),
                    State::Notice => {
                        self.notice_tick(seq, &pad, &mut sounds);
                        false
                    }
                };
                self.driver(seq, moved);
            }
        }
        self.layout.update(layout, 1.);
        self.sprites.update(seq);
        self.model_ticks += 1;
        self.place_help(layout);
        // The picture stands on the prop the buy and sell scenes place.
        if let Some(h) = self.picture_holder.filter(|&h| self.sprites.alive(h))
            && let Some(a) = crate::world_map_menu::anchor(layout, &self.layout, 4)
        {
            self.sprites.get_mut(h).base = a.at;
        }
        if let Some(ipic) = self.data.picture_sequence.clone() {
            let mut cache = std::mem::take(&mut self.pictures);
            self.picture
                .timers(seq, &ipic, &mut self.sprites, &mut cache);
            self.pictures = cache;
            self.picture.update(&ipic, &self.sprites);
        }
        for p in &mut self.faces {
            p.update(&layout.sequences[FACE_SEQUENCE], 1.);
        }
        for p in &mut self.money_digits {
            p.update(seq, 1.);
        }
        self.place_light(seq);
        sounds
    }

    fn digit_quads(&self, seq: &Sequence) -> Vec<Quad> {
        let mut out = Vec::new();
        let Some(id) = self.money.filter(|&m| self.sprites.alive(m)) else {
            return out;
        };
        let s = self.sprites.get(id);
        let g = group_at(seq, s.player.group);
        let c = self.sprites.context(id);
        let mut x = c[0] + g[0];
        let y = c[1] + g[1];
        let a = seq
            .context_color(&s.player, &Modifiers::default())
            .map_or(1., |c| c[3]);
        let m = Modifiers {
            color: [1., 1., 1., a],
            ..Modifiers::default()
        };
        for p in &self.money_digits {
            out.extend(seq.quads(p, [x, y], &m));
            x -= seq.groups[p.group].text_position_x as f32;
        }
        out
    }

    pub fn draw(&self, layout: &Layout, font: &Font) -> Vec<(Sheet, Vec<Quad>)> {
        let seq = &layout.sequences[SEQUENCE];
        let m = Modifiers::default();
        let mut out = Vec::new();
        if !self.layout.dead {
            for (prop, player, origin) in self.layout.player.visible(layout, [0, 0]) {
                out.push((
                    Sheet::Image(prop.texture),
                    layout.sequences[prop.sequence].quads(player, origin, &m),
                ));
            }
        }
        // Tz::LayMsg: the shop's name on its page's title bar.
        let title = PAGES.iter().flat_map(|p| p.title).find_map(|g| {
            crate::world_map_menu::anchor(layout, &self.layout, g + FLAT).map(|a| (g, a))
        });
        if let Some((g, a)) = title {
            let at = group_at(seq, g);
            let q = sprite_player::text_quads(
                &self.data.name,
                [a.at[0] + at[0], a.at[1] + at[1]],
                TEXT_SIZE,
                2,
                seq.groups[g].ui_padding as f32,
                [0.898, 0.898, 0.898, a.color[3]],
                true,
                font,
            );
            out.push((Sheet::Font, q.glyphs));
            if !q.icons.is_empty() {
                out.push((Sheet::Icon, q.icons));
            }
        }
        let convert = |items: Vec<(sprite_player::Sheet, Vec<Quad>)>| {
            items.into_iter().map(|(sheet, quads)| match sheet {
                sprite_player::Sheet::Art(k) => (Sheet::Image(k), quads),
                sprite_player::Sheet::Font => (Sheet::Font, quads),
                sprite_player::Sheet::Icon => (Sheet::Icon, quads),
            })
        };
        out.extend(convert(
            self.sprites.draw_where(seq, font, |p| p <= PRIO_FRAME),
        ));
        // The model's paste, at 0x7538, in the frame's alpha.
        if let Some(v) = self.model_view().filter(|_| self.model_ready.is_some()) {
            out.push((
                Sheet::Model,
                vec![super::journal::model_quad(v.at, v.alpha)],
            ));
        }
        out.extend(convert(self.sprites.draw_where(seq, font, |p| {
            (PRIO_FRAME + 1..=PRIO_PANEL).contains(&p)
        })));
        out.push((Sheet::Image(IMAGE), self.digit_quads(seq)));
        out.extend(convert(self.sprites.draw_where(seq, font, |p| {
            (PRIO_PANEL + 1..PRIO_PICTURE).contains(&p)
        })));
        if let Some(ipic) = &self.data.picture_sequence
            && let Some((id, _, quads)) = self.picture.quads(ipic, &self.sprites)
        {
            out.push((Sheet::Picture(id), quads));
        }
        out.extend(convert(
            self.sprites
                .draw_where(seq, font, |p| (PRIO_PICTURE..PRIO_FACE).contains(&p)),
        ));
        // The party's faces on their columns.
        let face_seq = &layout.sequences[FACE_SEQUENCE];
        for (i, p) in self.faces.iter().enumerate() {
            let Some(&c) = self.columns.get(i) else {
                continue;
            };
            if !self.sprites.alive(c) {
                continue;
            }
            let a = self.sprites.child_alpha(c);
            let m = Modifiers {
                color: [1., 1., 1., a],
                ..Modifiers::default()
            };
            out.push((
                Sheet::Face(i),
                face_seq.quads(p, self.sprites.context(c), &m),
            ));
        }
        out.extend(convert(
            self.sprites.draw_where(seq, font, |p| p >= PRIO_FACE),
        ));
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    /// `Art(k)`: the 2LD's image k.
    Image(usize),
    Font,
    Icon,
    /// The shopkeeper's render.
    Model,
    /// An item picture.
    Picture(u16),
    /// Member i's face (`menu/shopface/<name>.bin`).
    Face(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u16, kind: u8, sell: u16) -> ItemInfo {
        ItemInfo {
            id,
            kind,
            sell,
            ..Default::default()
        }
    }

    fn data() -> Data {
        Data {
            table: vec![
                item(1, 0, 10),
                item(2, 2, 0),
                item(3, 0x12, 5),
                item(4, 0xf, 20),
            ],
            stock: BTreeMap::from([(1, 3), (2, 1), (3, 1), (4, 2)]),
            watch: vec![1, 4],
            ..Default::default()
        }
    }

    #[test]
    fn the_sell_list_keeps_file_order_and_skips_what_has_no_price() {
        // 2 has no sell price, 3's kind can't be sold.
        assert_eq!(data().sell_list(), vec![1, 4]);
    }

    #[test]
    fn seen_goods_use_their_watch_bit() {
        let mut d = data();
        assert!(!d.seen_bit(4));
        d.mark_seen(4);
        assert!(d.seen_bit(4) && !d.seen_bit(1));
        // A good off the list has no bit.
        d.mark_seen(9);
        assert!(!d.seen_bit(9));
    }

    #[test]
    fn a_member_counts_what_it_holds_and_sora_his_forms() {
        let d = data();
        let e = Equipment {
            weapon: 7,
            armor: vec![7],
            forms: vec![7],
            ..Default::default()
        };
        let member = Member {
            equipment: e.clone(),
            sora: false,
        };
        assert_eq!(d.held(&member, 7), 2);
        let sora = Member {
            equipment: e,
            sora: true,
        };
        assert_eq!(d.held(&sora, 7), 3);
    }
}
