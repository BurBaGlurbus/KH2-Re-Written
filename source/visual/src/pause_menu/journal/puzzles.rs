//! Puzzles: the menu (state 0x1b, FUN_140342860 / FUN_14033f600), the puzzle
//! list (0x1c, FUN_140342370 / FUN_14033f310), the tutorial (0x1e / 0x1f,
//! FUN_140342c20 / FUN_14033f890), the board (0x1d, FUN_140341c40 /
//! FUN_14033e650) and the solved banner (0x20, FUN_140342050). The List of
//! Pieces (0x21 / 8) is the Treasures page's third kind (`treasure.rs`).
//! A picture is `menu/jm_puzzle/<file>.bin`, one IMGD of 256x384.
use super::*;
use crate::pause_menu::picture;
use data::journal_table::{Piece, Puzzle};
use std::collections::BTreeSet;

pub(super) const MENU: u8 = 0x1b;
pub(super) const LIST: u8 = 0x1c;
pub(super) const BOARD: u8 = 0x1d;
pub(super) const HELP_FIRST: u8 = 0x1e;
pub(super) const HELP_READ: u8 = 0x1f;
pub(super) const SOLVED: u8 = 0x20;
pub(super) const PIECES_COLLECTION: u8 = 0x21;
pub(super) const PIECES_NOTES: u8 = 8;

const PANEL: usize = 90;
const ROW_GROUP: usize = 2;
const MARK: [usize; 2] = [92, 93];
const BOARD_PANEL: usize = 112;
/// 0x1405bbd80: the board frame by shape.
const BOARD_FRAME: [usize; 2] = [113, 114];
const GRAB_HAND: usize = 115;
/// 0x1405bbd88: normal cursor, moving cursor, origin marker by shape.
const CURSOR: [[usize; 3]; 2] = [[116, 118, 120], [117, 119, 121]];
const HELP_PANEL: usize = 97;
const HELP_TITLE: usize = 98;
const HELP_TRACK: usize = 100;
const PLAIN_LINE: usize = 3;
/// The banner: box and text, backdrop and picture frame, the item's name
/// (appear, idle, close).
const BANNER: [usize; 3] = [122, 123, 124];
const BANNER_BACK: [usize; 3] = [125, 126, 127];
const BANNER_NAME: [usize; 3] = [128, 129, 130];
const OBTAINED_LINE: usize = 131;
/// 0x1405bbd74: the item picture's show and close groups.
const REWARD_PICTURE: [usize; 2] = [4, 5];
const PRIO_PANEL: u32 = 0x755c;
const PRIO_BOARD: u32 = 0x755e;
const PRIO_MARK: u32 = 0x755e;
const PRIO_PIECE: u32 = 0x755f;
const PRIO_CURSOR_A: u32 = 0x7560;
const PRIO_CURSOR_B: u32 = 0x7561;
const PRIO_HAND: u32 = 0x756a;
const PRIO_BANNER: u32 = 0x756d;
const PRIO_REWARD: u32 = 0x756e;
const PITCH: f32 = 26.;
const SIZE: f32 = 18.;
const HELP_LINES: usize = 9;
const LIST_ROWS: usize = 10;
/// se000 #10 (logical sound 0x13).
const SOUND_SOLVED: u32 = 10;
/// Mode 7's right stick (0x1405c3420).
const ROTATE_RIGHT: u64 = 0x4_0000;
const ROTATE_LEFT: u64 = 0x8_0000;

/// 0x1405bbd68: the menu's rows and where they lead.
const MENU_ROWS: [(u16, u8); 3] = [
    (0x4f78, LIST),
    (0x4f79, PIECES_COLLECTION),
    (0x50c2, HELP_READ),
];
const TAB_PUZZLE: u16 = 0x4f78;
pub(super) const TAB_PIECES: u16 = 0x4f79;
const HELP_MENU: u16 = 0x4f85;
const HELP_LIST: u16 = 0x4f48;
const HELP_BOARD: u16 = 0x4f49;
const HELP_DONE: u16 = 0x50d8;
const HELP_HOW: u16 = 0x4fc4;
const HOW_TITLE: u16 = 0x4fc5;
const HOW_TEXT: u16 = 0x4f4a;
const BOARD_TEXT: u16 = 0x5011;
const BOARD_TEXT_ROTATE: u16 = 0x4f87;
const BOARD_TEXT_DONE: u16 = 0x50d2;
const BANNER_TEXT: u16 = 0x50d1;
const OBTAINED_TEXT: u16 = 0x50d0;
pub(super) const HELP_PIECES: u16 = 0x4f7a;

/// Every message these pages show besides the tables' own.
pub const MESSAGES: [u16; 17] = [
    0x4f78,
    0x4f79,
    0x50c2,
    HELP_MENU,
    HELP_LIST,
    HELP_BOARD,
    HELP_DONE,
    HELP_HOW,
    HOW_TITLE,
    HOW_TEXT,
    BOARD_TEXT,
    BOARD_TEXT_ROTATE,
    BOARD_TEXT_DONE,
    BANNER_TEXT,
    OBTAINED_TEXT,
    HELP_PIECES,
    0x50aa,
];

/// A board slot (save+0x4e20: piece, rotation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub piece: i8,
    pub rot: u8,
}

impl Slot {
    pub const EMPTY: Slot = Slot {
        piece: -1,
        rot: 0xff,
    };
}

/// The puzzle fields of the save (journal block J = save+0x4c38).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PuzzleSave {
    /// (puzzle, piece) owned (save+0x24a0).
    pub owned: BTreeSet<(u8, u8)>,
    /// Seen on a board (J+0x144) and in the List of Pieces (J+0x194).
    pub seen_board: BTreeSet<(u8, u8)>,
    pub seen_list: BTreeSet<(u8, u8)>,
    /// J+0x1e4 and J+0x1e6 bit 0.
    pub completed: u16,
    pub tutorial: bool,
    /// J+0x1e8: 10 puzzles of 48 slots.
    pub slots: Vec<[Slot; 48]>,
    /// The CRT `rand` state (seed 1 unless `srand` ran; the exe's other
    /// callers share it, so its placement can't be reproduced exactly).
    pub rng: u32,
    /// Items won and not yet handed to the inventory (FUN_1403c3f00).
    pub rewards: Vec<u16>,
}

