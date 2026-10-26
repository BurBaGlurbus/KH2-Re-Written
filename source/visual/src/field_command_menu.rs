//! The field command menu, translated from the executable's
//! dk::COMMAND_DRAW: update FUN_14015e800, level builder FUN_14015b6d0,
//! cursor FUN_14015ba00, cursor decoration FUN_14015bc30, slot fetch
//! FUN_14015d140, slot draw FUN_14015b4a0 and its text FUN_14016ae30. Input
//! follows FUN_1403b11e0 (wrapping up/down, right opens) and the manager's
//! FUN_1403f2470 (left swaps the top deck, menu 0, with the second deck,
//! menu 6). The art set follows the battle style; the art itself comes
//! from the loaded world layout. Layout units are the PS2 512x416 space;
//! the menu's X origin is -85 at 16:9.
use crate::gauge::Draw;
use data::sprite_sequence::{Modifiers, Player, Quad, Sequence, color};

/// X origin of the command menu outside exact 4:3 (0xffffffab).
pub const X_ORIGIN: f32 = -85.;
/// Root base (FUN_14016b330(0x1c, 0x142)).
const ROOT_BASE: [f32; 2] = [28., 322.];
/// Row pitch, first-row anchor (row n-1 sits at y 388), cursor shift.
const PITCH: f32 = 22.;
const ROW_ANCHOR: f32 = 410.;
const PANEL_ANCHOR: f32 = 388.;
const CURSOR_SHIFT: f32 = 8.;
/// Text offset, size and font reference height (font 1: 18 px).
const TEXT_OFFSET: [f32; 2] = [10., -8.];
const TEXT_SIZE: f32 = 14.;
const FONT_REFERENCE: f32 = 18.;
/// System font cell and page geometry (international: 18x24, 28 per row).
const CELL: [f32; 2] = [18., 24.];
const CELL_COLUMNS: usize = 28;

/// Root deck kinds (mgr+0xbd4): the top menu and the second deck
/// (Attack, Summon, Party, Limit; FUN_1403f3860), swapped by FUN_1403f2470.
pub const FIRST_DECK: usize = 0;
pub const SECOND_DECK: usize = 6;
/// Groups of kinds 0 and 6 by battle style (0 normal, 1 battle, 2 forced)
/// from the tables at 0x140717ef0 (cursor, row, last row), 0x140718280
/// (panel on a deck swap), 0x140718290 (panel intro), 0x1407182a0 (panel
/// loop), 0x1407182b0 (cursor refresh) and 0x1407182c0 (icon frame); the
/// root's loop is 0x140718388.
const ROOT_LOOP: usize = 6;
const PANEL_SWAP: [usize; 3] = [19, 88, 32];
const PANEL_INTRO: [usize; 3] = [17, 86, 30];
const PANEL_LOOP: [usize; 3] = [18, 87, 31];
const CURSOR: [usize; 3] = [9, 78, 22];
const CURSOR_REFRESH: [usize; 3] = [10, 79, 23];
const ROW: [usize; 3] = [16, 85, 29];
const LAST_ROW: [usize; 3] = [11, 80, 24];
const ICON_FRAME: [usize; 3] = [45, 47, 46];
/// Rows a command override replaced (item flag bit 0): the override art
/// played in (0x1407182d0, cursor then rows and last row) and its loop
/// (0x140718300), by style.
const OVERRIDE_IN: [usize; 12] = [14, 83, 27, 13, 82, 26, 14, 83, 27, 14, 83, 27];
const OVERRIDE: [usize; 12] = [15, 84, 28, 12, 81, 25, 15, 84, 28, 15, 84, 28];
/// The bouncing pointer: 20 beside the top deck, 21 beside any other kind
/// (FUN_14015bc30).
const POINTER: usize = 20;
const POINTER_OTHER: usize = 21;
/// Submenu art groups by menu kind (T_EF0[kind]: panel, cursor, row, last
/// row; equal in every style): Magic 1, Items 2, Drive 3, the party pick
/// 4, Summon 7, Party 8, Limit 9.
fn sub_groups(kind: usize) -> Option<[usize; 4]> {
    match kind {
        1 => Some([60, 57, 59, 58]),
        2 => Some([51, 48, 50, 49]),
        3 => Some([77, 74, 76, 75]),
        4 | 11 => Some([64, 61, 63, 62]),
        7 => Some([73, 70, 72, 71]),
        8 => Some([36, 33, 35, 34]),
        9 => Some([56, 53, 55, 54]),
        // The Limit Form's limits (T_EF0[12]).
        12 => Some([52, 53, 55, 54]),
        _ => None,
    }
}
/// Level step: each submenu level sits 16 units right of the one below.
const LEVEL_STEP: f32 = 16.;

/// The reaction slot's groups (0x140718358, both rows equal): shown 2, mode
/// 7 (the partner's) 5, record flag bit 3 set 3; empty 0 (the fade-out),
/// empty with the taken command's text 1; the ReactionDecideSprite burst 4.
const REACTION_SHOWN: usize = 2;
const REACTION_PARTNER: usize = 5;
const REACTION_FLAG3: usize = 3;
const REACTION_EMPTY: usize = 0;
const REACTION_EMPTY_TEXT: usize = 1;
const REACTION_BURST: usize = 4;
/// FUN_14015d670: the slot sits at (0x24, 0x179 - 22n).
const REACTION_X: f32 = 36.;
/// 0x1405b1c50: the slot's RGB multiplier by the candidate's mode.
const REACTION_RGB: [f32; 10] = [1., 1., 1., 1., 0.6, 0.6, 0.6, 1., 1., 0.95];
/// FUN_1402ebe70(0x1000): the Triangle prompt's glyph.
pub const TRIANGLE_GLYPH: u8 = 0xdf;
/// The list path's groups in deck mode 0xE (0x140718330 + 0x14): the
/// header, then the row by selector (0 none, 1..4).
const LIST_HEADER: usize = 8;
const LIST_ROW: [usize; 5] = [0, 0xa, 0xc, 0xb, 9];
/// The world map's deck (kind 10): FUN_14015df90 takes the header group 8
/// but its rows from the table's first five (0x140718330, the shown deck
/// not being 0xE).
pub const WORLD_MAP_DECK: u8 = 10;
const WORLD_LIST_ROW: [usize; 5] = [0x41, 0x43, 0x45, 0x44, 0x42];
/// FUN_14015df90's glyph for a list row's selector in deck mode 0xE (0
/// none): 1 0xdb, 2 and 3 FUN_1402ebe70(0x1000 / 0x8000) = 0xdf / 0xde
/// (the table at 0x1405ba7e0), 4 0xda.
const LIST_GLYPH: [Option<u8>; 5] = [None, Some(0xdb), Some(0xdf), Some(0xde), Some(0xda)];
/// FUN_14015aba0's first pen move by the deck's sheet mode (PLAYER_COMMAND
/// +0xab0, tested by FUN_1403b16e0..FUN_1403b1820), the low byte of x =
/// 0xffXX: mode 3 (a limit's `comd`) 0xe2, x = -30; mode 5 (Chicken
/// Little's) 0xe3, x = -29, as FUN_14015aab0 also gives it [I: in the
/// layout DAT_14079cf00 +0x274 other than 0 and 7].
pub fn list_pen(mode: u8) -> u8 {
    match mode {
        2 => 0xdd,
        3 | 4 | 10 => 0xe2,
        0xb => 0xe4,
        1 | 9 => 0xde,
        5 => 0xe3,
        6 | 7 => 0xdc,
        8 => 0xe1,
        0xc => 0xe0,
        _ => 0xff,
    }
}

