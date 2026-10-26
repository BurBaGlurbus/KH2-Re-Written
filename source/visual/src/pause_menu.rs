//! The camp (pause) menu, translated from the camp process: build
//! FUN_140306040 / FUN_140306790 / FUN_140367b50, state machine
//! FUN_140303260 with the top level FUN_1403037d0, the Status screens
//! (0x17 FUN_140351db0 / FUN_140353020 / FUN_1403540b0, 0x18 FUN_140351a00 /
//! FUN_140352be0 / FUN_140353800) and Config (0x24 FUN_140364d70 /
//! FUN_140365580 / FUN_140365a20), the per-frame driver FUN_140303cd0
//! (glove pointer, LightCur), list input FUN_1402f9440, title tabs
//! FUN_140323c60, help FUN_1402fa1d0 and the digit rows FUN_1402fb450 /
//! FUN_140324690. Everything draws from `menu/<lang>/camp.2ld` sequence 1 in
//! layout space (x origin 0). The sub-screens live in the child modules;
//! the Journal runs after the camp (`journal`). The 3D party models are pasted from off-screen
//! renders the caller makes (Tz::MenuObj).
pub mod abilities;
pub mod customize;
pub mod equip;
pub mod items;
pub mod journal;
pub mod notice;
pub mod party;
pub mod picture;

use crate::field_command_menu::Font;
use crate::sprite_player::{self, Id, LightCur, Sprites, Text};
use data::menu_layout::{Layout, LayoutTask};
use data::sprite_sequence::{Modifiers, Player, Quad, Sequence};

/// The camp sprites' SEQD (DAT_140743fd4) and image.
pub const SEQUENCE: usize = 1;
const ART: usize = 0;

/// Layout scenes (L2): open, idle, close.
const SCENE_OPEN: usize = 0;
const SCENE_IDLE: usize = 1;
const SCENE_CLOSE: usize = 2;
/// Layout elements the help text follows (open / idle / close).
const HELP_PARENTS: [usize; 3] = [39, 40, 41];

/// Priorities (draw order: ascending, then creation).
const PRIO_FRAME: u32 = 0x7533;
const PRIO_PANEL: u32 = 0x753b;
const PRIO_ROW: u32 = 0x753c;
/// The top rows' NEW! badges (camp-01 §3.3).
const PRIO_ROW_BADGE: u32 = 0x753d;
const PRIO_BAR: u32 = 0x753d;
const PRIO_LIGHT: u32 = 0x754a;

/// Top-level rows (entry table 0x1405badb0): appear, close, idle, selected.
const TOP_ROWS: RowGroups = RowGroups {
    appear: 133,
    close: 135,
    idle: 134,
    selected: 132,
};
/// Status character list rows: characters, forms/Summon (0x1405bad68..80).
const FORM_ROWS: RowGroups = RowGroups {
    appear: 129,
    close: 131,
    idle: 130,
    selected: 128,
};
const TEXT_SIZE: f32 = 15.;

const LIGHT: [usize; 3] = [27, 28, 29];
const GLOVE: usize = 25;
const HELP_IN: usize = 116;
const HELP_OUT: usize = 117;
const FRAME: [usize; 3] = [46, 47, 48];
const PROMPT: [usize; 3] = [351, 352, 353];
const MUNNY: [usize; 3] = [107, 108, 109];
const TIME: [usize; 3] = [113, 114, 115];
const MUNNY_DIGIT: usize = 51;
const TIME_DIGIT: usize = 61;
const BLANK: usize = 31;
const COLON: usize = 71;
/// Title tabs (FUN_140323c60): appear, idle, close.
const TAB: [usize; 3] = [75, 76, 77];
/// Party status block (FUN_140367b50 mode 0).
const STATUS: [usize; 3] = [101, 102, 103];
const BLOCK: usize = 93;
const SUB_ANCHOR: usize = 90;
/// LV, HP, MP style groups (FUN_140304d70(2/4/6)); yellow values (124).
const STYLES: [usize; 3] = [124, 121, 118];
const VALUE: usize = 124;
const GAUGE_FRAME: usize = 95;
const GAUGE_BARS: [usize; 2] = [98, 99];
/// Status 0x17 panel, label group; 0x18 panel, banner.
const STATUS_PANEL: [usize; 3] = [140, 141, 142];
const STATUS_LABEL: usize = 137;
const BROWSE_PANEL: [usize; 3] = [320, 321, 322];
const BROWSE_ROW: usize = 323;
const BANNER: [usize; 3] = [237, 238, 239];
/// Stat style s → (0x17 label group, 0x18 row group) (0x1405bbef0).
const STAT_STYLES: [(usize, usize); 5] =
    [(137, 323), (143, 325), (150, 327), (139, 324), (144, 326)];
/// Config panel, label group; choice groups {selected, unselected,
/// cursor frame} by choice count (0x1405bc320..30).
const CONFIG_PANEL: [usize; 3] = [233, 234, 235];
const CONFIG_LABEL: usize = 279;
const CHOICES: [[usize; 3]; 3] = [[261, 260, 259], [255, 254, 253], [258, 257, 256]];

/// Priorities of the sub-screens (camp-04 §2-3).
const PRIO_STATUS_PANEL: u32 = 0x753d;
const PRIO_STATUS_LABEL: u32 = 0x753e;
const PRIO_STATUS_ROW: u32 = 0x753f;
const PRIO_CONFIG_PANEL: u32 = 0x753a;

/// se000 sounds (FUN_1402f6270 → FUN_1402eb740): open, move, decide,
/// close/cancel, invalid.
pub const SOUND_OPEN: u32 = 3;
pub const SOUND_CURSOR: u32 = 1;
pub const SOUND_DECIDE: u32 = 2;
pub const SOUND_CLOSE: u32 = 4;
pub const SOUND_INVALID: u32 = 5;

/// Logical bits (pad mode 7 while a menu runs).
mod bits {
    pub const UP: u64 = 0x10;
    pub const DOWN: u64 = 0x20;
    pub const LEFT: u64 = 0x40;
    pub const RIGHT: u64 = 0x80;
    pub const DECIDE: u64 = 0x4;
    pub const CANCEL: u64 = 0x8;
    pub const START: u64 = 0x100;
    pub const SQUARE: u64 = 0x800;
    pub const TRIANGLE: u64 = 0x1000;
    pub const SELECT: u64 = 0x2000;
    pub const R1: u64 = 0x4000;
    pub const L1: u64 = 0x8000;
    pub const R2: u64 = 0x1_0000;
    pub const L2: u64 = 0x2_0000;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RowGroups {
    appear: usize,
    close: usize,
    idle: usize,
    selected: usize,
}

/// A top-level entry: id 0..7 (Items, Abilities, Customize, Party, Status,
/// Jiminy's Journal, Roxas's Journal, Config), label and help text.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: u8,
    pub label: Vec<u8>,
    pub help: Vec<u8>,
}

/// One party member's status block values.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub name: Vec<u8>,
    pub level: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
}

/// What the top-level panels show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Panels {
    pub munny: u32,
    /// Play time in 60 Hz frames (save+0x2444).
    pub time_frames: u32,
    pub members: Vec<Member>,
    /// "LV", "HP", "MP" (sys.bar 0xb9fc..0xb9fe).
    pub labels: [Vec<u8>; 3],
}

/// One Status stat (camp-04 §2.4): label, help, style and its encoded
/// value text.
#[derive(Clone, Debug, PartialEq)]
pub struct Stat {
    pub label: Vec<u8>,
    pub help: Vec<u8>,
    pub style: usize,
    pub value: Vec<u8>,
}

/// A Status list entry: its list bit (0 Sora, 1..5 forms, 6 Summon, 7..9
/// party), label, help line and its 20 stats (None = not shown).
#[derive(Clone, Debug, PartialEq)]
pub struct StatusEntry {
    pub bit: u8,
    pub label: Vec<u8>,
    pub help: Vec<u8>,
    pub stats: Vec<Option<Stat>>,
}

/// A Config row: label, choice labels and the help line of each choice.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigRow {
    pub label: Vec<u8>,
    pub choices: Vec<Vec<u8>>,
    pub help: Vec<Vec<u8>>,
}

/// Top-level entry ids from this one up are the caller's own choices
/// (Rematch, a vote's options): picking one hands it back in
/// `DuelPages::picked`.
pub const CALLER_ENTRY: u8 = 0x40;

/// Pages a caller puts on the Config screen in place of the exe's and the
/// settings' (the LAN duel's screens): rows of choices the caller owns. The
/// camp shows them as Config shows its own and hands back the Left / Right
/// steps; the caller applies them and updates the rows, and the camp
/// redraws what changed. The top level's entries can be the caller's
/// choices too, and their help lines change when the caller changes them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DuelPages {
    /// Who opened the camp and which version of its screen (the caller's).
    pub owner: (u8, u32),
    pub names: Vec<Vec<u8>>,
    pub rows: Vec<Vec<DuelRow>>,
    /// Left / Right on a row that is not locked: (page, row, forward), for
    /// the caller to take.
    pub steps: Vec<(usize, usize, bool)>,
    /// A caller entry picked on the top level (CALLER_ENTRY and up).
    pub picked: Option<u8>,
    /// The top level's Cancel and Start do not close the camp.
    pub keep_open: bool,
}

/// One caller row: label, choices (game text), the chosen one, help, and
/// whether this player may change it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DuelRow {
    pub label: Vec<u8>,
    pub choices: Vec<Vec<u8>>,
    pub selected: usize,
    pub help: Vec<u8>,
    pub locked: bool,
}

/// Everything the camp shows besides its art.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Data {
    pub entries: Vec<Entry>,
    pub panels: Panels,
    pub status: Vec<StatusEntry>,
    /// The 0x18 banner ("Switch", 0xb61a).
    pub banner: Vec<u8>,
    /// 8 rows; row 7 (Game Mode) lists all four modes.
    pub config: Vec<ConfigRow>,
    /// save+0x41a4 and save+0x2498.
    pub config_flags: u32,
    pub game_mode: u32,
    pub items: items::ItemsData,
    pub abilities: abilities::AbilitiesData,
    /// Party help: 0xbb88 "There are no Party Members to switch.", 0x846b
    /// "Select Party Member.", 0x846c "Switch Party Member."
    pub party_help: [Vec<u8>; 3],
    pub customize: customize::CustomizeData,
    pub equip: equip::EquipData,
    /// 03system.bin `ipic`: the item pictures' sequence (FUN_1403b0c90).
    pub picture_sequence: Option<Sequence>,
    /// The tutorial notices' flags, texts and callouts.
    pub notice: notice::NoticeData,
    /// The rebuild's settings pages after the exe's Config (None: only
    /// the exe's page); the caller applies them as they change.
    pub settings: Option<crate::settings::Settings>,
    /// The language codes the install has (the Text Language choices).
    pub languages: Vec<String>,
    /// The caller's pages on the Config screen (None: the exe's and the
    /// settings' pages).
    pub duel: Option<DuelPages>,
}

/// Cursor memory kept between camp openings (DAT_140beec30 / 7c).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    /// Top-level entry id.
    pub entry: u8,
    /// Status list bit.
    pub status_bit: u8,
    pub config_row: usize,
    /// The Config page (0: the exe's; 1.. the settings pages).
    pub config_page: usize,
    /// mem[1]: the Items row; mem[10]: the stock cursor (in page) and scroll.
    pub items_row: usize,
    pub stock: (usize, usize),
    /// mem[0xf] / mem[0x10]: the Abilities character row; the ability
    /// list's cursor and scroll.
    pub ability_char: usize,
    pub ability_list: (usize, usize),
    /// mem[0x19] / mem[0x1a]: the Customize character row; the settings
    /// list's cursor and scroll.
    pub customize_char: usize,
    pub customize_list: (usize, usize),
    /// mem[2]: the Equip slot list's cursor and scroll.
    pub equip: (usize, usize),
    /// save+0x4270: the notices already seen (bit per `notice::ROWS`).
    pub notices_seen: u16,
}

/// Tz::MenuObj's pastes: priority and (member, layout offset, alpha).
pub type ModelLayer = (u32, Vec<(usize, [f32; 2], f32)>);

