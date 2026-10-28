//! Jiminy's Journal and Roxas's Journal (menu mode 4), translated from the
//! journal init FUN_1402f3d70, its task FUN_1402f4bc0, the state machine
//! FUN_1402f24a0 and the driver FUN_1402f2cf0; the frame FUN_1402f5410 /
//! FUN_1402f5780 / FUN_1402f50c0 / FUN_1402f2940 and the page slide
//! FUN_1402f5330; the top pages Select World (state 0, FUN_140326b60 /
//! FUN_1403260f0 with the grid step FUN_1402f8240), Roxas's notes (1,
//! FUN_140328330 / FUN_1403276e0), Notes (2, FUN_140327d00 / FUN_140327330)
//! and Collection (0xd, FUN_14032bb90 / FUN_14032b210). The book draws
//! from `menu/<lang>/jmemo.2ld` sequence 5; the pictures use sequences 2-4
//! on the images of `menu/jm_world.2ld` and `menu/jm_top_<code>.2ld`. The
//! category pages are built except Puzzles and Pieces: deciding one of those
//! plays the decide sound only. The pause_menu hands over under the
//! pause.2ld curtain (`Curtain`, FUN_14031adc0 / FUN_14031a690).
use super::bits;
use super::{MenuList, Pad, SOUND_CLOSE, SOUND_CURSOR, SOUND_DECIDE, SOUND_INVALID, SOUND_OPEN};
use crate::field_command_menu::Font;
use crate::sprite_player::{self, Id, LightCur, Sprites, Text};
use data::journal_table::{Album, Char, Diag, Story, World};
use data::menu_layout::{Layout, LayoutTask};
use data::sprite_sequence::{Modifiers, Player, Quad, Sequence};
use std::collections::{HashMap, HashSet};

mod album;
mod category;
mod characters;
mod files;
mod limits;
mod lists;
mod story;
mod synthesis;
mod treasure;

pub use synthesis::Synthesis;

/// The journal sprites' SEQD (DAT_1407434e8) and image (DAT_1407434e4).
pub const SEQUENCE: usize = 5;
const ART: usize = 0;
/// Sequences of the pictures taken from other 2LDs: the world icon, the
/// page picture and the title strip.
const SEQ_ICON: usize = 2;
const SEQ_PICTURE: usize = 3;
const SEQ_STRIP: usize = 4;

/// Layout scenes (table 0x1405baaa0 / 0x1405baaa8): open, idle, close.
const JIMINY_SCENES: [usize; 3] = [0, 1, 2];
const ROXAS_SCENES: [usize; 3] = [3, 4, 5];

/// Priorities: frame texts, the page container, the page base
/// (DAT_140743710 = 0x755d) and the cursor sprites (FUN_1402f6c80).
const PRIO_FRAME_TEXT: u32 = 0x7558;
const PRIO_CONTAINER: u32 = 0x7559;
const BASE: u32 = 0x755d;
const PRIO_CURSOR: u32 = 0x7571;

/// Page container groups (0x1405baab0 / 0x1405baab8): in, out, idle per
/// direction (0 forward from the right, 1 backward from the left); 154 on
/// exit (FUN_1402f37c0).
const SLIDE: [[usize; 3]; 2] = [[153, 151, 152], [150, 154, 152]];
const CONTAINER_IDLE: usize = 152;
const CONTAINER_EXIT: usize = 154;
/// Frame texts: appear, idle, close.
const TITLE: [usize; 3] = [53, 54, 55];
const LEFT_TAB: [usize; 3] = [47, 48, 49];
const RIGHT_TAB: [usize; 3] = [50, 51, 52];
const HELP_ANCHOR: [usize; 3] = [44, 45, 46];
const HELP_IN: usize = 145;
const HELP_OUT: usize = 146;
/// The hand (FUN_1402f6610 group 0x21) and LightCur (0x1405baac8:
/// appear, close, idle).
const HAND: usize = 33;
const LIGHT: [usize; 3] = [35, 36, 37];
/// List rows (FUN_1402f8e20 with {3,3,3,3}; the world grid {1,1,1,1}).
const ROW: usize = 3;
const GRID_ROW: usize = 1;
const CELL: usize = 173;
const CELL_SELECTED: usize = 172;
/// Page panels: Notes (P0, P1, Summary, Objective), Collection (P0, P1),
/// Roxas (P0, P1, Summary, Objective).
const NOTES: [usize; 4] = [159, 169, 167, 168];
const COLLECTION: [usize; 2] = [155, 165];
const ROXAS: [usize; 4] = [163, 166, 170, 171];
/// The book's own picture group on images taken from other 2LDs (group 2
/// static, 1 fades in).
const PICTURE_STATIC: usize = 2;

/// Messages.
const SUMMARY_LABEL: u16 = 0x3253;
const OBJECTIVE_LABEL: u16 = 0x3254;
/// FUN_140344660's default when no story step is reached (sys.bar 0xadc).
pub const NO_STORY: u16 = 0x8adc;
const HELP_WORLDS: u16 = 0x48cb;
/// The top tabs ring (0x1405baae8): Select World, Notes, Collection.
const TABS: [u16; 3] = [0x2f60, 0x2f62, 0x2f61];
/// FUN_1402ebe70(0x400 / 0x800): the L1 / R1 prompt codes the tab labels
/// carry.
const L1_PROMPT: u8 = 0xe0;
const R1_PROMPT: u8 = 0xe1;
/// Notes options (0x1405bbc58 labels, 0x1405bbc70 help) and their states.
const NOTES_OPTIONS: [(u16, u16, u8); 9] = [
    (0x324c, 0x48cd, 6),
    (0x324d, 0x48ce, 3),
    (0x324e, 0x48cf, 4),
    (0x324f, 0x48d0, 7),
    (0x50a9, 0x50aa, 8),
    (0x48cc, 0x48d1, 9),
    (0x3250, 0x48d2, 10),
    (0x3251, 0x48d3, 0xb),
    (0x3252, 0x48d4, 0xc),
];
/// Collection options (0x1405bbc88 labels, 0x1405bbca0 help).
const COLLECTION_OPTIONS: [(u16, u16, u8); 12] = [
    (0x3679, 0x48e1, 0x18),
    (0x367a, 0x48e2, 0xe),
    (0x367b, 0x48e3, 0x12),
    (0x367c, 0x48e4, 0x15),
    (0x48da, 0x48e5, 0x1a),
    (0x4f47, 0x4f6b, 0x1b),
    (0x48db, 0x48e6, 0x22),
    (0x48dc, 0x48e7, 0x23),
    (0x48dd, 0x48e8, 0x24),
    (0x48de, 0x48e9, 0x25),
    (0x48df, 0x48ea, 0x27),
    (0x48e0, 0x48eb, 0x28),
];
/// Every message the journal shows besides the tables' own ids.
pub const MESSAGES: [u16; 52] = [
    SUMMARY_LABEL,
    OBJECTIVE_LABEL,
    HELP_WORLDS,
    0x2f60,
    0x2f62,
    0x2f61,
    0x324c,
    0x324d,
    0x324e,
    0x324f,
    0x50a9,
    0x48cc,
    0x3250,
    0x3251,
    0x3252,
    0x48cd,
    0x48ce,
    0x48cf,
    0x48d0,
    0x50aa,
    0x48d1,
    0x48d2,
    0x48d3,
    0x48d4,
    0x3679,
    0x367a,
    0x367b,
    0x367c,
    0x48da,
    0x4f47,
    0x48db,
    0x48dc,
    0x48dd,
    0x48de,
    0x48df,
    0x48e0,
    0x48e1,
    0x48e2,
    0x48e3,
    0x48e4,
    0x48e5,
    0x4f6b,
    0x48e6,
    0x48e7,
    0x48e8,
    0x48e9,
    0x48ea,
    0x48eb,
    0x493f,
    0x26c2,
    0x26c3,
    0x26b3,
];
/// Messages of the category pages (help lines and the empty album).
pub const PAGE_MESSAGES: [u16; 35] = [
    0x48d5, 0x48d6, 0x48d8, 0x48d9, 0x32dd, 0x48ee, 0x48ef, 0x48f0, 0x48f1, 0x48f2, 0x36b3, 0x2874,
    0x2dc6, 0x48f4, 0x48f5, 0x4a0f, 0x4a12, 0x4a11, 0x4a13, 0x48ec, 0x48ed, 0x48f8, 0x4b00, 0x4b06,
    0x48f6, 0x48f7, 0x4b71, 0x4b80, 0x48d7, 0x4b37, 0x4b38, 0x4b39, 0x4b3a, 0x36f8, 0x48f9,
];
/// The Moogle level names (0x1405bbd50: nine system messages).
pub const MOOGLE_NAMES: [u32; 9] = [
    0x3cd2, 0x3cd3, 0x3cd4, 0x3cd5, 0x3cd6, 0x3cd7, 0x3cd8, 0x3cd9, 0x3cda,
];

/// FUN_1402f5db0: the "show everything" story flag.
const SHOW_ALL: u16 = 0x4837;
/// Roxas's journal world (a8[0]).
const ROXAS_WORLD: i32 = 0x13;

/// States (DAT_140bee64c) of the pages built here.
const SELECT_WORLD: u8 = 0;
const ROXAS_NOTES: u8 = 1;
const NOTES_PAGE: u8 = 2;
const COLLECTION_PAGE: u8 = 0xd;