/// FUN_1401534b0: the style of a world's command layout (its file name,
/// `<world><n>command.2dd`), which FUN_140153ee0 keeps (DAT_1409babd0)
/// for the reaction-style labels. `alt` is the layout DAT_14079cf00 +0x274
/// other than 0 and 7 ([I] the room's).
pub fn layout_style(name: &str, alt: bool) -> u8 {
    let file = name.rsplit('/').next().unwrap_or(name);
    let code = file.strip_suffix("command.2dd").unwrap_or(file);
    match code {
        "bb0" => {
            if alt {
                10
            } else {
                4
            }
        }
        "tt6" => {
            if alt {
                0xb
            } else {
                3
            }
        }
        "he1" | "lm0" | "wi0" | "lk0" | "hb2" | "nm0" => 1,
        "he0" => 2,
        "wm0" | "hb0" | "hb1" => 3,
        "dc0" | "po0" | "tr0" | "tt0" | "tt2" | "tt3" | "tt4" | "tt5" => 4,
        "ca0" => 5,
        "mu0" => 6,
        "nm1" | "eh0" | "dc1" => 7,
        "al0" => 8,
        "eh1" | "tt1" => 9,
        _ => 0,
    }
}

/// FUN_14015aab0's first pen move (x = 0xffXX) for list decks of kinds 5
/// and 10 (FUN_14015df90): with the world command menus (save config bit
/// 6 clear, FUN_1402e5380) by the layout's style, else 0xe3 ([I] the
/// layout +0x274 other than 0 and 7; 0xe7 otherwise).
pub fn world_pen(style: Option<u8>) -> u8 {
    match style {
        None | Some(9 | 0xb) => 0xe3,
        Some(1) => 0xee,
        Some(3) => 0xe8,
        Some(4) => 0xeb,
        Some(5) => 0xea,
        Some(6) => 0xec,
        Some(7) => 0xed,
        Some(8) => 0xe6,
        Some(10) => 0xe5,
        Some(_) => 0xe7,
    }
}

/// FUN_14015aba0: a list row's label with its button glyph in front: the
/// pen to (x, 28) (`pen`, [`list_pen`]), `09 glyph`, the pen to (2, 28),
/// then the message.
pub fn glyph_label(pen: u8, glyph: u8, label: &[u8]) -> Vec<u8> {
    let mut out = vec![0x11, pen, 0xff, 0x1c, 0x00, 0x09, glyph];
    out.extend([0x11, 0x02, 0x00, 0x1c, 0x00]);
    out.extend_from_slice(label);
    out
}

/// The list rows' text colours by colour index (0x140717d50).
const LIST_TEXT: [u32; 9] = [
    0x80e6e6e6, 0x80646464, 0x8000a7ff, 0x80646464, 0x80646464, 0x80646464, 0x80646464, 0x80e6e6e6,
    0x80e6e6e6,
];
const REACTION_Y: f32 = 377.;
/// Row/cursor text colours by colour index (0x140717d78, 0x140717da0).
const ROW_TEXT: [u32; 9] = [
    0x80b4b4b4, 0x80646464, 0x800079b7, 0x80646464, 0x80646464, 0x80646464, 0x80646464, 0x80e6e6e6,
    0x80b4b4b4,
];
const CURSOR_TEXT: [u32; 9] = [
    0x80e6e6e6, 0x80969696, 0x8000a7ff, 0x80969696, 0x80969696, 0x80969696, 0x80969696, 0x80e6e6e6,
    0x80e6e6e6,
];

/// One top-menu row: encoded label, command icon type (1 Attack, 2 Magic,
/// 3 Item, 4 Form...) and colour index (>= 4 cannot be selected).
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub label: Vec<u8>,
    pub icon: u8,
    pub colour: usize,
}

/// System font: HD glyph sheet plus the original width table.
pub struct Font<'a> {
    pub widths: &'a [u8],
    /// Pad kind for the 09 da..f9 button prompts (FUN_1403af840): 1 Xbox,
    /// 2 PlayStation, 3 generic; 0 = the keyboard / mouse was used last (key
    /// caps, FUN_1403af130).
    pub pad: u8,
}

struct Slot {
    player: Player,
    queued: Option<usize>,
    base: [f32; 2],
    parent: Option<usize>,
    text: Option<Vec<u8>>,
    text_colour: u32,
    disabled: bool,
    /// The colour multiplier (the reaction slot's by mode, 1 elsewhere).
    rgb: f32,
    context: [f32; 2],
    resolved: [f32; 2],
    /// The record's flag bit 0 when the slot was last filled (+0x140).
    flag: bool,
}

/// The reaction command the manager's slot shows (mgr+0xb40: the command,
/// its mode and flags) with its message.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reaction {
    pub command: u16,
    pub label: Vec<u8>,
    /// The candidate's display mode (0x1405b1c50, 0x140717da0).
    pub mode: u8,
    /// The record's flag bit 3 (group 3).
    pub flag3: bool,
    /// The candidate's flag bit 1: the art restarts.
    pub restart: bool,
}

/// COMMAND_DRAW's reaction fields: +0x188 the command drawn last, +0x18c its
/// group, +0x194 the slot was shown, +0x196 the burst requested
/// (FUN_14015d660), +0x198 the taken command's text.
#[derive(Clone, Debug, Default, PartialEq)]
struct ReactionDraw {
    command: u16,
    group: Option<usize>,
    visible: bool,
    burst: bool,
    text: Option<Vec<u8>>,
}

/// A ReactionDecideSprite (vtable 0x1405b1bc8): the burst at the slot.
struct Burst {
    player: Player,
    base: [f32; 2],
}

/// An opened submenu (level 1): its rows and cursor with the kind's art.
pub struct Sub {
    pub kind: usize,
    pub items: Vec<Item>,
    pub cursor: usize,
    slots: Vec<Slot>,
    counter: usize,
    last_cursor: Option<usize>,
    last_count: usize,
}