/// What a draw item samples: the camp sheet, the system font, or member
/// `i`'s off-screen model render (Tz::MenuObj's paste, prio 0x7538).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sheet {
    Art,
    Font,
    /// The font image's icon plane (0x09 icons).
    Icon,
    Model(usize),
    /// `itempic/item-%03d.imd`.
    Picture(u16),
    /// A notice file (`menu/<lang>/<name>_<v>.bar`).
    Notice(String),
}

/// The menu model's layout offset for member `i` of `n` (FUN_140306400):
/// x {47, 145, 243}[i] - (98n - 98)/n, y 96 (61 in world 10): the shift of
/// the projection centre from the layout centre (camp-03 §1.4).
pub fn model_offset(i: usize, n: usize) -> [f32; 2] {
    let n = n.clamp(1, 3) as i32;
    let x = [47, 145, 243][i.min(2)] - (98 * n - 98) / n;
    [x as f32, 96.]
}

/// Motion time of the menu walk (FUN_1403001b0): t += 1, past the clip's
/// length it drops by the length, so it runs 1..=length.
pub fn model_time(t: f32, length: f32) -> f32 {
    let t = t + 1.;
    if t > length { t - length } else { t }
}

const PRIO_MODEL: u32 = 0x7538;
/// The paste tint (FUN_1402ff530: 0x60 = 0.75 of 0x80).
pub const MODEL_TINT: f32 = 0x60 as f32 / 128.;

/// Camp states (DAT_140bee64c).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Top,
    /// 1: members + Stock; 10: the stock grid; 11: Use / Throw away; 12:
    /// the use target; 13: the "no effect" box; 14: the throw-away box.
    Items,
    Stock,
    ItemCommand,
    UseTarget,
    NoEffect,
    ThrowAway,
    /// 0xf characters, 0x10 ability list, 0x11–0x13 the Equip popup of
    /// category 0 Action / 1 Growth / 2 Support.
    AbilityChars,
    AbilityList,
    AbilityPopup(u8),
    /// 0x14: pick a member; 0x16: pick the one to swap with.
    Party,
    PartySwap,
    /// 0x19 characters, 0x1a Sora's settings, 0x1d–0x1f a choice popup of
    /// category 0 shortcut / 1 MP Charge / 2 Auto-Reload, 0x1c the
    /// Auto-Reload warning.
    CustomizeChars,
    CustomizeList,
    CustomizePopup(u8),
    CustomizeConfirm,
    /// 2: a member's slot list; 5–9 the pick list of section 0 weapon /
    /// 1 form / 2 armor / 3 accessory / 4 items; 3 the auto-reload box; 4
    /// the accessory AP box.
    Equip,
    EquipPick(u8),
    EquipReload,
    EquipApBox,
    StatusList,
    StatusBrowse,
    Config,
    /// 0x25: the tutorial notices.
    Notice,
}

/// FUN_1403adcc0: ASCII into the font's encoding (digits, upper-case
/// letters and the listed marks; anything else is the 0x7f blank).
pub fn encode_number(s: &str) -> Vec<u8> {
    s.bytes()
        .map(|b| match b {
            b'0'..=b'9' => 0x21 + (b - b'0'),
            b'A'..=b'Z' => 0x2e + (b - b'A'),
            b' ' => 0x01,
            b'!' => 0x48,
            b'"' => 0x61,
            b'%' => 0x4a,
            b'\'' => 0x57,
            b'*' => 0x2d,
            b'+' => 0x2b,
            b'-' => 0x2c,
            b'.' => 0x4f,
            b'/' => 0x4b,
            b':' => 0x52,
            b'?' => 0x49,
            _ => 0x7f,
        })
        .collect()
}

/// ASCII text into the system font's encoding: `encode_number`'s set plus
/// lower case 0x9a..0xb3 and ',' 0x50 (OpenKH's InternationalSystemDecode
/// table); anything else is the 0x01 space.
pub fn encode_text(s: &str) -> Vec<u8> {
    s.bytes()
        .map(|b| match b {
            b'a'..=b'z' => 0x9a + (b - b'a'),
            b',' => 0x50,
            _ => match encode_number(&char::from(b).to_string())[0] {
                0x7f => 0x01,
                e => e,
            },
        })
        .collect()
}

/// Config values from save+0x41a4 / +0x2498 (FUN_140365580, camp-04 §3.1).
pub fn config_values(flags: u32, mode: u32) -> [usize; 8] {
    let bit = |b: u32| (flags >> b & 1) as usize;
    let summon = match flags >> 9 & 3 {
        2 => 0,
        1 | 3 => 1,
        _ => 2,
    };
    [
        bit(4),
        bit(8),
        bit(7),
        summon,
        1 - bit(3),
        1 - bit(0),
        bit(6),
        mode.min(3) as usize,
    ]
}

/// FUN_140365460: the values back into save+0x41a4.
pub fn config_flags(flags: u32, v: &[usize; 8]) -> u32 {
    let set = |f: u32, b: u32, on: bool| if on { f | 1 << b } else { f & !(1 << b) };
    let mut f = flags;
    f = set(f, 4, v[0] == 1);
    f = set(f, 8, v[1] == 1);
    f = set(f, 7, v[2] == 1);
    f = (f & !(3 << 9)) | ([2u32, 1, 0][v[3].min(2)] << 9);
    f = set(f, 3, v[4] == 0);
    f = set(f, 0, v[5] == 0);
    set(f, 6, v[6] == 1)
}

/// The list object (0x48 B, FUN_1402f7720) and its input FUN_1402f9440.
#[derive(Clone, Debug, Default)]
struct MenuList {
    rows: Vec<Id>,
    /// Number of usable rows (the rest are hidden).
    total: usize,
    cursor: usize,
    /// Enable mask (+0x40); 0x73 by default.
    mask: u32,
    /// Page keys (+0x27) with this page size.
    page: Option<usize>,
    /// FUN_1402f8eb0(list, prev, next): the bits that step back / forward
    /// (None = UP / DOWN; the dialog choices use RIGHT / LEFT, the Party
    /// cards LEFT / RIGHT).
    keys: Option<(u64, u64)>,
    /// +0x20: rows the cursor steps over.
    skip: u32,
}

/// A Config row as shown (`Camp::config_view`).
struct ConfigView {
    label: Vec<u8>,
    shown: Vec<Vec<u8>>,
    selected: usize,
    help: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pick {
    None,
    Row(usize),
    Cancel,
    Start,
    /// −5 / −6 / −7 (SQUARE, TRIANGLE, SELECT).
    Square,
    Triangle,
    Select,
    Next,
    Previous,
    Left,
    Right,
}

struct Pad {
    trig: u64,
    repeat: u64,
}

impl MenuList {
    fn new(rows: Vec<Id>, cursor: usize) -> Self {
        let total = rows.len();
        Self {
            rows,
            total,
            cursor: cursor.min(total.saturating_sub(1)),
            mask: 0x73,
            page: None,
            keys: None,
            skip: 0,
        }
    }

    fn input(&mut self, p: &Pad) -> (Pick, bool) {
        use bits::*;
        let n = self.total;
        if n == 0 {
            return (Pick::None, false);
        }
        let (up, down) = self.keys.unwrap_or((UP, DOWN));
        let skipped = |c: usize| c < 32 && self.skip >> c & 1 != 0;
        let before = self.cursor;
        if p.repeat & up != 0 && self.mask & 0x1 != 0 {
            for _ in 0..n {
                if self.cursor > 0 {
                    self.cursor -= 1;
                } else if p.trig & up != 0 {
                    self.cursor = n - 1;
                } else {
                    self.cursor = before;
                    break;
                }
                if !skipped(self.cursor) {
                    break;
                }
            }
        } else if p.repeat & down != 0 && self.mask & 0x2 != 0 {
            for _ in 0..n {
                if self.cursor + 1 < n {
                    self.cursor += 1;
                } else if p.trig & down != 0 {
                    self.cursor = 0;
                } else {
                    self.cursor = before;
                    break;
                }
                if !skipped(self.cursor) {
                    break;
                }
            }
        } else if let Some(page) = self.page {
            if p.repeat & L1 != 0 && self.mask & 0x2000 != 0 {
                if self.cursor > 0 {
                    self.cursor = self.cursor.saturating_sub(page);
                } else if p.trig & L1 != 0 {
                    self.cursor = n - 1;
                }
            } else if p.repeat & R1 != 0 && self.mask & 0x1000 != 0 {
                if self.cursor + 1 < n {
                    self.cursor = (self.cursor + page).min(n - 1);
                } else if p.trig & R1 != 0 {
                    self.cursor = 0;
                }
            }
        }
        if self.cursor != before {
            return (Pick::None, true);
        }
        let pick = if p.trig & LEFT != 0 && self.mask & 0x4 != 0
            || p.repeat & LEFT != 0 && self.mask & 0x4_0000 != 0
        {
            Pick::Left
        } else if p.trig & RIGHT != 0 && self.mask & 0x8 != 0
            || p.repeat & RIGHT != 0 && self.mask & 0x8_0000 != 0
        {
            Pick::Right
        } else if p.trig & DECIDE != 0 && self.mask & 0x10 != 0 {
            Pick::Row(self.cursor)
        } else if p.trig & CANCEL != 0 && self.mask & 0x20 != 0 {
            Pick::Cancel
        } else if p.trig & START != 0 && self.mask & 0x40 != 0 {
            Pick::Start
        } else if let Some(b) = buttons(p, self.mask) {
            b
        } else if p.trig & R2 != 0 && self.mask & 0x4000 != 0 {
            Pick::Next
        } else if p.trig & L2 != 0 && self.mask & 0x8000 != 0 {
            Pick::Previous
        } else {
            Pick::None
        };
        (pick, false)
    }
}

/// FUN_1402f9440's −5 / −6 / −7 (trig 0x800 / 0x1000 / 0x2000 with mask
/// 0x100 / 0x200 / 0x400).
fn buttons(p: &Pad, mask: u32) -> Option<Pick> {
    use bits::*;
    if p.trig & SQUARE != 0 && mask & 0x100 != 0 {
        Some(Pick::Square)
    } else if p.trig & TRIANGLE != 0 && mask & 0x200 != 0 {
        Some(Pick::Triangle)
    } else if p.trig & SELECT != 0 && mask & 0x400 != 0 {
        Some(Pick::Select)
    } else {
        None
    }
}

/// FUN_1402f6270: the sound for a list result; `valid` is the validator.
fn pick_sound(pick: Pick, moved: bool, valid: bool) -> Option<u32> {
    match pick {
        _ if moved => Some(SOUND_CURSOR),
        Pick::Left | Pick::Right => Some(SOUND_CURSOR),
        Pick::Cancel | Pick::Start => Some(SOUND_CLOSE),
        Pick::Row(_)
        | Pick::Next
        | Pick::Previous
        | Pick::Square
        | Pick::Triangle
        | Pick::Select => Some(if valid { SOUND_DECIDE } else { SOUND_INVALID }),
        Pick::None => None,
    }
}

/// A title tab with its orb (FUN_140323c60).
struct Tab {
    id: Id,
    light: LightCur,
}

pub struct Camp {
    pub layout: LayoutTask,
    pub sprites: Sprites,
    pub data: Data,
    pub memory: Memory,
    pub state: State,
    /// 0 opening, 1 interactive, 2 leaving for `pending`.
    pub substate: u8,
    pending: Option<State>,
    pub exit: bool,
    pub closed: bool,
    /// Set when Jiminy's (1) or Roxas's (2) Journal was decided: the camp
    /// leaves under the curtain and the journal runs next (FUN_1402e4930 /
    /// FUN_14031ad20).
    pub journal: Option<u8>,
    // Top level.
    top: MenuList,
    frame_sprite: Option<Id>,
    prompt: Option<Id>,
    munny: Id,
    time: Id,
    munny_digits: Vec<Player>,
    time_digits: Vec<Player>,
    status_block: Option<Id>,
    /// Tz::MenuObj: the members' walk time while the party frame lives.
    pub model_time: Option<f32>,
    // Shared.
    light: LightCur,
    glove: Option<Id>,
    help: Option<Id>,
    help_offset: [f32; 2],
    tabs: [Option<Tab>; 3],
    /// Text for a tab not created yet (FUN_140324480 before FUN_140323c60).
    pending_tab_text: [Option<Vec<u8>>; 3],
    /// Sprites to close on exit, with their close groups.
    closers: Vec<(Id, usize)>,
    // Status.
    status_list: MenuList,
    /// Status entry index per list row.
    status_rows: Vec<usize>,
    panel: Option<Id>,
    labels: Vec<Id>,
    values: Vec<Id>,
    columns: [MenuList; 2],
    column: usize,
    banner: Option<Id>,
    // Config.
    config_values: [usize; 8],
    /// The duel rows the Config screen last showed (its refresh).
    duel_seen: Vec<DuelRow>,
    config_list: MenuList,
    choices: Vec<Vec<Id>>,
    config_frame: Option<Id>,
    config_labels: Vec<Id>,
    config_page: usize,
    // Items.
    item_ui: items::Ui,
    // Abilities.
    ability_ui: abilities::Ui,
    // Party.
    party_ui: party::Ui,
    // Customize.
    custom_ui: customize::Ui,
    // Equip.
    equip_ui: equip::Ui,
    /// The item picture and the images the caller has loaded for it.
    picture: picture::Picture,
    pub pictures: picture::PictureCache,
    // Notices.
    notice: notice::Ui,
    pub notice_files: notice::NoticeCache,
}

impl Camp {
    /// FUN_140306040 (and the open sound the task plays right after).
    pub fn open(layout: &Layout, data: Data, memory: Memory) -> (Self, Vec<u32>) {
        let seq = &layout.sequences[SEQUENCE];
        let mut task = LayoutTask::new(layout, SCENE_OPEN);
        task.next = Some(SCENE_IDLE);
        let mut sp = Sprites::default();
        let munny = sp.create(seq, PRIO_PANEL, ART, MUNNY[0], Some(MUNNY[1]), None);
        let time = sp.create(seq, PRIO_PANEL, ART, TIME[0], Some(TIME[1]), None);
        let mut camp = Camp {
            layout: task,
            sprites: sp,
            config_values: config_values(data.config_flags, data.game_mode),
            duel_seen: Vec::new(),
            data,
            memory,
            state: State::Top,
            substate: 0,
            pending: None,
            exit: false,
            closed: false,
            journal: None,
            top: MenuList::default(),
            frame_sprite: None,
            prompt: None,
            munny,
            time,
            munny_digits: Vec::new(),
            time_digits: Vec::new(),
            status_block: None,
            model_time: None,
            light: LightCur::default(),
            glove: None,
            help: None,
            help_offset: [0.; 2],
            tabs: [None, None, None],
            pending_tab_text: [None, None, None],
            closers: vec![(munny, MUNNY[2]), (time, TIME[2])],
            status_list: MenuList::default(),
            status_rows: Vec::new(),
            panel: None,
            labels: Vec::new(),
            values: Vec::new(),
            columns: [MenuList::default(), MenuList::default()],
            column: 0,
            banner: None,
            config_list: MenuList::default(),
            choices: Vec::new(),
            config_frame: None,
            config_labels: Vec::new(),
            config_page: 0,
            item_ui: items::Ui::default(),
            ability_ui: abilities::Ui::default(),
            party_ui: party::Ui::default(),
            custom_ui: customize::Ui::default(),
            equip_ui: equip::Ui::default(),
            picture: picture::Picture::default(),
            pictures: picture::PictureCache::default(),
            notice: notice::Ui::default(),
            notice_files: notice::NoticeCache::default(),
        };
        camp.build_top(seq);
        // FUN_140306790: pending notices open over the camp.
        camp.start_notices(seq, None, State::Top);
        camp.munny_digits = digits(seq, &munny_groups(camp.data.panels.munny), Vec::new());
        let t = time_groups(camp.data.panels.time_frames);
        camp.time_digits = digits(seq, &t, Vec::new());
        (camp, vec![SOUND_OPEN])
    }

