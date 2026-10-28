//! Characters (state 6) and Character Links (0xc from Notes, 0x28 from
//! Collection): FUN_14032dcc0 builds, FUN_14032c930 runs, FUN_14032d350
//! lists, FUN_14032c5c0 hit-tests, FUN_14032d950 moves the free hand.
//! Portraits sit at fixed layout positions from the `diag` table; the face
//! files are `menu/jmface_<code>.2ld` (unlocalised). The Character File
//! page behind a decide is not built: the decide plays its sound only.
use super::*;
use data::journal_table::{Diag, DiagEntry};

const PANEL: usize = 83;
const BOUNDS: usize = 84;
const PLATE: usize = 85;
const FRAME: [usize; 2] = [10, 16];
const HIGHLIGHT: [usize; 2] = [22, 26];
const NEW_MARK: [[usize; 2]; 2] = [[24, 25], [27, 28]];
const FREE_HAND: usize = 34;
/// Face groups (seq0): in, idle for Characters then Links.
const FACE: [[usize; 2]; 2] = [[1, 2], [4, 5]];
const PRIO_PANEL: u32 = 0x755c;
const PRIO_FRAME: u32 = 0x755f;
const PRIO_FACE: u32 = 0x7560;
const PRIO_HIGHLIGHT: u32 = 0x7561;
const PRIO_NEW: u32 = 0x7563;
const HELP: [u16; 2] = [0x48d5, 0x48d6];
const STEP: i32 = 3;
const HIT_HALF: i32 = 32;
const MAX_ENTRIES: usize = 32;
/// Character Links' world in `diag` and its face file.
pub const LINKS_WORLD: u8 = 0x10;

#[derive(Default)]
pub(super) struct Page {
    entries: Vec<DiagEntry>,
    frames: Vec<Id>,
    panel: Option<Id>,
    bounds_anchor: Option<Id>,
    name: Option<Id>,
    cursor: [i32; 2],
    /// x min, y min, x max, y max once read.
    bounds: Option<[i32; 4]>,
    sel: i32,
    hand: Option<Id>,
    highlight: Option<Id>,
    faces: bool,
    /// The face file's key: the journal world, or `LINKS_WORLD`.
    pub key: u8,
    links: bool,
}

impl Page {
    /// The bounds anchor, to be killed with the page.
    pub(super) fn take_anchor(&mut self) -> Option<Id> {
        self.bounds_anchor.take()
    }
}

/// FUN_140345130: a `diag` record is active.
fn active(r: &Diag, flag: impl Fn(u16) -> bool, item: impl Fn(u16) -> u32) -> bool {
    let a = if r.kind == 2 {
        item(r.cond) > 0
    } else {
        flag(r.cond)
    };
    !(r.off != 0 && flag(r.off)) && a
}

/// FUN_14032d350: the page's entries — the world's base record (the first
/// inactive one ends the scan), then every active extra, visible entries
/// only.
pub fn layout(
    diag: &[Diag],
    world: i8,
    flag: impl Fn(u16) -> bool,
    item: impl Fn(u16) -> u32,
) -> Vec<DiagEntry> {
    let (mut best, mut seen) = (None, false);
    for (i, r) in diag.iter().enumerate() {
        if r.world == world {
            seen = true;
            if r.kind == 0 {
                if !active(r, &flag, &item) {
                    break;
                }
                best = Some(i);
            }
        } else if seen {
            break;
        }
    }
    let mut out: Vec<DiagEntry> = Vec::new();
    if let Some(b) = best {
        out.extend(diag[b].entries.iter().filter(|e| e.visible));
    }
    seen = false;
    for r in diag {
        if r.world == world {
            seen = true;
            if r.kind != 0 && active(r, &flag, &item) {
                out.extend(r.entries.iter().filter(|e| e.visible));
            }
        } else if seen {
            break;
        }
    }
    out.truncate(MAX_ENTRIES);
    out
}