pub struct CommandMenu {
    root: Player,
    slots: Vec<Slot>,
    counter: usize,
    /// The top deck's rows (menu 0).
    pub items: Vec<Item>,
    pub reaction: Option<Reaction>,
    /// The pen byte in front of the reaction's prompt (FUN_140153850, see
    /// [`reaction_pen`]) and the prompt's glyph.
    pub reaction_pen: u8,
    pub reaction_glyph: u8,
    reaction_draw: ReactionDraw,
    bursts: Vec<Burst>,
    pub cursor: usize,
    last_cursor: Option<usize>,
    last_count: usize,
    /// The submenu kind each top row opens on decide (None executes).
    pub opens: Vec<Option<usize>>,
    /// The second deck's rows (menu 6) and the submenu kinds they open.
    pub second: Vec<Item>,
    pub second_opens: Vec<Option<usize>>,
    /// The deck shown: `FIRST_DECK` or `SECOND_DECK` (mgr+0xbd4).
    pub kind: usize,
    /// The deck drawn last frame (COMMAND_DRAW+0x184).
    last_kind: usize,
    /// The battle style picking the art set (DAT_142a11404: 0 normal,
    /// 1 battle, 2 forced) and the style drawn last frame (+0x190).
    pub style: usize,
    last_style: usize,
    /// FUN_1403b2780: the deck swap is refused and the pointer hidden (a
    /// scripted deck or a restricted mode).
    pub locked: bool,
    /// Top-level rows a command override replaced this frame (FUN_1403b0f20:
    /// override record 0 takes Attack's row, record 2 Limit's): they show
    /// the override art.
    pub overridden: [bool; 8],
    /// The open submenu, if any.
    pub sub: Option<Sub>,
    /// A list-mode deck's row selectors (FUN_1403b1780: deck mode 0xE,
    /// Trinity Limit's deck): FUN_14015df90 draws a header and the rows
    /// with no cursor, and FUN_1403b1590 takes the row whose selector's
    /// face button is pressed.
    pub list: Option<Vec<u8>>,
    /// The list deck's sheet mode (PLAYER_COMMAND +0xab0): 3 for a party
    /// limit's `comd`, 5 for Chicken Little's; [`WORLD_MAP_DECK`] for the
    /// world map cursor's deck.
    pub list_mode: u8,
    /// The world command layout's style ([`layout_style`]); `None` with the
    /// classic menu (save config bit 6).
    pub layout_style: Option<u8>,
    last_list: bool,
}

/// What the menu did with this frame's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    None,
    /// Execute the command at this row of the shown deck.
    Execute(usize),
    /// The row cannot be selected (error sound in the original).
    Invalid,
    /// The row opens its submenu (the caller supplies it with `open_sub`).
    Open(usize),
    /// Execute row `index` of the open submenu (opened by top row `row`);
    /// the menu closes and the top cursor returns to 0.
    ExecuteSub {
        row: usize,
        index: usize,
    },
    /// The submenu was closed with back.
    Closed,
    /// The other deck is shown now (FUN_1403f2470, sound 0x14).
    Toggled,
}

/// The command-menu buttons pressed this frame, as logical pad bits
/// (FUN_1403b11e0, FUN_1403f2470): decide 0x200000004, back 8, up 0x4000,
/// down 0x8000, left 0x10000 (deck swap / close), right 0x10000000 (open).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buttons {
    pub up: bool,
    pub down: bool,
    pub decide: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    /// The face buttons pressed, logical bits 24..27 (bit k = bit 24 + k:
    /// Circle, Triangle, Square, Cross in MODE_FIELD).
    pub face: u8,
}

impl CommandMenu {
    /// Field-reload start: the root skips its intro (DAT_140abc510 = 1) and
    /// shows its loop group with the slots at full alpha.
    pub fn new(seq: &Sequence, items: Vec<Item>) -> Self {
        CommandMenu {
            root: Player::new(seq, ROOT_LOOP),
            slots: Vec::new(),
            counter: 0,
            items,
            reaction: None,
            reaction_pen: 0xea,
            reaction_glyph: TRIANGLE_GLYPH,
            reaction_draw: ReactionDraw::default(),
            bursts: Vec::new(),
            cursor: 0,
            last_cursor: None,
            last_count: 0,
            opens: Vec::new(),
            second: Vec::new(),
            second_opens: Vec::new(),
            kind: FIRST_DECK,
            last_kind: FIRST_DECK,
            style: 0,
            last_style: 0,
            locked: false,
            overridden: [false; 8],
            sub: None,
            list: None,
            list_mode: 3,
            layout_style: None,
            last_list: false,
        }
    }

    /// The same menu on a newly loaded layout (a world's own command file):
    /// the field reload keeps the deck, rows and cursor and rebuilds every
    /// slot without the intro (FUN_14015db50, FUN_14015cbe0).
    pub fn reload(&mut self, seq: &Sequence) {
        self.root = Player::new(seq, ROOT_LOOP);
        self.slots.clear();
        self.last_cursor = None;
        self.last_count = 0;
        self.last_kind = self.kind;
        if let Some(sub) = self.sub.as_mut() {
            sub.slots.clear();
            sub.last_cursor = None;
            sub.last_count = 0;
        }
    }

    /// The shown deck's rows.
    pub fn rows(&self) -> &[Item] {
        if self.kind == SECOND_DECK {
            &self.second
        } else {
            &self.items
        }
    }

    /// The submenu kind row `row` of the shown deck opens.
    pub fn opens_of(&self, row: usize) -> Option<usize> {
        let opens = if self.kind == SECOND_DECK {
            &self.second_opens
        } else {
            &self.opens
        };
        opens.get(row).copied().flatten()
    }

    /// [Rewrite choice] A room with no second deck (a duel turns it off):
    /// the swap is refused and its pointer hidden as FUN_1403b2780's lock
    /// does, and a menu on the second deck goes back to the first. Without
    /// this a swap showed an empty deck, with no rows and no Attack.
    pub fn hold_first_deck(&mut self) {
        self.locked = true;
        if self.kind == SECOND_DECK {
            self.kind = FIRST_DECK;
            self.sub = None;
            if self.cursor >= self.items.len() {
                self.cursor = 0;
            }
        }
    }

    /// Opens the submenu of `kind` on top of the top level.
    pub fn open_sub(&mut self, kind: usize, items: Vec<Item>) {
        self.sub = Some(Sub {
            kind,
            items,
            cursor: 0,
            slots: Vec::new(),
            counter: 0,
            last_cursor: None,
            last_count: 0,
        });
    }

    pub fn close_sub(&mut self) {
        self.sub = None;
    }

    /// Up/down wrap; decide executes or reports an unusable row. After an
    /// execution the cursor returns to the top (FUN_1403b1b80(menu, 0)).
    pub fn input(&mut self, up: bool, down: bool, decide: bool) -> Decision {
        self.input_with_back(up, down, decide, false)
    }

    /// `input` with the back button: it closes an open submenu.
    pub fn input_with_back(&mut self, up: bool, down: bool, decide: bool, back: bool) -> Decision {
        self.press(Buttons {
            up,
            down,
            decide,
            back,
            ..Buttons::default()
        })
    }