    fn row_text(&self, bytes: Vec<u8>) -> Text {
        let mut t = Text::new(bytes, TEXT_SIZE);
        t.group_position = false;
        t.keep_rgb = false;
        t
    }

    /// FUN_140306790 + FUN_140306400 + FUN_140367b50: rows, party frame,
    /// status block and settings prompt (also the rebuild on return).
    fn build_top(&mut self, seq: &Sequence) {
        let cursor = self
            .data
            .entries
            .iter()
            .position(|e| e.id == self.memory.entry)
            .unwrap_or(0);
        let pitch = seq.groups[TOP_ROWS.appear].text_position_x as f32;
        let mut rows = Vec::new();
        for (i, e) in self.data.entries.clone().iter().enumerate() {
            let idle = if i == cursor {
                TOP_ROWS.selected
            } else {
                TOP_ROWS.idle
            };
            let t = self.row_text(e.label.clone());
            let r = self
                .sprites
                .create(seq, PRIO_ROW, ART, TOP_ROWS.appear, Some(idle), Some(t));
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            self.closers.push((r, TOP_ROWS.close));
            rows.push(r);
        }
        // FUN_140306790: a badge on the row of an entry with something
        // new, pulsing (50) until all of it is seen, then dim (49). The
        // Items row asks FUN_14036a8f0(−1) / FUN_14036a7f0(−1).
        let g = &seq.groups[TOP_ROWS.appear];
        let badge_at = [g.light_position_x as f32, g.text_position_y as f32];
        for (i, e) in self.data.entries.clone().iter().enumerate() {
            let Some(all_seen) = self.top_badge(e.id) else {
                continue;
            };
            let b = self.sprites.create(
                seq,
                PRIO_ROW_BADGE,
                ART,
                items::BADGE[all_seen as usize],
                None,
                None,
            );
            self.child(b, rows[i]);
            self.sprites.get_mut(b).offset = badge_at;
        }
        self.top = MenuList::new(rows, cursor);
        let frame = self
            .sprites
            .create(seq, PRIO_FRAME, ART, FRAME[0], Some(FRAME[1]), None);
        self.closers.push((frame, FRAME[2]));
        self.frame_sprite = Some(frame);
        // FUN_1402feb20: the members are added with the frame.
        self.model_time = Some(0.);
        if self.prompt.is_none_or(|p| !self.sprites.alive(p)) {
            let p = self
                .sprites
                .create(seq, PRIO_FRAME, ART, PROMPT[0], Some(PROMPT[1]), None);
            self.closers.push((p, PROMPT[2]));
            self.prompt = Some(p);
        }
        self.status_block = Some(self.build_status_block(seq));
        self.state = State::Top;
        self.substate = 0;
    }

    /// Whether the top entry `id` shows a NEW! badge: `Some(all seen)`.
    fn top_badge(&self, id: u8) -> Option<bool> {
        match id {
            0 => {
                let new = self.new_stock_items();
                (!new.is_empty()).then(|| new.iter().all(|i| self.data.items.seen.contains(i)))
            }
            _ => None,
        }
    }

    fn build_status_block(&mut self, seq: &Sequence) -> Id {
        let n = self.data.panels.members.len();
        let pitch = seq.groups[STATUS[0]].text_position_x as f32;
        self.status_block_with(seq, STATUS, n, |i| [pitch * (n - i) as f32, 0.])
    }

    /// FUN_140367b50 with the value mode 0 (LV/HP/MP): the container in
    /// `groups` (appear, idle, close) and the first `count` members at
    /// `place(i)`.
    fn status_block_with(
        &mut self,
        seq: &Sequence,
        groups: [usize; 3],
        count: usize,
        place: impl Fn(usize) -> [f32; 2],
    ) -> Id {
        let sp = &mut self.sprites;
        let status = sp.create(seq, PRIO_PANEL, ART, groups[0], Some(groups[1]), None);
        self.closers.push((status, groups[2]));
        let mut panels = self.data.panels.clone();
        panels.members.truncate(count);
        let sub_size = seq.groups[SUB_ANCHOR].text_scale as f32;
        let row_pitch = seq.groups[SUB_ANCHOR].ui_padding as f32;
        let value_x = seq.groups[SUB_ANCHOR].light_position_x as f32;
        let child = |sp: &mut Sprites, id: Id, parent: Id| {
            sp.attach(id, parent);
            sp.get_mut(id).die_with_parent = true;
        };
        for (i, m) in panels.members.iter().enumerate() {
            let mut name = Text::new(m.name.clone(), TEXT_SIZE);
            name.align = 2;
            name.group_position = false;
            let block = sp.create(seq, PRIO_ROW, ART, BLOCK, None, Some(name));
            child(sp, block, status);
            sp.get_mut(block).offset = place(i);
            let sub = sp.create(seq, PRIO_ROW, ART, SUB_ANCHOR, None, None);
            child(sp, sub, block);
            let values = [
                format!("{}", m.level),
                format!("{}/{}", m.hp, m.max_hp),
                format!("{}/{}", m.mp, m.max_mp),
            ];
            for k in 0..3 {
                let y = row_pitch * k as f32;
                let text = |bytes: Vec<u8>, align: u8| {
                    let mut t = Text::new(bytes, sub_size);
                    t.align = align;
                    t.group_position = false;
                    t.keep_rgb = false;
                    t
                };
                let label = sp.create(
                    seq,
                    PRIO_ROW,
                    ART,
                    STYLES[k],
                    None,
                    Some(text(panels.labels[k].clone(), 0)),
                );
                child(sp, label, sub);
                sp.get_mut(label).offset = [0., y];
                let value = sp.create(
                    seq,
                    PRIO_ROW,
                    ART,
                    STYLES[k],
                    None,
                    Some(text(encode_number(&values[k]), 1)),
                );
                child(sp, value, sub);
                sp.get_mut(value).offset = [value_x, y];
                if k > 0 {
                    let f = sp.create(seq, PRIO_ROW, ART, GAUGE_FRAME, None, None);
                    child(sp, f, sub);
                    sp.get_mut(f).offset = [0., y];
                    let (now, max) = if k == 1 {
                        (m.hp, m.max_hp)
                    } else {
                        (m.mp, m.max_mp)
                    };
                    let bar = sp.create(seq, PRIO_BAR, ART, GAUGE_BARS[k - 1], None, None);
                    child(sp, bar, sub);
                    let s = sp.get_mut(bar);
                    s.offset = [0., y];
                    s.scale = [now as f32 / max.max(1) as f32, 1.];
                }
            }
        }
        status
    }