impl Default for PuzzleSave {
    fn default() -> Self {
        Self {
            owned: BTreeSet::new(),
            seen_board: BTreeSet::new(),
            seen_list: BTreeSet::new(),
            completed: 0,
            tutorial: false,
            slots: vec![[Slot::EMPTY; 48]; 10],
            rng: 1,
            rewards: Vec::new(),
        }
    }
}

/// FUN_14033da80's shape table (0x1405bbd78): columns, rows, tile.
fn shape(big: bool) -> (usize, usize, usize) {
    if big { (6, 8, 42) } else { (3, 4, 84) }
}

fn crt_rand(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(214013).wrapping_add(2531011);
    (*state >> 16) & 0x7fff
}

/// FUN_140341280: every owned piece not on a board yet goes to a random
/// empty slot of its puzzle, with a random rotation where pieces rotate.
pub fn place_pieces(save: &mut PuzzleSave, puzzles: &[Puzzle], pieces: &[Piece]) {
    let mut rng = save.rng;
    for p in puzzles {
        let row = p.id as usize;
        let Some(slots) = save.slots.get_mut(row) else {
            continue;
        };
        let on_board: u64 = slots
            .iter()
            .filter(|s| s.piece >= 0)
            .fold(0, |m, s| m | 1 << s.piece);
        let mut taken = 0u64;
        let mut waiting: Vec<u8> = Vec::new();
        for r in pieces.iter().filter(|r| r.puzzle == p.id && r.piece < 48) {
            let bit = 1u64 << r.piece;
            if save.owned.contains(&(r.puzzle, r.piece)) && (on_board | taken) & bit == 0 {
                taken |= bit;
                waiting.push(r.piece);
            }
        }
        if waiting.is_empty() {
            continue;
        }
        let (cols, rows, _) = shape(p.big);
        let mut empty: Vec<usize> = (0..cols * rows).filter(|&s| slots[s].piece < 0).collect();
        let u = empty.len();
        if u > 1 {
            for k in 0..256 {
                let j = (crt_rand(&mut rng) << 17) as usize % u;
                empty.swap(j, k % u);
            }
            if !p.rotates {
                // A piece doesn't start on its own place.
                for t in 0..waiting.len().min(u) {
                    if empty[t] == waiting[t] as usize
                        && let Some(q) = (0..u).find(|&q| q != t && empty[q] != waiting[t] as usize)
                    {
                        empty.swap(t, q);
                    }
                }
            }
        }
        for (t, &piece) in waiting.iter().enumerate().take(u) {
            let slot = &mut slots[empty[t]];
            slot.piece = piece as i8;
            slot.rot = if p.rotates {
                let rot = (crt_rand(&mut rng) >> 12) as u8 & 3;
                // The exe compares the piece with the loop counter here.
                if piece as usize == t && rot == 0 {
                    ((crt_rand(&mut rng) >> 12) % 3) as u8 + 1
                } else {
                    rot
                }
            } else {
                0
            };
        }
    }
    save.rng = rng;
}

/// What the rotation leaves at each corner of a slot: where the picture's
/// (u0,v0), (u1,v0), (u0,v1), (u1,v1) go among TL, TR, BL, BR (FUN_14033daa0).
fn rotated_corners(rot: u8) -> [usize; 4] {
    match rot & 3 {
        0 => [0, 1, 2, 3],
        1 => [1, 3, 0, 2],
        2 => [3, 2, 1, 0],
        _ => [2, 0, 3, 1],
    }
}

/// A piece's slice of the picture at `at`.
fn piece_quad(at: [f32; 2], big: bool, piece: i8, rot: u8, alpha: f32) -> Quad {
    let (cols, _, tile) = shape(big);
    let (col, row) = (piece as usize % cols, piece as usize / cols);
    let t = tile as f32;
    let box_ = [
        at,
        [at[0] + t, at[1]],
        [at[0], at[1] + t],
        [at[0] + t, at[1] + t],
    ];
    Quad {
        corners: rotated_corners(rot).map(|k| box_[k]),
        uv: [
            (tile * col) as i32,
            (tile * row) as i32,
            (tile * (col + 1)) as i32,
            (tile * (row + 1)) as i32,
        ],
        colors: [[1., 1., 1., alpha]; 4],
        blend: 0,
    }
}

/// FUN_1403ae030: the item's name in place of the message's 0x0d.
fn with_item(message: &[u8], item: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < message.len() {
        let b = message[i];
        if b == 0x0d {
            out.extend_from_slice(item);
            i += 1;
            continue;
        }
        let n = if b < 0x20 {
            crate::dialogue::code_length(b).max(1)
        } else {
            1
        };
        out.extend_from_slice(&message[i..(i + n).min(message.len())]);
        i += n;
    }
    out
}

#[derive(Default)]
pub(super) struct Page {
    /// `puzz` rows listed (PE bit 24), in table order.
    listed: Vec<usize>,
    /// PB+0x40 bits 14-17: the puzzles with a piece on their board.
    known: usize,
    rows: Vec<Id>,
    marks: Vec<Id>,
    panel: Option<Id>,
    /// The board: its `puzz` row, the cursors (A, B) as (col, row), the
    /// held piece's rotation and whether one is held.
    current: usize,
    cursor: [[usize; 2]; 2],
    holding: bool,
    held_rot: u8,
    ready: bool,
    frame: Option<Id>,
    text: Option<Id>,
    cursors: [Option<Id>; 2],
    hand: Option<Id>,
    cursors_gone: bool,
    /// The banner's objects.
    banner: Vec<Id>,
    reward: picture::Picture,
    /// The tutorial's lines and first line shown.
    lines: Vec<Vec<u8>>,
    top: usize,
    /// The NEW bits at the opening (DAT_140befe08 + 0x144 / +0x194 clear).
    seen_board_at_open: BTreeSet<(u8, u8)>,
    seen_list_at_open: BTreeSet<(u8, u8)>,
}