impl Journal {
    /// FUN_14032dcc0(-1).
    pub(super) fn build_characters(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        // No list rows: the generic hand never appears here.
        self.no_cursor = true;
        self.chars = Page::default();
        let links = self.state != 6;
        let world = if links {
            LINKS_WORLD as i8
        } else {
            self.world.max(0) as i8
        };
        self.chars.links = links;
        self.chars.key = world as u8;
        self.chars.entries = {
            let flags = &self.data.flags;
            layout(
                &self.data.diag,
                world,
                |f| f != 0xffff && flags.contains(&f),
                |_| 0,
            )
        };
        self.category_frame(seq);
        let c = self.container;
        let panel = self.page_sprite(seq, PRIO_PANEL, PANEL, c, None);
        self.chars.panel = Some(panel);
        let anchor = self.sprites.create(seq, BASE, ART, BOUNDS, None, None);
        self.chars.bounds_anchor = Some(anchor);
        let mut t = Self::text(Vec::new(), Self::size(seq, PLATE));
        t.align = 2;
        t.group_position = false;
        t.keep_rgb = false;
        self.chars.name = Some(self.page_sprite(seq, BASE, PLATE, c, Some(t)));
        let seen_at_open = self.seen_chars_at_open.clone();
        for k in 0..self.chars.entries.len() {
            let e = self.chars.entries[k];
            let ch = self.data.chars.get(e.character as usize).copied();
            let ft = ch.map_or(0, |c| c.frame as usize).min(5);
            let g = FRAME[links as usize] + ft;
            let f = self.page_sprite(seq, PRIO_FRAME, g, panel, None);
            self.sprites.get_mut(f).offset = [e.at[0] as f32, e.at[1] as f32];
            // NEW marks: available and not seen when the journal opened.
            let new = self.char_available(e.character) && !seen_at_open.contains(&e.character);
            let seen_now = self.seen_chars.contains(&e.character);
            let group = if !new {
                2
            } else {
                NEW_MARK[links as usize][seen_now as usize]
            };
            self.page_sprite(seq, PRIO_NEW, group, f, None);
            self.chars.frames.push(f);
        }
        self.show_help(seq, HELP[links as usize]);
        self.chars.cursor = [0; 2];
        self.chars.sel = -1;
    }

    pub(super) fn char_available(&self, character: u16) -> bool {
        self.data.char_available(character)
    }

    /// FUN_14032c5c0.
    fn char_hit(&mut self) {
        let [cx, cy] = self.chars.cursor;
        self.chars.sel = self
            .chars
            .entries
            .iter()
            .position(|e| {
                let (px, py) = (e.at[0] as i32, e.at[1] as i32);
                px - HIT_HALF <= cx
                    && cx <= px + HIT_HALF
                    && py - HIT_HALF <= cy
                    && cy <= py + HIT_HALF
            })
            .map_or(-1, |k| k as i32);
    }

    /// FUN_14032d950: one frame of the free hand.
    fn char_move(&mut self) {
        let Some([xmin, ymin, xmax, ymax]) = self.chars.bounds else {
            return;
        };
        let (held, stick) = (self.held, self.stick);
        let [mut x, mut y] = self.chars.cursor;
        let analog = |v: f32| (v.abs() * 6. + 0.5) as i32;
        if held & bits::LEFT != 0 {
            let dx = if stick[2] == 0. || stick[0] >= 0. {
                STEP
            } else {
                analog(stick[0])
            };
            x = xmin.max(x - dx);
        } else if held & bits::RIGHT != 0 {
            let dx = if stick[2] == 0. || stick[0] <= 0. {
                STEP
            } else {
                analog(stick[0])
            };
            x = xmax.min(x + dx);
        }
        if held & bits::UP != 0 {
            let dy = if stick[2] == 0. || stick[1] >= 0. {
                STEP
            } else {
                analog(stick[1])
            };
            y = ymin.max(y - dy);
        } else if held & bits::DOWN != 0 {
            let dy = if stick[2] == 0. || stick[1] <= 0. {
                STEP
            } else {
                analog(stick[1])
            };
            y = ymax.min(y + dy);
        }
        if [x, y] != self.chars.cursor {
            self.chars.cursor = [x, y];
            if let Some(h) = self.chars.hand {
                self.sprites.get_mut(h).offset = [x as f32, y as f32];
            }
        }
    }