    /// One camp frame (the camp task runs before the sprite tasks): the
    /// state machine with this frame's presses and repeats (logical, mode
    /// 7), the driver, then every sprite's update.
    pub fn frame(&mut self, layout: &Layout, trig: u64, repeat: u64, time_frames: u32) -> Vec<u32> {
        let seq = &layout.sequences[SEQUENCE];
        let pad = Pad { trig, repeat };
        let mut sounds = Vec::new();
        self.duel_refresh(seq);
        if !self.exit {
            // FUN_1403249e0: the clock rebuilds every frame.
            let groups = time_groups(time_frames);
            self.time_digits = digits(seq, &groups, std::mem::take(&mut self.time_digits));
            // No input while the layout opens; any press skips it to idle.
            let gated = !self.layout.dead && self.layout.scene != SCENE_IDLE;
            if gated && trig != 0 {
                self.layout.set_scene(SCENE_IDLE, None);
            }
            if !gated || trig != 0 {
                let moved = match self.state {
                    State::Top => self.top_level(seq, &pad, &mut sounds),
                    State::Items => self.items(seq, &pad, &mut sounds),
                    State::Stock => self.stock(seq, &pad, &mut sounds),
                    State::ItemCommand => self.item_command(seq, &pad, &mut sounds),
                    State::UseTarget => self.use_target(seq, &pad, &mut sounds),
                    State::NoEffect => self.no_effect(seq, &pad, &mut sounds),
                    State::ThrowAway => self.throw_away(seq, &pad, &mut sounds),
                    State::AbilityChars => self.ability_chars(seq, &pad, &mut sounds),
                    State::AbilityList => self.ability_list(seq, &pad, &mut sounds),
                    State::AbilityPopup(_) => self.ability_popup(seq, &pad, &mut sounds),
                    State::Party => self.party(seq, &pad, &mut sounds),
                    State::PartySwap => self.party_swap(seq, &pad, &mut sounds),
                    State::CustomizeChars => self.customize_chars(seq, &pad, &mut sounds),
                    State::CustomizeList => self.customize_list(seq, &pad, &mut sounds),
                    State::CustomizePopup(_) => self.customize_popup(seq, &pad, &mut sounds),
                    State::CustomizeConfirm => self.customize_confirm(seq, &pad, &mut sounds),
                    State::Equip => self.equip(seq, &pad, &mut sounds),
                    State::EquipPick(_) => self.equip_pick(seq, &pad, &mut sounds),
                    State::EquipReload => self.equip_reload_box(seq, &pad, &mut sounds),
                    State::EquipApBox => self.equip_ap_box(seq, &pad, &mut sounds),
                    State::StatusList => self.status_list(seq, &pad, &mut sounds),
                    State::StatusBrowse => self.status_browse(seq, &pad, &mut sounds),
                    State::Config => self.config(seq, &pad, &mut sounds),
                    State::Notice => self.notice(layout, &pad, &mut sounds),
                };
                self.driver(seq, moved);
            }
        }
        if self.exit {
            self.close(seq);
        }
        self.layout.update(layout, 1.);
        self.sprites.update(seq);
        self.notice_update(layout);
        if let Some(ipic) = &self.data.picture_sequence {
            self.picture.update(ipic, &self.sprites);
        }
        for d in self
            .munny_digits
            .iter_mut()
            .chain(self.time_digits.iter_mut())
        {
            d.update(seq, 1.);
        }
        self.place_help(layout);
        self.place_lights(seq);
        // FUN_1403001b0: the walk advances; the models go with their host
        // (the party frame, or Party's name cards).
        if self.models_hosted() {
            self.model_time = self.model_time.map(|t| model_time(t, MODEL_LENGTH));
        } else {
            self.model_time = None;
        }
        if self.exit && self.layout.dead {
            self.closed = true;
        }
        sounds
    }

    fn rows_of(&self, state: State) -> (&MenuList, RowGroupsFor) {
        let ui = &self.item_ui;
        match state {
            State::Top => (&self.top, RowGroupsFor::Top),
            State::Items => (&ui.list, RowGroupsFor::Top),
            State::Stock => (&ui.grid.list, RowGroupsFor::Fixed(items::STOCK_ROWS)),
            State::ItemCommand => (&ui.commands, RowGroupsFor::Fixed(items::COMMAND_ROWS)),
            State::UseTarget => (&ui.targets, RowGroupsFor::Fixed(items::COMMAND_ROWS)),
            State::NoEffect | State::ThrowAway => {
                (&ui.choices, RowGroupsFor::Fixed(items::CHOICE_ROWS))
            }
            State::AbilityChars => (&self.ability_ui.chars, RowGroupsFor::Top),
            State::AbilityList => (&self.ability_ui.list.list, RowGroupsFor::None),
            State::AbilityPopup(_) => (&self.ability_ui.popup, RowGroupsFor::None),
            State::Party => (&self.party_ui.cards, RowGroupsFor::None),
            State::PartySwap => (&self.party_ui.swap, RowGroupsFor::None),
            State::CustomizeChars => (&self.custom_ui.chars, RowGroupsFor::Top),
            State::CustomizeList => (&self.custom_ui.list.list, RowGroupsFor::None),
            State::CustomizePopup(_) => (&self.custom_ui.popup.list, RowGroupsFor::None),
            State::CustomizeConfirm | State::EquipReload | State::EquipApBox => {
                (&ui.choices, RowGroupsFor::Fixed(items::CHOICE_ROWS))
            }
            State::Equip => (&self.equip_ui.list.list, RowGroupsFor::None),
            State::EquipPick(_) => (&self.equip_ui.pick.list, RowGroupsFor::None),
            State::StatusList => (&self.status_list, RowGroupsFor::Status),
            State::StatusBrowse => (&self.columns[self.column], RowGroupsFor::None),
            State::Config => (&self.config_list, RowGroupsFor::None),
            State::Notice => (&self.top, RowGroupsFor::None),
        }
    }

    fn groups_for(&self, kind: RowGroupsFor, row: usize) -> Option<RowGroups> {
        match kind {
            RowGroupsFor::Top => Some(TOP_ROWS),
            RowGroupsFor::Status => {
                let bit = self.data.status[self.status_rows[row]].bit;
                Some(if bit == 0 || bit > 6 {
                    TOP_ROWS
                } else {
                    FORM_ROWS
                })
            }
            RowGroupsFor::Fixed(g) => Some(g),
            RowGroupsFor::None => None,
        }
    }

    /// FUN_140303cd0: the glove and LightCur follow the active list; on a
    /// cursor move the rows restart idle and the cursor row its selected
    /// group.
    fn driver(&mut self, seq: &Sequence, moved: bool) {
        let state = self.state;
        let (list, kind) = self.rows_of(state);
        let rows = list.rows.clone();
        let cursor = list.cursor;
        let live = rows.iter().any(|&r| self.sprites.alive(r));
        // DAT_140beec22: an empty stock list has no pointer.
        if self.substate == 1 && live && self.item_ui.no_pointer {
            if let Some(g) = self.glove.take() {
                self.sprites.kill(g);
            }
            self.light.kill(&mut self.sprites);
        } else if self.substate == 1 && live {
            if moved {
                for (i, &r) in rows.iter().enumerate() {
                    if let Some(g) = self.groups_for(kind, i) {
                        let group = if i == cursor { g.selected } else { g.idle };
                        self.sprites.set(seq, r, group, None);
                    }
                }
                self.light.retarget();
            }
            let row = rows[cursor];
            // States of list 0x1405baf20 (the stock list here) use G22.
            let group = if matches!(
                state,
                State::Stock
                    | State::AbilityPopup(_)
                    | State::CustomizePopup(_)
                    | State::EquipPick(_)
            ) {
                22
            } else {
                GLOVE
            };
            match self.glove {
                Some(g) if self.sprites.alive(g) => self.sprites.get_mut(g).parent = Some(row),
                _ => {
                    let g = self.sprites.create(seq, PRIO_LIGHT, ART, group, None, None);
                    self.sprites.get_mut(g).parent = Some(row);
                    self.glove = Some(g);
                }
            }
            if self.light.id.is_none_or(|l| !self.sprites.alive(l)) {
                let l = self
                    .sprites
                    .create(seq, PRIO_LIGHT, ART, LIGHT[0], Some(LIGHT[1]), None);
                self.light = LightCur::new(l);
            }
        } else if self.substate == 2 {
            if let Some(g) = self.glove.take() {
                self.sprites.kill(g);
            }
            if let Some(l) = self.light.id
                && self.sprites.alive(l)
                && self.sprites.get(l).player.group != LIGHT[2]
            {
                self.sprites.close(seq, l, LIGHT[2]);
            }
        }
    }

    /// The LightCur on the active row (0x24: on its selected choice) and
    /// each tab's orb on its tab, after the sprite update.
    fn place_lights(&mut self, seq: &Sequence) {
        let target_of = |sp: &Sprites, id: Id| {
            let c = sp.context(id);
            let g = &seq.groups[sp.get(id).player.group];
            [
                c[0] + g.light_position_x as f32,
                c[1] + g.text_position_y as f32,
            ]
        };
        if self.light.id.is_some_and(|l| self.sprites.alive(l)) {
            let (list, _) = self.rows_of(self.state);
            if let Some(&row) = list.rows.get(list.cursor) {
                let anchor = if self.state == State::Config {
                    self.sprites.get(row).parent.unwrap_or(row)
                } else {
                    row
                };
                if self.sprites.alive(anchor) {
                    let t = target_of(&self.sprites, anchor);
                    self.light.place(&mut self.sprites, t);
                }
            }
        }
        for tab in self.tabs.iter_mut().flatten() {
            if self.sprites.alive(tab.id) {
                let t = target_of(&self.sprites, tab.id);
                tab.light.place(&mut self.sprites, t);
            }
        }
    }

    // ---- top level -------------------------------------------------------

    /// FUN_1403037d0.
    fn top_level(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let prev = self.top.cursor;
        let (pick, moved) = self.top.input(pad);
        if self.substate == 2 {
            self.wait_top(seq, pick, moved);
            return false;
        }
        let row0 = self
            .top
            .rows
            .first()
            .map(|&r| self.sprites.get(r).player.group);
        if (pick != Pick::None || moved) && row0 == Some(TOP_ROWS.appear) {
            // FUN_1402f81a0: the rows skip their intros.
            for (i, &r) in self.top.rows.clone().iter().enumerate() {
                let g = if i == prev {
                    TOP_ROWS.selected
                } else {
                    TOP_ROWS.idle
                };
                self.sprites.set(seq, r, g, None);
            }
        }
        let row0 = self
            .top
            .rows
            .first()
            .map(|&r| self.sprites.get(r).player.group);
        if self.substate == 0
            && matches!(row0, Some(g) if g == TOP_ROWS.idle || g == TOP_ROWS.selected)
        {
            let help = self.data.entries[self.top.cursor].help.clone();
            self.show_help(seq, help);
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            let help = self.data.entries[self.top.cursor].help.clone();
            self.show_help(seq, help);
        }
        match pick {
            Pick::Cancel | Pick::Start if self.keep_open() => {}
            Pick::Cancel | Pick::Start => {
                self.memory.entry = self.data.entries[self.top.cursor].id;
                self.exit = true;
            }
            Pick::Row(i) if self.data.entries[i].id >= CALLER_ENTRY => {
                // The caller's choice goes back to it; the camp stays.
                let id = self.data.entries[i].id;
                if let Some(d) = self.data.duel.as_mut() {
                    d.picked = Some(id);
                }
            }
            Pick::Row(i) => {
                let entry = self.data.entries[i].clone();
                self.memory.entry = entry.id;
                let pending = match entry.id {
                    0 => Some(State::Items),
                    1 => Some(State::AbilityChars),
                    2 => Some(State::CustomizeChars),
                    3 => Some(State::Party),
                    4 => Some(State::StatusList),
                    7 => Some(State::Config),
                    _ => None,
                };
                if let Some(p) = pending {
                    self.leave_top(seq, p, entry.label);
                } else if matches!(entry.id, 5 | 6) {
                    // The curtain comes down while the camp closes.
                    self.journal = Some(entry.id - 4);
                    self.exit = true;
                }
            }
            _ => {}
        }
        moved
    }

    /// The same-frame hand-over (camp-04 §1.1).
    fn leave_top(&mut self, seq: &Sequence, pending: State, label: Vec<u8>) {
        self.pending = Some(pending);
        self.tab_text(0, label);
        for &r in &self.top.rows.clone() {
            self.sprites.close(seq, r, TOP_ROWS.close);
        }
        self.hide_help(seq);
        if let Some(f) = self.frame_sprite {
            self.sprites.close(seq, f, FRAME[2]);
        }
        if let Some(s) = self.status_block {
            self.sprites.close(seq, s, STATUS[2]);
        }
        if let Some(p) = self.prompt {
            self.sprites.close(seq, p, PROMPT[2]);
        }
        self.substate = 2;
    }

    /// Substate 2: commit once rows and the status block are gone; any
    /// press kills them at once.
    fn wait_top(&mut self, seq: &Sequence, pick: Pick, moved: bool) {
        let rows_dead = self.top.rows.iter().all(|&r| !self.sprites.alive(r));
        let block_dead = self.status_block.is_none_or(|s| !self.sprites.alive(s));
        if rows_dead && block_dead {
            match self.pending.take() {
                Some(State::Items) => self.enter_items(seq, None),
                Some(State::AbilityChars) => self.enter_ability_chars(seq, None),
                Some(State::Party) => self.enter_party(seq),
                Some(State::CustomizeChars) => self.enter_customize_chars(seq),
                Some(State::StatusList) => self.enter_status(seq),
                Some(State::Config) => self.enter_config(seq),
                _ => {}
            }
        } else if pick != Pick::None || moved {
            for &r in &self.top.rows {
                self.sprites.kill(r);
            }
            if let Some(s) = self.status_block {
                self.sprites.kill(s);
            }
        }
    }