    /// One frame of menu input. FUN_1403b11e0 takes decide, then back, then
    /// up, down and right (open the row's submenu); FUN_1403f2470 then
    /// takes left: with only the top level up it swaps the decks, keeping
    /// the cursor row when the other deck has one (FUN_1403b1b80), else it
    /// closes the submenu. A left pressed in the same frame as decide or
    /// right is not taken.
    pub fn press(&mut self, b: Buttons) -> Decision {
        // FUN_1403b1590: a list-mode deck takes the first row whose
        // selector's mask (DAT_1405c4ed8: selector s = logical bit 23 + s)
        // is pressed; FUN_1403b21d0 then runs it when its colour is under 4.
        if let Some(sel) = self.list.as_ref() {
            let rows = self.rows();
            for (i, &s) in sel.iter().enumerate().take(rows.len()) {
                if (1..=4).contains(&s) && b.face & 1 << (s - 1) != 0 {
                    return if rows[i].colour < 4 {
                        Decision::Execute(i)
                    } else {
                        Decision::Invalid
                    };
                }
            }
            return Decision::None;
        }
        if let Some(sub) = self.sub.as_mut() {
            let n = sub.items.len();
            if b.back || b.left || n == 0 {
                self.sub = None;
                return Decision::Closed;
            }
            if b.decide {
                return if sub.items[sub.cursor].colour < 4 {
                    let index = sub.cursor;
                    let row = self.cursor;
                    self.sub = None;
                    self.cursor = 0;
                    Decision::ExecuteSub { row, index }
                } else {
                    Decision::Invalid
                };
            }
            if b.up {
                sub.cursor = if sub.cursor > 0 {
                    sub.cursor - 1
                } else {
                    n - 1
                };
            } else if b.down {
                sub.cursor = if sub.cursor < n - 1 {
                    sub.cursor + 1
                } else {
                    0
                };
            }
            return Decision::None;
        }
        let n = self.rows().len();
        if n == 0 {
            return Decision::None;
        }
        self.cursor = self.cursor.min(n - 1);
        if b.decide {
            return if self.rows()[self.cursor].colour < 4 {
                let row = self.cursor;
                if self.opens_of(row).is_some() {
                    return Decision::Open(row);
                }
                self.cursor = 0;
                Decision::Execute(row)
            } else {
                Decision::Invalid
            };
        }
        if b.back {
            return Decision::None;
        }
        if b.up {
            self.cursor = if self.cursor > 0 {
                self.cursor - 1
            } else {
                n - 1
            };
        } else if b.down {
            self.cursor = if self.cursor < n - 1 {
                self.cursor + 1
            } else {
                0
            };
        } else if b.right {
            let row = self.cursor;
            if self.rows()[row].colour < 4 && self.opens_of(row).is_some() {
                return Decision::Open(row);
            }
        }
        if b.left && !self.locked {
            self.kind = if self.kind == FIRST_DECK {
                SECOND_DECK
            } else {
                FIRST_DECK
            };
            if self.cursor >= self.rows().len() {
                self.cursor = 0;
            }
            return Decision::Toggled;
        }
        Decision::None
    }

    #[allow(clippy::too_many_arguments)]
    fn slot(
        &mut self,
        seq: &Sequence,
        group: usize,
        force: bool,
        base: [f32; 2],
        text: Option<Vec<u8>>,
        colour: u32,
    ) -> usize {
        let i = self.counter;
        if i == self.slots.len() {
            self.slots.push(Slot {
                player: Player::new(seq, group),
                queued: None,
                base,
                parent: None,
                text: None,
                text_colour: colour,
                disabled: false,
                rgb: 1.,
                context: [0.; 2],
                resolved: [0.; 2],
                flag: false,
            });
        } else if force {
            self.slots[i].player = Player::new(seq, group);
            self.slots[i].queued = None;
        }
        let s = &mut self.slots[i];
        if text.is_some() {
            s.text = text;
        }
        s.base = base;
        s.parent = None;
        s.disabled = false;
        s.rgb = 1.;
        s.text_colour = colour;
        self.counter += 1;
        i
    }

    /// FUN_14015d660: the command was taken; the next draw starts the burst.
    pub fn burst(&mut self) {
        self.reaction_draw.burst = true;
    }

    /// FUN_14015d670: the reaction slot at (0x24, 0x179 - 22n). A shown
    /// command uses group 2 (5 for the partner's mode 7, 3 for a record with
    /// flag bit 3), its prompt and message, the cursor text colour and the
    /// RGB multiplier of its mode; a new command or group restarts the art.
    /// With none, group 0 fades the slot out once (group 1 keeps the taken
    /// command's text) and hides it at the end. A requested burst starts a
    /// ReactionDecideSprite on group 4 at the slot.
    fn draw_reaction(&mut self, seq: &Sequence, n: usize) {
        let base = [REACTION_X, REACTION_Y - PITCH * n as f32];
        let shown = self.reaction.clone().filter(|r| r.command != 0);
        let command = shown.as_ref().map_or(0, |r| r.command);
        let mut force = true;
        if self.reaction_draw.command == command {
            force = shown.as_ref().is_some_and(|r| r.restart);
        }
        let group;
        match shown {
            None => {
                group = if self.reaction_draw.text.is_some() {
                    REACTION_EMPTY_TEXT
                } else {
                    REACTION_EMPTY
                };
                let text = self.reaction_draw.text.clone();
                let i = self.slot(seq, group, force, base, None, 0x80e6e6e6);
                self.slots[i].text = text;
                // FUN_1401672f0: the art hides itself when it ends.
                let ended = self.slots[i].player.finished();
                self.slots[i].disabled = if force || self.reaction_draw.visible && !ended {
                    false
                } else {
                    self.reaction_draw.visible = false;
                    true
                };
            }
            Some(r) => {
                self.reaction_draw.visible = true;
                group = if r.mode == 7 {
                    REACTION_PARTNER
                } else if r.flag3 {
                    REACTION_FLAG3
                } else {
                    REACTION_SHOWN
                };
                if Some(group) != self.reaction_draw.group {
                    force = true;
                }
                let text = glyph_label(self.reaction_pen, self.reaction_glyph, &r.label);
                let mode = usize::from(r.mode);
                let colour = CURSOR_TEXT[mode.min(CURSOR_TEXT.len() - 1)];
                let i = self.slot(seq, group, force, base, None, colour);
                self.slots[i].text = Some(text.clone());
                self.slots[i].rgb = REACTION_RGB[mode.min(REACTION_RGB.len() - 1)];
                self.reaction_draw.text = self.reaction_draw.burst.then_some(text);
            }
        }
        self.reaction_draw.group = Some(group);
        self.reaction_draw.command = command;
        if std::mem::take(&mut self.reaction_draw.burst) {
            self.bursts.push(Burst {
                player: Player::new(seq, REACTION_BURST),
                base,
            });
        }
    }

    /// The bursts play once and delete themselves at their end.
    fn step_bursts(&mut self, seq: &Sequence) {
        self.bursts.retain_mut(|b| {
            b.player.update(seq, 1.);
            !b.player.finished()
        });
    }

    /// The submenu level's build (FUN_14015b6d0 with L = 1): the kind's
    /// panel, its rows bottom-anchored like the top level, and the cursor
    /// row moved 8 right. A style change restarts the panel and cursor.
    fn update_sub(sub: &mut Sub, seq: &Sequence, changed: bool) {
        let Some([panel, cursor_group, row_group, last_group]) = sub_groups(sub.kind) else {
            return;
        };
        let n = sub.items.len();
        let cur = sub.cursor.min(n.saturating_sub(1));
        if sub.last_count != n {
            sub.slots.clear();
        }
        sub.counter = 0;
        let x = ROOT_BASE[0] + LEVEL_STEP;
        let place = |sub: &mut Sub,
                     group: usize,
                     force: bool,
                     base: [f32; 2],
                     text: Option<Vec<u8>>,
                     colour: u32| {
            let i = sub.counter;
            if i == sub.slots.len() {
                sub.slots.push(Slot {
                    player: Player::new(seq, group),
                    queued: None,
                    base,
                    parent: None,
                    text: None,
                    text_colour: colour,
                    disabled: false,
                    rgb: 1.,
                    context: [0.; 2],
                    resolved: [0.; 2],
                    flag: false,
                });
            } else if force {
                sub.slots[i].player = Player::new(seq, group);
                sub.slots[i].queued = None;
            }
            let s = &mut sub.slots[i];
            if text.is_some() {
                s.text = text;
            }
            s.base = base;
            s.text_colour = colour;
            sub.counter += 1;
        };
        place(
            sub,
            panel,
            changed,
            [x, PANEL_ANCHOR - PITCH * n as f32],
            None,
            0x80b4b4b4,
        );
        let mut y = ROW_ANCHOR - PITCH * n as f32;
        let mut cursor_y = y;
        for i in 0..n {
            if i != cur {
                let group = if i == n - 1 { last_group } else { row_group };
                let item = &sub.items[i];
                let (label, colour) = (item.label.clone(), ROW_TEXT[item.colour.min(8)]);
                place(sub, group, true, [x, y], Some(label), colour);
            } else {
                cursor_y = y;
            }
            y += PITCH;
        }
        if n > 0 {
            let force = sub.last_cursor != Some(cur) || sub.last_count != n || changed;
            let item = &sub.items[cur];
            let (label, colour) = (item.label.clone(), CURSOR_TEXT[item.colour.min(8)]);
            place(
                sub,
                cursor_group,
                force,
                [x + CURSOR_SHIFT, cursor_y],
                Some(label),
                colour,
            );
        }
        sub.last_cursor = Some(cur);
        sub.last_count = n;
        sub.slots.truncate(sub.counter);
        for s in &mut sub.slots {
            s.player.update(seq, 1.);
        }
    }