/// The game world id (DAT_140717008) → journal world (FUN_1402f3d70).
pub fn journal_world(world: i32) -> i32 {
    match world {
        2 => 0,
        4 => 1,
        5 => 2,
        1 | 6 => 3,
        7 => 4,
        8 => 5,
        9 => 6,
        10 => 7,
        11 => 8,
        12 => 9,
        13 => 10,
        14 => 11,
        16 => 12,
        17 => 13,
        18 => 14,
        _ => -1,
    }
}

/// What the save reports for the conditional rows (FUN_140334540 Ansem
/// reports, FUN_140343c90 pieces, FUN_14033d370 missions, FUN_14033bdd0
/// minigames, FUN_140339500 limits, FUN_14033a980 synthesis).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Unlocks {
    pub reports: bool,
    pub pieces: bool,
    pub missions: bool,
    pub minigames: bool,
    pub limits: bool,
    pub synthesis: bool,
}

/// An item a chest or map holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TreasureItem {
    pub name: Vec<u8>,
    pub icon: u8,
    /// Item type 22.
    pub map: bool,
}

/// Everything the journal shows besides its art.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JournalData {
    /// DAT_140bede94: 1 Jiminy's Journal, 2 Roxas's Journal.
    pub kind: u8,
    /// The game world id (DAT_140717008).
    pub world: i32,
    /// jiminy.bar `worl` and `stor`.
    pub worlds: Vec<World>,
    pub stories: Vec<Story>,
    /// Story flags set in the save (save+0x1c90 bits, FUN_14039a980).
    pub flags: Vec<u16>,
    pub unlocks: Unlocks,
    /// Messages by id (jm.bar; `NO_STORY` from sys.bar).
    pub texts: HashMap<u16, Vec<u8>>,
    /// The jm_top_<code>.2ld files the caller has (else no pictures).
    pub tops: Vec<String>,
    /// Whether jm_world.2ld is there (else no world icons).
    pub world_icons: bool,
    /// jiminy.bar `char`, `diag` and `albu`.
    pub chars: Vec<Char>,
    pub diag: Vec<Diag>,
    pub albums: Vec<Album>,
    /// 03system `trsr`, the items they hold (names, icons, map type) and
    /// the save's opened-chest bits and owned map items.
    pub treasures: Vec<data::journal_table::Treasure>,
    pub item_names: HashMap<u16, TreasureItem>,
    pub opened_bits: Vec<u16>,
    pub owned_items: Vec<u16>,
    /// jiminy.bar `anse` and the rows already seen (save+0x4d74).
    pub reports: Vec<data::journal_table::Report>,
    pub seen_reports: Vec<u16>,
    /// The `char` rows and `albu` rows already seen (save+0x4c38 / +0x4c7c).
    pub seen_chars: Vec<u16>,
    pub seen_albums: Vec<u16>,
    /// jiminy.bar `limi`, each row's use counter (save+0x3d48 + 2 * the
    /// limit's record) and the rows already seen (save+0x4c38 + 0x84).
    pub limits: Vec<data::journal_table::Limit>,
    pub limit_counts: Vec<u16>,
    pub seen_limits: Vec<u16>,
    /// jiminy.bar `ques` and `mini`, the minigame records (state, value;
    /// save+0x3db4 + 8 * i), the menu flags (save+0x2390) and the rows
    /// already seen (save+0x4c38 + 0x94 / 0x88).
    pub quests: Vec<data::journal_table::Quest>,
    pub minigames: Vec<data::journal_table::Minigame>,
    pub minigame_records: Vec<(u8, u32)>,
    pub menu_flags: Vec<u16>,
    pub seen_quests: Vec<u16>,
    pub seen_minigames: Vec<u16>,
    /// The `stor` rows already seen (save+0x4c38 + 0xac).
    pub seen_story: Vec<u16>,
    /// Synthesis Notes: the save's fields, the Moogle level names and the
    /// four lines already seen (save+0x4d78).
    pub synthesis: Synthesis,
    pub moogle_names: Vec<Vec<u8>>,
    pub seen_synthesis: Vec<u8>,
}

/// FUN_140346b30 reads album and story bits of journal worlds 0..0xe and
/// 13 report bits.
const NEW_WORLDS: std::ops::Range<i8> = 0..0xf;
const NEW_REPORTS: usize = 0xd;

impl JournalData {
    /// FUN_14039a980 (0xffff is never set).
    fn flag(&self, f: u16) -> bool {
        f != 0xffff && self.flags.contains(&f)
    }

    /// FUN_1403444f0's character bits: an entry of a record whose
    /// condition holds (the off flag is not tested).
    fn char_available(&self, character: u16) -> bool {
        self.diag.iter().any(|r| {
            let cond = if r.kind == 2 {
                false
            } else {
                r.cond != 0xffff && self.flags.contains(&r.cond)
            };
            cond && r.entries.iter().any(|e| e.character == character)
        })
    }

    /// FUN_140346b30 over the bits FUN_140345420 fills: some entry is
    /// available but not yet seen (the camp's Journal badge, camp-01
    /// §3.3). The treasure, map and puzzle piece bits are not built
    /// (their seen fields are not kept) [U].
    pub fn any_new(&self) -> bool {
        let chars = (0..self.chars.len() as u16)
            .any(|c| self.char_available(c) && !self.seen_chars.contains(&c));
        let reports = self
            .reports
            .iter()
            .take(NEW_REPORTS)
            .enumerate()
            .any(|(i, r)| {
                self.owned_items.contains(&r.item) && !self.seen_reports.contains(&(i as u16))
            });
        let limits = (0..self.limits.len()).any(|i| {
            self.limit_counts.get(i).is_some_and(|&n| n > 0)
                && !self.seen_limits.contains(&(i as u16))
        });
        let record = |r: u16| {
            self.minigame_records
                .get(r as usize)
                .copied()
                .unwrap_or((0, 0))
        };
        let missions = self.quests.iter().enumerate().any(|(i, q)| {
            let status = lists::quest_status(
                q,
                self.flag(SHOW_ALL),
                |f| self.flag(f),
                |f| self.menu_flags.contains(&f),
                record,
            );
            !matches!(status, 0 | 3) && !self.seen_quests.contains(&(i as u16))
        });
        let minigames = self
            .minigames
            .iter()
            .enumerate()
            .any(|(i, m)| record(m.record).0 > 1 && !self.seen_minigames.contains(&(i as u16)));
        let s = self.synthesis;
        let synthesis = [
            s.moogle_seen,
            s.materials > 0,
            s.materials > 0,
            s.moogle_seen,
        ]
        .iter()
        .enumerate()
        .any(|(line, &reached)| reached && !self.seen_synthesis.contains(&(line as u8)));
        let albums = self.albums.iter().enumerate().any(|(r, a)| {
            NEW_WORLDS.contains(&a.world)
                && self.flag(a.flag)
                && !self.seen_albums.contains(&(r as u16))
        });
        let story = self.stories.iter().enumerate().any(|(k, s)| {
            NEW_WORLDS.contains(&s.world)
                && self.flag(s.flag)
                && !self.seen_story.contains(&(k as u16))
        });
        chars || reports || limits || missions || minigames || synthesis || albums || story
    }
}

/// The face and photo files the caller has loaded for the journal, and
/// the ones it should load next.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JournalCache {
    /// `menu/jmface_<code>.2ld` by journal world (0x10 = Links).
    pub faces: HashSet<u8>,
    /// `menu/<lang>/jm_photo/<code><nn>.bin` by `albu` row.
    pub photos: HashSet<u16>,
    pub wanted_face: Option<u8>,
    pub wanted_photo: Option<u16>,
}

/// What a draw item samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    Art,
    Font,
    Icon,
    /// jm_world.2ld image i.
    World(usize),
    /// Image i of the page's jm_top file (`Journal::top`).
    Top(usize),
    /// Face image `.1` of the world's `jmface` file (`.0`).
    Face(u8, u8),
    /// The album photo of `albu` row `.0`.
    Photo(u16),
}

/// A dk::Sprite on another 2LD's image (FUN_14016a900 with an image
/// holder): its sequence, image and parent.
#[derive(Clone, Debug)]
struct Picture {
    prio: u32,
    sequence: usize,
    sheet: Sheet,
    player: Player,
    queued: Option<usize>,
    parent: Id,
    offset: [f32; 2],
    dead: bool,
}

/// FUN_1402f9440's results the journal handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum In {
    None,
    Row(usize),
    Cancel,
    Start,
    /// −8 / −9.
    R1,
    L1,
    /// R2 / L2 (the album photos).
    R2,
    L2,
}

/// FUN_1402f6270: the sound of a list result.
fn sound(input: In, moved: bool, valid: bool) -> Option<u32> {
    match input {
        _ if moved => Some(SOUND_CURSOR),
        In::Cancel | In::Start => Some(SOUND_CLOSE),
        In::Row(_) | In::R1 | In::L1 | In::R2 | In::L2 => {
            Some(if valid { SOUND_DECIDE } else { SOUND_INVALID })
        }
        In::None => None,
    }
}

/// The grid list fields FUN_1402f8240 reads: cursor (+0) and scroll (+0x12)
/// in cells, rows created (+4), columns (+0x18), visible rows (+0x1a), the
/// row wrap (+0x24) and the column wrap / centring (+0x26).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Grid {
    cursor: i32,
    scroll: i32,
    created: i32,
    cols: i32,
    rows_visible: i32,
    wrap_rows: bool,
    wrap_cols: bool,
}