    // ---- shared pieces ---------------------------------------------------

    /// FUN_140324480: a tab's text (created by the screen's entry).
    fn tab_text(&mut self, i: usize, bytes: Vec<u8>) {
        if let Some(tab) = &self.tabs[i]
            && let Some(t) = self.sprites.get_mut(tab.id).text.as_mut()
        {
            t.bytes = bytes;
            return;
        }
        self.pending_tab_text[i] = Some(bytes);
    }

    /// FUN_140323c60: tab `i` (G75 → 76) at (120·i, 0) with its orb.
    fn create_tab(&mut self, seq: &Sequence, i: usize) {
        if self.tabs[i]
            .as_ref()
            .is_some_and(|t| self.sprites.alive(t.id))
        {
            return;
        }
        let bytes = self.pending_tab_text[i].take().unwrap_or_default();
        let mut t = Text::new(bytes, TEXT_SIZE);
        t.group_position = false;
        let id = self
            .sprites
            .create(seq, PRIO_ROW, ART, TAB[0], Some(TAB[1]), Some(t));
        self.sprites.get_mut(id).offset =
            [seq.groups[TAB[0]].text_position_x as f32 * i as f32, 0.];
        self.closers.push((id, TAB[2]));
        let l = self
            .sprites
            .create(seq, PRIO_BAR, ART, LIGHT[0], Some(LIGHT[1]), None);
        self.tabs[i] = Some(Tab {
            id,
            light: LightCur::new(l),
        });
    }

    fn close_tab(&mut self, seq: &Sequence, i: usize) {
        if let Some(tab) = &self.tabs[i]
            && self.sprites.alive(tab.id)
        {
            let id = tab.id;
            self.sprites.close(seq, id, TAB[2]);
        }
    }

    fn kill_tab(&mut self, i: usize) {
        if let Some(tab) = self.tabs[i].take() {
            self.sprites.kill(tab.id);
            if let Some(l) = tab.light.id {
                self.sprites.kill(l);
            }
        }
    }

    fn tab_alive(&self, i: usize) -> bool {
        self.tabs[i]
            .as_ref()
            .is_some_and(|t| self.sprites.alive(t.id))
    }

    /// FUN_1402fa1d0: a live help text only swaps its text (no re-fade
    /// while in G116; a fading one restarts G116); otherwise it is created
    /// on the layout element with that element's (LightPositionX,
    /// TextPositionY).
    fn show_help(&mut self, seq: &Sequence, bytes: Vec<u8>) {
        if let Some(h) = self.help
            && self.sprites.alive(h)
        {
            if let Some(t) = self.sprites.get_mut(h).text.as_mut() {
                t.bytes = bytes;
            }
            if self.sprites.get(h).player.group != HELP_IN {
                self.sprites.set(seq, h, HELP_IN, None);
            }
            return;
        }
        let size = seq.groups[HELP_IN].text_scale as f32;
        let mut t = Text::new(bytes, size);
        t.keep_rgb = false;
        let h = self
            .sprites
            .create(seq, PRIO_ROW, ART, HELP_IN, None, Some(t));
        let g = &seq.groups[HELP_PARENTS[1]];
        self.help_offset = [g.light_position_x as f32, g.text_position_y as f32];
        self.help = Some(h);
    }

    /// FUN_1402fa510: the help fades out (G117) and goes.
    fn hide_help(&mut self, seq: &Sequence) {
        if let Some(h) = self.help
            && self.sprites.alive(h)
        {
            self.sprites.close(seq, h, HELP_OUT);
        }
    }

    /// The help's parent is the layout element of the current scene: its
    /// context, with its alpha as the colour multiplier (0x200|0x400).
    fn place_help(&mut self, layout: &Layout) {
        let Some(h) = self.help else { return };
        if self.layout.dead || !self.sprites.alive(h) {
            return;
        }
        let m = Modifiers::default();
        for (prop, player, origin) in self.layout.player.visible(layout, [0, 0]) {
            if HELP_PARENTS.contains(&prop.group) {
                let seq = &layout.sequences[prop.sequence];
                let c = seq.child_context(player, &m).unwrap_or([0.; 2]);
                let a = seq.context_color(player, &m).map_or(1., |c| c[3]);
                let s = self.sprites.get_mut(h);
                s.base = [origin[0] + c[0], origin[1] + c[1]];
                s.offset = self.help_offset;
                s.multiplier = [1., 1., 1., a];
                return;
            }
        }
    }

    fn child(&mut self, id: Id, parent: Id) {
        self.sprites.attach(id, parent);
        self.sprites.get_mut(id).die_with_parent = true;
    }

    /// Back to the top level (camp-04 §1.3): the rebuild plays no sound.
    fn return_top(&mut self, seq: &Sequence) {
        self.kill_tab(0);
        self.kill_tab(1);
        self.kill_tab(2);
        self.build_top(seq);
    }

    // ---- Status 0x17 -----------------------------------------------------

    /// FUN_140353020: tab 0, the character list and the stat panel.
    fn enter_status(&mut self, seq: &Sequence) {
        self.state = State::StatusList;
        self.substate = 0;
        self.create_tab(seq, 0);
        let pitch = seq.groups[TOP_ROWS.appear].text_position_x as f32;
        let cursor = self
            .data
            .status
            .iter()
            .position(|e| e.bit == self.memory.status_bit)
            .unwrap_or(0);
        let mut rows = Vec::new();
        for (i, e) in self.data.status.clone().iter().enumerate() {
            let g = if e.bit == 0 || e.bit > 6 {
                TOP_ROWS
            } else {
                FORM_ROWS
            };
            let queued = if i == cursor { g.selected } else { g.idle };
            let t = self.row_text(e.label.clone());
            let r = self
                .sprites
                .create(seq, PRIO_STATUS_ROW, ART, g.appear, Some(queued), Some(t));
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            self.closers.push((r, g.close));
            rows.push(r);
        }
        self.status_rows = (0..self.data.status.len()).collect();
        self.status_list = MenuList::new(rows, cursor);
        let panel = self.sprites.create(
            seq,
            PRIO_STATUS_PANEL,
            ART,
            STATUS_PANEL[0],
            Some(STATUS_PANEL[1]),
            None,
        );
        self.closers.push((panel, STATUS_PANEL[2]));
        self.panel = Some(panel);
        self.labels.clear();
        self.values.clear();
        let g = &seq.groups[STATUS_LABEL];
        let value_offset = [g.light_position_x as f32, g.text_position_y as f32];
        for _ in 0..20 {
            let mut t = self.row_text(Vec::new());
            t.size = TEXT_SIZE;
            let l = self
                .sprites
                .create(seq, PRIO_STATUS_LABEL, ART, BLANK, None, Some(t));
            self.child(l, panel);
            let mut vt = Text::new(Vec::new(), TEXT_SIZE);
            vt.align = 1;
            vt.group_position = false;
            vt.keep_rgb = false;
            let v = self
                .sprites
                .create(seq, PRIO_STATUS_ROW, ART, VALUE, None, Some(vt));
            self.child(v, l);
            self.sprites.get_mut(v).offset = value_offset;
            self.labels.push(l);
            self.values.push(v);
        }
        self.layout_status_panel(seq);
        let help = self.data.status[cursor].help.clone();
        self.show_help(seq, help);
    }

    /// FUN_1403540b0: labels and values of the cursor character, compacted
    /// per column at (170·col, 24·rank).
    fn layout_status_panel(&mut self, seq: &Sequence) {
        let entry = self.data.status[self.status_rows[self.status_list.cursor]].clone();
        let mut rank = [0usize; 2];
        for k in 0..20 {
            let (l, v) = (self.labels[k], self.values[k]);
            match entry.stats.get(k).cloned().flatten() {
                Some(stat) => {
                    let col = k / 10;
                    let group = STAT_STYLES[stat.style.min(4)].0;
                    let gg = &seq.groups[group];
                    let offset = [
                        gg.text_position_x as f32 * col as f32,
                        gg.ui_padding as f32 * rank[col] as f32,
                    ];
                    rank[col] += 1;
                    self.sprites.set(seq, l, group, None);
                    let s = self.sprites.get_mut(l);
                    s.offset = offset;
                    if let Some(t) = s.text.as_mut() {
                        t.bytes = stat.label;
                    }
                    if let Some(t) = self.sprites.get_mut(v).text.as_mut() {
                        t.bytes = stat.value;
                    }
                }
                None => {
                    self.sprites.set(seq, l, BLANK, None);
                    if let Some(t) = self.sprites.get_mut(l).text.as_mut() {
                        t.bytes.clear();
                    }
                    if let Some(t) = self.sprites.get_mut(v).text.as_mut() {
                        t.bytes.clear();
                    }
                }
            }
        }
    }