    /// One frame: root update, the level-0 build (FUN_14015b6d0) for the
    /// shown deck, then the slots' players in creation order. A deck swap
    /// plays the panel's swap group (T_280) before its loop and restarts
    /// the cursor; a battle-style change replays the panel intro and
    /// flashes the cursor (T_2B0).
    pub fn update(&mut self, seq: &Sequence) {
        self.root.update(seq, 1.);
        self.counter = 0;
        if self.list.is_some() != self.last_list {
            self.slots.clear();
            self.last_cursor = None;
            self.last_count = 0;
            self.last_list = self.list.is_some();
        }
        if let Some(sel) = self.list.clone() {
            self.update_list(seq, &sel);
            return;
        }
        let style = self.style.min(2);
        let changed = style != self.last_style;
        let kind = self.kind;
        let swap = (kind == FIRST_DECK && self.last_kind == SECOND_DECK)
            || (kind == SECOND_DECK && self.last_kind == FIRST_DECK);
        let rows: Vec<Item> = self.rows().to_vec();
        let n = rows.len();
        let cur = self.cursor.min(n.saturating_sub(1));
        if self.last_count != n {
            self.slots.clear();
        }
        let x = ROOT_BASE[0];
        let (group, force) = if swap {
            (PANEL_SWAP[style], true)
        } else {
            (PANEL_INTRO[style], changed)
        };
        let panel = self.slot(
            seq,
            group,
            force,
            [x, PANEL_ANCHOR - PITCH * n as f32],
            None,
            0x80b4b4b4,
        );
        self.slots[panel].queued = Some(PANEL_LOOP[style]);
        let mut y = ROW_ANCHOR - PITCH * n as f32;
        let mut cursor_y = y;
        for (i, item) in rows.iter().enumerate() {
            if i != cur {
                let last = i == n - 1;
                let mut group = if last { LAST_ROW[style] } else { ROW[style] };
                let mut queued = None;
                let overridden = self.overridden.get(i).copied().unwrap_or(false);
                if overridden {
                    let k = if last { 9 } else { 6 } + style;
                    group = OVERRIDE[k];
                    if !self.slots.get(self.counter).is_some_and(|s| s.flag) {
                        group = OVERRIDE_IN[k];
                        queued = Some(OVERRIDE[k]);
                    }
                }
                let (label, colour) = (item.label.clone(), ROW_TEXT[item.colour.min(8)]);
                let s = self.slot(seq, group, true, [x, y], Some(label), colour);
                if queued.is_some() {
                    self.slots[s].queued = queued;
                }
                self.slots[s].flag = overridden;
            } else {
                cursor_y = y;
            }
            y += PITCH;
        }
        if n > 0 {
            let mut force = self.last_cursor != Some(cur) || self.last_count != n || swap;
            let mut group = CURSOR[style];
            if changed {
                force = true;
                group = CURSOR_REFRESH[style];
            }
            // FUN_14015ba00: an overridden cursor row plays the override art
            // in, then loops it; a row that stops being overridden restarts.
            let overridden = self.overridden.get(cur).copied().unwrap_or(false);
            let was = self.slots.get(self.counter).is_some_and(|s| s.flag);
            let mut cursor_queued = None;
            if overridden {
                group = OVERRIDE[3 + style];
                if !was {
                    group = OVERRIDE_IN[3 + style];
                    force = true;
                    cursor_queued = Some(OVERRIDE[3 + style]);
                }
            } else if was {
                force = true;
            }
            let item = &rows[cur];
            let (label, colour, icon) = (
                item.label.clone(),
                CURSOR_TEXT[item.colour.min(8)],
                item.icon,
            );
            let c = self.slot(
                seq,
                group,
                force,
                [x + CURSOR_SHIFT, cursor_y],
                Some(label),
                colour,
            );
            if cursor_queued.is_some() {
                self.slots[c].queued = cursor_queued;
            }
            self.slots[c].flag = overridden;
            // Decoration (FUN_14015bc30): icon frame, category icon and the
            // bouncing pointer, all parented to the cursor row. The pointer
            // shows only while no submenu was drawn and the menu is free.
            let icon_group = match icon {
                1 => Some(37),
                2 => Some(41),
                3 => Some(39),
                4 => Some(44),
                5 => Some(43),
                6 => Some(38),
                7 => Some(40),
                _ => None,
            };
            let frame = self.slot(seq, ICON_FRAME[style], force, [0., 0.], None, colour);
            let sub_drawn = self.sub.as_ref().is_some_and(|s| s.last_cursor.is_some());
            let pointer_group = if kind != FIRST_DECK {
                POINTER_OTHER
            } else {
                POINTER
            };
            match icon_group {
                Some(g) => {
                    self.slots[frame].parent = Some(c);
                    let ic = self.slot(seq, g, force, [0., 0.], None, colour);
                    self.slots[ic].parent = Some(c);
                    let pointer = self.slot(seq, pointer_group, force, [0., 0.], None, colour);
                    self.slots[pointer].parent = Some(c);
                    self.slots[pointer].disabled = sub_drawn || self.locked;
                }
                None => {
                    self.slots[frame].disabled = true;
                    let placeholder = self.slot(seq, 2, force, [0., 0.], None, 0x80e6e6e6);
                    self.slots[placeholder].disabled = true;
                    let pointer = self.slot(seq, pointer_group, force, [0., 0.], None, colour);
                    self.slots[pointer].parent = Some(c);
                    self.slots[pointer].disabled = self.locked;
                }
            }
        }
        self.draw_reaction(seq, n);
        self.last_cursor = Some(cur);
        self.last_count = n;
        if let Some(sub) = self.sub.as_mut() {
            Self::update_sub(sub, seq, changed);
        }
        self.last_kind = kind;
        self.last_style = style;
        self.slots.truncate(self.counter);
        for s in &mut self.slots {
            s.player.update(seq, 1.);
            if seq.groups[s.player.group].play_count != 0
                && s.player.finished()
                && let Some(next) = s.queued.take()
            {
                s.player = Player::new(seq, next);
                s.player.update(seq, 1.);
            }
        }
        self.step_bursts(seq);
    }