impl Grid {
    /// FUN_1402f8240 for `n` cells: one 2-D step (UP/DOWN/LEFT/RIGHT with
    /// mask bits 0..3); a held key stops at the edges, a fresh press wraps,
    /// and a partial last row is centred.
    fn step(&mut self, n: i32, p: &Pad, mask: u32) {
        use bits::{DOWN, LEFT, RIGHT, UP};
        if n <= 0 {
            return;
        }
        let (rep, trig) = (p.repeat, p.trig);
        let cols = self.cols;
        let c0 = self.cursor;
        let s = self.scroll;
        let abs = c0 + s;
        let row = abs / cols;
        let col = abs % cols;
        let visible_cursor = if c0 >= 0 { c0 } else { abs };
        let mut row_len = cols;
        if rep & (LEFT | RIGHT) != 0 && n - n % cols <= abs {
            row_len = n % cols;
        }
        let rows = n / cols + i32::from(n % cols != 0);
        let up = rep & UP != 0 && mask & 1 != 0;
        let down = rep & DOWN != 0 && mask & 2 != 0;
        let left = rep & LEFT != 0 && mask & 4 != 0;
        let right = rep & RIGHT != 0 && mask & 8 != 0;
        if up {
            if (row == 0 && trig & UP == 0) || (rows == 1 && trig & UP != 0) {
                return;
            }
        } else if down && ((rows - 1 <= row && trig & DOWN == 0) || (rows == 1 && trig & DOWN != 0))
        {
            return;
        }
        if self.wrap_cols {
            if left {
                if (col == 0 && trig & LEFT == 0) || (row_len == 1 && trig & LEFT != 0) {
                    return;
                }
            } else if right
                && ((row_len - 1 <= col && trig & RIGHT == 0)
                    || (row_len == 1 && trig & RIGHT != 0))
            {
                return;
            }
        }
        let centre = |on: bool| {
            if on && self.wrap_cols && n % cols != 0 {
                (cols - n % cols) / 2
            } else {
                0
            }
        };
        if up {
            let off = centre(rows - 1 <= row || row == 0);
            if visible_cursor / cols >= 1 {
                self.cursor = c0 - cols + off;
            } else if s >= 1 {
                self.scroll = s - cols;
                self.cursor = c0 + off;
            } else if self.wrap_rows {
                let (last_row, scroll) = if self.rows_visible * cols < n {
                    let mut x = n - self.rows_visible * cols;
                    if x % cols != 0 {
                        x = x - x % cols + cols;
                    }
                    self.scroll = x;
                    (self.rows_visible, x)
                } else {
                    (rows, self.scroll)
                };
                let sub = if off <= col { off } else { col };
                self.cursor = (last_row - 1) * cols + col - sub;
                if n <= self.cursor + scroll {
                    self.cursor = n - scroll - 1;
                }
            }
        } else if down {
            let off = centre(rows - 2 <= row);
            let visible = if self.created < n {
                self.rows_visible
            } else {
                n / cols + i32::from(self.created % cols != 0)
            };
            if row < rows - 1 {
                let mut c = c0;
                if visible_cursor / cols < visible - 1 || rows <= self.rows_visible {
                    c = cols + c0;
                } else {
                    self.scroll += cols;
                }
                let sub = if off <= col { off } else { col };
                self.cursor = c - sub;
                if n <= self.cursor + self.scroll {
                    self.cursor = n - self.scroll - 1;
                }
            } else if self.wrap_rows {
                self.scroll = 0;
                self.cursor = col + off;
            }
        } else if left {
            let base = if col > 0 { c0 } else { row_len + c0 };
            self.cursor = base - 1;
        } else if right {
            let base = if col < row_len - 1 { c0 } else { c0 - row_len };
            self.cursor = base + 1;
        }
    }
}

/// FUN_1402f3b20: the (L1, R1) labels of the top tab ring for `state`;
/// Notes leaves the ring when the journal world is past 0xe.
fn tab_labels(state: u8, world: i32) -> (u16, u16) {
    let (l, k) = match state {
        1 | 2 => (0, 1),
        COLLECTION_PAGE => (1, 2),
        _ => (2, 0),
    };
    let mut left = TABS[l];
    let r = if k > 1 { 0 } else { k + 1 };
    let mut right = TABS[r];
    // The world as the exe's unsigned byte.
    if world as u8 > 0xe {
        let other = if k == 2 { 0 } else { 2 };
        if l == 1 {
            left = TABS[other];
        }
        if r == 1 {
            right = TABS[other];
        }
    }
    (left, right)
}

/// FUN_1402f3bd0: the states the (L1, R1) tabs lead to.
fn tab_targets(state: u8, world: i32) -> (u8, u8) {
    let target = |label: u16| match TABS.iter().position(|&t| t == label) {
        Some(0) => SELECT_WORLD,
        Some(1) => NOTES_PAGE,
        _ => COLLECTION_PAGE,
    };
    let (l, r) = tab_labels(state, world);
    (target(l), target(r))
}

pub struct Journal {
    pub layout: LayoutTask,
    pub sprites: Sprites,
    pictures: Vec<Picture>,
    data: JournalData,
    /// The layout scenes (a8[2]: Roxas's).
    scenes: [usize; 3],
    /// a8[0]: the journal world (−1 none, 0x13 Roxas).
    world: i32,
    pub state: u8,
    /// 0 opening, 1 interactive, 2 leaving for `pending`.
    pub substate: u8,
    pending: Option<u8>,
    pub exit: bool,
    pub closed: bool,
    /// DAT_1407434e0: the slide direction.
    dir: usize,
    list: MenuList,
    grid: Option<Grid>,
    /// Grid cell → worl row.
    cells: Vec<usize>,
    /// List row → option (Notes / Collection).
    options: Vec<usize>,
    container: Id,
    title: Option<Id>,
    tabs: [Option<Id>; 2],
    help_anchor: Id,
    help: Option<Id>,
    /// The page's sprites (5e8 objects), killed when the page goes.
    page: Vec<Id>,
    hand: Option<Id>,
    light: LightCur,
    /// DAT_140bee4f4 / 4f5: no hand, no LightCur.
    no_cursor: bool,
    /// DAT_140bee500: the remembered row per state (zeroed at the build).
    memory: [usize; 0x29],
    /// The page's jm_top file code (FUN_1402f4ae0).
    pub top: Option<String>,
    /// The category pages' pill and the pages' own state.
    cat: category::Category,
    files: files::Page,
    limits: limits::Page,
    lists: lists::Page,
    story: story::Page,
    synthesis: synthesis::Page,
    treasure: treasure::Page,
    /// First list entry shown by the 10-row file lists (the hand follows
    /// the row `cursor − scroll`).
    scroll: usize,
    chars: characters::Page,
    album: album::Page,
    /// Files loaded for the pages and the ones to load next.
    pub cache: JournalCache,
    /// This frame's held bits and left stick (x, y, magnitude).
    held: u64,
    stick: [f32; 3],
    /// DAT_140bee4f5: no LightCur (the hand stays).
    no_light: bool,
    /// Rows seen so far and when the journal opened.
    pub seen_chars: HashSet<u16>,
    pub seen_albums: HashSet<u16>,
    seen_chars_at_open: HashSet<u16>,
    seen_albums_at_open: HashSet<u16>,
}

impl Journal {
    /// FUN_1402f3d70 + FUN_1402f4bc0's build (the files are already
    /// loaded) and the open sound se000 #3.
    pub fn open(layout: &Layout, data: JournalData) -> (Self, Vec<u32>) {
        let seq = &layout.sequences[SEQUENCE];
        let roxas = data.kind == 2;
        let mut world = -1;
        let mut kind = data.kind;
        if kind != 2 {
            world = journal_world(data.world);
            if world < 0 {
                kind = 0;
            }
        }
        let first = match kind {
            1 => NOTES_PAGE,
            2 => {
                world = ROXAS_WORLD;
                ROXAS_NOTES
            }
            _ => SELECT_WORLD,
        };
        let scenes = if roxas { ROXAS_SCENES } else { JIMINY_SCENES };
        // FUN_1402f5410: the layout, the page container and the help anchor.
        let mut task = LayoutTask::new(layout, scenes[0]);
        task.next = Some(scenes[1]);
        let mut sprites = Sprites::default();
        let container = sprites.create(
            seq,
            PRIO_CONTAINER,
            ART,
            SLIDE[0][0],
            Some(CONTAINER_IDLE),
            None,
        );
        let help_anchor = sprites.create(
            seq,
            PRIO_FRAME_TEXT,
            ART,
            HELP_ANCHOR[0],
            Some(HELP_ANCHOR[1]),
            None,
        );
        let mut j = Journal {
            layout: task,
            sprites,
            pictures: Vec::new(),
            data,
            scenes,
            world,
            state: first,
            substate: 0,
            pending: None,
            exit: false,
            closed: false,
            dir: 0,
            list: MenuList::default(),
            grid: None,
            cells: Vec::new(),
            options: Vec::new(),
            container,
            title: None,
            tabs: [None, None],
            help_anchor,
            help: None,
            page: Vec::new(),
            hand: None,
            light: LightCur::default(),
            no_cursor: false,
            memory: [0; 0x29],
            top: None,
            cat: category::Category::default(),
            files: files::Page::default(),
            limits: limits::Page::default(),
            lists: lists::Page::default(),
            story: story::Page::default(),
            synthesis: synthesis::Page::default(),
            treasure: treasure::Page::default(),
            scroll: 0,
            chars: characters::Page::default(),
            album: album::Page::default(),
            cache: JournalCache::default(),
            held: 0,
            stick: [0.; 3],
            no_light: false,
            seen_chars: HashSet::new(),
            seen_albums: HashSet::new(),
            seen_chars_at_open: HashSet::new(),
            seen_albums_at_open: HashSet::new(),
        };
        j.seen_chars = j.data.seen_chars.iter().copied().collect();
        j.seen_albums = j.data.seen_albums.iter().copied().collect();
        j.seen_chars_at_open = j.seen_chars.clone();
        j.seen_albums_at_open = j.seen_albums.clone();
        j.files.seen_reports = j.data.seen_reports.iter().copied().collect();
        j.files.seen_reports_at_open = j.files.seen_reports.clone();
        j.limits.seen = j.data.seen_limits.iter().copied().collect();
        j.limits.seen_at_open = j.limits.seen.clone();
        j.lists.seen_quests = j.data.seen_quests.iter().copied().collect();
        j.lists.seen_quests_at_open = j.lists.seen_quests.clone();
        j.lists.seen_minis = j.data.seen_minigames.iter().copied().collect();
        j.lists.seen_minis_at_open = j.lists.seen_minis.clone();
        j.story.seen = j.data.seen_story.iter().copied().collect();
        j.story.seen_at_open = j.story.seen.clone();
        j.synthesis.seen = j.data.seen_synthesis.iter().copied().collect();
        j.synthesis.seen_at_open = j.synthesis.seen.clone();
        j.build(layout, first);
        (j, vec![SOUND_OPEN])
    }