impl Page {
    pub(super) fn at_open(save: &PuzzleSave) -> Self {
        Self {
            seen_board_at_open: save.seen_board.clone(),
            seen_list_at_open: save.seen_list.clone(),
            ..Self::default()
        }
    }
}

impl Journal {
    /// The save's puzzle fields, with the items won handed over.
    pub fn take_puzzles(&self) -> PuzzleSave {
        self.data.puzzle.clone()
    }

    fn puzzle_row(&self, row: usize) -> Option<&Puzzle> {
        self.data.puzzles.get(row)
    }

    fn puzzle_known(&self, row: usize) -> bool {
        self.puzzle_row(row)
            .and_then(|p| self.data.puzzle.slots.get(p.id as usize))
            .is_some_and(|s| s.iter().any(|x| x.piece >= 0))
    }

    fn puzzle_complete(&self, row: usize) -> bool {
        self.puzzle_row(row)
            .is_some_and(|p| p.id < 16 && self.data.puzzle.completed & (1 << p.id) != 0)
    }

    /// FUN_140347650 / FUN_140347540: a piece of the puzzle (`None`: of any)
    /// was new at the opening, and whether all of those were seen since.
    fn board_mark(&self, puzzle: Option<u8>) -> Option<bool> {
        let at_open = &self.puzzle.seen_board_at_open;
        let fresh: Vec<&(u8, u8)> = self
            .data
            .puzzle
            .owned
            .iter()
            .filter(|k| puzzle.is_none_or(|p| k.0 == p) && !at_open.contains(k))
            .collect();
        (!fresh.is_empty()).then(|| {
            fresh
                .iter()
                .all(|k| self.data.puzzle.seen_board.contains(k))
        })
    }

    /// FUN_1403474b0 / FUN_140347400 for the List of Pieces.
    pub(super) fn list_mark(&self, world: Option<u8>) -> Option<bool> {
        let at_open = &self.puzzle.seen_list_at_open;
        let fresh: Vec<(u8, u8)> = self
            .data
            .pieces
            .iter()
            .filter(|r| {
                world.is_none_or(|w| data::journal_table::journal_world(r.world) == w as i8)
                    && r.flag != 0
                    && self.flag(r.flag)
                    && !at_open.contains(&(r.puzzle, r.piece))
            })
            .map(|r| (r.puzzle, r.piece))
            .collect();
        (!fresh.is_empty()).then(|| fresh.iter().all(|k| self.data.puzzle.seen_list.contains(k)))
    }

    /// FUN_140346550: the world's found pieces count as seen in the list.
    pub(super) fn pieces_seen(&mut self, world: u8) {
        let found: Vec<(u8, u8)> = self
            .data
            .pieces
            .iter()
            .filter(|r| {
                data::journal_table::journal_world(r.world) == world as i8
                    && r.flag != 0
                    && self.flag(r.flag)
            })
            .map(|r| (r.puzzle, r.piece))
            .collect();
        self.data.puzzle.seen_list.extend(found);
    }

    fn puzzle_rows(&mut self, seq: &Sequence, panel: Id, n: usize) {
        self.puzzle.rows.clear();
        self.puzzle.marks.clear();
        for i in 0..n {
            let mut t = Self::text(Vec::new(), SIZE);
            t.group_position = false;
            t.keep_rgb = false;
            let r = self.page_sprite(seq, BASE, ROW_GROUP, panel, Some(t));
            self.sprites.get_mut(r).offset = [0., PITCH * i as f32];
            let m = self.page_sprite(seq, PRIO_MARK, ROW_GROUP, r, None);
            self.puzzle.rows.push(r);
            self.puzzle.marks.push(m);
        }
    }

    fn set_mark(&mut self, seq: &Sequence, row: usize, mark: Option<bool>) {
        if let (Some(&m), Some(seen)) = (self.puzzle.marks.get(row), mark) {
            self.sprites.set(seq, m, MARK[seen as usize], None);
        }
    }

    /// FUN_140342860.
    pub(super) fn build_puzzle_menu(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_light = true;
        self.cat.detail = false;
        self.category_frame(seq);
        let c = self.container;
        let panel = self.page_sprite(seq, PRIO_PANEL, PANEL, c, None);
        self.puzzle.panel = Some(panel);
        self.puzzle_rows(seq, panel, MENU_ROWS.len());
        for (i, (label, _)) in MENU_ROWS.iter().enumerate() {
            let text = self.msg(*label);
            self.set_text(self.puzzle.rows.get(i).copied(), text);
        }
        self.set_mark(seq, 0, self.board_mark(None));
        self.set_mark(seq, 1, self.list_mark(None));
        self.list = MenuList::new(self.puzzle.rows.clone(), self.memory[MENU as usize]);
        self.list.total = MENU_ROWS.len();
        self.list.mask = 0x73;
        self.show_help(seq, HELP_MENU);
    }