    /// FUN_14032c930.
    pub(super) fn characters(
        &mut self,
        layout_: &Layout,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        let key_press = pad.trig & 0x1fc != 0;
        if self.substate == 2 {
            let input = if key_press { In::Row(0) } else { In::None };
            self.leave(layout_, input, false, false);
            return false;
        }
        let links = self.chars.links;
        // The faces appear once the file is bound.
        if !self.chars.faces {
            if self.cache.faces.contains(&self.chars.key) {
                self.chars.faces = true;
                let seq0 = &layout_.sequences[0];
                let key = self.chars.key;
                for k in 0..self.chars.entries.len() {
                    let img = self
                        .data
                        .chars
                        .get(self.chars.entries[k].character as usize)
                        .map_or(0, |c| c.image);
                    let g = FACE[links as usize];
                    self.pictures.push(Picture {
                        prio: PRIO_FACE,
                        sequence: 0,
                        sheet: Sheet::Face(key, img),
                        player: Player::new(seq0, g[0]),
                        queued: Some(g[1]),
                        parent: self.chars.frames[k],
                        offset: [0.; 2],
                        dead: false,
                    });
                }
            } else {
                self.cache.wanted_face = Some(self.chars.key);
            }
        }
        if self.chars.bounds.is_none()
            && let Some(a) = self.chars.bounds_anchor
        {
            let c = self.sprites.context(a);
            let g = &seq.groups[BOUNDS];
            self.chars.bounds = Some([
                c[0] as i32,
                c[1] as i32,
                g.light_position_x,
                g.text_position_y,
            ]);
        }
        if key_press && !self.container_idle() {
            self.slide(seq, 0, 2);
            self.frame_idle(seq);
        }
        if self.substate == 0 && self.container_idle() {
            self.substate = 1;
            return false;
        }
        // The hand: at the box's centre first.
        if self.chars.hand.is_none_or(|h| !self.sprites.alive(h))
            && let (Some(panel), Some([x0, y0, x1, y1])) = (self.chars.panel, self.chars.bounds)
        {
            if self.chars.cursor == [0, 0] {
                self.chars.cursor = [(x1 - x0) / 2 + x0, (y1 - y0) / 2 + y0];
            }
            let h = self
                .sprites
                .create(seq, PRIO_CURSOR, ART, FREE_HAND, None, None);
            self.sprites.attach(h, panel);
            self.sprites.get_mut(h).die_with_parent = true;
            self.sprites.get_mut(h).offset =
                [self.chars.cursor[0] as f32, self.chars.cursor[1] as f32];
            self.page.push(h);
            self.chars.hand = Some(h);
        }
        self.char_hit();
        // The name plate and the highlight follow the selection.
        let name = (self.chars.sel >= 0)
            .then(|| self.chars.entries[self.chars.sel as usize].character)
            .and_then(|c| self.data.chars.get(c as usize))
            .map(|c| self.msg(c.name))
            .unwrap_or_default();
        self.set_text(self.chars.name, name);
        self.char_highlight(seq);
        if self.held & 0xf0 != 0 {
            self.char_move();
            self.char_hit();
            self.char_highlight(seq);
            return false;
        }
        if pad.trig & bits::START != 0 {
            sounds.push(SOUND_CLOSE);
            self.kill_highlight();
            self.exit = true;
        } else if pad.trig & bits::CANCEL != 0 {
            sounds.push(SOUND_CLOSE);
            self.pill_out(seq);
            if let Some(h) = self.chars.hand.take() {
                self.sprites.kill(h);
            }
            self.hide_help(seq);
            self.kill_highlight();
            self.slide(seq, 1, 1);
            self.pending = Some(if self.state == 0x28 {
                COLLECTION_PAGE
            } else {
                NOTES_PAGE
            });
            self.substate = 2;
        } else if pad.trig & bits::DECIDE != 0 {
            if self.chars.sel < 0 {
                sounds.push(SOUND_INVALID);
            } else {
                // FUN_140331ce0: the Character File page (state 0x10).
                sounds.push(SOUND_DECIDE);
                let rows: Vec<usize> = self
                    .chars
                    .entries
                    .iter()
                    .map(|e| e.character as usize)
                    .collect();
                let sel = self.chars.sel as usize;
                if let Some(h) = self.chars.hand.take() {
                    self.sprites.kill(h);
                }
                self.kill_highlight();
                self.files_from_characters(seq, rows, sel);
            }
        }
        false
    }

    fn kill_highlight(&mut self) {
        if let Some(h) = self.chars.highlight.take() {
            self.sprites.kill(h);
        }
    }

    /// FUN_14032c6a0: the pulse follows the selected frame.
    fn char_highlight(&mut self, seq: &Sequence) {
        let sel = self.chars.sel;
        let alive = self.chars.highlight.is_some_and(|h| self.sprites.alive(h));
        match (alive, sel >= 0) {
            (false, true) => {
                let f = self.chars.frames[sel as usize];
                let g = HIGHLIGHT[self.chars.links as usize];
                let h = self.page_sprite(seq, PRIO_HIGHLIGHT, g, f, None);
                self.chars.highlight = Some(h);
            }
            (true, false) => self.kill_highlight(),
            (true, true) => {
                if let Some(h) = self.chars.highlight {
                    self.sprites.get_mut(h).parent = Some(self.chars.frames[sel as usize]);
                }
            }
            _ => {}
        }
    }
}