    fn msg(&self, id: u16) -> Vec<u8> {
        self.data.texts.get(&id).cloned().unwrap_or_default()
    }

    fn flag(&self, f: u16) -> bool {
        self.data.flag(f)
    }

    fn show_all(&self) -> bool {
        self.flag(SHOW_ALL)
    }

    fn row_of_world(&self, world: i32) -> Option<&World> {
        self.data.worlds.iter().find(|w| w.world as i32 == world)
    }

    /// FUN_140326960(w, 0): the world's name (the alternate one once its
    /// flag is set).
    fn world_name(&self, world: i32) -> Vec<u8> {
        match self.row_of_world(world) {
            Some(w) if self.flag(w.alt_flag) => self.msg(w.alt_names[0]),
            Some(w) => self.msg(w.names[0]),
            None => Vec::new(),
        }
    }

    /// FUN_140344660: the last reached story step's Summary and Objective.
    fn story(&self, world: i32) -> (u16, u16) {
        let mut out = (NO_STORY, NO_STORY);
        for s in &self.data.stories {
            if s.world as i32 == world && self.flag(s.flag) {
                out = (s.summary, s.objective);
            }
        }
        out
    }

    /// FUN_1402f4ae0: the page's jm_top file (Roxas tt2; Hollow Bastion's
    /// alternate hb2; Collection row 0x0f's code).
    fn top_code(&self, state: u8) -> Option<String> {
        let code = match state {
            ROXAS_NOTES => "tt2".to_owned(),
            NOTES_PAGE => {
                let w = self.row_of_world(self.world)?;
                if self.world == 1 && self.flag(w.alt_flag) {
                    "hb2".to_owned()
                } else {
                    w.code.clone()
                }
            }
            COLLECTION_PAGE => self.row_of_world(0x0f)?.code.clone(),
            _ => return None,
        };
        self.data.tops.contains(&code).then_some(code)
    }

    fn text(bytes: Vec<u8>, size: f32) -> Text {
        Text::new(bytes, size)
    }

    /// The creation group's TextScale, else 15.
    fn size(seq: &Sequence, group: usize) -> f32 {
        match seq.groups[group].text_scale {
            0 => 15.,
            s => s as f32,
        }
    }

    // ---- frame -------------------------------------------------------

    /// FUN_1402f5780: the title and the L / R tabs if not alive.
    fn frame_texts(&mut self, seq: &Sequence) {
        let make = |sp: &mut Sprites, g: [usize; 3]| {
            let mut t = Self::text(Vec::new(), Self::size(seq, g[0]));
            t.align = 2;
            sp.create(seq, PRIO_FRAME_TEXT, ART, g[0], Some(g[1]), Some(t))
        };
        if self.title.is_none_or(|t| !self.sprites.alive(t)) {
            self.title = Some(make(&mut self.sprites, TITLE));
        }
        for (k, g) in [LEFT_TAB, RIGHT_TAB].into_iter().enumerate() {
            if self.tabs[k].is_none_or(|t| !self.sprites.alive(t)) {
                self.tabs[k] = Some(make(&mut self.sprites, g));
            }
        }
    }

    fn set_text(&mut self, id: Option<Id>, bytes: Vec<u8>) {
        if let Some(id) = id
            && let Some(t) = self.sprites.get_mut(id).text.as_mut()
        {
            t.bytes = bytes;
        }
    }

    /// FUN_1402f50c0: "<L1> label" and "label <R1>".
    fn set_tabs(&mut self, state: u8) {
        let (l, r) = tab_labels(state, self.world);
        let mut left = vec![0x09, L1_PROMPT];
        left.extend(self.msg(l));
        let mut right = self.msg(r);
        right.extend([0x09, R1_PROMPT]);
        self.set_text(self.tabs[0], left);
        self.set_text(self.tabs[1], right);
    }

    /// FUN_1402f4f40: the frame texts skip their intros.
    fn frame_idle(&mut self, seq: &Sequence) {
        let items = [
            (self.title, TITLE[1]),
            (Some(self.help_anchor), HELP_ANCHOR[1]),
            (self.tabs[0], LEFT_TAB[1]),
            (self.tabs[1], RIGHT_TAB[1]),
        ];
        for (id, idle) in items {
            if let Some(id) = id
                && self.sprites.alive(id)
                && self.sprites.get(id).player.group != idle
            {
                self.sprites.set(seq, id, idle, None);
            }
        }
    }

    fn close_if(&mut self, seq: &Sequence, id: Option<Id>, group: usize) {
        if let Some(id) = id
            && self.sprites.alive(id)
            && self.sprites.get(id).player.group != group
        {
            self.sprites.close(seq, id, group);
        }
    }

    /// FUN_1402f2940: a live help text swaps its text (back to G145 if it
    /// was fading); otherwise it is created on the help anchor.
    fn show_help(&mut self, seq: &Sequence, id: u16) {
        let bytes = self.msg(id);
        if let Some(h) = self.help
            && self.sprites.alive(h)
        {
            self.set_text(Some(h), bytes);
            if self.sprites.get(h).player.group != HELP_IN {
                self.sprites.set(seq, h, HELP_IN, None);
            }
            return;
        }
        let mut t = Self::text(bytes, Self::size(seq, HELP_IN));
        t.keep_rgb = false;
        let h = self.sprites.create(seq, BASE, ART, HELP_IN, None, Some(t));
        if self.sprites.alive(self.help_anchor) {
            self.sprites.attach(h, self.help_anchor);
        }
        self.help = Some(h);
    }

    /// FUN_1402f39e0.
    fn hide_help(&mut self, seq: &Sequence) {
        self.close_if(seq, self.help, HELP_OUT);
    }

    /// FUN_1402f5330: the container's in (phase 0, then idle), out (1) or
    /// idle (2) group for direction `dir`.
    fn slide(&mut self, seq: &Sequence, dir: usize, phase: usize) {
        let group = SLIDE[dir][phase];
        if !self.sprites.alive(self.container) {
            self.container = self
                .sprites
                .create(seq, PRIO_CONTAINER, ART, group, None, None);
        }
        if phase == 0 {
            self.sprites
                .set(seq, self.container, group, Some(CONTAINER_IDLE));
        } else {
            self.sprites.close(seq, self.container, group);
        }
        self.dir = dir;
    }

    /// FUN_1402f5dc0.
    fn container_idle(&self) -> bool {
        self.sprites.get(self.container).player.group == CONTAINER_IDLE
    }

    // ---- pages -------------------------------------------------------

    /// A page sprite attached to `parent`.
    fn page_sprite(
        &mut self,
        seq: &Sequence,
        prio: u32,
        group: usize,
        parent: Id,
        text: Option<Text>,
    ) -> Id {
        let id = self.sprites.create(seq, prio, ART, group, None, text);
        self.sprites.attach(id, parent);
        self.sprites.get_mut(id).die_with_parent = true;
        self.page.push(id);
        id
    }

    /// The page kill hooks (FUN_140326a70 / FUN_140327bd0 / FUN_14032bac0):
    /// every page sprite and picture goes; the list empties.
    fn kill_page(&mut self) {
        for id in self.page.drain(..) {
            self.sprites.kill(id);
        }
        // The Characters page's bounds anchor has no parent (S0).
        if let Some(a) = self.chars.take_anchor() {
            self.sprites.kill(a);
        }
        for p in &mut self.pictures {
            p.dead = true;
        }
        self.list.rows.clear();
        self.list.total = 0;
    }

    fn page_alive(&self) -> bool {
        self.page.iter().any(|&id| self.sprites.alive(id)) || self.pictures.iter().any(|p| !p.dead)
    }

    /// FUN_1402f7b70 for a fresh page list.
    fn reset_list(&mut self) {
        self.list = MenuList::default();
        self.list.mask = 0x73;
        self.scroll = 0;
        self.grid = None;
        self.cells.clear();
        self.options.clear();
    }