    /// FUN_14033f600.
    pub(super) fn puzzle_menu(
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
        if let Some(s) = sound(input, moved, true) {
            sounds.push(s);
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
                self.memory[MENU as usize] = k;
                self.hide_help(seq);
                self.slide(seq, 0, 1);
                self.pending = Some(MENU_ROWS[k].1);
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }

    /// FUN_14033de40 and FUN_140342370.
    pub(super) fn build_puzzle_list(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_light = true;
        self.cat.detail = false;
        let all = self.show_all();
        self.puzzle.listed = (0..self.data.puzzles.len())
            .filter(|&r| all || self.puzzle_known(r))
            .collect();
        self.puzzle.known = (0..self.data.puzzles.len())
            .filter(|&r| self.puzzle_known(r))
            .count();
        self.category_frame(seq);
        let tab = self.msg(TAB_PUZZLE);
        self.tab1_in(seq, tab);
        let c = self.container;
        let panel = self.page_sprite(seq, PRIO_PANEL, PANEL, c, None);
        self.puzzle.panel = Some(panel);
        self.puzzle_rows(seq, panel, LIST_ROWS);
        for (k, r) in self.puzzle.listed.clone().into_iter().enumerate() {
            let known = self.puzzle_known(r);
            let Some(p) = self.puzzle_row(r).cloned() else {
                continue;
            };
            let text = if known {
                self.msg(p.name)
            } else {
                vec![0x49; 5]
            };
            self.set_text(self.puzzle.rows.get(k).copied(), text);
            if known {
                self.set_mark(seq, k, self.board_mark(Some(p.id)));
            }
        }
        let n = self.puzzle.listed.len();
        let cursor = self.memory[LIST as usize].min(n.saturating_sub(1));
        self.list = MenuList::new(self.puzzle.rows[..n].to_vec(), cursor);
        self.list.total = n;
        self.list.mask = 0x73;
        self.show_help(seq, HELP_LIST);
    }

    /// FUN_14033f310.
    pub(super) fn puzzle_list(
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
        let valid = match input {
            In::Row(k) => self
                .puzzle
                .listed
                .get(k)
                .is_some_and(|&r| self.puzzle_known(r)),
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
                self.tab1_out(seq);
                self.hide_help(seq);
                self.slide(seq, 1, 1);
                self.pending = Some(MENU);
                self.substate = 2;
            }
            In::Row(k) => {
                self.memory[LIST as usize] = k;
                self.hide_help(seq);
                self.slide(seq, 0, 1);
                self.pending = Some(if self.data.puzzle.tutorial {
                    BOARD
                } else {
                    HELP_FIRST
                });
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }

    /// FUN_140342c20. The exe clips the whole text to nine lines; here the
    /// nine rows hold the lines in view.
    pub(super) fn build_puzzle_help(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_cursor = true;
        self.no_light = true;
        self.cat.detail = false;
        self.category_frame(seq);
        if self.state != HELP_READ {
            let tab = self.msg(TAB_PUZZLE);
            self.tab1_in(seq, tab);
        }
        let c = self.container;
        let panel = self.page_sprite(seq, PRIO_PANEL, HELP_PANEL, c, None);
        let track = self.page_sprite(seq, PRIO_PANEL, HELP_TRACK, c, None);
        let mut t = Self::text(self.msg(HOW_TITLE), Self::size(seq, HELP_TITLE));
        t.align = 2;
        t.group_position = false;
        t.keep_rgb = false;
        self.page_sprite(seq, BASE, HELP_TITLE, c, Some(t));
        self.puzzle.lines = self
            .msg(HOW_TEXT)
            .split(|&b| b == 0x02)
            .map(<[u8]>::to_vec)
            .collect();
        self.puzzle.top = 0;
        self.puzzle_rows(seq, panel, HELP_LINES);
        self.bar_in(seq, track, self.puzzle.lines.len(), HELP_LINES);
        self.help_lines(seq);
        self.show_help(seq, HELP_HOW);
    }

    fn help_lines(&mut self, seq: &Sequence) {
        for i in 0..HELP_LINES {
            let text = self
                .puzzle
                .lines
                .get(self.puzzle.top + i)
                .cloned()
                .unwrap_or_default();
            self.set_text(self.puzzle.rows.get(i).copied(), text);
        }
        self.bar_to(seq, self.puzzle.top);
    }

    /// FUN_14033f890: decide (the first time) or cancel go on; the tutorial
    /// counts as read.
    pub(super) fn puzzle_help(
        &mut self,
        layout_: &Layout,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        let any = pad.trig & 0xc1fc != 0;
        if self.substate == 2 {
            self.leave(
                layout_,
                if any { In::Row(0) } else { In::None },
                false,
                false,
            );
            return false;
        }
        self.settle(seq, if any { In::Row(0) } else { In::None }, false, true);
        let rep = pad.repeat | pad.trig;
        let last = self.puzzle.lines.len().saturating_sub(HELP_LINES);
        let top = self.puzzle.top;
        self.puzzle.top = if rep & bits::UP != 0 {
            top.saturating_sub(1)
        } else if rep & bits::DOWN != 0 {
            (top + 1).min(last)
        } else if rep & bits::L1 != 0 {
            top.saturating_sub(HELP_LINES)
        } else if rep & bits::R1 != 0 {
            (top + HELP_LINES).min(last)
        } else {
            top
        };
        if self.puzzle.top != top {
            sounds.push(SOUND_CURSOR);
            self.help_lines(seq);
            return false;
        }
        let first = !self.data.puzzle.tutorial;
        let decide = pad.trig & bits::DECIDE != 0 && self.state == HELP_FIRST && first;
        if pad.trig & bits::START != 0 {
            sounds.push(SOUND_CLOSE);
            self.data.puzzle.tutorial = true;
            self.exit = true;
        } else if decide || pad.trig & bits::CANCEL != 0 {
            sounds.push(if decide { SOUND_DECIDE } else { SOUND_CLOSE });
            self.hide_help(seq);
            self.data.puzzle.tutorial = true;
            let forward = first && self.state == HELP_FIRST;
            self.slide(seq, if forward { 0 } else { 1 }, 1);
            self.pending = Some(if self.state == HELP_FIRST {
                BOARD
            } else {
                MENU
            });
            self.substate = 2;
        }
        false
    }

    /// FUN_140341c40: the page; the pieces and cursors follow the picture.
    pub(super) fn build_puzzle_board(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_cursor = true;
        self.no_light = true;
        self.cat.detail = false;
        self.list.mask = 0;
        let k = self.memory[LIST as usize];
        let row = self.puzzle.listed.get(k).copied().unwrap_or(0);
        let Some(p) = self.puzzle_row(row).cloned() else {
            return;
        };
        // FUN_140346600: the puzzle's owned pieces count as seen.
        let owned: Vec<(u8, u8)> = self
            .data
            .puzzle
            .owned
            .iter()
            .filter(|o| o.0 == p.id)
            .copied()
            .collect();
        self.data.puzzle.seen_board.extend(owned);
        let page = &mut self.puzzle;
        page.current = row;
        page.cursor = [[0; 2]; 2];
        page.holding = false;
        page.held_rot = 0;
        page.ready = false;
        page.cursors = [None; 2];
        page.hand = None;
        page.cursors_gone = false;
        self.category_frame(seq);
        let c = self.container;
        let panel = self.page_sprite(seq, PRIO_PANEL, BOARD_PANEL, c, None);
        self.puzzle.panel = Some(panel);
        let frame = self.page_sprite(seq, PRIO_BOARD, BOARD_FRAME[p.big as usize], c, None);
        self.puzzle.frame = Some(frame);
        // PE's name: "?????" for a puzzle only show-all lists.
        let name = if self.puzzle_known(row) {
            self.msg(p.name)
        } else {
            vec![0x49; 5]
        };
        let mut t = Self::text(name, SIZE);
        t.group_position = false;
        t.keep_rgb = false;
        self.page_sprite(seq, PRIO_BOARD, ROW_GROUP, panel, Some(t));
        let complete = self.puzzle_complete(row);
        let id = match (complete, p.rotates) {
            (true, _) => BOARD_TEXT_DONE,
            (false, true) => BOARD_TEXT_ROTATE,
            (false, false) => BOARD_TEXT,
        };
        let mut t = Self::text(self.msg(id), SIZE);
        t.group_position = false;
        t.keep_rgb = false;
        let text = self.page_sprite(seq, PRIO_BOARD, ROW_GROUP, panel, Some(t));
        self.puzzle.text = Some(text);
        if complete {
            self.obtained_line(seq);
        }
        self.board_load(seq);
        self.show_help(seq, if complete { HELP_DONE } else { HELP_BOARD });
    }

    /// FUN_14033e2a0: the OBTAINED line under a solved board.
    fn obtained_line(&mut self, seq: &Sequence) {
        let Some(panel) = self.puzzle.panel else {
            return;
        };
        let item = self.reward_name();
        let line = with_item(&self.msg(OBTAINED_TEXT), &item);
        let mut t = Self::text(line, Self::size(seq, OBTAINED_LINE));
        t.align = 2;
        t.group_position = false;
        self.page_sprite(seq, PRIO_PIECE, OBTAINED_LINE, panel, Some(t));
    }

    /// FUN_1403c4330: the reward's name with its icon.
    fn reward_name(&self) -> Vec<u8> {
        let Some(p) = self.puzzle_row(self.puzzle.current) else {
            return Vec::new();
        };
        let Some(info) = self.data.item_names.get(&p.reward) else {
            return Vec::new();
        };
        let mut t = Vec::new();
        if info.icon != 0 {
            t.extend([0x09, info.icon]);
        }
        t.extend(&info.name);
        t
    }

    /// FUN_1403433b0 / FUN_14033e3d0: once the picture is in, the cursors.
    fn board_load(&mut self, seq: &Sequence) {
        if self.puzzle.ready {
            return;
        }
        let row = self.puzzle.current as u8;
        if !self.cache.puzzles.contains(&row) {
            self.cache.wanted_puzzle = Some(row);
            return;
        }
        let (Some(frame), Some(p)) = (self.puzzle.frame, self.puzzle_row(self.puzzle.current))
        else {
            return;
        };
        let group = CURSOR[p.big as usize];
        self.puzzle.ready = true;
        if self.puzzle_complete(self.puzzle.current) {
            return;
        }
        let a = self.page_sprite(seq, PRIO_CURSOR_A, group[0], frame, None);
        let b = self.page_sprite(seq, PRIO_CURSOR_B, group[1], frame, None);
        self.sprites.get_mut(b).hidden = true;
        let hand = self.page_sprite(seq, PRIO_HAND, GRAB_HAND, a, None);
        self.puzzle.cursors = [Some(a), Some(b)];
        self.puzzle.hand = Some(hand);
        self.place_cursor();
    }

    /// FUN_1403431f0: the active cursor (and the hand on it) on its slot.
    fn place_cursor(&mut self) {
        let Some(p) = self.puzzle_row(self.puzzle.current) else {
            return;
        };
        let tile = shape(p.big).2 as f32;
        let a = self.puzzle.holding as usize;
        let [col, row] = self.puzzle.cursor[a];
        if let Some(c) = self.puzzle.cursors[a] {
            self.sprites.get_mut(c).offset = [col as f32 * tile, row as f32 * tile];
            if let Some(h) = self.puzzle.hand {
                self.sprites.attach(h, c);
            }
        }
    }

    /// FUN_14033f000: the board's own pad decode; a direction wins.
    fn board_input(&mut self, pad: &Pad) -> u64 {
        let Some(p) = self.puzzle_row(self.puzzle.current).cloned() else {
            return 0;
        };
        let complete = self.puzzle_complete(self.puzzle.current);
        let (w, h, _) = shape(p.big);
        let a = self.puzzle.holding as usize;
        let [mut col, mut row] = self.puzzle.cursor[a];
        let rep = pad.repeat | pad.trig;
        let mut moved = 0;
        if !complete {
            if rep & bits::UP != 0 {
                let r = match row {
                    0 if pad.trig & bits::UP != 0 => h - 1,
                    0 => 0,
                    _ => row - 1,
                };
                moved |= if r != row { bits::UP } else { 0 };
                row = r;
            } else if rep & bits::DOWN != 0 {
                let r = if row + 1 < h {
                    row + 1
                } else if pad.trig & bits::DOWN != 0 {
                    0
                } else {
                    row
                };
                moved |= if r != row { bits::DOWN } else { 0 };
                row = r;
            }
            if rep & bits::LEFT != 0 {
                let c = match col {
                    0 if pad.trig & bits::LEFT != 0 => w - 1,
                    0 => 0,
                    _ => col - 1,
                };
                moved |= if c != col { bits::LEFT } else { 0 };
                col = c;
            } else if rep & bits::RIGHT != 0 {
                let c = if col + 1 < w {
                    col + 1
                } else if pad.trig & bits::RIGHT != 0 {
                    0
                } else {
                    col
                };
                moved |= if c != col { bits::RIGHT } else { 0 };
                col = c;
            }
        }
        if moved != 0 {
            self.puzzle.cursor[a] = [col, row];
            return moved;
        }
        if !complete && pad.trig & bits::DECIDE != 0 {
            return bits::DECIDE;
        }
        for bit in [bits::CANCEL, bits::START, bits::SELECT] {
            if pad.trig & bit != 0 {
                return bit;
            }
        }
        if self.puzzle.known >= 2 {
            for bit in [bits::R2, bits::L2] {
                if pad.trig & bit != 0 {
                    return bit;
                }
            }
        }
        if !complete && p.rotates {
            for bit in [ROTATE_RIGHT, ROTATE_LEFT] {
                if pad.trig & bit != 0 {
                    return bit;
                }
            }
        }
        0
    }

    fn board_slot_index(&self, cursor: usize) -> usize {
        let Some(p) = self.puzzle_row(self.puzzle.current) else {
            return 0;
        };
        let [col, row] = self.puzzle.cursor[cursor];
        col + shape(p.big).0 * row
    }

    fn board_slot(&mut self, slot: usize) -> Option<&mut Slot> {
        let id = self.puzzle_row(self.puzzle.current)?.id as usize;
        self.data.puzzle.slots.get_mut(id)?.get_mut(slot)
    }

    /// FUN_1403405b0: the hand, cursors and held piece go.
    fn kill_cursors(&mut self) {
        for c in self.puzzle.cursors.into_iter().flatten() {
            self.sprites.kill(c);
        }
        if let Some(h) = self.puzzle.hand {
            self.sprites.kill(h);
        }
        self.puzzle.cursors_gone = true;
        self.puzzle.holding = false;
    }

    /// FUN_140343050 / FUN_1403400f0: the cursor sprites for grab or place.
    fn show_grab(&mut self, seq: &Sequence, grabbing: bool) {
        let Some(big) = self.puzzle_row(self.puzzle.current).map(|p| p.big as usize) else {
            return;
        };
        let [Some(a), Some(b)] = self.puzzle.cursors else {
            return;
        };
        self.sprites.get_mut(b).hidden = !grabbing;
        let group = CURSOR[big][if grabbing { 2 } else { 0 }];
        self.sprites.set(seq, a, group, None);
    }

    /// FUN_140340b80: grab, place or (cancel) put back.
    fn grab_place(&mut self, seq: &Sequence, place: bool) {
        let a = self.board_slot_index(0);
        if !self.puzzle.holding {
            self.puzzle.cursor[1] = self.puzzle.cursor[0];
            self.puzzle.held_rot = self.board_slot(a).map_or(0, |s| s.rot) & 3;
            self.show_grab(seq, true);
        } else {
            if place {
                let b = self.board_slot_index(1);
                let rot = self.puzzle.held_rot;
                if let Some(id) = self.puzzle_row(self.puzzle.current).map(|p| p.id as usize)
                    && let Some(slots) = self.data.puzzle.slots.get_mut(id)
                {
                    slots[a].rot = rot;
                    slots.swap(a, b);
                }
                self.puzzle.cursor[0] = self.puzzle.cursor[1];
            }
            self.show_grab(seq, false);
        }
        self.puzzle.holding = !self.puzzle.holding;
        self.place_cursor();
    }

    /// FUN_14033dd30.
    fn board_solved(&self) -> bool {
        let Some(p) = self.puzzle_row(self.puzzle.current) else {
            return false;
        };
        let (w, h, _) = shape(p.big);
        self.data
            .puzzle
            .slots
            .get(p.id as usize)
            .is_some_and(|s| (0..w * h).all(|k| s[k].piece == k as i8 && s[k].rot == 0))
    }

    /// FUN_14033e650.
    pub(super) fn puzzle_board(
        &mut self,
        layout_: &Layout,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        let mut cmd = self.board_input(pad);
        let press = if cmd != 0 { In::Row(0) } else { In::None };
        if self.substate == 2 {
            self.leave(layout_, press, false, false);
            return false;
        }
        self.board_load(seq);
        self.settle(seq, press, false, true);
        let complete = self.puzzle_complete(self.puzzle.current);
        let usable = self.puzzle.ready && (complete || self.puzzle.cursors[0].is_some());
        if !usable {
            cmd = 0;
        }
        // The sound, or the empty slot's refusal.
        let a = self.board_slot_index(0);
        let turning = cmd & (bits::DECIDE | ROTATE_RIGHT | ROTATE_LEFT) != 0;
        if turning && !self.puzzle.holding && self.board_slot(a).is_none_or(|s| s.piece < 0) {
            sounds.push(SOUND_INVALID);
            return false;
        }
        let directions = bits::UP | bits::DOWN | bits::LEFT | bits::RIGHT;
        if cmd & (directions | ROTATE_RIGHT | ROTATE_LEFT) != 0 {
            sounds.push(SOUND_CURSOR);
        } else if cmd & (bits::CANCEL | bits::START) != 0 {
            sounds.push(SOUND_CLOSE);
        } else if cmd != 0 {
            sounds.push(SOUND_DECIDE);
        }
        match cmd {
            c if c & directions != 0 => self.place_cursor(),
            ROTATE_RIGHT | ROTATE_LEFT => {
                let delta = if cmd == ROTATE_RIGHT { 1 } else { 3 };
                if self.puzzle.holding {
                    self.puzzle.held_rot = (self.puzzle.held_rot + delta) & 3;
                } else if let Some(s) = self.board_slot(a) {
                    s.rot = (s.rot + delta) & 3;
                }
            }
            bits::START => {
                self.kill_cursors();
                self.exit = true;
            }
            bits::SELECT => {
                self.kill_cursors();
                self.hide_help(seq);
                self.slide(seq, 0, 1);
                self.pending = Some(HELP_FIRST);
                self.substate = 2;
            }
            bits::R2 | bits::L2 => {
                let n = self.puzzle.known.max(1) as i32;
                let step = if cmd == bits::R2 { 1 } else { -1 };
                self.kill_cursors();
                self.hide_help(seq);
                self.memory[LIST as usize] =
                    (self.memory[LIST as usize] as i32 + step).rem_euclid(n) as usize;
                self.slide(seq, if cmd == bits::R2 { 0 } else { 1 }, 1);
                self.pending = Some(BOARD);
                self.substate = 2;
            }
            bits::CANCEL => {
                if self.puzzle.holding {
                    self.grab_place(seq, false);
                } else {
                    self.kill_cursors();
                    self.hide_help(seq);
                    self.slide(seq, 1, 1);
                    self.pending = Some(LIST);
                    self.substate = 2;
                }
            }
            bits::DECIDE => self.grab_place(seq, true),
            _ => {}
        }
        if turning && !complete && self.board_solved() {
            self.puzzle_finish(layout_);
            sounds.push(SOUND_SOLVED);
        }
        false
    }

    /// The tail of FUN_14033e650 and FUN_140342050: the banner.
    fn puzzle_finish(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        let Some(p) = self.puzzle_row(self.puzzle.current).cloned() else {
            return;
        };
        if p.id < 16 {
            self.data.puzzle.completed |= 1 << p.id;
        }
        self.kill_cursors();
        self.state = SOLVED;
        self.substate = 0;
        let make = |this: &mut Self, g: [usize; 3], text: Option<Text>| {
            let id = this
                .sprites
                .create(seq, PRIO_BANNER, ART, g[0], Some(g[1]), text);
            this.page.push(id);
            this.puzzle.banner.push(id);
            id
        };
        self.puzzle.banner.clear();
        let back = make(self, BANNER_BACK, None);
        let mut t = Self::text(self.msg(BANNER_TEXT), Self::size(seq, BANNER[0]));
        t.align = 2;
        t.group_position = false;
        make(self, BANNER, Some(t));
        let mut t = Self::text(self.reward_name(), Self::size(seq, BANNER_NAME[0]));
        t.align = 2;
        t.group_position = false;
        t.keep_rgb = false;
        let name = make(self, BANNER_NAME, Some(t));
        let info = self.data.item_names.get(&p.reward).cloned();
        let description = info.as_ref().map(|i| i.description.clone());
        let mut t = Self::text(description.unwrap_or_default(), Self::size(seq, PLAIN_LINE));
        t.align = 2;
        t.group_position = false;
        let d = self.page_sprite(seq, PRIO_BANNER, PLAIN_LINE, name, Some(t));
        let g = &seq.groups[BANNER_NAME[0]];
        self.sprites.get_mut(d).offset = [g.light_position_x as f32, g.text_position_y as f32];
        // FUN_1403022b0 / FUN_1403022a0 / FUN_140302370.
        if let (Some(i), Some(ipic)) = (info, self.data.ipic.clone()) {
            self.puzzle.reward = picture::Picture::default();
            self.puzzle.reward.setup(PRIO_REWARD, None);
            self.puzzle.reward.set_groups(REWARD_PICTURE);
            self.puzzle.reward.set_parent(back);
            self.puzzle
                .reward
                .request(seq, &ipic, &mut self.sprites, i.picture);
        }
    }

    /// FUN_14033e080's case 0x20.
    pub(super) fn puzzle_solved(
        &mut self,
        layout_: &Layout,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        if let Some(ipic) = self.data.ipic.clone() {
            self.puzzle
                .reward
                .timers(seq, &ipic, &mut self.sprites, &mut self.cache.pictures);
            self.puzzle.reward.update(&ipic, &self.sprites);
        }
        match self.substate {
            0 => {
                let idle = |this: &Self, k: usize, g: usize| {
                    this.puzzle
                        .banner
                        .get(k)
                        .is_some_and(|&id| this.sprites.get(id).player.group == g)
                };
                if idle(self, 0, BANNER_BACK[1]) && idle(self, 2, BANNER_NAME[1]) {
                    self.substate = 1;
                }
            }
            1 => {
                if pad.trig & bits::DECIDE != 0 {
                    sounds.push(SOUND_DECIDE);
                    let closes = [BANNER_BACK[2], BANNER[2], BANNER_NAME[2]];
                    for (k, g) in closes.into_iter().enumerate() {
                        if let Some(&id) = self.puzzle.banner.get(k) {
                            self.sprites.close(seq, id, g);
                        }
                    }
                    if let Some(ipic) = self.data.ipic.clone() {
                        self.puzzle.reward.close(seq, &ipic, &mut self.sprites);
                    }
                    self.substate = 2;
                }
            }
            _ => {
                if self.puzzle.banner.iter().all(|&id| !self.sprites.alive(id)) {
                    // FUN_1403c3f00, the solved text and the OBTAINED line.
                    if let Some(p) = self.puzzle_row(self.puzzle.current) {
                        let reward = p.reward;
                        self.data.puzzle.rewards.push(reward);
                    }
                    self.puzzle.reward.reset();
                    let text = self.msg(BOARD_TEXT_DONE);
                    self.set_text(self.puzzle.text, text);
                    self.obtained_line(seq);
                    self.state = BOARD;
                    self.substate = 1;
                    self.hide_help(seq);
                    self.show_help(seq, HELP_DONE);
                }
            }
        }
        false
    }

    /// The picture's slices on the board, the held one and the item picture:
    /// (priority, sheet, quads). The held piece goes under the moving cursor.
    pub(super) fn puzzle_draws(&self) -> Vec<(u32, Sheet, Vec<Quad>)> {
        let mut out = Vec::new();
        if let (true, Some(frame), Some(p)) = (
            self.puzzle.ready,
            self.puzzle.frame,
            self.puzzle_row(self.puzzle.current),
        ) && matches!(self.state, BOARD | SOLVED)
            && self.sprites.alive(frame)
            && let Some(slots) = self.data.puzzle.slots.get(p.id as usize)
        {
            let (cols, rows, tile) = shape(p.big);
            let c = self.sprites.context(frame);
            let alpha = self.sprites.child_alpha(frame);
            let at =
                |col: usize, row: usize| [c[0] + (col * tile) as f32, c[1] + (row * tile) as f32];
            let sheet = Sheet::Puzzle(self.puzzle.current as u8);
            let quads = (0..cols * rows)
                .filter(|&k| slots[k].piece >= 0)
                .map(|k| {
                    piece_quad(
                        at(k % cols, k / cols),
                        p.big,
                        slots[k].piece,
                        slots[k].rot,
                        alpha,
                    )
                })
                .collect();
            out.push((PRIO_PIECE, sheet, quads));
            let origin = self.puzzle.cursor[0];
            let slot = slots[origin[0] + cols * origin[1]];
            if self.puzzle.holding && slot.piece >= 0 {
                let [col, row] = self.puzzle.cursor[1];
                let q = piece_quad(at(col, row), p.big, slot.piece, self.puzzle.held_rot, alpha);
                out.push((PRIO_CURSOR_A, sheet, vec![q]));
            }
        }
        if let Some(ipic) = &self.data.ipic
            && let Some((image, prio, quads)) = self.puzzle.reward.quads(ipic, &self.sprites)
        {
            out.push((prio, Sheet::Picture(image), quads));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn puzzle(id: u8, big: bool, rotates: bool) -> Puzzle {
        Puzzle {
            id,
            big,
            rotates,
            name: 0x4f7b,
            reward: 0x117,
            file: "puzzle020".into(),
        }
    }

    fn pieces(puzzle: u8, n: u8) -> Vec<Piece> {
        (0..n)
            .map(|piece| Piece {
                puzzle,
                piece,
                name: 0xcf8c,
                world: 2,
                room: 0,
                set: piece,
                flag: 1,
            })
            .collect()
    }

    #[test]
    fn owned_pieces_go_to_empty_slots_once() {
        let mut save = PuzzleSave::default();
        save.owned.extend((0..5).map(|k| (0, k)));
        // A piece already on the board stays where it is.
        save.slots[0][7] = Slot { piece: 2, rot: 0 };
        let table = pieces(0, 12);
        place_pieces(&mut save, &[puzzle(0, false, false)], &table);
        let placed: Vec<i8> = save.slots[0]
            .iter()
            .filter(|s| s.piece >= 0)
            .map(|s| s.piece)
            .collect();
        assert_eq!(placed.len(), 5);
        assert_eq!(save.slots[0][7].piece, 2);
        let mut sorted = placed.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3, 4]);
        // Without rotation a piece doesn't start on its own place.
        for (k, s) in save.slots[0].iter().enumerate() {
            assert!(s.piece < 0 || k == 7 || s.piece != k as i8);
            assert!(s.piece < 0 || s.rot == 0);
        }
        // Placing again changes nothing.
        let before = save.slots.clone();
        place_pieces(&mut save, &[puzzle(0, false, false)], &table);
        assert_eq!(save.slots, before);
    }

    #[test]
    fn a_rotating_puzzle_gives_rotations() {
        let mut save = PuzzleSave::default();
        save.owned.extend((0..12).map(|k| (1, k)));
        place_pieces(&mut save, &[puzzle(1, false, true)], &pieces(1, 12));
        assert!(
            save.slots[1][..12]
                .iter()
                .all(|s| s.piece >= 0 && s.rot < 4)
        );
        assert!(save.slots[1][..12].iter().any(|s| s.rot != 0));
    }

    #[test]
    fn a_rotated_piece_keeps_the_picture_turned() {
        // Rotation 1 (FUN_14033daa0): the slice's top-left corner shows
        // at the slot's top-right.
        let q = piece_quad([100., 200.], false, 4, 1, 1.);
        assert_eq!(q.uv, [84, 84, 168, 168]);
        assert_eq!(q.corners[0], [184., 200.]);
        assert_eq!(q.corners[1], [184., 284.]);
        assert_eq!(q.corners[2], [100., 200.]);
        assert_eq!(q.corners[3], [100., 284.]);
        // The tile of a 6 x 8 picture.
        assert_eq!(piece_quad([0., 0.], true, 7, 0, 1.).uv, [42, 42, 84, 84]);
    }

    #[test]
    fn the_item_goes_in_the_obtained_line() {
        let m = [0x07, 1, 2, 3, 4, 0x4a, 0x02, 0x0d];
        assert_eq!(
            with_item(&m, &[0x50, 0x51]),
            [0x07, 1, 2, 3, 4, 0x4a, 0x02, 0x50, 0x51]
        );
    }
}