    /// The list path (FUN_14015df90 in deck mode 0xE): the header at
    /// (0x1c, 0x184 - 22n), each row at (0x1c, 0x19a - 22(n - i)) with its
    /// selector's group (0x140718330 + 0x14) and colour (0x140717d50), then
    /// the reaction slot (FUN_14015d670). A row with a selector carries its
    /// button glyph in front of its label ([`glyph_label`]).
    fn update_list(&mut self, seq: &Sequence, sel: &[u8]) {
        let rows: Vec<Item> = self.rows().to_vec();
        let n = rows.len();
        if self.last_count != n {
            self.slots.clear();
        }
        let x = ROOT_BASE[0];
        self.slot(
            seq,
            LIST_HEADER,
            false,
            [x, PANEL_ANCHOR - PITCH * n as f32],
            None,
            0x80b4b4b4,
        );
        let mut y = ROW_ANCHOR - PITCH * n as f32;
        for (i, item) in rows.iter().enumerate() {
            let s = sel.get(i).copied().unwrap_or(0) as usize;
            let rows = if self.list_mode == WORLD_MAP_DECK {
                &WORLD_LIST_ROW
            } else {
                &LIST_ROW
            };
            let group = rows.get(s).copied().unwrap_or(0);
            let colour = LIST_TEXT[item.colour.min(8)];
            let label = match LIST_GLYPH.get(s).copied().flatten() {
                // Chicken Little's deck (kind 5) and the world map's (kind
                // 10) take FUN_14015aab0's world pen, a limit's `comd`
                // (kind 0xE) FUN_14015aba0's.
                Some(g) if self.list_mode == 5 || self.list_mode == WORLD_MAP_DECK => {
                    glyph_label(world_pen(self.layout_style), g, &item.label)
                }
                Some(g) => glyph_label(list_pen(self.list_mode), g, &item.label),
                None => item.label.clone(),
            };
            self.slot(seq, group, false, [x, y], Some(label), colour);
            y += PITCH;
        }
        self.draw_reaction(seq, n);
        self.last_count = n;
        self.slots.truncate(self.counter);
        for s in &mut self.slots {
            s.player.update(seq, 1.);
        }
        self.step_bursts(seq);
    }

    /// Draws the root, then each slot: sprite, then its label in the system
    /// font. `sheet`, `font_sheet` and `icon_sheet` (the font's 0x09 icon
    /// plane) index the canvas textures.
    pub fn draw(
        &mut self,
        seq: &Sequence,
        font: &Font,
        sheet: usize,
        font_sheet: usize,
        icon_sheet: usize,
    ) -> Vec<Draw> {
        let m = Modifiers::default();
        let mut out = vec![Draw::Quads {
            quads: seq.quads(&self.root, [X_ORIGIN + ROOT_BASE[0], ROOT_BASE[1]], &m),
            z: 0,
            sheet,
        }];
        let sheets = [sheet, font_sheet, icon_sheet];
        draw_slots(&mut self.slots, seq, font, sheets, &mut out);
        if let Some(sub) = self.sub.as_mut() {
            draw_slots(&mut sub.slots, seq, font, sheets, &mut out);
        }
        // The ReactionDecideSprites come after the slots (created later at
        // the same priority, 0x14b7).
        for b in &self.bursts {
            out.push(Draw::Quads {
                quads: seq.quads(&b.player, [X_ORIGIN + b.base[0], b.base[1]], &m),
                z: 0,
                sheet,
            });
        }
        out
    }
}

/// FUN_140153850: the pen byte in front of the reaction prompt, by the
/// command layout's file name (`deck` is the layout's sheet mode,
/// DAT_14079cf00 +0x274; a list-mode deck, menu kind 0xe, always gives
/// 0xea).
pub fn reaction_pen(file: &str, deck: u8, list: bool) -> u8 {
    if list {
        return 0xea;
    }
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let world = name.strip_suffix("command.2dd").unwrap_or("");
    match world {
        "bb0" | "he1" | "dc0" | "mu0" => 0xec,
        "lm0" => 0xed,
        "lk0" | "tt0" | "ca0" => 0xeb,
        "nm1" => 0xea,
        "eh0" | "dc1" => 0xe6,
        "al0" if deck == 0 || deck == 7 => 0xe3,
        "al0" => 0xe6,
        _ => 0xea,
    }
}

/// Draws one level's slots in creation order: sprite, then label (glyphs,
/// then icons). `sheets`: the menu's, the font's and its icon plane's.
fn draw_slots(
    slots: &mut [Slot],
    seq: &Sequence,
    font: &Font,
    [sheet, font_sheet, icon_sheet]: [usize; 3],
    out: &mut Vec<Draw>,
) {
    let m = Modifiers::default();
    for i in 0..slots.len() {
        let base = match slots[i].parent {
            Some(p) => {
                let p = &slots[p];
                [p.context[0] + p.resolved[0], p.context[1] + p.resolved[1]]
            }
            None => slots[i].base,
        };
        let s = &mut slots[i];
        s.resolved = base;
        s.context = seq.child_context(&s.player, &m).unwrap_or([0.; 2]);
        if s.disabled {
            continue;
        }
        let mut m = m;
        m.color = [s.rgb, s.rgb, s.rgb, 1.];
        out.push(Draw::Quads {
            quads: seq.quads(&s.player, [X_ORIGIN + base[0], base[1]], &m),
            z: 0,
            sheet,
        });
        if let Some(text) = &s.text {
            // Text RGB from the colour table; alpha from the row's current
            // animation colour (flags 4 and 8 set).
            let alpha = seq.current_color(&s.player).map_or(1., |c| c[3]);
            let mut rgba = color(s.text_colour);
            rgba[3] = alpha;
            let origin = [
                X_ORIGIN + base[0] + s.context[0] + TEXT_OFFSET[0],
                base[1] + s.context[1] + TEXT_OFFSET[1],
            ];
            let (glyphs, icons) = label_quads(text, origin, rgba, font, TEXT_SIZE);
            out.push(Draw::Quads {
                quads: glyphs,
                z: 0,
                sheet: font_sheet,
            });
            if !icons.is_empty() {
                out.push(Draw::Quads {
                    quads: icons,
                    z: 0,
                    sheet: icon_sheet,
                });
            }
        }
    }
}

/// Glyph quads for an international system-font string at size 14: glyph
/// index = byte - 0x20, cell 18x24 scaled by 14/18, advance =
/// (int)(width * scale + 0.5); 0x01 advances a third of a cell
/// (FUN_1403e2be0).
pub fn text_quads(text: &[u8], origin: [f32; 2], rgba: [f32; 4], font: &Font) -> Vec<Quad> {
    text_quads_sized(text, origin, rgba, font, TEXT_SIZE)
}

/// Width of a system-font string at `size` (the sum of its advances).
pub fn text_width(text: &[u8], font: &Font, size: f32) -> f32 {
    let scale = size / FONT_REFERENCE;
    text.iter()
        .map(|&byte| {
            if byte == 0x01 {
                ((CELL[0] / 3.) as f64 * scale as f64 + 0.5).floor() as f32
            } else if byte < 0x20 {
                0.
            } else {
                font.widths
                    .get((byte - 0x20) as usize)
                    .map_or(0., |&w| (w as f64 * scale as f64 + 0.5).floor() as f32)
            }
        })
        .sum()
}

/// `text_quads` at any size (cell 18x24 scaled by size / 18).
pub fn text_quads_sized(
    text: &[u8],
    origin: [f32; 2],
    rgba: [f32; 4],
    font: &Font,
    size: f32,
) -> Vec<Quad> {
    label_quads(text, origin, rgba, font, size).0
}