    fn build(&mut self, layout: &Layout, state: u8) {
        self.state = state;
        self.substate = 0;
        self.no_cursor = false;
        self.no_light = false;
        // The pill stays between a category's own pages only.
        if !category::is_category(state) {
            self.kill_pill();
        }
        match state {
            SELECT_WORLD => self.build_worlds(layout),
            ROXAS_NOTES => self.build_roxas(layout),
            NOTES_PAGE => self.build_notes(layout),
            COLLECTION_PAGE => self.build_collection(layout),
            4 => self.build_album_list(layout),
            5 => self.build_album_photo(layout),
            6 | 0xc | 0x28 => self.build_characters(layout),
            0xe => self.build_files_menu(layout),
            0x18 => self.build_reports_list(layout),
            0x19 => self.build_report_reader(layout),
            limits::LIST => self.build_limits_list(layout),
            limits::DETAIL => self.build_limit_detail(layout),
            lists::MISSIONS_NOTES
            | lists::MISSIONS_COLLECTION
            | lists::MINIGAMES_NOTES
            | lists::MINIGAMES_COLLECTION => self.build_list(layout),
            story::STATE => self.build_story(layout),
            synthesis::STATE => self.build_synthesis(layout),
            7 | 9 | 0x1a | 0x22 => self.build_treasure(layout),
            0xf | 0x12 | 0x15 => self.build_files_list(layout),
            0x10 | 0x13 | 0x16 => self.build_files_detail(layout),
            // Pages not built yet go back to the Notes of the world.
            _ => self.build_notes(layout),
        }
    }

    /// FUN_14016a900 on an image holder: a picture attached to `parent`
    /// (group 2 at a build, 1 → 2 when it arrives later).
    fn picture(
        &mut self,
        seq: &Sequence,
        sequence: usize,
        sheet: Sheet,
        prio: u32,
        parent: Id,
        offset: [f32; 2],
    ) {
        self.pictures.push(Picture {
            prio,
            sequence,
            sheet,
            player: Player::new(seq, PICTURE_STATIC),
            queued: None,
            parent,
            offset,
            dead: false,
        });
    }

    /// FUN_1403277b0 / FUN_14032b680: the title strip on P0 (offset by P0's
    /// group LightPositionX / TextPositionY) and the picture on P1.
    fn page_pictures(&mut self, layout: &Layout, p0: Id, p1: Id, prio: [u32; 2]) {
        if self.top.is_none() {
            return;
        }
        let seq = &layout.sequences[SEQUENCE];
        let g = &seq.groups[self.sprites.get(p0).player.group];
        let offset = [g.light_position_x as f32, g.text_position_y as f32];
        let strip = &layout.sequences[SEQ_STRIP];
        self.picture(strip, SEQ_STRIP, Sheet::Top(0), prio[0], p0, offset);
        let picture = &layout.sequences[SEQ_PICTURE];
        self.picture(picture, SEQ_PICTURE, Sheet::Top(1), prio[1], p1, [0.; 2]);
    }

    /// FUN_140326b60 + FUN_140326380: the world grid.
    fn build_worlds(&mut self, layout: &Layout) {
        let seq = &layout.sequences[SEQUENCE];
        self.reset_list();
        self.top = None;
        self.frame_texts(seq);
        self.set_tabs(SELECT_WORLD);
        let mut rows = Vec::new();
        for _ in 0..0x12 {
            let r = self.page_sprite(seq, BASE, GRID_ROW, self.container, None);
            self.sprites.get_mut(r).queued = Some(GRID_ROW);
            self.page_sprite(seq, BASE + 2, GRID_ROW, r, None);
            self.page_sprite(seq, BASE + 3, GRID_ROW, r, None);
            rows.push(r);
        }
        let show_all = self.show_all();
        for (k, w) in self.data.worlds.iter().enumerate() {
            if w.world < 0x10 && (self.flag(w.unlock) || show_all) {
                self.cells.push(k);
            }
        }
        let n = self.cells.len();
        for &r in rows.iter().take(n) {
            self.sprites.set(seq, r, CELL, None);
        }
        // FUN_140326380(0): the icons (locked worlds show image 16).
        if self.data.world_icons {
            for (k, &row) in self.cells.clone().iter().enumerate() {
                let w = &self.data.worlds[row];
                let image = if self.flag(w.unlock) {
                    w.world as usize
                } else {
                    16
                };
                self.pictures.push(Picture {
                    prio: BASE + 1,
                    sequence: SEQ_ICON,
                    sheet: Sheet::World(image),
                    player: Player::new(&layout.sequences[SEQ_ICON], PICTURE_STATIC),
                    queued: None,
                    parent: rows[k],
                    offset: [0.; 2],
                    dead: false,
                });
            }
        }
        // The cursor on the journal world (Collection when none).
        let target = if self.world < 0 { 0x0f } else { self.world };
        let mut cursor = None;
        let mut count = 0;
        for w in &self.data.worlds {
            if w.world < 0x14 && (self.flag(w.unlock) || show_all) {
                if w.world as i32 == target {
                    cursor = Some(count);
                    break;
                }
                count += 1;
            }
        }
        self.list = MenuList::new(rows.clone(), cursor.unwrap_or(0));
        self.list.total = n;
        self.list.cursor = cursor.unwrap_or(0).min(n.saturating_sub(1));
        self.list.mask = 0x7f | 0x3000;
        self.grid = Some(Grid {
            cursor: self.list.cursor as i32,
            scroll: 0,
            created: 0x12,
            cols: 6,
            rows_visible: 3,
            wrap_rows: true,
            wrap_cols: true,
        });
        // Cell offsets: rows of 6 centred, the last row on its own count.
        if n > 0 {
            let row_pitch = seq.groups[CELL].ui_padding;
            let col_pitch = seq.groups[CELL].text_position_x;
            let n = n as i32;
            let per_row = n.min(6);
            let lines = n / 6 + i32::from(n % 6 != 0);
            let mut x0 = -((per_row - 1) * col_pitch) / 2;
            let mut y = -((lines - 1) * row_pitch) / 2;
            for line in 0..lines {
                let first = line * 6;
                if n < first + 6 {
                    x0 = -((n % 6 - 1) * col_pitch) / 2;
                }
                let mut x = x0;
                for c in 0..per_row {
                    if let Some(&r) = rows.get((first + c) as usize)
                        && first + c < n
                    {
                        self.sprites.get_mut(r).offset = [x as f32, y as f32];
                    }
                    x += col_pitch;
                }
                y += row_pitch;
            }
        }
        self.cell_groups(seq);
    }

    /// FUN_1403270f0: the cursor cell pulses (G172), the others idle.
    fn cell_groups(&mut self, seq: &Sequence) {
        let cursor = self.list.cursor;
        for i in 0..self.list.total {
            let r = self.list.rows[i];
            if self.sprites.get(r).player.group == CELL_SELECTED {
                self.sprites.set(seq, r, CELL, None);
            }
            if i == cursor {
                self.sprites.set(seq, r, CELL_SELECTED, None);
            }
        }
    }

    /// FUN_1403271a0: the title is the cursor world's name ("?" marks while
    /// it is only listed by the show-all flag). With no cell nothing is set
    /// [unresolved: the exe reads row −1].
    fn world_title(&mut self) {
        let Some(&row) = self.cells.get(self.list.cursor) else {
            return;
        };
        let w = &self.data.worlds[row];
        let name = if self.flag(w.alt_flag) {
            self.msg(w.alt_names[0])
        } else {
            self.msg(w.names[0])
        };
        let title = if self.flag(w.unlock) {
            name
        } else {
            // FUN_1403008d0: one "?" per glyph (FUN_1403adcc0: 0x49).
            name.iter().filter(|&&b| b >= 0x20).map(|_| 0x49).collect()
        };
        self.set_text(self.title, title);
    }

    /// The Notes / Collection list rows (FUN_1402f8e20 with {3,3,3,3}) with
    /// their new (A) and status (B) marks.
    fn option_rows(&mut self, seq: &Sequence, p0: Id, count: usize, pitch: f32) -> Vec<Id> {
        let mut rows = Vec::new();
        for i in 0..count {
            let mut t = Self::text(Vec::new(), Self::size(seq, ROW));
            t.group_position = false;
            let r = self.page_sprite(seq, BASE, ROW, p0, Some(t));
            let s = self.sprites.get_mut(r);
            s.queued = Some(ROW);
            s.offset = [0., pitch * i as f32];
            self.page_sprite(seq, BASE + 1, ROW, r, None);
            self.page_sprite(seq, BASE + 2, ROW, r, None);
            rows.push(r);
        }
        rows
    }

    /// Summary / Objective texts with their label pills (shared by Notes
    /// and Roxas's notes).
    fn story_boxes(&mut self, seq: &Sequence, groups: [usize; 2], prio: u32) {
        let (summary, objective) = self.story(self.world);
        for (g, msg, label) in [
            (groups[0], summary, SUMMARY_LABEL),
            (groups[1], objective, OBJECTIVE_LABEL),
        ] {
            let mut t = Self::text(self.msg(msg), Self::size(seq, g));
            t.keep_rgb = false;
            let b = self.page_sprite(seq, prio, g, self.container, Some(t));
            let mut l = Self::text(self.msg(label), Self::size(seq, ROW));
            l.group_position = false;
            l.align = 2;
            self.page_sprite(seq, prio, ROW, b, Some(l));
        }
    }

    /// FUN_140327c00's rows: the Notes options this world shows.
    fn notes_options(&self) -> Vec<usize> {
        let u = self.data.unlocks;
        (0..9)
            .filter(|&i| match i {
                3 => self.world != 8,
                4 => u.pieces || self.show_all(),
                6 => u.missions || self.missions_available(Some(self.world)),
                7 => u.minigames || self.minigames_available(Some(self.world)),
                _ => true,
            })
            .collect()
    }