    /// FUN_140351db0.
    fn status_list(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let (pick, moved) = self.status_list.input(pad);
        if self.substate == 2 {
            let panel_dead = self.panel.is_none_or(|p| !self.sprites.alive(p));
            let rows_dead = self
                .status_list
                .rows
                .iter()
                .all(|&r| !self.sprites.alive(r));
            let tab_ok = self.pending != Some(State::Top) || !self.tab_alive(0);
            if panel_dead && rows_dead && tab_ok {
                match self.pending.take() {
                    Some(State::StatusBrowse) => self.enter_browse(seq),
                    _ => self.return_top(seq),
                }
            } else if pick != Pick::None || moved {
                if let Some(p) = self.panel {
                    self.sprites.kill(p);
                }
                for &r in &self.status_list.rows {
                    self.sprites.kill(r);
                }
                if self.pending == Some(State::Top) {
                    self.kill_tab(0);
                }
            }
            return false;
        }
        let row0 = self
            .status_list
            .rows
            .first()
            .map(|&r| self.sprites.get(r).player.group);
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == STATUS_PANEL[1]);
        let row0_idle = row0.is_some_and(|g| matches!(g, 134 | 132 | 130 | 128));
        if (pick != Pick::None || moved) && !(row0_idle && panel_idle) {
            self.skip_status_intros(seq);
        }
        let row0 = self
            .status_list
            .rows
            .first()
            .map(|&r| self.sprites.get(r).player.group);
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == STATUS_PANEL[1]);
        if self.substate == 0
            && row0.is_some_and(|g| matches!(g, 134 | 132 | 130 | 128))
            && panel_idle
        {
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        let cursor_entry = self.status_rows[self.status_list.cursor];
        if moved && self.substate == 1 {
            self.layout_status_panel(seq);
            let help = self.data.status[cursor_entry].help.clone();
            self.show_help(seq, help);
        }
        match pick {
            Pick::Start => {
                self.memory.status_bit = self.data.status[cursor_entry].bit;
                self.exit = true;
            }
            Pick::Cancel => {
                self.hide_help(seq);
                self.close_tab(seq, 0);
                self.close_status_list(seq);
                self.memory.status_bit = self.data.status[cursor_entry].bit;
                self.pending = Some(State::Top);
                self.substate = 2;
            }
            Pick::Row(_) => {
                self.hide_help(seq);
                self.close_status_list(seq);
                let label = self.data.status[cursor_entry].label.clone();
                self.pending_tab_text[1] = Some(label);
                self.memory.status_bit = self.data.status[cursor_entry].bit;
                self.pending = Some(State::StatusBrowse);
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }

    fn skip_status_intros(&mut self, seq: &Sequence) {
        let cursor = self.status_list.cursor;
        for (i, &r) in self.status_list.rows.clone().iter().enumerate() {
            if let Some(g) = self.groups_for(RowGroupsFor::Status, i) {
                let group = if i == cursor { g.selected } else { g.idle };
                if self.sprites.get(r).player.group == g.appear {
                    self.sprites.set(seq, r, group, None);
                }
            }
        }
        if let Some(p) = self.panel {
            self.sprites.set(seq, p, STATUS_PANEL[1], None);
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

    fn close_status_list(&mut self, seq: &Sequence) {
        if let Some(p) = self.panel {
            self.sprites.close(seq, p, STATUS_PANEL[2]);
        }
        for (i, &r) in self.status_list.rows.clone().iter().enumerate() {
            if let Some(g) = self.groups_for(RowGroupsFor::Status, i) {
                self.sprites.close(seq, r, g.close);
            }
        }
    }

    // ---- Status 0x18 -----------------------------------------------------

    /// FUN_140352be0 + FUN_140353800: tab 1, the stat browser panel, two
    /// columns of rows with their values, and the "Switch" banner.
    fn enter_browse(&mut self, seq: &Sequence) {
        self.state = State::StatusBrowse;
        self.substate = 0;
        self.create_tab(seq, 1);
        let entry = self.data.status[self.status_rows[self.status_list.cursor]].clone();
        let panel = self.sprites.create(
            seq,
            PRIO_PANEL,
            ART,
            BROWSE_PANEL[0],
            Some(BROWSE_PANEL[1]),
            None,
        );
        self.closers.push((panel, BROWSE_PANEL[2]));
        self.panel = Some(panel);
        let g = &seq.groups[BROWSE_ROW];
        let value_offset = [g.light_position_x as f32, g.text_position_y as f32];
        self.values.clear();
        for c in 0..2 {
            let mut rows = Vec::new();
            for _ in 0..10 {
                let t = self.row_text(Vec::new());
                let r = self
                    .sprites
                    .create(seq, PRIO_ROW, ART, BLANK, None, Some(t));
                self.child(r, panel);
                rows.push(r);
            }
            self.columns[c] = MenuList::new(rows, 0);
            self.columns[c].total = 0;
            self.columns[c].mask = (0x73 & !0x10) | 0xc_c000 | 0x3000;
            self.columns[c].page = Some(10);
        }
        for col in 0..2 {
            let mut j = 0;
            for k in col * 10..col * 10 + 10 {
                let Some(stat) = entry.stats.get(k).cloned().flatten() else {
                    continue;
                };
                let row = self.columns[col].rows[j];
                let group = STAT_STYLES[stat.style.min(4)].1;
                let gg = &seq.groups[group];
                let offset = [
                    gg.text_position_x as f32 * col as f32,
                    gg.ui_padding as f32 * j as f32,
                ];
                self.sprites.set(seq, row, group, None);
                let s = self.sprites.get_mut(row);
                s.offset = offset;
                if let Some(t) = s.text.as_mut() {
                    t.bytes = stat.label.clone();
                }
                let mut vt = Text::new(stat.value.clone(), TEXT_SIZE);
                vt.align = 1;
                vt.group_position = false;
                vt.keep_rgb = false;
                let v = self
                    .sprites
                    .create(seq, PRIO_BAR, ART, VALUE, None, Some(vt));
                self.child(v, row);
                self.sprites.get_mut(v).offset = value_offset;
                self.values.push(v);
                j += 1;
            }
            self.columns[col].total = j;
        }
        self.column = 0;
        if self.banner.is_none_or(|b| !self.sprites.alive(b)) {
            let mut t = Text::new(self.data.banner.clone(), TEXT_SIZE);
            t.align = 1;
            t.group_position = false;
            let b = self
                .sprites
                .create(seq, PRIO_ROW, ART, BANNER[0], Some(BANNER[1]), Some(t));
            self.closers.push((b, BANNER[2]));
            self.banner = Some(b);
        }
        self.browse_help(seq);
    }

    /// FUN_140353c40 for 0x18: the help line of the stat under the cursor.
    fn browse_help(&mut self, seq: &Sequence) {
        let entry = &self.data.status[self.status_rows[self.status_list.cursor]];
        let col = self.column;
        let cursor = self.columns[col].cursor;
        let help = entry.stats[col * 10..col * 10 + 10]
            .iter()
            .flatten()
            .nth(cursor)
            .map(|s| s.help.clone());
        if let Some(h) = help {
            self.show_help(seq, h);
        }
    }

    /// FUN_140351a00.
    fn status_browse(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let col = self.column;
        let (pick, mut moved) = self.columns[col].input(pad);
        if self.substate == 2 {
            let panel_dead = self.panel.is_none_or(|p| !self.sprites.alive(p));
            let tab_dead = !self.tab_alive(1);
            let banner_ok = self.pending != Some(State::StatusList)
                || self.banner.is_none_or(|b| !self.sprites.alive(b));
            if panel_dead && tab_dead && banner_ok {
                self.kill_tab(1);
                match self.pending.take() {
                    Some(State::StatusBrowse) => self.enter_browse(seq),
                    _ => self.enter_status_again(seq),
                }
            } else if pick != Pick::None || moved {
                if let Some(p) = self.panel {
                    self.sprites.kill(p);
                }
                self.kill_tab(1);
                if self.pending == Some(State::StatusList)
                    && let Some(b) = self.banner
                {
                    self.sprites.kill(b);
                }
            }
            return false;
        }
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == BROWSE_PANEL[1]);
        if (pick != Pick::None || moved) && !panel_idle {
            if let Some(p) = self.panel {
                self.sprites.set(seq, p, BROWSE_PANEL[1], None);
            }
            if let Some(b) = self.banner {
                self.sprites.set(seq, b, BANNER[1], None);
            }
            for i in 0..self.tabs.len() {
                if let Some(tab) = &self.tabs[i] {
                    let id = tab.id;
                    if self.sprites.alive(id) {
                        self.sprites.set(seq, id, TAB[1], None);
                    }
                }
            }
        }
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == BROWSE_PANEL[1]);
        if self.substate == 0 && panel_idle {
            self.substate = 1;
        }
        // FUN_140352b10: L2/R2 need more than one character.
        let valid = self.data.status.len() > 1;
        if let Some(s) = pick_sound(pick, moved, valid) {
            sounds.push(s);
        }
        match pick {
            Pick::Left | Pick::Right => {
                let other = 1 - col;
                if self.columns[other].total > 0 {
                    let index = self.columns[col].cursor;
                    self.column = other;
                    self.columns[other].cursor = index.min(self.columns[other].total - 1);
                    moved = true;
                }
            }
            Pick::Next | Pick::Previous if valid => {
                let n = self.data.status.len();
                let c = self.status_list.cursor;
                let next = if pick == Pick::Next {
                    (c + 1) % n
                } else {
                    (c + n - 1) % n
                };
                self.status_list.cursor = next;
                let e = &self.data.status[self.status_rows[next]];
                self.memory.status_bit = e.bit;
                let label = e.label.clone();
                self.pending_tab_text[1] = Some(label);
                self.hide_help(seq);
                self.close_tab(seq, 1);
                self.close_browse(seq);
                self.pending = Some(State::StatusBrowse);
                self.substate = 2;
            }
            Pick::Start => self.exit = true,
            Pick::Cancel => {
                self.hide_help(seq);
                self.close_tab(seq, 1);
                self.close_browse(seq);
                if let Some(b) = self.banner {
                    self.sprites.close(seq, b, BANNER[2]);
                }
                self.pending = Some(State::StatusList);
                self.substate = 2;
            }
            _ => {}
        }
        if moved && self.substate == 1 {
            self.browse_help(seq);
        }
        moved
    }

    fn close_browse(&mut self, seq: &Sequence) {
        if let Some(p) = self.panel {
            self.sprites.close(seq, p, BROWSE_PANEL[2]);
        }
    }

    /// FUN_140353020(0): back to the character list, objects kept.
    fn enter_status_again(&mut self, seq: &Sequence) {
        self.enter_status(seq);
    }

    // ---- Config 0x24 -----------------------------------------------------

    /// The exe's page, then the settings pages when the caller offers them.
    fn config_pages(&self) -> usize {
        if let Some(d) = &self.data.duel {
            d.names.len().max(1)
        } else if self.data.settings.is_some() {
            crate::settings::PAGES.len()
        } else {
            1
        }
    }

    fn page_name(&self) -> Vec<u8> {
        if let Some(d) = &self.data.duel {
            return d.names.get(self.config_page).cloned().unwrap_or_default();
        }
        encode_text(crate::settings::PAGES[self.config_page])
    }

    /// The caller keeps the camp open (the duel's lobby).
    fn keep_open(&self) -> bool {
        self.data.duel.as_ref().is_some_and(|d| d.keep_open)
    }

    /// The caller's rows on the current page.
    fn duel_rows(&self) -> Option<&[DuelRow]> {
        let d = self.data.duel.as_ref()?;
        Some(d.rows.get(self.config_page).map_or(&[], |r| r.as_slice()))
    }

    /// The caller changed its rows: the Config screen shows the new ones
    /// (rebuilt when their number changed, else laid out again).
    fn duel_refresh(&mut self, seq: &Sequence) {
        // The top level's help follows the caller's entry under the cursor.
        if self.data.duel.is_some() && self.state == State::Top && self.substate == 1 {
            let want = &self.data.entries[self.top.cursor].help;
            let changed = self
                .help
                .filter(|&h| self.sprites.alive(h))
                .and_then(|h| self.sprites.get(h).text.as_ref())
                .is_some_and(|t| &t.bytes != want);
            if changed {
                self.show_help(seq, want.clone());
            }
        }
        if self.state != State::Config || self.substate != 1 {
            return;
        }
        let Some(rows) = self.duel_rows().map(<[DuelRow]>::to_vec) else {
            return;
        };
        if rows == self.duel_seen {
            return;
        }
        if rows.len() != self.choices.len() {
            let cursor = self.config_list.cursor.min(rows.len().saturating_sub(1));
            self.build_config_rows(seq, cursor);
        } else {
            self.config_layout(seq);
        }
        self.config_help(seq);
        self.duel_seen = rows;
    }

    /// FUN_140365580 + FUN_140364a10: tab 0, the panel, 8 labels, their
    /// choices and invisible anchor rows. With settings pages, tab 1 names
    /// the page and the Status browser's "Switch" banner marks L2 / R2.
    fn enter_config(&mut self, seq: &Sequence) {
        self.state = State::Config;
        self.substate = 0;
        self.create_tab(seq, 0);
        self.config_values = config_values(self.data.config_flags, self.data.game_mode);
        let panel = self.sprites.create(
            seq,
            PRIO_CONFIG_PANEL,
            ART,
            CONFIG_PANEL[0],
            Some(CONFIG_PANEL[1]),
            None,
        );
        self.closers.push((panel, CONFIG_PANEL[2]));
        self.panel = Some(panel);
        self.config_page = self.memory.config_page.min(self.config_pages() - 1);
        if self.config_pages() > 1 {
            let name = self.page_name();
            self.tab_text(1, name);
            self.create_tab(seq, 1);
            let banner = self.data.banner.clone();
            self.show_banner(seq, banner);
        }
        let cursor = if self.config_page == 0 && self.data.duel.is_none() {
            self.memory.config_row
        } else {
            0
        };
        self.build_config_rows(seq, cursor);
        self.duel_seen = self
            .duel_rows()
            .map(<[DuelRow]>::to_vec)
            .unwrap_or_default();
    }

    /// The labels, choices and anchor rows of the current page.
    fn build_config_rows(&mut self, seq: &Sequence, cursor: usize) {
        let Some(panel) = self.panel else { return };
        for id in self.config_labels.drain(..) {
            self.sprites.kill(id);
        }
        for id in self.choices.drain(..).flatten() {
            self.sprites.kill(id);
        }
        for &id in &self.config_list.rows {
            self.sprites.kill(id);
        }
        let pitch = seq.groups[CONFIG_LABEL].ui_padding as f32;
        let label_size = seq.groups[CONFIG_LABEL].text_scale as f32;
        let mut rows = Vec::new();
        for i in 0..self.config_row_count() {
            let mut t = Text::new(self.config_view(i).label, label_size);
            t.align = 2;
            t.group_position = false;
            let l = self
                .sprites
                .create(seq, PRIO_PANEL, ART, CONFIG_LABEL, None, Some(t));
            self.child(l, panel);
            self.sprites.get_mut(l).offset = [0., pitch * i as f32];
            self.config_labels.push(l);
            let mut cs = Vec::new();
            for _ in 0..4 {
                let mut t = Text::new(Vec::new(), TEXT_SIZE);
                t.align = 2;
                t.group_position = false;
                let c = self
                    .sprites
                    .create(seq, PRIO_PANEL, ART, BLANK, None, Some(t));
                self.child(c, panel);
                cs.push(c);
            }
            self.choices.push(cs);
            let anchor = self.sprites.create(seq, PRIO_ROW, ART, BLANK, None, None);
            self.sprites.get_mut(anchor).die_with_parent = true;
            rows.push(anchor);
        }
        self.config_list = MenuList::new(rows, cursor);
        self.config_list.page = Some(8);
        self.config_layout(seq);
        self.config_mask();
    }

    fn config_row_count(&self) -> usize {
        if let Some(rows) = self.duel_rows() {
            return rows.len();
        }
        match &self.data.settings {
            Some(s) if self.config_page > 0 => s.rows(self.config_page, &self.data.languages).len(),
            _ => self.data.config.len().min(8),
        }
    }

    /// Row `i` of the current page: label, the choices shown (at most 3;
    /// a longer list shows only the current one, as Game Mode does), the
    /// chosen one and its help.
    fn config_view(&self, i: usize) -> ConfigView {
        if let Some(r) = self.duel_rows().and_then(|rows| rows.get(i)) {
            let (shown, selected) = if r.choices.len() <= 3 {
                (r.choices.clone(), r.selected)
            } else {
                (
                    vec![r.choices.get(r.selected).cloned().unwrap_or_default()],
                    0,
                )
            };
            return ConfigView {
                label: r.label.clone(),
                shown,
                selected,
                help: Some(r.help.clone()),
            };
        }
        if let Some(s) = &self.data.settings
            && self.config_page > 0
        {
            let r = &s.rows(self.config_page, &self.data.languages)[i];
            let (shown, selected) = if r.choices.len() <= 3 {
                (
                    r.choices.iter().map(|c| encode_text(c)).collect(),
                    r.selected,
                )
            } else {
                (vec![encode_text(&r.choices[r.selected])], 0)
            };
            return ConfigView {
                label: encode_text(r.label),
                shown,
                selected,
                help: Some(encode_text(r.help)),
            };
        }
        let row = &self.data.config[i];
        let (shown, selected, v) = if i == 7 {
            let mode = self.data.game_mode.min(3) as usize;
            (
                vec![row.choices.get(mode).cloned().unwrap_or_default()],
                0,
                mode,
            )
        } else {
            let mut shown = row.choices.clone();
            shown.resize(row.choices.len().clamp(1, 3), Vec::new());
            (shown, self.config_values[i], self.config_values[i])
        };
        ConfigView {
            label: row.label.clone(),
            shown,
            selected,
            help: row.help.get(v).cloned(),
        }
    }

    fn config_mask(&mut self) {
        let base = (0x73 & !0x10) | 0x3000;
        let sides =
            if self.data.duel.is_none() && self.config_page == 0 && self.config_list.cursor == 7 {
                0
            } else {
                0xc_0000
            };
        let pages = if self.config_pages() > 1 { 0xc000 } else { 0 };
        self.config_list.mask = base | sides | pages;
    }

    /// FUN_140365a20: each row's choices (selected / unselected looks), the
    /// anchor rows on the selected choice, and the cursor frame.
    fn config_layout(&mut self, seq: &Sequence) {
        for i in 0..self.choices.len() {
            let view = self.config_view(i);
            let n = view.shown.len().clamp(1, 3);
            let pair = CHOICES[n - 1];
            for c in 0..4 {
                let id = self.choices[i][c];
                if c >= n {
                    self.sprites.set(seq, id, BLANK, None);
                    if let Some(t) = self.sprites.get_mut(id).text.as_mut() {
                        t.bytes.clear();
                    }
                    continue;
                }
                let group = if view.selected == c { pair[0] } else { pair[1] };
                if self.sprites.get(id).player.group != group {
                    self.sprites.set(seq, id, group, None);
                }
                let g = &seq.groups[group];
                let s = self.sprites.get_mut(id);
                s.offset = [g.ui_padding as f32 * c as f32, 26. * i as f32];
                if let Some(t) = s.text.as_mut() {
                    t.bytes = view.shown.get(c).cloned().unwrap_or_default();
                    t.offset = [g.text_position_x as f32, 0.];
                }
            }
            let anchor = self.config_list.rows[i];
            self.sprites.get_mut(anchor).parent = Some(self.choices[i][view.selected.min(n - 1)]);
        }
        self.update_config_frame(seq);
    }

    /// FUN_1403657e0: the cursor frame on the selected choice (substate 1).
    fn update_config_frame(&mut self, seq: &Sequence) {
        if self.substate != 1 || self.choices.is_empty() {
            if let Some(f) = self.config_frame.take() {
                self.sprites.kill(f);
            }
            return;
        }
        let row = self.config_list.cursor;
        let view = self.config_view(row);
        let n = view.shown.len().clamp(1, 3);
        let parent = self.choices[row][view.selected.min(n - 1)];
        let group = CHOICES[n - 1][2];
        match self.config_frame {
            Some(f) if self.sprites.alive(f) => {
                if self.sprites.get(f).parent != Some(parent)
                    || self.sprites.get(f).player.group != group
                {
                    self.sprites.set(seq, f, group, None);
                    self.sprites.get_mut(f).parent = Some(parent);
                }
            }
            _ => {
                let f = self.sprites.create(seq, PRIO_BAR, ART, group, None, None);
                self.sprites.get_mut(f).parent = Some(parent);
                self.config_frame = Some(f);
            }
        }
    }

    fn config_help(&mut self, seq: &Sequence) {
        if self.choices.is_empty() {
            return;
        }
        if let Some(h) = self.config_view(self.config_list.cursor).help {
            self.show_help(seq, h);
        }
    }

    /// FUN_140364d70; L2 / R2 turn the settings pages.
    fn config(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let (pick, moved) = self.config_list.input(pad);
        if self.substate == 2 {
            let panel_dead = self.panel.is_none_or(|p| !self.sprites.alive(p));
            let banner_dead = self.banner.is_none_or(|b| !self.sprites.alive(b));
            if panel_dead && banner_dead && !self.tab_alive(0) && !self.tab_alive(1) {
                self.banner = None;
                self.return_top(seq);
            } else if pick != Pick::None || moved {
                if let Some(p) = self.panel {
                    self.sprites.kill(p);
                }
                if let Some(b) = self.banner.take() {
                    self.sprites.kill(b);
                }
                self.kill_tab(0);
                self.kill_tab(1);
            }
            return false;
        }
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == CONFIG_PANEL[1]);
        if (pick != Pick::None || moved) && !panel_idle {
            if let Some(p) = self.panel {
                self.sprites.set(seq, p, CONFIG_PANEL[1], None);
            }
            for i in 0..2 {
                if let Some(tab) = &self.tabs[i] {
                    let id = tab.id;
                    if self.sprites.alive(id) {
                        self.sprites.set(seq, id, TAB[1], None);
                    }
                }
            }
        }
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == CONFIG_PANEL[1]);
        if self.substate == 0 && panel_idle {
            self.substate = 1;
            self.config_help(seq);
            self.update_config_frame(seq);
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if moved {
            self.config_mask();
            self.config_layout(seq);
            self.config_help(seq);
        }
        match pick {
            Pick::Left | Pick::Right => {
                let row = self.config_list.cursor;
                let page = self.config_page;
                if let Some(d) = self.data.duel.as_mut() {
                    // The caller's row: its step goes back to the caller; a
                    // row this player may not change refuses.
                    if d.rows
                        .get(page)
                        .and_then(|r| r.get(row))
                        .is_some_and(|r| r.locked)
                    {
                        sounds.pop();
                        sounds.push(SOUND_INVALID);
                    } else {
                        d.steps.push((page, row, pick == Pick::Right));
                    }
                } else if self.config_page == 0 {
                    let n = self.data.config[row].choices.len().clamp(1, 3);
                    let v = &mut self.config_values[row];
                    *v = if pick == Pick::Left {
                        (*v + n - 1) % n
                    } else {
                        (*v + 1) % n
                    };
                } else if let Some(s) = self.data.settings.as_mut() {
                    s.step(
                        self.config_page,
                        row,
                        pick == Pick::Right,
                        &self.data.languages,
                    );
                }
                self.config_layout(seq);
                self.config_help(seq);
                self.light.retarget();
            }
            Pick::Next | Pick::Previous => {
                let n = self.config_pages();
                self.config_page = if pick == Pick::Next {
                    (self.config_page + 1) % n
                } else {
                    (self.config_page + n - 1) % n
                };
                let name = self.page_name();
                self.tab_text(1, name);
                self.build_config_rows(seq, 0);
                self.config_help(seq);
                self.light.retarget();
            }
            Pick::Start if self.keep_open() => {}
            Pick::Start => {
                self.save_config();
                self.exit = true;
            }
            Pick::Cancel => {
                self.save_config();
                self.hide_help(seq);
                self.close_tab(seq, 0);
                self.close_tab(seq, 1);
                self.close_banner(seq);
                if let Some(p) = self.panel {
                    self.sprites.close(seq, p, CONFIG_PANEL[2]);
                }
                if let Some(f) = self.config_frame.take() {
                    self.sprites.kill(f);
                }
                self.substate = 2;
                self.pending = Some(State::Top);
            }
            _ => {}
        }
        moved
    }

    /// FUN_140365460: the values back into the save flags; the cursor row
    /// (on the exe's page) and the page are remembered.
    fn save_config(&mut self) {
        if self.data.duel.is_some() {
            return;
        }
        self.data.config_flags = config_flags(self.data.config_flags, &self.config_values);
        if self.config_page == 0 {
            self.memory.config_row = self.config_list.cursor;
        }
        self.memory.config_page = self.config_page;
    }

    /// The Config values and page into `data` / `memory` now (before the
    /// caller rebuilds the camp).
    pub fn commit_config(&mut self) {
        if self.state == State::Config {
            self.save_config();
        }
    }

    /// The Config row under the cursor (for `open_on_config`).
    /// The top-level entry under the cursor (its id).
    pub fn top_entry(&self) -> Option<u8> {
        self.data.entries.get(self.top.cursor).map(|e| e.id)
    }

    pub fn config_cursor(&self) -> usize {
        self.config_list.cursor
    }

    /// The camp reopened straight on Config, on the remembered page, with
    /// the cursor on `row` (after the caller swapped the text language):
    /// no opening scene and no top level.
    pub fn open_on_config(layout: &Layout, data: Data, memory: Memory, row: usize) -> Self {
        // No tutorial notice opens over it.
        let quiet = Memory {
            notices_seen: u16::MAX,
            ..memory
        };
        let (mut camp, _) = Camp::open(layout, data, quiet);
        camp.memory.notices_seen = memory.notices_seen;
        let seq = &layout.sequences[SEQUENCE];
        camp.layout.set_scene(SCENE_IDLE, None);
        for id in camp.top.rows.clone() {
            camp.sprites.kill(id);
        }
        for id in [camp.frame_sprite, camp.status_block, camp.prompt]
            .into_iter()
            .flatten()
        {
            camp.sprites.kill(id);
        }
        if let Some(h) = camp.help.take() {
            camp.sprites.kill(h);
        }
        let label = camp
            .data
            .entries
            .iter()
            .find(|e| e.id == 7)
            .map(|e| e.label.clone())
            .unwrap_or_default();
        camp.tab_text(0, label);
        camp.enter_config(seq);
        if let Some(p) = camp.panel {
            camp.sprites.set(seq, p, CONFIG_PANEL[1], None);
        }
        for i in 0..2 {
            if let Some(tab) = &camp.tabs[i] {
                let id = tab.id;
                camp.sprites.set(seq, id, TAB[1], None);
            }
        }
        camp.config_list.cursor = row.min(camp.config_list.total.saturating_sub(1));
        camp.config_mask();
        camp.config_layout(seq);
        camp
    }

    // ---- exit and drawing ------------------------------------------------

    /// FUN_140303cd0 with the exit flag (and the sub-screens' exit hooks):
    /// everything plays its close group; glove and help go at once.
    fn close(&mut self, seq: &Sequence) {
        if !self.layout.dead && self.layout.scene != SCENE_CLOSE {
            self.layout.set_scene(SCENE_CLOSE, None);
        }
        for (id, group) in self.closers.clone() {
            let s = self.sprites.get(id);
            if !s.dead && s.player.group != group && !s.die_when_done {
                self.sprites.close(seq, id, group);
            }
        }
        if let Some(l) = self.light.id
            && self.sprites.alive(l)
            && self.sprites.get(l).player.group != LIGHT[2]
        {
            self.sprites.close(seq, l, LIGHT[2]);
        }
        if let Some(g) = self.glove.take() {
            self.sprites.kill(g);
        }
        if let Some(h) = self.help.take() {
            self.sprites.kill(h);
        }
        if let Some(f) = self.config_frame.take() {
            self.sprites.kill(f);
        }
        self.item_ui.kill_dim(&mut self.sprites);
        self.picture_close(seq);
    }

    // ---- item picture --------------------------------------------------

    /// FUN_140302370 for `id` (None: FUN_140302280, no picture).
    fn picture_request(&mut self, seq: &Sequence, id: Option<u16>) {
        match (id, &self.data.picture_sequence) {
            (Some(id), Some(ipic)) => self.picture.request(seq, ipic, &mut self.sprites, id),
            _ => self.picture.reset(),
        }
    }

    /// FUN_140302040.
    fn picture_close(&mut self, seq: &Sequence) {
        if let Some(ipic) = &self.data.picture_sequence {
            self.picture.close(seq, ipic, &mut self.sprites);
        }
    }

    /// FUN_140301cc0 + FUN_140301c80.
    fn picture_timers(&mut self, seq: &Sequence) {
        if let Some(ipic) = &self.data.picture_sequence {
            self.picture
                .timers(seq, ipic, &mut self.sprites, &mut self.pictures);
        }
    }

    /// FUN_140305480 (the curtain's callback): every camp sprite goes and
    /// the camp ends.
    pub fn kill_all(&mut self) {
        self.layout.kill();
        for s in &mut self.sprites.list {
            s.dead = true;
        }
        self.model_time = None;
        self.picture.kill(&mut self.sprites);
        self.notice.kill();
        self.closed = true;
    }

    /// The draw list: the layout (0x7532), then every sprite in priority
    /// order with the digit rows after their labels.
    pub fn draw(&self, layout: &Layout, font: &Font) -> Vec<(Sheet, Vec<Quad>)> {
        let seq = &layout.sequences[SEQUENCE];
        let m = Modifiers::default();
        let mut out = Vec::new();
        if !self.layout.dead {
            for (prop, player, origin) in self.layout.player.visible(layout, [0, 0]) {
                out.push((
                    Sheet::Art,
                    layout.sequences[prop.sequence].quads(player, origin, &m),
                ));
            }
        }
        let convert = |items: Vec<(sprite_player::Sheet, Vec<Quad>)>| {
            items.into_iter().map(|(sheet, quads)| match sheet {
                sprite_player::Sheet::Art(_) => (Sheet::Art, quads),
                sprite_player::Sheet::Font => (Sheet::Font, quads),
                sprite_player::Sheet::Icon => (Sheet::Icon, quads),
            })
        };
        // The model paste at its priority (0x7538 on the top level, 0x7545
        // in Party); the digit rows (Tz::MultiSeq) follow the panel band
        // (0x753b) with their labels.
        let layer = self.model_layer();
        // The item picture goes after the sprites of its priority.
        let picture = self
            .data
            .picture_sequence
            .as_ref()
            .and_then(|ipic| self.picture.quads(ipic, &self.sprites));
        let mut cuts: Vec<(u32, Cut)> = vec![(PRIO_PANEL + 1, Cut::Digits)];
        cuts.push((0x7547, Cut::Notice));
        if let Some((prio, _)) = &layer {
            cuts.push((*prio, Cut::Models));
        }
        if let Some((_, prio, _)) = &picture {
            cuts.push((prio + 1, Cut::Picture));
        }
        cuts.sort_by_key(|c| c.0);
        let mut low = 0;
        for (cut, kind) in cuts {
            out.extend(convert(
                self.sprites
                    .draw_where(seq, font, |p| (low..cut).contains(&p)),
            ));
            match kind {
                Cut::Models => {
                    for &(member, offset, alpha) in &layer.as_ref().unwrap().1 {
                        out.push((Sheet::Model(member), model_quads(offset, alpha)));
                    }
                }
                Cut::Digits => out.push((Sheet::Art, self.digit_quads(seq))),
                Cut::Notice => out.extend(self.notice_draw(layout, font)),
                Cut::Picture => {
                    if let Some((id, _, quads)) = &picture {
                        out.push((Sheet::Picture(*id), quads.clone()));
                    }
                }
            }
            low = cut;
        }
        out.extend(convert(self.sprites.draw_where(seq, font, |p| p >= low)));
        out
    }

    /// Tz::MenuObj's pastes: (priority, [(member, layout offset, alpha)]).
    /// The top level pastes at 0x7538 with the party frame's colour alpha
    /// (FUN_1402ffac0); Party at 0x7545 on each name card, with the card's
    /// alpha (party-spec §4).
    pub fn model_layer(&self) -> Option<ModelLayer> {
        self.model_time?;
        if let Some(layer) = self.party_models() {
            return Some(layer);
        }
        let f = self.frame_sprite?;
        let s = self.sprites.get(f);
        if s.dead {
            return None;
        }
        let n = self.data.panels.members.len();
        let members = (0..n.min(3))
            .map(|i| (i, model_offset(i, n), s.ctx_color[3]))
            .collect();
        Some((PRIO_MODEL, members))
    }

    /// Whether a model host lives (the party frame or a Party name card).
    fn models_hosted(&self) -> bool {
        self.frame_sprite.is_some_and(|f| self.sprites.alive(f)) || self.party_models().is_some()
    }

    /// Tz::MultiSeq: from the label's context + (LightPositionX,
    /// TextPositionY), each digit at the cursor, which then moves left by
    /// its group's TextPositionX; alpha follows the label (0x200|0x400).
    fn digit_quads(&self, seq: &Sequence) -> Vec<Quad> {
        let mut out = Vec::new();
        for (label, players) in [
            (self.munny, &self.munny_digits),
            (self.time, &self.time_digits),
        ] {
            let s = self.sprites.get(label);
            if s.dead {
                continue;
            }
            let g = &seq.groups[s.player.group];
            let c = self.sprites.context(label);
            let mut x = c[0] + g.light_position_x as f32;
            let y = c[1] + g.text_position_y as f32;
            let a = seq
                .context_color(&s.player, &Modifiers::default())
                .map_or(1., |c| c[3]);
            let m = Modifiers {
                color: [1., 1., 1., a],
                ..Modifiers::default()
            };
            for p in players {
                out.extend(seq.quads(p, [x, y], &m));
                x -= seq.groups[p.group].text_position_x as f32;
            }
        }
        out
    }
}

/// Sora's menu motion 0x8b (P_EX100.mset slot 558 = the field walk) runs
/// 64 ticks.
pub const MODEL_LENGTH: f32 = 64.;

/// One whole-layout quad for a member's render shifted by `offset` (the
/// projection centre's shift from the layout centre); the caller samples
/// that member's target (uv in the caller's texels).
fn model_quads(offset: [f32; 2], alpha: f32) -> Vec<Quad> {
    let (l, r) = (256. - 512. * 2. / 3., 256. + 512. * 2. / 3.);
    let [dx, dy] = offset;
    vec![Quad {
        corners: [
            [l + dx, dy],
            [r + dx, dy],
            [l + dx, 416. + dy],
            [r + dx, 416. + dy],
        ],
        uv: [0, 0, 1, 1],
        colors: [[MODEL_TINT, MODEL_TINT, MODEL_TINT, alpha]; 4],
        blend: 0,
    }]
}

/// What the draw inserts between priority bands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut {
    Digits,
    Models,
    Picture,
    Notice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowGroupsFor {
    Top,
    Status,
    Fixed(RowGroups),
    None,
}

/// Munny digit groups, ones first: stops at the first leading zero except
/// the ones digit; 6 slots.
fn munny_groups(value: u32) -> Vec<usize> {
    let mut v = value % 1_000_000;
    (0..6)
        .map(|k| {
            let g = if k == 0 || v > 0 {
                MUNNY_DIGIT + (v % 10) as usize
            } else {
                BLANK
            };
            v /= 10;
            g
        })
        .collect()
}

/// Clock groups (FUN_140157390: h = t/216000, m = t%216000/3600): minute
/// ones and tens, the colon, then up to 3 hour digits; 9 slots.
fn time_groups(frames: u32) -> Vec<usize> {
    let hours = frames / 216_000;
    let minutes = frames % 216_000 / 3600;
    let mut g = vec![
        TIME_DIGIT + (minutes % 10) as usize,
        TIME_DIGIT + (minutes / 10) as usize,
        COLON,
    ];
    let mut h = hours;
    for k in 0..3 {
        if k > 0 && h == 0 {
            break;
        }
        g.push(TIME_DIGIT + (h % 10) as usize);
        h /= 10;
    }
    g.resize(9, BLANK);
    g
}

/// Players for digit groups, keeping a slot's player when its group is
/// unchanged (a group is only set when it changes).
fn digits(seq: &Sequence, groups: &[usize], mut old: Vec<Player>) -> Vec<Player> {
    old.resize_with(groups.len(), || Player::new(seq, BLANK));
    old.into_iter()
        .zip(groups)
        .map(|(p, &g)| if p.group == g { p } else { Player::new(seq, g) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_follow_the_exe_rules() {
        assert_eq!(munny_groups(0), vec![51, 31, 31, 31, 31, 31]);
        assert_eq!(munny_groups(1205), vec![56, 51, 53, 52, 31, 31]);
        // 2 h 05 min: 5, 0, colon, 2, blanks.
        let t = 2 * 216_000 + 5 * 3600 + 59;
        assert_eq!(time_groups(t), vec![66, 61, 71, 63, 31, 31, 31, 31, 31]);
        assert_eq!(
            encode_number("12/30%"),
            vec![0x22, 0x23, 0x4b, 0x24, 0x21, 0x4a]
        );
    }

    #[test]
    fn member_models_follow_the_exe_offsets_and_clock() {
        assert_eq!(model_offset(0, 1), [47., 96.]);
        assert_eq!(model_offset(0, 2), [-2., 96.]);
        assert_eq!(model_offset(1, 2), [96., 96.]);
        assert_eq!(model_offset(2, 3), [178., 96.]);
        assert_eq!(model_time(0., MODEL_LENGTH), 1.);
        assert_eq!(model_time(63., MODEL_LENGTH), 64.);
        assert_eq!(model_time(64., MODEL_LENGTH), 1.);
    }

    #[test]
    fn config_values_round_trip_the_save_bits() {
        // Default new-game config 0x40b: vibration on (bit 0), map off?
        let v = config_values(0x40b, 1);
        assert_eq!(v, [0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(config_flags(0x40b, &v), 0x40b);
        let mut w = v;
        w[3] = 1;
        w[5] = 1;
        w[6] = 1;
        let f = config_flags(0x40b, &w);
        assert_eq!(f & 0x600, 0x200);
        assert_eq!(f & 1, 0);
        assert_eq!(f & 0x40, 0x40);
        assert_eq!(config_values(f, 1), w);
    }

    #[test]
    fn lists_page_and_switch_with_the_exe_codes() {
        let rows: Vec<Id> = (0..8).map(Id).collect();
        let mut l = MenuList::new(rows, 0);
        l.mask = (0x73 & !0x10) | 0x3000 | 0xc_0000;
        l.page = Some(8);
        let p = |trig: u64, repeat: u64| Pad { trig, repeat };
        assert_eq!(l.input(&p(bits::R1, bits::R1)), (Pick::None, true));
        assert_eq!(l.cursor, 7);
        assert_eq!(l.input(&p(bits::R1, bits::R1)), (Pick::None, true));
        assert_eq!(l.cursor, 0);
        assert_eq!(l.input(&p(0, bits::LEFT)), (Pick::Left, false));
        assert_eq!(l.input(&p(bits::DECIDE, bits::DECIDE)), (Pick::None, false));
        assert_eq!(l.input(&p(bits::CANCEL, 0)), (Pick::Cancel, false));
        assert_eq!(pick_sound(Pick::Next, false, false), Some(SOUND_INVALID));
    }
}