/// Glyph and icon quads of a system-font string. Control codes take their
/// parameters (FUN_1403b0280's lengths); 0x11 moves the pen to x from the
/// origin (y: the line's own baseline [I]); 0x09 draws an icon as the text
/// routines do (FUN_1403af9e0, [`crate::sprite_player::text_quads`]): a
/// square of trunc(28k) - 4 px, 2 px right of the pen, its bottom at the
/// baseline + 4k - 1, at the text alpha.
pub fn label_quads(
    text: &[u8],
    origin: [f32; 2],
    rgba: [f32; 4],
    font: &Font,
    size: f32,
) -> (Vec<Quad>, Vec<Quad>) {
    use crate::sprite_player::{icon_width, keyboard_prompt, prompt_icon};
    let scale = size / FONT_REFERENCE;
    let (w, h) = (CELL[0] * scale, CELL[1] * scale);
    let qh = ((CELL[1] * 16. * scale) as i32) as f32 / 16.;
    let baseline16 = (origin[1] - 2. + qh) * 16.;
    let mut x = origin[0];
    let mut glyphs = Vec::new();
    let mut icons = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let byte = text[i];
        if byte == 0x01 {
            x += ((CELL[0] / 3.) as f64 * scale as f64 + 0.5).floor() as f32;
            i += 1;
            continue;
        }
        if byte < 0x20 {
            let n = crate::dialogue::code_length(byte).max(1);
            let p = text.get(i + 1..i + n).unwrap_or(&[]);
            match (byte, p) {
                (0x11, &[x0, x1, ..]) => {
                    x = origin[0] + f32::from(i16::from_le_bytes([x0, x1]));
                }
                (0x09, &[raw]) => {
                    let cells: Vec<u8> = match keyboard_prompt(raw) {
                        Some(cells) if font.pad == 0 => cells.to_vec(),
                        _ => vec![prompt_icon(raw, font.pad.max(1))],
                    };
                    for icon in cells {
                        let side = ((28. * scale) as i32 - 4) as f32;
                        let bottom = ((scale * 64. + baseline16) as i32 - 16) as f32 / 16.;
                        let left = x + 2.;
                        let (u, v) = ((icon as i32 % 21) * 24, (icon as i32 / 21) * 24);
                        icons.push(Quad {
                            corners: [
                                [left, bottom - side],
                                [left + side, bottom - side],
                                [left, bottom],
                                [left + side, bottom],
                            ],
                            uv: [u, v, u + 24, v + 24],
                            colors: [[1., 1., 1., rgba[3]]; 4],
                            blend: 0,
                        });
                        x += ((icon_width(icon) as f32 * 16.) * scale) as i32 as f32 / 16.;
                    }
                }
                _ => {}
            }
            i += n;
            continue;
        }
        i += 1;
        let index = (byte - 0x20) as usize;
        let Some(&width) = font.widths.get(index) else {
            continue;
        };
        let (u, v) = (
            (index % CELL_COLUMNS) as f32 * CELL[0],
            (index / CELL_COLUMNS) as f32 * CELL[1],
        );
        let y = origin[1];
        glyphs.push(Quad {
            corners: [[x, y], [x + w, y], [x, y + h], [x + w, y + h]],
            uv: [
                u as i32,
                v as i32,
                (u + CELL[0]) as i32,
                (v + CELL[1]) as i32,
            ],
            colors: [rgba; 4],
            blend: 0,
        });
        x += (width as f64 * scale as f64 + 0.5).floor() as f32;
    }
    (glyphs, icons)
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_world_layout_names_its_reaction_pen() {
        assert_eq!(layout_style("field2d/us/he1command.2dd", true), 1);
        assert_eq!(layout_style("field2d/us/tt6command.2dd", true), 0xb);
        assert_eq!(layout_style("field2d/us/tt6command.2dd", false), 3);
        assert_eq!(layout_style("field2d/us/zz0command.2dd", true), 0);
        assert_eq!(world_pen(Some(1)), 0xee);
        assert_eq!(world_pen(Some(0)), 0xe7);
        assert_eq!(world_pen(None), 0xe3);
    }
    use super::*;

    /// A menu with no sequence behind it (input only).
    fn bare(items: Vec<Item>, opens: Vec<Option<usize>>) -> CommandMenu {
        CommandMenu {
            root: Player {
                group: 0,
                time: 0.,
                state: 1,
                plays_left: 0,
            },
            slots: Vec::new(),
            counter: 0,
            items,
            reaction: None,
            reaction_pen: 0xea,
            reaction_glyph: TRIANGLE_GLYPH,
            reaction_draw: ReactionDraw::default(),
            bursts: Vec::new(),
            cursor: 0,
            last_cursor: None,
            last_count: 0,
            opens,
            second: Vec::new(),
            second_opens: Vec::new(),
            kind: FIRST_DECK,
            last_kind: FIRST_DECK,
            style: 0,
            last_style: 0,
            locked: false,
            overridden: [false; 8],
            sub: None,
            list: None,
            list_mode: 3,
            layout_style: None,
            last_list: false,
        }
    }

    fn row(icon: u8, colour: usize) -> Item {
        Item {
            label: vec![0x2e],
            icon,
            colour,
        }
    }

    #[test]
    fn the_reaction_prompt_pen_follows_the_world_layout() {
        assert_eq!(reaction_pen("field2d/us/tt0command.2dd", 0, false), 0xeb);
        assert_eq!(reaction_pen("tt6command.2dd", 0, false), 0xea);
        assert_eq!(reaction_pen("bb0command.2dd", 0, false), 0xec);
        assert_eq!(reaction_pen("lm0command.2dd", 0, false), 0xed);
        assert_eq!(reaction_pen("al0command.2dd", 0, false), 0xe3);
        assert_eq!(reaction_pen("al0command.2dd", 3, false), 0xe6);
        assert_eq!(reaction_pen("eh0command.2dd", 0, false), 0xe6);
        assert_eq!(reaction_pen("lk0command.2dd", 0, true), 0xea);
    }

    #[test]
    fn a_list_deck_takes_the_row_of_the_pressed_face_button() {
        let mut menu = bare(vec![row(0, 0), row(0, 0), row(0, 5)], vec![None; 3]);
        menu.list = Some(vec![2, 3, 4]);
        let face = |k: u8| Buttons {
            face: 1 << k,
            ..Buttons::default()
        };
        // Square (bit 26) is selector 3: the second row.
        assert_eq!(menu.press(face(2)), Decision::Execute(1));
        // Cross (bit 27) names the third row, which cannot be used.
        assert_eq!(menu.press(face(3)), Decision::Invalid);
        // Circle names no row; decide and the D-pad do nothing.
        assert_eq!(menu.press(face(0)), Decision::None);
        let decide = Buttons {
            decide: true,
            down: true,
            ..Buttons::default()
        };
        assert_eq!(menu.press(decide), Decision::None);
        assert_eq!(menu.cursor, 0);
    }

    #[test]
    fn left_swaps_the_decks_and_keeps_the_cursor_row() {
        let mut menu = bare(
            vec![row(1, 0), row(2, 0), row(3, 0), row(4, 0)],
            vec![None, Some(1), Some(2), Some(3)],
        );
        // Attack, Summon (blank: colour 5), Party, Limit.
        menu.second = vec![row(1, 0), row(0, 5), row(6, 0), row(7, 0)];
        menu.second_opens = vec![None, None, Some(8), Some(9)];
        menu.input(false, true, false);
        menu.input(false, true, false);
        let left = Buttons {
            left: true,
            ..Buttons::default()
        };
        assert_eq!(menu.press(left), Decision::Toggled);
        assert_eq!((menu.kind, menu.cursor), (SECOND_DECK, 2));
        // Party opens the party list; right opens it too.
        assert_eq!(menu.input(false, false, true), Decision::Open(2));
        let right = Buttons {
            right: true,
            ..Buttons::default()
        };
        assert_eq!(menu.press(right), Decision::Open(2));
        // A blank row neither opens nor executes.
        menu.input(true, false, false);
        assert_eq!(menu.press(right), Decision::None);
        assert_eq!(menu.input(false, false, true), Decision::Invalid);
        // With a submenu up, left closes it instead of swapping.
        menu.open_sub(8, vec![row(6, 0)]);
        assert_eq!(menu.press(left), Decision::Closed);
        assert_eq!(menu.kind, SECOND_DECK);
        // Back on the top level swaps back; a cursor past the other deck's
        // end returns to its first row (FUN_1403b1b80).
        menu.second.push(row(1, 0));
        menu.cursor = 4;
        assert_eq!(menu.press(left), Decision::Toggled);
        assert_eq!((menu.kind, menu.cursor), (FIRST_DECK, 0));
        // A locked menu does not swap.
        menu.locked = true;
        assert_eq!(menu.press(left), Decision::None);
        assert_eq!(menu.kind, FIRST_DECK);
    }

    #[test]
    fn without_a_second_deck_the_first_stays_shown() {
        let mut menu = bare(
            vec![row(1, 0), row(2, 5), row(3, 0), row(4, 5)],
            vec![None, Some(1), Some(2), None],
        );
        let left = Buttons {
            left: true,
            ..Buttons::default()
        };
        // The duel's empty second deck: a swap leaves no rows and no Attack.
        assert_eq!(menu.press(left), Decision::Toggled);
        assert!(menu.rows().is_empty());
        assert_eq!(menu.input(false, false, true), Decision::None);
        // Held on the first deck: it comes back, and left no longer swaps.
        menu.hold_first_deck();
        assert_eq!((menu.kind, menu.rows().len()), (FIRST_DECK, 4));
        assert_eq!(menu.press(left), Decision::None);
        assert_eq!(menu.kind, FIRST_DECK);
        assert_eq!(menu.input(false, false, true), Decision::Execute(0));
    }

    #[test]
    fn execution_on_the_second_deck_reports_its_row() {
        let mut menu = bare(vec![row(1, 0)], vec![None]);
        menu.second = vec![row(1, 0), row(5, 0)];
        menu.second_opens = vec![None, None];
        menu.kind = SECOND_DECK;
        menu.input(false, true, false);
        assert_eq!(menu.rows()[menu.cursor].icon, 5);
        assert_eq!(menu.input(false, false, true), Decision::Execute(1));
        assert_eq!((menu.kind, menu.cursor), (SECOND_DECK, 0));
    }

    #[test]
    fn text_advances_by_scaled_width() {
        let widths = [11u8; 100];
        let font = Font {
            widths: &widths,
            pad: 1,
        };
        let q = text_quads(&[0x2e, 0x01, 0x2e], [10., 20.], [1.; 4], &font);
        assert_eq!(q.len(), 2);
        // 'A' is glyph 14: cell (14 * 18, 0).
        assert_eq!(q[0].uv, [252, 0, 270, 24]);
        // 11 * 14/18 + 0.5 -> 9; space 6 * 14/18 + 0.5 -> 5.
        assert_eq!(q[1].corners[0][0], 10. + 9. + 5.);
    }

    #[test]
    fn list_label_puts_the_button_glyph_left_of_the_text() {
        let label = glyph_label(list_pen(3), 0xdf, &[0x2e]);
        assert_eq!(
            label,
            [
                0x11, 0xe2, 0xff, 0x1c, 0, 0x09, 0xdf, 0x11, 2, 0, 0x1c, 0, 0x2e
            ]
        );
        let widths = [11u8; 100];
        let font = Font {
            widths: &widths,
            pad: 2,
        };
        let (glyphs, icons) = label_quads(&label, [10., 20.], [1.; 4], &font, 14.);
        // The codes' parameters draw nothing; the text starts 2 right of
        // the origin.
        assert_eq!(glyphs.len(), 1);
        assert_eq!(glyphs[0].corners[0][0], 12.);
        // Triangle on a PlayStation pad (0x1405c4d40 row 3): icon 67, a
        // 17 px square 2 right of x - 30.
        assert_eq!(icons.len(), 1);
        let i = &icons[0];
        assert_eq!(
            i.uv,
            [(67 % 21) * 24, (67 / 21) * 24, (67 % 21) * 24 + 24, 72 + 24]
        );
        assert_eq!(i.corners[0][0], 10. - 30. + 2.);
        assert_eq!(i.corners[3][0] - i.corners[0][0], 17.);
    }

    #[test]
    fn cursor_wraps_and_execution_returns_to_top() {
        let items = (0..4)
            .map(|i| Item {
                label: vec![0x2e],
                icon: 1,
                colour: if i == 1 { 5 } else { 0 },
            })
            .collect();
        let mut menu = bare(items, Vec::new());
        menu.input(true, false, false);
        assert_eq!(menu.cursor, 3);
        assert_eq!(menu.input(false, false, true), Decision::Execute(3));
        assert_eq!(menu.cursor, 0);
        menu.input(false, true, false);
        assert_eq!(menu.input(false, false, true), Decision::Invalid);
    }

    #[test]
    fn a_submenu_opens_executes_and_backs_out() {
        let items = (0..3)
            .map(|_| Item {
                label: vec![0x2e],
                icon: 2,
                colour: 0,
            })
            .collect();
        let mut menu = bare(items, vec![None, Some(1), None]);
        // Row 1 opens the submenu and the cursor stays on it.
        menu.input(false, true, false);
        assert_eq!(menu.input(false, false, true), Decision::Open(1));
        assert_eq!(menu.cursor, 1);
        let sub_items = vec![
            Item {
                label: vec![0x2e],
                icon: 2,
                colour: 0,
            },
            Item {
                label: vec![0x2e],
                icon: 2,
                colour: 5,
            },
        ];
        menu.open_sub(1, sub_items);
        // Back closes it without leaving the top row.
        assert_eq!(
            menu.input_with_back(false, false, false, true),
            Decision::Closed
        );
        assert_eq!(menu.cursor, 1);
        // Reopen, move to the unusable row (error), back to the usable one,
        // decide: the sub closes and the top cursor returns to 0.
        menu.open_sub(
            1,
            vec![
                Item {
                    label: vec![0x2e],
                    icon: 2,
                    colour: 0,
                },
                Item {
                    label: vec![0x2e],
                    icon: 2,
                    colour: 5,
                },
            ],
        );
        menu.input(false, true, false);
        assert_eq!(menu.input(false, false, true), Decision::Invalid);
        menu.input(true, false, false);
        assert_eq!(
            menu.input(false, false, true),
            Decision::ExecuteSub { row: 1, index: 0 }
        );
        assert!(menu.sub.is_none());
        assert_eq!(menu.cursor, 0);
    }
}