    /// FUN_14032b860's rows: the Collection options shown.
    fn collection_options(&self) -> Vec<usize> {
        let u = self.data.unlocks;
        let all = self.show_all();
        (0..12)
            .filter(|&i| match i {
                0 => u.reports || all,
                5 => u.pieces || all,
                7 => u.missions || all || self.missions_available(None),
                8 => u.minigames || all || self.minigames_available(None),
                9 => u.limits || all || self.data.limit_counts.iter().any(|&n| n > 0),
                10 => u.synthesis || all || self.synthesis_any(),
                _ => true,
            })
            .collect()
    }

    fn fill_options(&mut self, labels: &[(u16, u16, u8)], options: Vec<usize>) {
        for (k, &o) in options.iter().enumerate() {
            let text = self.msg(labels[o].0);
            self.set_text(self.list.rows.get(k).copied(), text);
        }
        self.list.total = options.len();
        self.list.cursor = self.list.cursor.min(options.len().saturating_sub(1));
        self.options = options;
    }

    /// FUN_140327d00 + FUN_140328740: the world's Notes.
    fn build_notes(&mut self, layout: &Layout) {
        let seq = &layout.sequences[SEQUENCE];
        self.reset_list();
        self.top = self.top_code(NOTES_PAGE);
        self.frame_texts(seq);
        self.set_tabs(NOTES_PAGE);
        let title = self.world_name(self.world);
        self.set_text(self.title, title);
        let c = self.container;
        let p0 = self.page_sprite(seq, BASE - 1, NOTES[0], c, None);
        let p1 = self.page_sprite(seq, BASE - 1, NOTES[1], c, None);
        self.story_boxes(seq, [NOTES[2], NOTES[3]], BASE - 1);
        let pitch = seq.groups[NOTES[0]].ui_padding as f32;
        let rows = self.option_rows(seq, p0, 9, pitch);
        self.page_pictures(layout, p0, p1, [BASE + 2, BASE + 1]);
        self.list = MenuList::new(rows, self.memory[NOTES_PAGE as usize]);
        self.list.mask = 0x73 | 0x3000;
        let options = self.notes_options();
        self.fill_options(&NOTES_OPTIONS, options);
        if let Some(&o) = self.options.get(self.list.cursor) {
            self.show_help(seq, NOTES_OPTIONS[o].1);
        }
    }

    /// FUN_14032bb90 + FUN_14032bf60: the Collection.
    fn build_collection(&mut self, layout: &Layout) {
        let seq = &layout.sequences[SEQUENCE];
        self.reset_list();
        self.top = self.top_code(COLLECTION_PAGE);
        self.frame_texts(seq);
        self.set_tabs(COLLECTION_PAGE);
        let title = self.world_name(0x0f);
        self.set_text(self.title, title);
        let c = self.container;
        let p0 = self.page_sprite(seq, BASE - 1, COLLECTION[0], c, None);
        let p1 = self.page_sprite(seq, BASE - 1, COLLECTION[1], c, None);
        let pitch = seq.groups[COLLECTION[0]].ui_padding as f32;
        let rows = self.option_rows(seq, p0, 12, pitch);
        self.page_pictures(layout, p0, p1, [BASE + 4, BASE + 4]);
        self.list = MenuList::new(rows, self.memory[COLLECTION_PAGE as usize]);
        let options = self.collection_options();
        self.fill_options(&COLLECTION_OPTIONS, options);
        self.list.mask = 0x73 | 0x3000;
        if let Some(&o) = self.options.get(self.list.cursor) {
            self.show_help(seq, COLLECTION_OPTIONS[o].1);
        }
    }

    /// FUN_140328330: Roxas's notes (no title, tabs, hand or LightCur; the
    /// list takes no decide).
    fn build_roxas(&mut self, layout: &Layout) {
        let seq = &layout.sequences[SEQUENCE];
        self.reset_list();
        self.top = self.top_code(ROXAS_NOTES);
        let c = self.container;
        let p0 = self.page_sprite(seq, BASE - 1, ROXAS[0], c, None);
        let p1 = self.page_sprite(seq, BASE - 1, ROXAS[1], c, None);
        self.story_boxes(seq, [ROXAS[2], ROXAS[3]], BASE + 3);
        let r = self.page_sprite(seq, BASE, ROW, p0, None);
        self.sprites.get_mut(r).queued = Some(ROW);
        self.page_pictures(layout, p0, p1, [BASE + 2, BASE + 1]);
        self.no_cursor = true;
        self.list = MenuList::new(vec![r], 0);
        self.list.mask = 0x73 & !0x10;
    }

    // ---- per frame ---------------------------------------------------

    /// FUN_1402f9440: the grid step or the vertical list, then the buttons
    /// under the list's enable mask.
    fn input(&mut self, pad: &Pad) -> (In, bool) {
        use bits::{CANCEL, DECIDE, L1, R1, START};
        let before = self.list.cursor;
        let n = self.list.total;
        if let Some(g) = self.grid.as_mut() {
            g.cursor = before as i32;
            g.step(n as i32, pad, self.list.mask);
            self.list.cursor = (g.cursor.max(0) as usize).min(n.saturating_sub(1));
        } else {
            let mask = self.list.mask;
            self.list.mask &= 0x3;
            self.list.input(pad);
            self.list.mask = mask;
        }
        if self.list.cursor != before {
            return (In::None, true);
        }
        let m = self.list.mask;
        let input = if pad.trig & DECIDE != 0 && m & 0x10 != 0 && n > 0 {
            In::Row(self.list.cursor)
        } else if pad.trig & CANCEL != 0 && m & 0x20 != 0 {
            In::Cancel
        } else if pad.trig & START != 0 && m & 0x40 != 0 {
            In::Start
        } else if pad.trig & R1 != 0 && m & 0x1000 != 0 {
            In::R1
        } else if pad.trig & L1 != 0 && m & 0x2000 != 0 {
            In::L1
        } else if pad.trig & bits::R2 != 0 && m & 0x4000 != 0 {
            In::R2
        } else if pad.trig & bits::L2 != 0 && m & 0x8000 != 0 {
            In::L2
        } else {
            In::None
        };
        (input, false)
    }

    /// One journal frame: the state machine (FUN_1402f24a0), the driver
    /// (FUN_1402f2cf0), then the sprite tasks.
    pub fn frame(
        &mut self,
        layout: &Layout,
        trig: u64,
        repeat: u64,
        held: u64,
        stick: [f32; 3],
    ) -> Vec<u32> {
        let seq = &layout.sequences[SEQUENCE];
        let pad = Pad { trig, repeat };
        self.held = held;
        self.stick = stick;
        let mut sounds = Vec::new();
        if !self.exit {
            // No input while the book opens; any press skips it to idle.
            let opening = !self.layout.dead && self.layout.scene != self.scenes[1];
            if opening && trig != 0 {
                self.layout.set_scene(self.scenes[1], None);
            }
            if !opening || trig != 0 {
                let moved = match self.state {
                    SELECT_WORLD => self.select_world(layout, &pad, &mut sounds),
                    ROXAS_NOTES => self.roxas_notes(&pad, seq, &mut sounds),
                    4 => self.album_list(layout, &pad, &mut sounds),
                    5 => self.album_photo(layout, &pad, &mut sounds),
                    6 | 0xc | 0x28 => self.characters(layout, &pad, &mut sounds),
                    0xe => self.files_menu(layout, &pad, &mut sounds),
                    0x18 => self.reports_list(layout, &pad, &mut sounds),
                    0x19 => self.report_reader(layout, &pad, &mut sounds),
                    limits::LIST => self.limits_list(layout, &pad, &mut sounds),
                    limits::DETAIL => self.limit_detail(layout, &pad, &mut sounds),
                    lists::MISSIONS_NOTES
                    | lists::MISSIONS_COLLECTION
                    | lists::MINIGAMES_NOTES
                    | lists::MINIGAMES_COLLECTION => self.list_page(layout, &pad, &mut sounds),
                    story::STATE => self.story_page(layout, &pad, &mut sounds),
                    synthesis::STATE => self.synthesis_page(layout, &pad, &mut sounds),
                    7 | 9 | 0x1a | 0x22 => self.treasure_page(layout, &pad, &mut sounds),
                    0xf | 0x12 | 0x15 => self.files_list(layout, &pad, &mut sounds),
                    0x10 | 0x13 | 0x16 => self.files_detail(layout, &pad, &mut sounds),
                    _ => self.option_page(layout, &pad, &mut sounds),
                };
                self.driver(seq, moved);
            }
        }
        if self.exit {
            self.close(seq);
        }
        self.layout.update(layout, 1.);
        self.sprites.update(seq);
        self.update_pictures(layout);
        self.place_light(seq);
        self.place_pill_orb(seq);
        if self.exit && self.layout.dead {
            // FUN_1402f4350: everything goes; the wrapper rebuilds the camp.
            for s in &mut self.sprites.list {
                s.dead = true;
            }
            self.pictures.clear();
            self.closed = true;
        }
        sounds
    }

    /// The common leave step of the page handlers: wait for the old page
    /// (any press kills it now), then commit the pending state and slide
    /// the new page in.
    fn leave(&mut self, layout: &Layout, input: In, moved: bool, rows: bool) {
        let seq = &layout.sequences[SEQUENCE];
        let rows_alive = rows && self.list.rows.iter().any(|&r| self.sprites.alive(r));
        if self.sprites.alive(self.container) || self.page_alive() || rows_alive {
            if input != In::None || moved {
                self.kill_page();
            }
            return;
        }
        self.kill_page();
        let state = self.pending.take().unwrap_or(SELECT_WORLD);
        self.slide(seq, self.dir, 0);
        self.build(layout, state);
    }

    /// While the container slides in, a press puts it (and the frame
    /// texts) at idle; once idle the page turns interactive.
    fn settle(&mut self, seq: &Sequence, input: In, moved: bool, texts: bool) -> bool {
        if (input != In::None || moved) && !self.container_idle() {
            self.slide(seq, 0, 2);
            if texts {
                self.frame_idle(seq);
            }
        }
        if self.substate == 0 && self.container_idle() {
            self.substate = 1;
            return true;
        }
        false
    }

    /// L1 / R1 to another top page (FUN_1402f3bd0).
    fn switch_top(&mut self, seq: &Sequence, input: In) {
        let (l, r) = tab_targets(self.state, self.world);
        let (target, dir) = if input == In::R1 { (r, 0) } else { (l, 1) };
        self.slide(seq, dir, 1);
        self.hide_help(seq);
        self.pending = Some(target);
        self.substate = 2;
    }

    /// The top-tab validators: Notes is refused past world 0xe.
    fn tab_valid(&self, input: In) -> bool {
        let (l, r) = tab_targets(self.state, self.world);
        let target = if input == In::R1 { r } else { l };
        target != NOTES_PAGE || (self.world as u8) < 0xf
    }

    /// FUN_1403260f0.
    fn select_world(&mut self, layout: &Layout, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let seq = &layout.sequences[SEQUENCE];
        let (input, moved) = self.input(pad);
        if self.substate == 2 {
            self.leave(layout, input, moved, false);
            return false;
        }
        if self.settle(seq, input, moved, true) {
            self.world_title();
            self.show_help(seq, HELP_WORLDS);
        }
        let valid = match input {
            In::R1 | In::L1 => self.tab_valid(input),
            In::Row(k) => {
                let w = &self.data.worlds[self.cells[k]];
                self.flag(w.unlock)
            }
            _ => true,
        };
        if let Some(s) = sound(input, moved, valid) {
            sounds.push(s);
        }
        if !valid {
            return moved;
        }
        match input {
            In::Cancel | In::Start => self.exit = true,
            In::R1 | In::L1 => self.switch_top(seq, input),
            In::Row(k) => {
                let w = self.data.worlds[self.cells[k]].world as i32;
                self.world = w;
                self.slide(seq, 0, 1);
                self.hide_help(seq);
                self.pending = Some(if w < 0xf { NOTES_PAGE } else { COLLECTION_PAGE });
                self.substate = 2;
            }
            In::R2 | In::L2 | In::None => {}
        }
        moved
    }

    /// FUN_140327330 / FUN_14032b210: Notes and Collection.
    fn option_page(&mut self, layout: &Layout, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let seq = &layout.sequences[SEQUENCE];
        let (input, moved) = self.input(pad);
        if self.substate == 2 {
            self.leave(layout, input, moved, true);
            return false;
        }
        self.settle(seq, input, moved, true);
        let valid = match input {
            In::R1 | In::L1 => self.tab_valid(input),
            In::Row(k) => self.options.get(k).is_some(),
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
                self.slide(seq, 1, 1);
                self.hide_help(seq);
                self.pending = Some(SELECT_WORLD);
                self.substate = 2;
            }
            In::R1 | In::L1 => self.switch_top(seq, input),
            In::Row(k) => {
                // The pages built so far; the rest play the decide sound.
                let table: &[(u16, u16, u8)] = if self.state == NOTES_PAGE {
                    &NOTES_OPTIONS
                } else {
                    &COLLECTION_OPTIONS
                };
                if let Some(&o) = self.options.get(k) {
                    let (label, _, state) = table[o];
                    if matches!(
                        state,
                        3 | 4
                            | 6
                            | 0xc
                            | 0x28
                            | 0xe
                            | 0x12
                            | 0x15
                            | 7
                            | 9
                            | 0x1a
                            | 0x22
                            | 0x18
                            | 0x25
                            | 0xa
                            | 0xb
                            | 0x23
                            | 0x24
                            | 0x27
                    ) {
                        self.enter_category(seq, state, label, k);
                    }
                }
            }
            In::R2 | In::L2 | In::None => {}
        }
        moved
    }

    /// FUN_1403276e0: cancel or START close the journal.
    fn roxas_notes(&mut self, pad: &Pad, seq: &Sequence, sounds: &mut Vec<u32>) -> bool {
        let (input, moved) = self.input(pad);
        if (input != In::None || moved) && !self.container_idle() {
            self.slide(seq, 0, 2);
        }
        if self.substate == 0 && self.container_idle() {
            self.substate = 1;
        }
        if let Some(s) = sound(input, moved, true) {
            sounds.push(s);
        }
        if matches!(input, In::Cancel | In::Start) {
            self.exit = true;
        }
        moved
    }

    /// FUN_1402f2cf0 (exit branch in `close`): the hand and LightCur on the
    /// cursor row; a move re-attaches them and runs the page's move hook.
    fn driver(&mut self, seq: &Sequence, moved: bool) {
        let live = self.list.rows[..self.list.total.min(self.list.rows.len())]
            .iter()
            .any(|&r| self.sprites.alive(r));
        let row = self
            .list
            .rows
            .get(self.list.cursor.saturating_sub(self.scroll))
            .copied();
        if !self.no_cursor && live && self.substate == 1 {
            if self.hand.is_none_or(|h| !self.sprites.alive(h))
                && let Some(row) = row
            {
                let h = self
                    .sprites
                    .create(seq, PRIO_CURSOR, ART, HAND, Some(HAND), None);
                self.sprites.get_mut(h).parent = Some(row);
                self.hand = Some(h);
                if !self.no_light && self.light.id.is_none_or(|l| !self.sprites.alive(l)) {
                    let l =
                        self.sprites
                            .create(seq, PRIO_CURSOR, ART, LIGHT[0], Some(LIGHT[1]), None);
                    self.light = LightCur::new(l);
                }
            }
        } else if self.no_cursor || self.substate == 2 {
            if let Some(h) = self.hand.take() {
                self.sprites.kill(h);
            }
            self.close_light(seq);
        }
        if self.substate == 1 && moved {
            if let (Some(h), Some(row)) = (self.hand, row)
                && self.sprites.alive(h)
            {
                self.sprites.get_mut(h).parent = Some(row);
            }
            self.light.retarget();
            match self.state {
                SELECT_WORLD => {
                    self.world_title();
                    self.cell_groups(seq);
                }
                NOTES_PAGE => {
                    if let Some(&o) = self.options.get(self.list.cursor) {
                        self.show_help(seq, NOTES_OPTIONS[o].1);
                    }
                }
                COLLECTION_PAGE => {
                    if let Some(&o) = self.options.get(self.list.cursor) {
                        self.show_help(seq, COLLECTION_OPTIONS[o].1);
                    }
                }
                _ => {}
            }
        }
    }

    fn close_light(&mut self, seq: &Sequence) {
        if let Some(l) = self.light.id
            && self.sprites.alive(l)
            && self.sprites.get(l).player.group != LIGHT[2]
        {
            self.sprites.close(seq, l, LIGHT[2]);
        }
    }

    /// FUN_1402f2cf0's exit branch with FUN_1402f37c0: the book closes, the
    /// container slides out to the right, the frame texts close, the help
    /// and the hand go at once.
    fn close(&mut self, seq: &Sequence) {
        if !self.layout.dead && self.layout.scene != self.scenes[2] {
            self.layout.set_scene(self.scenes[2], None);
        }
        if self.sprites.alive(self.container)
            && self.sprites.get(self.container).player.group != CONTAINER_EXIT
        {
            self.sprites.close(seq, self.container, CONTAINER_EXIT);
            self.dir = 1;
        }
        self.close_light(seq);
        self.pill_out(seq);
        if let Some(h) = self.help.take() {
            self.sprites.kill(h);
        }
        self.close_if(seq, self.title, TITLE[2]);
        self.close_if(seq, Some(self.help_anchor), HELP_ANCHOR[2]);
        self.close_if(seq, self.tabs[0], LEFT_TAB[2]);
        self.close_if(seq, self.tabs[1], RIGHT_TAB[2]);
        if let Some(h) = self.hand.take() {
            self.sprites.kill(h);
        }
    }

    /// The pictures' players; they die with their parents.
    fn update_pictures(&mut self, layout: &Layout) {
        for p in &mut self.pictures {
            if p.dead || !self.sprites.alive(p.parent) {
                p.dead = true;
                continue;
            }
            let seq = &layout.sequences[p.sequence];
            p.player.update(seq, 1.);
            if seq.groups[p.player.group].play_count != 0
                && p.player.finished()
                && let Some(next) = p.queued.take()
            {
                p.player = Player::new(seq, next);
            }
        }
        self.pictures.retain(|p| !p.dead);
    }

    /// The LightCur on the cursor row: the row's context + its group's
    /// (LightPositionX, TextPositionY).
    fn place_light(&mut self, seq: &Sequence) {
        if self.light.id.is_none_or(|l| !self.sprites.alive(l)) {
            return;
        }
        let Some(&row) = self.list.rows.get(self.list.cursor) else {
            return;
        };
        if !self.sprites.alive(row) {
            return;
        }
        let c = self.sprites.context(row);
        let g = &seq.groups[self.sprites.get(row).player.group];
        let t = [
            c[0] + g.light_position_x as f32,
            c[1] + g.text_position_y as f32,
        ];
        self.light.place(&mut self.sprites, t);
    }

    /// The seen rows to carry to the next opening (save+0x4c38 / 0x4c7c).
    pub fn seen(&self) -> (Vec<u16>, Vec<u16>) {
        let mut c: Vec<u16> = self.seen_chars.iter().copied().collect();
        let mut a: Vec<u16> = self.seen_albums.iter().copied().collect();
        c.sort_unstable();
        a.sort_unstable();
        (c, a)
    }

    /// The seen Synthesis Notes lines (save+0x4d78).
    pub fn seen_synthesis(&self) -> Vec<u8> {
        let mut s: Vec<u8> = self.synthesis.seen.iter().copied().collect();
        s.sort_unstable();
        s
    }

    /// The seen `stor` rows to carry to the next opening (save+0x4c38 +
    /// 0xac).
    pub fn seen_story(&self) -> Vec<u16> {
        let mut s: Vec<u16> = self.story.seen.iter().copied().collect();
        s.sort_unstable();
        s
    }

    /// The seen `ques` and `mini` rows to carry to the next opening
    /// (save+0x4c38 + 0x94 / 0x88).
    pub fn seen_lists(&self) -> (Vec<u16>, Vec<u16>) {
        let mut q: Vec<u16> = self.lists.seen_quests.iter().copied().collect();
        let mut m: Vec<u16> = self.lists.seen_minis.iter().copied().collect();
        q.sort_unstable();
        m.sort_unstable();
        (q, m)
    }

    /// The seen `anse` rows to carry to the next opening (save+0x4d74).
    pub fn seen_reports(&self) -> Vec<u16> {
        let mut r: Vec<u16> = self.files.seen_reports.iter().copied().collect();
        r.sort_unstable();
        r
    }

    /// The seen `limi` rows to carry to the next opening (save+0x4c38 +
    /// 0x84).
    pub fn seen_limits(&self) -> Vec<u16> {
        let mut l: Vec<u16> = self.limits.seen.iter().copied().collect();
        l.sort_unstable();
        l
    }

    /// The draw list: the book layout (0x7557), then sprites and pictures
    /// in priority order.
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
        let mut prios: Vec<u32> = self.pictures.iter().map(|p| p.prio).collect();
        prios.sort_unstable();
        prios.dedup();
        let mut low = 0;
        for prio in prios {
            out.extend(convert(
                self.sprites
                    .draw_where(seq, font, |p| (low..=prio).contains(&p)),
            ));
            for p in self.pictures.iter().filter(|p| p.prio == prio && !p.dead) {
                if !self.sprites.alive(p.parent) {
                    continue;
                }
                let c = self.sprites.context(p.parent);
                let origin = [c[0] + p.offset[0], c[1] + p.offset[1]];
                let a = self.sprites.child_alpha(p.parent);
                let m = Modifiers {
                    color: [1., 1., 1., a],
                    ..Modifiers::default()
                };
                out.push((
                    p.sheet,
                    layout.sequences[p.sequence].quads(&p.player, origin, &m),
                ));
            }
            low = prio + 1;
        }
        out.extend(convert(self.sprites.draw_where(seq, font, |p| p >= low)));
        out
    }
}

/// The menu-chain curtain (DAT_140bef390, prio 0x7554): pause.2ld sequence
/// 0, G5 fades to black (16 f) then holds G1; G4 fades back (16 f) and the
/// curtain dies.
#[derive(Clone, Debug)]
pub struct Curtain {
    player: Player,
    queued: Option<usize>,
    die_when_done: bool,
    pub dead: bool,
}

pub const CURTAIN_SEQUENCE: usize = 0;
const CURTAIN_IN: usize = 5;
const CURTAIN_HOLD: usize = 1;
const CURTAIN_OUT: usize = 4;

impl Curtain {
    /// FUN_14031adc0.
    pub fn cover(layout: &Layout) -> Self {
        let seq = &layout.sequences[CURTAIN_SEQUENCE];
        Self {
            player: Player::new(seq, CURTAIN_IN),
            queued: Some(CURTAIN_HOLD),
            die_when_done: false,
            dead: false,
        }
    }

    /// The wrapper's callback test: the curtain holds black (or is gone).
    pub fn covered(&self) -> bool {
        self.dead || self.player.group == CURTAIN_HOLD
    }

    /// FUN_14031a690.
    pub fn uncover(&mut self, layout: &Layout) {
        let seq = &layout.sequences[CURTAIN_SEQUENCE];
        self.player = Player::new(seq, CURTAIN_OUT);
        self.queued = None;
        self.die_when_done = true;
    }

    pub fn update(&mut self, layout: &Layout) {
        if self.dead {
            return;
        }
        let seq = &layout.sequences[CURTAIN_SEQUENCE];
        self.player.update(seq, 1.);
        if seq.groups[self.player.group].play_count != 0 && self.player.finished() {
            if let Some(next) = self.queued.take() {
                self.player = Player::new(seq, next);
                self.player.update(seq, 1.);
            } else if self.die_when_done {
                self.dead = true;
            }
        }
    }

    pub fn quads(&self, layout: &Layout) -> Vec<Quad> {
        if self.dead {
            return Vec::new();
        }
        layout.sequences[CURTAIN_SEQUENCE].quads(&self.player, [0., 0.], &Modifiers::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camp_badge_asks_for_an_available_unseen_entry() {
        let char = Char {
            world: 0,
            image: 0,
            frame: 0,
            bit: 0,
            name: 0,
            description: 0,
            origin: 0,
            flags: 0,
        };
        let mut d = JournalData {
            flags: vec![0x10],
            chars: vec![char; 2],
            diag: vec![Diag {
                cond: 0x10,
                off: 0,
                world: 0,
                kind: 0,
                entries: vec![data::journal_table::DiagEntry {
                    character: 1,
                    at: [0, 0],
                    visible: true,
                }],
            }],
            ..Default::default()
        };
        assert!(d.any_new());
        d.seen_chars = vec![1];
        assert!(!d.any_new());
        // A story entry of world 0xf or above is not asked about.
        let story = |world| Story {
            world,
            summary: 0,
            objective: 0,
            entry: 0,
            flag: 0x10,
        };
        d.stories = vec![story(0xf)];
        assert!(!d.any_new());
        d.stories = vec![story(0xf), story(3)];
        assert!(d.any_new());
        d.seen_story = vec![1];
        assert!(!d.any_new());
        // Synthesis lines 0 and 3 follow the Moogle; seeing both clears it.
        d.synthesis.moogle_seen = true;
        assert!(d.any_new());
        d.seen_synthesis = vec![0, 3];
        assert!(!d.any_new());
        // An owned report not yet read.
        d.reports = vec![data::journal_table::Report {
            item: 0x200,
            title: 0,
            text: 0,
        }];
        assert!(!d.any_new());
        d.owned_items = vec![0x200];
        assert!(d.any_new());
        d.seen_reports = vec![0];
        assert!(!d.any_new());
    }

    fn pad(trig: u64, repeat: u64) -> Pad {
        Pad { trig, repeat }
    }

    fn grid(cursor: i32) -> Grid {
        Grid {
            cursor,
            scroll: 0,
            created: 0x12,
            cols: 6,
            rows_visible: 3,
            wrap_rows: true,
            wrap_cols: true,
        }
    }

    #[test]
    fn the_grid_steps_wraps_and_centres_the_last_row() {
        use bits::{DOWN, LEFT, RIGHT, UP};
        // 8 cells: a row of 6 and a centred row of 2 (offset 2).
        let mut g = grid(3);
        g.step(8, &pad(DOWN, DOWN), 0x7f);
        assert_eq!(g.cursor, 7);
        g.step(8, &pad(UP, UP), 0x7f);
        assert_eq!(g.cursor, 3);
        // A held RIGHT stops at the row end; a fresh press wraps.
        let mut g = grid(5);
        g.step(8, &pad(0, RIGHT), 0x7f);
        assert_eq!(g.cursor, 5);
        g.step(8, &pad(RIGHT, RIGHT), 0x7f);
        assert_eq!(g.cursor, 0);
        g.step(8, &pad(LEFT, LEFT), 0x7f);
        assert_eq!(g.cursor, 5);
        // From the last row DOWN wraps to the first, UP from the first row
        // wraps to the last.
        let mut g = grid(6);
        g.step(8, &pad(DOWN, DOWN), 0x7f);
        assert_eq!(g.cursor, 2);
        let mut g = grid(0);
        g.step(8, &pad(UP, UP), 0x7f);
        assert_eq!(g.cursor, 6);
    }

    #[test]
    fn the_tab_ring_follows_the_exe_table() {
        assert_eq!(tab_labels(0, 0), (0x2f61, 0x2f62));
        assert_eq!(tab_labels(2, 0), (0x2f60, 0x2f61));
        assert_eq!(tab_labels(0xd, 0), (0x2f62, 0x2f60));
        assert_eq!(tab_targets(2, 0), (SELECT_WORLD, COLLECTION_PAGE));
        // No world: Notes leaves the ring.
        assert_eq!(tab_labels(0, -1), (0x2f61, 0x2f61));
        assert_eq!(tab_labels(0xd, -1), (0x2f60, 0x2f60));
        assert_eq!(journal_world(2), 0);
        assert_eq!(journal_world(6), 3);
        assert_eq!(journal_world(3), -1);
    }
}
